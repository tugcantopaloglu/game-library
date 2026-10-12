use library_core::models::{LaunchTarget, RootScan, ScanReport, ScanResult, Source};
use library_core::{epic, launcher, riot, storage::Store};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const EPIC: &str = include_str!("../../../fixtures/epic/game.item");
const RIOT: &str = include_str!("../../../fixtures/riot/valorant.live.product_settings.yaml");

#[test]
fn epic_can_launch_a_manifest_without_an_executable_and_omits_software() {
    let temp = tempfile::tempdir().unwrap();
    let (root, mut value) = epic_setup(temp.path());
    value["LaunchExecutable"] = "".into();
    let game = epic::parse_manifest(&value.to_string(), &root)
        .unwrap()
        .unwrap();
    assert!(game.installed);
    assert!(launcher::prepare(&game).is_ok());
    value["LaunchExecutable"] = "../outside.exe".into();
    assert!(epic::parse_manifest(&value.to_string(), &root).is_err());
    value["LaunchExecutable"] = "".into();
    value["AppCategories"] = serde_json::json!(["software", "applications"]);
    assert!(epic::parse_manifest(&value.to_string(), &root)
        .unwrap()
        .is_none());
    value["AppCategories"] = serde_json::json!(["games"]);
    value["InstallLocation"] = temp
        .path()
        .join("removed-game")
        .to_string_lossy()
        .as_ref()
        .into();
    assert!(
        !epic::parse_manifest(&value.to_string(), &root)
            .unwrap()
            .unwrap()
            .installed
    );
}

#[test]
fn riot_empty_placeholders_and_patch_metadata_do_not_produce_game_warnings() {
    let temp = tempfile::tempdir().unwrap();
    let (root, _) = riot_setup(temp.path());
    std::fs::create_dir_all(root.join("Metadata/bacon.live")).unwrap();
    std::fs::create_dir_all(root.join("Metadata/league_of_legends.live.game_patch")).unwrap();
    let mut warnings = vec![];
    let scan = riot::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    assert!(scan.complete);
    assert_eq!(scan.games.len(), 1);
    assert!(warnings.is_empty());
}

fn epic_setup(base: &Path) -> (PathBuf, serde_json::Value) {
    let manifests = base.join("Epic/Data/Manifests");
    let install = base.join("Epic Game With Spaces");
    std::fs::create_dir_all(&manifests).unwrap();
    std::fs::create_dir_all(&install).unwrap();
    std::fs::write(install.join("Game.exe"), b"fixture, not executable").unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(EPIC).unwrap();
    manifest["InstallLocation"] = install.to_string_lossy().as_ref().into();
    (std::fs::canonicalize(manifests).unwrap(), manifest)
}

fn riot_setup(base: &Path) -> (PathBuf, PathBuf) {
    let data = base.join("Riot Data");
    let install = base.join("VALORANT/live");
    let client = base.join("Riot Client/RiotClientServices.exe");
    std::fs::create_dir_all(data.join("Metadata/valorant.live")).unwrap();
    std::fs::create_dir_all(&install).unwrap();
    std::fs::create_dir_all(client.parent().unwrap()).unwrap();
    std::fs::write(install.join("VALORANT.exe"), b"fixture").unwrap();
    std::fs::write(&client, b"fixture").unwrap();
    let settings = RIOT
        .replace("__INSTALL__", &install.to_string_lossy())
        .replace("__ROOT__", &base.to_string_lossy());
    std::fs::write(
        data.join("Metadata/valorant.live/valorant.live.product_settings.yaml"),
        settings,
    )
    .unwrap();
    std::fs::write(
        data.join("RiotClientInstalls.json"),
        serde_json::to_vec(&serde_json::json!({"rc_default":client,"associated_client":{}}))
            .unwrap(),
    )
    .unwrap();
    (
        std::fs::canonicalize(data).unwrap(),
        std::fs::canonicalize(client).unwrap(),
    )
}

fn result(libraries: Vec<RootScan>) -> ScanResult {
    ScanResult {
        report: ScanReport {
            roots: vec![],
            found: libraries.iter().map(|root| root.games.len() as u32).sum(),
            warnings: vec![],
            duration_ms: 0.0,
            cancelled: false,
        },
        libraries,
    }
}

#[test]
fn epic_manifest_produces_documented_uri_and_validates_installation() {
    let dir = tempfile::tempdir().unwrap();
    let (root, mut value) = epic_setup(dir.path());
    let game = epic::parse_manifest(&value.to_string(), &root)
        .unwrap()
        .unwrap();
    assert_eq!(game.source, Source::Epic);
    assert!(game.installed);
    assert_eq!(game.size_bytes, 1_073_741_824.0);
    let plan = launcher::prepare(&game).unwrap();
    assert!(
        matches!(plan,launcher::LaunchPlan::Uri {uri} if uri=="com.epicgames.launcher://apps/fixture-namespace%3Afixture-catalog%3AFixtureEpic?action=launch&silent=true")
    );
    value["bIsIncompleteInstall"] = true.into();
    assert!(
        !epic::parse_manifest(&value.to_string(), &root)
            .unwrap()
            .unwrap()
            .installed
    );
    value["bIsIncompleteInstall"] = false.into();
    value["LaunchExecutable"] = "../other.exe".into();
    assert!(epic::parse_manifest(&value.to_string(), &root).is_err());
    value["LaunchExecutable"] = "Game.exe".into();
    value["AppName"] = "Fixture?injected=true".into();
    assert!(epic::parse_manifest(&value.to_string(), &root).is_err());
}

#[test]
fn epic_skips_engine_and_dlc_and_keeps_healthy_records_when_one_manifest_breaks() {
    let dir = tempfile::tempdir().unwrap();
    let (root, mut value) = epic_setup(dir.path());
    std::fs::write(root.join("game.item"), value.to_string()).unwrap();
    value["AppName"] = "UE_5.6".into();
    assert!(epic::parse_manifest(&value.to_string(), &root)
        .unwrap()
        .is_none());
    value["AppName"] = "Dlc".into();
    assert!(epic::parse_manifest(&value.to_string(), &root)
        .unwrap()
        .is_none());
    std::fs::write(root.join("broken.item"), "{broken").unwrap();
    let mut warnings = Vec::new();
    let scan = epic::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    assert_eq!(scan.games.len(), 1);
    assert!(!scan.complete);
    assert_eq!(warnings.len(), 1);
    let cancelled = epic::scan_at(&root, &AtomicBool::new(true), &mut warnings);
    assert!(!cancelled.complete);
    assert!(cancelled.games.is_empty());
}

#[test]
fn riot_yaml_and_client_json_produce_fixed_argument_vector() {
    let dir = tempfile::tempdir().unwrap();
    let (root, client) = riot_setup(dir.path());
    let mut warnings = Vec::new();
    let scan = riot::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    assert!(warnings.is_empty());
    assert_eq!(scan.games.len(), 1);
    assert!(scan.complete);
    let game = &scan.games[0];
    assert!(game.installed);
    assert_eq!(game.source_id, "valorant.live");
    assert!(
        matches!(launcher::prepare(game).unwrap(),launcher::LaunchPlan::Process {executable,args,..} if executable==client && args==["--launch-product=valorant","--launch-patchline=live"])
    );
    std::fs::remove_file(client).unwrap();
    assert!(launcher::prepare(game).is_err());
}

#[test]
fn riot_aliases_and_pathless_metadata_do_not_replace_previous_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (root, _) = riot_setup(dir.path());
    let mut warnings = Vec::new();
    let file = root.join("Metadata/valorant.live/valorant.live.product_settings.yaml");
    let mut store = Store::open(&dir.path().join("library.db")).unwrap();
    let scan = riot::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    store.apply_scan(&result(vec![scan])).unwrap();
    std::fs::write(
        &file,
        "x: &alias 'value'\nproduct_install_full_path: *alias\n",
    )
    .unwrap();
    let failed = riot::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    assert!(!failed.complete);
    assert!(failed.games.is_empty());
    store.apply_scan(&result(vec![failed])).unwrap();
    assert_eq!(store.status().unwrap().installed, 1);
    std::fs::write(file, "locale: tr_TR\n").unwrap();
    assert!(!riot::scan_at(&root, &AtomicBool::new(false), &mut warnings).complete);
    let cancelled = riot::scan_at(&root, &AtomicBool::new(true), &mut warnings);
    assert!(!cancelled.complete);
}

#[test]
fn rescans_preserve_favorites_and_a_source_cannot_remove_other_source_games() {
    let dir = tempfile::tempdir().unwrap();
    let (epic_root, value) = epic_setup(dir.path());
    let (riot_root, _) = riot_setup(dir.path());
    std::fs::write(epic_root.join("game.item"), value.to_string()).unwrap();
    let mut warnings = Vec::new();
    let e = epic::scan_at(&epic_root, &AtomicBool::new(false), &mut warnings);
    let r = riot::scan_at(&riot_root, &AtomicBool::new(false), &mut warnings);
    let id = e.games[0].id.clone();
    let mut store = Store::open(&dir.path().join("library.db")).unwrap();
    store.apply_scan(&result(vec![e, r])).unwrap();
    store.set_favorite(&id, true).unwrap();
    let repeated = epic::scan_at(&epic_root, &AtomicBool::new(false), &mut warnings);
    store.apply_scan(&result(vec![repeated])).unwrap();
    assert!(store.game(&id).unwrap().favorite);
    assert_eq!(store.status().unwrap().total, 2);
    assert_eq!(
        store
            .list_for_source("", "all", 0, 100, Some(Source::Riot))
            .unwrap()
            .total,
        1
    );
    std::fs::remove_file(epic_root.join("game.item")).unwrap();
    let empty = epic::scan_at(&epic_root, &AtomicBool::new(false), &mut warnings);
    store.apply_scan(&result(vec![empty])).unwrap();
    assert!(!store.game(&id).unwrap().installed);
    assert_eq!(store.status().unwrap().installed, 1);
}

#[test]
fn launch_target_source_mismatch_and_argument_injection_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (root, value) = epic_setup(dir.path());
    let mut game = epic::parse_manifest(&value.to_string(), &root)
        .unwrap()
        .unwrap();
    game.source = Source::Steam;
    assert!(launcher::prepare(&game).is_err());
    game.source = Source::Riot;
    game.launch = LaunchTarget::Riot {
        client_path: "C:\\missing\\RiotClientServices.exe".into(),
        product: "valorant --extra".into(),
        patchline: "live".into(),
        executable: PathBuf::from(&game.install_path)
            .join("Game.exe")
            .to_string_lossy()
            .into_owned(),
    };
    assert!(launcher::prepare(&game).is_err());
}

#[test]
fn a_disconnected_riot_install_keeps_its_identity_and_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let (root, _) = riot_setup(dir.path());
    let mut warnings = Vec::new();
    let initial = riot::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    let id = initial.games[0].id.clone();
    let mut store = Store::open(&dir.path().join("library.db")).unwrap();
    store.apply_scan(&result(vec![initial])).unwrap();
    store.set_favorite(&id, true).unwrap();
    std::fs::rename(
        dir.path().join("VALORANT/live"),
        dir.path().join("Moved Install"),
    )
    .unwrap();
    let missing = riot::scan_at(&root, &AtomicBool::new(false), &mut warnings);
    assert_eq!(missing.games[0].id, id);
    assert!(!missing.games[0].installed);
    store.apply_scan(&result(vec![missing])).unwrap();
    assert_eq!(store.status().unwrap().total, 1);
    assert!(store.game(&id).unwrap().favorite);
}
