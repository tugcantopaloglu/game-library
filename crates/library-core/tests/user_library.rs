use library_core::{
    models::{RootScan, ScanReport, ScanResult, Source},
    storage::Store,
};

#[test]
fn manual_game_identity_collections_and_overrides_survive_rescan_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("Sample Game.exe");
    std::fs::write(&executable, b"fixture executable; never run").unwrap();
    let database = directory.path().join("library.db");
    let mut store = Store::open(&database).unwrap();
    let id = store
        .add_manual(&executable, "Original title", vec!["--example".into()])
        .unwrap();
    assert_eq!(
        id,
        store
            .add_manual(&executable, "Original title", vec![])
            .unwrap()
    );
    assert_eq!(store.status().unwrap().total, 1);
    let collection = store.create_collection("RPG").unwrap();
    store.collection_member(collection, &id, true).unwrap();
    store
        .set_override(&id, Some("My title"), "My note", Some("custom-cover.png"))
        .unwrap();
    store.set_favorite(&id, true).unwrap();
    let game = store.game(&id).unwrap();
    assert_eq!(game.name, "My title");
    store
        .apply_scan(&ScanResult {
            report: ScanReport {
                roots: vec![],
                found: 0,
                warnings: vec![],
                duration_ms: 0.0,
                cancelled: false,
            },
            libraries: vec![RootScan {
                source: Source::Steam,
                path: "different-root".into(),
                games: vec![],
                complete: true,
            }],
        })
        .unwrap();
    drop(store);
    let store = Store::open(&database).unwrap();
    let detail = store.detail(&id).unwrap();
    assert!(detail.game.favorite);
    assert_eq!(detail.notes, "My note");
    assert_eq!(
        detail.game.artwork_path.as_deref(),
        Some("custom-cover.png")
    );
    assert_eq!(detail.collections[0].id, collection);
    assert_eq!(
        store
            .list_filtered(
                "my title",
                "all",
                0,
                100,
                Some(Source::Manual),
                Some(collection)
            )
            .unwrap()
            .total,
        1
    );
    store.collection_member(collection, &id, false).unwrap();
    assert_eq!(
        store
            .list_filtered("", "all", 0, 100, None, Some(collection))
            .unwrap()
            .total,
        0
    );
}

#[test]
fn invalid_manual_targets_are_rejected_and_sessions_do_not_claim_success() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("Sample.exe");
    std::fs::write(&executable, b"fixture").unwrap();
    let mut store = Store::open(&directory.path().join("library.db")).unwrap();
    assert!(store.add_manual(&executable, "", vec![]).is_err());
    assert!(store
        .add_manual(&executable, "Game", vec!["bad\0arg".into()])
        .is_err());
    let id = store.add_manual(&executable, "Game", vec![]).unwrap();
    let session = store.session_start(&id).unwrap();
    assert_eq!(store.detail(&id).unwrap().sessions[0].status, "requested");
    store
        .session_update(session, "unconfirmed", None, true)
        .unwrap();
    let history = store.detail(&id).unwrap().sessions;
    assert!(history[0].ended_ms.is_some());
    assert_eq!(history[0].status, "unconfirmed");
    store.session_start(&id).unwrap();
    store.recover_sessions().unwrap();
    assert_eq!(store.detail(&id).unwrap().sessions[0].status, "interrupted");
}

#[test]
fn schema_two_is_backed_up_before_user_library_migration() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("library.db");
    drop(Store::open(&database).unwrap());
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection.execute_batch("DROP VIEW library_games; DROP TABLE game_overrides; DROP TABLE collection_games; DROP TABLE collections; DROP TABLE play_sessions; PRAGMA user_version=2;").unwrap();
    drop(connection);
    drop(Store::open(&database).unwrap());
    let backup = std::fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("library.schema-2-backup-")
        })
        .unwrap();
    let backup = rusqlite::Connection::open(backup.path()).unwrap();
    assert_eq!(
        backup
            .query_row::<u32, _, _>("PRAGMA user_version", [], |row| row.get(0))
            .unwrap(),
        2
    );
}
