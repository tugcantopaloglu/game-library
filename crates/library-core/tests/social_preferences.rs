use library_core::storage::Store;

#[test]
fn room_favorites_survive_restart_are_account_scoped_and_bounded() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.db");
    let account = "123456789012345678";
    let other = "223456789012345678";
    let store = Store::open(&path).unwrap();
    for index in 0..50 {
        store
            .set_room_favorite(account, &(323456789012345678u64 + index).to_string(), true)
            .unwrap();
    }
    assert!(store
        .set_room_favorite(account, "423456789012345678", true)
        .is_err());
    assert!(store.room_favorites(other).unwrap().is_empty());
    assert!(store.friend_favorites(account).unwrap().is_empty());
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.room_favorites(account).unwrap().len(), 50);
    store
        .set_room_favorite(account, "323456789012345678", false)
        .unwrap();
    store
        .set_room_favorite(account, "423456789012345678", true)
        .unwrap();
    assert_eq!(store.room_favorites(account).unwrap().len(), 50);
    assert!(store.set_room_favorite(account, "bad", true).is_err());
}

#[test]
fn friend_favorites_are_account_scoped_and_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("library.db");
    let store = Store::open(&database).unwrap();
    let first_account = "123456789012345678";
    let second_account = "223456789012345678";
    let friend = "323456789012345678";
    store
        .set_friend_favorite(first_account, friend, true)
        .unwrap();
    store
        .set_friend_favorite(first_account, friend, true)
        .unwrap();
    assert!(store.friend_favorites(second_account).unwrap().is_empty());
    drop(store);
    let store = Store::open(&database).unwrap();
    assert_eq!(store.friend_favorites(first_account).unwrap(), vec![friend]);
    store
        .set_friend_favorite(first_account, friend, false)
        .unwrap();
    assert!(store.friend_favorites(first_account).unwrap().is_empty());
    assert!(store
        .set_friend_favorite("bad account", friend, true)
        .is_err());
    assert!(store
        .set_friend_favorite(first_account, "bad friend", true)
        .is_err());
}
