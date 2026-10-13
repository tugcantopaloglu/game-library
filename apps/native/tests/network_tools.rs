// A narrow harness: only pure validation/default-preference tests run. No DNS,
// elevation, driver, GoodbyeDPI process, or public-network calls are made.
#[allow(dead_code)]
#[path = "../src/network_tools.rs"]
mod network_tools;

#[cfg(windows)]
#[test]
fn inspect_reads_windows_network_without_mutation() {
    let data = tempfile::tempdir().unwrap();
    let report = network_tools::inspect(data.path());
    assert!(report.error.is_none(), "{:?}", report.error);
    assert!(report.runtime["interfaces"].is_array());
    assert_eq!(report.runtime["dnsEnabled"], false);
    assert_eq!(report.runtime["running"], false);
    assert!(!report.preferences.auto_update);
}

#[cfg(windows)]
#[test]
fn malformed_mutable_metadata_does_not_disable_runtime_inspection() {
    let data = tempfile::tempdir().unwrap();
    let directory = data.path().join("network-tools");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("preferences.json"), b"malformed").unwrap();
    std::fs::write(directory.join("prepared.json"), b"malformed").unwrap();
    let report = network_tools::inspect(data.path());
    assert!(report.error.is_some());
    assert!(report.runtime["interfaces"].is_array());
    assert_eq!(report.runtime["rollbackAvailable"], false);
    assert_eq!(report.runtime["running"], false);
    assert!(!report.preferences.auto_update);
    network_tools::set_auto_update(data.path(), false).unwrap();
    let report = network_tools::inspect(data.path());
    assert!(report.runtime["interfaces"].is_array());
    assert!(!report.preferences.auto_update);
}

#[cfg(windows)]
#[test]
#[ignore = "Downloads/stages the official package; never runs it or changes DNS"]
fn official_stage_probe() {
    let data = tempfile::tempdir().unwrap();
    let package = network_tools::prepare_latest(data.path()).unwrap();
    assert!(std::path::Path::new(&package.directory)
        .join(&package.executable)
        .is_file());
    assert!(package.files.len() >= 3);
    println!(
        "Official package {} staged; publisher digest: {}",
        package.release.version, package.publisher_digest_verified
    );
    let report = network_tools::inspect(data.path());
    assert!(report.error.is_none(), "{:?}", report.error);
    assert_eq!(
        report.prepared_version.as_deref(),
        Some(package.release.version.as_str())
    );
    assert_eq!(report.runtime["running"], false);
    assert_eq!(report.runtime["dnsEnabled"], false);
}
