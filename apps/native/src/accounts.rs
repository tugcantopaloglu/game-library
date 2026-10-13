use crate::{media, updates};
use library_core::models::Game;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};

pub fn steam(data: &Path, id: &str) -> Result<Vec<Game>, String> {
    if crate::steam_web::account_id(data).is_some() {
        return crate::steam_web::library(data, false);
    }
    if platform_windows::credentials::steam_key()?.is_none() {
        return Err(
            "Kurulu olmayan Steam oyunlarını getirmek için Steam'e bağlan düğmesini kullan.".into(),
        );
    }
    steam_api(id)
}
pub fn steam_api(id: &str) -> Result<Vec<Game>, String> {
    if id.len() != 17 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("SteamID64 17 rakam olmalı".into());
    }
    let key = platform_windows::credentials::steam_key()?
        .ok_or("Steam Web API anahtarını Steam ekranında kaydet.")?;
    let mut url = url::Url::parse("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/")
        .map_err(|error| error.to_string())?;
    url.query_pairs_mut().append_pair(
        "input_json",
        &serde_json::json!({"steamid":id,"include_appinfo":true,"include_played_free_games":true})
            .to_string(),
    );
    let mut response = media::agent(30)
        .get(url.as_str())
        .header("x-webapi-key", &key)
        .header("User-Agent", "GameLibrary/0.1.0")
        .call()
        .map_err(|_| "Steam hesap isteği başarısız; anahtar ve profil görünürlüğünü kontrol et")?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Steam yanıtı okunamadı")?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Steam hesap yanıtı boyut sınırını aşıyor".into());
    }
    library_core::owned::steam(&bytes).map_err(|error| error.to_string())
}

fn helper(data: &Path) -> PathBuf {
    data.join("integrations/legendary/legendary.exe")
}
fn command(data: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(helper(data));
    command.env(
        "LEGENDARY_CONFIG_PATH",
        updates::external_path(&data.join("integrations/legendary/config")),
    );
    command
}
pub fn prepare_epic(data: &Path) -> Result<(), String> {
    let path = helper(data);
    if path.is_file() {
        return Ok(());
    }
    let bytes = media::download(
        "https://api.github.com/repos/legendary-gl/legendary/releases/latest",
        &["api.github.com"],
        1024 * 1024,
    )?;
    let release: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Legendary sürüm bilgisi okunamadı")?;
    let asset = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|asset| asset["name"] == "legendary_windows_x64.exe")
        })
        .ok_or("Legendary Windows sürümü bulunamadı")?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("Legendary adresi eksik")?;
    let parsed = media::trusted_url(url, &["github.com"])?;
    if !parsed
        .path()
        .starts_with("/legendary-gl/legendary/releases/download/")
    {
        return Err("Legendary kaynağı geçersiz".into());
    }
    let digest = asset["digest"]
        .as_str()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or("Legendary SHA-256 özeti yayımlanmamış")?;
    let bytes = media::download(
        url,
        &[
            "github.com",
            "release-assets.githubusercontent.com",
            "objects.githubusercontent.com",
        ],
        40 * 1024 * 1024,
    )?;
    if !bytes.starts_with(b"MZ") || format!("{:x}", Sha256::digest(&bytes)) != digest {
        return Err("Legendary dosyası doğrulanamadı".into());
    }
    std::fs::create_dir_all(path.parent().ok_or("Legendary klasörü eksik")?)
        .map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}
pub fn login_epic(data: &Path) -> Result<EpicLibrary, String> {
    prepare_epic(data)?;
    // The official helper owns the Epic WebView and authentication callback.
    let mut child = command(data)
        .args(["auth"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            if !status.success() {
                return Err("Epic girişi tamamlanamadı. Giriş penceresini ve WebView2 kurulumunu kontrol et; önceki oyunlar korundu.".into());
            }
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "Epic girişi zaman aşımına uğradı; yeniden bağlan. Önceki oyunlar korundu.".into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Legendary can exit successfully after cancellation: verify the library too.
    epic(data).map_err(|_| "Epic oturumu doğrulanamadı. Girişi tamamla; pencere açılmıyorsa WebView2 kurulumunu kontrol et. Önceki oyunlar korundu.".into())
}
pub type EpicLibrary = (Vec<Game>, Vec<(String, String)>);
pub fn epic(data: &Path) -> Result<EpicLibrary, String> {
    if !data
        .join("integrations/legendary/config/user.json")
        .is_file()
    {
        return Err("Epic hesabı henüz bağlı değil. Hesabı bağla ile giriş yap; kurulu olmayan oyunlar hesabından alınır.".into());
    }
    prepare_epic(data)?;
    let output = data.join("integrations/legendary/library-result.json");
    let file = std::fs::File::create(&output).map_err(|error| error.to_string())?;
    let mut child = command(data)
        .args(["list", "--json", "--include-non-installable"])
        .stdin(std::process::Stdio::null())
        .stdout(file)
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| error.to_string())?;
    let start = std::time::Instant::now();
    loop {
        if std::fs::metadata(&output).is_ok_and(|metadata| metadata.len() > 32 * 1024 * 1024) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Epic hesap yanıtı çok büyük".into());
        }
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            if !status.success() {
                return Err("Epic hesabı bağlı değil veya oturum süresi dolmuş. Epic hesabını bağla ile giriş yap.".into());
            }
            break;
        }
        if start.elapsed().as_secs() > 120 {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Epic hesap eşitlemesi zaman aşımına uğradı; önceki liste korundu".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let bytes = std::fs::read(&output).map_err(|error| error.to_string())?;
    library_core::owned::epic(&bytes).map_err(|error| error.to_string())
}
