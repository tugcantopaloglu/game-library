use crate::models::{Game, LaunchTarget, Source};
use crate::{discovery, LibraryError, Result};
use std::path::Path;

pub fn game(executable: &Path, title: &str, args: Vec<String>) -> Result<Game> {
    let executable = std::fs::canonicalize(executable)?;
    if !executable.is_file()
        || !executable
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        || args.len() > 32
        || args
            .iter()
            .any(|argument| argument.len() > 2048 || argument.contains('\0'))
    {
        return Err(LibraryError::Invalid(
            "Geçerli bir .exe dosyası seç.".into(),
        ));
    }
    let title = title.trim();
    if title.is_empty() || title.len() > 512 || title.contains('\0') {
        return Err(LibraryError::Invalid(
            "Oyun adı 1–512 karakter olmalı.".into(),
        ));
    }
    let parent = executable
        .parent()
        .ok_or_else(|| LibraryError::Invalid("Kurulum klasörü yok".into()))?;
    let key = discovery::path_key(&executable);
    Ok(Game {
        id: format!("manual:{key}"),
        source: Source::Manual,
        source_id: key,
        app_id: 0,
        name: title.into(),
        library_path: parent.to_string_lossy().into_owned(),
        install_path: parent.to_string_lossy().into_owned(),
        installed: true,
        favorite: false,
        size_bytes: 0.0,
        artwork_path: None,
        launch: LaunchTarget::Manual {
            executable: executable.to_string_lossy().into_owned(),
            args,
            working_dir: parent.to_string_lossy().into_owned(),
        },
    })
}
