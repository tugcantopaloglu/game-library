use crate::{
    ChatMessageTile, CollectionTile, CoverTile, GameTile, LibraryWindow, MusicTile, RoomTile,
    SocialTile,
};
use library_core::models::{Game, LibraryStatus, Source};
use library_core::{launcher, sources, storage::Store};
use platform_windows::shell::open_target;
use slint::{Model, ModelRc, SharedPixelBuffer, VecModel};
use crate::covers::thumbnail;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

fn reconcile_model<T: Clone + PartialEq + 'static>(model: ModelRc<T>, rows: Vec<T>) -> ModelRc<T> {
    if let Some(current) = model.as_any().downcast_ref::<VecModel<T>>() {
        if current.row_count() == rows.len() {
            for (index, row) in rows.into_iter().enumerate() {
                if current.row_data(index).as_ref() != Some(&row) {
                    current.set_row_data(index, row);
                }
            }
        } else {
            current.set_vec(rows);
        }
        model
    } else {
        Rc::new(VecModel::from(rows)).into()
    }
}

pub struct CoverRequest {
    game: Game,
    url: Option<String>,
}
type CoverQueue = mpsc::SyncSender<CoverRequest>;
pub struct State {
    pub scanning: AtomicBool,
    pub cancel_scan: AtomicBool,
    pub revision: AtomicU64,
    pub chat_open: std::sync::Mutex<Option<crate::chat::OpenAttempt>>,
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
    CoverFailed(String),
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
    Chat(String, String, String, String),
    SocialFilter(String, bool),
    FriendFavorite(String, bool),
    RoomFilter(i32, String),
    RoomFavorite(String, bool),
    StreamControl(String, String, i32),
    StreamCatalog(bool),
    StreamSelection(String, String, String, i32, i32, bool),
    Installer(crate::updates::InstallerAction, String),
    PrepareBridge,
    UpdateCheck,
    UpdateStage,
    UpdateApply,
    UpdatePreferences(bool),
    Session(i64, String, Option<u32>, bool),
    ArtworkKey(String),
    SearchCovers(String),
    SteamCredentials(String, String),
    SteamLogin,
    SteamKeyPage,
    SyncAccount(Source, String, String),
    EpicLogin,
    AvatarReady,
    AvatarFallback(String),
    AvatarData(String, Option<String>),
    FriendsPanel(bool),
    Extension(i32, String),
    ObsConnect(String, String),
    ObsAction(i32, String),
    BroadcastInspect,
    BroadcastOpen(i32),
    NetworkAction(i32, i32, bool),
    NetworkAutoUpdate(bool),
    SpotifyOpen,
    SpotifyTrack(String, String),
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
    ExtensionRun(String),
    SteamApi(String),
    SteamLogin,
    Libraries(String),
    Obs(u16, Option<crate::obs::Action>),
    Broadcast,
    NetworkTools(NetworkToolTask),
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
enum NetworkToolTask {
    Inspect,
    Prepare,
    Start,
    Stop,
    Dns(String, bool),
    Restore,
    AutoUpdate(bool),
    BackgroundUpdate,
}
pub enum NetworkResult {
    Extension(Result<String, String>),
    Libraries(Vec<(Source, Result<crate::accounts::EpicLibrary, String>)>),
    Obs(Result<crate::obs::ActionResult, String>),
    Broadcast(crate::broadcast::Report),
    NetworkTools(crate::network_tools::Report, Result<String, String>),
    Cover(String, Result<PathBuf, String>),
    Search(String, Result<Vec<crate::media::CoverCandidate>, String>),
    Notice(Result<String, String>),
    Check(Result<crate::updates::UpdateManifest, String>),
    Stage(Result<PathBuf, String>),
    Owned(
        Source,
        Result<crate::accounts::EpicLibrary, String>,
        Option<String>,
    ),
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

fn publish_extensions(
    data: &std::path::Path,
    ui: &slint::Weak<LibraryWindow>,
) -> Result<(), String> {
    let mut errors = vec![];
    let extensions = crate::extensions::list(data)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(extension) => Some(crate::ExtensionTile {
                id: extension.manifest.id.into(),
                name: extension.manifest.name.into(),
                version: extension.manifest.version.into(),
                enabled: extension.preferences.enabled,
                details: format!(
                    "{} · İzinler: {} · {} klasör",
                    extension
                        .manifest
                        .capabilities
                        .iter()
                        .map(|capability| if capability == "games.discover" {
                            "Oyun keşfi"
                        } else {
                            "Kapak sağlayıcısı"
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    extension
                        .manifest
                        .permissions
                        .iter()
                        .map(|permission| if permission == "filesystem.read" {
                            "Seçili klasörleri okuma"
                        } else {
                            "Ağ erişimi"
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    extension.preferences.roots.len()
                )
                .into(),
            }),
            Err(error) => {
                errors.push(error);
                None
            }
        })
        .collect::<Vec<_>>();
    ui.upgrade_in_event_loop(move |ui| {
        ui.set_extensions(Rc::new(VecModel::from(extensions)).into());
        if !errors.is_empty() {
            ui.set_extension_status(errors.join("\n").into());
        }
    })
    .map_err(|error| error.to_string())
}

fn request_cover(store: &Store, state: &State, game: &Game) {
    if game.source == Source::Manual { return; }
    let url = store.artwork_url(game).ok().flatten().or_else(|| (game.source == Source::Steam).then(|| format!("https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/{}/library_600x900.jpg",game.app_id)));
    if let (Ok(queue), Ok(mut attempted)) = (state.cover_requests.lock(), state.cover_attempts.lock()) {
        if !attempted.contains(&game.id) && queue.as_ref().is_some_and(|queue| queue.try_send(CoverRequest { game: game.clone(), url }).is_ok()) {
            attempted.insert(game.id.clone());
        }
    }
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
        let local_artwork = game.artwork_path.as_ref().is_some_and(|path| std::path::Path::new(path).is_file());
        if !local_artwork && game.source != Source::Manual {
            request_cover(store, state, &game);
        }
        if game.artwork_path.is_some() && !local_artwork {
            unavailable_artwork += 1;
        }
        tiles.push(game);
    }
    let revision = query.revision;
    let matched = page.total;
    let columns = query.columns.clamp(3, 12);
    let state = state.clone();
    ui.upgrade_in_event_loop(move |ui| {
        if state.revision.load(Ordering::Relaxed) != revision {
            return;
        }
        let mut entries: Vec<GameTile> = tiles
            .into_iter()
            .map(|game| {
                GameTile {
                    id: game.id.into(),
                    name: game.name.into(),
                    size: size_text(game.size_bytes).into(),
                    installed: game.installed,
                    favorite: game.favorite,
                    artwork: Default::default(),
                    has_artwork: false,
                    artwork_loaded: false,
                    artwork_path: game.artwork_path.unwrap_or_default().into(),
                    source: game.source.label().into(),
                    status_label: if game.installed {
                        "Kurulu"
                    } else if game.install_path.is_empty() {
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
        ui.invoke_cover_viewport();
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
    avatar_cache: &crate::avatars::Cache,
    state: &Arc<State>,
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
    let followed = crate::social::friends(
        &snapshot,
        &favorites,
        &crate::social::Query {
            favorites_only: true,
            ..Default::default()
        },
        now_ms,
    );
    for friend in friends.iter().chain(followed.iter()) {
        avatar_cache.request(
            &friend.avatar_url,
            snapshot["capabilities"]["avatar_request"] == true,
        );
    }
    let avatar_cache = avatar_cache.clone();
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
    let state = state.clone();
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
        let chat = crate::chat::view(&snapshot);
        let selected = ui.get_chat_user_id().as_str() == chat.user_id;
        let panel = crate::chat::panel_state(&snapshot, ui.get_chat_user_id().as_str(), &mut state.chat_open.lock().unwrap(), now_ms);
        ui.set_chat_status(panel.status.into());
        ui.set_chat_retry(panel.retry);
        if selected && !chat.name.is_empty() {
            ui.set_chat_name(chat.name.into());
        }
        ui.set_chat_loading(ui.get_chat_active() && panel.loading);
        ui.set_chat_pending(selected && chat.pending);
        ui.set_chat_can_send(selected && chat.can_send);
        ui.set_chat_can_open_discord(
            connected
                && snapshot["capabilities"]["open_dm"] == true
                && snapshot["capabilities"]["chat_open"] == true,
        );
        ui.set_chat_channel_id(if selected {
            chat.channel_id.into()
        } else {
            "".into()
        });
        if selected
            && !chat.sent_nonce.is_empty()
            && ui.get_chat_sent_nonce().as_str() != chat.sent_nonce
        {
            ui.set_chat_sent_nonce(chat.sent_nonce.into());
            ui.set_chat_draft("".into());
        }
        let messages = if selected { chat.messages } else { Vec::new() };
        ui.set_chat_messages(reconcile_model(
            ui.get_chat_messages(),
            messages
                .into_iter()
                .map(|message| ChatMessageTile {
                    id: message.id.into(),
                    author: message.author.into(),
                    content: message.content.into(),
                    time: message.time.into(),
                    own: message.own,
                })
                .collect::<Vec<_>>(),
        ));
        ui.set_can_broadcast_audio(connected && snapshot["broadcastAudio"]["canSelect"] == true);
        ui.set_broadcast_discord_status(
            format!(
                "{} {}",
                snapshot["broadcastAudio"]["status"]
                    .as_str()
                    .unwrap_or("Güncel Broadcast köprüsü bekleniyor."),
                snapshot["broadcastAudio"]["result"].as_str().unwrap_or("")
            )
            .into(),
        );
        let music = &snapshot["spotify"];
        ui.set_spotify_status(
            if connected {
                music["status"]
                    .as_str()
                    .unwrap_or("Güncel Spotify köprüsü bekleniyor")
            } else {
                "Spotify etkinliği için Discord bağlantısını aç."
            }
            .into(),
        );
        if connected
            && music["current"]["id"]
                .as_str()
                .is_some_and(crate::spotify::valid_track_id)
        {
            ui.set_spotify_track(
                format!(
                    "{} · {}",
                    music["current"]["title"].as_str().unwrap_or(""),
                    music["current"]["artist"].as_str().unwrap_or("")
                )
                .into(),
            );
            ui.set_spotify_track_id(music["current"]["id"].as_str().unwrap_or("").into());
        } else {
            ui.set_spotify_track("".into());
            ui.set_spotify_track_id("".into());
        }
        let music_friends = if connected {
            music["friends"].as_array()
        } else {
            None
        }
        .into_iter()
        .flatten()
        .take(200)
        .filter_map(|row| {
            let id = row["userId"].as_str()?;
            let track = row["track"]["id"]
                .as_str()
                .filter(|id| crate::spotify::valid_track_id(id))?;
            let name = snapshot["friends"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|friend| friend["id"] == id)
                .and_then(|friend| friend["name"].as_str())
                .unwrap_or("Spotify arkadaşı");
            Some(MusicTile {
                user_id: id.into(),
                name: name.into(),
                title: row["track"]["title"].as_str().unwrap_or("").into(),
                artist: row["track"]["artist"].as_str().unwrap_or("").into(),
                track_id: track.into(),
                can_profile: row["canOpenProfile"] == true,
                can_join: row["canListenAlong"] == true,
            })
        })
        .collect::<Vec<_>>();
        ui.set_music_friends(Rc::new(VecModel::from(music_friends)).into());
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
        ui.set_stream_result(if connected && snapshot["bridgeBuild"] != "20261014.2" {
            "Yayın kontrolleri için hazırlanan güncelleme bu Discord oturumunda henüz etkin değil.".into()
        } else { stream.result.into() });
        ui.set_can_stream_start(
            connected && snapshot["capabilities"]["stream_start"] == true && !stream.pending,
        );
        ui.set_can_stream_sources(
            connected && snapshot["capabilities"]["stream_sources"] == true && !stream.pending,
        );
        if ui.get_stream_source_revision() != stream.source_revision
            || ui.get_stream_source_index() >= stream.sources.len() as i32
            || stream.sources.is_empty()
        {
            ui.set_stream_source_index(-1);
        }
        ui.set_stream_source_revision(stream.source_revision);
        ui.set_can_stream_source(
            connected
                && snapshot["capabilities"]["stream_source"] == true
                && stream.active
                && !stream.pending,
        );
        ui.set_stream_source_channel(stream.source_channel.into());
        ui.set_stream_source_pending(stream.source_pending);
        ui.set_stream_source_audio_capabilities(reconcile_model(
            ui.get_stream_source_audio_capabilities(),
            stream
                .sources
                .iter()
                .map(|source| source.audio_available)
                .collect::<Vec<_>>(),
        ));
        ui.set_can_stream_audio(
            connected
                && snapshot["capabilities"]["stream_audio"] == true
                && stream.active
                && !stream.pending,
        );
        ui.set_stream_audio_enabled(stream.audio == Some(true));
        ui.set_stream_source_error(stream.source_error.into());
        ui.set_stream_source_names(reconcile_model(
            ui.get_stream_source_names(),
            stream
                .sources
                .iter()
                .map(|source| {
                    slint::SharedString::from(format!(
                        "{} · {}",
                        if source.kind == "screen" {
                            "Ekran"
                        } else {
                            "Pencere"
                        },
                        source.name
                    ))
                })
                .collect::<Vec<_>>(),
        ));
        if !connected || stream.pending {
            ui.set_stream_source_picker(false);
        }
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
        ui.set_stream_resolution_values(reconcile_model(ui.get_stream_resolution_values(), stream.resolutions.iter().map(|value| *value as i32).collect()));
        ui.set_stream_fps_values(reconcile_model(ui.get_stream_fps_values(), stream.fps_options.iter().map(|value| *value as i32).collect()));
        ui.set_stream_resolutions(reconcile_model(
            ui.get_stream_resolutions(),
            stream
                .resolutions
                .iter()
                .map(|value| {
                    slint::SharedString::from(if *value == 0 {
                        "Kaynak".into()
                    } else {
                        format!("{value}p")
                    })
                })
                .collect::<Vec<_>>(),
        ));
        ui.set_stream_fps_options(reconcile_model(
            ui.get_stream_fps_options(),
            stream
                .fps_options
                .iter()
                .map(|value| slint::SharedString::from(format!("{value} FPS")))
                .collect::<Vec<_>>(),
        ));
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
                    avatar: avatar_cache.image(&friend.avatar_url).unwrap_or_default(),
                    has_avatar: avatar_cache.image(&friend.avatar_url).is_some(),
                    presence: status.into(),
                    voice: friend.voice_channel.as_str().into(),
                    game: friend.activity.as_str().into(),
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
        let followed: Vec<SocialTile> = followed
            .iter()
            .map(|friend| {
                let avatar = avatar_cache.image(&friend.avatar_url);
                let presence = match friend.status.as_str() {
                    "online" => "Çevrimiçi",
                    "idle" => "Boşta",
                    "dnd" => "Rahatsız etmeyin",
                    "offline" => "Çevrimdışı",
                    _ => "Durum bilinmiyor",
                };
                SocialTile {
                    id: friend.id.as_str().into(),
                    name: friend.name.as_str().into(),
                    initial: friend.name.chars().next().unwrap_or('?').to_string().into(),
                    subtitle: if friend.activity.is_empty() {
                        presence.into()
                    } else {
                        format!("{presence} · Oynuyor: {}", friend.activity).into()
                    },
                    voice: friend.voice_channel.as_str().into(),
                    game: friend.activity.as_str().into(),
                    presence: friend.status.as_str().into(),
                    online: matches!(friend.status.as_str(), "online" | "idle" | "dnd"),
                    favorite: true,
                    recent: "".into(),
                    has_avatar: avatar.is_some(),
                    avatar: avatar.unwrap_or_default(),
                }
            })
            .collect();
        if connected {
            ui.set_followed_friends(reconcile_model(ui.get_followed_friends(), followed));
        }
        ui.set_friends(reconcile_model(ui.get_friends(), friends));
        ui.set_rooms(reconcile_model(ui.get_rooms(), rooms));
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
    let avatars = crate::avatars::Cache::start(&data_dir, sender.clone());
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
                NetworkTask::ExtensionRun(id) => NetworkResult::Extension((|| {
                    let mut store = Store::open(&network_data.join("library.db"))
                        .map_err(|error| error.to_string())?;
                    crate::extensions::run(&network_data, &id, &mut store)
                })()),
                NetworkTask::SteamApi(id) => NetworkResult::Owned(
                    Source::Steam,
                    crate::accounts::steam_api(&id).map(|games| (games, vec![])),
                    None,
                ),
                NetworkTask::SteamLogin => {
                    let result =
                        crate::steam_web::library(&network_data, true).map(|games| (games, vec![]));
                    let account = if result.is_ok() {
                        crate::steam_web::account_id(&network_data)
                    } else {
                        None
                    };
                    NetworkResult::Owned(Source::Steam, result, account)
                }
                NetworkTask::Libraries(id) => NetworkResult::Libraries(vec![
                    (
                        Source::Steam,
                        crate::accounts::steam(&network_data, &id).map(|games| (games, vec![])),
                    ),
                    (Source::Epic, crate::accounts::epic(&network_data)),
                ]),
                NetworkTask::Obs(port, action) => NetworkResult::Obs((|| {
                    let mut password = platform_windows::credentials::obs_password(port)?;
                    let result = match action {
                        Some(action) => crate::obs::perform(port, password.as_deref(), action),
                        None => crate::obs::snapshot(port, password.as_deref()).map(|snapshot| {
                            crate::obs::ActionResult {
                                snapshot,
                                verified: true,
                                message: "OBS bağlantısı doğrulandı.".into(),
                            }
                        }),
                    };
                    if let Some(secret) = &mut password {
                        unsafe {
                            secret.as_bytes_mut().fill(0);
                        }
                    }
                    result
                })()),
                NetworkTask::Broadcast => NetworkResult::Broadcast(crate::broadcast::inspect()),
                NetworkTask::NetworkTools(task) => {
                    let message = (|| match task {
                        NetworkToolTask::Inspect => Ok(String::new()),
                        NetworkToolTask::Prepare => {
                            crate::network_tools::prepare_latest(&network_data).map(|package| {
                                format!("GoodbyeDPI {} hazırlandı.", package.release.version)
                            })
                        }
                        NetworkToolTask::Start => {
                            if !network_data.join("network-tools/prepared.json").is_file() {
                                crate::network_tools::prepare_latest(&network_data)?;
                            }
                            crate::network_tools::start(&network_data).map(|value| {
                                value["message"]
                                    .as_str()
                                    .unwrap_or("GoodbyeDPI başlatıldı.")
                                    .to_owned()
                            })
                        }
                        NetworkToolTask::Stop => {
                            crate::network_tools::stop(&network_data).map(|value| {
                                value["message"]
                                    .as_str()
                                    .unwrap_or("GoodbyeDPI durduruldu.")
                                    .to_owned()
                            })
                        }
                        NetworkToolTask::Dns(guid, encrypted) => {
                            crate::network_tools::enable_dns(&network_data, &guid, encrypted).map(
                                |value| {
                                    value["message"]
                                        .as_str()
                                        .unwrap_or("Cloudflare DNS uygulandı.")
                                        .to_owned()
                                },
                            )
                        }
                        NetworkToolTask::Restore => {
                            crate::network_tools::restore_dns(&network_data).map(|value| {
                                value["message"]
                                    .as_str()
                                    .unwrap_or("Önceki DNS ayarları geri yüklendi.")
                                    .to_owned()
                            })
                        }
                        NetworkToolTask::AutoUpdate(enabled) => {
                            crate::network_tools::set_auto_update(&network_data, enabled)?;
                            if enabled {
                                crate::network_tools::auto_update_if_due(&network_data)?;
                            }
                            Ok(String::new())
                        }
                        NetworkToolTask::BackgroundUpdate => {
                            crate::network_tools::auto_update_if_due(&network_data).map(|updated| {
                                if updated {
                                    "Yeni GoodbyeDPI paketi hazır. Sonraki başlatmada kullanılacak."
                                        .into()
                                } else {
                                    String::new()
                                }
                            })
                        }
                    })();
                    NetworkResult::NetworkTools(
                        crate::network_tools::inspect(&network_data),
                        message,
                    )
                }
                NetworkTask::Cover(id, url) => NetworkResult::Cover(
                    id,
                    crate::media::remote_cover(
                        &network_data,
                        &url,
                        crate::media::CoverUse::Selected,
                    ),
                ),
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
                NetworkTask::Owned(source, id) => NetworkResult::Owned(
                    source,
                    match source {
                        Source::Steam => {
                            crate::accounts::steam(&network_data, &id).map(|games| (games, vec![]))
                        }
                        Source::Epic => crate::accounts::epic(&network_data),
                        _ => Err("Hesap kaynağı desteklenmiyor".into()),
                    },
                    None,
                ),
                NetworkTask::EpicLogin => NetworkResult::Owned(
                    Source::Epic,
                    crate::accounts::login_epic(&network_data),
                    None,
                ),
                NetworkTask::Completion(id, game, url) => {
                    NetworkResult::Completion(id, crate::completion::lookup(&game, &url))
                }
            };
            busy.store(false, Ordering::SeqCst);
            if network_sender.send(Command::NetworkDone(result)).is_err() {
                break;
            }
        }
    });
    let event_sender = sender.clone();
    std::thread::spawn(move || {
        let mut store = store;
        let _ = publish_extensions(&data_dir, &ui);
        let _ = store.recover_sessions();
        let mut bridge: Option<crate::bridge::Bridge> = None;
        let mut available_update = None;
        let mut staged_update = None;
        let mut auto_update_app =
            store.setting("auto_update_app").ok().flatten().as_deref() == Some("true");
        let mut next_update = std::time::Instant::now();
        let mut next_network_check = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut network_interfaces: Vec<String> = Vec::new();
        let mut query = Query {
            filter: "all".into(),
            ..Query::default()
        };
        let mut completion_attempts = std::collections::HashSet::new();
        let mut social_query = crate::social::Query::default();
        let mut social_view = None;
        loop {
            if std::time::Instant::now() >= next_network_check
                && !network.busy.load(Ordering::SeqCst)
            {
                next_network_check =
                    std::time::Instant::now() + std::time::Duration::from_secs(3600);
                let _ = submit(
                    &network,
                    &ui,
                    NetworkTask::NetworkTools(NetworkToolTask::BackgroundUpdate),
                );
            }
            let command = if auto_update_app && std::time::Instant::now() >= next_update {
                next_update =
                    std::time::Instant::now() + std::time::Duration::from_secs(24 * 60 * 60);
                Command::UpdateCheck
            } else {
                match receiver.recv_timeout(std::time::Duration::from_secs(30)) {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                }
            };
            let result = match command {
                Command::CoverFailed(id) => store.game(&id).map(|game| request_cover(&store, &state, &game)).map_err(|error| error.to_string()),
                Command::FriendsPanel(enabled)=>store.set_setting("friends_panel_enabled",if enabled {"true"}else{"false"}).map_err(|error|error.to_string()),
                Command::AvatarReady=> {social_view=None;let _=ui.upgrade_in_event_loop(|ui|ui.set_avatar_status("".into()));bridge_status(bridge.as_ref(),&store,&social_query,&avatars,&state,&ui)},
                Command::AvatarFallback(url)=> {
                    let _=ui.upgrade_in_event_loop(|ui|ui.set_avatar_status("Fotoğraflar Discord bağlantısından yükleniyor…".into()));
                    bridge.as_ref().ok_or("Fotoğraflar için Discord bağlantısını aç.").and_then(|bridge|bridge.command("avatar_request",serde_json::json!({"url":url})).map_err(|_|"Fotoğraflar için Discord köprüsünü güncelle.")).map_err(str::to_owned)
                },
                Command::AvatarData(url,encoded)=> {
                    if let Some(encoded)=encoded {avatars.deliver(url,&encoded);}else{let _=ui.upgrade_in_event_loop(|ui|ui.set_avatar_status("Fotoğraflar yüklenemedi. Discord bağlantısını ve ağ erişimini kontrol et.".into()));}
                    Ok(())
                },
                Command::Extension(action,id) => (|| {
                    match action {
                        0=>{},
                        1=>{if let Some(folder)=rfd::FileDialog::new().set_title("extension.json içeren eklenti paketini seç").pick_folder() {let installed=crate::extensions::install(&data_dir,&folder)?;set_notice(&ui,format!("{installed} kuruldu. Çalıştırmak için etkinleştir."));}},
                        2=>crate::extensions::enable(&data_dir,&id,true)?,
                        3=>crate::extensions::enable(&data_dir,&id,false)?,
                        4=>return submit(&network,&ui,NetworkTask::ExtensionRun(id)),
                        5=>{if let Some(folder)=rfd::FileDialog::new().set_title("Eklentinin okuyacağı oyun klasörünü seç").pick_folder() {crate::extensions::add_root(&data_dir,&id,&folder)?;}},
                        6=>crate::extensions::remove(&data_dir,&id)?,
                        7=>{let folder=data_dir.join(format!("extension-example-{}",std::process::id()));crate::extensions::create_example(&folder)?;let result=crate::extensions::install(&data_dir,&folder);let _=std::fs::remove_dir_all(&folder);result?;},
                        _=>return Err("Eklenti işlemi geçersiz".into()),
                    }
                    publish_extensions(&data_dir,&ui)
                })(),
                Command::ObsConnect(port, password) => (|| {
                    let port = port
                        .trim()
                        .parse::<u16>()
                        .ok()
                        .filter(|port| *port > 0)
                        .ok_or("OBS portu 1–65535 arasında olmalı")?;
                    if !password.is_empty() {
                        platform_windows::credentials::save_obs_password(port, &password)?;
                    }
                    store
                        .set_setting("obs_port", &port.to_string())
                        .map_err(|error| error.to_string())?;
                    submit(&network, &ui, NetworkTask::Obs(port, None))
                })(),
                Command::ObsAction(action, scene) => (|| {
                    let port = store
                        .setting("obs_port")
                        .map_err(|error| error.to_string())?
                        .and_then(|port| port.parse().ok())
                        .unwrap_or(crate::obs::DEFAULT_PORT);
                    let action = match action {
                        0 => crate::obs::Action::SetScene(scene),
                        1 => crate::obs::Action::StartStream,
                        2 => crate::obs::Action::StopStream,
                        3 => crate::obs::Action::StartRecording,
                        4 => crate::obs::Action::StopRecording,
                        _ => return Err("OBS işlemi geçersiz".into()),
                    };
                    submit(&network, &ui, NetworkTask::Obs(port, Some(action)))
                })(),
                Command::BroadcastInspect => submit(&network, &ui, NetworkTask::Broadcast),
                Command::NetworkAutoUpdate(enabled) => submit(
                    &network,
                    &ui,
                    NetworkTask::NetworkTools(NetworkToolTask::AutoUpdate(enabled)),
                ),
                Command::NetworkAction(action, index, encrypted) => (|| {
                    let task = match action {
                        0 => NetworkToolTask::Inspect,
                        1 => NetworkToolTask::Prepare,
                        2 => NetworkToolTask::Start,
                        3 => NetworkToolTask::Stop,
                        4 => NetworkToolTask::Dns(
                            network_interfaces
                                .get(
                                    usize::try_from(index)
                                        .map_err(|_| "Bir ağ bağdaştırıcısı seç")?,
                                )
                                .cloned()
                                .ok_or("Bir ağ bağdaştırıcısı seç")?,
                            encrypted,
                        ),
                        5 => NetworkToolTask::Restore,
                        6 => {
                            return open_target(crate::network_tools::CLOUDFLARE_DOCS)
                                .map_err(|error| error.to_string())
                        }
                        7 => {
                            return open_target(crate::network_tools::RELEASES_URL)
                                .map_err(|error| error.to_string())
                        }
                        _ => return Err("Ağ aracı işlemi geçersiz".into()),
                    };
                    submit(&network, &ui, NetworkTask::NetworkTools(task))
                })(),
                Command::BroadcastOpen(action) => match action {
                    0 => crate::broadcast::open(),
                    1 => crate::broadcast::open_download(),
                    2 => crate::broadcast::open_setup(),
                    _ => Err("Broadcast işlemi geçersiz".into()),
                }
                .map(|message| set_notice(&ui, message)),
                Command::SpotifyOpen => crate::spotify::open_app(),
                Command::SpotifyTrack(user, track) => (|| {
                    let snapshot = bridge
                        .as_ref()
                        .ok_or("Discord bağlantısını aç.")?
                        .snapshot();
                    if snapshot["connected"] != true {
                        return Err("Discord bağlantısı güncel değil".into());
                    }
                    let selected = if user.is_empty() {
                        snapshot["spotify"]["current"]["id"].as_str()
                    } else {
                        snapshot["spotify"]["friends"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|row| row["userId"] == user)
                            .and_then(|row| row["track"]["id"].as_str())
                    };
                    if selected != Some(track.as_str()) {
                        return Err("Spotify parçası değişti; güncel parçayı seç".into());
                    }
                    crate::spotify::open_track(&track)
                })(),
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
                    let updated = ui.upgrade_in_event_loop({
                        let id = id.clone();
                        let path = path.to_string_lossy().to_string();
                        move |ui| {
                            for row in ui.get_game_rows().iter() {
                                for column in 0..row.row_count() {
                                    if let Some(mut tile) = row.row_data(column) {
                                        if tile.id == id {
                                            tile.artwork_path = path.clone().into();
                                            tile.artwork = Default::default();
                                            tile.has_artwork = false;
                                            tile.artwork_loaded = false;
                                            row.set_row_data(column, tile);
                                        }
                                    }
                                }
                            }
                            ui.invoke_cover_viewport();
                            if ui.get_section() == 9 && ui.get_detail_id() == id { ui.invoke_detail(id.into()); }
                        }
                    }).map_err(|error| error.to_string());
                    // Detail rendering is independent of the grid's viewport ownership.
                    updated
                })(),
                Command::SteamLogin => {
                    let _=ui.upgrade_in_event_loop(|ui|ui.set_steam_account_status("Steam giriş penceresinde hesabını doğrula; oyunların ardından otomatik alınacak.".into()));
                    submit(&network, &ui, NetworkTask::SteamLogin)
                }
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
                Command::SyncAccount(source, id, key) => (|| {
                    let api = source == Source::Steam && !id.trim().is_empty();
                    let id = if source == Source::Steam && !id.trim().is_empty() {
                        let id = id.trim();
                        if id.len() != 17 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
                            return Err("SteamID64 17 rakam olmalı".into());
                        }
                        if !key.trim().is_empty() {
                            platform_windows::credentials::save_steam_key(key.trim())?;
                        }
                        store
                            .set_setting("steam_account_id", id)
                            .map_err(|error| error.to_string())?;
                        id.to_owned()
                    } else {
                        store
                            .setting("steam_account_id")
                            .ok()
                            .flatten()
                            .or_else(library_core::steam::active_user_id)
                            .unwrap_or_default()
                    };
                    submit(
                        &network,
                        &ui,
                        if api {
                            NetworkTask::SteamApi(id)
                        } else {
                            NetworkTask::Owned(source, id)
                        },
                    )
                })(),
                Command::EpicLogin => {
                    let _=ui.upgrade_in_event_loop(|ui|ui.set_epic_account_status("Epic giriş penceresinde hesabını doğrula; oyunların ardından otomatik alınacak.".into()));
                    submit(&network, &ui, NetworkTask::EpicLogin)
                }
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
                        NetworkResult::Extension(result)=> {
                            let message=result.as_ref().cloned().unwrap_or_else(|error|error.clone());
                            let _=ui.upgrade_in_event_loop(move|ui|ui.set_extension_status(message.into()));
                            result.and_then(|_|publish(&store,&query,&state,&ui))
                        },
                        NetworkResult::Libraries(results) => {
                            let mut errors = Vec::new();
                            for (source,result) in results {
                                match result {
                                    Ok((mut games,covers)) => {
                                        if source==Source::Steam {let roots=library_core::steam::discover_roots(&store.roots().unwrap_or_default());for game in &mut games {game.artwork_path=library_core::steam::local_artwork(&roots,game.app_id);}}
                                        let apply=(|| {store.apply_owned_games(source,&games).map_err(|error|error.to_string())?;for (id,url) in covers {store.set_artwork_url(source,&id,&url).map_err(|error|error.to_string())?;}Ok::<_,String>(())})();
                                        let message=match apply {Ok(())=>format!("{} hesabından {} oyun eşitlendi.",source.label(),games.len()),Err(error)=>{errors.push(source.label());error}};
                                        let _=ui.upgrade_in_event_loop(move |ui|if source==Source::Steam {ui.set_steam_account_status(message.into());}else{ui.set_epic_account_status(message.into());});
                                    },
                                    Err(error) => {errors.push(source.label());let _=ui.upgrade_in_event_loop(move |ui|if source==Source::Steam {ui.set_steam_account_status(error.into());}else{ui.set_epic_account_status(error.into());});}
                                }
                            }
                            if !errors.is_empty() {set_notice(&ui,format!("{} hesapları eşitlenemedi. Ayarlar → Kaynaklar bölümünde bağlantıyı tamamla; önceki oyunlar korundu.",errors.join(" / ")));}
                            publish(&store,&query,&state,&ui)
                        },
                        NetworkResult::Obs(result) => {
                            ui.upgrade_in_event_loop(move |ui| match result {
                                Ok(result) => {
                                    let snapshot = result.snapshot;
                                    let index = snapshot.scenes.iter().position(|scene|scene==&snapshot.current_scene).unwrap_or(0) as i32;
                                    ui.set_obs_scenes(Rc::new(VecModel::from(snapshot.scenes.into_iter().map(slint::SharedString::from).collect::<Vec<_>>())).into());
                                    ui.set_obs_scene_index(index);
                                    ui.set_obs_connected(true);ui.set_obs_streaming(snapshot.streaming);ui.set_obs_recording(snapshot.recording);
                                    ui.set_obs_status(format!("{} {}OBS {} · Sahne: {} · Yayın: {} · Kayıt: {}",result.message,if result.verified {""}else{"Sonuç henüz doğrulanmadı. "},snapshot.obs_version,snapshot.current_scene,if snapshot.streaming {"açık"}else{"kapalı"},if snapshot.recording {"açık"}else{"kapalı"}).into());
                                },
                                Err(error) => {ui.set_obs_connected(false);ui.set_obs_status(error.into());ui.set_obs_scenes(Rc::new(VecModel::from(Vec::<slint::SharedString>::new())).into());}
                            }).map_err(|error|error.to_string())
                        },
                        NetworkResult::Broadcast(report) => {
                            let summary=report.summary();
                            let audio=report.device(crate::broadcast::DeviceKind::Microphone).is_some() && report.device(crate::broadcast::DeviceKind::Speaker).is_some();
                            ui.upgrade_in_event_loop(move |ui| {ui.set_broadcast_status(summary.into());ui.set_broadcast_installed(report.installed);ui.set_broadcast_rtx(report.rtx_detected);ui.set_broadcast_audio_available(audio);}).map_err(|error|error.to_string())
                        },
                        NetworkResult::NetworkTools(report,result) => {
                            let interfaces=report.runtime["interfaces"].as_array().cloned().unwrap_or_default();
                            network_interfaces=interfaces.iter().filter_map(|interface|interface["guid"].as_str().map(str::to_owned)).collect();
                            let preferred=report.runtime["preferredInterfaceGuid"].as_str().unwrap_or_default();
                            let index=network_interfaces.iter().position(|guid|guid==preferred).unwrap_or(0) as i32;
                            let names=interfaces.iter().filter(|interface|interface["guid"].is_string()).map(|interface|format!("{}{}",interface["name"].as_str().unwrap_or("Ağ"),if interface["up"]==true {" · Bağlı"}else{" · Kapalı"})).collect::<Vec<_>>();
                            let message=match result {Ok(message)=>message,Err(error)=>format!("İşlem tamamlanamadı: {error}")};
                            let status=format!("{}{}",if message.is_empty(){String::new()}else{format!("{message}\n\n")},report.summary());
                            ui.upgrade_in_event_loop(move|ui|{
                                ui.set_network_status(status.into());ui.set_network_interfaces(Rc::new(VecModel::from(names.into_iter().map(slint::SharedString::from).collect::<Vec<_>>())).into());ui.set_network_interface_index(index);
                                ui.set_network_supported(report.windows);ui.set_network_dns_enabled(report.runtime["dnsEnabled"]==true);ui.set_network_rollback_available(report.runtime["rollbackAvailable"]==true);ui.set_network_running(report.runtime["running"]==true);ui.set_network_stop_available(report.runtime["stopAvailable"]==true);
                                ui.set_network_doh_supported(report.runtime["dohSupported"]==true);ui.set_network_encrypted_dns(report.runtime["dohSupported"]==true);ui.set_network_auto_update(report.preferences.auto_update);ui.set_network_version(report.prepared_version.unwrap_or_default().into());ui.set_network_digest_verified(report.publisher_digest_verified);
                                ui.set_network_dns_details(format!("IPv4: {} · IPv6: {}\nŞifreli DNS: {}",crate::network_tools::CLOUDFLARE_IPV4.join(" / "),crate::network_tools::CLOUDFLARE_IPV6.join(" / "),crate::network_tools::CLOUDFLARE_DOH).into());
                            }).map_err(|error|error.to_string())
                        },
                        NetworkResult::Owned(source,result,account_id) => result.inspect_err(|error| {let message=error.clone();let _=ui.upgrade_in_event_loop(move |ui|if source==Source::Steam {ui.set_steam_account_status(message.into());}else{ui.set_epic_account_status(message.into());});}).and_then(|(mut games,covers)| {
                            if source==Source::Steam {
                                if let Some(id)=account_id {store.set_setting("steam_account_id",&id).map_err(|error|error.to_string())?;let _=ui.upgrade_in_event_loop(move|ui|ui.set_steam_account_id(id.into()));}
                                let roots=library_core::steam::discover_roots(&store.roots().map_err(|error|error.to_string())?);for game in &mut games {game.artwork_path=library_core::steam::local_artwork(&roots,game.app_id);}
                            }
                            store.apply_owned_games(source,&games).map_err(|error|error.to_string())?;
                            for (id,url) in covers { store.set_artwork_url(source,&id,&url).map_err(|error|error.to_string())?; }
                            let status=format!("{} hesabından {} oyun eşitlendi. Kurulum bilgileri korundu.",source.label(),games.len());
                            query.filter="all".into();
                            query.text.clear();
                            query.offset=0;
                            query.collection=None;
                            query.source=Some(source);
                            let _=ui.upgrade_in_event_loop(move |ui| {ui.set_installed_only(false);ui.set_query("".into());ui.set_section(0);ui.set_collection_id(0);ui.set_source_index(if source==Source::Steam {1}else{2});});
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
                        if let Some(bridge)=bridge.as_ref() {bridge.subscribe(event_sender.clone());}
                    }
                    if !enabled {
                        bridge = None;
                    }
                    bridge_status(
                        if enabled { bridge.as_ref() } else { None },
                        &store,
                        &social_query,
                        &avatars,
                        &state,
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
                    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                    let open_expired = state.chat_open.lock().unwrap().as_ref().is_some_and(|attempt| attempt.expired(now_ms));
                    if social_view == Some(next_view) && !open_expired {
                        return Ok(());
                    }
                    if let Some(bridge) = &bridge {
                        let snapshot = bridge.snapshot();
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
                    bridge_status(bridge.as_ref(), &store, &social_query, &avatars, &state, &ui)?;
                    social_view = Some(next_view);
                    Ok(())
                })(),
                Command::SocialFilter(text, favorites_only) => {
                    social_view = None;
                    social_query.text = text.chars().take(256).collect();
                    social_query.favorites_only = favorites_only;
                    bridge_status(bridge.as_ref(), &store, &social_query, &avatars, &state, &ui)
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
                    bridge_status(bridge.as_ref(), &store, &social_query, &avatars, &state, &ui)
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
                    bridge_status(bridge.as_ref(), &store, &social_query, &avatars, &state, &ui)
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
                    bridge_status(Some(bridge), &store, &social_query, &avatars, &state, &ui)
                })(),
                Command::StreamControl(action, key, index) => (|| {
                    let bridge = bridge.as_ref().ok_or("Discord bağlantısını aç.")?;
                    let data = crate::stream::request(&bridge.snapshot(), &action, &key, index)?;
                    bridge.command(&action, data)
                })(),
                Command::StreamCatalog(open) => (|| {
                    let bridge = bridge.as_ref().ok_or("Discord bağlantısını aç.")?;
                    bridge.command("stream_sources", serde_json::json!({"open":open}))
                })(),
                Command::StreamSelection(action, key, channel, revision, index, audio) => (|| {
                    let bridge = bridge.as_ref().ok_or("Discord bağlantısını aç.")?;
                    let data = crate::stream::selection_request(
                        &bridge.snapshot(),
                        &action,
                        &key,
                        &channel,
                        revision,
                        index,
                        audio,
                    )?;
                    bridge.command(&action, data)
                })(
                ),
                Command::Chat(action, user, channel, content) => {
                    let snapshot = bridge.as_ref().map(|bridge| bridge.snapshot()).unwrap_or_default();
                    if action == "chat_open" {
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                        *state.chat_open.lock().unwrap() = Some(crate::chat::OpenAttempt::new(&snapshot, &user, now));
                    }
                    let result = (|| {
                        let bridge = bridge.as_ref().ok_or("Discord bağlantısını aç.")?;
                        let request = crate::chat::request(&snapshot, &action, &user, &channel, &content)?;
                        bridge.command(&action, request)
                    })();
                    if action == "chat_open" {
                        if let Err(error) = &result {
                            if let Some(attempt) = state.chat_open.lock().unwrap().as_mut() { attempt.fail(error); }
                        }
                    }
                    let refresh = bridge_status(bridge.as_ref(), &store, &social_query, &avatars, &state, &ui);
                    if result.is_ok() && action == "chat_send" {
                        let _ = ui.upgrade_in_event_loop(move |ui| {
                            if ui.get_chat_user_id().as_str() == user {
                                ui.set_chat_pending(true);
                                ui.set_chat_can_send(false);
                                ui.set_chat_status("Mesajın Discord onayı bekleniyor.".into());
                            }
                        });
                    }
                    result.and(refresh)
                },
                Command::BridgeAction(action, data) => bridge
                    .as_ref()
                    .ok_or("Discord bağlantısını aç.")
                    .and_then(|bridge| {
                        if action=="open_dm" && bridge.snapshot()["capabilities"]["chat_open"]!=true {return Err("Yeni birebir sohbet köprüsü için Discord'u tamamen kapatıp yeniden aç.");}
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
                Command::UpdatePreferences(app) => (|| {
                    auto_update_app = app;
                    store
                        .set_setting("auto_update_app", if app { "true" } else { "false" })
                        .map_err(|error| error.to_string())?;
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
                        publish(&store, &query, &state, &ui)?;
                        if !scanned.report.cancelled && !network.busy.load(Ordering::SeqCst) {
                            let id = store
                                .setting("steam_account_id")
                                .ok()
                                .flatten()
                                .or_else(library_core::steam::active_user_id)
                                .unwrap_or_default();
                            submit(&network, &ui, NetworkTask::Libraries(id))?;
                        }
                        Ok(())
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
    fn bridge_updates_preserve_row_models_across_unchanged_and_changed_data() {
        use slint::{Model, ModelRc, VecModel};
        let original: ModelRc<i32> = std::rc::Rc::new(VecModel::from(vec![1, 2])).into();
        let unchanged = super::reconcile_model(original.clone(), vec![1, 2]);
        assert_eq!(original, unchanged);
        let changed = super::reconcile_model(unchanged, vec![1, 3]);
        assert_eq!(original, changed);
        assert_eq!(changed.row_data(1), Some(3));
        let extended = super::reconcile_model(changed, vec![1, 3, 4]);
        assert_eq!(original, extended);
        assert_eq!(extended.row_count(), 3);
    }

    #[test]
    fn local_artwork_is_resized_and_oversized_files_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("cover.png");
        image::RgbaImage::from_pixel(600, 900, image::Rgba([42, 57, 66, 255]))
            .save(&valid)
            .unwrap();
        let (width, height, pixels) = thumbnail(valid.to_str().unwrap()).unwrap();
        assert_eq!((width, height), (480, 720));
        assert_eq!(pixels.len(), 480 * 720 * 4);
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
