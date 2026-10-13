use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const MAX_COVER: u64 = 8 * 1024 * 1024;
const CACHE_BUDGET: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy)]
pub enum CoverUse {
    Automatic,
    Selected,
}

pub fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(seconds)))
        .max_redirects(0)
        .build()
        .into()
}

pub fn trusted_url(value: &str, hosts: &[&str]) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "Adres geçersiz".to_string())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
        || !url.host_str().is_some_and(|host| hosts.contains(&host))
    {
        return Err("Adres güvenilir HTTPS kaynaklarından biri değil".into());
    }
    Ok(url)
}

pub fn download(url: &str, hosts: &[&str], limit: u64) -> Result<Vec<u8>, String> {
    let mut url = trusted_url(url, hosts)?;
    let client = agent(45);
    let mut response = client
        .get(url.as_str())
        .header(
            "User-Agent",
            "GameLibrary/0.1.0 (https://github.com/tugcantopaloglu/game-library)",
        )
        .call()
        .map_err(|error| error.to_string())?;
    for _ in 0..3 {
        if !response.status().is_redirection() {
            break;
        }
        let location = response
            .headers()
            .get("location")
            .and_then(|header| header.to_str().ok())
            .ok_or("İndirme yönlendirmesi geçersiz")?;
        url = trusted_url(
            url.join(location)
                .map_err(|error| error.to_string())?
                .as_str(),
            hosts,
        )?;
        response = client
            .get(url.as_str())
            .header(
                "User-Agent",
                "GameLibrary/0.1.0 (https://github.com/tugcantopaloglu/game-library)",
            )
            .call()
            .map_err(|error| error.to_string())?;
    }
    if !response.status().is_success() {
        return Err("İndirme tamamlanamadı".into());
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("İndirme boyut sınırını aşıyor".into());
    }
    Ok(bytes)
}

fn validate_image(bytes: &[u8]) -> Result<(), String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err("JPEG veya PNG kapak seç.".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| "Kapak okunamadı veya piksel sınırını aşıyor".to_string())?;
    Ok(())
}

fn save_cover(data: &Path, bytes: &[u8], usage: CoverUse) -> Result<PathBuf, String> {
    if bytes.len() as u64 > MAX_COVER {
        return Err("Kapak 8 MiB sınırını aşıyor".into());
    }
    validate_image(bytes)?;
    let user = matches!(usage, CoverUse::Selected);
    let directory = data.join(if user {
        "artwork/user"
    } else {
        "artwork/cache"
    });
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let digest = format!("{:x}", Sha256::digest(bytes));
    let target = directory.join(format!("{digest}.image"));
    if !target.exists() {
        let temporary = directory.join(format!("{digest}.part"));
        std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, &target).map_err(|error| error.to_string())?;
    }
    if !user {
        trim_cache(&directory, &target, CACHE_BUDGET)?;
    }
    Ok(target)
}

pub fn import_cover(data: &Path, path: &Path) -> Result<PathBuf, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_COVER + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    save_cover(data, &bytes, CoverUse::Selected)
}

pub fn remote_cover(data: &Path, url: &str, usage: CoverUse) -> Result<PathBuf, String> {
    let bytes = download(
        url,
        &[
            "cdn2.steamgriddb.com",
            "cdn.steamgriddb.com",
            "shared.fastly.steamstatic.com",
            "cdn.cloudflare.steamstatic.com",
            "shared.steamstatic.com",
            "cdn1.epicgames.com",
            "cdn2.epicgames.com",
            "cdn1.unrealengine.com",
            "cdn2.unrealengine.com",
            "howlongtobeat.com",
        ],
        MAX_COVER,
    )?;
    save_cover(data, &bytes, usage)
}

fn trim_cache(directory: &Path, keep: &Path, budget: u64) -> Result<(), String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if metadata.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "image")
        {
            entries.push((entry.path(), metadata.len(), metadata.modified().ok()));
        }
    }
    entries.sort_by_key(|entry| entry.2);
    let mut size: u64 = entries.iter().map(|entry| entry.1).sum();
    for (path, bytes, _) in entries {
        if size <= budget {
            break;
        }
        if path != keep {
            std::fs::remove_file(path).map_err(|error| error.to_string())?;
            size -= bytes;
        }
    }
    Ok(())
}

pub struct CoverCandidate {
    pub title: String,
    pub url: String,
}

fn api_json(url: &str, key: &str) -> Result<serde_json::Value, String> {
    trusted_url(url, &["www.steamgriddb.com"])?;
    let mut response = agent(15)
        .get(url)
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(|_| {
            "SteamGridDB isteği başarısız; API anahtarı, bağlantı ve oran sınırını kontrol et"
                .to_string()
        })?;
    if !response.status().is_success() {
        return Err("SteamGridDB isteği tamamlanamadı".into());
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "SteamGridDB yanıtı okunamadı")?;
    if bytes.len() > 1024 * 1024 {
        return Err("SteamGridDB yanıtı çok büyük".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "SteamGridDB yanıtı geçersiz".into())
}

pub fn search_covers(title: &str, steam_id: Option<u32>) -> Result<Vec<CoverCandidate>, String> {
    let key = platform_windows::credentials::artwork_key()?
        .ok_or("Önce SteamGridDB API anahtarını ayarlarda kaydet.")?;
    let mut games = Vec::new();
    if let Some(id) = steam_id {
        games.push(("steam", id.to_string(), title.to_string()));
    } else {
        let mut url = url::Url::parse("https://www.steamgriddb.com/api/v2/search/autocomplete/")
            .map_err(|error| error.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "Arama adresi geçersiz")?
            .pop_if_empty()
            .push(title);
        let found = api_json(url.as_str(), &key)?;
        if let Some(values) = found["data"].as_array() {
            for game in values.iter().take(4) {
                if let (Some(id), Some(name)) = (game["id"].as_u64(), game["name"].as_str()) {
                    games.push(("game", id.to_string(), name.to_string()));
                }
            }
        }
    }
    let mut candidates = Vec::new();
    for (kind, id, name) in games {
        std::thread::sleep(Duration::from_millis(500));
        let url = format!("https://www.steamgriddb.com/api/v2/grids/{kind}/{id}?dimensions=600x900&types=static&mimes=image/png,image/jpeg&nsfw=false&humor=false");
        let found = api_json(&url, &key)?;
        if let Some(values) = found["data"].as_array() {
            for cover in values.iter().take(4) {
                if let Some(url) = cover["url"].as_str() {
                    if trusted_url(url, &["cdn2.steamgriddb.com", "cdn.steamgriddb.com"]).is_ok() {
                        candidates.push(CoverCandidate {
                            title: format!("{} · kapak {}", name, cover["id"]),
                            url: url.into(),
                        });
                    }
                }
            }
        }
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    #[test]
    fn remote_artwork_does_not_accept_local_addresses_or_oversized_input() {
        for url in [
            "http://cdn2.steamgriddb.com/a",
            "https://localhost/a",
            "https://cdn2.steamgriddb.com.evil.test/a",
            "https://user@cdn2.steamgriddb.com/a",
        ] {
            assert!(super::trusted_url(url, &["cdn2.steamgriddb.com"]).is_err());
        }
        let directory = tempfile::tempdir().unwrap();
        assert!(super::save_cover(directory.path(), &[0; 10], super::CoverUse::Selected).is_err());
    }

    #[test]
    fn selected_covers_survive_automatic_cache_eviction() {
        let directory = tempfile::tempdir().unwrap();
        let image = |color| {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::RgbaImage::from_pixel(2, 2, image::Rgba(color))
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            bytes.into_inner()
        };
        let original = image([42, 57, 66, 255]);
        let selected =
            super::save_cover(directory.path(), &original, super::CoverUse::Selected).unwrap();
        let automatic =
            super::save_cover(directory.path(), &original, super::CoverUse::Automatic).unwrap();
        let newest = super::save_cover(
            directory.path(),
            &image([100, 57, 66, 255]),
            super::CoverUse::Automatic,
        )
        .unwrap();
        super::trim_cache(&directory.path().join("artwork/cache"), &newest, 0).unwrap();
        assert!(!automatic.exists());
        assert!(newest.exists());
        assert_eq!(std::fs::read(selected).unwrap(), original);
    }
}
