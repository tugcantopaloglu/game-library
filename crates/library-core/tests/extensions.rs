use library_core::{
    models::{Game, LaunchTarget, Source},
    storage::Store,
};
fn game() -> Game {
    Game {
        id: "extension:fixture:game".into(),
        source: Source::Manual,
        source_id: "fixture:game".into(),
        name: "Game".into(),
        app_id: 0,
        launch: LaunchTarget::Manual {
            executable: "C:/Games/game.exe".into(),
            args: vec![],
            working_dir: "C:/Games".into(),
        },
        library_path: "extension://fixture".into(),
        install_path: "C:/Games".into(),
        installed: true,
        favorite: false,
        size_bytes: 0.0,
        artwork_path: None,
    }
}
#[test]
fn extension_refresh_preserves_user_edits_and_rolls_back_invalid_batches() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = Store::open(&directory.path().join("library.db")).unwrap();
    let game = game();
    store
        .apply_extension_games("fixture", std::slice::from_ref(&game))
        .unwrap();
    store.set_favorite(&game.id, true).unwrap();
    store
        .set_override(&game.id, Some("My game"), "My notes", None)
        .unwrap();
    store
        .apply_extension_games("fixture", std::slice::from_ref(&game))
        .unwrap();
    let detail = store.detail(&game.id).unwrap();
    assert!(detail.game.favorite);
    assert_eq!(detail.game.name, "My game");
    assert_eq!(detail.notes, "My notes");
    let mut invalid = game.clone();
    invalid.library_path = "extension://other".into();
    assert!(store.apply_extension_games("fixture", &[invalid]).is_err());
    assert!(store.game(&game.id).unwrap().installed);
    assert!(store
        .apply_extension_games("fixture", &[game.clone(), game.clone()])
        .is_err());
    assert!(store.game(&game.id).unwrap().installed);
    store.apply_extension_games("fixture", &[]).unwrap();
    assert!(!store.game(&game.id).unwrap().installed);
    assert!(store.game(&game.id).unwrap().favorite);
}
