use crate::discovery;
use crate::models::{Game, LaunchTarget, RootScan, Source};
use crate::{LibraryError, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use yaml_rust2::parser::{Event, Parser};

#[derive(Default, Deserialize)]
struct ClientInstalls {
    #[serde(default)]
    associated_client: HashMap<String, String>,
    #[serde(default)]
    rc_default: String,
    #[serde(default)]
    rc_live: String,
    #[serde(default)]
    rc_beta: String,
}

pub fn product(product: &str) -> Option<(&'static str, &'static str)> {
    match product {
        "league_of_legends" => Some(("League of Legends", "LeagueClient.exe")),
        "valorant" => Some(("VALORANT", "VALORANT.exe")),
        "bacon" => Some(("Legends of Runeterra", "LoR.exe")),
        "lion" => Some(("2XKO", "Lion.exe")),
        _ => None,
    }
}

pub fn normalize_root(path: &Path) -> Result<PathBuf> {
    let base = if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Metadata"))
    {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    for root in [base.to_path_buf(), base.join("Riot Games")] {
        if root.join("Metadata").is_dir() || root.join("RiotClientInstalls.json").is_file() {
            return Ok(std::fs::canonicalize(root)?);
        }
    }
    Err(LibraryError::Invalid(
        "Metadata veya RiotClientInstalls.json içeren Riot Games veri klasörünü seç".into(),
    ))
}

fn settings(text: &str) -> Result<BTreeMap<String, String>> {
    let mut parser = Parser::new_from_str(text);
    let mut depth = 0_usize;
    let mut documents = 0;
    let mut root_mapping = false;
    let mut key = None;
    let mut fields = BTreeMap::new();
    let mut seen = HashSet::new();
    for _ in 0..10_000 {
        let (event, _) = parser
            .next_token()
            .map_err(|error| LibraryError::Invalid(format!("Riot YAML okunamadı: {error}")))?;
        let is_mapping = matches!(&event, Event::MappingStart(..));
        match event {
            Event::DocumentStart => {
                documents += 1;
                if documents > 1 {
                    return Err(LibraryError::Invalid(
                        "Riot metadata birden fazla YAML belgesi içeriyor".into(),
                    ));
                }
            }
            Event::MappingStart(anchor, tag) | Event::SequenceStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(LibraryError::Invalid(
                        "Riot YAML anchor/tag içeriyor".into(),
                    ));
                }
                if depth == 0 {
                    if !is_mapping {
                        return Err(LibraryError::Invalid(
                            "Riot metadata kök alanları mapping olmalı".into(),
                        ));
                    }
                    root_mapping = true;
                }
                if depth == 1 {
                    key = None;
                }
                depth += 1;
                if depth > 16 {
                    return Err(LibraryError::Invalid(
                        "Riot YAML derinlik sınırını aşıyor".into(),
                    ));
                }
            }
            Event::MappingEnd | Event::SequenceEnd => depth = depth.saturating_sub(1),
            Event::Alias(_) => {
                return Err(LibraryError::Invalid("Riot YAML alias içeriyor".into()))
            }
            Event::Scalar(value, _, anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(LibraryError::Invalid(
                        "Riot YAML anchor/tag içeriyor".into(),
                    ));
                }
                if depth == 1 {
                    if let Some(name) = key.take() {
                        fields.insert(name, value);
                    } else {
                        if !seen.insert(value.clone()) {
                            return Err(LibraryError::Invalid(
                                "Riot YAML tekrarlanan alan içeriyor".into(),
                            ));
                        }
                        key = Some(value);
                    }
                }
            }
            Event::StreamEnd => {
                if root_mapping {
                    return Ok(fields);
                }
                return Err(LibraryError::Invalid(
                    "Riot metadata boş veya geçersiz".into(),
                ));
            }
            _ => {}
        }
    }
    Err(LibraryError::Invalid(
        "Riot YAML olay sınırını aşıyor".into(),
    ))
}

fn client_path(value: &str) -> Option<PathBuf> {
    let path = discovery::absolute_path(value).ok()?;
    (path.is_file()
        && path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("RiotClientServices.exe")))
    .then_some(path)
}

fn resolve_client(index: &ClientInstalls, install: &Path, fallback: &[PathBuf]) -> String {
    for (game, client) in &index.associated_client {
        if discovery::absolute_path(game).ok().is_some_and(|path| {
            path.to_string_lossy()
                .eq_ignore_ascii_case(&install.to_string_lossy())
        }) {
            if let Some(client) = client_path(client) {
                return client.to_string_lossy().into_owned();
            }
        }
    }
    for value in [&index.rc_default, &index.rc_live, &index.rc_beta] {
        if let Some(client) = client_path(value) {
            return client.to_string_lossy().into_owned();
        }
    }
    fallback
        .iter()
        .find_map(|path| client_path(&path.to_string_lossy()))
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn game(
    root: &str,
    product_id: &str,
    patchline: &str,
    install: &Path,
    enabled: bool,
    client: String,
) -> Result<Game> {
    let (name, exe) = product(product_id)
        .ok_or_else(|| LibraryError::Invalid("Desteklenmeyen Riot ürünü".into()))?;
    if !discovery::identifier(patchline) {
        return Err(LibraryError::Invalid("Riot patchline geçersiz".into()));
    }
    let executable = discovery::relative_executable(install, exe)?;
    let source_id = format!("{product_id}.{patchline}");
    let title = if patchline == "live" {
        name.to_owned()
    } else {
        format!("{name} ({patchline})")
    };
    Ok(Game {
        id: format!("riot:{source_id}:{}", discovery::path_key(install)),
        source: Source::Riot,
        source_id,
        app_id: 0,
        name: title,
        library_path: root.into(),
        install_path: install.to_string_lossy().into_owned(),
        installed: enabled && install.is_dir() && executable.is_file(),
        favorite: false,
        size_bytes: 0.0,
        artwork_path: None,
        launch: LaunchTarget::Riot {
            client_path: client,
            product: product_id.into(),
            patchline: patchline.into(),
            executable: executable.to_string_lossy().into_owned(),
        },
    })
}

fn metadata(
    root: &Path,
    cancel: &AtomicBool,
    warnings: &mut Vec<String>,
    fallback: &[PathBuf],
) -> RootScan {
    let mut result = RootScan {
        source: Source::Riot,
        path: root.to_string_lossy().into_owned(),
        games: Vec::new(),
        complete: true,
    };
    if cancel.load(Ordering::Relaxed) {
        result.complete = false;
        return result;
    }
    let installs_file = root.join("RiotClientInstalls.json");
    let index = if installs_file.try_exists().unwrap_or(true) {
        match discovery::bounded_text(&installs_file).and_then(|text| {
            serde_json::from_str::<ClientInstalls>(&text)
                .map_err(|error| LibraryError::Invalid(error.to_string()))
        }) {
            Ok(index) => index,
            Err(error) => {
                result.complete = false;
                discovery::warn(warnings, format!("{}: {error}", installs_file.display()));
                ClientInstalls::default()
            }
        }
    } else {
        ClientInstalls::default()
    };
    let entries = match std::fs::read_dir(root.join("Metadata")) {
        Ok(entries) => entries,
        Err(error) => {
            result.complete = false;
            discovery::warn(warnings, format!("{}: {error}", root.display()));
            return result;
        }
    };
    for (position, entry) in entries.enumerate() {
        if cancel.load(Ordering::Relaxed) || position >= 20_000 {
            result.complete = false;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.complete = false;
                discovery::warn(warnings, error.to_string());
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((product_id, patchline)) = name.split_once('.') else {
            continue;
        };
        if product(product_id).is_none() {
            continue;
        }
        let file = entry.path().join(format!("{name}.product_settings.yaml"));
        let parsed = (|| {
            let fields = settings(&discovery::bounded_text(&file)?)?;
            let install = discovery::absolute_path(
                fields
                    .get("product_install_full_path")
                    .ok_or_else(|| LibraryError::Invalid("Riot kurulum yolu eksik".into()))?,
            )?;
            let enabled = !fields
                .get("product_install_enabled")
                .is_some_and(|value| value.eq_ignore_ascii_case("false"));
            game(
                &result.path,
                product_id,
                patchline,
                &install,
                enabled,
                resolve_client(&index, &install, fallback),
            )
        })();
        match parsed {
            Ok(game) => result.games.push(game),
            Err(error) => {
                result.complete = false;
                discovery::warn(warnings, format!("{}: {error}", file.display()));
            }
        }
    }
    result
}

#[cfg(windows)]
fn registered_client() -> Vec<PathBuf> {
    use winreg::{enums::*, RegKey};
    let mut clients = Vec::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(
                "Software\\Classes\\riotclient\\shell\\open\\command",
                KEY_READ | view,
            ) {
                if let Ok(command) = key.get_value::<String, _>("") {
                    let command = command.trim();
                    let path = if let Some(quoted) = command.strip_prefix('"') {
                        quoted.split('"').next()
                    } else {
                        command
                            .to_lowercase()
                            .find("riotclientservices.exe")
                            .map(|index| &command[..index + "riotclientservices.exe".len()])
                    };
                    if let Some(path) = path.and_then(client_path) {
                        clients.push(path);
                    }
                }
            }
        }
    }
    clients
}

#[cfg(windows)]
fn registry(cancel: &AtomicBool, warnings: &mut Vec<String>, clients: &[PathBuf]) -> RootScan {
    use winreg::{enums::*, RegKey};
    let mut result = RootScan {
        source: Source::Riot,
        path: "registry://RiotGames".into(),
        games: Vec::new(),
        complete: true,
    };
    let mut games = BTreeMap::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            for product in ["league_of_legends", "valorant", "bacon", "lion"] {
                for patch in ["live", "pbe"] {
                    if cancel.load(Ordering::Relaxed) {
                        result.complete = false;
                        continue;
                    }
                    let key=format!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Riot Game {product}.{patch}");
                    match RegKey::predef(hive).open_subkey_with_flags(&key, KEY_READ | view) {
                        Ok(key) => {
                            let parsed = key
                                .get_value::<String, _>("InstallLocation")
                                .map_err(LibraryError::Io)
                                .and_then(|path| discovery::absolute_path(&path))
                                .and_then(|install| {
                                    game(
                                        &result.path,
                                        product,
                                        patch,
                                        &install,
                                        true,
                                        resolve_client(
                                            &ClientInstalls::default(),
                                            &install,
                                            clients,
                                        ),
                                    )
                                });
                            match parsed {
                                Ok(game) => {
                                    games.insert(game.id.clone(), game);
                                }
                                Err(error) => {
                                    result.complete = false;
                                    discovery::warn(warnings, error.to_string());
                                }
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => {
                            result.complete = false;
                            discovery::warn(warnings, error.to_string());
                        }
                    }
                }
            }
        }
    }
    result.games = games.into_values().collect();
    result
}

pub fn scan(extra: &[String], cancel: &AtomicBool, warnings: &mut Vec<String>) -> Vec<RootScan> {
    let data =
        PathBuf::from(std::env::var_os("PROGRAMDATA").unwrap_or_else(|| "C:\\ProgramData".into()));
    let roots = discovery::roots(extra, &[data.join("Riot Games"), data.join("RiotGames")]);
    #[cfg(windows)]
    let mut clients = registered_client();
    #[cfg(not(windows))]
    let mut clients = Vec::new();
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    clients.push(PathBuf::from(format!(
        "{drive}\\Riot Games\\Riot Client\\RiotClientServices.exe"
    )));
    for root in &roots {
        if let Ok(index) =
            discovery::bounded_text(&root.join("RiotClientInstalls.json")).and_then(|text| {
                serde_json::from_str::<ClientInstalls>(&text)
                    .map_err(|error| LibraryError::Invalid(error.to_string()))
            })
        {
            clients.extend(
                [index.rc_default, index.rc_live, index.rc_beta]
                    .iter()
                    .chain(index.associated_client.values())
                    .filter_map(|path| client_path(path)),
            );
        }
    }
    let mut results: Vec<_> = roots
        .iter()
        .map(|root| metadata(root, cancel, warnings, &clients))
        .collect();
    #[cfg(windows)]
    {
        let mut registered = registry(cancel, warnings, &clients);
        let known: HashSet<_> = results
            .iter()
            .flat_map(|root| root.games.iter().map(|game| game.id.clone()))
            .collect();
        registered.games.retain(|game| !known.contains(&game.id));
        results.push(registered);
    }
    results
}

pub fn scan_at(root: &Path, cancel: &AtomicBool, warnings: &mut Vec<String>) -> RootScan {
    metadata(root, cancel, warnings, &[])
}
