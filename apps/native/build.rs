fn main() {
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
