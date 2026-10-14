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
#[derive(Default, Deserialize, Serialize)]
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
    !id.is_empty()
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
    let destination = data.join("extensions").join(&extension.manifest.id);
    if destination.exists() {
        return Err("Bu eklenti zaten kurulu. Yeni sürüm için önce kaldır.".into());
    }
    std::fs::create_dir_all(data.join("extensions")).map_err(|error| error.to_string())?;
    if std::fs::read_dir(data.join("extensions"))
        .map_err(|error| error.to_string())?
        .count()
        >= 64
    {
        return Err("En fazla 64 eklenti kurulabilir".into());
    }
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
    let result = (|| {
        std::fs::create_dir(&destination).map_err(|error| error.to_string())?;
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
        let mut installed = load(&destination)?;
        installed.preferences = Preferences::default();
        save(&installed)?;
        Ok(extension.manifest.id.clone())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&destination);
    }
    result
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
    std::fs::remove_dir_all(extension.directory).map_err(|error| error.to_string())
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
    if extension
        .manifest
        .capabilities
        .iter()
        .any(|method| method == "metadata.lookup")
    {
        let page = store
            .list_filtered("", "all", 0, 100, None, None)
            .map_err(|error| error.to_string())?;
        let targets:Vec<_>=page.games.iter().map(|game|json!({"id":game.id,"name":game.name,"source":game.source.as_str(),"sourceId":game.source_id})).collect();
        let result = invoke(
            &extension,
            "metadata.lookup",
            json!({"protocolVersion":1,"pluginId":id,"games":targets}),
            Duration::from_secs(15),
        )?;
        let rows = result["metadata"]
            .as_array()
            .filter(|rows| rows.len() <= 100)
            .ok_or("Eklenti metadata listesi geçersiz")?;
        let mut updates = vec![];
        for row in rows {
            let game = page
                .games
                .iter()
                .find(|game| row["id"] == game.id)
                .ok_or("Eklenti istenmeyen oyun metadata'sı gönderdi")?;
            let url = row["artworkUrl"]
                .as_str()
                .ok_or("Eklenti kapak adresi eksik")?;
            let parsed = url::Url::parse(url).map_err(|_| "Eklenti kapak adresi geçersiz")?;
            if url.len() > 4096
                || parsed.scheme() != "https"
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err("Eklenti kapak adresi geçersiz".into());
            }
            updates.push((game.source, game.source_id.clone(), url.to_owned()));
        }
        for (source, key, url) in updates {
            store
                .set_artwork_url(source, &key, &url)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(format!(
        "{} tamamlandı · {count} oyun",
        extension.manifest.name
    ))
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
