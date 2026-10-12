#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod measurement;
mod worker;

use library_core::{steam, storage::Store};
use platform_windows::shell::InstanceGuard;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Timer, TimerMode};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use worker::{Command, Query};

slint::include_modules!();

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
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
    let instance = InstanceGuard::acquire(&data_dir)?;
    if !instance.primary && !args.iter().any(|arg| arg == "--scan-json") {
        return Ok(());
    }
    let mut store = Store::open(&data_dir.join("library.db"))?;
    if let Some(root) = argument_value(&args, "--steam-root") {
        let root = steam::normalize_root(Path::new(&root))?;
        store.add_root(&root.to_string_lossy())?;
    }
    if args.iter().any(|arg| arg == "--scan-json") {
        let result = steam::scan(&store.roots()?, &AtomicBool::new(false));
        store.apply_scan(&result)?;
        let value = serde_json::json!({"scan": result.report, "status": store.status()?, "page": store.list("", "all", 0, 100)?});
        if let Some(path) = argument_value(&args, "--output") {
            std::fs::write(path, serde_json::to_vec_pretty(&value)?)?;
        } else {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        return Ok(());
    }
    let first_scan = store.status()?.last_scan_unix_ms.is_none();
    let ui = LibraryWindow::new()?;
    let tray = LibraryTray::new()?;
    let about = AboutWindow::new()?;
    let state = Arc::new(worker::State {
        scanning: AtomicBool::new(false),
        cancel_scan: AtomicBool::new(false),
        revision: AtomicU64::new(0),
    });
    let sender = worker::start(store, data_dir.clone(), state.clone(), ui.as_weak());
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
    ui.on_refresh(move |text, favorites, installed, offset| {
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
    for pick_folder in [false, true] {
        let sender = sender.clone();
        let state = state.clone();
        let weak = ui.as_weak();
        let callback = move || {
            if state.scanning.swap(true, Ordering::SeqCst) {
                return;
            }
            state.cancel_scan.store(false, Ordering::Relaxed);
            if let Some(ui) = weak.upgrade() {
                ui.set_scanning(true);
            }
            if let Err(error) = sender.try_send(Command::Scan { pick_folder }) {
                state.scanning.store(false, Ordering::SeqCst);
                if let Some(ui) = weak.upgrade() {
                    ui.set_scanning(false);
                    ui.set_notice(error.to_string().into());
                }
            }
        };
        if pick_folder {
            ui.on_add_folder(callback);
        } else {
            ui.on_scan(callback);
        }
    }
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
    let weak = ui.as_weak();
    ui.on_open_reports(move || {
        if let Err(error) = sender.try_send(Command::Reports) {
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
    ui.invoke_refresh("".into(), false, false, 0);
    if first_scan && !args.iter().any(|arg| arg == "--benchmark") {
        ui.invoke_scan();
    }
    if !args.iter().any(|arg| arg == "--tray") {
        ui.show()?;
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
            argument == "--benchmark" || argument == "--scan-json" || argument == "--lifecycle-test"
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
