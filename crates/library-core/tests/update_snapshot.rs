use library_core::storage::Store;
use rusqlite::Connection;

#[test]
fn updater_snapshot_includes_live_wal_without_running_migrations() {
    let data = tempfile::tempdir().unwrap();
    let database = data.path().join("library.db");
    let backup = data.path().join("backup.db");
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; PRAGMA user_version=99; CREATE TABLE future_data(value TEXT); INSERT INTO future_data VALUES('favorite and notes');").unwrap();
    Store::snapshot_file(&database, &backup).unwrap();
    let snapshot = Connection::open(&backup).unwrap();
    assert_eq!(
        snapshot
            .query_row("SELECT value FROM future_data", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "favorite and notes"
    );
    assert_eq!(
        snapshot
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        99
    );
    assert!(Store::snapshot_file(&database, &backup).is_err());
}
