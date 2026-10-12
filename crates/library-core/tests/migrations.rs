use library_core::models::{LaunchTarget, Source};
use library_core::storage::Store;
use rusqlite::{params, Connection};

#[test]
fn schema_one_is_backed_up_and_preserves_steam_identity_and_favorites() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.db");
    let old = Connection::open(&path).unwrap();
    old.execute_batch("CREATE TABLE games(id TEXT PRIMARY KEY,app_id INTEGER NOT NULL,name TEXT NOT NULL,search_name TEXT NOT NULL,library_path TEXT NOT NULL,install_path TEXT NOT NULL,installed INTEGER NOT NULL,favorite INTEGER NOT NULL DEFAULT 0,size_bytes REAL NOT NULL,artwork_path TEXT);
        CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); PRAGMA user_version=1;").unwrap();
    old.execute("INSERT INTO games VALUES('steam:570:library',570,'Dota 2','dota 2','library','game',1,1,100,NULL)", []).unwrap();
    old.execute("INSERT INTO settings VALUES('roots',?1)", ["[\"library\"]"])
        .unwrap();
    drop(old);
    let store = Store::open(&path).unwrap();
    let game = store.game("steam:570:library").unwrap();
    assert!(game.favorite);
    assert!(game.installed);
    assert_eq!(game.source, Source::Steam);
    assert_eq!(game.source_id, "570");
    assert!(matches!(game.launch, LaunchTarget::Steam { app_id: 570 }));
    assert_eq!(store.roots().unwrap(), ["library"]);
    let backup = std::fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("library.schema-1-backup-")
        })
        .unwrap();
    let backup = Connection::open(backup.path()).unwrap();
    assert_eq!(
        backup
            .query_row::<u32, _, _>("PRAGMA user_version", [], |row| row.get(0))
            .unwrap(),
        1
    );
    assert!(backup
        .query_row::<bool, _, _>("SELECT favorite FROM games", [], |row| row.get(0))
        .unwrap());
    drop(store);
    assert!(
        Store::open(&path)
            .unwrap()
            .game("steam:570:library")
            .unwrap()
            .favorite
    );
}

#[test]
fn source_roots_are_isolated_and_future_schemas_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.db");
    let store = Store::open(&path).unwrap();
    store.add_root("same-path").unwrap();
    store.add_source_root(Source::Epic, "same-path").unwrap();
    store.add_source_root(Source::Epic, "SAME-PATH").unwrap();
    assert_eq!(store.source_roots().unwrap().len(), 2);
    assert_eq!(store.roots().unwrap(), ["same-path"]);
    drop(store);
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(
        connection
            .query_row::<u32, _, _>("PRAGMA user_version", params![], |row| row.get(0))
            .unwrap(),
        99
    );
}
