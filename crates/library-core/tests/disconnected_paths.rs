use library_core::discovery::absolute_path;

#[test]
fn a_missing_install_preserves_the_canonical_existing_parent() {
    let directory = tempfile::tempdir().unwrap();
    let install = directory.path().join("Riot Games/VALORANT/live");
    std::fs::create_dir_all(&install).unwrap();
    let before = absolute_path(&install.to_string_lossy()).unwrap();
    std::fs::rename(
        directory.path().join("Riot Games"),
        directory.path().join("Moved Games"),
    )
    .unwrap();
    let after = absolute_path(&install.to_string_lossy()).unwrap();
    assert_eq!(before, after);
    assert!(!after.exists());
}
