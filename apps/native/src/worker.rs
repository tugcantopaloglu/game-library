use crate::{GameTile, LibraryWindow};
use library_core::models::{Game, LibraryStatus, Source};
use library_core::{launcher, sources, storage::Store};
use platform_windows::shell::open_target;
use slint::{ModelRc, SharedPixelBuffer, VecModel};
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

pub struct State {
    pub scanning: AtomicBool,
    pub cancel_scan: AtomicBool,
    pub revision: AtomicU64,
}

#[derive(Clone, Default)]
pub struct Query {
    pub text: String,
    pub filter: String,
    pub offset: u32,
    pub revision: u64,
    pub source: Option<Source>,
}

pub enum Command {
    Query(Query),
    Scan { add_source: Option<Source> },
    Favorite(String, bool),
    Launch(String),
    Reports,
}

struct Tile {
    game: Game,
    pixels: Option<(u32, u32, Vec<u8>)>,
}

fn thumbnail(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let resized = reader.decode().ok()?.thumbnail(180, 270).into_rgba8();
    Some((resized.width(), resized.height(), resized.into_raw()))
}

fn size_text(bytes: f64) -> String {
    if bytes <= 0.0 {
        return "Boyut bilinmiyor".into();
    }
    if bytes >= 1_073_741_824.0 {
        format!("{:.1} GB", bytes / 1_073_741_824.0)
    } else {
        format!("{:.0} MB", bytes / 1_048_576.0)
    }
}

fn set_notice(ui: &slint::Weak<LibraryWindow>, text: String) {
    let _ = ui.upgrade_in_event_loop(move |ui| ui.set_notice(text.into()));
}

fn publish(
    store: &Store,
    query: &Query,
    state: &Arc<State>,
    ui: &slint::Weak<LibraryWindow>,
) -> Result<(), String> {
    let mut page = store
        .list_for_source(&query.text, &query.filter, query.offset, 100, query.source)
        .map_err(|error| error.to_string())?;
    let offset = if page.total > 0 && query.offset >= page.total {
        ((page.total - 1) / 100) * 100
    } else {
        query.offset
    };
    if offset != query.offset {
        page = store
            .list_for_source(&query.text, &query.filter, offset, 100, query.source)
            .map_err(|error| error.to_string())?;
    }
    let status = store.status().map_err(|error| error.to_string())?;
    let mut tiles = Vec::with_capacity(page.games.len());
    let mut unavailable_artwork = 0;
    for game in page.games {
        if query.revision != state.revision.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pixels = game.artwork_path.as_deref().and_then(thumbnail);
        if game.artwork_path.is_some() && pixels.is_none() {
            unavailable_artwork += 1;
        }
        tiles.push(Tile { game, pixels });
    }
    let revision = query.revision;
    let matched = page.total;
    let state = state.clone();
    ui.upgrade_in_event_loop(move |ui| {
        if state.revision.load(Ordering::Relaxed) != revision {
            return;
        }
        let mut entries: Vec<GameTile> = tiles
            .into_iter()
            .map(|tile| {
                let has_artwork = tile.pixels.is_some();
                let artwork = tile
                    .pixels
                    .map(|(width, height, pixels)| {
                        slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
                            &pixels, width, height,
                        ))
                    })
                    .unwrap_or_default();
                GameTile {
                    id: tile.game.id.into(),
                    name: tile.game.name.into(),
                    size: size_text(tile.game.size_bytes).into(),
                    installed: tile.game.installed,
                    favorite: tile.game.favorite,
                    artwork,
                    has_artwork,
                    source: tile.game.source.label().into(),
                }
            })
            .collect();
        while !entries.len().is_multiple_of(5) {
            entries.push(GameTile::default());
        }
        let rows: Vec<ModelRc<GameTile>> = entries
            .chunks(5)
            .map(|row| Rc::new(VecModel::from(row.to_vec())).into())
            .collect();
        ui.set_game_rows(Rc::new(VecModel::from(rows)).into());
        ui.set_total(status.total as i32);
        ui.set_installed_count(status.installed as i32);
        ui.set_favorite_count(status.favorites as i32);
        ui.set_matched(matched as i32);
        ui.set_offset(offset as i32);
        for source in &status.sources {
            match source.source {
                Source::Steam => ui.set_steam_count(source.total as i32),
                Source::Epic => ui.set_epic_count(source.total as i32),
                Source::Riot => ui.set_riot_count(source.total as i32),
            };
        }
        let summaries: Vec<slint::SharedString> = Source::ALL
            .iter()
            .map(|source| source_summary(&status, *source, unavailable_artwork).into())
            .collect();
        ui.set_source_summaries(Rc::new(VecModel::from(summaries)).into());
        ui.set_loading(false);
    })
    .map_err(|error| error.to_string())
}

fn source_summary(status: &LibraryStatus, selected: Source, artwork_failures: u32) -> String {
    let source = status
        .sources
        .iter()
        .find(|source| source.source == selected);
    let mut lines = vec![format!(
        "{} · {} oyun · {} kurulu",
        selected.label(),
        source.map(|source| source.total).unwrap_or(0),
        source.map(|source| source.installed).unwrap_or(0)
    )];
    if let Some(source) = source {
        for root in &source.roots {
            lines.push(if root == "registry://RiotGames" {
                "Windows kayıt defteri".into()
            } else {
                root.trim_start_matches("\\\\?\\").to_owned()
            });
        }
        if source.roots.is_empty() {
            lines.push(
                "Standart veri klasörü bulunamadı. Veri klasörünü elle ekleyebilirsin.".into(),
            );
        }
    }
    if let Some(duration) = status.last_scan_duration_ms {
        lines.push(format!("Son tarama: {duration:.0} ms"));
    }
    if artwork_failures > 0 {
        lines.push(format!(
            "Bu sayfada {artwork_failures} yerel kapak okunamadı veya boyut sınırını aştı."
        ));
    }
    if !status.warnings.is_empty() {
        lines.push("Son tarama uyarıları (tüm kaynaklar):".into());
        lines.extend(status.warnings.clone());
    }
    lines.join("\n\n")
}

pub fn start(
    store: Store,
    data_dir: PathBuf,
    state: Arc<State>,
    ui: slint::Weak<LibraryWindow>,
) -> mpsc::SyncSender<Command> {
    let (sender, receiver) = mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let mut store = store;
        let mut query = Query {
            filter: "all".into(),
            ..Query::default()
        };
        while let Ok(command) = receiver.recv() {
            let result = match command {
                Command::Query(next) => {
                    query = next;
                    publish(&store, &query, &state, &ui)
                }
                Command::Favorite(id, value) => store
                    .set_favorite(&id, value)
                    .map_err(|error| error.to_string())
                    .and_then(|_| publish(&store, &query, &state, &ui)),
                Command::Launch(id) => (|| {
                    let game = store.game(&id).map_err(|error| error.to_string())?;
                    let plan = launcher::prepare(&game).map_err(|error| error.to_string())?;
                    match plan {
                        launcher::LaunchPlan::Uri { uri } => {
                            platform_windows::shell::open_game_uri(&uri)?
                        }
                        launcher::LaunchPlan::Process {
                            executable,
                            args,
                            working_dir,
                        } => {
                            platform_windows::shell::start_process(
                                &executable,
                                &args,
                                &working_dir,
                            )?;
                        }
                    }
                    set_notice(
                        &ui,
                        format!(
                            "{} için başlatma isteği {}'e iletildi.",
                            game.name,
                            game.source.label()
                        ),
                    );
                    Ok(())
                })(),
                Command::Reports => (|| {
                    let path = data_dir.join("measurements");
                    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
                    open_target(&path.to_string_lossy())
                })(),
                Command::Scan { add_source } => {
                    let result = (|| {
                        if let Some(source) = add_source {
                            let folder = rfd::FileDialog::new()
                                .set_title(format!("{} veri klasörünü seç", source.label()))
                                .pick_folder();
                            let Some(folder) = folder else {
                                return Ok(());
                            };
                            let root = sources::normalize_root(source, &folder)
                                .map_err(|error| error.to_string())?;
                            store
                                .add_source_root(source, &root.to_string_lossy())
                                .map_err(|error| error.to_string())?;
                        }
                        let roots = store.source_roots().map_err(|error| error.to_string())?;
                        let scanned = sources::scan(&roots, &state.cancel_scan);
                        store
                            .apply_source_scan(&scanned)
                            .map_err(|error| error.to_string())?;
                        let message = if scanned.report.cancelled {
                            "Tarama iptal edildi. Tamamlanmamış kaynaklardaki kayıtlar korundu."
                                .into()
                        } else {
                            format!(
                                "{} oyun bulundu · {:.0} ms · {} uyarı",
                                scanned.report.found,
                                scanned.report.duration_ms,
                                scanned.report.warnings.len()
                            )
                        };
                        set_notice(&ui, message);
                        publish(&store, &query, &state, &ui)
                    })();
                    state.scanning.store(false, Ordering::SeqCst);
                    let _ = ui.upgrade_in_event_loop(|ui| ui.set_scanning(false));
                    result
                }
            };
            if let Err(error) = result {
                set_notice(&ui, error);
                let _ = ui.upgrade_in_event_loop(|ui| ui.set_loading(false));
            }
        }
    });
    sender
}

#[cfg(test)]
mod tests {
    use super::thumbnail;

    #[test]
    fn local_artwork_is_resized_and_oversized_files_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("cover.png");
        image::RgbaImage::from_pixel(600, 900, image::Rgba([42, 57, 66, 255]))
            .save(&valid)
            .unwrap();
        let (width, height, pixels) = thumbnail(valid.to_str().unwrap()).unwrap();
        assert_eq!((width, height), (180, 270));
        assert_eq!(pixels.len(), 180 * 270 * 4);
        let oversized = directory.path().join("large.png");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(8 * 1024 * 1024 + 1)
            .unwrap();
        assert!(thumbnail(oversized.to_str().unwrap()).is_none());
        let broken = directory.path().join("broken.jpg");
        std::fs::write(&broken, b"not an image").unwrap();
        assert!(thumbnail(broken.to_str().unwrap()).is_none());
    }
}
