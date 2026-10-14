#![allow(dead_code)]
#[path = "../src/avatars.rs"]
mod avatars;
#[path = "../src/bridge.rs"]
mod bridge;
#[path = "../src/extensions.rs"]
mod extensions;
#[path = "../src/media.rs"]
mod media;
mod worker {
    pub enum Command {
        AvatarReady,
        AvatarFallback(String),
        AvatarData(String, Option<String>),
        BridgeRefresh,
    }
}
fn main() {
    if let Some(directory) = std::env::args().nth(1) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let cache = avatars::Cache::start(std::path::Path::new(&directory), sender);
        let url = "https://cdn.discordapp.com/embed/avatars/0.png";
        cache.request(url, false);
        receiver
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("Discord CDN avatarı alınamadı");
        assert!(cache.image(url).is_some());
        println!("{{\"avatarDownloadedAndDecoded\":true}}");
    }
}
