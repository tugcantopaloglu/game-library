//! Opt-in Windows network tools. Inspection/update staging never changes the network.
//! Only explicit DNS/start/stop calls invoke Windows UAC; rollback and process ownership
//! records live in an administrator-protected directory, independent of mutable UI state.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const RELEASES_URL: &str = "https://github.com/ValdikSS/GoodbyeDPI/releases";
pub const CLOUDFLARE_DOCS: &str = "https://developers.cloudflare.com/1.1.1.1/setup/windows/";
pub const CLOUDFLARE_DOH: &str = "https://cloudflare-dns.com/dns-query";
pub const CLOUDFLARE_IPV4: [&str; 2] = ["1.1.1.1", "1.0.0.1"];
pub const CLOUDFLARE_IPV6: [&str; 2] = ["2606:4700:4700::1111", "2606:4700:4700::1001"];
const RELEASE_API: &str = "https://api.github.com/repos/ValdikSS/GoodbyeDPI/releases/latest";
const HELPER: &str = include_str!("../../../scripts/native-network.ps1");
const MAX_ZIP: u64 = 32 * 1024 * 1024;
const CHECK_INTERVAL: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    /// Opt-in background check/staging only; never starts a process or changes DNS.
    pub auto_update: bool,
    pub last_check: u64,
    pub last_update_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Release {
    pub version: String,
    pub tag: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    /// GitHub's published SHA-256, when supplied. Older official assets omit it.
    pub published_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageFile {
    pub relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Package {
    pub release: Release,
    pub archive_sha256: String,
    pub directory: String,
    pub executable: String,
    pub files: Vec<PackageFile>,
    pub publisher_digest_verified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub windows: bool,
    pub preferences: Preferences,
    pub prepared_version: Option<String>,
    pub publisher_digest_verified: bool,
    pub runtime: Value,
    pub error: Option<String>,
}

impl Report {
    pub fn summary(&self) -> String {
        let dns = if self.runtime["dnsEnabled"] == true {
            "Cloudflare DNS açık; önceki ayarlar geri yüklenebilir."
        } else {
            "Cloudflare DNS uygulama tarafından etkinleştirilmedi."
        };
        let dpi = if self.runtime["running"] == true {
            "GoodbyeDPI çalışıyor."
        } else if self.prepared_version.is_some() {
            "GoodbyeDPI hazır; başlatmak için Windows izni gerekir."
        } else {
            "GoodbyeDPI paketi henüz hazırlanmadı."
        };
        let update = if self.preferences.auto_update {
            "Otomatik paket güncellemesi açık; yeni sürüm sonraki başlatmada uygulanır."
        } else {
            "Otomatik paket güncellemesi kapalı."
        };
        let error = self
            .preferences
            .last_update_error
            .as_ref()
            .map(|error| format!(" Güncelleme: {error}"))
            .unwrap_or_default();
        let recovery = self.runtime["dnsError"]
            .as_str()
            .map(|error| format!(" Geri alma: {error}"))
            .unwrap_or_default();
        let metadata_error = self
            .error
            .as_ref()
            .map(|error| format!(" Kayıt/inceleme: {error}"))
            .unwrap_or_default();
        format!("{dns} {dpi} {update}{error}{recovery}{metadata_error}")
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn directory(data: &Path) -> PathBuf {
    data.join("network-tools")
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Ağ araçları kayıt boyutu geçersiz".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn save_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("next.json");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    // Windows rename replaces an existing regular file; no remove-before-rename window.
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn preferences(data: &Path) -> Result<Preferences, String> {
    let path = directory(data).join("preferences.json");
    if path.exists() {
        read_json(&path, 8 * 1024)
    } else {
        Ok(Preferences::default())
    }
}

pub fn set_auto_update(data: &Path, enabled: bool) -> Result<(), String> {
    // An explicit preference click can repair malformed mutable preferences.
    // It never infers opt-in from an unreadable file.
    let mut value = preferences(data).unwrap_or_default();
    value.auto_update = enabled;
    if enabled {
        value.last_check = 0;
    }
    std::fs::create_dir_all(directory(data)).map_err(|error| error.to_string())?;
    save_json(&directory(data).join("preferences.json"), &value)
}

pub fn inspect(data: &Path) -> Report {
    let mut report = Report {
        windows: cfg!(windows),
        preferences: Preferences::default(),
        prepared_version: None,
        publisher_digest_verified: false,
        runtime: json!({}),
        error: None,
    };
    let mut errors = Vec::new();
    match preferences(data) {
        Ok(value) => report.preferences = value,
        Err(error) => errors.push(format!("Tercihler okunamadı: {error}")),
    }
    match prepared(data) {
        Ok(Some(package)) => {
            report.prepared_version = Some(package.release.version);
            report.publisher_digest_verified = package.publisher_digest_verified;
        }
        Ok(None) => {}
        Err(error) => errors.push(format!("Hazır paket okunamadı: {error}")),
    }
    // Protected DNS recovery and running-process inspection must stay available
    // even if mutable preferences/prepared.json are damaged or tampered with.
    match helper(data, "inspect", json!({}), false) {
        Ok(runtime) => report.runtime = runtime,
        Err(error) => errors.push(format!("Windows incelemesi: {error}")),
    }
    report.error = (!errors.is_empty()).then(|| errors.join(" · "));
    report
}

fn release_version(value: &str) -> Result<semver::Version, String> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let version = semver::Version::parse(value).map_err(|_| "GoodbyeDPI sürümü geçersiz")?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err("Yalnız kararlı GoodbyeDPI sürümleri desteklenir".into());
    }
    Ok(version)
}

fn parse_release(value: &Value) -> Result<Release, String> {
    if value["draft"] == true || value["prerelease"] == true {
        return Err("Kararlı GoodbyeDPI sürümü yok".into());
    }
    let tag = value["tag_name"]
        .as_str()
        .ok_or("GoodbyeDPI sürüm etiketi yok")?;
    let version = release_version(tag)?;
    let assets = value["assets"]
        .as_array()
        .ok_or("GoodbyeDPI sürüm dosyası yok")?;
    let expected_name = format!("goodbyedpi-{version}.zip");
    let asset = assets
        .iter()
        .find(|asset| asset["name"] == expected_name)
        .ok_or("Resmi GoodbyeDPI ZIP paketi yok")?;
    let release = Release {
        version: version.to_string(),
        tag: tag.into(),
        asset_name: expected_name,
        asset_url: asset["browser_download_url"]
            .as_str()
            .ok_or("GoodbyeDPI dosya adresi yok")?
            .into(),
        asset_size: asset["size"]
            .as_u64()
            .ok_or("GoodbyeDPI dosya boyutu yok")?,
        published_sha256: match asset["digest"].as_str() {
            None | Some("") => None,
            Some(digest) => Some(
                digest
                    .strip_prefix("sha256:")
                    .ok_or("Desteklenmeyen yayıncı özeti")?
                    .to_ascii_lowercase(),
            ),
        },
    };
    validate_release(&release)?;
    Ok(release)
}

fn digest_valid(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_release(release: &Release) -> Result<(), String> {
    let version = release_version(&release.tag)?;
    let url = https_url(&release.asset_url)?;
    let expected = format!(
        "/ValdikSS/GoodbyeDPI/releases/download/{}/goodbyedpi-{version}.zip",
        release.tag
    );
    if release.version != version.to_string()
        || release.asset_name != format!("goodbyedpi-{version}.zip")
        || url.host_str() != Some("github.com")
        || url.path() != expected
        || url.query().is_some()
        || url.fragment().is_some()
        || release.asset_size == 0
        || release.asset_size > MAX_ZIP
        || release
            .published_sha256
            .as_deref()
            .is_some_and(|digest| !digest_valid(digest))
    {
        return Err("GoodbyeDPI sürüm veya resmi paket kaynağı geçersiz".into());
    }
    Ok(())
}

fn https_url(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "Ağ aracı adresi geçersiz")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
    {
        return Err("Ağ aracı kaynağı HTTPS olmalı".into());
    }
    Ok(url)
}

fn redirect_allowed(url: &url::Url, initial: &url::Url) -> bool {
    match url.host_str() {
        Some("github.com") => url.path() == initial.path() && url.query().is_none(),
        Some("release-assets.githubusercontent.com") => {
            url.path().starts_with("/github-production-release-asset/")
        }
        Some("objects.githubusercontent.com") => {
            url.path().starts_with("/github-production-release-asset-")
        }
        _ => false,
    }
}

fn download(value: &str, limit: u64, release_asset: bool) -> Result<Vec<u8>, String> {
    let initial = https_url(value)?;
    if (!release_asset && value != RELEASE_API)
        || (release_asset && initial.host_str() != Some("github.com"))
    {
        return Err("Ağ aracı kaynağı resmi repo değil".into());
    }
    let client: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(45)))
        .max_redirects(0)
        .build()
        .into();
    let mut current = initial.clone();
    for _ in 0..=3 {
        let mut response = client
            .get(current.as_str())
            .header(
                "User-Agent",
                "GameLibrary-Network/0.1 (github.com/tugcantopaloglu/game-library)",
            )
            .call()
            .map_err(|error| error.to_string())?;
        if response.status().is_redirection() {
            if !release_asset {
                return Err("Sürüm API yönlendirmesi reddedildi".into());
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or("Paket yönlendirmesi geçersiz")?;
            current = https_url(
                current
                    .join(location)
                    .map_err(|error| error.to_string())?
                    .as_str(),
            )?;
            if !redirect_allowed(&current, &initial) {
                return Err("Paket farklı kaynağa yönlendi".into());
            }
            continue;
        }
        if !response.status().is_success() {
            return Err("GoodbyeDPI indirilemedi".into());
        }
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > limit {
            return Err("Paket boyut sınırını aşıyor".into());
        }
        return Ok(bytes);
    }
    Err("Paket yönlendirme sınırını aşıyor".into())
}

pub fn check_update(data: &Path) -> Result<Option<Release>, String> {
    let bytes = download(RELEASE_API, 256 * 1024, false)?;
    let release = parse_release(
        &serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string())?,
    )?;
    if let Some(package) = prepared(data)? {
        if release_version(&release.version)? <= release_version(&package.release.version)? {
            return Ok(None);
        }
    }
    Ok(Some(release))
}

fn verify_archive(release: &Release, bytes: &[u8]) -> Result<String, String> {
    validate_release(release)?;
    let digest = format!("{:x}", Sha256::digest(bytes));
    if bytes.len() as u64 != release.asset_size
        || !bytes.starts_with(b"PK\x03\x04")
        || release
            .published_sha256
            .as_ref()
            .is_some_and(|expected| expected != &digest)
    {
        return Err("Resmi paket boyut veya SHA-256 doğrulamasını geçmedi".into());
    }
    Ok(digest)
}

pub fn prepare_latest(data: &Path) -> Result<Package, String> {
    match check_update(data)? {
        Some(release) => prepare(data, &release),
        None => prepared(data)?.ok_or_else(|| "GoodbyeDPI paketi yok".into()),
    }
}

pub fn prepare(data: &Path, release: &Release) -> Result<Package, String> {
    validate_release(release)?;
    let bytes = download(&release.asset_url, release.asset_size, true)?;
    let digest = verify_archive(release, &bytes)?;
    let root = directory(data);
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let archive = root.join(format!("package-{}-{}.zip", release.version, &digest[..16]));
    std::fs::write(&archive, bytes).map_err(|error| error.to_string())?;
    let result = helper(
        data,
        "extract",
        json!({"archive":external_path(&archive), "archiveSha256":digest, "version":release.version}),
        false,
    );
    let _ = std::fs::remove_file(&archive);
    let result = result?;
    let package = Package {
        release: release.clone(),
        archive_sha256: digest,
        directory: result["directory"]
            .as_str()
            .ok_or("Paket klasörü bulunamadı")?
            .into(),
        executable: result["executable"]
            .as_str()
            .ok_or("GoodbyeDPI dosyası bulunamadı")?
            .into(),
        files: serde_json::from_value(result["files"].clone())
            .map_err(|error| error.to_string())?,
        publisher_digest_verified: release.published_sha256.is_some(),
    };
    validate_package(data, &package)?;
    save_json(&root.join("prepared.json"), &package)?;
    Ok(package)
}

pub fn auto_update_if_due(data: &Path) -> Result<bool, String> {
    let mut preference = preferences(data)?;
    if !preference.auto_update
        || (preference.last_check != 0
            && now().saturating_sub(preference.last_check) < CHECK_INTERVAL)
    {
        return Ok(false);
    }
    preference.last_check = now();
    let result = (|| -> Result<bool, String> {
        if let Some(release) = check_update(data)? {
            prepare(data, &release)?;
            return Ok(true);
        }
        Ok(false)
    })();
    preference.last_update_error = result.as_ref().err().cloned();
    save_json(&directory(data).join("preferences.json"), &preference)?;
    result
}

fn prepared(data: &Path) -> Result<Option<Package>, String> {
    let path = directory(data).join("prepared.json");
    if !path.exists() {
        return Ok(None);
    }
    let value: Package = read_json(&path, 64 * 1024)?;
    validate_package(data, &value)?;
    Ok(Some(value))
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && !value.contains('\\')
        && !value.contains(':')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}

fn validate_package(data: &Path, value: &Package) -> Result<(), String> {
    validate_release(&value.release)?;
    if !digest_valid(&value.archive_sha256)
        || value.files.is_empty()
        || value.files.len() > 32
        || !safe_relative(&value.executable)
        || !value.executable.ends_with("/goodbyedpi.exe")
        || value.publisher_digest_verified != value.release.published_sha256.is_some()
        || value
            .release
            .published_sha256
            .as_ref()
            .is_some_and(|digest| digest != &value.archive_sha256)
    {
        return Err("Hazırlanan GoodbyeDPI paketi geçersiz".into());
    }
    let root = std::fs::canonicalize(directory(data).join("packages"))
        .map_err(|error| error.to_string())?;
    let package = std::fs::canonicalize(&value.directory).map_err(|error| error.to_string())?;
    let expected_name = format!("{}-{}", value.release.version, &value.archive_sha256[..16]);
    if !package.starts_with(&root)
        || package.file_name().and_then(|n| n.to_str()) != Some(&expected_name)
        || !value
            .files
            .iter()
            .any(|file| file.relative_path == value.executable)
    {
        return Err("GoodbyeDPI paket yolu geçersiz".into());
    }
    for file in &value.files {
        if !safe_relative(&file.relative_path)
            || !digest_valid(&file.sha256)
            || file.size == 0
            || file.size > MAX_ZIP
        {
            return Err("GoodbyeDPI paket kaydı geçersiz".into());
        }
        let path = std::fs::canonicalize(package.join(&file.relative_path))
            .map_err(|error| error.to_string())?;
        if !path.starts_with(&package) {
            return Err("Paket dosyası klasör dışında".into());
        }
        let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        if bytes.len() as u64 != file.size || format!("{:x}", Sha256::digest(&bytes)) != file.sha256
        {
            return Err("GoodbyeDPI paket dosyası değişmiş; yeniden hazırla".into());
        }
    }
    Ok(())
}

/// The UI must use a stable GUID from inspect().runtime.interfaces, never an alias/index.
pub fn enable_dns(data: &Path, interface_guid: &str, encrypted: bool) -> Result<Value, String> {
    let guid = interface_guid.trim_matches(['{', '}']);
    if guid.len() != 36
        || !guid.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err("Bir ağ bağdaştırıcısı seç".into());
    }
    helper(
        data,
        "enable-dns",
        json!({"interfaceGuid":guid, "encrypted":encrypted}),
        true,
    )
}

pub fn restore_dns(data: &Path) -> Result<Value, String> {
    helper(data, "restore-dns", json!({}), true)
}

pub fn start(data: &Path) -> Result<Value, String> {
    let package = prepared(data)?.ok_or("Önce GoodbyeDPI paketini hazırla")?;
    helper(
        data,
        "start",
        json!({"package":package, "appPid":std::process::id(), "appExecutable":external_path(&std::env::current_exe().map_err(|error| error.to_string())?)}),
        true,
    )
}

pub fn stop(data: &Path) -> Result<Value, String> {
    helper(data, "stop", json!({}), true)
}

fn external_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(value) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{value}")
    } else {
        value.trim_start_matches("\\\\?\\").into()
    }
}

fn powershell_invocation(
    bootstrap_data: &str,
    helper_length: usize,
    powershell: &Path,
    elevated: bool,
) -> (&'static str, String) {
    // Only Base64 is interpolated into PowerShell source; file content is read once,
    // hashed against the compiled helper, then executed from those same bytes.
    let bootstrap = r#"$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';$d=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__BOOTSTRAP__'))|ConvertFrom-Json;$f=[IO.File]::Open($d.script,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read);try{if($f.Length -ne __HELPER_SIZE__){throw 'Network helper length check failed'};$b=[byte[]]::new(__HELPER_SIZE__);$o=0;while($o -lt $b.Length){$n=$f.Read($b,$o,$b.Length-$o);if($n -eq 0){throw 'Network helper read failed'};$o+=$n}}finally{$f.Dispose()};$h=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($b)).Replace('-','').ToLowerInvariant();if($h -ne $d.digest){throw 'Network helper integrity check failed'};& ([scriptblock]::Create([Text.Encoding]::UTF8.GetString($b))) -RequestBase64 $d.request"#
        .replace("__BOOTSTRAP__", bootstrap_data)
        .replace("__HELPER_SIZE__", &helper_length.to_string());
    let encode = |value: &str| {
        base64::engine::general_purpose::STANDARD.encode(
            value
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
    };
    let encoded = encode(&bootstrap);
    let command = if elevated {
        let shell = base64::engine::general_purpose::STANDARD.encode(external_path(powershell));
        // Start-Process -Wait waits for descendants too on Windows. The network
        // helper deliberately creates a session child/guardian, so wait only for
        // the elevated helper's own process handle to receive its prompt result.
        // Elevated code writes only administrator-owned output. This outer
        // unelevated launcher copies the result into mutable LocalAppData,
        // preventing a hardlink/junction race from becoming a privileged write.
        r#"$ErrorActionPreference='Stop';$p=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__SHELL__'));try{$c=Start-Process -FilePath $p -Verb RunAs -WindowStyle Hidden -ArgumentList @('-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand','__ENCODED__') -PassThru;$c.WaitForExit();$code=$c.ExitCode;$c.Dispose();$d=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__BOOTSTRAP__'))|ConvertFrom-Json;$r=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($d.request))|ConvertFrom-Json;$identity=[Text.Encoding]::UTF8.GetBytes([IO.Path]::GetFullPath($r.dataDirectory).TrimEnd('\').ToLowerInvariant());$h=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($identity)).Replace('-','').ToLowerInvariant().Substring(0,24);$root=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) ('GameLibraryNetwork\'+$r.callerSid+'\'+$h);$result=Join-Path $root ('responses\'+[IO.Path]::GetFileName($r.response));if([IO.File]::Exists($result)){[IO.File]::WriteAllBytes($r.response,[IO.File]::ReadAllBytes($result))}elseif($code -ne 0){[Console]::Error.WriteLine('Windows yönetici işlemi tamamlanamadı; ağ durumu yeniden okunacak.')}exit $code}catch{[Console]::Error.WriteLine('Windows izni verilmedi veya yönetici işlemi başlatılamadı.');exit 1}"#
            .replace("__SHELL__", &shell)
            .replace("__ENCODED__", &encoded)
            .replace("__BOOTSTRAP__", bootstrap_data)
    } else {
        encoded
    };
    // Re-encoding the outer launcher exceeds CreateProcessW's limit for the
    // official three-file package at the normal LocalAppData path. All dynamic
    // values remain Base64 data and the elevated bootstrap remains encoded.
    (
        if elevated {
            "-Command"
        } else {
            "-EncodedCommand"
        },
        command,
    )
}

fn windows_argument_units(value: &str, force_quotes: bool) -> usize {
    if !force_quotes && !value.is_empty() && !value.contains([' ', '\t', '"']) {
        return value.encode_utf16().count();
    }
    let mut units = 2;
    let mut backslashes = 0;
    for character in value.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '"' {
            units += backslashes * 2 + 2;
            backslashes = 0;
        } else {
            units += backslashes + character.len_utf16();
            backslashes = 0;
        }
    }
    units + backslashes * 2
}

fn windows_command_line_units(powershell: &Path, mode: &str, command: &str) -> usize {
    let executable = external_path(powershell);
    windows_argument_units(&executable, true)
        + ["-NoLogo", "-NoProfile", "-NonInteractive", mode, command]
            .iter()
            .map(|argument| 1 + windows_argument_units(argument, false))
            .sum::<usize>()
        + 1 // terminating NUL is included in CreateProcessW's 32767 limit
}

#[cfg(windows)]
fn helper(data: &Path, action: &str, parameters: Value, elevated: bool) -> Result<Value, String> {
    use std::os::windows::process::CommandExt;
    // The system executable comes from protected HKLM, never the user PATH.
    let key = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        .map_err(|error| error.to_string())?;
    let system_root: String = key
        .get_value("SystemRoot")
        .map_err(|error| error.to_string())?;
    let powershell =
        PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    static CALLER_SID: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    let caller_sid = CALLER_SID
        .get_or_init(|| {
            let output = std::process::Command::new(&powershell)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
                ])
                .creation_flags(0x08000000)
                .output()
                .map_err(|error| error.to_string())?;
            let sid = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !output.status.success()
                || !sid.starts_with("S-1-")
                || sid.len() > 128
                || !sid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b"S-".contains(&b))
            {
                return Err("Windows kullanıcı kimliği doğrulanamadı".into());
            }
            Ok(sid)
        })
        .clone()?;
    let root = directory(data);
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|error| error.to_string())?;
    let nonce = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let script = root.join(format!("helper-{nonce}.ps1"));
    let response = root.join(format!("response-{nonce}.json"));
    std::fs::write(&script, HELPER.as_bytes()).map_err(|error| error.to_string())?;
    let script_digest = format!("{:x}", Sha256::digest(HELPER.as_bytes()));
    let request = json!({"action":action,"dataDirectory":external_path(data),"response":external_path(&response),"callerSid":caller_sid,"parameters":parameters});
    let request = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(&request).map_err(|error| error.to_string())?);
    let bootstrap_data = base64::engine::general_purpose::STANDARD.encode(
        serde_json::to_vec(
            &json!({"script":external_path(&script),"digest":script_digest,"request":request}),
        )
        .map_err(|error| error.to_string())?,
    );
    let (command_mode, command) =
        powershell_invocation(&bootstrap_data, HELPER.len(), &powershell, elevated);
    if windows_command_line_units(&powershell, command_mode, &command) > 32767 {
        let _ = std::fs::remove_file(&script);
        return Err(
            "Ağ aracı komutu Windows uzunluk sınırını aşıyor; daha kısa bir veri klasörü seç"
                .into(),
        );
    }
    let result = std::process::Command::new(&powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            command_mode,
            &command,
        ])
        .creation_flags(0x08000000)
        .output();
    let result = match result {
        Ok(_output) if response.is_file() => {
            read_json::<Value>(&response, 256 * 1024).and_then(|result| {
                if result["ok"] == true {
                    Ok(result["value"].clone())
                } else {
                    Err(result["error"]
                        .as_str()
                        .unwrap_or("Ağ aracı işlemi başarısız")
                        .into())
                }
            })
        }
        Ok(output) => {
            let error = String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(600)
                .collect::<String>();
            Err(if error.is_empty() {
                "Ağ aracı yanıt vermedi; Windows izni veya PowerShell durumunu denetle".into()
            } else {
                error
            })
        }
        Err(error) => Err(error.to_string()),
    };
    let _ = std::fs::remove_file(script);
    let _ = std::fs::remove_file(response);
    result
}

#[cfg(not(windows))]
fn helper(
    _data: &Path,
    _action: &str,
    _parameters: Value,
    _elevated: bool,
) -> Result<Value, String> {
    Err("Ağ araçları yalnız Windows üzerinde kullanılabilir".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(digest: Value) -> Value {
        json!({"tag_name":"0.2.2","draft":false,"prerelease":false,"assets":[{"name":"goodbyedpi-0.2.2.zip","size":10,"digest":digest,"browser_download_url":"https://github.com/ValdikSS/GoodbyeDPI/releases/download/0.2.2/goodbyedpi-0.2.2.zip"}]})
    }
    #[test]
    fn release_source_and_channel_are_restricted() {
        assert!(parse_release(&release(Value::Null)).is_ok());
        let mut value = release(Value::Null);
        value["assets"][0]["browser_download_url"] = json!(
            "https://github.com/Other/GoodbyeDPI/releases/download/0.2.2/goodbyedpi-0.2.2.zip"
        );
        assert!(parse_release(&value).is_err());
        value = release(Value::Null);
        value["prerelease"] = json!(true);
        assert!(parse_release(&value).is_err());
        assert!(release_version("0.2.3rc3").is_err());
        assert!(release_version("v0.2.2").unwrap() < release_version("0.3.0").unwrap());
    }
    #[test]
    fn published_digest_is_mandatory_when_present() {
        let bytes = b"PK\x03\x04abcdef";
        let good = format!("sha256:{:x}", Sha256::digest(bytes));
        assert!(verify_archive(&parse_release(&release(json!(good))).unwrap(), bytes).is_ok());
        assert!(verify_archive(
            &parse_release(&release(json!(format!("sha256:{}", "0".repeat(64))))).unwrap(),
            bytes
        )
        .is_err());
        assert!(parse_release(&release(json!("md5:abcd"))).is_err());
        assert!(parse_release(&release(json!("sha256:short"))).is_err());
    }
    #[test]
    fn redirects_and_package_paths_are_bounded() {
        let initial = https_url(
            "https://github.com/ValdikSS/GoodbyeDPI/releases/download/0.2.2/goodbyedpi-0.2.2.zip",
        )
        .unwrap();
        assert!(redirect_allowed(&https_url("https://release-assets.githubusercontent.com/github-production-release-asset/1/2?token=ok").unwrap(), &initial));
        for bad in [
            "https://evil.example/file",
            "https://github.com/Other/file",
            "https://release-assets.githubusercontent.com/unrelated",
        ] {
            assert!(!redirect_allowed(&https_url(bad).unwrap(), &initial));
        }
        assert!(https_url("http://github.com/file").is_err());
        assert!(https_url("https://user@github.com/file").is_err());
        for bad in [
            "../evil.exe",
            "x/../../evil.exe",
            "C:/evil.exe",
            "x\\evil.exe",
            "x/file:stream",
            "/evil.exe",
            "x//evil.exe",
            "x/evil. ",
        ] {
            assert!(!safe_relative(bad), "{bad}");
        }
        assert!(safe_relative("x86_64/WinDivert64.sys"));
    }
    #[test]
    fn auto_update_is_off_by_default_and_requires_no_network() {
        let data = tempfile::tempdir().unwrap();
        assert!(!auto_update_if_due(data.path()).unwrap());
        assert!(!data.path().join("network-tools").exists());
        set_auto_update(data.path(), true).unwrap();
        let mut value = preferences(data.path()).unwrap();
        value.last_check = now();
        save_json(&directory(data.path()).join("preferences.json"), &value).unwrap();
        assert!(!auto_update_if_due(data.path()).unwrap());
        set_auto_update(data.path(), false).unwrap();
        assert!(!preferences(data.path()).unwrap().auto_update);
    }

    #[test]
    fn official_start_payload_fits_windows_command_line_without_outer_encoding() {
        // Real official 0.2.2 x86_64 paths/sizes; hashes have the same fixed length
        // as the release. This builds command arguments only and never spawns PS.
        let data = r"C:\Users\topal\AppData\Local\GameLibrary";
        let nonce = "0123456789abcdef0123456789abcdef";
        let mut published = parse_release(&release(Value::Null)).unwrap();
        published.asset_size = 635551;
        let package = Package {
            release: published,
            archive_sha256: "a".repeat(64),
            directory: format!(r"{data}\network-tools\packages\0.2.2-aaaaaaaaaaaaaaaa"),
            executable: "goodbyedpi-0.2.2/x86_64/goodbyedpi.exe".into(),
            files: [
                ("WinDivert.dll", 23552),
                ("WinDivert64.sys", 50592),
                ("goodbyedpi.exe", 75264),
            ]
            .into_iter()
            .map(|(name, size)| PackageFile {
                relative_path: format!("goodbyedpi-0.2.2/x86_64/{name}"),
                size,
                sha256: "a".repeat(64),
            })
            .collect(),
            publisher_digest_verified: false,
        };
        let request = json!({"action":"start", "dataDirectory":data,
            "response":format!(r"{data}\network-tools\response-{nonce}.json"),
            "callerSid":"S-1-5-21-3415448987-74617474-1444610274-1001",
            "parameters":{"package":package,"appPid":u32::MAX,
                "appExecutable":r"F:\game-library\target\release\game-library-native.exe"}});
        let request =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&request).unwrap());
        let bootstrap = base64::engine::general_purpose::STANDARD.encode(
            serde_json::to_vec(&json!({
            "script":format!(r"{data}\network-tools\helper-{nonce}.ps1"),
            "digest":"b".repeat(64),"request":request}))
            .unwrap(),
        );
        let powershell = Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        let (mode, command) = powershell_invocation(&bootstrap, HELPER.len(), powershell, true);
        assert_eq!(mode, "-Command");
        let fixed = windows_command_line_units(powershell, mode, &command);
        let old_command = base64::engine::general_purpose::STANDARD.encode(
            command
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let old = windows_command_line_units(powershell, "-EncodedCommand", &old_command);
        assert!(
            old > 32767,
            "Original default-path overflow was not reproduced: {old}"
        );
        assert!(
            fixed < 32767,
            "Default-path command still exceeds the Windows limit: {fixed}"
        );
        let (_, inner) = powershell_invocation(&bootstrap, HELPER.len(), powershell, false);
        assert!(windows_command_line_units(powershell, "-EncodedCommand", &inner) < 32767);
        let large = base64::engine::general_purpose::STANDARD.encode(vec![b'x'; 16000]);
        let (mode, command) = powershell_invocation(&large, HELPER.len(), powershell, true);
        assert!(windows_command_line_units(powershell, mode, &command) > 32767);
        println!("Official default start: previous {old}, fixed {fixed} UTF-16 units including NUL; no process started.");
    }
}
