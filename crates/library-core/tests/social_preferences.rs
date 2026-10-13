use library_core::storage::Store;

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
