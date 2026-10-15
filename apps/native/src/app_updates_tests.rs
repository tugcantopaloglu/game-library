use super::*;
use ed25519_dalek::{Signer, SigningKey};
use library_core::storage::Store;

fn signed(bytes: &[u8], version: &str) -> (UpdateManifest, String) {
    let signer = SigningKey::from_bytes(&[42; 32]);
    let key = base64::engine::general_purpose::STANDARD.encode(signer.verifying_key().as_bytes());
    let mut manifest = UpdateManifest {
        version: version.into(),
        url: format!("{REPOSITORY}/releases/download/v{version}/game-library-native.exe"),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        size: bytes.len() as u64,
        minimum_bridge_protocol: 1,
        signature: String::new(),
    };
    manifest.signature = base64::engine::general_purpose::STANDARD
        .encode(signer.sign(manifest.signed_payload().as_bytes()).to_bytes());
    (manifest, key)
}

fn prepare(data: &Path, bytes: &[u8]) -> (UpdateManifest, String, PathBuf) {
    let (manifest, key) = signed(bytes, "9.8.7");
    let directory = data.join("updates/9.8.7");
    std::fs::create_dir_all(&directory).unwrap();
    atomic_write(&directory.join("game-library-native.exe"), bytes).unwrap();
    atomic_write(
        &directory.join("update.json"),
        &serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    atomic_write(
        &data.join("updates/pending.json"),
        br#"{"version":"9.8.7"}"#,
    )
    .unwrap();
    (manifest, key, directory)
}

#[test]
fn pending_update_survives_restart_but_rejects_corruption_and_failed_versions() {
    let data = tempfile::tempdir().unwrap();
    let (manifest, key, directory) = prepare(data.path(), b"MZ correct signed package");
    assert!(pending_with_key(data.path(), &key, "0.1.0")
        .unwrap()
        .is_some());
    assert!(pending_with_key(data.path(), &key, "9.8.7")
        .unwrap()
        .is_none());
    assert!(pending_with_key(data.path(), &key, "10.0.0")
        .unwrap()
        .is_none());
    std::fs::write(
        directory.join("game-library-native.exe"),
        b"MZ corrupt signed package",
    )
    .unwrap();
    assert!(pending_with_key(data.path(), &key, "0.1.0").is_err());
    std::fs::write(
        directory.join("game-library-native.exe"),
        b"MZ correct signed package",
    )
    .unwrap();
    std::fs::write(data.path().join("updates/result.json"),serde_json::to_vec(&serde_json::json!({"success":false,"version":manifest.version,"sha256":manifest.sha256})).unwrap()).unwrap();
    assert!(pending_with_key(data.path(), &key, "0.1.0")
        .unwrap()
        .is_none());
    std::fs::write(
        data.path().join("updates/pending.json"),
        br#"{"version":"../../outside"}"#,
    )
    .unwrap();
    assert!(pending_with_key(data.path(), &key, "0.1.0").is_err());
}

#[test]
fn signed_manifest_binds_exact_release_tag_protocol_and_binary() {
    let (manifest, key) = signed(b"MZ valid", "0.2.0");
    assert!(manifest.verify(&key, "0.1.0").is_ok());
    for bad_url in [
        manifest.url.replace("/v0.2.0/", "/v99.0.0/"),
        format!("{}?extra", manifest.url),
        manifest.url.replace("github.com", "evil.example"),
        manifest.url.replace("https:", "http:"),
    ] {
        let mut changed = manifest.clone();
        changed.url = bad_url;
        assert!(changed.verify(&key, "0.1.0").is_err());
    }
    for version in ["0.2.0-rc.1", "0.2.0+metadata", "00.2.0"] {
        assert!(stable_version(version).is_err());
    }
    let mut changed = manifest;
    changed.minimum_bridge_protocol = 2;
    assert!(changed.verify(&key, "0.1.0").is_err());
}

#[test]
fn scheduled_checks_persist_and_network_errors_retry_earlier() {
    let data = tempfile::tempdir().unwrap();
    assert_eq!(check_delay(data.path()).as_secs(), 0);
    schedule_check(data.path(), true).unwrap();
    assert!((86_398..=86_400).contains(&check_delay(data.path()).as_secs()));
    schedule_check(data.path(), false).unwrap();
    assert!((898..=900).contains(&check_delay(data.path()).as_secs()));
}

fn fixture() -> &'static Path {
    static FIXTURE: std::sync::OnceLock<(tempfile::TempDir, PathBuf)> = std::sync::OnceLock::new();
    &FIXTURE
        .get_or_init(|| {
            let temp = tempfile::tempdir().unwrap();
            let binary = temp.path().join("fixture.exe");
            let result = std::process::Command::new("rustc")
                .args(["--edition=2021", "--crate-name=update_fixture"])
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update-client.rs"))
                .arg("-o")
                .arg(&binary)
                .creation_flags(0x08000000)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            (temp, binary)
        })
        .1
}

fn executable_update(mode: &str, success: bool) {
    let data = tempfile::tempdir().unwrap();
    let install = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(fixture()).unwrap();
    let (manifest, key, directory) = prepare(data.path(), &bytes);
    read_manifest(&directory, &key, "0.1.0").unwrap();
    let target = install.path().join("game-library-native.exe");
    let mut old = bytes.clone();
    old.extend_from_slice(b"previous executable trailer");
    std::fs::write(&target, &old).unwrap();
    std::fs::write(data.path().join("fixture-mode"), mode).unwrap();
    let db = data.path().join("library.db");
    let store = Store::open(&db).unwrap();
    store.set_setting("favorite-marker", "preserved").unwrap();
    drop(store);
    let mut parent = std::process::Command::new(fixture())
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    let pid = parent.id();
    parent.wait().unwrap();
    let result = apply_verified(&directory, &target, data.path(), pid, &manifest);
    assert_eq!(result.is_ok(), success, "{result:?}");
    if success {
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        assert_eq!(std::fs::read(directory.join("previous.exe")).unwrap(), old);
        // The verified fixture exits on its own; keep the temp install alive until then.
        std::thread::sleep(std::time::Duration::from_millis(2200));
    } else {
        assert_eq!(std::fs::read(&target).unwrap(), old);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !data.path().join("restarted-previous").exists()
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(data.path().join("restarted-previous").is_file());
        assert!(rejected(data.path(), &manifest));
        assert!(pending_with_key(data.path(), &key, "0.1.0")
            .unwrap()
            .is_none());
    }
    let restored = Store::open(&db).unwrap();
    assert_eq!(
        restored.setting("favorite-marker").unwrap().as_deref(),
        Some("preserved")
    );
    assert!(directory.join("library.previous.db").is_file());
}

#[test]
fn real_windows_executable_replacement_accepts_authenticated_health() {
    executable_update("healthy", true);
}
#[test]
fn failed_candidate_restores_executable_database_and_reopens_old_app() {
    executable_update("fail", false);
}
#[test]
fn candidate_reporting_wrong_version_is_rolled_back() {
    executable_update("wrong-version", false);
}
