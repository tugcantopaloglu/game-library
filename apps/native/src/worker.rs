use crate::{CollectionTile, CoverTile, GameTile, LibraryWindow};
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
    pub collection: Option<i64>,
}

pub enum Command {
    Query(Query),
    Scan { add_source: Option<Source> },
    Favorite(String, bool),
    Launch(String),
    Reports,
    AddManual,
    Detail(String),
    Tracking(String),
    SaveDetail(String, String, String),
    Cover(String, bool),
    RemoteCover(String, String),
    CollectionCreate(String),
    CollectionDelete(i64),
    CollectionMember(String, i64, bool),
    BridgeEnable(bool),
    BridgeRefresh,
    BridgeAction(String, serde_json::Value),
    Installer(crate::updates::InstallerAction, String),
    PrepareBridge,
    UpdateCheck,
    UpdateStage,
    UpdateApply,
    UpdatePreferences(bool, bool),
    Session(i64, String, Option<u32>, bool),
    ArtworkKey(String),
    SearchCovers(String),
    NetworkDone(NetworkResult),
}

enum NetworkTask {
    Cover(String, String),
    Search(String, String, Option<u32>),
    Installer(crate::updates::InstallerAction, String),
    PrepareBridge,
    Check,
    Stage(crate::updates::UpdateManifest),
}
pub enum NetworkResult {
    Cover(String, Result<PathBuf, String>),
    Search(String, Result<Vec<crate::media::CoverCandidate>, String>),
    Notice(Result<String, String>),
    Check(Result<crate::updates::UpdateManifest, String>),
    Stage(Result<PathBuf, String>),
}

struct Network {
    sender: mpsc::SyncSender<NetworkTask>,
    busy: Arc<AtomicBool>,
}

fn submit(
    network: &Network,
    ui: &slint::Weak<LibraryWindow>,
    task: NetworkTask,
) -> Result<(), String> {
    if network.busy.swap(true, Ordering::SeqCst) {
        return Err("Bir indirme / entegrasyon işlemi zaten sürüyor".into());
    }
    if network.sender.try_send(task).is_err() {
        network.busy.store(false, Ordering::SeqCst);
        return Err("İndirme / entegrasyon kuyruğu dolu; işlem bitince yeniden dene".into());
    }
    let _ = ui.upgrade_in_event_loop(|ui| ui.set_integration_busy(true));
    Ok(())
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
        .list_filtered(
            &query.text,
            &query.filter,
            query.offset,
            100,
            query.source,
            query.collection,
        )
        .map_err(|error| error.to_string())?;
    let offset = if page.total > 0 && query.offset >= page.total {
        ((page.total - 1) / 100) * 100
    } else {
        query.offset
    };
    if offset != query.offset {
        page = store
            .list_filtered(
                &query.text,
                &query.filter,
                offset,
                100,
                query.source,
                query.collection,
            )
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
                Source::Manual => ui.set_manual_count(source.total as i32),
            };
        }
        let summaries: Vec<slint::SharedString> = Source::ALL
            .iter()
            .map(|source| source_summary(&status, *source, unavailable_artwork).into())
            .collect();
        ui.set_source_summaries(Rc::new(VecModel::from(summaries)).into());
        ui.set_loading(false);
        ui.invoke_library_ready();
    })
    .map_err(|error| error.to_string())
}

fn publish_collections(store: &Store, ui: &slint::Weak<LibraryWindow>) -> Result<(), String> {
    let collections = store.collections().map_err(|error| error.to_string())?;
    ui.upgrade_in_event_loop(move |ui| {
        let names: Vec<slint::SharedString> = collections
            .iter()
            .map(|collection| collection.name.clone().into())
            .collect();
        let tiles: Vec<CollectionTile> = collections
            .into_iter()
            .map(|collection| CollectionTile {
                id: collection.id as i32,
                name: collection.name.into(),
                count: collection.games as i32,
            })
            .collect();
        ui.set_collection_names(Rc::new(VecModel::from(names)).into());
        ui.set_collections(Rc::new(VecModel::from(tiles)).into());
    })
    .map_err(|error| error.to_string())
}

fn detail(store: &Store, id: &str, ui: &slint::Weak<LibraryWindow>) -> Result<(), String> {
    detail_impl(store, id, ui, false)
}

fn detail_impl(
    store: &Store,
    id: &str,
    ui: &slint::Weak<LibraryWindow>,
    only_current: bool,
) -> Result<(), String> {
    let detail = store.detail(id).map_err(|error| error.to_string())?;
    let path = detail
        .game
        .install_path
        .trim_start_matches("\\\\?\\")
        .to_string();
    let history = detail
        .sessions
        .iter()
        .map(|session| {
            format!(
                "{} · {}{}",
                session.started_ms,
                match session.status.as_str() {
                    "requested" => "Başlatma isteği",
                    "running" => "Çalışan süreç doğrulandı",
                    "ended" => "Süreç sona erdi",
                    "failed" => "Başlatma başarısız",
                    "unconfirmed" => "Oyun süreci doğrulanamadı",
                    _ => "Oturum kesildi",
                },
                session
                    .pid
                    .map(|pid| format!(" · PID {pid}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let memberships = detail
        .collections
        .iter()
        .map(|collection| collection.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let pixels = detail.game.artwork_path.as_deref().and_then(thumbnail);
    ui.upgrade_in_event_loop(move |ui| {
        if only_current && (ui.get_detail_id() != detail.game.id || ui.get_section() != 9) {
            return;
        }
        if ui.get_detail_id() != detail.game.id {
            ui.set_cover_candidates(Rc::new(VecModel::<CoverTile>::default()).into());
            ui.set_cover_names(Rc::new(VecModel::<slint::SharedString>::default()).into());
            ui.set_cover_index(-1);
        }
        ui.set_detail_id(detail.game.id.into());
        ui.set_detail_title(detail.game.name.into());
        ui.set_detail_notes(detail.notes.into());
        ui.set_detail_path(path.into());
        ui.set_detail_history(if history.is_empty() {
            "Henüz oyun oturumu yok.".into()
        } else {
            history.into()
        });
        ui.set_detail_collections(memberships.into());
        ui.set_detail_installed(detail.game.installed);
        ui.set_detail_artwork(
            pixels
                .map(|(width, height, bytes)| {
                    slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
                        &bytes, width, height,
                    ))
                })
                .unwrap_or_default(),
        );
        ui.set_section(9);
    })
    .map_err(|error| error.to_string())
}

fn bridge_status(
    bridge: Option<&crate::bridge::Bridge>,
    ui: &slint::Weak<LibraryWindow>,
) -> Result<(), String> {
    let snapshot = bridge
        .map(|bridge| bridge.snapshot())
        .unwrap_or_else(|| serde_json::json!({"connected":false,"status":"Yerel bağlantı kapalı"}));
    let connected = snapshot["connected"] == true;
    let mut lines = vec![snapshot["status"]
        .as_str()
        .unwrap_or("Vencord bekleniyor")
        .to_string()];
    if connected {
        if let Some(friends) = snapshot["friends"].as_array() {
            lines.push("\nSeçili kişiler".into());
            for friend in friends {
                lines.push(format!(
                    "{} · {} · {}",
                    friend["name"].as_str().unwrap_or(""),
                    friend["status"].as_str().unwrap_or("bilinmiyor"),
                    friend["activity"].as_str().unwrap_or("")
                ));
            }
        }
        if let Some(rooms) = snapshot["rooms"].as_array() {
            lines.push("\nSes odaları".into());
            for room in rooms {
                lines.push(format!(
                    "{} / {} · {} kişi",
                    room["guild"].as_str().unwrap_or(""),
                    room["name"].as_str().unwrap_or(""),
                    room["members"]
                ));
            }
        }
        if let Some(result) = snapshot["result"].as_str() {
            if !result.is_empty() {
                lines.push(format!("\n{result}"));
            }
        }
    }
    ui.upgrade_in_event_loop(move |ui| {
        ui.set_bridge_connected(connected);
        ui.set_bridge_status(lines.join("\n").into());
        ui.set_can_mute(connected && snapshot["capabilities"]["mute"] == true);
        ui.set_can_deafen(connected && snapshot["capabilities"]["deafen"] == true);
        ui.set_can_stop_stream(
            connected
                && snapshot["capabilities"]["stream_stop"] == true
                && !snapshot["stream"].is_null(),
        );
        ui.set_muted(snapshot["muted"] == true);
        ui.set_deafened(snapshot["deafened"] == true);
        ui.invoke_bridge_ready();
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
    let session_sender = sender.clone();
    let network_sender = sender.clone();
    let network_data = data_dir.clone();
    let (network, network_receiver) = mpsc::sync_channel(2);
    let busy = Arc::new(AtomicBool::new(false));
    let network = Network {
        sender: network,
        busy: busy.clone(),
    };
    std::thread::spawn(move || {
        while let Ok(task) = network_receiver.recv() {
            let result = match task {
                NetworkTask::Cover(id, url) => {
                    NetworkResult::Cover(id, crate::media::remote_cover(&network_data, &url))
                }
                NetworkTask::Search(id, title, steam_id) => {
                    NetworkResult::Search(id, crate::media::search_covers(&title, steam_id))
                }
                NetworkTask::Installer(action, branch) => NetworkResult::Notice(
                    crate::updates::vencord_installer(&network_data, action, &branch),
                ),
                NetworkTask::PrepareBridge => {
                    NetworkResult::Notice(crate::updates::prepare_bridge(&network_data))
                }
                NetworkTask::Check => NetworkResult::Check(crate::updates::check()),
                NetworkTask::Stage(manifest) => {
                    NetworkResult::Stage(crate::updates::stage(&network_data, &manifest))
                }
            };
            busy.store(false, Ordering::SeqCst);
            if network_sender.send(Command::NetworkDone(result)).is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        let mut store = store;
        let _ = store.recover_sessions();
        let mut bridge = None;
        let mut available_update = None;
        let mut staged_update = None;
        let mut auto_update_app =
            store.setting("auto_update_app").ok().flatten().as_deref() == Some("true");
        let mut next_update = std::time::Instant::now();
        let mut query = Query {
            filter: "all".into(),
            ..Query::default()
        };
        loop {
            let command = if auto_update_app && std::time::Instant::now() >= next_update {
                next_update =
                    std::time::Instant::now() + std::time::Duration::from_secs(24 * 60 * 60);
                Command::UpdateCheck
            } else {
                match if auto_update_app {
                    receiver.recv_timeout(std::time::Duration::from_secs(3600))
                } else {
                    receiver
                        .recv()
                        .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                } {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                }
            };
            let result = match command {
                Command::ArtworkKey(key) => platform_windows::credentials::save_artwork_key(&key)
                    .map(|_| {
                        set_notice(
                            &ui,
                            "SteamGridDB anahtarı Windows Credential Manager'da kaydedildi.".into(),
                        )
                    }),
                Command::SearchCovers(id) => store
                    .game(&id)
                    .map_err(|error| error.to_string())
                    .and_then(|game| {
                        submit(
                            &network,
                            &ui,
                            NetworkTask::Search(
                                id,
                                game.name,
                                (game.source == Source::Steam).then_some(game.app_id),
                            ),
                        )
                    }),
                Command::NetworkDone(result) => {
                    let _ = ui.upgrade_in_event_loop(|ui| ui.set_integration_busy(false));
                    match result {
                        NetworkResult::Notice(result) => result.map(|notice|set_notice(&ui,notice)),
                        NetworkResult::Search(id,result) => result.and_then(|candidates| {
                            if candidates.is_empty() {return Err("Uygun kapak bulunamadı.".into());}
                            let names: Vec<slint::SharedString> = candidates.iter().map(|candidate|candidate.title.clone().into()).collect();
                            let tiles: Vec<CoverTile> = candidates.into_iter().map(|candidate|CoverTile {title:candidate.title.into(),url:candidate.url.into()}).collect();
                            ui.upgrade_in_event_loop(move |ui|{if ui.get_detail_id() != id || ui.get_section() != 9 {return;} ui.set_cover_candidates(Rc::new(VecModel::from(tiles)).into());ui.set_cover_names(Rc::new(VecModel::from(names)).into());ui.set_cover_index(0);}).map_err(|error|error.to_string())
                        }),
                        NetworkResult::Cover(id,result) => result.and_then(|cover| {
                            let current = store.detail(&id).map_err(|error|error.to_string())?;
                            store.set_override(&id,Some(&current.game.name),&current.notes,Some(&cover.to_string_lossy())).map_err(|error|error.to_string())?;
                            publish(&store,&query,&state,&ui)?; detail_impl(&store,&id,&ui,true)
                        }),
                        NetworkResult::Check(result) => result.and_then(|manifest| {
                            let version = manifest.version.clone(); available_update = Some(manifest.clone());
                            ui.upgrade_in_event_loop(move |ui| {ui.set_update_status(format!("{version} sürümü hazır. İmza doğrulandı.").into());ui.set_update_available(true);}).map_err(|error|error.to_string())?;
                            if auto_update_app {submit(&network,&ui,NetworkTask::Stage(manifest))?;}
                            Ok(())
                        }),
                        NetworkResult::Stage(result) => result.and_then(|path| {
                            staged_update = Some(path);
                            ui.upgrade_in_event_loop(|ui|{ui.set_update_staged(true);ui.set_update_status("Güncelleme indirildi ve doğrulandı. Yeniden başlatınca uygulanabilir.".into());}).map_err(|error|error.to_string())
                        }),
                    }
                }
                Command::Session(id, status, pid, finished) => store
                    .session_update(id, &status, pid, finished)
                    .map_err(|error| error.to_string()),
                Command::AddManual => (|| {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Oyun executable dosyasını seç")
                        .add_filter("Windows executable", &["exe"])
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    let title = path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Oyun");
                    let id = store
                        .add_manual(&path, title, vec![])
                        .map_err(|error| error.to_string())?;
                    publish(&store, &query, &state, &ui)?;
                    detail(&store, &id, &ui)
                })(),
                Command::Detail(id) => detail(&store, &id, &ui),
                Command::Tracking(id) => (|| {
                    let game = store.game(&id).map_err(|error| error.to_string())?;
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("İzlenecek oyun executable dosyasını seç")
                        .set_directory(&game.install_path)
                        .add_filter("Executable", &["exe"])
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    store
                        .set_tracking_executable(&id, &path)
                        .map_err(|error| error.to_string())?;
                    set_notice(
                        &ui,
                        "Oturum takibi için oyun executable dosyası kaydedildi.".into(),
                    );
                    Ok(())
                })(),
                Command::SaveDetail(id, title, notes) => (|| {
                    let artwork = store
                        .game(&id)
                        .map_err(|error| error.to_string())?
                        .artwork_path;
                    store
                        .set_override(&id, Some(&title), &notes, artwork.as_deref())
                        .map_err(|error| error.to_string())?;
                    set_notice(&ui, "Oyun bilgileri kaydedildi.".into());
                    publish(&store, &query, &state, &ui)?;
                    detail(&store, &id, &ui)
                })(),
                Command::Cover(id, reset) => (|| {
                    let current = store.detail(&id).map_err(|error| error.to_string())?;
                    let artwork = if reset {
                        None
                    } else {
                        let Some(path) = rfd::FileDialog::new()
                            .set_title("Kapak seç")
                            .add_filter("Kapak", &["png", "jpg", "jpeg"])
                            .pick_file()
                        else {
                            return Ok(());
                        };
                        Some(
                            crate::media::import_cover(&data_dir, &path)?
                                .to_string_lossy()
                                .into_owned(),
                        )
                    };
                    store
                        .set_override(
                            &id,
                            Some(&current.game.name),
                            &current.notes,
                            artwork.as_deref(),
                        )
                        .map_err(|error| error.to_string())?;
                    publish(&store, &query, &state, &ui)?;
                    detail(&store, &id, &ui)
                })(),
                Command::RemoteCover(id, url) => submit(&network, &ui, NetworkTask::Cover(id, url)),
                Command::CollectionCreate(name) => store
                    .create_collection(&name)
                    .map_err(|error| error.to_string())
                    .and_then(|_| publish_collections(&store, &ui)),
                Command::CollectionDelete(id) => store
                    .delete_collection(id)
                    .map_err(|error| error.to_string())
                    .and_then(|_| publish_collections(&store, &ui)),
                Command::CollectionMember(id, collection, member) => store
                    .collection_member(collection, &id, member)
                    .map_err(|error| error.to_string())
                    .and_then(|_| publish_collections(&store, &ui))
                    .and_then(|_| detail(&store, &id, &ui)),
                Command::BridgeEnable(enabled) => (|| {
                    store
                        .set_setting("bridge_enabled", if enabled { "true" } else { "false" })
                        .map_err(|error| error.to_string())?;
                    if enabled && bridge.is_none() {
                        bridge = Some(crate::bridge::Bridge::start(&data_dir)?);
                    }
                    if !enabled {
                        bridge = None;
                    }
                    bridge_status(if enabled { bridge.as_ref() } else { None }, &ui)
                })(),
                Command::BridgeRefresh => (|| {
                    if let Some(bridge) = &bridge {
                        let snapshot = bridge.snapshot();
                        let desired = store
                            .setting("auto_update_vencord")
                            .map_err(|error| error.to_string())?
                            .as_deref()
                            != Some("false");
                        if snapshot["connected"] == true && snapshot["autoUpdate"] != desired {
                            bridge
                                .command("auto_update", serde_json::json!({"enabled":desired}))?;
                        }
                    }
                    bridge_status(bridge.as_ref(), &ui)
                })(),
                Command::BridgeAction(action, data) => bridge
                    .as_ref()
                    .ok_or("Discord bağlantısını aç.")
                    .and_then(|bridge| {
                        bridge
                            .command(&action, data)
                            .map_err(|_| "Discord işlemi gönderilemedi")
                    })
                    .map_err(str::to_string),
                Command::Installer(action, branch) => {
                    submit(&network, &ui, NetworkTask::Installer(action, branch))
                }
                Command::PrepareBridge => submit(&network, &ui, NetworkTask::PrepareBridge),
                Command::UpdateCheck => submit(&network, &ui, NetworkTask::Check),
                Command::UpdateStage => available_update
                    .as_ref()
                    .ok_or_else(|| "Önce güncelleme kontrolü yap.".to_string())
                    .and_then(|manifest| {
                        submit(&network, &ui, NetworkTask::Stage(manifest.clone()))
                    }),
                Command::UpdateApply => staged_update
                    .as_ref()
                    .ok_or_else(|| "Önce doğrulanmış güncellemeyi indir.".to_string())
                    .and_then(|path| crate::updates::request_apply(&data_dir, path))
                    .and_then(|_| {
                        ui.upgrade_in_event_loop(|_| {
                            let _ = slint::quit_event_loop();
                        })
                        .map_err(|error| error.to_string())
                    }),
                Command::UpdatePreferences(app, vencord) => (|| {
                    auto_update_app = app;
                    store
                        .set_setting("auto_update_app", if app { "true" } else { "false" })
                        .map_err(|error| error.to_string())?;
                    store
                        .set_setting(
                            "auto_update_vencord",
                            if vencord { "true" } else { "false" },
                        )
                        .map_err(|error| error.to_string())?;
                    if let Some(bridge) = &bridge {
                        bridge.command("auto_update", serde_json::json!({"enabled":vencord}))?;
                    }
                    set_notice(&ui, "Güncelleme tercihleri kaydedildi.".into());
                    Ok(())
                })(),
                Command::Query(next) => {
                    query = next;
                    publish_collections(&store, &ui)
                        .and_then(|_| publish(&store, &query, &state, &ui))
                }
                Command::Favorite(id, value) => store
                    .set_favorite(&id, value)
                    .map_err(|error| error.to_string())
                    .and_then(|_| publish(&store, &query, &state, &ui)),
                Command::Launch(id) => (|| {
                    let game = store.game(&id).map_err(|error| error.to_string())?;
                    let plan = launcher::prepare(&game).map_err(|error| error.to_string())?;
                    let session = store
                        .session_start(&id)
                        .map_err(|error| error.to_string())?;
                    let executable = store
                        .tracking_executable(&id)
                        .map_err(|error| error.to_string())?
                        .map(PathBuf::from)
                        .or_else(|| match &game.launch {
                            library_core::models::LaunchTarget::Manual { executable, .. }
                            | library_core::models::LaunchTarget::Epic { executable, .. }
                            | library_core::models::LaunchTarget::Riot { executable, .. } => {
                                Some(PathBuf::from(executable))
                            }
                            _ => None,
                        });
                    let before = if executable.is_some() {
                        platform_windows::sessions::baseline()
                    } else {
                        vec![]
                    };
                    let launched = (|| -> Result<(), String> {
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
                        Ok(())
                    })();
                    if let Err(error) = launched {
                        store
                            .session_update(session, "failed", None, true)
                            .map_err(|error| error.to_string())?;
                        return Err(error);
                    }
                    if let Some(executable) = executable {
                        let sender = session_sender.clone();
                        platform_windows::sessions::watch(
                            executable,
                            before,
                            move |status, pid, finished| {
                                let _ = sender.send(Command::Session(
                                    session,
                                    status.into(),
                                    pid,
                                    finished,
                                ));
                            },
                        );
                    } else {
                        store
                            .session_update(session, "unconfirmed", None, true)
                            .map_err(|error| error.to_string())?;
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
