#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
use platform_windows::resources;

use library_core::storage::Store;
use resources::{MeasurementReport, Sampler};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    store: Mutex<Store>,
    data_dir: PathBuf,
    scanning: AtomicBool,
    cancel_scan: AtomicBool,
    sampler: Mutex<Sampler>,
    measuring: AtomicBool,
    cancel_measurement: AtomicBool,
    last_report: Mutex<Option<MeasurementReport>>,
    ui_ready: AtomicBool,
}

fn show_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    app.state::<Arc<AppState>>()
        .ui_ready
        .store(false, Ordering::SeqCst);
    if let Ok(mut sampler) = app.state::<Arc<AppState>>().sampler.lock() {
        *sampler = Sampler::default();
    }
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Game Library")
        .inner_size(1280.0, 820.0)
        .min_inner_size(940.0, 620.0)
        .center()
        .build()?;
    Ok(())
}

pub fn open_target(target: &str) -> Result<(), String> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let target = HSTRING::from(target);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize <= 32 {
        Err(format!("Windows başlatma isteğini kabul etmedi ({}). Steam'in kurulu ve steam:// protokolünün kayıtlı olduğunu kontrol et.", result.0 as usize))
    } else {
        Ok(())
    }
}

pub fn start_measurement(
    app: AppHandle,
    state: Arc<AppState>,
    seconds: u32,
    output: Option<PathBuf>,
    exit_after: bool,
) -> Result<(), String> {
    if !(10..=120).contains(&seconds) {
        return Err("Ölçüm 10–120 saniye arasında olmalı".into());
    }
    if state.measuring.swap(true, Ordering::SeqCst) {
        return Err("Ölçüm zaten devam ediyor".into());
    }
    state.cancel_measurement.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        let mut sampler = Sampler::default();
        let mut samples = Vec::with_capacity(seconds as usize + 1);
        let mut failure = None;
        for index in 0..=seconds {
            if state.cancel_measurement.load(Ordering::Relaxed) {
                break;
            }
            match sampler.sample() {
                Ok(sample) => {
                    if !exit_after {
                        let _ = app.emit("measurement-sample", &sample);
                    }
                    samples.push(sample);
                }
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
            if index < seconds {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        if !state.cancel_measurement.load(Ordering::Relaxed) && failure.is_none() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let path = output.unwrap_or_else(|| {
                state
                    .data_dir
                    .join("measurements")
                    .join(format!("measurement-{timestamp}.json"))
            });
            let report = resources::report(samples, seconds, path.to_string_lossy().into_owned());
            let saved = (|| -> Result<(), String> {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                let bytes =
                    serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
                std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
                Ok(())
            })();
            if let Err(error) = saved {
                failure = Some(error);
            } else {
                if let Ok(mut last) = state.last_report.lock() {
                    *last = Some(report.clone());
                }
                let _ = app.emit("measurement-finished", &report);
            }
        }
        state.measuring.store(false, Ordering::SeqCst);
        if let Some(error) = failure {
            let _ = app.emit("measurement-error", error);
            if exit_after {
                app.exit(1);
            }
        } else if exit_after {
            app.exit(0);
        } else {
            let _ = app.emit("measurement-stopped", ());
        }
    });
    Ok(())
}

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let data_dir = argument_value(&args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                .join("GameLibrary")
        });
    let mut store = Store::open(&data_dir.join("library.db"))?;
    if let Some(root) = argument_value(&args, "--steam-root") {
        let root = library_core::steam::normalize_root(std::path::Path::new(&root))?;
        store.add_root(&root.to_string_lossy())?;
    }
    if args.iter().any(|arg| arg == "--scan-json") {
        let result = library_core::steam::scan(&store.roots()?, &AtomicBool::new(false));
        store.apply_scan(&result)?;
        let value = serde_json::json!({ "scan":result.report, "status":store.status()?, "page":store.list("", "all", 0, 200)? });
        if let Some(path) = argument_value(&args, "--output") {
            std::fs::write(path, serde_json::to_vec_pretty(&value)?)?;
        } else {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        return Ok(());
    }
    let state = Arc::new(AppState {
        store: Mutex::new(store),
        data_dir,
        scanning: AtomicBool::new(false),
        cancel_scan: AtomicBool::new(false),
        sampler: Mutex::new(commands::new_sampler()),
        measuring: AtomicBool::new(false),
        cancel_measurement: AtomicBool::new(false),
        last_report: Mutex::new(None),
        ui_ready: AtomicBool::new(false),
    });
    let setup_state = state.clone();
    let tray_only = args.iter().any(|arg| arg == "--tray");
    let benchmark_path = argument_value(&args, "--benchmark").map(PathBuf::from);
    let seconds = argument_value(&args, "--seconds")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(60);
    let lifecycle_path = argument_value(&args, "--lifecycle-test").map(PathBuf::from);
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = show_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::library_status,
            commands::scan_steam,
            commands::cancel_scan,
            commands::add_steam_folder,
            commands::set_favorite,
            commands::launch_game,
            commands::sample_resources,
            commands::start_measurement,
            commands::measurement_status,
            commands::cancel_measurement,
            commands::open_measurements_folder,
            commands::minimize_to_tray,
            commands::ui_ready
        ])
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Kütüphaneyi aç", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Game Library")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        let _ = show_window(app);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        let _ = show_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if !tray_only {
                show_window(app.handle())?;
            }
            let cached = setup_state
                .store
                .lock()
                .map_err(|_| "Kütüphane kilidi alınamadı")?
                .list("", "all", 0, 200)?;
            for game in cached.games {
                if let Some(path) = game.artwork_path {
                    let _ = app.asset_protocol_scope().allow_file(path);
                }
            }
            if let Some(output) = benchmark_path {
                let handle = app.handle().clone();
                let state = setup_state.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(8));
                    if let Err(error) =
                        start_measurement(handle.clone(), state, seconds, Some(output), true)
                    {
                        eprintln!("{error}");
                        handle.exit(1);
                    }
                });
            }
            if let Some(output) = lifecycle_path {
                run_lifecycle_test(app.handle().clone(), setup_state.clone(), output);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.destroy();
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|_, event| {
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
    Ok(())
}

fn run_lifecycle_test(app: AppHandle, state: Arc<AppState>, output: PathBuf) {
    std::thread::spawn(move || {
        let mut cycles = Vec::new();
        let mut error = None;
        for cycle in 0..5 {
            let started = std::time::Instant::now();
            while !state.ui_ready.load(Ordering::SeqCst)
                && started.elapsed() < Duration::from_secs(20)
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            if !state.ui_ready.load(Ordering::SeqCst) {
                error = Some("Arayüz hazır sinyali alınamadı");
                break;
            }
            let handshake_ms = started.elapsed().as_secs_f64() * 1000.0;
            std::thread::sleep(Duration::from_secs(2));
            let before = Sampler::default().sample().ok();
            let destroy_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = destroy_app.get_webview_window("main") {
                    let _ = window.destroy();
                }
            });
            std::thread::sleep(Duration::from_secs(3));
            let after = Sampler::default().sample().ok();
            cycles.push(serde_json::json!({"cycle":cycle,"uiHandshakeMs":handshake_ms,"open":before,"tray":after,"windowGone":app.get_webview_window("main").is_none()}));
            if cycle < 4 {
                state.ui_ready.store(false, Ordering::SeqCst);
                let show_app = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = show_window(&show_app);
                });
            }
        }
        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let value = serde_json::json!({"cycles":cycles,"error":error});
        let _ = std::fs::write(
            output,
            serde_json::to_vec_pretty(&value).unwrap_or_default(),
        );
        app.exit(if error.is_some() { 1 } else { 0 });
    });
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn measures_current_process_and_reuses_membership() {
        let mut sampler = Sampler::default();
        let first = sampler.sample().unwrap();
        assert!(first
            .processes
            .iter()
            .any(|process| process.pid == std::process::id()));
        assert!(first.working_set_mib > 0.0);
        assert!(first.private_bytes_mib > 0.0);
        assert!(first.cpu_percent.is_none());
        let second = sampler.sample().unwrap();
        assert!(second.cpu_percent.unwrap().is_finite());
        assert_eq!(first.process_count, second.process_count);
    }

    #[test]
    fn export_types() {
        let declarations = [
            library_core::models::Source::decl(),
            library_core::models::LaunchTarget::decl(),
            library_core::models::SourceStatus::decl(),
            library_core::models::Game::decl(),
            library_core::models::GamePage::decl(),
            library_core::models::LibraryStatus::decl(),
            library_core::models::ScanReport::decl(),
            resources::ProcessSample::decl(),
            resources::ResourceSample::decl(),
            resources::MeasurementReport::decl(),
        ]
        .map(|declaration| format!("export {declaration}"));
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/src/bindings.ts");
        std::fs::write(path, declarations.join("\n\n") + "\n").unwrap();
    }
}
