use crate::discovery;
use crate::models::{Game, LaunchTarget, RootScan, Source};
use crate::{LibraryError, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    app_name: String,
    display_name: String,
    install_location: String,
    #[serde(default)]
    launch_executable: String,
    #[serde(default)]
    catalog_namespace: String,
    #[serde(default)]
    catalog_item_id: String,
    #[serde(default)]
    main_game_app_name: String,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(rename = "bIsApplication")]
    is_application: Option<bool>,
    #[serde(rename = "bIsExecutable")]
    is_executable: Option<bool>,
    #[serde(rename = "bIsIncompleteInstall", default)]
    incomplete: bool,
    #[serde(default)]
    install_size: serde_json::Value,
}

pub fn normalize_root(path: &Path) -> Result<PathBuf> {
    for candidate in [
        path.to_path_buf(),
        path.join("Data/Manifests"),
        path.join("EpicGamesLauncher/Data/Manifests"),
        path.join("Epic/EpicGamesLauncher/Data/Manifests"),
    ] {
        if !candidate.is_dir() {
            continue;
        }
        let named = candidate
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Manifests"));
        let contains = std::fs::read_dir(&candidate)?
            .take(20_000)
            .filter_map(std::result::Result::ok)
            .any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("item"))
            });
        if named || contains {
            return Ok(std::fs::canonicalize(candidate)?);
        }
    }
    Err(LibraryError::Invalid(
        "Epic Data\\Manifests klasörünü veya onu içeren Epic veri klasörünü seç".into(),
    ))
}

pub fn parse_manifest(text: &str, root: &Path) -> Result<Option<Game>> {
    let manifest: Manifest = serde_json::from_str(text)
        .map_err(|error| LibraryError::Invalid(format!("Epic manifesti okunamadı: {error}")))?;
    if !discovery::identifier(&manifest.app_name) {
        return Err(LibraryError::Invalid("Epic oyun kimliği geçersiz".into()));
    }
    if manifest.is_application == Some(false)
        || manifest.is_executable == Some(false)
        || manifest.app_name.starts_with("UE_")
        || (!manifest.main_game_app_name.is_empty()
            && manifest.main_game_app_name != manifest.app_name)
        || manifest.app_categories.iter().any(|category| {
            let category = category.to_ascii_lowercase();
            category == "ue"
                || category == "dlc"
                || category == "software"
                || category.starts_with("plugins")
                || category.starts_with("engines")
        })
    {
        return Ok(None);
    }
    if manifest.display_name.trim().is_empty() || manifest.display_name.len() > 512 {
        return Err(LibraryError::Invalid(
            "Epic oyun kimliği veya adı geçersiz".into(),
        ));
    }
    if !(manifest.catalog_namespace.is_empty() && manifest.catalog_item_id.is_empty())
        && !(discovery::identifier(&manifest.catalog_namespace)
            && discovery::identifier(&manifest.catalog_item_id))
    {
        return Err(LibraryError::Invalid(
            "Epic namespace/catalog kimliği eksik veya geçersiz".into(),
        ));
    }
    let install = discovery::absolute_path(&manifest.install_location)?;
    // Epic's launch URI resolves the executable itself. Current game manifests may omit it.
    let executable = if manifest.launch_executable.is_empty() {
        None
    } else {
        Some(discovery::relative_executable(
            &install,
            &manifest.launch_executable,
        )?)
    };
    let source_id = if manifest.catalog_namespace.is_empty() {
        manifest.app_name.clone()
    } else {
        format!(
            "{}:{}:{}",
            manifest.catalog_namespace, manifest.catalog_item_id, manifest.app_name
        )
    };
    let size = manifest
        .install_size
        .as_u64()
        .or_else(|| {
            manifest
                .install_size
                .as_str()
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0);
    Ok(Some(Game {
        id: format!("epic:{source_id}:{}", root.to_string_lossy().to_lowercase()),
        app_id: 0,
        source: Source::Epic,
        source_id,
        name: manifest.display_name,
        library_path: root.to_string_lossy().into_owned(),
        install_path: install.to_string_lossy().into_owned(),
        installed: !manifest.incomplete
            && install.is_dir()
            && executable.as_ref().map_or_else(
                || {
                    std::fs::read_dir(&install).is_ok_and(|entries| {
                        entries
                            .take(100)
                            .filter_map(std::result::Result::ok)
                            .any(|entry| entry.file_name() != ".egstore")
                    })
                },
                |path| path.is_file(),
            ),
        favorite: false,
        size_bytes: size as f64,
        artwork_path: None,
        launch: LaunchTarget::Epic {
            namespace_id: manifest.catalog_namespace,
            catalog_id: manifest.catalog_item_id,
            app_name: manifest.app_name,
            executable: executable
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
        },
    }))
}

pub fn scan(extra: &[String], cancel: &AtomicBool, warnings: &mut Vec<String>) -> Vec<RootScan> {
    let data =
        PathBuf::from(std::env::var_os("PROGRAMDATA").unwrap_or_else(|| "C:\\ProgramData".into()));
    let standard = data.join("Epic/EpicGamesLauncher/Data/Manifests");
    discovery::roots(extra, &[standard])
        .iter()
        .map(|root| {
            discovery::scan_directory(Source::Epic, root, root, "item", cancel, warnings, |path| {
                parse_manifest(&discovery::bounded_text(path)?, root)
            })
        })
        .collect()
}

pub fn scan_at(root: &Path, cancel: &AtomicBool, warnings: &mut Vec<String>) -> RootScan {
    discovery::scan_directory(Source::Epic, root, root, "item", cancel, warnings, |path| {
        parse_manifest(&discovery::bounded_text(path)?, root)
    })
}
