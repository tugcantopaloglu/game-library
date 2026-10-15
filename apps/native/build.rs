fn main() {
    use sha2::{Digest, Sha256};
    let bridge = std::path::Path::new("../../integrations/vencord");
    let manifest_path = bridge.join("prebuilt/bundle.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("Hazır köprü paketi bulunamadı"),
    )
    .expect("Hazır köprü bildirimi geçersiz");
    let plugin_directory = bridge.join("gameLibraryBridge.desktop");
    println!("cargo:rerun-if-changed={}", plugin_directory.display());
    let source_count = std::fs::read_dir(&plugin_directory)
        .expect("Köprü kaynak klasörü eksik")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "ts")
        })
        .count();
    assert_eq!(
        source_count,
        manifest["plugin_sources"]
            .as_object()
            .expect("Köprü kaynak listesi eksik")
            .len(),
        "Hazır köprü kaynak listesi eski; köprüyü yeniden paketle"
    );
    for (name, expected) in manifest["plugin_sources"]
        .as_object()
        .expect("Köprü kaynak listesi eksik")
    {
        let path = bridge.join("gameLibraryBridge.desktop").join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read_to_string(path).expect("Köprü kaynağı okunamadı").replace("\r\n", "\n").as_bytes())
        );
        assert_eq!(
            Some(hash.as_str()),
            expected.as_str(),
            "Hazır köprü eski: Vencord'u derleyip scripts/package-vencord-bridge.mjs çalıştır"
        );
    }
    for file in manifest["files"].as_array().expect("Köprü dosyaları eksik") {
        let path = bridge
            .join("prebuilt")
            .join(format!("{}.gz", file["name"].as_str().unwrap()));
        println!("cargo:rerun-if-changed={}", path.display());
        assert!(path.is_file(), "Hazır köprü dosyası eksik");
    }
    slint_build::compile("ui/library.slint").expect("Arayüz derlenemedi");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        println!("cargo:rerun-if-changed=ui/logos/game-library.ico");
        tauri_winres::WindowsResource::new()
            .set_icon("ui/logos/game-library.ico")
            .set("ProductName", "Game Library")
            .set("FileDescription", "Game Library")
            .compile()
            .expect("Windows uygulama simgesi derlenemedi");
    }
}
