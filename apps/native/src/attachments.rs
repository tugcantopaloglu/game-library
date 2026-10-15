//! Small, account-scoped image previews; full attachments open only on an explicit click.
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::{mpsc, Arc, Mutex},
};
type Pixels = slint::SharedPixelBuffer<slint::Rgba8Pixel>;
#[derive(Default)]
struct Images {
    account: String,
    pixels: HashMap<String, Pixels>,
    attempted: std::collections::HashSet<String>,
}
#[derive(Clone)]
pub struct Cache {
    images: Arc<Mutex<Images>>,
    requests: mpsc::SyncSender<(String, String)>,
}
impl Cache {
    pub fn start(sender: mpsc::SyncSender<crate::worker::Command>) -> Self {
        let images = Arc::new(Mutex::new(Images::default()));
        let saved = images.clone();
        let (requests, receiver) = mpsc::sync_channel::<(String, String)>(8);
        std::thread::spawn(move || {
            while let Ok((account, url)) = receiver.recv() {
                if !saved.lock().is_ok_and(|images| images.account == account) {
                    continue;
                }
                if let Ok(pixels) = download(&url) {
                    if let Ok(mut images) = saved.lock() {
                        if images.account != account {
                            continue;
                        }
                        if images.pixels.len() >= 16 {
                            images.pixels.clear();
                        }
                        images.pixels.insert(url, pixels);
                    }
                    let _ = sender.send(crate::worker::Command::AvatarReady);
                }
            }
        });
        Self { images, requests }
    }
    pub fn account(&self, account: &str) {
        if let Ok(mut images) = self.images.lock() {
            if images.account != account {
                *images = Images {
                    account: account.into(),
                    ..Default::default()
                };
            }
        }
    }
    pub fn request(&self, account: &str, attachment: &crate::chat::Attachment) {
        if !attachment.image {
            return;
        }
        if let Ok(mut images) = self.images.lock() {
            if images.account != account
                || images.attempted.contains(&attachment.url)
                || images.attempted.len() >= 150
            {
                return;
            }
            if self
                .requests
                .try_send((account.into(), attachment.url.clone()))
                .is_ok()
            {
                images.attempted.insert(attachment.url.clone());
            }
        }
    }
    pub fn image(&self, url: &str) -> Option<slint::Image> {
        let images = self.images.lock().ok()?;
        Some(slint::Image::from_rgba8(images.pixels.get(url)?.clone()))
    }
}
fn download(url: &str) -> Result<Pixels, String> {
    let mut response = crate::media::agent(8)
        .get(url)
        .call()
        .map_err(|_| "Görsel alınamadı")?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Görsel okunamadı")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Görsel boyut sınırı".into());
    }
    decode(bytes)
}
fn decode(bytes: Vec<u8>) -> Result<Pixels, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Görsel biçimi")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Görsel çözülemedi")?
        .thumbnail(640, 360)
        .into_rgba8();
    Ok(Pixels::clone_from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previews_are_bounded_shared_pixels_and_bad_media_is_rejected() {
        let image = image::RgbaImage::from_pixel(1200, 800, image::Rgba([20, 40, 60, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let pixels = decode(bytes.into_inner()).unwrap();
        assert!(pixels.width() <= 640 && pixels.height() <= 360);
        let shared = pixels.clone();
        assert_eq!(pixels.as_bytes().as_ptr(), shared.as_bytes().as_ptr());
        assert!(decode(b"not an image".to_vec()).is_err());
    }
}
