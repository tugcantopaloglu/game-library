use crate::models::{Game, LaunchTarget, RootScan, ScanReport, ScanResult, Source};
use crate::{LibraryError, Result};
use keyvalues_parser::Obj;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

fn field<'a>(object: &'a Obj<'_>, key: &str) -> Option<&'a str> {
    object.get(key)?.first()?.get_str()
}

fn bounded_text(path: &Path, limit: u64) -> Result<String> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > limit {
        return Err(LibraryError::Invalid(
            "Manifest boyut sınırını aşıyor".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(LibraryError::Invalid(
            "Manifest boyut sınırını aşıyor".into(),
        ));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| LibraryError::Invalid("Manifest UTF-8 olarak okunamadı".into()))?;
    let mut depth = 0_u32;
    for character in text.chars() {
        match character {
            '{' => {
                depth += 1;
                if depth > 32 {
                    return Err(LibraryError::Invalid(
                        "Manifest iç içe geçme sınırını aşıyor".into(),
                    ));
                }
            }
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(text.trim_start_matches('\u{feff}').to_owned())
}

pub fn normalize_root(path: &Path) -> Result<PathBuf> {
    let root = if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
    {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    if !root.join("steamapps").is_dir() {
        return Err(LibraryError::Invalid(
            "Steam veya SteamLibrary klasörünü seç. İçinde steamapps bulunmalı.".into(),
        ));
    }
    Ok(fs::canonicalize(root)?)
}

pub fn discover_roots(extra: &[String]) -> Vec<PathBuf> {
    roots_with_system(extra, true)
}

fn roots_with_system(extra: &[String], include_system: bool) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = extra.iter().map(PathBuf::from).collect();
    if include_system {
        #[cfg(windows)]
        {
            use winreg::enums::*;
            use winreg::RegKey;
            for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
                for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
                    if let Ok(key) = RegKey::predef(hive)
                        .open_subkey_with_flags("Software\\Valve\\Steam", KEY_READ | view)
                    {
                        for name in ["SteamPath", "InstallPath"] {
                            if let Ok(value) = key.get_value::<String, _>(name) {
                                candidates.push(PathBuf::from(value));
                            }
                        }
                    }
                }
            }
            if let Ok(key) =
                RegKey::predef(HKEY_CLASSES_ROOT).open_subkey("steam\\shell\\open\\command")
            {
                if let Ok(command) = key.get_value::<String, _>("") {
                    let executable = command
                        .strip_prefix('"')
                        .and_then(|rest| rest.split('"').next());
                    if let Some(parent) = executable.and_then(|exe| Path::new(exe).parent()) {
                        candidates.push(parent.to_owned());
                    }
                }
            }
        }
        for variable in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(path) = std::env::var_os(variable) {
                candidates.push(PathBuf::from(path).join("Steam"));
            }
        }
    }
    let initial = candidates.clone();
    for root in initial {
        for file in [
            root.join("steamapps/libraryfolders.vdf"),
            root.join("config/libraryfolders.vdf"),
        ] {
            if let Ok(text) = bounded_text(&file, 4 * 1024 * 1024) {
                if let Ok(paths) = parse_library_paths(&text) {
                    candidates.extend(paths);
                }
            }
        }
    }
    let mut unique = BTreeMap::new();
    for path in candidates {
        let path = if path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
        {
            path.parent().unwrap_or(&path).to_owned()
        } else {
            path
        };
        if path.join("steamapps").is_dir() || extra.iter().any(|saved| Path::new(saved) == path) {
            let canonical = fs::canonicalize(&path).unwrap_or(path);
            unique.insert(canonical.to_string_lossy().to_lowercase(), canonical);
        }
    }
    unique.into_values().collect()
}

pub fn parse_library_paths(text: &str) -> Result<Vec<PathBuf>> {
    let vdf = keyvalues_parser::parse(text)
        .map_err(|_| LibraryError::Invalid("Steam kütüphane listesi okunamadı".into()))?;
    if !vdf.key.eq_ignore_ascii_case("libraryfolders") {
        return Err(LibraryError::Invalid(
            "Beklenmeyen Steam kütüphane belgesi".into(),
        ));
    }
    let object = vdf
        .value
        .get_obj()
        .ok_or_else(|| LibraryError::Invalid("Kütüphane listesi nesne değil".into()))?;
    let mut paths = Vec::new();
    for (key, values) in object.iter() {
        if key.parse::<u32>().is_err() {
            continue;
        }
        for value in values {
            let path = value
                .get_obj()
                .and_then(|object| field(object, "path"))
                .or_else(|| value.get_str());
            if let Some(path) = path.filter(|path| !path.is_empty()) {
                paths.push(PathBuf::from(path));
            }
        }
    }
    Ok(paths)
}

pub fn parse_manifest(text: &str, root: &Path, expected_app_id: u32) -> Result<Game> {
    let document = keyvalues_parser::parse(text)
        .map_err(|_| LibraryError::Invalid("Oyun manifesti ayrıştırılamadı".into()))?;
    if !document.key.eq_ignore_ascii_case("AppState") {
        return Err(LibraryError::Invalid("AppState alanı bulunamadı".into()));
    }
    let object = document
        .value
        .get_obj()
        .ok_or_else(|| LibraryError::Invalid("AppState nesne değil".into()))?;
    let app_id = field(object, "appid")
        .and_then(|id| id.parse::<u32>().ok())
        .filter(|id| *id != 0 && *id == expected_app_id)
        .ok_or_else(|| LibraryError::Invalid("Dosya adı ve Steam AppID uyuşmuyor".into()))?;
    let name = field(object, "name")
        .filter(|name| !name.trim().is_empty() && name.len() <= 512)
        .ok_or_else(|| LibraryError::Invalid("Oyun adı bulunamadı".into()))?;
    let folder = field(object, "installdir")
        .filter(|folder| !folder.is_empty() && folder.len() <= 512)
        .ok_or_else(|| LibraryError::Invalid("Kurulum klasörü bulunamadı".into()))?;
    let relative = Path::new(folder);
    if folder.contains(':')
        || folder.contains('\\')
        || folder.contains('/')
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(LibraryError::Invalid(
            "Kurulum klasörü güvenli bir göreli yol değil".into(),
        ));
    }
    let install = root.join("steamapps/common").join(relative);
    let flags = field(object, "StateFlags")
        .and_then(|flags| flags.parse::<u32>().ok())
        .unwrap_or(0);
    let size = field(object, "SizeOnDisk")
        .and_then(|size| size.parse::<u64>().ok())
        .unwrap_or(0);
    let library = root.to_string_lossy().into_owned();
    Ok(Game {
        id: format!("steam:{app_id}:{}", library.to_lowercase()),
        app_id,
        source: Source::Steam,
        source_id: app_id.to_string(),
        launch: LaunchTarget::Steam { app_id },
        name: name.to_owned(),
        library_path: library,
        install_path: install.to_string_lossy().into_owned(),
        installed: flags & 4 != 0 && install.is_dir(),
        favorite: false,
        size_bytes: size as f64,
        artwork_path: None,
    })
}

fn local_artwork(roots: &[PathBuf], app_id: u32) -> Option<String> {
    for root in roots {
        for suffix in ["library_600x900.jpg", "library_600x900.png", "header.jpg"] {
            let path = root
                .join("appcache/librarycache")
                .join(format!("{app_id}_{suffix}"));
            if path.is_file() {
                return Some(path.to_string_lossy().into_owned());
            }
        }
    }
    None
}

pub fn scan(extra: &[String], cancel: &AtomicBool) -> ScanResult {
    scan_discovered(discover_roots(extra), cancel)
}

/// Scan only explicitly supplied libraries and their libraryfolders, without host discovery.
pub fn scan_configured(roots: &[String], cancel: &AtomicBool) -> ScanResult {
    scan_discovered(roots_with_system(roots, false), cancel)
}

fn scan_discovered(roots: Vec<PathBuf>, cancel: &AtomicBool) -> ScanResult {
    let start = Instant::now();
    let mut warnings = Vec::new();
    let mut libraries = Vec::new();
    let mut found = 0_u32;
    for root in &roots {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut library = RootScan {
            source: Source::Steam,
            path: root.to_string_lossy().into_owned(),
            games: Vec::new(),
            complete: true,
        };
        match fs::read_dir(root.join("steamapps")) {
            Ok(entries) => {
                for entry in entries {
                    if cancel.load(Ordering::Relaxed) {
                        library.complete = false;
                        break;
                    }
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(error) => {
                            library.complete = false;
                            warnings.push(format!("{}: {error}", root.display()));
                            continue;
                        }
                    };
                    let file_name = entry.file_name().to_string_lossy().into_owned();
                    let Some(app_id) = file_name
                        .strip_prefix("appmanifest_")
                        .and_then(|name| name.strip_suffix(".acf"))
                        .and_then(|id| id.parse::<u32>().ok())
                    else {
                        continue;
                    };
                    let result = bounded_text(&entry.path(), 1024 * 1024)
                        .and_then(|text| parse_manifest(&text, root, app_id));
                    match result {
                        Ok(mut game) => {
                            game.artwork_path = local_artwork(&roots, app_id);
                            library.games.push(game);
                            found += 1;
                        }
                        Err(error) => {
                            library.complete = false;
                            if warnings.len() < 100 {
                                warnings.push(format!("{file_name}: {error}"));
                            }
                        }
                    }
                }
            }
            Err(error) => {
                library.complete = false;
                warnings.push(format!("{}: {error}", root.display()));
            }
        }
        libraries.push(library);
    }
    ScanResult {
        report: ScanReport {
            roots: roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect(),
            found,
            warnings,
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            cancelled: cancel.load(Ordering::Relaxed),
        },
        libraries,
    }
}

pub fn launch_uri(app_id: u32) -> Result<String> {
    if app_id == 0 {
        return Err(LibraryError::Invalid("Geçersiz Steam AppID".into()));
    }
    Ok(format!("steam://rungameid/{app_id}"))
}
