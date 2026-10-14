use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
};

type Pixels = (u32, u32, Vec<u8>);
enum Request {
    Download(String, bool),
    Delivered(String, Vec<u8>),
}
#[derive(Clone)]
pub struct Cache {
    images: Arc<Mutex<HashMap<String, Pixels>>>,
    attempted: Arc<Mutex<HashMap<String, std::time::Instant>>>,
    requests: mpsc::SyncSender<Request>,
}
impl Cache {
    pub fn start(data: &Path, sender: mpsc::SyncSender<crate::worker::Command>) -> Self {
        let (requests, receiver) = mpsc::sync_channel::<Request>(64);
        let images = Arc::new(Mutex::new(HashMap::new()));
        let saved = images.clone();
        let attempted = Arc::new(Mutex::new(HashMap::new()));
        let completed = attempted.clone();
        let directory = data.join("cache/discord-avatars");
        std::thread::spawn(move || {
            while let Ok(request) = receiver.recv() {
                let (url, provided, prefer_bridge) = match request {
                    Request::Download(url, prefer_bridge) => (url, None, prefer_bridge),
                    Request::Delivered(url, bytes) => (url, Some(bytes), false),
                };
                if prefer_bridge
                    && !directory
                        .join(format!("{:x}.png", Sha256::digest(url.as_bytes())))
                        .exists()
                {
                    let _ = sender.send(crate::worker::Command::AvatarFallback(url));
                    continue;
                }
                let fallback = provided.is_none();
                if let Ok(pixels) = load(&directory, &url, provided) {
                    if let Ok(mut images) = saved.lock() {
                        if images.len() >= 256 {
                            if let Some(old) = images.keys().next().cloned() {
                                images.remove(&old);
                                if let Ok(mut attempts) = completed.lock() {
                                    attempts.remove(&old);
                                }
                            }
                        }
                        images.insert(url, pixels);
                    }
                    let _ = sender.send(crate::worker::Command::AvatarReady);
                } else if fallback {
                    let _ = sender.send(crate::worker::Command::AvatarFallback(url));
                }
            }
        });
        Self {
            images,
            attempted,
            requests,
        }
    }
    pub fn request(&self, url: &str, prefer_bridge: bool) {
        if !valid_url(url) {
            return;
        }
        if self
            .images
            .lock()
            .is_ok_and(|images| images.contains_key(url))
        {
            return;
        }
        if let Ok(mut attempted) = self.attempted.lock() {
            if attempted
                .get(url)
                .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(60))
            {
                return;
            }
            if attempted.len() >= 512 {
                attempted.clear();
            }
            if self
                .requests
                .try_send(Request::Download(url.into(), prefer_bridge))
                .is_ok()
            {
                attempted.insert(url.into(), std::time::Instant::now());
            }
        }
    }
    pub fn deliver(&self, url: String, encoded: &str) {
        use base64::Engine;
        if !valid_url(&url) || encoded.len() > 90000 {
            return;
        }
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) {
            if bytes.len() <= 65536 {
                let _ = self.requests.try_send(Request::Delivered(url, bytes));
            }
        }
    }
    pub fn image(&self, url: &str) -> Option<slint::Image> {
        let images = self.images.lock().ok()?;
        let (width, height, bytes) = images.get(url)?;
        let pixels =
            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(bytes, *width, *height);
        Some(slint::Image::from_rgba8(pixels))
    }
}
fn valid_url(value: &str) -> bool {
    crate::media::trusted_url(value, &["cdn.discordapp.com"]).is_ok_and(|url| {
        url.path().starts_with("/avatars/") || url.path().starts_with("/embed/avatars/")
    }) && value.len() <= 512
}
fn load(directory: &Path, url: &str, provided: Option<Vec<u8>>) -> Result<Pixels, String> {
    if !valid_url(url) {
        return Err("Avatar adresi geçersiz".into());
    }
    let path = directory.join(format!("{:x}.png", Sha256::digest(url.as_bytes())));
    let cached = std::fs::File::open(&path).ok().and_then(|file| {
        let mut bytes = vec![];
        file.take(256 * 1024 + 1).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    });
    let bytes = if let Some(bytes) = provided.or(cached) {
        bytes
    } else {
        let mut response = crate::media::agent(8)
            .get(url)
            .call()
            .map_err(|_| "Avatar indirilemedi")?;
        let mut bytes = vec![];
        response
            .body_mut()
            .as_reader()
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Avatar okunamadı")?;
        bytes
    };
    if bytes.len() > 256 * 1024 {
        return Err("Avatar boyut sınırını aşıyor".into());
    }
    let mut reader = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "Avatar biçimi geçersiz")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(512);
    limits.max_image_height = Some(512);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Avatar çözülemedi")?
        .thumbnail(64, 64)
        .into_rgba8();
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
        .map(|entry| entry.path())
        .collect();
    files.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|value| value.modified())
            .ok()
    });
    for stale in files.iter().take(files.len().saturating_sub(255)) {
        let _ = std::fs::remove_file(stale);
    }
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok((image.width(), image.height(), image.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivered_images_use_the_bounded_decode_worker() {
        use base64::Engine;
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::sync_channel(4);
        let cache = Cache::start(directory.path(), sender);
        let mut bytes = Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([255, 0, 0, 255]),
        ))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
        let url = "https://cdn.discordapp.com/embed/avatars/0.png";
        cache.deliver(
            url.into(),
            &base64::engine::general_purpose::STANDARD.encode(bytes.into_inner()),
        );
        assert!(matches!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            crate::worker::Command::AvatarReady
        ));
        assert!(cache.image(url).is_some());
        cache.deliver("https://evil.test/image.png".into(), "invalid");
        assert!(receiver
            .recv_timeout(std::time::Duration::from_millis(20))
            .is_err());
    }
    #[test]
    fn avatar_downloads_are_restricted_to_discord_images() {
        assert!(valid_url(
            "https://cdn.discordapp.com/avatars/123/abc.png?size=64"
        ));
        assert!(valid_url("https://cdn.discordapp.com/embed/avatars/0.png"));
        for url in [
            "https://evil.test/avatars/a.png",
            "http://cdn.discordapp.com/avatars/a.png",
            "https://cdn.discordapp.com/attachments/a.png",
            "https://cdn.discordapp.com:444/avatars/a.png",
        ] {
            assert!(!valid_url(url));
        }
    }
}
