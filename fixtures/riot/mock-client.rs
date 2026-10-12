fn main() {
    let request = std::env::current_exe()
        .expect("executable")
        .parent()
        .expect("directory")
        .join("request.txt");
    let mut lines = vec![std::env::current_dir()
        .expect("working directory")
        .to_string_lossy()
        .into_owned()];
    lines.extend(std::env::args().skip(1));
    std::fs::write(request, lines.join("\n")).expect("request file");
}
