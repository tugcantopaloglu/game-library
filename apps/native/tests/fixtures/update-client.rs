use std::{path::PathBuf, time::Duration};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |key: &str| args.windows(2).find(|p| p[0] == key).map(|p| p[1].clone());
    let Some(data) = value("--data-dir").map(PathBuf::from) else {
        return;
    };
    let Some(health) = value("--update-health-report") else {
        std::fs::write(data.join("restarted-previous"), b"yes").unwrap();
        return;
    };
    let mode = std::fs::read_to_string(data.join("fixture-mode")).unwrap();
    if mode == "fail" {
        std::fs::write(data.join("library.db"), b"broken future migration").unwrap();
        std::fs::write(data.join("library.db-wal"), b"future WAL").unwrap();
        return;
    }
    let token = value("--update-health-token").unwrap();
    let version = if mode == "wrong-version" {
        "9.8.6"
    } else {
        "9.8.7"
    };
    let report = format!(
        r#"{{"ready":true,"version":"{version}","pid":{},"token":"{token}"}}"#,
        std::process::id()
    );
    std::fs::write(health, report).unwrap();
    std::thread::sleep(Duration::from_millis(if mode == "wrong-version" {
        300
    } else {
        2000
    }));
}
