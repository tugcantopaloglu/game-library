use crate::media;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const ASSET_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

/// Node/Electron module loaders and command-line tools need regular DOS/UNC paths.
pub fn external_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        value.trim_start_matches("\\\\?\\").to_string()
    }
}
pub const MANIFEST_URL: &str =
    "https://github.com/tugcantopaloglu/game-library/releases/latest/download/update.json";

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
    pub fn verify(&self, public_key: &str, current: &str) -> Result<(), String> {
        let version =
            semver::Version::parse(&self.version).map_err(|_| "Güncelleme sürümü geçersiz")?;
        if version <= semver::Version::parse(current).map_err(|_| "Uygulama sürümü geçersiz")?
            || !version.pre.is_empty()
        {
            return Err("Daha yeni kararlı sürüm yok".into());
        }
        let url = media::trusted_url(&self.url, &["github.com"])?;
        if !url
            .path()
            .starts_with("/tugcantopaloglu/game-library/releases/download/")
            || !url.path().ends_with("/game-library-native.exe")
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.size == 0
            || self.size > 80 * 1024 * 1024
            || self.minimum_bridge_protocol > 1
        {
            return Err("Güncelleme paketi veya köprü uyumu geçersiz".into());
        }
        let key = base64::engine::general_purpose::STANDARD
            .decode(public_key)
            .map_err(|_| "Güncelleme anahtarı geçersiz")?;
        let key: [u8; 32] = key
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
            .map_err(|_| "Güncelleme imzası doğrulanamadı".to_string())
    }
}

pub fn check() -> Result<UpdateManifest, String> {
    let key = option_env!("GAME_LIBRARY_UPDATE_PUBLIC_KEY")
        .ok_or("Bu geliştirme derlemesinde güncelleme imza anahtarı tanımlı değil")?;
    let bytes = media::download(MANIFEST_URL, ASSET_HOSTS, 16 * 1024)?;
    let manifest: UpdateManifest =
        serde_json::from_slice(&bytes).map_err(|_| "Güncelleme bildirimi okunamadı")?;
    manifest.verify(key, env!("CARGO_PKG_VERSION"))?;
    Ok(manifest)
}

pub fn stage(data: &Path, manifest: &UpdateManifest) -> Result<PathBuf, String> {
    manifest.verify(
        option_env!("GAME_LIBRARY_UPDATE_PUBLIC_KEY").ok_or("Güncelleme anahtarı yok")?,
        env!("CARGO_PKG_VERSION"),
    )?;
    let bytes = media::download(&manifest.url, ASSET_HOSTS, manifest.size)?;
    if bytes.len() as u64 != manifest.size
        || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256.to_lowercase()
        || !bytes.starts_with(b"MZ")
    {
        return Err("Güncelleme dosyası boyut veya SHA-256 doğrulamasını geçmedi".into());
    }
    let directory = data.join("updates").join(&manifest.version);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let target = directory.join("game-library-native.exe");
    std::fs::write(&target, bytes).map_err(|error| error.to_string())?;
    std::fs::write(
        directory.join("update.json"),
        serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(target)
}

pub fn request_apply(data: &Path, staged: &Path) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    if executable.file_name().and_then(|name| name.to_str()) != Some("game-library-native.exe") {
        return Err("Güncelleme yalnızca native release executable üzerinde uygulanabilir".into());
    }
    let helper = data.join("updates/updater-helper.exe");
    std::fs::copy(&executable, &helper).map_err(|error| error.to_string())?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new(helper)
        .arg("--apply-update")
        .arg(staged.parent().ok_or("Güncelleme klasörü yok")?)
        .arg("--update-target")
        .arg(executable)
        .arg("--update-parent")
        .arg(std::process::id().to_string())
        .arg("--data-dir")
        .arg(data)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn apply(directory: &Path, target: &Path, data: &Path, parent: u32) -> Result<(), String> {
    let root = std::fs::canonicalize(data.join("updates")).map_err(|error| error.to_string())?;
    let directory = std::fs::canonicalize(directory).map_err(|error| error.to_string())?;
    let target = std::fs::canonicalize(target).map_err(|error| error.to_string())?;
    if !directory.starts_with(&root)
        || target.file_name().and_then(|name| name.to_str()) != Some("game-library-native.exe")
    {
        return Err("Güncelleme hedefi geçersiz".into());
    }
    let manifest_bytes =
        std::fs::read(directory.join("update.json")).map_err(|error| error.to_string())?;
    if manifest_bytes.len() > 16 * 1024 {
        return Err("Güncelleme bildirimi çok büyük".into());
    }
    let manifest: UpdateManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    manifest.verify(
        option_env!("GAME_LIBRARY_UPDATE_PUBLIC_KEY").ok_or("Güncelleme anahtarı yok")?,
        env!("CARGO_PKG_VERSION"),
    )?;
    let bytes = std::fs::read(directory.join("game-library-native.exe"))
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 != manifest.size
        || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256.to_lowercase()
    {
        return Err("Güncelleme yeniden doğrulanamadı".into());
    }
    if !platform_windows::sessions::wait_for_exit(parent, 60_000)? {
        return Err("Uygulamanın kapanması beklenirken süre doldu".into());
    }
    let backup = target.with_file_name("game-library-native.previous.exe");
    std::fs::copy(&target, &backup).map_err(|error| error.to_string())?;
    let next = target.with_file_name("game-library-native.next.exe");
    std::fs::write(&next, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&next, &target).map_err(|error| error.to_string())?;
    use std::os::windows::process::CommandExt;
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|error| error.to_string())?;
    let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let health = directory.join(format!("health-{token}.json"));
    let launch = std::process::Command::new(&target)
        .arg("--data-dir")
        .arg(data)
        .arg("--update-health-report")
        .arg(&health)
        .arg("--update-health-token")
        .arg(&token)
        .creation_flags(0x08000000)
        .spawn();
    let readiness = match launch {
        Ok(mut child) => {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            let result = loop {
                match child.try_wait() {
                    Ok(Some(_)) => break Err("Yeni sürüm hazır olmadan kapandı".to_string()),
                    Err(error) => break Err(error.to_string()),
                    Ok(None) => {}
                }
                if std::fs::metadata(&health).is_ok_and(|metadata| metadata.len() <= 4096) {
                    if let Ok(bytes) = std::fs::read(&health) {
                        if valid_health(&bytes, &manifest.version, child.id(), &token) {
                            break Ok(());
                        }
                    }
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
            result
        }
        Err(error) => Err(error.to_string()),
    };
    let _ = std::fs::remove_file(&health);
    if let Err(error) = readiness {
        std::fs::copy(&backup, &target).map_err(|error| error.to_string())?;
        std::process::Command::new(&target)
            .arg("--data-dir")
            .arg(data)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|restart| {
                format!("Önceki executable geri yüklendi; yeniden açılması başarısız: {restart}")
            })?;
        return Err(format!("{error}; önceki executable geri yüklendi ve yeniden açıldı. Veritabanı yedeği korunuyor."));
    }
    Ok(())
}

fn valid_health(bytes: &[u8], version: &str, pid: u32, token: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes).is_ok_and(|report| {
        report["ready"] == true
            && report["version"] == version
            && report["pid"] == pid
            && report["token"] == token
    })
}

#[derive(Clone, Copy)]
pub enum InstallerAction {
    Install,
    Repair,
    Uninstall,
}

fn configure_bridge(data: &Path) -> Result<(), String> {
    let roaming = std::env::var_os("APPDATA").ok_or("Windows kullanıcı veri klasörü bulunamadı")?;
    let directory = PathBuf::from(roaming).join("Vencord/settings");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let settings_file = directory.join("settings.json");
    let mut settings: serde_json::Value = if settings_file.is_file() {
        let bytes = std::fs::read(&settings_file).map_err(|error| error.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Vencord ayar dosyası boyut sınırını aşıyor".into());
        }
        let settings = serde_json::from_slice(&bytes)
            .map_err(|_| "Vencord ayarları okunamadı; mevcut dosya korundu")?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        std::fs::copy(
            &settings_file,
            directory.join(format!("settings.game-library-backup-{stamp}.json")),
        )
        .map_err(|error| error.to_string())?;
        settings
    } else {
        serde_json::json!({})
    };
    enable_bridge_settings(&mut settings, data)?;
    let temporary = directory.join("settings.game-library-next.json");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::rename(temporary, settings_file).map_err(|error| error.to_string())?;
    Ok(())
}

fn enable_bridge_settings(settings: &mut serde_json::Value, data: &Path) -> Result<(), String> {
    if !settings.is_object() {
        return Err("Vencord ayar biçimi geçersiz; mevcut dosya korundu".into());
    }
    if settings["plugins"].is_null() {
        settings["plugins"] = serde_json::json!({});
    }
    if !settings["plugins"].is_object() {
        return Err("Vencord eklenti ayarları geçersiz; mevcut dosya korundu".into());
    }
    if settings["plugins"]["GameLibraryBridge"].is_null() {
        settings["plugins"]["GameLibraryBridge"] = serde_json::json!({});
    }
    if !settings["plugins"]["GameLibraryBridge"].is_object() {
        return Err("GameLibraryBridge ayarları geçersiz; mevcut dosya korundu".into());
    }
    settings["plugins"]["GameLibraryBridge"]["enabled"] = serde_json::json!(true);
    settings["autoUpdate"] = serde_json::json!(false);
    settings["plugins"]["GameLibraryBridge"]["dataDirectory"] =
        serde_json::json!(external_path(data));
    if settings["plugins"]["GameLibraryBridge"]["selectedFriends"].is_null() {
        settings["plugins"]["GameLibraryBridge"]["selectedFriends"] = serde_json::json!("");
    }
    Ok(())
}

pub fn prepare_bridge(data: &Path) -> Result<String, String> {
    let directory = data.join("integrations/vencord");
    let plugin = directory.join("plugin");
    std::fs::create_dir_all(&plugin).map_err(|error| error.to_string())?;
    std::fs::write(
        plugin.join("index.ts"),
        include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/index.ts"),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        plugin.join("native.ts"),
        include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/native.ts"),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        plugin.join("stream.ts"),
        include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/stream.ts"),
    )
    .map_err(|error| error.to_string())?;
    for (name, contents) in [
        (
            "capture.ts",
            include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/capture.ts"),
        ),
        (
            "streamAdapter.ts",
            include_str!(
                "../../../integrations/vencord/gameLibraryBridge.desktop/streamAdapter.ts"
            ),
        ),
        (
            "transport.ts",
            include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/transport.ts"),
        ),
        (
            "spotify.ts",
            include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/spotify.ts"),
        ),
        (
            "broadcast.ts",
            include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/broadcast.ts"),
        ),
        (
            "directMessage.ts",
            include_str!(
                "../../../integrations/vencord/gameLibraryBridge.desktop/directMessage.ts"
            ),
        ),
    ] {
        std::fs::write(plugin.join(name), contents).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        plugin.join("state.ts"),
        include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/state.ts"),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        plugin.join("social.ts"),
        include_str!("../../../integrations/vencord/gameLibraryBridge.desktop/social.ts"),
    )
    .map_err(|error| error.to_string())?;
    let script = directory.join("prepare.ps1");
    std::fs::write(
        &script,
        include_str!("../../../scripts/prepare-vencord-bridge.ps1"),
    )
    .map_err(|error| error.to_string())?;
    use std::os::windows::process::CommandExt;
    let log =
        std::fs::File::create(directory.join("prepare.log")).map_err(|error| error.to_string())?;
    let error_log = log.try_clone().map_err(|error| error.to_string())?;
    let mut process = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(external_path(&script))
        .arg("-SourceDirectory")
        .arg(external_path(&directory.join("source")))
        .arg("-PluginDirectory")
        .arg(external_path(&plugin))
        .arg("-Update")
        .creation_flags(0x08000000)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(error_log)
        .spawn()
        .map_err(|_| "PowerShell 7, Node.js 22+, Git ve pnpm kurulumu gerekli".to_string())?;
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = process.try_wait().map_err(|error| error.to_string())? {
            if !status.success() {
                return Err("Vencord köprüsü hazırlanamadı. integrations/vencord/prepare.log dosyasını kontrol et.".into());
            }
            break;
        }
        if started.elapsed().as_secs() > 600 {
            let _ = process.kill();
            let _ = process.wait();
            return Err("Köprü hazırlama zaman aşımına uğradı".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let installed =
        PathBuf::from(std::env::var_os("APPDATA").ok_or("Windows kullanıcı klasörü eksik")?)
            .join("Vencord/dist");
    if installed.join("patcher.js").is_file() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        install_prepared_bridge(
            &directory.join("source/dist"),
            &installed,
            &directory.join(format!("backups/standalone-{stamp}")),
        )?;
        configure_bridge(data)?;
        Ok("Vencord köprüsü güncellendi ve eski paket yedeklendi. Discord'u tamamen kapatıp açınca bağlantı etkinleşir.".into())
    } else {
        Ok("Vencord ve köprü derlendi. Vencord kur/onar ile seçili Discord kurulumuna uygulanabilir.".into())
    }
}

fn install_prepared_bridge(source: &Path, installed: &Path, backup: &Path) -> Result<(), String> {
    let names = ["patcher.js", "preload.js", "renderer.js", "renderer.css"];
    let mut files = Vec::new();
    for name in names {
        let metadata =
            std::fs::metadata(source.join(name)).map_err(|_| "Derlenmiş Vencord paketi eksik")?;
        if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 {
            return Err("Derlenmiş Vencord paketi geçersiz".into());
        }
        let bytes = std::fs::read(source.join(name)).map_err(|error| error.to_string())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("Derlenmiş Vencord paketi çok büyük".into());
        }
        files.push((name, bytes));
    }
    let renderer = &files[2].1;
    if !renderer
        .windows(b"GameLibraryBridge".len())
        .any(|bytes| bytes == b"GameLibraryBridge")
    {
        return Err("Vencord paketinde Game Library köprüsü yok; mevcut kurulum korundu".into());
    }
    std::fs::create_dir_all(backup).map_err(|error| error.to_string())?;
    for name in names {
        if installed.join(name).is_file() {
            std::fs::copy(installed.join(name), backup.join(name))
                .map_err(|error| error.to_string())?;
        }
    }
    for (name, bytes) in &files {
        if let Err(error) = std::fs::write(installed.join(name), bytes) {
            let mut failures = Vec::new();
            for original in names {
                let restored = if backup.join(original).is_file() {
                    std::fs::copy(backup.join(original), installed.join(original)).map(|_| ())
                } else if installed.join(original).is_file() {
                    std::fs::remove_file(installed.join(original))
                } else {
                    Ok(())
                };
                if let Err(error) = restored {
                    failures.push(format!("{original}: {error}"));
                }
            }
            return Err(format!(
                "Vencord paketi uygulanamadı: {error}. {}Yedek: {}",
                if failures.is_empty() {
                    "Önceki paket geri yüklendi. ".into()
                } else {
                    format!("Geri alma hatası: {}. ", failures.join(", "))
                },
                backup.display()
            ));
        }
    }
    Ok(())
}

pub fn vencord_installer(
    data: &Path,
    action: InstallerAction,
    branch: &str,
) -> Result<String, String> {
    if !["stable", "ptb", "canary"].contains(&branch) {
        return Err("Discord kanalı geçersiz".into());
    }
    let release_bytes = media::download(
        "https://api.github.com/repos/Vencord/Installer/releases/latest",
        &["api.github.com"],
        1024 * 1024,
    )?;
    let release: serde_json::Value =
        serde_json::from_slice(&release_bytes).map_err(|error| error.to_string())?;
    let asset = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|asset| asset["name"] == "VencordInstallerCli.exe")
        })
        .ok_or("Resmi Windows installer bulunamadı")?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("Installer adresi yok")?;
    let parsed = media::trusted_url(url, &["github.com"])?;
    if !parsed
        .path()
        .starts_with("/Vencord/Installer/releases/download/")
    {
        return Err("Installer kaynağı doğrulanamadı".into());
    }
    let digest = asset["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or("Resmi installer SHA-256 özeti yayımlanmamış; çalıştırma durduruldu")?;
    let bytes = media::download(url, ASSET_HOSTS, 40 * 1024 * 1024)?;
    if format!("{:x}", Sha256::digest(&bytes)) != digest || !bytes.starts_with(b"MZ") {
        return Err("Installer dosyası doğrulanamadı".into());
    }
    let directory = data.join("integrations/vencord");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let installer = directory.join("VencordInstallerCli.exe");
    std::fs::write(&installer, bytes).map_err(|error| error.to_string())?;
    let source = directory.join("source");
    let custom = source.join("game-library-bridge.json").is_file()
        && source.join("dist/patcher.js").is_file();
    let flag = match action {
        InstallerAction::Install => "--install",
        InstallerAction::Repair if custom => "--install",
        InstallerAction::Repair => "--repair",
        InstallerAction::Uninstall => "--uninstall",
    };
    use std::os::windows::process::CommandExt;
    let log = std::fs::File::create(directory.join("installer.log"))
        .map_err(|error| error.to_string())?;
    let error_log = log.try_clone().map_err(|error| error.to_string())?;
    let mut command = std::process::Command::new(&installer);
    if custom {
        command
            .env("VENCORD_USER_DATA_DIR", external_path(&source))
            .env("VENCORD_DEV_INSTALL", "1");
    }
    let mut child = command
        .args([flag, "--branch", branch])
        .current_dir(&directory)
        .creation_flags(0x08000000)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(error_log)
        .spawn()
        .map_err(|error| error.to_string())?;
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            if !status.success() {
                return Err("Resmi Vencord installer işlemi başarısız oldu. integrations/vencord/installer.log dosyasını kontrol et.".into());
            }
            break;
        }
        if start.elapsed().as_secs() > 180 {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "Vencord installer zaman aşımına uğradı; kurulum durumunu kontrol et".into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if custom && !matches!(action, InstallerAction::Uninstall) {
        configure_bridge(data)?;
    }
    Ok(format!("Resmi installer işlemi tamamlandı ({branch}). Discord yeniden açıldıktan sonra Vencord/köprü bağlantısı ayrıca doğrulanır."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn installed_bridge_rejects_missing_plugin_and_preserves_backup() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let installed = temp.path().join("installed");
        let backup = temp.path().join("backup");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&installed).unwrap();
        for name in ["patcher.js", "preload.js", "renderer.js", "renderer.css"] {
            std::fs::write(source.join(name), b"new bundle").unwrap();
            std::fs::write(installed.join(name), b"original bundle").unwrap();
        }
        assert!(install_prepared_bridge(&source, &installed, &backup).is_err());
        assert!(!backup.exists());
        assert_eq!(
            std::fs::read(installed.join("renderer.js")).unwrap(),
            b"original bundle"
        );
        std::fs::write(source.join("renderer.js"), b"new GameLibraryBridge bundle").unwrap();
        install_prepared_bridge(&source, &installed, &backup).unwrap();
        assert_eq!(
            std::fs::read(installed.join("renderer.js")).unwrap(),
            b"new GameLibraryBridge bundle"
        );
        for name in ["patcher.js", "preload.js", "renderer.js", "renderer.css"] {
            assert_eq!(
                std::fs::read(backup.join(name)).unwrap(),
                b"original bundle"
            );
        }
    }
    #[test]
    fn update_readiness_rejects_stale_or_unrelated_process_reports() {
        let report = serde_json::to_vec(
            &serde_json::json!({"ready":true,"version":"0.2.0","pid":42,"token":"nonce"}),
        )
        .unwrap();
        assert!(valid_health(&report, "0.2.0", 42, "nonce"));
        assert!(!valid_health(&report, "0.2.0", 41, "nonce"));
        assert!(!valid_health(&report, "0.2.0", 42, "old-nonce"));
        assert!(!valid_health(&report, "0.1.0", 42, "nonce"));
        assert!(!valid_health(b"{}", "0.2.0", 42, "nonce"));
    }
    #[test]
    fn bridge_settings_preserve_other_plugins_and_selected_friends() {
        let mut settings = serde_json::json!({"plugins":{"Other":{"enabled":true},"GameLibraryBridge":{"enabled":false,"selectedFriends":"123,456"}},"autoUpdate":true});
        enable_bridge_settings(&mut settings, Path::new(r"\\?\C:\GameLibrary")).unwrap();
        assert_eq!(settings["plugins"]["Other"]["enabled"], true);
        assert_eq!(settings["autoUpdate"], false);
        assert_eq!(
            settings["plugins"]["GameLibraryBridge"]["selectedFriends"],
            "123,456"
        );
        assert_eq!(
            settings["plugins"]["GameLibraryBridge"]["dataDirectory"],
            r"C:\GameLibrary"
        );
        assert_eq!(settings["plugins"]["GameLibraryBridge"]["enabled"], true);
        assert!(enable_bridge_settings(
            &mut serde_json::json!({"plugins":{"GameLibraryBridge":[]}}),
            Path::new(r"C:\GameLibrary")
        )
        .is_err());
    }
    #[test]
    fn installer_receives_paths_accepted_by_electron_module_loading() {
        assert_eq!(
            external_path(Path::new(r"\\?\C:\Users\User\GameLibrary\source")),
            r"C:\Users\User\GameLibrary\source"
        );
        assert_eq!(
            external_path(Path::new(r"\\?\UNC\server\share\source")),
            r"\\server\share\source"
        );
    }
    #[test]
    fn update_requires_signature_trusted_release_and_newer_version() {
        let signing = SigningKey::from_bytes(&[42; 32]);
        let key =
            base64::engine::general_purpose::STANDARD.encode(signing.verifying_key().as_bytes());
        let mut manifest = UpdateManifest { version:"0.2.0".into(), url:"https://github.com/tugcantopaloglu/game-library/releases/download/v0.2.0/game-library-native.exe".into(),sha256:"a".repeat(64),size:4096,minimum_bridge_protocol:1,signature:String::new() };
        manifest.signature = base64::engine::general_purpose::STANDARD.encode(
            signing
                .sign(manifest.signed_payload().as_bytes())
                .to_bytes(),
        );
        assert!(manifest.verify(&key, "0.1.0").is_ok());
        assert!(manifest.verify(&key, "0.2.0").is_err());
        manifest.size += 1;
        assert!(manifest.verify(&key, "0.1.0").is_err());
        manifest.url =
            "https://github.com/other/project/releases/download/v0.2.0/game-library-native.exe"
                .into();
        assert!(manifest.verify(&key, "0.1.0").is_err());
    }
}
