use library_core::models::{RootScan, ScanReport, ScanResult};
use library_core::{steam, storage::Store};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const DOTA: &str = include_str!("../../../fixtures/steam/appmanifest_570.acf");
const CS: &str = include_str!("../../../fixtures/steam/appmanifest_730.acf");

#[test]
fn active_account_skips_game_libraries_without_client_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("SteamLibrary");
    let client = temp.path().join("Steam");
    std::fs::create_dir_all(client.join("config")).unwrap();
    std::fs::write(client.join("config/loginusers.vdf"),r#""users" { "76561198000000001" { "MostRecent" "0" } "76561198000000002" { "MostRecent" "1" } }"#).unwrap();
    assert_eq!(
        steam::active_user_id_from(&[missing, client]),
        Some("76561198000000002".into())
    );
}

#[test]
fn current_steam_cover_folders_are_found_and_redistributables_are_not_games() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    std::fs::write(root.join("steamapps/appmanifest_570.acf"), DOTA).unwrap();
    std::fs::write(
        root.join("steamapps/appmanifest_228980.acf"),
        DOTA.replace("570", "228980"),
    )
    .unwrap();
    let cover = root.join("appcache/librarycache/570/library_600x900.jpg");
    std::fs::create_dir_all(cover.parent().unwrap()).unwrap();
    std::fs::write(&cover, b"fixture cover path").unwrap();
    let scan = steam::scan_configured(&[root.to_string_lossy().into()], &AtomicBool::new(false));
    assert_eq!(scan.report.found, 1);
    assert!(scan.report.warnings.is_empty());
    assert_eq!(
        scan.libraries[0].games[0].artwork_path.as_deref(),
        Some(
            std::fs::canonicalize(cover)
                .unwrap()
                .to_string_lossy()
                .as_ref()
        )
    );
    let hashed = root.join(
        "appcache/librarycache/730/0123456789abcdef0123456789abcdef01234567/library_capsule.jpg",
    );
    std::fs::create_dir_all(hashed.parent().unwrap()).unwrap();
    std::fs::write(&hashed, b"new Steam capsule path").unwrap();
    assert_eq!(
        std::fs::canonicalize(steam::local_artwork(&[root], 730).unwrap()).unwrap(),
        std::fs::canonicalize(hashed).unwrap()
    );
}

fn library(base: &Path, name: &str) -> PathBuf {
    let root = base.join(name);
    std::fs::create_dir_all(root.join("steamapps/common/dota 2 beta")).unwrap();
    std::fs::create_dir_all(root.join("steamapps/common/Counter-Strike Global Offensive")).unwrap();
    root
}

#[test]
fn reads_modern_and_legacy_library_formats() {
    let modern = steam::parse_library_paths(include_str!(
        "../../../fixtures/steam/libraryfolders-modern.vdf"
    ))
    .unwrap();
    let legacy = steam::parse_library_paths(include_str!(
        "../../../fixtures/steam/libraryfolders-legacy.vdf"
    ))
    .unwrap();
    assert_eq!(
        modern,
        vec![
            PathBuf::from("C:\\Program Files (x86)\\Steam"),
            PathBuf::from("D:\\SteamLibrary")
        ]
    );
    assert_eq!(legacy.len(), 2);
    assert_eq!(legacy[1], PathBuf::from("D:\\Games\\SteamLibrary"));
}

#[test]
fn verifies_installation_and_uses_typed_launch_uri() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    let game = steam::parse_manifest(DOTA, &root, 570).unwrap();
    assert_eq!(game.name, "Dota 2");
    assert!(game.installed);
    assert_eq!(
        steam::launch_uri(game.app_id).unwrap(),
        "steam://rungameid/570"
    );
    assert!(steam::launch_uri(0).is_err());
    std::fs::remove_dir(root.join("steamapps/common/dota 2 beta")).unwrap();
    assert!(!steam::parse_manifest(DOTA, &root, 570).unwrap().installed);
}

#[test]
fn rejects_mismatched_ids_and_unsafe_install_paths() {
    assert!(steam::parse_manifest(DOTA, Path::new("Steam"), 730).is_err());
    for folder in ["..", "../outside", "..\\outside", "C:\\other", "/other"] {
        assert!(steam::parse_manifest(
            &DOTA.replace("dota 2 beta", folder),
            Path::new("Steam"),
            570
        )
        .is_err());
    }
    assert!(steam::parse_manifest("\"AppState\" { broken", Path::new("Steam"), 570).is_err());
}

#[test]
fn partial_download_is_not_launchable() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    assert!(
        !steam::parse_manifest(&DOTA.replace("\"4\"", "\"2\""), &root, 570)
            .unwrap()
            .installed
    );
}

#[test]
fn scans_multiple_libraries_and_deduplicates_roots() {
    let temp = tempfile::tempdir().unwrap();
    let first = library(temp.path(), "Steam");
    let second = library(temp.path(), "More games");
    let escaped = second.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        first.join("steamapps/libraryfolders.vdf"),
        format!("\"libraryfolders\" {{ \"0\" {{ \"path\" \"{escaped}\" }} }}"),
    )
    .unwrap();
    std::fs::write(first.join("steamapps/appmanifest_570.acf"), DOTA).unwrap();
    std::fs::write(second.join("steamapps/appmanifest_730.acf"), CS).unwrap();
    let result = steam::scan_configured(
        &[
            first.to_string_lossy().into_owned(),
            first.join("steamapps").to_string_lossy().into_owned(),
        ],
        &AtomicBool::new(false),
    );
    assert_eq!(result.report.found, 2);
    assert_eq!(result.report.roots.len(), 2);
    assert!(result.report.warnings.is_empty());
}

#[test]
fn bad_manifest_does_not_prevent_valid_results() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    std::fs::write(root.join("steamapps/appmanifest_570.acf"), DOTA).unwrap();
    std::fs::write(root.join("steamapps/appmanifest_730.acf"), "broken").unwrap();
    let result = steam::scan_configured(
        &[root.to_string_lossy().into_owned()],
        &AtomicBool::new(false),
    );
    assert_eq!(result.report.found, 1);
    assert_eq!(result.report.warnings.len(), 1);
    assert!(!result.libraries[0].complete);
}

#[test]
fn rescan_preserves_favorites_and_missing_game_records() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    let manifest = root.join("steamapps/appmanifest_570.acf");
    std::fs::write(&manifest, DOTA).unwrap();
    let mut store = Store::open(&temp.path().join("data/library.db")).unwrap();
    let roots = [root.to_string_lossy().into_owned()];
    store
        .apply_scan(&steam::scan_configured(&roots, &AtomicBool::new(false)))
        .unwrap();
    let game = store.list("", "all", 0, 20).unwrap().games.remove(0);
    store.set_favorite(&game.id, true).unwrap();
    store
        .apply_scan(&steam::scan_configured(&roots, &AtomicBool::new(false)))
        .unwrap();
    assert_eq!(store.status().unwrap().total, 1);
    assert!(store.game(&game.id).unwrap().favorite);
    std::fs::remove_file(manifest).unwrap();
    store
        .apply_scan(&steam::scan_configured(&roots, &AtomicBool::new(false)))
        .unwrap();
    assert!(!store.game(&game.id).unwrap().installed);
    assert_eq!(
        store.list("", "favorites-installed", 0, 20).unwrap().total,
        0
    );
    assert_eq!(store.status().unwrap().favorites, 1);
}

#[test]
fn incomplete_and_cancelled_scans_keep_previous_installation_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    let manifest = root.join("steamapps/appmanifest_570.acf");
    std::fs::write(&manifest, DOTA).unwrap();
    let mut store = Store::open(&temp.path().join("library.db")).unwrap();
    let roots = [root.to_string_lossy().into_owned()];
    store
        .apply_scan(&steam::scan_configured(&roots, &AtomicBool::new(false)))
        .unwrap();
    std::fs::write(&manifest, "broken").unwrap();
    store
        .apply_scan(&steam::scan_configured(&roots, &AtomicBool::new(false)))
        .unwrap();
    assert_eq!(store.status().unwrap().installed, 1);
    store
        .apply_scan(&steam::scan_configured(&roots, &AtomicBool::new(true)))
        .unwrap();
    assert_eq!(store.status().unwrap().installed, 1);
}

#[test]
fn search_escapes_sql_wildcards_and_limits_pages() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    let mut game = steam::parse_manifest(DOTA, &root, 570).unwrap();
    let mut games = Vec::new();
    for index in 0..250 {
        game.id = format!("steam:{index}");
        game.name = format!("Game {index:03}");
        games.push(game.clone());
    }
    games[0].name = "100%_Fun".into();
    let result = ScanResult {
        report: ScanReport {
            roots: vec![],
            found: 250,
            warnings: vec![],
            duration_ms: 0.0,
            cancelled: false,
        },
        libraries: vec![RootScan {
            source: library_core::models::Source::Steam,
            path: root.to_string_lossy().into_owned(),
            games,
            complete: true,
        }],
    };
    let mut store = Store::open(&temp.path().join("library.db")).unwrap();
    store.apply_scan(&result).unwrap();
    assert_eq!(store.list("", "all", 0, 10_000).unwrap().games.len(), 200);
    assert_eq!(store.list("%_", "all", 0, 20).unwrap().total, 1);
    assert_eq!(store.list("", "all", 200, 100).unwrap().games.len(), 50);
    assert!(store.list("", "invalid", 0, 10).is_err());
}

#[test]
fn inaccessible_library_keeps_saved_games() {
    let temp = tempfile::tempdir().unwrap();
    let root = library(temp.path(), "Steam");
    std::fs::write(root.join("steamapps/appmanifest_570.acf"), DOTA).unwrap();
    let mut store = Store::open(&temp.path().join("library.db")).unwrap();
    let normalized = steam::normalize_root(&root).unwrap();
    store.add_root(&normalized.to_string_lossy()).unwrap();
    store
        .apply_scan(&steam::scan_configured(
            &store.roots().unwrap(),
            &AtomicBool::new(false),
        ))
        .unwrap();
    std::fs::rename(&root, temp.path().join("Disconnected")).unwrap();
    let result = steam::scan_configured(&store.roots().unwrap(), &AtomicBool::new(false));
    assert!(!result.report.warnings.is_empty());
    store.apply_scan(&result).unwrap();
    assert_eq!(store.status().unwrap().total, 1);
}
