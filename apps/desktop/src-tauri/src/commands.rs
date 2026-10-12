use crate::resources::{MeasurementReport, ResourceSample, Sampler};
use crate::{open_target, AppState};
use library_core::models::{GamePage, LibraryStatus, ScanReport};
use library_core::steam;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub async fn run_scan(app: AppHandle, state: Arc<AppState>) -> Result<ScanReport, String> {
    if state.scanning.swap(true, Ordering::SeqCst) {
        return Err("Tarama zaten devam ediyor".into());
    }
    state.cancel_scan.store(false, Ordering::Relaxed);
    let worker_state = state.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let roots = worker_state
            .store
            .lock()
            .map_err(|_| "Kütüphane kilidi alınamadı")?
            .roots()
            .map_err(|error| error.to_string())?;
        let result = steam::scan(&roots, &worker_state.cancel_scan);
        for library in &result.libraries {
            for game in &library.games {
                if let Some(path) = &game.artwork_path {
                    let _ = app.asset_protocol_scope().allow_file(path);
                }
            }
        }
        worker_state
            .store
            .lock()
            .map_err(|_| "Kütüphane kilidi alınamadı")?
            .apply_scan(&result)
            .map_err(|error| error.to_string())?;
        let _ = app.emit("library-changed", &result.report);
        Ok(result.report)
    })
    .await
    .map_err(|error| error.to_string());
    state.scanning.store(false, Ordering::SeqCst);
    result?
}

#[tauri::command]
pub async fn scan_steam(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ScanReport, String> {
    run_scan(app, state.inner().clone()).await
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, Arc<AppState>>) {
    state.cancel_scan.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub async fn list_games(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    query: String,
    filter: String,
    offset: u32,
    limit: u32,
) -> Result<GamePage, String> {
    if query.len() > 512 {
        return Err("Arama metni çok uzun".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let page = state
            .store
            .lock()
            .map_err(|_| "Kütüphane kilidi alınamadı")?
            .list(&query, &filter, offset, limit)
            .map_err(|error| error.to_string())?;
        for game in &page.games {
            if let Some(path) = &game.artwork_path {
                let _ = app.asset_protocol_scope().allow_file(path);
            }
        }
        Ok(page)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn library_status(state: State<'_, Arc<AppState>>) -> Result<LibraryStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .store
            .lock()
            .map_err(|_| "Kütüphane kilidi alınamadı")?
            .status()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn add_steam_folder(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ScanReport>, String> {
    let state = state.inner().clone();
    let picker_app = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        picker_app
            .dialog()
            .file()
            .set_title("Steam veya SteamLibrary klasörünü seç")
            .blocking_pick_folder()
    })
    .await
    .map_err(|error| error.to_string())?;
    let Some(folder) = selected else {
        return Ok(None);
    };
    let path = folder.into_path().map_err(|error| error.to_string())?;
    let root = steam::normalize_root(&path).map_err(|error| error.to_string())?;
    state
        .store
        .lock()
        .map_err(|_| "Kütüphane kilidi alınamadı")?
        .add_root(&root.to_string_lossy())
        .map_err(|error| error.to_string())?;
    run_scan(app, state).await.map(Some)
}

#[tauri::command]
pub fn set_favorite(
    state: State<'_, Arc<AppState>>,
    id: String,
    favorite: bool,
) -> Result<(), String> {
    state
        .store
        .lock()
        .map_err(|_| "Kütüphane kilidi alınamadı")?
        .set_favorite(&id, favorite)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn launch_game(state: State<'_, Arc<AppState>>, id: String) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let game = state
            .store
            .lock()
            .map_err(|_| "Kütüphane kilidi alınamadı")?
            .game(&id)
            .map_err(|error| error.to_string())?;
        if !game.installed || !std::path::Path::new(&game.install_path).is_dir() {
            return Err("Oyun kurulumu erişilebilir değil. Kütüphaneyi tekrar tara.".into());
        }
        let uri = steam::launch_uri(game.app_id).map_err(|error| error.to_string())?;
        open_target(&uri)?;
        Ok(format!(
            "{} için başlatma isteği Steam'e iletildi",
            game.name
        ))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn sample_resources(state: State<'_, Arc<AppState>>) -> Result<ResourceSample, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .sampler
            .lock()
            .map_err(|_| "Ölçüm kilidi alınamadı")?
            .sample()
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn start_measurement(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    duration_seconds: u32,
) -> Result<(), String> {
    crate::start_measurement(app, state.inner().clone(), duration_seconds, None, false)
}

#[tauri::command]
pub fn measurement_status(
    state: State<'_, Arc<AppState>>,
) -> Result<(bool, Option<MeasurementReport>), String> {
    Ok((
        state.measuring.load(Ordering::Relaxed),
        state
            .last_report
            .lock()
            .map_err(|_| "Ölçüm kilidi alınamadı")?
            .clone(),
    ))
}

#[tauri::command]
pub fn cancel_measurement(state: State<'_, Arc<AppState>>) {
    state.cancel_measurement.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn open_measurements_folder(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let folder = state.data_dir.join("measurements");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    open_target(&folder.to_string_lossy())
}

#[tauri::command]
pub fn minimize_to_tray(window: tauri::WebviewWindow) -> Result<(), String> {
    window.destroy().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn ui_ready(state: State<'_, Arc<AppState>>) {
    state.ui_ready.store(true, Ordering::SeqCst);
}

pub fn new_sampler() -> Sampler {
    Sampler::default()
}
