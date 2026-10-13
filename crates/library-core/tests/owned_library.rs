use library_core::{
    models::{CompletionTimes, Source},
    owned, steam,
    storage::Store,
};
use std::sync::atomic::AtomicBool;

#[test]
fn account_games_merge_with_later_installs_without_losing_user_data() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("library.db");
    let mut store = Store::open(&database).unwrap();
    let games=owned::steam(br#"{"response":{"game_count":2,"games":[{"appid":570,"name":"Dota 2"},{"appid":730,"name":"Counter-Strike 2"}]}}"#).unwrap();
    store.apply_owned_games(Source::Steam, &games).unwrap();
    let id = games[0].id.clone();
    store.set_favorite(&id, true).unwrap();
    store
        .set_override(&id, Some("User title"), "My notes", None)
        .unwrap();
    let completion = CompletionTimes {
        hltb_id: 42,
        name: "Fixture game".into(),
        main_seconds: Some(3600),
        extra_seconds: None,
        completionist_seconds: None,
        fetched_ms: 1,
    };
    store.save_completion(&id, &completion).unwrap();
    let root = dir.path().join("Steam");
    std::fs::create_dir_all(root.join("steamapps/common/dota 2 beta")).unwrap();
    std::fs::write(
        root.join("steamapps/appmanifest_570.acf"),
        include_str!("../../../fixtures/steam/appmanifest_570.acf"),
    )
    .unwrap();
    for _ in 0..2 {
        store
            .apply_scan(&steam::scan_configured(
                &[root.to_string_lossy().into()],
                &AtomicBool::new(false),
            ))
            .unwrap();
    }
    store.apply_owned_games(Source::Steam, &games).unwrap();
    drop(store);
    let store = Store::open(&database).unwrap();
    assert_eq!(store.status().unwrap().total, 2);
    let detail = store.detail(&id).unwrap();
    assert!(detail.game.installed);
    assert!(detail.game.favorite);
    assert_eq!(detail.game.name, "User title");
    assert_eq!(detail.notes, "My notes");
    assert_eq!(detail.completion.unwrap().hltb_id, 42);
}

#[test]
fn private_or_incomplete_account_responses_are_not_accepted_as_empty_libraries() {
    assert!(owned::steam(br#"{"response":{}}"#).is_err());
    assert!(owned::steam(br#"{"response":{"game_count":2,"games":[]}}"#).is_err());
    assert!(owned::steam(
        br#"{"response":{"game_count":1,"games":[{"appid":0,"name":"Invalid"}]}}"#
    )
    .is_err());
    assert!(owned::steam(br#"{"response":{"game_count":0}}"#)
        .unwrap()
        .is_empty());
    let bytes=br#"[{"app_name":"game-app","app_title":"Owned game","metadata":{"namespace":"namespace","id":"catalog","categories":[{"path":"games"}],"keyImages":[{"type":"OfferImageTall","url":"https://cdn1.epicgames.com/cover.jpg"}]},"dlcs":[{"app_title":"DLC"}]},{"app_name":"software","app_title":"Software","metadata":{"namespace":"ns","id":"cat","categories":[{"path":"software"}]}}]"#;
    let (games, covers) = owned::epic(bytes).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(covers.len(), 1);
    assert!(!games[0].installed);
    assert_eq!(games[0].source_id, "namespace:catalog:game-app");
}

#[test]
fn long_game_identifiers_can_persist_completion_and_cover_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("library.db");
    let mut store = Store::open(&database).unwrap();
    let bytes = serde_json::to_vec(&serde_json::json!([{
        "app_name":"fixture-app", "app_title":"Fixture game",
        "metadata":{"namespace":"n".repeat(200),"id":"c".repeat(200)}
    }]))
    .unwrap();
    let (games, _) = owned::epic(&bytes).unwrap();
    store.apply_owned_games(Source::Epic, &games).unwrap();
    let executable = dir
        .path()
        .join("long-install-directory-".repeat(5))
        .join("Game.exe");
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(&executable, b"fixture; never run").unwrap();
    let manual = store
        .add_manual(&executable, "Manual game", vec![])
        .unwrap();
    let ids = [games[0].id.clone(), manual];
    let completion = CompletionTimes {
        hltb_id: 42,
        name: "Fixture game".into(),
        main_seconds: Some(3600),
        extra_seconds: None,
        completionist_seconds: None,
        fetched_ms: 1,
    };
    for id in &ids {
        let game = store.game(id).unwrap();
        assert!(game.source_id.len() > 128);
        store.save_completion(id, &completion).unwrap();
        store
            .set_artwork_url(
                game.source,
                &game.source_id,
                "https://howlongtobeat.com/games/fixture.jpg",
            )
            .unwrap();
    }
    assert!(store
        .set_artwork_url(Source::Epic, &"x".repeat(4097), "https://example.test")
        .is_err());
    assert!(store.set_setting(&"x".repeat(129), "value").is_err());
    drop(store);
    let store = Store::open(&database).unwrap();
    for id in &ids {
        let game = store.game(id).unwrap();
        assert_eq!(store.completion(&game).unwrap().unwrap().hltb_id, 42);
        assert_eq!(
            store.artwork_url(&game).unwrap().as_deref(),
            Some("https://howlongtobeat.com/games/fixture.jpg")
        );
    }
}
