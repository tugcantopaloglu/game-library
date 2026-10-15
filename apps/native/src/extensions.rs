use library_core::{
    models::{Game, LaunchTarget, Source},
    storage::Store,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::os::windows::process::CommandExt;
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

const MAX_PACKET: u64 = 4 * 1024 * 1024;
const METHODS: [&str; 2] = ["games.discover", "metadata.lookup"];

struct ProcessJob(windows::Win32::Foundation::HANDLE);
impl ProcessJob {
    fn attach(child: &std::process::Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::System::JobObjects::*;
        unsafe {
            let job = Self(
                CreateJobObjectW(None, None).map_err(|_| "Eklenti süreç grubu oluşturulamadı")?,
            );
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
            .map_err(|_| "Eklenti süreç sınırı uygulanamadı")?;
            AssignProcessToJobObject(
                job.0,
                windows::Win32::Foundation::HANDLE(child.as_raw_handle()),
            )
            .map_err(|_| "Eklenti süreç grubu bağlanamadı")?;
            Ok(job)
        }
    }
}
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub protocol_version: u32,
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Preferences {
    pub enabled: bool,
    pub roots: Vec<PathBuf>,
}
pub struct Extension {
    pub manifest: Manifest,
    pub directory: PathBuf,
    pub preferences: Preferences,
}

fn valid_id(id: &str) -> bool {
    id.as_bytes().first().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && id.len() <= 64
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
}
fn bounded_json(path: &Path, limit: u64) -> Result<Value, String> {
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Eklenti dosyası boyut sınırını aşıyor".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Eklenti JSON dosyası geçersiz".into())
}
fn load(directory: &Path) -> Result<Extension, String> {
    let manifest: Manifest =
        serde_json::from_value(bounded_json(&directory.join("extension.json"), 16384)?)
            .map_err(|_| "Eklenti bildirimi geçersiz".to_string())?;
    if !valid_id(&manifest.id)
        || manifest.name.trim().is_empty()
        || manifest.name.len() > 128
        || manifest.name.contains('\0')
        || manifest.protocol_version != 1
        || semver::Version::parse(&manifest.version).is_err()
        || manifest.capabilities.is_empty()
        || manifest
            .capabilities
            .iter()
            .any(|method| !METHODS.contains(&method.as_str()))
        || manifest
            .permissions
            .iter()
            .any(|permission| !["filesystem.read", "network"].contains(&permission.as_str()))
        || manifest.args.len() > 16
        || manifest
            .args
            .iter()
            .any(|arg| arg.len() > 1024 || arg.contains('\0'))
    {
        return Err("Eklenti sürümü, yetenekleri veya izinleri desteklenmiyor".into());
    }
    let relative = Path::new(&manifest.entrypoint);
    if relative.is_absolute()
        || manifest.entrypoint.contains(':')
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || relative
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("exe"))
    {
        return Err("Eklenti executable yolu geçersiz".into());
    }
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let executable = directory
        .join(relative)
        .canonicalize()
        .map_err(|_| "Eklenti executable dosyası bulunamadı".to_string())?;
    if !executable.starts_with(&directory) || !executable.is_file() {
        return Err("Eklenti executable dosyası paket dışında".into());
    }
    let preferences = if directory.join("preferences.json").exists() {
        serde_json::from_value(bounded_json(&directory.join("preferences.json"), 65536)?)
            .map_err(|_| "Eklenti tercihleri geçersiz".to_string())?
    } else {
        Preferences::default()
    };
    Ok(Extension {
        manifest,
        directory,
        preferences,
    })
}
pub fn list(data: &Path) -> Vec<Result<Extension, String>> {
    let Ok(entries) = std::fs::read_dir(data.join("extensions")) else {
        return vec![];
    };
    let mut entries: Vec<_> = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_type()
                .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
                && valid_id(&entry.file_name().to_string_lossy())
        })
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    entries
        .into_iter()
        .take(64)
        .map(|entry| {
            let extension = load(&entry.path())?;
            if entry.file_name().to_string_lossy() != extension.manifest.id {
                return Err("Eklenti klasörü ve kimliği uyuşmuyor".into());
            }
            Ok(extension)
        })
        .collect()
}
fn get(data: &Path, id: &str) -> Result<Extension, String> {
    if !valid_id(id) {
        return Err("Eklenti kimliği geçersiz".into());
    }
    let extension = load(&data.join("extensions").join(id))?;
    if extension.manifest.id != id {
        return Err("Eklenti kimliği uyuşmuyor".into());
    }
    Ok(extension)
}
fn save(extension: &Extension) -> Result<(), String> {
    std::fs::write(
        extension.directory.join("preferences.json"),
        serde_json::to_vec(&extension.preferences).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}
pub fn enable(data: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let mut extension = get(data, id)?;
    extension.preferences.enabled = enabled;
    save(&extension)
}
pub fn add_root(data: &Path, id: &str, root: &Path) -> Result<(), String> {
    let mut extension = get(data, id)?;
    if !extension
        .manifest
        .permissions
        .iter()
        .any(|permission| permission == "filesystem.read")
    {
        return Err("Bu eklenti klasör erişimi tanımlamıyor".into());
    }
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !root.is_dir() || extension.preferences.roots.len() >= 32 {
        return Err("Eklenti klasör sınırı aşıldı".into());
    }
    if !extension.preferences.roots.contains(&root) {
        extension.preferences.roots.push(root);
    }
    save(&extension)
}
pub fn install(data: &Path, source: &Path) -> Result<String, String> {
    let extension = load(source)?;
    let destination = owned_path(&data.join("extensions"), &extension.manifest.id)?;
    if destination.exists() {
        return Err("Bu eklenti zaten kurulu. Sürüm yükseltmek için Güncelle kullan.".into());
    }
    std::fs::create_dir_all(data.join("extensions")).map_err(|error| error.to_string())?;
    if std::fs::read_dir(data.join("extensions"))
        .map_err(|error| error.to_string())?
        .count()
        >= 64
    {
        return Err("En fazla 64 eklenti kurulabilir".into());
    }
    copy_package(&extension, &destination, Preferences::default())?;
    Ok(extension.manifest.id)
}

// Only immediate children of a canonical, host-owned directory may be replaced.
fn owned_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || Path::new(name).components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err("Eklenti paket yolu geçersiz".into());
    }
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    use std::os::windows::fs::MetadataExt;
    if root.symlink_metadata().map_err(|error| error.to_string())?.file_attributes() & 0x400 != 0 { return Err("Eklenti ana klasörü junction olamaz".into()); }
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let path = root.join(name);
    if path.exists() {
        use std::os::windows::fs::MetadataExt;
        let metadata = path.symlink_metadata().map_err(|error| error.to_string())?;
        if metadata.file_attributes() & 0x400 != 0 || path.canonicalize().map_err(|error| error.to_string())?.parent() != Some(root.as_path()) {
            return Err("Eklenti paket yolu sahipliği doğrulanamadı".into());
        }
    }
    Ok(path)
}
fn remove_owned(root: &Path, name: &str) -> Result<(), String> {
    let path = owned_path(root, name)?;
    if path.exists() { std::fs::remove_dir_all(path).map_err(|error| error.to_string())?; }
    Ok(())
}
fn copy_package(extension: &Extension, destination: &Path, preferences: Preferences) -> Result<(), String> {
    let mut files = vec![];
    let mut pending = vec![extension.directory.clone()];
    let mut size = 0u64;
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            visited += 1;
            if visited > 512 {
                return Err("Eklenti paket dosya sayısı sınırını aşıyor".into());
            }
            let metadata = entry
                .path()
                .symlink_metadata()
                .map_err(|error| error.to_string())?;
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Eklenti paketi bağlantı veya junction içeremez".into());
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                size += metadata.len();
                files.push(entry.path());
            }
            if files.len() + pending.len() > 512 || size > 128 * 1024 * 1024 {
                return Err("Eklenti paket sınırı aşıldı".into());
            }
        }
    }
    std::fs::create_dir(destination).map_err(|error| error.to_string())?;
    let result = (|| {
        for file in files {
            let relative = file
                .strip_prefix(&extension.directory)
                .map_err(|error| error.to_string())?;
            if relative == Path::new("preferences.json") {
                continue;
            }
            let target = destination.join(relative);
            std::fs::create_dir_all(target.parent().ok_or("Eklenti yolu geçersiz")?)
                .map_err(|error| error.to_string())?;
            std::fs::copy(&file, &target).map_err(|error| error.to_string())?;
        }
        let mut installed = load(destination)?;
        installed.preferences = preferences;
        save(&installed)
    })();
    if result.is_err() {
        if let (Some(root), Some(name)) = (destination.parent(), destination.file_name().and_then(|name| name.to_str())) {
            let _ = remove_owned(root, name);
        }
    }
    result
}

pub fn previous_version(data: &Path, id: &str) -> Option<String> {
    if !valid_id(id) { return None; }
    let path = owned_path(&data.join("extension-backups"), id).ok()?;
    let extension = load(&path).ok()?;
    (extension.manifest.id == id).then_some(extension.manifest.version)
}

/// Local packages are staged and validated before the working installation moves.
pub fn update(data: &Path, id: &str, source: &Path) -> Result<String, String> {
    let current = get(data, id)?;
    let next = load(source)?;
    if next.manifest.id != id || semver::Version::parse(&next.manifest.version).map_err(|error| error.to_string())? <= semver::Version::parse(&current.manifest.version).map_err(|error| error.to_string())? {
        return Err("Aynı kimliğe sahip daha yeni bir eklenti sürümü seç".into());
    }
    let root = data.join("extensions");
    let backups = data.join("extension-backups");
    let installed = owned_path(&root, id)?;
    let staged = owned_path(&root, &format!("_update-{id}"))?;
    if staged.exists() { return Err("Önceki güncelleme paketi bulunuyor; eklenti klasörünü kontrol et".into()); }
    let backup = owned_path(&backups, id)?;
    let mut preferences = current.preferences.clone();
    let new_access = next.manifest.permissions.iter().any(|item| !current.manifest.permissions.contains(item))
        || next.manifest.capabilities.iter().any(|item| !current.manifest.capabilities.contains(item));
    if new_access { preferences.enabled = false; }
    copy_package(&next, &staged, preferences)?;
    // Keep the older rollback until the new package is completely staged.
    if let Err(error) = remove_owned(&backups, id) { let _ = remove_owned(&root, &format!("_update-{id}")); return Err(error); }
    if let Err(error) = std::fs::rename(&installed, &backup) { let _ = remove_owned(&root, &format!("_update-{id}")); return Err(error.to_string()); }
    if let Err(error) = std::fs::rename(&staged, &installed) {
        std::fs::rename(&backup, &installed).map_err(|restore| format!("Güncelleme başarısız: {error}. Önceki paket {} konumunda: {restore}", backup.display()))?;
        let _ = remove_owned(&root, &format!("_update-{id}"));
        return Err(error.to_string());
    }
    Ok(format!("{} → {} güncellendi. {}", current.manifest.version, next.manifest.version, if new_access { "İzinler değişti; yeniden etkinleştirmeden önce izinleri incele." } else { "Ayarlar korundu; önceki sürüme dönebilirsin." }))
}

pub fn rollback(data: &Path, id: &str) -> Result<String, String> {
    let current = get(data, id)?;
    if previous_version(data, id).is_none() { return Err("Geri dönülebilecek sürüm yok".into()); }
    let root = data.join("extensions");
    let backup = owned_path(&data.join("extension-backups"), id)?;
    let installed = owned_path(&root, id)?;
    let temporary = owned_path(&root, &format!("_rollback-{id}"))?;
    if temporary.exists() { return Err("Önceki sürüme dönüş tamamlanmamış".into()); }
    // Preserve current root choices; do not silently enable permissions on rollback.
    let mut previous = load(&backup)?;
    previous.preferences = current.preferences;
    if previous.manifest.permissions.iter().any(|item| !current.manifest.permissions.contains(item)) { previous.preferences.enabled = false; }
    save(&previous)?;
    std::fs::rename(&installed, &temporary).map_err(|error| error.to_string())?;
    if let Err(error) = std::fs::rename(&backup, &installed) {
        std::fs::rename(&temporary, &installed).map_err(|restore| format!("Geri dönüş başarısız: {error}; mevcut paket {} konumunda: {restore}", temporary.display()))?;
        return Err(error.to_string());
    }
    remove_owned(&root, &format!("_rollback-{id}"))?;
    Ok(format!("{} sürümüne dönüldü; ayarlar korundu.", previous.manifest.version))
}
pub fn remove(data: &Path, id: &str) -> Result<(), String> {
    let extension = get(data, id)?;
    let root = data
        .join("extensions")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if extension.directory.parent() != Some(root.as_path()) {
        return Err("Eklenti silme yolu doğrulanamadı".into());
    }
    remove_owned(&data.join("extension-backups"), id)?;
    remove_owned(&data.join("extensions"), id)
}
fn invoke(
    extension: &Extension,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    if !extension.preferences.enabled
        || !extension
            .manifest
            .capabilities
            .iter()
            .any(|capability| capability == method)
    {
        return Err("Eklenti kapalı veya bu yeteneği desteklemiyor".into());
    }
    let packet =
        serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .map_err(|error| error.to_string())?;
    if packet.len() > 65536 {
        return Err("Eklenti istek sınırı aşıldı".into());
    }
    let mut child =
        std::process::Command::new(extension.directory.join(&extension.manifest.entrypoint))
            .args(&extension.manifest.args)
            .current_dir(&extension.directory)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "Eklenti işlemi başlatılamadı".to_string())?;
    let _job = match ProcessJob::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut input = child.stdin.take().ok_or("Eklenti girişi açılamadı")?;
    let output = child.stdout.take().ok_or("Eklenti çıkışı açılamadı")?;
    std::thread::spawn(move || {
        let _ = input.write_all(&packet);
        let _ = input.write_all(b"\n");
    });
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = vec![];
        let result = output
            .take(MAX_PACKET + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let started = std::time::Instant::now();
    let result = receiver.recv_timeout(timeout);
    let exit = loop {
        let exit = child.try_wait().ok().flatten();
        if exit.is_some() || started.elapsed() >= timeout {
            break exit;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if exit.is_none() {
        let _ = child.kill();
    }
    let waited = child.wait().ok();
    let bytes = result
        .map_err(|_| "Eklenti zaman aşımına uğradı; işlem kapatıldı".to_string())?
        .map_err(|_| "Eklenti yanıtı okunamadı".to_string())?;
    if bytes.len() as u64 > MAX_PACKET {
        return Err("Eklenti yanıtı boyut sınırını aşıyor".into());
    }
    if !exit.or(waited).is_some_and(|status| status.success()) {
        return Err("Eklenti işlemi başarısız oldu".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Eklenti yanıtı geçersiz".to_string())?;
    if value["jsonrpc"] != "2.0"
        || value["id"] != 1
        || !value["error"].is_null()
        || value["result"]["protocolVersion"] != 1
        || value["result"]["pluginId"] != extension.manifest.id
    {
        return Err("Eklenti protokolü veya kimliği uyuşmuyor".into());
    }
    Ok(value["result"].clone())
}
pub fn run(data: &Path, id: &str, store: &mut Store) -> Result<String, String> {
    let extension = get(data, id)?;
    let mut count = 0;
    if extension
        .manifest
        .capabilities
        .iter()
        .any(|method| method == "games.discover")
    {
        let result = invoke(
            &extension,
            "games.discover",
            json!({"protocolVersion":1,"pluginId":id,"roots":extension.preferences.roots}),
            Duration::from_secs(15),
        )?;
        let rows = result["games"]
            .as_array()
            .filter(|rows| rows.len() <= 2000)
            .ok_or("Eklenti oyun listesi geçersiz")?;
        let mut games = vec![];
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            let key = row["id"]
                .as_str()
                .filter(|id| valid_id(id))
                .ok_or("Eklenti oyun kimliği geçersiz")?;
            if !seen.insert(key) {
                return Err("Eklenti yinelenen oyun kimliği gönderdi".into());
            }
            let name = row["name"]
                .as_str()
                .filter(|name| !name.trim().is_empty() && name.len() <= 512 && !name.contains('\0'))
                .ok_or("Eklenti oyun adı geçersiz")?;
            let executable = PathBuf::from(
                row["executable"]
                    .as_str()
                    .ok_or("Eklenti executable yolu eksik")?,
            )
            .canonicalize()
            .map_err(|_| "Eklenti oyun dosyası bulunamadı")?;
            if !executable.is_file()
                || executable
                    .extension()
                    .is_none_or(|ext| !ext.eq_ignore_ascii_case("exe"))
                || !extension.preferences.roots.iter().any(|root| {
                    root.canonicalize()
                        .is_ok_and(|root| executable.starts_with(root))
                })
            {
                return Err("Eklenti oyun yolu seçilen klasörlerin dışında".into());
            }
            let directory = executable
                .parent()
                .ok_or("Oyun klasörü eksik")?
                .to_string_lossy()
                .to_string();
            games.push(Game {
                id: format!("extension:{id}:{key}"),
                source: Source::Manual,
                source_id: format!("{id}:{key}"),
                name: name.into(),
                app_id: 0,
                launch: LaunchTarget::Manual {
                    executable: executable.to_string_lossy().into(),
                    args: vec![],
                    working_dir: directory.clone(),
                },
                library_path: format!("extension://{id}"),
                install_path: directory,
                installed: true,
                favorite: false,
                size_bytes: 0.0,
                artwork_path: None,
            });
        }
        count = games.len();
        store
            .apply_extension_games(id, &games)
            .map_err(|error| error.to_string())?;
    }
    let mut metadata = (0, 0, 0);
    if extension.manifest.capabilities.iter().any(|method| method == "metadata.lookup") {
        metadata = metadata_pages(store, |params| invoke(&extension, "metadata.lookup", params, Duration::from_secs(15)), id)?;
    }
    Ok(format!("{} tamamlandı · {count} oyun · {} metadata kaydı / {} oyun, {} sayfa", extension.manifest.name, metadata.1, metadata.0, metadata.2))
}

fn metadata_pages(store: &mut Store, mut call: impl FnMut(Value) -> Result<Value, String>, id: &str) -> Result<(usize, usize, usize), String> {
    let through = store.metadata_upper_bound().map_err(|error| error.to_string())?;
    let mut after = String::new();
    let (mut checked, mut updated, mut pages) = (0, 0, 0);
    loop {
        let games = store.metadata_page(&after, &through).map_err(|error| error.to_string())?;
        if games.is_empty() { break; }
        let targets: Vec<_> = games.iter().map(|game| json!({"id":game.id,"name":game.name,"source":game.source.as_str(),"sourceId":game.source_id})).collect();
        let result = call(json!({"protocolVersion":1,"pluginId":id,"games":targets,"page":{"after":after,"limit":100}}))
            .map_err(|error| format!("{checked} oyun / {pages} sayfa işlendi; kalan metadata tamamlanamadı: {error}"))?;
        let rows = result["metadata"].as_array().filter(|rows| rows.len() <= games.len()).ok_or("Eklenti metadata sayfası geçersiz")?;
        let mut updates = vec![];
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            let game = games.iter().find(|game| row["id"] == game.id).ok_or("Eklenti istenmeyen oyun metadata'sı gönderdi")?;
            if !seen.insert(game.id.as_str()) { return Err("Eklenti yinelenen metadata gönderdi".into()); }
            let url = row["artworkUrl"].as_str().ok_or("Eklenti kapak adresi eksik")?;
            let parsed = url::Url::parse(url).map_err(|_| "Eklenti kapak adresi geçersiz")?;
            if url.len() > 4096 || parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() {
                return Err("Eklenti kapak adresi geçersiz".into());
            }
            updates.push((game.source, game.source_id.clone(), url.to_owned()));
        }
        store.set_artwork_urls(&updates).map_err(|error| error.to_string())?;
        after = games.last().unwrap().id.clone();
        checked += games.len(); updated += updates.len(); pages += 1;
    }
    Ok((checked, updated, pages))
}

pub fn create_example(directory: &Path) -> Result<(), String> {
    std::fs::create_dir(directory).map_err(|error| error.to_string())?;
    std::fs::copy(
        std::env::current_exe().map_err(|error| error.to_string())?,
        directory.join("local-library.exe"),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(directory.join("extension.json"),json!({"id":"local-library","name":"Yerel oyun listesi","version":"1.0.0","protocolVersion":1,"entrypoint":"local-library.exe","args":["--extension-example"],"capabilities":["games.discover"],"permissions":["filesystem.read"]}).to_string()).map_err(|error|error.to_string())
}
pub fn example_process() -> Result<(), String> {
    let mut input = vec![];
    std::io::stdin()
        .take(65537)
        .read_to_end(&mut input)
        .map_err(|error| error.to_string())?;
    if input.len() > 65536 {
        return Err("İstek sınırı aşıldı".into());
    }
    let request: Value = serde_json::from_slice(&input).map_err(|error| error.to_string())?;
    if request["method"] != "games.discover" || request["params"]["protocolVersion"] != 1 {
        return Err("Desteklenmeyen eklenti isteği".into());
    }
    let mut games = vec![];
    for root in request["params"]["roots"]
        .as_array()
        .ok_or("Klasörler eksik")?
        .iter()
        .take(32)
    {
        let root = PathBuf::from(root.as_str().ok_or("Klasör geçersiz")?);
        let path = root.join("games.json");
        if path.is_file() {
            games.extend(
                bounded_json(&path, MAX_PACKET)?
                    .as_array()
                    .ok_or("Oyun listesi geçersiz")?
                    .iter()
                    .cloned(),
            );
        }
    }
    std::io::stdout().write_all(json!({"jsonrpc":"2.0","id":request["id"],"result":{"protocolVersion":1,"pluginId":"local-library","games":games}}).to_string().as_bytes()).map_err(|error|error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(command: &str) -> (tempfile::TempDir, Extension) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::copy(
            PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe"),
            directory.path().join("helper.exe"),
        )
        .unwrap();
        std::fs::write(directory.path().join("extension.json"),json!({"id":"fixture","name":"Fixture","version":"1.0.0","protocolVersion":1,"entrypoint":"helper.exe","args":["/D","/C",command],"capabilities":["games.discover"]}).to_string()).unwrap();
        let mut extension = load(directory.path()).unwrap();
        extension.preferences.enabled = true;
        (directory, extension)
    }
    fn rewrite_manifest(directory: &Path, version: &str, permissions: Vec<&str>) {
        let mut manifest = load(directory).unwrap().manifest;
        manifest.version = version.into(); manifest.permissions = permissions.into_iter().map(String::from).collect();
        std::fs::write(directory.join("extension.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    }
    #[test]
    fn updates_preserve_preferences_and_rollback_without_enabling_new_access() {
        let (package, _) = fixture("exit /b 0");
        let data=tempfile::tempdir().unwrap();
        rewrite_manifest(package.path(),"1.0.0",vec!["filesystem.read"]);
        install(data.path(),package.path()).unwrap();
        add_root(data.path(),"fixture",package.path()).unwrap(); enable(data.path(),"fixture",true).unwrap();
        rewrite_manifest(package.path(),"1.1.0",vec!["filesystem.read"]);
        update(data.path(),"fixture",package.path()).unwrap();
        let current=get(data.path(),"fixture").unwrap(); assert!(current.preferences.enabled); assert_eq!(current.preferences.roots.len(),1);
        assert_eq!(previous_version(data.path(),"fixture").as_deref(),Some("1.0.0"));
        assert!(update(data.path(),"fixture",package.path()).is_err());
        rewrite_manifest(package.path(),"1.2.0",vec!["filesystem.read","network"]);
        update(data.path(),"fixture",package.path()).unwrap(); assert!(!get(data.path(),"fixture").unwrap().preferences.enabled);
        rollback(data.path(),"fixture").unwrap(); let current=get(data.path(),"fixture").unwrap();
        assert_eq!(current.manifest.version,"1.1.0"); assert!(!current.preferences.enabled); assert_eq!(current.preferences.roots.len(),1);
        assert!(previous_version(data.path(),"fixture").is_none());
        assert!(!valid_id("..")); assert!(update(data.path(),"..",package.path()).is_err());
    }
    #[test]
    fn metadata_covers_the_entire_library_and_rejects_a_bad_page_atomically() {
        let directory=tempfile::tempdir().unwrap(); let mut store=Store::open(&directory.path().join("library.db")).unwrap();
        let games:Vec<_>=(0..257).map(|index| Game {
            id:format!("extension:fixture:{index:04}"), source:Source::Manual,source_id:format!("fixture:{index:04}"),name:format!("Game {index}"), app_id:0,
            launch:LaunchTarget::Manual{executable:"C:/Games/game.exe".into(),args:vec![],working_dir:"C:/Games".into()},library_path:"extension://fixture".into(),install_path:"C:/Games".into(),installed:true,favorite:false,size_bytes:0.0,artwork_path:None
        }).collect();
        store.apply_extension_games("fixture",&games).unwrap();
        store.set_favorite(&games[0].id,true).unwrap();
        let result=metadata_pages(&mut store,|params| {
            let rows=params["games"].as_array().unwrap(); assert!(rows.len()<=100);
            Ok(json!({"metadata":rows.iter().map(|row|json!({"id":row["id"],"artworkUrl":"https://example.com/cover.png"})).collect::<Vec<_>>()}))
        },"fixture").unwrap();
        assert_eq!(result,(257,257,3)); assert!(store.game(&games[0].id).unwrap().favorite);
        let before=store.setting("cover-url:manual:fixture:0000").unwrap();
        assert!(metadata_pages(&mut store,|params|Ok(json!({"metadata":[{"id":params["games"][0]["id"],"artworkUrl":"https://example.com/new.png"},{"id":"unknown","artworkUrl":"https://example.com/no.png"}]})),"fixture").is_err());
        assert_eq!(store.setting("cover-url:manual:fixture:0000").unwrap(),before);
    }
    #[test]
    fn process_responses_require_matching_protocol_and_identity() {
        let (directory, extension) = fixture("type response.json");
        let response = directory.path().join("response.json");
        std::fs::write(&response,json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"pluginId":"fixture","games":[]}}).to_string()).unwrap();
        invoke(
            &extension,
            "games.discover",
            json!({}),
            Duration::from_secs(2),
        )
        .unwrap();
        std::fs::write(&response,json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":2,"pluginId":"fixture","games":[]}}).to_string()).unwrap();
        assert!(invoke(
            &extension,
            "games.discover",
            json!({}),
            Duration::from_secs(2)
        )
        .is_err());
        std::fs::write(&response,json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"pluginId":"another","games":[]}}).to_string()).unwrap();
        assert!(invoke(
            &extension,
            "games.discover",
            json!({}),
            Duration::from_secs(2)
        )
        .is_err());
        let mut extension = extension;
        extension.preferences.enabled = false;
        assert!(invoke(
            &extension,
            "games.discover",
            json!({}),
            Duration::from_secs(2)
        )
        .is_err());
    }
    #[test]
    fn crashes_and_timeouts_are_contained() {
        let (_directory, extension) = fixture("exit /b 7");
        assert!(invoke(
            &extension,
            "games.discover",
            json!({}),
            Duration::from_secs(2)
        )
        .is_err());
        let (_directory, extension) =
            fixture("powershell -NoProfile -NonInteractive -Command Start-Sleep -Seconds 10");
        let started = std::time::Instant::now();
        assert!(invoke(
            &extension,
            "games.discover",
            json!({}),
            Duration::from_millis(150)
        )
        .unwrap_err()
        .contains("zaman aşımı"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn imported_packages_start_disabled_and_invalid_entrypoints_are_rejected() {
        let (directory, mut extension) = fixture("exit /b 0");
        let data = tempfile::tempdir().unwrap();
        install(data.path(), directory.path()).unwrap();
        assert!(!get(data.path(), "fixture").unwrap().preferences.enabled);
        extension.manifest.entrypoint = "../helper.exe".into();
        std::fs::write(
            directory.path().join("extension.json"),
            serde_json::to_vec(&extension.manifest).unwrap(),
        )
        .unwrap();
        assert!(load(directory.path()).is_err());
        assert!(remove(data.path(), "../fixture").is_err());
        remove(data.path(), "fixture").unwrap();
    }
}
