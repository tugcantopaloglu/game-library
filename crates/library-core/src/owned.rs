use crate::models::{Game, LaunchTarget, Source};
use crate::{LibraryError, Result};
use serde_json::Value;

fn account_game(
    source: Source,
    id: String,
    name: String,
    app_id: u32,
    launch: LaunchTarget,
) -> Game {
    Game {
        id: format!("{}:{id}:account", source.as_str()),
        source,
        source_id: id,
        name,
        app_id,
        launch,
        library_path: format!("account://{}", source.as_str()),
        install_path: String::new(),
        installed: false,
        favorite: false,
        size_bytes: 0.0,
        artwork_path: None,
    }
}
pub fn steam(bytes: &[u8]) -> Result<Vec<Game>> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| LibraryError::Invalid(error.to_string()))?;
    let response = &value["response"];
    let count=response["game_count"].as_u64().filter(|count|*count<=20_000).ok_or_else(||LibraryError::Invalid("Steam oyun ayrıntıları erişilemiyor. API anahtarını ve profil görünürlüğünü kontrol et.".into()))?;
    let rows = response["games"].as_array();
    if rows.map_or(0, Vec::len) != count as usize {
        return Err(LibraryError::Invalid(
            "Steam hesap listesi eksik; mevcut kütüphane korundu".into(),
        ));
    }
    let mut games = Vec::new();
    for row in rows.into_iter().flatten() {
        let app_id = row["appid"]
            .as_u64()
            .filter(|id| *id > 0 && *id <= u64::from(u32::MAX))
            .ok_or_else(|| LibraryError::Invalid("Steam AppID geçersiz".into()))?
            as u32;
        if app_id == 228980 {
            continue;
        }
        let name = row["name"]
            .as_str()
            .filter(|name| !name.trim().is_empty() && name.len() <= 512 && !name.contains('\0'))
            .ok_or_else(|| LibraryError::Invalid("Steam oyun adı eksik".into()))?;
        games.push(account_game(
            Source::Steam,
            app_id.to_string(),
            name.into(),
            app_id,
            LaunchTarget::Steam { app_id },
        ));
    }
    games.sort_by_key(|game| game.app_id);
    games.dedup_by_key(|game| game.app_id);
    Ok(games)
}

pub type OwnedLibrary = (Vec<Game>, Vec<(String, String)>);
pub fn epic(bytes: &[u8]) -> Result<OwnedLibrary> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| LibraryError::Invalid(error.to_string()))?;
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= 20_000)
        .ok_or_else(|| {
            LibraryError::Invalid("Epic hesap listesi geçersiz; mevcut kütüphane korundu".into())
        })?;
    let mut games = Vec::new();
    let mut covers = Vec::new();
    for row in rows {
        let metadata = &row["metadata"];
        let app = row["app_name"]
            .as_str()
            .filter(|app| crate::discovery::identifier(app))
            .ok_or_else(|| LibraryError::Invalid("Epic hesap oyun kimliği geçersiz".into()))?;
        let name = row["app_title"]
            .as_str()
            .filter(|name| !name.is_empty() && name.len() <= 512 && !name.contains('\0'))
            .ok_or_else(|| LibraryError::Invalid("Epic hesap oyun adı eksik".into()))?;
        let ns = metadata["namespace"].as_str().unwrap_or("");
        let catalog = metadata["id"].as_str().unwrap_or("");
        if ns == "ue"
            || app.starts_with("UE_")
            || metadata["categories"].as_array().is_some_and(|rows| {
                rows.iter().any(|category| {
                    category["path"].as_str().is_some_and(|path| {
                        path.starts_with("plugins")
                            || path.starts_with("engines")
                            || path == "dlc"
                            || path == "software"
                    })
                })
            })
        {
            continue;
        }
        if !(crate::discovery::identifier(ns) && crate::discovery::identifier(catalog)) {
            return Err(LibraryError::Invalid("Epic catalog kimliği eksik".into()));
        }
        let source_id = format!("{ns}:{catalog}:{app}");
        if let Some(images) = metadata["keyImages"].as_array() {
            let selected = images
                .iter()
                .find(|image| {
                    image["type"].as_str().is_some_and(|kind| {
                        matches!(kind, "OfferImageTall" | "DieselGameBoxTall" | "Thumbnail")
                    })
                })
                .or_else(|| images.first());
            if let Some(url) = selected.and_then(|image| image["url"].as_str()) {
                covers.push((source_id.clone(), url.into()));
            }
        }
        games.push(account_game(
            Source::Epic,
            source_id,
            name.into(),
            0,
            LaunchTarget::Epic {
                namespace_id: ns.into(),
                catalog_id: catalog.into(),
                app_name: app.into(),
                executable: String::new(),
            },
        ));
    }
    games.sort_by(|a, b| a.source_id.cmp(&b.source_id));
    games.dedup_by(|a, b| a.source_id == b.source_id);
    Ok((games, covers))
}
