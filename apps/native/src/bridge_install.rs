//! The shipped bridge is installed without invoking a compiler or a shell.
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MANIFEST: &str = include_str!("../../../integrations/vencord/prebuilt/bundle.json");
const NAMES: [&str; 4] = ["patcher.js", "preload.js", "renderer.js", "renderer.css"];
const COMPRESSED: [&[u8]; 4] = [
    include_bytes!("../../../integrations/vencord/prebuilt/patcher.js.gz"),
    include_bytes!("../../../integrations/vencord/prebuilt/preload.js.gz"),
    include_bytes!("../../../integrations/vencord/prebuilt/renderer.js.gz"),
    include_bytes!("../../../integrations/vencord/prebuilt/renderer.css.gz"),
];
const MODULE_BOUNDARY: &[u8] = b"{\"type\":\"commonjs\"}\n";
const NOTICES: [(&str, &[u8]); 4] = [
    (
        "dist/patcher.js.LEGAL.txt",
        include_bytes!("../../../integrations/vencord/prebuilt/patcher.js.LEGAL.txt"),
    ),
    (
        "dist/renderer.js.LEGAL.txt",
        include_bytes!("../../../integrations/vencord/prebuilt/renderer.js.LEGAL.txt"),
    ),
    (
        "LICENSE.GameLibraryBridge",
        include_bytes!("../../../integrations/vencord/prebuilt/LICENSE"),
    ),
    ("dist/package.json", MODULE_BOUNDARY),
];

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub protocol: u32,
    pub build: String,
    pub vencord_revision: String,
    pub plugin_sources: std::collections::BTreeMap<String, String>,
    pub files: Vec<FileInfo>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

pub fn manifest() -> Manifest {
    serde_json::from_str(MANIFEST).expect("Embedded bridge manifest was checked during packaging")
}

pub fn verified_manifest() -> Result<Manifest, String> {
    Bundle::unpack().map(|bundle| bundle.manifest)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Bundle {
    manifest: Manifest,
    files: Vec<Vec<u8>>,
}

impl Bundle {
    fn unpack() -> Result<Self, String> {
        Self::decode(manifest(), &COMPRESSED)
    }
    fn decode(manifest: Manifest, compressed: &[&[u8]]) -> Result<Self, String> {
        if manifest.protocol != 1
            || manifest.files.len() != NAMES.len()
            || compressed.len() != NAMES.len()
        {
            return Err("Hazır köprü paketi bu uygulamayla uyumlu değil".into());
        }
        let mut files = Vec::new();
        for ((info, name), compressed) in manifest.files.iter().zip(NAMES).zip(compressed) {
            if info.name != name || info.size == 0 || info.size > 8 * 1024 * 1024 {
                return Err("Hazır köprü dosya listesi geçersiz".into());
            }
            let mut bytes = Vec::new();
            GzDecoder::new(*compressed)
                .take(info.size + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Hazır köprü paketi açılamadı")?;
            if bytes.len() as u64 != info.size || digest(&bytes) != info.sha256 {
                return Err("Hazır köprü SHA-256 doğrulamasını geçmedi".into());
            }
            files.push(bytes);
        }
        let renderer = String::from_utf8_lossy(&files[2]);
        if !renderer.contains("GameLibraryBridge")
            || !renderer.contains(&format!("bridgeBuild:\"{}\"", manifest.build))
        {
            return Err("Hazır pakette güncel Game Library köprüsü bulunamadı".into());
        }
        Ok(Self { manifest, files })
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub branch: String,
    pub auto_update: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            branch: "stable".into(),
            auto_update: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    build: String,
    bundle_sha256: String,
    branch: String,
    backup: String,
}

#[derive(Serialize)]
pub struct InstallReport {
    pub build: String,
    pub changed: bool,
    pub backup: Option<String>,
    pub message: String,
}

struct Paths {
    data: PathBuf,
    local: PathBuf,
    roaming: PathBuf,
}

impl Paths {
    fn system(data: &Path) -> Result<Self, String> {
        Ok(Self {
            data: data.to_owned(),
            local: std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .ok_or("Windows yerel veri klasörü bulunamadı")?,
            roaming: std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .ok_or("Windows kullanıcı veri klasörü bulunamadı")?,
        })
    }
    fn root(&self) -> PathBuf {
        self.roaming.join("Vencord")
    }
    fn receipt(&self, branch: &str) -> PathBuf {
        self.data
            .join(format!("integrations/vencord/installations/{branch}.json"))
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<Option<T>, String> {
    match std::fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Ok(metadata) if metadata.is_file() && metadata.len() <= limit => {}
        _ => {
            return Err(format!(
                "Ayar dosyası okunamıyor; mevcut dosya korundu: {}",
                path.display()
            ))
        }
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Ayar dosyası boyut sınırını aşıyor".into());
    }
    serde_json::from_slice(&bytes).map(Some).map_err(|_| {
        format!(
            "Ayar biçimi geçersiz; mevcut dosya korundu: {}",
            path.display()
        )
    })
}

fn preferences_file(data: &Path) -> PathBuf {
    data.join("integrations/vencord/preferences.json")
}

pub fn preferences(data: &Path) -> Result<Preferences, String> {
    let value: Preferences = read_json(&preferences_file(data), 4096)?.unwrap_or_default();
    branch_directory(&value.branch)?;
    Ok(value)
}

pub fn set_auto_update(data: &Path, enabled: bool) -> Result<(), String> {
    let mut value = preferences(data)?;
    value.auto_update = enabled;
    atomic_write(&preferences_file(data), &json_bytes(&value)?)
}

fn branch_directory(branch: &str) -> Result<&'static str, String> {
    match branch {
        "stable" => Ok("Discord"),
        "ptb" => Ok("DiscordPTB"),
        "canary" => Ok("DiscordCanary"),
        _ => Err("Discord kanalı geçersiz".into()),
    }
}

fn resources(paths: &Paths, branch: &str) -> Result<PathBuf, String> {
    let name = branch_directory(branch)?;
    // Match the official Windows installer's choice of app-* directory.
    std::fs::read_dir(paths.local.join(name))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry.file_name().to_string_lossy().starts_with("app-")
                && entry.path().join("resources").is_dir()
        })
        .map(|entry| entry.path().join("resources"))
        .max()
        .ok_or_else(|| {
            format!(
                "{name} kurulumu bulunamadı. Önce Discord'u kur veya doğru Discord sürümünü seç."
            )
        })
}

fn loader_matches(resources: &Path, patcher: &Path) -> bool {
    if !resources.join("_app.asar").is_file() {
        return false;
    }
    let path = resources.join("app.asar");
    if !std::fs::metadata(&path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= 64 * 1024)
    {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut bytes = Vec::new();
    if file.take(64 * 1024 + 1).read_to_end(&mut bytes).is_err() || bytes.len() > 64 * 1024 {
        return false;
    }
    // The official installer writes require(JSON.stringify(patcherPath)) into app.asar.
    // Compare the decoded Windows paths: Go filepath.Join normalizes separators,
    // while Rust PathBuf::join can retain a slash from "dist/patcher.js".
    let normalize = |path: &str| {
        crate::updates::external_path(Path::new(&path.replace('/', "\\"))).to_lowercase()
    };
    let expected = normalize(&crate::updates::external_path(patcher));
    let text = String::from_utf8_lossy(&bytes);
    text.match_indices("require(").any(|(offset, _)| {
        let tail = &text[offset + "require(".len()..];
        let mut values = serde_json::Deserializer::from_str(tail).into_iter::<String>();
        let Some(Ok(value)) = values.next() else {
            return false;
        };
        tail[values.byte_offset()..].trim_start().starts_with(')') && normalize(&value) == expected
    })
}

fn files_match(root: &Path, manifest: &Manifest) -> bool {
    let scripts_match = manifest.files.iter().all(|file| {
        let path = root.join("dist").join(&file.name);
        std::fs::metadata(&path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == file.size)
            && std::fs::read(path).is_ok_and(|bytes| digest(&bytes) == file.sha256)
    });
    // The official updater creates a package boundary as well: an unrelated
    // parent package.json with type=module must not change how Electron loads us.
    scripts_match
        && read_json::<serde_json::Value>(&root.join("dist/package.json"), 4096)
            .is_ok_and(|value| value.is_some_and(|value| value["type"] == "commonjs"))
}

pub fn status(data: &Path, branch: &str) -> String {
    let result = (|| {
        branch_directory(branch)?;
        let paths = Paths::system(data)?;
        let receipt: Option<Receipt> = read_json(&paths.receipt(branch), 16 * 1024)?;
        Ok::<_, String>(match receipt {
            Some(receipt) if receipt.bundle_sha256 == digest(MANIFEST.as_bytes()) => {
                format!("Son kurulum: {}.", receipt.build)
            }
            Some(receipt) => format!(
                "Kurulu köprü: {}. {} güncellemesi hazır.",
                receipt.build,
                manifest().build
            ),
            None => format!("Köprü {} kurulmaya hazır.", manifest().build),
        })
    })();
    result.unwrap_or_else(|error| error)
}

pub fn automatic_update_needed(data: &Path) -> Result<Option<String>, String> {
    automatic_update_needed_with(&Paths::system(data)?)
}

fn automatic_update_needed_with(paths: &Paths) -> Result<Option<String>, String> {
    let preferences = preferences(&paths.data)?;
    if !preferences.auto_update {
        return Ok(None);
    }
    let Some(receipt): Option<Receipt> = read_json(&paths.receipt(&preferences.branch), 16 * 1024)?
    else {
        return Ok(None);
    };
    let current = resources(paths, &preferences.branch)?;
    if receipt.bundle_sha256 != digest(MANIFEST.as_bytes())
        || !files_match(&paths.root(), &manifest())
        || !loader_matches(&current, &paths.root().join("dist/patcher.js"))
    {
        Ok(Some(preferences.branch))
    } else {
        Ok(None)
    }
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value).map_err(|error| error.to_string())
}

fn suffix() -> Result<String, String> {
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|error| error.to_string())?;
    Ok(nonce.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Kurulum dosyasının klasörü yok")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let staged = parent.join(format!(".game-library-{}.next", suffix()?));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| error.to_string())?;
        drop(file);
        std::fs::rename(&staged, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(staged);
    }
    result
}

#[derive(Serialize)]
struct BackupFile {
    target: PathBuf,
    copy: PathBuf,
    existed: bool,
}
struct Transaction {
    directory: PathBuf,
    files: Vec<BackupFile>,
}

impl Transaction {
    fn begin(data: &Path) -> Result<Self, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        let directory = data.join(format!(
            "integrations/vencord/backups/install-{stamp}-{}",
            suffix()?
        ));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        Ok(Self {
            directory,
            files: vec![],
        })
    }
    fn capture(&mut self, path: PathBuf, name: &str) -> Result<(), String> {
        let copy = self.directory.join(name);
        let existed = match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= 512 * 1024 * 1024 => {
                std::fs::copy(&path, &copy).map_err(|error| format!("Yedek alınamadı: {error}"))?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            _ => {
                return Err(format!(
                    "Kurulum dosyası yedeklenemiyor; mevcut kurulum korundu: {}",
                    path.display()
                ))
            }
        };
        self.files.push(BackupFile {
            target: path,
            copy,
            existed,
        });
        Ok(())
    }
    fn rollback(&self) -> Vec<String> {
        self.files
            .iter()
            .rev()
            .filter_map(|file| {
                if file.existed && identical_files(&file.copy, &file.target) {
                    return None;
                }
                let result = if file.existed {
                    std::fs::copy(&file.copy, &file.target).map(|_| ())
                } else if file.target.is_file() {
                    std::fs::remove_file(&file.target)
                } else {
                    Ok(())
                };
                result
                    .err()
                    .map(|error| format!("{}: {error}", file.target.display()))
            })
            .collect()
    }
}

fn identical_files(first: &Path, second: &Path) -> bool {
    let hash = |path: &Path| -> std::io::Result<(u64, Vec<u8>)> {
        let mut file = std::fs::File::open(path)?.take(512 * 1024 * 1024 + 1);
        let mut digest = Sha256::new();
        let mut bytes = [0u8; 16 * 1024];
        let mut size = 0;
        loop {
            let count = file.read(&mut bytes)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            digest.update(&bytes[..count]);
        }
        Ok((size, digest.finalize().to_vec()))
    };
    match (hash(first), hash(second)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn install(
    data: &Path,
    branch: &str,
    progress: impl Fn(&str),
) -> Result<InstallReport, String> {
    let paths = Paths::system(data)?;
    let lock = platform_windows::shell::InstanceGuard::acquire(
        &paths.root().join("game-library-bridge-install"),
    )?;
    if !lock.primary {
        return Err("Başka bir köprü kurulumu sürüyor; bitince yeniden dene.".into());
    }
    progress("Hazır köprü paketi doğrulanıyor…");
    let bundle = Bundle::unpack()?;
    let running =
        platform_windows::sessions::running_named(&format!("{}.exe", branch_directory(branch)?));
    install_with(&paths, branch, &bundle, running, &progress, |root| {
        progress("Resmi Vencord kurucusu hazırlanıyor ve Discord bağlantısı kuruluyor…");
        crate::updates::run_vencord_installer(
            data,
            crate::updates::InstallerAction::Install,
            branch,
            root,
        )
    })
}

fn install_with(
    paths: &Paths,
    branch: &str,
    bundle: &Bundle,
    running: bool,
    progress: &impl Fn(&str),
    patch: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<InstallReport, String> {
    let resources = resources(paths, branch)?;
    let root = paths.root();
    let patch_required = !loader_matches(&resources, &root.join("dist/patcher.js"));
    if patch_required && running {
        return Err("Discord açık. İlk kurulum veya onarım için Discord'dan tepsi menüsündeki Çıkış ile çıkıp yeniden dene. Oturumun kapatılmadı.".into());
    }
    let settings_path = root.join("settings/settings.json");
    let previous_settings: serde_json::Value =
        read_json(&settings_path, 1024 * 1024)?.unwrap_or_else(|| serde_json::json!({}));
    let mut settings = previous_settings.clone();
    crate::updates::enable_bridge_settings(&mut settings, &paths.data)?;
    let mut prefs = preferences(&paths.data)?;
    let previous_branch = prefs.branch.clone();
    prefs.branch = branch.into();
    let receipt_path = paths.receipt(branch);
    let previous_receipt: Option<Receipt> = read_json(&receipt_path, 16 * 1024)?;
    if !patch_required
        && files_match(&root, &bundle.manifest)
        && settings == previous_settings
        && previous_branch == branch
        && previous_receipt
            .as_ref()
            .is_some_and(|receipt| receipt.bundle_sha256 == digest(MANIFEST.as_bytes()))
    {
        return Ok(InstallReport { build: bundle.manifest.build.clone(), changed: false, backup: None,
            message: format!("Köprü {} zaten güncel. Açık Discord eski sürümü kullanıyorsa Discord'u yeniden aç.", bundle.manifest.build) });
    }
    progress("Mevcut köprü ve ayarlar yedekleniyor…");
    let mut transaction = Transaction::begin(&paths.data)?;
    for name in NAMES {
        transaction.capture(root.join("dist").join(name), name)?;
    }
    for (index, (name, _)) in NOTICES.iter().enumerate() {
        transaction.capture(root.join(name), &format!("notice-{index}"))?;
    }
    transaction.capture(settings_path.clone(), "settings.json")?;
    transaction.capture(preferences_file(&paths.data), "preferences.json")?;
    transaction.capture(receipt_path.clone(), "receipt.json")?;
    if patch_required {
        transaction.capture(resources.join("app.asar"), "discord-app.asar")?;
        transaction.capture(resources.join("_app.asar"), "discord-original-app.asar")?;
    }
    atomic_write(
        &transaction.directory.join("backup.json"),
        &json_bytes(&transaction.files)?,
    )?;
    let backup = crate::updates::external_path(&transaction.directory);
    let result = (|| {
        progress("Hazır köprü kuruluyor…");
        for (name, bytes) in NAMES.into_iter().zip(&bundle.files) {
            atomic_write(&root.join("dist").join(name), bytes)?;
        }
        for (name, bytes) in NOTICES {
            atomic_write(&root.join(name), bytes)?;
        }
        atomic_write(&settings_path, &json_bytes(&settings)?)?;
        if patch_required {
            patch(&root)?;
        }
        if !loader_matches(&resources, &root.join("dist/patcher.js"))
            || !files_match(&root, &bundle.manifest)
        {
            return Err(
                "Kurulum tamamlandıktan sonra Discord köprü dosyaları doğrulanamadı".into(),
            );
        }
        let receipt = Receipt {
            build: bundle.manifest.build.clone(),
            bundle_sha256: digest(MANIFEST.as_bytes()),
            branch: branch.into(),
            backup: backup.clone(),
        };
        atomic_write(&receipt_path, &json_bytes(&receipt)?)?;
        atomic_write(&preferences_file(&paths.data), &json_bytes(&prefs)?)?;
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        let failures = transaction.rollback();
        return Err(format!(
            "{error}. {} Yedek: {backup}",
            if failures.is_empty() {
                "Önceki köprü ve ayarlar geri yüklendi.".into()
            } else {
                format!("Geri alma tamamlanamadı: {}", failures.join(", "))
            }
        ));
    }
    Ok(InstallReport { build: bundle.manifest.build.clone(), changed: true, backup: Some(backup),
        message: format!("Köprü {} kuruldu ve önceki dosyalar yedeklendi. Etkinleşmesi için Discord'u yeniden aç.", bundle.manifest.build) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture {
        _temp: tempfile::TempDir,
        paths: Paths,
        resources: PathBuf,
    }
    impl Fixture {
        fn new(branch: &str) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let paths = Paths {
                data: temp.path().join("game-library"),
                local: temp.path().join("local"),
                roaming: temp.path().join("roaming"),
            };
            let resources = paths
                .local
                .join(branch_directory(branch).unwrap())
                .join("app-1.0.0/resources");
            std::fs::create_dir_all(&resources).unwrap();
            std::fs::write(resources.join("app.asar"), b"original Discord archive").unwrap();
            Self {
                _temp: temp,
                paths,
                resources,
            }
        }
        fn patch(&self, root: &Path) -> Result<(), String> {
            std::fs::rename(
                self.resources.join("app.asar"),
                self.resources.join("_app.asar"),
            )
            .unwrap();
            std::fs::write(
                self.resources.join("app.asar"),
                format!(
                    "require({})",
                    serde_json::to_string(
                        &crate::updates::external_path(&root.join("dist/patcher.js"))
                            .replace('/', "\\")
                    )
                    .unwrap()
                ),
            )
            .unwrap();
            Ok(())
        }
        fn settings(&self, value: &serde_json::Value) {
            atomic_write(
                &self.paths.root().join("settings/settings.json"),
                &json_bytes(value).unwrap(),
            )
            .unwrap();
        }
        fn apply(&self, branch: &str) -> Result<InstallReport, String> {
            install_with(
                &self.paths,
                branch,
                &Bundle::unpack().unwrap(),
                false,
                &|_| {},
                |root| self.patch(root),
            )
        }
    }

    #[test]
    fn official_windows_loader_normalizes_paths_and_rejects_a_different_target() {
        let fixture = Fixture::new("stable");
        let root = fixture.paths.root();
        fixture.patch(&root).unwrap();
        assert!(loader_matches(
            &fixture.resources,
            &root.join("dist/patcher.js")
        ));
        assert!(!loader_matches(
            &fixture.resources,
            &root.join("other/patcher.js")
        ));
        let encoded = serde_json::to_string(
            &crate::updates::external_path(&root.join("dist/patcher.js")).replace('/', "\\"),
        )
        .unwrap();
        std::fs::write(
            fixture.resources.join("app.asar"),
            format!("require({encoded} + \"other\")"),
        )
        .unwrap();
        assert!(!loader_matches(
            &fixture.resources,
            &root.join("dist/patcher.js")
        ));
    }

    #[test]
    fn embedded_bundle_rejects_corruption_size_and_path_changes() {
        assert_eq!(Bundle::unpack().unwrap().manifest.build, "20261015.1");
        let mut altered = manifest();
        altered.files[0].sha256 = "0".repeat(64);
        assert!(Bundle::decode(altered, &COMPRESSED).is_err());
        let mut altered = manifest();
        altered.files[0].name = "../patcher.js".into();
        assert!(Bundle::decode(altered, &COMPRESSED).is_err());
        let mut altered = manifest();
        altered.files[0].size = 8 * 1024 * 1024 + 1;
        assert!(Bundle::decode(altered, &COMPRESSED).is_err());
        let truncated = [
            &COMPRESSED[0][..8],
            COMPRESSED[1],
            COMPRESSED[2],
            COMPRESSED[3],
        ];
        assert!(Bundle::decode(manifest(), &truncated).is_err());
    }

    #[test]
    fn fresh_install_is_idempotent_and_preserves_other_settings() {
        let fixture = Fixture::new("stable");
        fixture.settings(&json!({"themeLinks":["keep.css"],"plugins":{"Other":{"enabled":true},"GameLibraryBridge":{"selectedFriends":"123,456"}}}));
        assert!(automatic_update_needed_with(&fixture.paths)
            .unwrap()
            .is_none());
        let report = fixture.apply("stable").unwrap();
        assert!(report.changed);
        assert!(Path::new(report.backup.as_ref().unwrap())
            .join("backup.json")
            .is_file());
        let settings: serde_json::Value = read_json(
            &fixture.paths.root().join("settings/settings.json"),
            1024 * 1024,
        )
        .unwrap()
        .unwrap();
        assert_eq!(settings["themeLinks"], json!(["keep.css"]));
        assert_eq!(settings["plugins"]["Other"]["enabled"], true);
        assert_eq!(
            settings["plugins"]["GameLibraryBridge"]["selectedFriends"],
            "123,456"
        );
        assert_eq!(settings["plugins"]["GameLibraryBridge"]["enabled"], true);
        assert_eq!(settings["autoUpdate"], false);
        let package: serde_json::Value =
            read_json(&fixture.paths.root().join("dist/package.json"), 4096)
                .unwrap()
                .unwrap();
        assert_eq!(package["type"], "commonjs");
        let repeated = install_with(
            &fixture.paths,
            "stable",
            &Bundle::unpack().unwrap(),
            true,
            &|_| {},
            |_| panic!("Current loader must not be patched again"),
        )
        .unwrap();
        assert!(!repeated.changed);
        assert!(automatic_update_needed_with(&fixture.paths)
            .unwrap()
            .is_none());
    }

    #[test]
    fn failed_patch_restores_discord_bundle_settings_and_receipt() {
        let fixture = Fixture::new("stable");
        fixture.settings(
            &json!({"plugins":{"GameLibraryBridge":{"enabled":false,"selectedFriends":"keep"}}}),
        );
        for name in NAMES {
            atomic_write(
                &fixture.paths.root().join("dist").join(name),
                b"previous bundle",
            )
            .unwrap();
        }
        let original_settings =
            std::fs::read(fixture.paths.root().join("settings/settings.json")).unwrap();
        let error = install_with(
            &fixture.paths,
            "stable",
            &Bundle::unpack().unwrap(),
            false,
            &|_| {},
            |root| {
                fixture.patch(root)?;
                Err("Installer failed after changing the loader".into())
            },
        )
        .err()
        .unwrap();
        assert!(
            error.contains("Önceki köprü ve ayarlar geri yüklendi"),
            "{error}"
        );
        for name in NAMES {
            assert_eq!(
                std::fs::read(fixture.paths.root().join("dist").join(name)).unwrap(),
                b"previous bundle"
            );
        }
        assert_eq!(
            std::fs::read(fixture.resources.join("app.asar")).unwrap(),
            b"original Discord archive"
        );
        assert!(!fixture.resources.join("_app.asar").exists());
        assert_eq!(
            std::fs::read(fixture.paths.root().join("settings/settings.json")).unwrap(),
            original_settings
        );
        assert!(!fixture.paths.receipt("stable").exists());
        assert!(!preferences_file(&fixture.paths.data).exists());
    }

    #[test]
    fn zero_exit_without_a_correct_loader_is_rolled_back() {
        let fixture = Fixture::new("ptb");
        let error = install_with(
            &fixture.paths,
            "ptb",
            &Bundle::unpack().unwrap(),
            false,
            &|_| {},
            |_| Ok(()),
        )
        .err()
        .unwrap();
        assert!(error.contains("doğrulanamadı"));
        assert!(!fixture.paths.root().join("dist/patcher.js").exists());
        assert_eq!(
            std::fs::read(fixture.resources.join("app.asar")).unwrap(),
            b"original Discord archive"
        );
    }

    #[test]
    fn missing_discord_bad_settings_and_running_discord_are_non_destructive() {
        let fixture = Fixture::new("stable");
        assert!(fixture.apply("canary").is_err());
        assert!(fixture.apply("../../stable").is_err());
        assert!(!fixture.paths.root().exists());
        let error = install_with(
            &fixture.paths,
            "stable",
            &Bundle::unpack().unwrap(),
            true,
            &|_| {},
            |_| panic!("Must not close Discord"),
        )
        .err()
        .unwrap();
        assert!(error.contains("Discord açık"));
        assert!(!fixture.paths.root().exists());
        atomic_write(
            &fixture.paths.root().join("settings/settings.json"),
            b"{bad json",
        )
        .unwrap();
        assert!(fixture.apply("stable").is_err());
        assert!(!fixture.paths.root().join("dist").exists());
        assert_eq!(
            std::fs::read(fixture.paths.root().join("settings/settings.json")).unwrap(),
            b"{bad json"
        );
    }

    #[test]
    fn selected_channel_and_auto_update_preference_are_respected() {
        let fixture = Fixture::new("ptb");
        let stable = fixture.paths.local.join("Discord/app-2.0.0/resources");
        std::fs::create_dir_all(&stable).unwrap();
        std::fs::write(stable.join("app.asar"), b"keep stable").unwrap();
        fixture.apply("ptb").unwrap();
        assert_eq!(preferences(&fixture.paths.data).unwrap().branch, "ptb");
        assert_eq!(
            std::fs::read(stable.join("app.asar")).unwrap(),
            b"keep stable"
        );
        // A replacement application carries a new bundle identity even when the
        // installation is otherwise healthy. Its startup must request an update.
        let receipt_path = fixture.paths.receipt("ptb");
        let mut older: serde_json::Value = read_json(&receipt_path, 16 * 1024).unwrap().unwrap();
        older["bundle_sha256"] = json!("0".repeat(64));
        atomic_write(&receipt_path, &json_bytes(&older).unwrap()).unwrap();
        assert_eq!(
            automatic_update_needed_with(&fixture.paths)
                .unwrap()
                .as_deref(),
            Some("ptb")
        );
        fixture.apply("ptb").unwrap();
        assert!(automatic_update_needed_with(&fixture.paths)
            .unwrap()
            .is_none());
        atomic_write(
            &fixture.paths.root().join("dist/renderer.js"),
            b"overwritten by another update",
        )
        .unwrap();
        assert_eq!(
            automatic_update_needed_with(&fixture.paths)
                .unwrap()
                .as_deref(),
            Some("ptb")
        );
        set_auto_update(&fixture.paths.data, false).unwrap();
        assert!(automatic_update_needed_with(&fixture.paths)
            .unwrap()
            .is_none());
        fixture.apply("ptb").unwrap();
        assert!(!preferences(&fixture.paths.data).unwrap().auto_update);
    }

    #[test]
    fn write_failure_after_bundle_changes_restores_previous_files() {
        use std::os::windows::fs::OpenOptionsExt;
        let fixture = Fixture::new("stable");
        fixture.apply("stable").unwrap();
        let old_renderer = b"older renderer bytes";
        atomic_write(&fixture.paths.root().join("dist/renderer.js"), old_renderer).unwrap();
        let old_receipt = std::fs::read(fixture.paths.receipt("stable")).unwrap();
        let settings = fixture.paths.root().join("settings/settings.json");
        let lock = std::cell::RefCell::new(None);
        let progress = |text: &str| {
            if text == "Hazır köprü kuruluyor…" {
                *lock.borrow_mut() = Some(
                    std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(1)
                        .open(&settings)
                        .unwrap(),
                );
            }
        };
        let error = install_with(
            &fixture.paths,
            "stable",
            &Bundle::unpack().unwrap(),
            true,
            &progress,
            |_| panic!("Loader is current"),
        )
        .err()
        .unwrap();
        assert!(
            error.contains("Önceki köprü ve ayarlar geri yüklendi"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(fixture.paths.root().join("dist/renderer.js")).unwrap(),
            old_renderer
        );
        assert_eq!(
            std::fs::read(fixture.paths.receipt("stable")).unwrap(),
            old_receipt
        );
    }
}
