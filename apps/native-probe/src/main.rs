#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use library_core::storage::Store;
use platform_windows::resources::{report, Sampler};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

slint::include_modules!();

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn rows(
    store: &Store,
    query: &str,
) -> Result<ModelRc<ModelRc<SharedString>>, Box<dyn std::error::Error>> {
    let page = store.list(query, "all", 0, 200)?;
    let names: Vec<SharedString> = page
        .games
        .iter()
        .map(|game| SharedString::from(&game.name))
        .collect();
    let rows: Vec<ModelRc<SharedString>> = names
        .chunks(5)
        .map(|row| Rc::new(VecModel::from(row.to_vec())).into())
        .collect();
    Ok(Rc::new(VecModel::from(rows)).into())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|argument| argument == "--tray") {
        return Err("Native denemede tepsi senaryosu henüz yok".into());
    }
    std::env::set_var("SLINT_BACKEND", "winit-software");
    let data_dir = argument_value(&args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                .join("GameLibrary")
        });
    let store = Rc::new(Store::open(&data_dir.join("library.db"))?);
    let ui = BenchmarkWindow::new()?;
    let status = store.status()?;
    ui.set_game_count(status.total as i32);
    ui.set_game_rows(rows(&store, "")?);
    let weak = ui.as_weak();
    let search_store = store.clone();
    ui.on_search(move |query| {
        if let Some(ui) = weak.upgrade() {
            if let Ok(model) = rows(&search_store, &query) {
                ui.set_game_rows(model);
            }
        }
    });
    let failure = Arc::new(Mutex::new(None::<String>));
    if let Some(path) = argument_value(&args, "--benchmark") {
        let worker_failure = failure.clone();
        let path = PathBuf::from(path);
        let seconds = argument_value(&args, "--seconds")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(60)
            .clamp(10, 120);
        std::thread::spawn(move || {
            let measured = (|| -> Result<(), String> {
                std::thread::sleep(Duration::from_secs(8));
                let mut sampler = Sampler::default();
                let mut samples = Vec::with_capacity(seconds as usize + 1);
                for index in 0..=seconds {
                    samples.push(sampler.sample()?);
                    if index < seconds {
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
                let result = report(samples, seconds, path.to_string_lossy().into_owned());
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                let bytes =
                    serde_json::to_vec_pretty(&result).map_err(|error| error.to_string())?;
                std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
                Ok(())
            })();
            if let Err(error) = measured {
                if let Ok(mut failure) = worker_failure.lock() {
                    *failure = Some(error);
                }
            }
            let _ = slint::quit_event_loop();
        });
    }
    ui.run()?;
    if let Some(error) = failure.lock().map_err(|_| "Ölçüm kilidi alınamadı")?.take() {
        return Err(error.into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
