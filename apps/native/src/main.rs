#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod bridge;
mod completion;
mod measurement;
mod media;
mod updates;
mod validation;
mod worker;

use library_core::models::Source;
use library_core::{launcher, sources, storage::Store};
use platform_windows::shell::InstanceGuard;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Timer, TimerMode};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use worker::{Command, Query};

fn enqueue(
    sender: &std::sync::mpsc::SyncSender<Command>,
    weak: &slint::Weak<LibraryWindow>,
    command: Command,
) {
    if let Err(error) = sender.try_send(command) {
        if let Some(ui) = weak.upgrade() {
            ui.set_notice(error.to_string().into());
        }
    }
}

fn queue_scan(
    sender: &std::sync::mpsc::SyncSender<Command>,
    state: &Arc<worker::State>,
    weak: &slint::Weak<LibraryWindow>,
    add_source: Option<Source>,
) {
    if state.scanning.swap(true, Ordering::SeqCst) {
        return;
    }
    state.cancel_scan.store(false, Ordering::Relaxed);
    if let Some(ui) = weak.upgrade() {
        ui.set_scanning(true);
    }
    if let Err(error) = sender.try_send(Command::Scan { add_source }) {
        state.scanning.store(false, Ordering::SeqCst);
        if let Some(ui) = weak.upgrade() {
            ui.set_scanning(false);
            ui.set_notice(error.to_string().into());
        }
    }
}

slint::include_modules!();

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .filter(|value| !value.starts_with("--"))
}

fn show(ui: &LibraryWindow) {
    if let Err(error) = ui.show() {
        ui.set_notice(error.to_string().into());
    } else {
        if let Ok(handle) = ui.window().window_handle().window_handle() {
            if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                platform_windows::shell::activate_window(handle.hwnd.get());
            }
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--benchmark")
        && args.iter().any(|arg| arg == "--lifecycle-test")
    {
        return Err("Benchmark ve yaşam döngüsü denemesi ayrı çalıştırılmalı.".into());
    }
    std::env::set_var("SLINT_BACKEND", "winit-software");
    let data_dir = argument_value(&args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                .join("GameLibrary")
        });
    std::fs::create_dir_all(&data_dir)?;
    let data_dir = std::fs::canonicalize(data_dir)?;
    if let Some(output) = argument_value(&args, "--data-directory-report") {
        std::fs::write(
            output,
            serde_json::to_vec_pretty(
                &serde_json::json!({"dataDirectory":updates::external_path(&data_dir)}),
            )?,
        )?;
        return Ok(());
    }
    if args.iter().any(|argument| argument == "--prepare-vencord") {
        println!("{}", updates::prepare_bridge(&data_dir)?);
        return Ok(());
    }
    if args.iter().any(|argument| argument == "--prepare-epic") {
        accounts::prepare_epic(&data_dir)?;
        return Ok(());
    }
    if args.iter().any(|argument| argument == "--open-epic-login") {
        accounts::login_epic(&data_dir)?;
        return Ok(());
    }
    if let Some(action) = argument_value(&args, "--vencord-action") {
        let action = match action.as_str() {
            "install" => updates::InstallerAction::Install,
            "repair" => updates::InstallerAction::Repair,
            "uninstall" => updates::InstallerAction::Uninstall,
            _ => return Err("Geçersiz Vencord işlemi".into()),
        };
        println!(
            "{}",
            updates::vencord_installer(
                &data_dir,
                action,
                &argument_value(&args, "--discord-branch").unwrap_or_else(|| "stable".into())
            )?
        );
        return Ok(());
    }
    if let Some(output) = argument_value(&args, "--bridge-probe") {
        let bridge = bridge::Bridge::start(&data_dir)?;
        let start = std::time::Instant::now();
        loop {
            let snapshot = bridge.snapshot();
            if snapshot["connected"] == true || start.elapsed().as_secs() >= 90 {
                let report = serde_json::json!({"connected":snapshot["connected"],"status":snapshot["status"],"friendCount":snapshot["friends"].as_array().map(Vec::len),"roomCount":snapshot["rooms"].as_array().map(Vec::len),"capabilities":snapshot["capabilities"],"muted":snapshot["muted"],"deafened":snapshot["deafened"],"autoUpdate":snapshot["autoUpdate"]});
                std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        return Ok(());
    }
    if let Some(directory) = argument_value(&args, "--apply-update") {
        let target = argument_value(&args, "--update-target").ok_or("Güncelleme hedefi yok")?;
        let parent = argument_value(&args, "--update-parent")
            .ok_or("Güncelleme parent PID yok")?
            .parse()?;
        updates::apply(Path::new(&directory), Path::new(&target), &data_dir, parent)?;
        return Ok(());
    }
    let instance = InstanceGuard::acquire(&data_dir)?;
    if !instance.primary
        && !args
            .iter()
            .any(|arg| arg == "--scan-json" || arg == "--launch-plan" || arg == "--list-json")
    {
        return Ok(());
    }
    let mut store = Store::open(&data_dir.join("library.db"))?;
    if let Some(source) = argument_value(&args, "--sync-account") {
        let source = Source::parse(&source)?;
        let (games, covers) = match source {
            Source::Steam => {
                let id = argument_value(&args, "--steam-id")
                    .or(store.setting("steam_account_id")?)
                    .or_else(library_core::steam::active_user_id)
                    .ok_or("SteamID64 bulunamadı")?;
                (accounts::steam(&id)?, vec![])
            }
            Source::Epic => accounts::epic(&data_dir)?,
            _ => return Err("Bu kaynak hesap eşitlemesini desteklemiyor".into()),
        };
        store.apply_owned_games(source, &games)?;
        for (id, url) in covers {
            store.set_setting(&format!("cover-url:{}:{id}", source.as_str()), &url)?;
        }
        if let Some(output) = argument_value(&args, "--output") {
            std::fs::write(
                output,
                serde_json::to_vec_pretty(
                    &serde_json::json!({"source":source,"ownedGames":games.len()}),
                )?,
            )?;
        }
        return Ok(());
    }
    if let Some(id) = argument_value(&args, "--completion-for") {
        let game = store.game(&id)?;
        let completion = completion::lookup(
            &game,
            &argument_value(&args, "--hltb-url").unwrap_or_default(),
        )?;
        store.save_completion(&id, &completion.times)?;
        if let Some(url) = &completion.image_url {
            store.set_setting(
                &format!("cover-url:{}:{}", game.source.as_str(), game.source_id),
                url,
            )?;
        }
        if let Some(output) = argument_value(&args, "--output") {
            std::fs::write(output, serde_json::to_vec_pretty(&completion.times)?)?;
        }
        return Ok(());
    }
    if let Some(output) = argument_value(&args, "--search-benchmark") {
        validation::search_benchmark(&store, Path::new(&output))?;
        return Ok(());
    }
    if let Some(executable) = argument_value(&args, "--add-manual") {
        let title = argument_value(&args, "--title").unwrap_or_else(|| {
            Path::new(&executable)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let id = store.add_manual(Path::new(&executable), &title, vec![])?;
        if let Some(output) = argument_value(&args, "--output") {
            std::fs::write(output, serde_json::to_vec_pretty(&store.detail(&id)?)?)?;
        }
        return Ok(());
    }
    let source_filter = argument_value(&args, "--source")
        .map(|value| Source::parse(&value))
        .transpose()?;
    for (source, flag) in [
        (Source::Steam, "--steam-root"),
        (Source::Epic, "--epic-root"),
        (Source::Riot, "--riot-root"),
    ] {
        if let Some(root) = argument_value(&args, flag) {
            let root = sources::normalize_root(source, Path::new(&root))?;
            store.add_source_root(source, &root.to_string_lossy())?;
        }
    }
    if let Some(id) = argument_value(&args, "--launch-plan") {
        let plan = launcher::prepare(&store.game(&id)?)?;
        if let Some(path) = argument_value(&args, "--output") {
            std::fs::write(path, serde_json::to_vec_pretty(&plan)?)?;
        } else {
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        return Ok(());
    }
    if args
        .iter()
        .any(|arg| arg == "--scan-json" || arg == "--list-json")
    {
        let report = if args.iter().any(|arg| arg == "--scan-json") {
            let result = if args.iter().any(|argument| argument == "--fixture-only") {
                library_core::steam::scan_configured(&store.roots()?, &AtomicBool::new(false))
            } else {
                sources::scan(&store.source_roots()?, &AtomicBool::new(false))
            };
            store.apply_source_scan(&result)?;
            Some(result.report)
        } else {
            None
        };
        let value = serde_json::json!({"scan": report, "status": store.status()?, "page": store.list_for_source("", "all", 0, 100,source_filter)?});
        if let Some(path) = argument_value(&args, "--output") {
            std::fs::write(path, serde_json::to_vec_pretty(&value)?)?;
        } else {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        return Ok(());
    }
    let first_scan = store.needs_source_scan()?;
    if args.iter().any(|argument| argument == "--enable-bridge") {
        store.set_setting("bridge_enabled", "true")?;
    }
    if let (Some(id), Some(executable)) = (
        argument_value(&args, "--launch-test"),
        argument_value(&args, "--tracking-exe"),
    ) {
        store.set_tracking_executable(&id, Path::new(&executable))?;
    }
    let bridge_enabled = store.setting("bridge_enabled")?.as_deref() == Some("true");
    let auto_update_app = store.setting("auto_update_app")?.as_deref() == Some("true");
    let auto_update_vencord = store.setting("auto_update_vencord")?.as_deref() != Some("false");
    let ui = LibraryWindow::new()?;
    ui.window().set_size(slint::LogicalSize::new(1360.0, 860.0));
    ui.set_steam_account_id(
        store
            .setting("steam_account_id")?
            .or_else(library_core::steam::active_user_id)
            .unwrap_or_default()
            .into(),
    );
    for source in [Source::Steam, Source::Epic] {
        let status = store
            .setting(&format!("account_sync:{}", source.as_str()))?
            .map(|value| {
                format!(
                    "Son hesap eşitlemesi: {}",
                    value.split(" · ").next().unwrap_or("")
                )
            })
            .unwrap_or_else(|| "Hesap kütüphanesi henüz eşitlenmedi.".into());
        if source == Source::Steam {
            ui.set_steam_account_status(status.into());
        } else {
            ui.set_epic_account_status(status.into());
        }
    }
    ui.set_bridge_enabled(bridge_enabled);
    ui.set_auto_update_app(auto_update_app);
    ui.set_auto_update_vencord(auto_update_vencord);
    let tray = LibraryTray::new()?;
    let about = AboutWindow::new()?;
    let state = Arc::new(worker::State {
        scanning: AtomicBool::new(false),
        cancel_scan: AtomicBool::new(false),
        revision: AtomicU64::new(0),
        cover_requests: Default::default(),
        cover_attempts: Default::default(),
    });
    let sender = worker::start(store, data_dir.clone(), state.clone(), ui.as_weak());
    if let Some(output) = argument_value(&args, "--bridge-status-report") {
        let weak = ui.as_weak();
        ui.on_bridge_ready(move || {if let Some(ui) = weak.upgrade() {
            if ui.get_bridge_connected() {
                let report = serde_json::json!({"connected":true,"canMute":ui.get_can_mute(),"canDeafen":ui.get_can_deafen(),"canStopStream":ui.get_can_stop_stream()});
                let _ = std::fs::write(&output,serde_json::to_vec_pretty(&report).unwrap_or_default());
            }
        }});
    }
    let startup_timer = Rc::new(Timer::default());
    let startup_output = argument_value(&args, "--startup-report");
    let update_health = argument_value(&args, "--update-health-report")
        .zip(argument_value(&args, "--update-health-token"));
    if startup_output.is_some() || update_health.is_some() {
        let timer = startup_timer.clone();
        let weak = ui.as_weak();
        let recorded = std::cell::Cell::new(false);
        ui.on_library_ready(move || {
            if recorded.replace(true) {return;}
            let output = startup_output.clone(); let health = update_health.clone(); let weak = weak.clone();
            timer.start(TimerMode::SingleShot,Duration::from_millis(1),move || {
                if let Some(ui) = weak.upgrade() {
                    let rendered = ui.window().take_snapshot().is_ok();
                    if let Some(output) = &output {
                        let report = serde_json::json!({"startupUsableMs":started.elapsed().as_secs_f64()*1000.0,"thumbnailAndOffscreenRenderIncluded":true,"osPaintLatencyIncluded":false,"rendered":rendered});
                        let _ = std::fs::write(output,serde_json::to_vec_pretty(&report).unwrap_or_default());
                    }
                    if rendered {
                        if let Some((output,token)) = &health {
                            let report = serde_json::json!({"ready":true,"version":env!("CARGO_PKG_VERSION"),"pid":std::process::id(),"token":token});
                            let temporary = PathBuf::from(output).with_extension("part");
                            if std::fs::write(&temporary,serde_json::to_vec_pretty(&report).unwrap_or_default()).is_ok() {
                                let _ = std::fs::rename(temporary,output);
                            }
                        }
                    }
                }
                if output.is_some() {let _ = slint::quit_event_loop();}
            });
        });
    }
    let measurements = Arc::new(measurement::Measurements::default());
    let weak = ui.as_weak();
    instance.listen(move || weak.upgrade_in_event_loop(|ui| show(&ui)).is_ok());
    let weak = ui.as_weak();
    tray.on_open(move || {
        if let Some(ui) = weak.upgrade() {
            show(&ui);
        }
    });
    tray.on_exit(|| {
        let _ = slint::quit_event_loop();
    });
    ui.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    let weak_about = about.as_weak();
    ui.on_about(move || {
        if let Some(about) = weak_about.upgrade() {
            let _ = about.show();
        }
    });
    let weak = ui.as_weak();
    ui.on_hide_window(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.hide();
        }
    });
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.hide();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let debounce = Rc::new(Timer::default());
    let weak = ui.as_weak();
    let query_sender = sender.clone();
    let query_state = state.clone();
    ui.on_refresh(move |text, favorites, installed, offset, source_index| {
        let revision = query_state.revision.fetch_add(1, Ordering::Relaxed) + 1;
        let query = Query {
            text: text.chars().take(512).collect(),
            filter: match (favorites, installed) {
                (true, true) => "favorites-installed",
                (true, false) => "favorites",
                (false, true) => "installed",
                (false, false) => "all",
            }
            .into(),
            offset: offset.max(0) as u32,
            revision,
            source: match source_index {
                1 => Some(Source::Steam),
                2 => Some(Source::Epic),
                3 => Some(Source::Riot),
                4 => Some(Source::Manual),
                _ => None,
            },
            collection: weak
                .upgrade()
                .map(|ui| ui.get_collection_id() as i64)
                .filter(|id| *id > 0),
            columns: weak
                .upgrade()
                .map(|ui| ui.get_grid_columns() as usize)
                .unwrap_or(4),
        };
        let sender = query_sender.clone();
        let weak = weak.clone();
        if let Some(ui) = weak.upgrade() {
            ui.set_loading(true);
            ui.set_scroll_position(0.0);
        }
        debounce.start(
            TimerMode::SingleShot,
            Duration::from_millis(150),
            move || {
                if let Err(error) = sender.try_send(Command::Query(query.clone())) {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_loading(false);
                        ui.set_notice(format!("Kütüphane isteği gönderilemedi: {error}").into());
                    }
                }
            },
        );
    });
    let scan_sender = sender.clone();
    let scan_state = state.clone();
    let weak = ui.as_weak();
    ui.on_scan(move || queue_scan(&scan_sender, &scan_state, &weak, None));
    let folder_sender = sender.clone();
    let folder_state = state.clone();
    let weak = ui.as_weak();
    ui.on_add_folder(move |index| {
        if let Some(source) = Source::ALL.get(index as usize) {
            queue_scan(&folder_sender, &folder_state, &weak, Some(*source));
        }
    });
    let cancel_state = state.clone();
    ui.on_cancel_scan(move || cancel_state.cancel_scan.store(true, Ordering::Relaxed));
    let favorite_sender = sender.clone();
    let weak = ui.as_weak();
    ui.on_favorite(move |id, value| {
        if let Err(error) = favorite_sender.try_send(Command::Favorite(id.into(), value)) {
            if let Some(ui) = weak.upgrade() {
                ui.set_notice(error.to_string().into());
            }
        }
    });
    let launch_sender = sender.clone();
    let weak = ui.as_weak();
    ui.on_launch(move |id| {
        if let Err(error) = launch_sender.try_send(Command::Launch(id.into())) {
            if let Some(ui) = weak.upgrade() {
                ui.set_notice(error.to_string().into());
            }
        }
    });
    macro_rules! register_command {
        ($method:ident, ($($argument:ident),*) => $command:expr) => {{
            let sender = sender.clone(); let weak = ui.as_weak();
            ui.$method(move |$($argument),*| enqueue(&sender,&weak,$command));
        }};
    }
    register_command!(on_add_manual, () => Command::AddManual);
    register_command!(on_detail, (id) => Command::Detail(id.into()));
    register_command!(on_tracking, (id) => Command::Tracking(id.into()));
    register_command!(on_save_detail, (id,title,notes) => Command::SaveDetail(id.into(),title.into(),notes.into()));
    register_command!(on_cover, (id,reset) => Command::Cover(id.into(),reset));
    register_command!(on_remote_cover, (id,url) => Command::RemoteCover(id.into(),url.into()));
    register_command!(on_create_collection, (name) => Command::CollectionCreate(name.into()));
    register_command!(on_delete_collection, (id) => Command::CollectionDelete(id as i64));
    register_command!(on_collection_member, (id,collection,member) => Command::CollectionMember(id.into(),collection as i64,member));
    register_command!(on_bridge_action, (action,id) => Command::BridgeAction(action.into(),serde_json::json!({"id":id.as_str()})));
    register_command!(on_update_check, () => Command::UpdateCheck);
    register_command!(on_prepare_bridge, () => Command::PrepareBridge);
    register_command!(on_artwork_key, (key) => Command::ArtworkKey(key.into()));
    register_command!(on_search_covers, (id) => Command::SearchCovers(id.into()));
    register_command!(on_steam_credentials, (id,key) => Command::SteamCredentials(id.into(),key.into()));
    register_command!(on_steam_key_page, () => Command::SteamKeyPage);
    register_command!(on_sync_account, (source) => Command::SyncAccount(if source==0 {Source::Steam} else {Source::Epic}));
    register_command!(on_epic_login, () => Command::EpicLogin);
    register_command!(on_completion_lookup, (id,url) => Command::Completion(id.into(),url.into()));
    register_command!(on_completion_open, (id,search) => Command::CompletionOpen(id.into(),search));
    register_command!(on_update_stage, () => Command::UpdateStage);
    register_command!(on_update_apply, () => Command::UpdateApply);
    register_command!(on_update_preferences, (app,vencord) => Command::UpdatePreferences(app,vencord));
    let installer_sender = sender.clone();
    let weak = ui.as_weak();
    ui.on_installer(move |action,branch| {
        let branch = match branch { 1=>"ptb",2=>"canary",_=>"stable" };
        let message = format!("Resmi Vencord installer Discord {branch} kurulumunu değiştirecek. Discord kapanabilir ve yeniden açılması gerekebilir. Devam edilsin mi?");
        if rfd::MessageDialog::new().set_title("Vencord işlemi").set_description(message).set_buttons(rfd::MessageButtons::YesNo).show() == rfd::MessageDialogResult::Yes {
            enqueue(&installer_sender,&weak,Command::Installer(match action { 1=>updates::InstallerAction::Repair,2=>updates::InstallerAction::Uninstall,_=>updates::InstallerAction::Install },branch.into()));
        }
    });
    let bridge_timer = Rc::new(Timer::default());
    let timer = bridge_timer.clone();
    let bridge_sender = sender.clone();
    let weak = ui.as_weak();
    ui.on_bridge_toggle(move |enabled| {
        enqueue(&bridge_sender, &weak, Command::BridgeEnable(enabled));
        if enabled {
            let sender = bridge_sender.clone();
            let weak = weak.clone();
            timer.start(TimerMode::Repeated, Duration::from_secs(2), move || {
                if weak.upgrade().is_some_and(|ui| ui.window().is_visible()) {
                    enqueue(&sender, &weak, Command::BridgeRefresh);
                }
            });
        } else {
            timer.stop();
        }
    });
    if bridge_enabled {
        ui.invoke_bridge_toggle(true);
    }
    let weak = ui.as_weak();
    let reports_sender = sender.clone();
    ui.on_open_reports(move || {
        if let Err(error) = reports_sender.try_send(Command::Reports) {
            if let Some(ui) = weak.upgrade() {
                ui.set_notice(error.to_string().into());
            }
        }
    });
    let sample_state = measurements.clone();
    let weak = ui.as_weak();
    ui.on_sample(move || measurement::sample(sample_state.clone(), weak.clone()));
    let measure_state = measurements.clone();
    let measure_directory = data_dir.clone();
    let weak = ui.as_weak();
    ui.on_measure(move || {
        if let Err(error) = measurement::start(
            measure_state.clone(),
            weak.clone(),
            measure_directory.clone(),
            None,
            60,
        ) {
            if let Some(ui) = weak.upgrade() {
                ui.set_notice(error.into());
            }
        }
    });
    let cancel_state = measurements.clone();
    ui.on_cancel_measurement(move || cancel_state.cancel.store(true, Ordering::Relaxed));
    let source_index = match source_filter {
        None => 0,
        Some(Source::Steam) => 1,
        Some(Source::Epic) => 2,
        Some(Source::Riot) => 3,
        Some(Source::Manual) => 4,
    };
    ui.set_source_index(source_index);
    if let Some(section) = argument_value(&args, "--section") {
        ui.set_section(section.parse()?);
    }
    if let Some(tab) = argument_value(&args, "--detail-tab") {
        ui.set_detail_tab(tab.parse()?);
    }
    let initial_query = argument_value(&args, "--query").unwrap_or_default();
    ui.set_query(initial_query.clone().into());
    ui.invoke_refresh(
        initial_query.into(),
        ui.get_section() == 1,
        false,
        0,
        source_index,
    );
    if let Some(id) = argument_value(&args, "--detail-id") {
        enqueue(&sender, &ui.as_weak(), Command::Detail(id));
    }
    if first_scan
        && !args
            .iter()
            .any(|arg| arg == "--benchmark" || arg == "--skip-initial-scan")
    {
        ui.invoke_scan();
    }
    if !args.iter().any(|arg| arg == "--tray") {
        if let Some(width) = argument_value(&args, "--window-width") {
            let width: f32 = width.parse()?;
            let height: f32 = argument_value(&args, "--window-height")
                .unwrap_or_else(|| "860".into())
                .parse()?;
            ui.window().set_size(slint::LogicalSize::new(
                width.clamp(960.0, 4096.0),
                height.clamp(640.0, 2160.0),
            ));
        }
        ui.show()?;
    }
    let screenshot_timer = Rc::new(Timer::default());
    if let Some(output) = argument_value(&args, "--screenshot") {
        if let Some(section) = argument_value(&args, "--section") {
            ui.set_section(section.parse()?);
        }
        let weak = ui.as_weak();
        let capture_started = std::time::Instant::now();
        let detail_target = argument_value(&args, "--detail-id");
        let capture_timer = screenshot_timer.clone();
        screenshot_timer.start(TimerMode::Repeated, Duration::from_millis(500), move || {
            if let Some(ui) = weak.upgrade() {
                let ready = !ui.get_loading() && detail_target.as_ref().is_none_or(|id| ui.get_detail_id() == id);
                if capture_started.elapsed() < Duration::from_secs(3) || (!ready && capture_started.elapsed() < Duration::from_secs(30)) { return; }
                capture_timer.stop();
                if let Err(error) = validation::screenshot(&ui, Path::new(&output)) {
                    eprintln!("{error}");
                }
                let rows = ui.get_game_rows();
                let layout = serde_json::json!({"ready":ready,"physicalSize":{"width":ui.window().size().width,"height":ui.window().size().height},"scaleFactor":ui.window().scale_factor(),"columns":ui.get_grid_columns(),"section":ui.get_section(),"matched":ui.get_matched(),"offset":ui.get_offset(),"rowCount":slint::Model::row_count(&rows),"friends":slint::Model::row_count(&ui.get_friends()),"rooms":slint::Model::row_count(&ui.get_rooms())});
                let _ = std::fs::write(Path::new(&output).with_extension("layout.json"),serde_json::to_vec_pretty(&layout).unwrap_or_default());
                let _ = slint::quit_event_loop();
            }
        });
    }
    if let Some(id) = argument_value(&args, "--launch-test") {
        let report =
            argument_value(&args, "--acceptance-report").ok_or("Oyun kabul raporu yolu gerekli")?;
        enqueue(&sender.clone(), &ui.as_weak(), Command::Launch(id.clone()));
        validation::launch_acceptance(data_dir.join("library.db"), id, PathBuf::from(report));
    }
    tray.show()?;
    let initial_focus = Timer::default();
    let weak = ui.as_weak();
    initial_focus.start(
        TimerMode::SingleShot,
        Duration::from_millis(200),
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.invoke_focus_idle();
            }
        },
    );
    if let Some(path) = argument_value(&args, "--benchmark") {
        let seconds = argument_value(&args, "--seconds")
            .map(|value| value.parse())
            .transpose()?
            .unwrap_or(60);
        measurement::start(
            measurements.clone(),
            ui.as_weak(),
            data_dir,
            Some(PathBuf::from(path)),
            seconds,
        )?;
    }
    if let Some(path) = argument_value(&args, "--lifecycle-test") {
        measurement::lifecycle(measurements.clone(), ui.as_weak(), PathBuf::from(path));
    }
    if let Some(path) = argument_value(&args, "--stability-report") {
        let seconds = argument_value(&args, "--stability-seconds")
            .map(|value| value.parse())
            .transpose()?
            .unwrap_or(28_800);
        validation::stability(PathBuf::from(path), seconds)?;
    }
    slint::run_event_loop()?;
    state.cancel_scan.store(true, Ordering::Relaxed);
    measurements.cancel.store(true, Ordering::Relaxed);
    if let Some(error) = measurements
        .exit_error
        .lock()
        .map_err(|_| "Ölçüm kilidi alınamadı")?
        .take()
    {
        return Err(error.into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        if !std::env::args().any(|argument| {
            argument == "--benchmark"
                || argument == "--scan-json"
                || argument == "--lifecycle-test"
                || argument == "--launch-plan"
                || argument == "--list-json"
                || argument == "--screenshot"
                || argument == "--search-benchmark"
                || argument == "--stability-report"
                || argument == "--apply-update"
                || argument == "--add-manual"
                || argument == "--launch-test"
                || argument == "--startup-report"
                || argument == "--vencord-action"
                || argument == "--bridge-probe"
                || argument == "--prepare-vencord"
                || argument == "--data-directory-report"
                || argument == "--update-health-report"
                || argument == "--prepare-epic"
                || argument == "--open-epic-login"
                || argument == "--sync-account"
                || argument == "--completion-for"
        }) {
            rfd::MessageDialog::new()
                .set_title("Game Library")
                .set_description(error.to_string())
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
        std::process::exit(1);
    }
}
