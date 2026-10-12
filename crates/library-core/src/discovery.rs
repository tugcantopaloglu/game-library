use crate::models::{Game, RootScan, Source};
use crate::{LibraryError, Result};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub fn bounded_text(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > 1024 * 1024 {
        return Err(LibraryError::Invalid(
            "Kurulum kaydı 1 MiB sınırını aşıyor".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(LibraryError::Invalid(
            "Kurulum kaydı 1 MiB sınırını aşıyor".into(),
        ));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| LibraryError::Invalid("Kurulum kaydı UTF-8 değil".into()))?;
    Ok(text.trim_start_matches('\u{feff}').to_owned())
}

pub fn absolute_path(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if value.is_empty()
        || value.contains('\0')
        || !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(LibraryError::Invalid(
            "Kurulum yolu mutlak ve geçerli bir dosya yolu olmalı".into(),
        ));
    }
    Ok(std::fs::canonicalize(&path).unwrap_or(path))
}

pub fn relative_executable(root: &Path, value: &str) -> Result<PathBuf> {
    let relative = Path::new(value);
    if value.is_empty()
        || value.contains('\0')
        || value.contains(':')
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(LibraryError::Invalid(
            "Oyun executable yolu kurulumun içinde olmalı".into(),
        ));
    }
    let path = root.join(relative);
    if let (Ok(root), Ok(path)) = (std::fs::canonicalize(root), std::fs::canonicalize(&path)) {
        if !path.starts_with(&root) {
            return Err(LibraryError::Invalid(
                "Executable kurulum klasörünün dışına çıkıyor".into(),
            ));
        }
        return Ok(path);
    }
    Ok(path)
}

pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

pub fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}").to_lowercase()
    } else {
        value.trim_start_matches("\\\\?\\").to_lowercase()
    }
}

pub fn roots(extra: &[String], standard: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = BTreeMap::new();
    for root in standard
        .iter()
        .filter(|path| path.try_exists().unwrap_or(true))
        .cloned()
        .chain(extra.iter().map(PathBuf::from))
    {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        roots.insert(root.to_string_lossy().to_lowercase(), root);
    }
    roots.into_values().collect()
}

pub fn warn(warnings: &mut Vec<String>, message: impl Into<String>) {
    if warnings.len() < 100 {
        warnings.push(message.into());
    }
}

pub fn scan_directory(
    source: Source,
    root: &Path,
    directory: &Path,
    extension: &str,
    cancel: &AtomicBool,
    warnings: &mut Vec<String>,
    mut parse: impl FnMut(&Path) -> Result<Option<Game>>,
) -> RootScan {
    let mut result = RootScan {
        source,
        path: root.to_string_lossy().into_owned(),
        games: Vec::new(),
        complete: true,
    };
    if cancel.load(Ordering::Relaxed) {
        result.complete = false;
        return result;
    }
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            result.complete = false;
            warn(warnings, format!("{}: {error}", directory.display()));
            return result;
        }
    };
    let mut games: BTreeMap<String, Game> = BTreeMap::new();
    for (index, entry) in entries.enumerate() {
        if index >= 20_000 || cancel.load(Ordering::Relaxed) {
            result.complete = false;
            if index >= 20_000 {
                warn(warnings, "Kaynak dizini 20.000 kayıt sınırını aşıyor");
            }
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.complete = false;
                warn(warnings, error.to_string());
                continue;
            }
        };
        if !entry
            .path()
            .extension()
            .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        {
            continue;
        }
        match parse(&entry.path()) {
            Ok(Some(game)) => {
                if let Some(old) = games.get(&game.id) {
                    if !old.install_path.eq_ignore_ascii_case(&game.install_path) {
                        result.complete = false;
                        warn(
                            warnings,
                            format!(
                                "{}: aynı kaynak kimliği için birden fazla kurulum kaydı var",
                                game.name
                            ),
                        );
                    }
                    if old.installed || !game.installed {
                        continue;
                    }
                }
                games.insert(game.id.clone(), game);
            }
            Ok(None) => {}
            Err(error) => {
                result.complete = false;
                warn(warnings, format!("{}: {error}", entry.path().display()));
            }
        }
    }
    result.games = games.into_values().collect();
    result
}
