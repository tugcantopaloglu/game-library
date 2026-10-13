use crate::{CollectionTile, CoverTile, GameTile, LibraryWindow, RoomTile, SocialTile};
use library_core::models::{Game, LibraryStatus, Source};
use library_core::{launcher, sources, storage::Store};
use platform_windows::shell::open_target;
use slint::{Model, ModelRc, SharedPixelBuffer, VecModel};
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

pub struct CoverRequest {
    game: Game,
    url: Option<String>,
}
type CoverQueue = mpsc::SyncSender<CoverRequest>;
pub struct State {
    pub scanning: AtomicBool,
    pub cancel_scan: AtomicBool,
    pub revision: AtomicU64,
    pub cover_requests: std::sync::Mutex<Option<CoverQueue>>,
    pub cover_attempts: std::sync::Mutex<std::collections::HashSet<String>>,
}

#[derive(Clone, Default)]
pub struct Query {
    pub text: String,
    pub filter: String,
    pub offset: u32,
    pub revision: u64,
    pub source: Option<Source>,
    pub collection: Option<i64>,
    pub columns: usize,
}

pub enum Command {
    Query(Query),
    Scan {
        add_source: Option<Source>,
    },
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
    SocialFilter(String, bool),
    FriendFavorite(String, bool),
    RoomFilter(i32, String),
    RoomFavorite(String, bool),
    StreamControl(String, String, i32),
    Installer(crate::updates::InstallerAction, String),
    PrepareBridge,
    UpdateCheck,
    UpdateStage,
    UpdateApply,
    UpdatePreferences(bool, bool),
    Session(i64, String, Option<u32>, bool),
    ArtworkKey(String),
    SearchCovers(String),
    SteamCredentials(String, String),
    SteamKeyPage,
    SyncAccount(Source),
    EpicLogin,
    Completion(String, String),
    CompletionOpen(String, bool),
    AutoCover(
        String,
        Result<PathBuf, String>,
        Option<library_core::models::CompletionTimes>,
    ),
    NetworkDone(NetworkResult),
}

enum NetworkTask {
    Cover(String, String),
    Search(String, String, Option<u32>),
    Installer(crate::updates::InstallerAction, String),
    PrepareBridge,
    Check,
    Stage(crate::updates::UpdateManifest),
    Owned(Source, String),
    EpicLogin,
    Completion(String, Game, String),
}
pub enum NetworkResult {
    Cover(String, Result<PathBuf, String>),
    Search(String, Result<Vec<crate::media::CoverCandidate>, String>),
    Notice(Result<String, String>),
    Check(Result<crate::updates::UpdateManifest, String>),
    Stage(Result<PathBuf, String>),
    Owned(Source, Result<crate::accounts::EpicLibrary, String>),
    Completion(String, Result<crate::completion::FetchedCompletion, String>),
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
        if pixels.is_none() && game.source != Source::Manual {
            let url=store.artwork_url(&game).ok().flatten().or_else(||(game.source==Source::Steam).then(||format!("https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/{}/library_600x900.jpg",game.app_id)));
            if let (Ok(queue), Ok(mut attempted)) =
                (state.cover_requests.lock(), state.cover_attempts.lock())
            {
                if !attempted.contains(&game.id)
                    && queue.as_ref().is_some_and(|queue| {
                        queue
                            .try_send(CoverRequest {
                                game: game.clone(),
                                url,
                            })
                            .is_ok()
                    })
                {
                    attempted.insert(game.id.clone());
                }
            }
        }
        if game.artwork_path.is_some() && pixels.is_none() {
            unavailable_artwork += 1;
        }
        tiles.push(Tile { game, pixels });
    }
    let revision = query.revision;
    let matched = page.total;
    let columns = query.columns.clamp(3, 6);
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
                    status_label: if tile.game.installed {
                        "Kurulu"
                    } else if tile.game.install_path.is_empty() {
                        "Kurulu değil"
                    } else {
                        "Erişilemiyor"
                    }
                    .into(),
                }
            })
            .collect();
        while !entries.len().is_multiple_of(columns) {
            entries.push(GameTile::default());
        }
        let rows: Vec<ModelRc<GameTile>> = entries
            .chunks(columns)
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

fn set_completion(ui: &LibraryWindow, times: Option<&library_core::models::CompletionTimes>) {
    ui.set_completion_main(
        times
            .map(|value| crate::completion::hours(value.main_seconds))
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    ui.set_completion_extra(
        times
            .map(|value| crate::completion::hours(value.extra_seconds))
            .unwrap_or_else(|| "—".into())
            .into(),
    );
    ui.set_completion_full(
        times
            .map(|value| crate::completion::hours(value.completionist_seconds))
            .unwrap_or_else(|| "—".into())
            .into(),
    );
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
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_millis() as i64)
                .unwrap_or(session.started_ms);
            let minutes = now_ms.saturating_sub(session.started_ms).max(0) / 60_000;
            let ago = if minutes < 1 {
                "Az önce".into()
            } else if minutes < 60 {
                format!("{minutes} dk önce")
            } else if minutes < 1440 {
                format!("{} saat önce", minutes / 60)
            } else {
                format!("{} gün önce", minutes / 1440)
            };
            let duration = session
                .ended_ms
                .filter(|end| *end >= session.started_ms)
                .map(|end| format!(" · {} dk", (end - session.started_ms) / 60_000))
                .unwrap_or_default();
            format!(
                "{} · {}{}",
                ago,
                match session.status.as_str() {
                    "requested" => "Başlatılıyor",
                    "running" => "Oynanıyor",
                    "ended" => "Oynandı",
                    "failed" => "Başlatılamadı",
                    "unconfirmed" => "Başlatma doğrulanamadı",
                    _ => "Takip kesildi",
                },
                duration
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
    let times = detail
        .completion
        .as_ref()
        .map(crate::completion::summary)
        .unwrap_or_else(|| "HowLongToBeat süresi henüz alınmadı.".into());
    ui.upgrade_in_event_loop(move |ui| {
        if only_current && (ui.get_detail_id() != detail.game.id || ui.get_section() != 9) {
            return;
        }
        if only_current {
            ui.set_detail_has_artwork(pixels.is_some());
            ui.set_detail_artwork(
                pixels
                    .map(|(width, height, bytes)| {
                        slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
                            &bytes, width, height,
                        ))
                    })
                    .unwrap_or_default(),
            );
            return;
        }
        if ui.get_detail_id() != detail.game.id {
            ui.set_cover_candidates(Rc::new(VecModel::<CoverTile>::default()).into());
            ui.set_cover_names(Rc::new(VecModel::<slint::SharedString>::default()).into());
            ui.set_cover_index(-1);
        }
        ui.set_detail_id(detail.game.id.into());
        ui.set_detail_source(detail.game.source.label().into());
        ui.set_detail_size(size_text(detail.game.size_bytes).into());
        ui.set_detail_favorite(detail.game.favorite);
        ui.set_detail_has_artwork(pixels.is_some());
        set_completion(&ui, detail.completion.as_ref());
        ui.set_completion_summary(times.into());
        ui.set_completion_url(
            detail
                .completion
                .map(|completion| format!("https://howlongtobeat.com/game/{}", completion.hltb_id))
                .unwrap_or_default()
                .into(),
        );
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
    store: &Store,
    query: &crate::social::Query,
    ui: &slint::Weak<LibraryWindow>,
) -> Result<(), String> {
    let snapshot = bridge
        .map(|bridge| bridge.snapshot())
        .unwrap_or_else(|| serde_json::json!({"connected":false,"status":"Yerel bağlantı kapalı"}));
    let connected = snapshot["connected"] == true;
    let favorites = snapshot["accountId"]
        .as_str()
        .and_then(|account| store.friend_favorites(account).ok())
        .unwrap_or_default();
    let can_favorite = connected
        && snapshot["accountId"]
            .as_str()
            .is_some_and(|account| store.friend_favorites(account).is_ok());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_millis() as u64)
        .unwrap_or_default();
    let total = snapshot["friends"].as_array().map(Vec::len).unwrap_or(0);
    let favorite_count = snapshot["friends"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|friend| {
            friend["id"]
                .as_str()
                .is_some_and(|id| favorites.iter().any(|saved| saved == id))
        })
        .count();
    let friends = crate::social::friends(&snapshot, &favorites, query, now_ms);
    let room_favorites = snapshot["accountId"]
        .as_str()
        .and_then(|account| store.room_favorites(account).ok())
        .unwrap_or_default();
    let rooms = crate::social::rooms(&snapshot, &room_favorites, query);
    let can_favorite_rooms =
        connected && snapshot["capabilities"]["favorite_rooms"] == true && can_favorite;
    let catalog_pending = connected
        && query.room_mode == 2
        && snapshot["roomCatalogQuery"].as_str() != Some(query.room_query.as_str());
    let stream = crate::stream::view(&snapshot);
    let mut lines = vec![snapshot["status"]
        .as_str()
        .unwrap_or("Vencord bekleniyor")
        .to_string()];
    if connected {
        if let Some(result) = snapshot["result"].as_str() {
            if !result.is_empty() {
                lines.push(format!("\n{result}"));
            }
        }
    }
    ui.upgrade_in_event_loop(move |ui| {
        ui.set_bridge_connected(connected);
        ui.set_social_fixture(snapshot["fixture"] == true);
        ui.set_bridge_status(lines.join("\n").into());
        ui.set_can_favorite_friends(can_favorite);
        ui.set_recent_conversations(
            connected && snapshot["capabilities"]["recent_conversations"] == true,
        );
        ui.set_friend_count(if connected { total as i32 } else { 0 });
        ui.set_favorite_friend_count(if connected { favorite_count as i32 } else { 0 });
        ui.set_can_favorite_rooms(can_favorite_rooms);
        ui.set_can_room_catalog(connected && snapshot["capabilities"]["room_catalog"] == true);
        ui.set_favorite_room_count(room_favorites.len() as i32);
        ui.set_room_catalog_pending(catalog_pending);
        ui.set_room_catalog_more(snapshot["roomCatalogMore"] == true);
        ui.set_can_inspect_stream(connected && snapshot["capabilities"]["stream_inspect"] == true);
        ui.set_stream_active(stream.active);
        ui.set_stream_known(stream.known);
        ui.set_stream_key(stream.key.into());
        ui.set_stream_source(
            if stream.source_name.is_empty() {
                if stream.source_id.starts_with("window:") {
                    "Paylaşılan pencere".into()
                } else if stream.source_id.starts_with("screen:") {
                    "Paylaşılan ekran".into()
                } else {
                    "Yayın kaynağı".into()
                }
            } else {
                stream.source_name
            }
            .into(),
        );
        ui.set_stream_pending(stream.pending);
        ui.set_stream_result(stream.result.into());
        ui.set_stream_audio(
            match stream.audio {
                Some(true) => "Açık",
                Some(false) => "Kapalı",
                None => "Bilinmiyor",
            }
            .into(),
        );
        ui.set_stream_resolution_index(
            stream
                .resolution
                .and_then(|value| {
                    stream
                        .resolutions
                        .iter()
                        .position(|option| *option == value)
                })
                .map(|index| index as i32)
                .unwrap_or(-1),
        );
        ui.set_stream_fps_index(
            stream
                .fps
                .and_then(|value| {
                    stream
                        .fps_options
                        .iter()
                        .position(|option| *option == value)
                })
                .map(|index| index as i32)
                .unwrap_or(-1),
        );
        ui.set_stream_resolutions(
            Rc::new(VecModel::from(
                stream
                    .resolutions
                    .iter()
                    .map(|value| slint::SharedString::from(format!("{value}p")))
                    .collect::<Vec<_>>(),
            ))
            .into(),
        );
        ui.set_stream_fps_options(
            Rc::new(VecModel::from(
                stream
                    .fps_options
                    .iter()
                    .map(|value| slint::SharedString::from(format!("{value} FPS")))
                    .collect::<Vec<_>>(),
            ))
            .into(),
        );
        ui.set_can_stream_quality(
            connected
                && snapshot["capabilities"]["stream_quality"] == true
                && stream.active
                && !stream.pending,
        );
        ui.set_can_stream_fps(
            connected
                && snapshot["capabilities"]["stream_fps"] == true
                && stream.active
                && !stream.pending,
        );
        let friends: Vec<SocialTile> = friends
            .iter()
            .map(|friend| {
                let name = friend.name.as_str();
                let status = friend.status.as_str();
                let label = match status {
                    "online" => "Çevrimiçi",
                    "idle" => "Boşta",
                    "dnd" => "Rahatsız etmeyin",
                    "offline" => "Çevrimdışı",
                    _ => "Durum bilinmiyor",
                };
                let activity = friend.activity.as_str();
                SocialTile {
                    id: friend.id.as_str().into(),
                    name: name.into(),
                    subtitle: if activity.is_empty() {
                        label.to_string()
                    } else {
                        format!("{label} · {activity}")
                    }
                    .into(),
                    initial: name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string()
                        .into(),
                    online: matches!(status, "online" | "idle" | "dnd"),
                    favorite: favorites.contains(&friend.id),
                    recent: crate::social::last_conversation(friend.last_message_at, now_ms).into(),
                }
            })
            .collect();
        let rooms: Vec<RoomTile> = rooms
            .iter()
            .map(|room| RoomTile {
                id: room.id.as_str().into(),
                name: room.name.as_str().into(),
                subtitle: if !room.available {
                    "Erişim yok veya oda silinmiş".into()
                } else {
                    let occupancy = room
                        .members
                        .map(|count| {
                            if count == 0 {
                                "Boş".to_string()
                            } else {
                                format!("{count} kişi")
                            }
                        })
                        .unwrap_or("Durum bilinmiyor".into());
                    format!("{} · {occupancy}", room.guild).into()
                },
                online: room.available && room.members.is_some_and(|count| count > 0),
                favorite: room_favorites.contains(&room.id),
                can_join: room.available
                    && room.can_join
                    && snapshot["capabilities"]["join_voice"] == true,
            })
            .collect();
        ui.set_friends(Rc::new(VecModel::from(friends)).into());
        ui.set_rooms(Rc::new(VecModel::from(rooms)).into());
        ui.set_can_mute(connected && snapshot["capabilities"]["mute"] == true);
        ui.set_can_deafen(connected && snapshot["capabilities"]["deafen"] == true);
        ui.set_can_stop_stream(
            connected
                && snapshot["capabilities"]["stream_stop"] == true
                && stream.active
                && !stream.pending,
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
    let prefix = format!("[{}]", selected.label());
    let warnings: Vec<String> = status
        .warnings
        .iter()
        .filter(|warning| warning.starts_with(&prefix))
        .cloned()
        .collect();
    if !warnings.is_empty() {
        lines.push("Bu kaynağın son tarama uyarıları:".into());
        lines.extend(warnings);
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
    let (cover_queue, cover_receiver) = mpsc::sync_channel::<CoverRequest>(100);
    if let Ok(mut queue) = state.cover_requests.lock() {
        *queue = Some(cover_queue);
    }
    let cover_sender = sender.clone();
    let cover_data = data_dir.clone();
    std::thread::spawn(move || {
        while let Ok(request) = cover_receiver.recv() {
            let mut completion = None;
            let result = (|| {
                let url = if let Some(url) = request.url {
                    url
                } else {
                    let found = crate::completion::lookup(&request.game, "")?;
                    completion = Some(found.times);
                    found
                        .image_url
                        .ok_or("Bu oyunun kaynak kapağı bulunamadı")?
                };
                crate::media::remote_cover(&cover_data, &url, crate::media::CoverUse::Automatic)
            })();
            if cover_sender
                .send(Command::AutoCover(request.game.id, result, completion))
                .is_err()
            {
                break;
            }
        }
    });
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
                    NetworkResult::Cover(id, crate::media::remote_cover(&network_data, &url, crate::media::CoverUse::Selected))
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
                NetworkTask::Owned(source,id) => NetworkResult::Owned(source,match source {Source::Steam=>crate::accounts::steam(&id).map(|games|(games,vec![])),Source::Epic=>crate::accounts::epic(&network_data),_=>Err("Hesap kaynağı desteklenmiyor".into())}),
                NetworkTask::EpicLogin => NetworkResult::Notice(crate::accounts::login_epic(&network_data).map(|_|"Epic giriş penceresi açıldı. Bağlantıyı tamamladıktan sonra hesap oyunlarını eşitle.".into())),
                NetworkTask::Completion(id,game,url) => NetworkResult::Completion(id,crate::completion::lookup(&game,&url)),
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
        let mut completion_attempts = std::collections::HashSet::new();
        let mut social_query = crate::social::Query::default();
        let mut social_view = None;
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
                Command::AutoCover(id, result, completion) => (|| {
                    if let Some(completion) = completion {
                        store
                            .save_completion(&id, &completion)
                            .map_err(|error| error.to_string())?;
                    }
                    let Ok(path) = result else {
                        return Ok(());
                    };
                    store
                        .set_source_artwork(&id, &path.to_string_lossy())
                        .map_err(|error| error.to_string())?;
                    let current = store.game(&id).map_err(|error| error.to_string())?;
                    if current.artwork_path.as_deref() != Some(path.to_string_lossy().as_ref()) {
                        return Ok(());
                    }
                    let Some((width, height, bytes)) = thumbnail(&path.to_string_lossy()) else {
                        return Ok(());
                    };
                    ui.upgrade_in_event_loop(move |ui| {
                        let image = slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
                            &bytes, width, height,
                        ));
                        let rows = ui.get_game_rows();
                        for row in rows.iter() {
                            for column in 0..row.row_count() {
                                if let Some(mut tile) = row.row_data(column) {
                                    if tile.id == id {
                                        tile.artwork = image.clone();
                                        tile.has_artwork = true;
                                        row.set_row_data(column, tile);
                                    }
                                }
                            }
                        }
                        if ui.get_detail_id() == id {
                            ui.set_detail_artwork(image);
                            ui.set_detail_has_artwork(true);
                        }
                    })
                    .map_err(|error| error.to_string())
                })(),
                Command::SteamKeyPage => open_target("https://steamcommunity.com/dev/apikey")
                    .map_err(|error| error.to_string()),
                Command::SteamCredentials(id, key) => (|| {
                    if id.len() != 17 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
                        return Err("SteamID64 17 rakam olmalı".into());
                    }
                    if !key.is_empty() {
                        platform_windows::credentials::save_steam_key(&key)?;
                    }
                    store
                        .set_setting("steam_account_id", &id)
                        .map_err(|error| error.to_string())?;
                    let _ = ui.upgrade_in_event_loop(|ui| {
                        ui.set_steam_account_status(
                            "Hesap bilgileri kaydedildi. Hesap oyunlarını eşitle ile getir.".into(),
                        )
                    });
                    Ok(())
                })(),
                Command::SyncAccount(source) => submit(
                    &network,
                    &ui,
                    NetworkTask::Owned(
                        source,
                        store
                            .setting("steam_account_id")
                            .ok()
                            .flatten()
                            .or_else(library_core::steam::active_user_id)
                            .unwrap_or_default(),
                    ),
                ),
                Command::EpicLogin => submit(&network, &ui, NetworkTask::EpicLogin),
                Command::Completion(id, url) => store
                    .game(&id)
                    .map_err(|error| error.to_string())
                    .and_then(|game| submit(&network, &ui, NetworkTask::Completion(id, game, url))),
                Command::CompletionOpen(id, search) => (|| {
                    let game = store.game(&id).map_err(|error| error.to_string())?;
                    let mut url = url::Url::parse("https://howlongtobeat.com/")
                        .map_err(|error| error.to_string())?;
                    if let Some(completion) = store
                        .completion(&game)
                        .map_err(|error| error.to_string())?
                        .filter(|_| !search)
                    {
                        url.set_path(&format!("/game/{}", completion.hltb_id));
                    } else {
                        url.query_pairs_mut().append_pair("q", &game.name);
                    }
                    open_target(url.as_str()).map_err(|error| error.to_string())
                })(),
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
                        NetworkResult::Owned(source,result) => result.and_then(|(mut games,covers)| {
                            if source==Source::Steam { let roots=library_core::steam::discover_roots(&store.roots().map_err(|error|error.to_string())?);for game in &mut games {game.artwork_path=library_core::steam::local_artwork(&roots,game.app_id);} }
                            store.apply_owned_games(source,&games).map_err(|error|error.to_string())?;
                            for (id,url) in covers { store.set_artwork_url(source,&id,&url).map_err(|error|error.to_string())?; }
                            let status=format!("{} hesabından {} oyun eşitlendi. Kurulum bilgileri korundu.",source.label(),games.len());
                            ui.upgrade_in_event_loop(move |ui| if source==Source::Steam {ui.set_steam_account_status(status.into());}else{ui.set_epic_account_status(status.into());}).map_err(|error|error.to_string())?;
                            publish(&store,&query,&state,&ui)
                        }),
                        NetworkResult::Completion(id,result) => (|| {
                            let (summary,url)=match result {Ok(completion)=> {store.save_completion(&id,&completion.times).map_err(|error|error.to_string())?; let game=store.game(&id).map_err(|error|error.to_string())?; if let Some(url)=completion.image_url {store.set_artwork_url(game.source,&game.source_id,&url).map_err(|error|error.to_string())?;} (crate::completion::summary(&completion.times),Some(format!("https://howlongtobeat.com/game/{}",completion.times.hltb_id)))},Err(error)=>(format!("HowLongToBeat: {error}"),None)};
                            let times=store.completion(&store.game(&id).map_err(|error|error.to_string())?).map_err(|error|error.to_string())?; ui.upgrade_in_event_loop(move |ui| {if ui.get_detail_id()==id {set_completion(&ui,times.as_ref());ui.set_completion_summary(summary.into());if let Some(url)=url {ui.set_completion_url(url.into());}}}).map_err(|error|error.to_string())
                        })(),
                        NetworkResult::Notice(result) => result.map(|notice|set_notice(&ui,notice)),
                        NetworkResult::Search(id,result) => result.and_then(|candidates| {
                            if candidates.is_empty() {return Err("Uygun kapak bulunamadı.".into());}
                            let names: Vec<slint::SharedString> = candidates.iter().map(|candidate|candidate.title.clone().into()).collect();
                            let tiles: Vec<CoverTile> = candidates.into_iter().map(|candidate|CoverTile {title:candidate.title.into(),url:candidate.url.into()}).collect();
                            ui.upgrade_in_event_loop(move |ui|{if ui.get_detail_id() != id || ui.get_section() != 9 {return;} ui.set_cover_candidates(Rc::new(VecModel::from(tiles)).into());ui.set_cover_names(Rc::new(VecModel::from(names)).into());ui.set_cover_index(0);}).map_err(|error|error.to_string())
                        }),
                        NetworkResult::Cover(id,result) => result.and_then(|cover| {
                            store.set_artwork_override(&id,Some(&cover.to_string_lossy())).map_err(|error|error.to_string())?;
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
                Command::Detail(id) => (|| {
                    detail(&store, &id, &ui)?;
                    let game = store.game(&id).map_err(|error| error.to_string())?;
                    let cached = store.completion(&game).map_err(|error| error.to_string())?;
                    if cached.is_none()
                        && !completion_attempts.contains(&id)
                        && !network.busy.load(Ordering::SeqCst)
                    {
                        submit(
                            &network,
                            &ui,
                            NetworkTask::Completion(id.clone(), game, String::new()),
                        )?;
                        completion_attempts.insert(id);
                    }
                    Ok(())
                })(),
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
                    store
                        .set_details(&id, &title, &notes)
                        .map_err(|error| error.to_string())?;
                    set_notice(&ui, "Oyun bilgileri kaydedildi.".into());
                    publish(&store, &query, &state, &ui)?;
                    detail(&store, &id, &ui)
                })(),
                Command::Cover(id, reset) => (|| {
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
                        .set_artwork_override(&id, artwork.as_deref())
                        .map_err(|error| error.to_string())?;
                    publish(&store, &query, &state, &ui)?;
                    detail_impl(&store, &id, &ui, true)
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
                    social_view = None;
                    store
                        .set_setting("bridge_enabled", if enabled { "true" } else { "false" })
                        .map_err(|error| error.to_string())?;
                    if enabled && bridge.is_none() {
                        bridge = Some(crate::bridge::Bridge::start(&data_dir)?);
                    }
                    if !enabled {
                        bridge = None;
                    }
                    bridge_status(
                        if enabled { bridge.as_ref() } else { None },
                        &store,
                        &social_query,
                        &ui,
                    )
                })(),
                Command::BridgeRefresh => (|| {
                    let token = bridge.as_ref().map(|bridge| bridge.view_token());
                    let minute = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|time| time.as_secs() / 60)
                        .unwrap_or_default();
                    let next_view = (token, minute);
                    if social_view == Some(next_view) {
                        return Ok(());
                    }
                    if let Some(bridge) = &bridge {
                        let snapshot = bridge.snapshot();
                        let desired = store
                            .setting("auto_update_vencord")
                            .map_err(|error| error.to_string())?
                            .as_deref()
                            != Some("false");
                        if snapshot["connected"] == true
                            && snapshot["capabilities"]["auto_update"] == true
                            && snapshot["autoUpdate"] != desired
                        {
                            bridge
                                .command("auto_update", serde_json::json!({"enabled":desired}))?;
                        }
                        if snapshot["connected"] == true
                            && snapshot["capabilities"]["favorite_friends"] == true
                        {
                            if let Some(account) = snapshot["accountId"].as_str() {
                                let favorites = store
                                    .friend_favorites(account)
                                    .map_err(|error| error.to_string())?;
                                if snapshot["favoriteIds"] != serde_json::json!(favorites) {
                                    bridge.command(
                                        "favorite_friends",
                                        serde_json::json!({"ids":favorites}),
                                    )?;
                                }
                            }
                        }
                        if snapshot["connected"] == true
                            && snapshot["capabilities"]["room_catalog"] == true
                            && social_query.room_mode == 2
                            && snapshot["roomCatalogQuery"].as_str()
                                != Some(social_query.room_query.as_str())
                        {
                            bridge.command(
                                "room_catalog",
                                serde_json::json!({"query":social_query.room_query}),
                            )?;
                        }
                        if snapshot["connected"] == true
                            && snapshot["capabilities"]["favorite_rooms"] == true
                        {
                            if let Some(account) = snapshot["accountId"].as_str() {
                                let rooms = store
                                    .room_favorites(account)
                                    .map_err(|error| error.to_string())?;
                                if snapshot["favoriteRoomIds"] != serde_json::json!(rooms) {
                                    bridge.command(
                                        "favorite_rooms",
                                        serde_json::json!({"ids":rooms}),
                                    )?;
                                }
                            }
                        }
                    }
                    bridge_status(bridge.as_ref(), &store, &social_query, &ui)?;
                    social_view = Some(next_view);
                    Ok(())
                })(),
                Command::SocialFilter(text, favorites_only) => {
                    social_view = None;
                    social_query.text = text.chars().take(256).collect();
                    social_query.favorites_only = favorites_only;
                    bridge_status(bridge.as_ref(), &store, &social_query, &ui)
                }
                Command::FriendFavorite(id, favorite) => (|| {
                    social_view = None;
                    let snapshot = bridge
                        .as_ref()
                        .ok_or("Discord bağlantısını aç.")?
                        .snapshot();
                    if snapshot["connected"] != true
                        || !snapshot["friends"]
                            .as_array()
                            .is_some_and(|friends| friends.iter().any(|friend| friend["id"] == id))
                    {
                        return Err("Arkadaş artık bağlı Discord listesinde değil".into());
                    }
                    let account = snapshot["accountId"]
                        .as_str()
                        .ok_or("Arkadaş favorileri için köprüyü güncelle.")?;
                    store
                        .set_friend_favorite(account, &id, favorite)
                        .map_err(|error| error.to_string())?;
                    bridge_status(bridge.as_ref(), &store, &social_query, &ui)
                })(),
                Command::RoomFilter(mode, text) => (|| {
                    social_view = None;
                    social_query.room_mode = mode.clamp(0, 2);
                    social_query.room_query = text.chars().take(128).collect();
                    if let Some(bridge) = bridge.as_ref() {
                        if bridge.snapshot()["capabilities"]["room_catalog"] == true {
                            bridge.command("room_catalog", serde_json::json!({"query":if social_query.room_mode == 2 {Some(&social_query.room_query)} else {None}}))?;
                        }
                    }
                    bridge_status(bridge.as_ref(), &store, &social_query, &ui)
                })(),
                Command::RoomFavorite(id, favorite) => (|| {
                    social_view = None;
                    let bridge = bridge.as_ref().ok_or("Discord bağlantısını aç.")?;
                    let snapshot = bridge.snapshot();
                    if snapshot["connected"] != true
                        || snapshot["capabilities"]["favorite_rooms"] != true
                    {
                        return Err("Oda takibi için köprüyü güncelle.".into());
                    }
                    let account = snapshot["accountId"]
                        .as_str()
                        .ok_or("Discord hesabı okunamadı")?;
                    let known = ["rooms", "roomCatalog"].iter().any(|key| {
                        snapshot[*key].as_array().is_some_and(|rows| {
                            rows.iter()
                                .any(|room| room["id"] == id && room["available"] != false)
                        })
                    });
                    if favorite && !known {
                        return Err("Oda artık erişilemiyor".into());
                    }
                    store
                        .set_room_favorite(account, &id, favorite)
                        .map_err(|error| error.to_string())?;
                    bridge_status(Some(bridge), &store, &social_query, &ui)
                })(),
                Command::StreamControl(action, key, index) => (|| {
                    let bridge = bridge.as_ref().ok_or("Discord bağlantısını aç.")?;
                    let data = crate::stream::request(&bridge.snapshot(), &action, &key, index)?;
                    bridge.command(&action, data)
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
                    .and_then(|_| {
                        let detail_id = id.clone();
                        ui.upgrade_in_event_loop(move |ui| {
                            if ui.get_detail_id() == detail_id {
                                ui.set_detail_favorite(value);
                            }
                        })
                        .map_err(|error| error.to_string())?;
                        publish(&store, &query, &state, &ui)
                    }),
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
                        let _ =
                            ui.upgrade_in_event_loop(move |ui| ui.set_scan_notice(message.into()));
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
