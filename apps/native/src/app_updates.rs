use crate::media;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};

pub const PUBLIC_KEY: &str = include_str!("../../../release/update-public-key.txt");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY: &str = "https://github.com/tugcantopaloglu/game-library";
const RELEASE_API: &str =
    "https://api.github.com/repos/tugcantopaloglu/game-library/releases/latest";
const ASSET_HOSTS: &[&str] = &[
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];
const MAX_MANIFEST: u64 = 16 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateManifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub minimum_bridge_protocol: u32,
    pub signature: String,
}

impl UpdateManifest {
    pub fn signed_payload(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}",
            self.version, self.url, self.sha256, self.size, self.minimum_bridge_protocol
        )
    }

    fn verify_signature(&self, public_key: &str) -> Result<semver::Version, String> {
        let version = stable_version(&self.version)?;
        let expected = format!("{REPOSITORY}/releases/download/v{version}/game-library-native.exe");
        if self.url != expected
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.size < 2
            || self.size > 80 * 1024 * 1024
            || self.minimum_bridge_protocol > 1
        {
            return Err("Güncelleme paketi veya köprü uyumu geçersiz".into());
        }
        let key: [u8; 32] = base64::engine::general_purpose::STANDARD
            .decode(public_key.trim())
            .map_err(|_| "Güncelleme anahtarı geçersiz")?
            .try_into()
            .map_err(|_| "Güncelleme anahtarı boyutu geçersiz")?;
        let signature = base64::engine::general_purpose::STANDARD
            .decode(&self.signature)
            .map_err(|_| "Güncelleme imzası geçersiz")?;
        let signature =
            Signature::from_slice(&signature).map_err(|_| "Güncelleme imzası boyutu geçersiz")?;
        VerifyingKey::from_bytes(&key)
            .map_err(|_| "Güncelleme anahtarı geçersiz")?
            .verify_strict(self.signed_payload().as_bytes(), &signature)
            .map_err(|_| "Güncelleme imzası doğrulanamadı")?;
        Ok(version)
    }

    pub fn verify(&self, public_key: &str, current: &str) -> Result<(), String> {
        if self.verify_signature(public_key)? <= stable_version(current)? {
            return Err("Daha yeni kararlı sürüm yok".into());
        }
        Ok(())
    }

    fn verify_binary(&self, path: &Path) -> Result<(), String> {
        let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        if file.metadata().map_err(|error| error.to_string())?.len() != self.size {
            return Err("Güncelleme dosyasının boyutu değişmiş; yeniden indir.".into());
        }
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut length = 0;
        loop {
            let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            if length == 0 && !buffer[..read].starts_with(b"MZ") {
                return Err("Güncelleme bir Windows uygulaması değil".into());
            }
            length += read as u64;
            if length > self.size {
                return Err("Güncelleme dosyasının boyutu değişmiş".into());
            }
            digest.update(&buffer[..read]);
        }
        if length != self.size
            || format!("{:x}", digest.finalize()) != self.sha256.to_ascii_lowercase()
        {
            return Err("Güncelleme SHA-256 doğrulamasını geçmedi; yeniden indir.".into());
        }
        Ok(())
    }
}

fn stable_version(value: &str) -> Result<semver::Version, String> {
    let version = semver::Version::parse(value).map_err(|_| "Güncelleme sürümü geçersiz")?;
    if !version.pre.is_empty() || !version.build.is_empty() || version.to_string() != value {
        return Err("Yalnızca kararlı sürümler kurulabilir".into());
    }
    Ok(version)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub fn check(data: &Path) -> Result<Option<UpdateManifest>, String> {
    let result = check_release();
    let _ = schedule_check(data, result.is_ok());
    result
}

pub fn check_delay(data: &Path) -> std::time::Duration {
    let next = read_bounded(&data.join("updates/schedule.json"), 1024)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value["next_check"].as_u64())
        .unwrap_or(0);
    std::time::Duration::from_secs(next.saturating_sub(now()).min(24 * 60 * 60))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn schedule_check(data: &Path, success: bool) -> Result<(), String> {
    std::fs::create_dir_all(data.join("updates")).map_err(|error| error.to_string())?;
    atomic_write(
        &data.join("updates/schedule.json"),
        &serde_json::to_vec(
            &serde_json::json!({"next_check":now()+if success {24*60*60}else{15*60}}),
        )
        .map_err(|error| error.to_string())?,
    )
}

fn check_release() -> Result<Option<UpdateManifest>, String> {
    let mut response = match media::agent(30)
        .get(RELEASE_API)
        .header(
            "User-Agent",
            concat!("GameLibrary/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(ureq::Error::StatusCode(403 | 429)) => {
            return Err("GitHub istek sınırına ulaşıldı. Daha sonra yeniden dene.".into())
        }
        Err(error) => return Err(format!("GitHub'a bağlanılamadı: {error}")),
    };
    let bytes = response
        .body_mut()
        .with_config()
        .limit(128 * 1024)
        .read_to_vec()
        .map_err(|error| error.to_string())?;
    let release: Release =
        serde_json::from_slice(&bytes).map_err(|_| "GitHub sürüm bilgisi okunamadı")?;
    let version = release
        .tag_name
        .strip_prefix('v')
        .ok_or("GitHub sürüm etiketi geçersiz")?;
    if release.draft || release.prerelease {
        return Err("GitHub sürümü kararlı değil".into());
    }
    if stable_version(version)? <= stable_version(VERSION)? {
        return Ok(None);
    }
    let manifest_url = format!("{REPOSITORY}/releases/download/v{version}/update.json");
    let manifest_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == "update.json")
        .ok_or("Bu sürümün imzalı güncelleme bildirimi eksik")?;
    if manifest_asset.browser_download_url != manifest_url || manifest_asset.size > MAX_MANIFEST {
        return Err("GitHub güncelleme bildirimi geçersiz".into());
    }
    let bytes = media::download(&manifest_url, ASSET_HOSTS, MAX_MANIFEST)?;
    let manifest: UpdateManifest =
        serde_json::from_slice(&bytes).map_err(|_| "Güncelleme bildirimi okunamadı")?;
    manifest.verify(PUBLIC_KEY, VERSION)?;
    if manifest.version != version
        || !release.assets.iter().any(|asset| {
            asset.name == "game-library-native.exe"
                && asset.size == manifest.size
                && asset.browser_download_url == manifest.url
        })
    {
        return Err("GitHub sürümü ve imzalı paket eşleşmiyor".into());
    }
    Ok(Some(manifest))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Güncelleme kaydı boyut sınırını aşıyor".into());
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let temporary = path.with_extension("part");
    let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())?;
    drop(file);
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    version: String,
}

pub fn rejected(data: &Path, manifest: &UpdateManifest) -> bool {
    read_bounded(&data.join("updates/result.json"), MAX_MANIFEST)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|report| {
            report["success"] == false
                && report["version"] == manifest.version
                && report["sha256"] == manifest.sha256
        })
}

pub fn stage(
    data: &Path,
    manifest: &UpdateManifest,
    progress: impl FnMut(u64, u64),
) -> Result<PathBuf, String> {
    manifest.verify(PUBLIC_KEY, VERSION)?;
    if rejected(data, manifest) {
        return Err(
            "Bu sürüm açılış testini geçemedi. Düzeltilmiş bir sonraki sürüm beklenecek.".into(),
        );
    }
    schedule_check(data, false)?;
    let directory = data.join("updates").join(&manifest.version);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let target = directory.join("game-library-native.exe");
    if manifest.verify_binary(&target).is_err() {
        let bytes = media::download_progress(&manifest.url, ASSET_HOSTS, manifest.size, progress)?;
        atomic_write(&target, &bytes)?;
        manifest.verify_binary(&target)?;
    }
    atomic_write(
        &directory.join("update.json"),
        &serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?,
    )?;
    atomic_write(
        &data.join("updates/pending.json"),
        &serde_json::to_vec(&Pending {
            version: manifest.version.clone(),
        })
        .map_err(|error| error.to_string())?,
    )?;
    schedule_check(data, true)?;
    Ok(target)
}

pub fn pending(data: &Path) -> Result<Option<PathBuf>, String> {
    pending_with_key(data, PUBLIC_KEY, VERSION)
}

fn pending_with_key(data: &Path, key: &str, current: &str) -> Result<Option<PathBuf>, String> {
    let path = data.join("updates/pending.json");
    if !path.exists() {
        return Ok(None);
    }
    let receipt: Pending = serde_json::from_slice(&read_bounded(&path, 1024)?)
        .map_err(|_| "Hazırlanmış güncelleme kaydı okunamadı")?;
    if stable_version(&receipt.version)? <= stable_version(current)? {
        return Ok(None);
    }
    let directory = data.join("updates").join(&receipt.version);
    let manifest = read_manifest(&directory, key, current)?;
    if manifest.version != receipt.version {
        return Err("Güncelleme kaydı sürümle eşleşmiyor".into());
    }
    if rejected(data, &manifest) {
        return Ok(None);
    }
    let target = directory.join("game-library-native.exe");
    manifest.verify_binary(&target)?;
    Ok(Some(target))
}

pub fn status(data: &Path) -> String {
    match pending(data) {
        Ok(Some(path)) => format!(
            "{} indirildi ve doğrulandı. Sonraki açılışta kurulmaya hazır.",
            path.parent()
                .and_then(Path::file_name)
                .unwrap_or_default()
                .to_string_lossy()
        ),
        Err(error) => {
            format!("Hazır güncelleme kullanılamıyor: {error} Kontrol et ile yeniden indir.")
        }
        Ok(None) => read_bounded(&data.join("updates/result.json"), MAX_MANIFEST)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|report| report["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "Yeni sürümler GitHub Releases üzerinden denetlenir.".into()),
    }
}

fn read_manifest(directory: &Path, key: &str, current: &str) -> Result<UpdateManifest, String> {
    let manifest: UpdateManifest =
        serde_json::from_slice(&read_bounded(&directory.join("update.json"), MAX_MANIFEST)?)
            .map_err(|_| "Güncelleme bildirimi okunamadı")?;
    manifest.verify(key, current)?;
    Ok(manifest)
}

pub fn verify_package(directory: &Path) -> Result<UpdateManifest, String> {
    let manifest: UpdateManifest =
        serde_json::from_slice(&read_bounded(&directory.join("update.json"), MAX_MANIFEST)?)
            .map_err(|_| "Güncelleme bildirimi okunamadı")?;
    manifest.verify_signature(PUBLIC_KEY)?;
    manifest.verify_binary(&directory.join("game-library-native.exe"))?;
    Ok(manifest)
}

pub fn request_apply(data: &Path, staged: &Path) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    if executable.file_name().and_then(|name| name.to_str()) != Some("game-library-native.exe") {
        return Err("Güncelleme için uygulamayı game-library-native.exe adıyla çalıştır.".into());
    }
    let directory = staged.parent().ok_or("Güncelleme klasörü yok")?;
    let manifest = read_manifest(directory, PUBLIC_KEY, VERSION)?;
    manifest.verify_binary(staged)?;
    let helper = data.join(format!("updates/updater-helper-{}.exe", std::process::id()));
    let ready = data.join(format!("updates/helper-{}.ready", std::process::id()));
    if ready.exists() {
        std::fs::remove_file(&ready).map_err(|error| error.to_string())?;
    }
    std::fs::copy(&executable, &helper)
        .map_err(|error| format!("Güncelleme yardımcısı hazırlanamadı: {error}"))?;
    let log = std::fs::File::create(data.join("updates/helper.log"))
        .map_err(|error| error.to_string())?;
    let mut child = std::process::Command::new(helper)
        .arg("--apply-update")
        .arg(directory)
        .arg("--update-target")
        .arg(executable)
        .arg("--update-parent")
        .arg(std::process::id().to_string())
        .arg("--data-dir")
        .arg(data)
        .stderr(log)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if ready.is_file() {
            let _ = std::fs::remove_file(&ready);
            return Ok(());
        }
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            let detail = read_bounded(&data.join("updates/helper.log"), 4096).unwrap_or_default();
            return Err(format!(
                "Güncelleme yardımcısı başlatılamadı: {}",
                String::from_utf8_lossy(&detail).trim()
            ));
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "Güncelleme yardımcısı hazırlanamadı. Uygulama açık bırakıldı; yeniden dene."
                    .into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub fn apply(directory: &Path, target: &Path, data: &Path, parent: u32) -> Result<(), String> {
    let manifest = read_manifest(directory, PUBLIC_KEY, VERSION)?;
    let result = apply_verified(directory, target, data, parent, &manifest);
    let message = match &result {
        Ok(_) => format!("{} sürümü başarıyla kuruldu.", manifest.version),
        Err(error) => format!("{} güncellemesi kurulamadı: {error}", manifest.version),
    };
    // Keep a failure receipt even when the old application must be restarted.
    let receipt = serde_json::json!({"success":result.is_ok(),"version":manifest.version,"sha256":manifest.sha256,"message":message,"pid":result.as_ref().ok()});
    atomic_write(
        &data.join("updates/result.json"),
        &serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?,
    )?;
    result.map(|_| ())
}

fn apply_verified(
    directory: &Path,
    target: &Path,
    data: &Path,
    parent: u32,
    manifest: &UpdateManifest,
) -> Result<u32, String> {
    let root = std::fs::canonicalize(data.join("updates")).map_err(|error| error.to_string())?;
    let directory = std::fs::canonicalize(directory).map_err(|error| error.to_string())?;
    let target = std::fs::canonicalize(target).map_err(|error| error.to_string())?;
    if directory != root.join(&manifest.version)
        || target.starts_with(&root)
        || target.file_name().and_then(|name| name.to_str()) != Some("game-library-native.exe")
    {
        return Err("Güncelleme hedefi geçersiz".into());
    }
    let candidate = directory.join("game-library-native.exe");
    manifest.verify_binary(&candidate)?;
    atomic_write(
        &data.join(format!("updates/helper-{parent}.ready")),
        b"ready",
    )?;
    if !platform_windows::sessions::wait_for_exit(parent, 60_000)? {
        return Err("Uygulama kapanmadı; güncelleme uygulanmadı.".into());
    }
    let backup = directory.join("previous.exe");
    let database = data.join("library.db");
    let database_backup = directory.join("library.previous.db");
    let next = target.with_file_name("game-library-native.next.exe");
    let mut replaced = false;
    let result = (|| {
        // VACUUM INTO includes committed WAL contents without running migrations.
        if database_backup.exists() {
            return Err("Önceki güncelleme yedeği korunuyor; bu paket yeniden uygulanmadı.".into());
        }
        library_core::storage::Store::snapshot_file(&database, &database_backup)
            .map_err(|error| format!("Kütüphane yedeklenemedi: {error}"))?;
        std::fs::copy(&target, &backup)
            .map_err(|error| format!("Uygulama yedeklenemedi: {error}"))?;
        std::fs::copy(&candidate, &next).map_err(|error| error.to_string())?;
        manifest.verify_binary(&next)?;
        std::fs::rename(&next, &target)
            .map_err(|error| format!("Uygulama klasörüne yazılamadı: {error}"))?;
        replaced = true;
        launch_and_verify(&target, data, &directory, &manifest.version)
    })();
    let _ = std::fs::remove_file(next);
    if let Err(error) = &result {
        if replaced {
            std::fs::copy(&backup, &target).map_err(|restore| {
                format!(
                    "{error}; uygulama yedeği {} konumunda: {restore}",
                    backup.display()
                )
            })?;
            for suffix in ["-wal", "-shm"] {
                let sidecar = data.join(format!("library.db{suffix}"));
                if sidecar.exists() {
                    std::fs::remove_file(sidecar).map_err(|restore| restore.to_string())?;
                }
            }
            if database_backup.exists() {
                std::fs::copy(&database_backup, &database).map_err(|restore| {
                    format!(
                        "Kütüphane yedeği {} konumunda: {restore}",
                        database_backup.display()
                    )
                })?;
            } else if database.exists() {
                std::fs::remove_file(&database).map_err(|error| error.to_string())?;
            }
        }
        // Publish the failure before reopening, so automatic startup cannot loop.
        let receipt = serde_json::json!({"success":false,"version":manifest.version,"sha256":manifest.sha256,"message":format!("{error}; önceki sürüm ve kütüphane geri yüklendi.")});
        atomic_write(
            &data.join("updates/result.json"),
            &serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?,
        )?;
        std::process::Command::new(&target)
            .arg("--data-dir")
            .arg(data)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|restart| format!("Önceki sürüm geri yüklendi; yeniden aç: {restart}"))?;
        return Err(format!("{error}; önceki sürüm ve kütüphane geri yüklendi."));
    }
    result
}

fn launch_and_verify(
    target: &Path,
    data: &Path,
    directory: &Path,
    version: &str,
) -> Result<u32, String> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|error| error.to_string())?;
    let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let health = directory.join(format!("health-{token}.json"));
    let mut child = std::process::Command::new(target)
        .arg("--data-dir")
        .arg(data)
        .arg("--update-health-report")
        .arg(&health)
        .arg("--update-health-token")
        .arg(&token)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let result = loop {
        match child.try_wait() {
            Ok(Some(_)) => break Err("Yeni sürüm hazır olmadan kapandı".into()),
            Err(error) => break Err(error.to_string()),
            Ok(None) => {}
        }
        if read_bounded(&health, 4096)
            .is_ok_and(|bytes| valid_health(&bytes, version, child.id(), &token))
        {
            break Ok(child.id());
        }
        if std::time::Instant::now() >= deadline {
            break Err("Yeni sürüm açılış doğrulaması zaman aşımına uğradı".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_file(health);
    result
}

pub(crate) fn valid_health(bytes: &[u8], version: &str, pid: u32, token: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes).is_ok_and(|report| {
        report["ready"] == true
            && report["version"] == version
            && report["pid"] == pid
            && report["token"] == token
    })
}

#[cfg(test)]
#[path = "app_updates_tests.rs"]
mod tests;
