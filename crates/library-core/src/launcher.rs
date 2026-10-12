use crate::models::{Game, LaunchTarget, Source};
use crate::{discovery, riot, steam, LibraryError, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LaunchPlan {
    Uri {
        uri: String,
    },
    Process {
        executable: PathBuf,
        args: Vec<String>,
        working_dir: PathBuf,
    },
}

fn installed_executable(install: &Path, executable: &str) -> Result<PathBuf> {
    let root = std::fs::canonicalize(install)?;
    let path = discovery::absolute_path(executable)?;
    if !path.is_file() || !path.starts_with(root) {
        return Err(LibraryError::Invalid(
            "Oyun executable dosyası kurulum içinde bulunamadı. Yeniden tara.".into(),
        ));
    }
    Ok(path)
}

pub fn prepare(game: &Game) -> Result<LaunchPlan> {
    let install = Path::new(&game.install_path);
    if !game.installed || !install.is_dir() {
        return Err(LibraryError::Invalid(
            "Oyun kurulumu erişilemiyor. Kütüphaneyi yeniden tara.".into(),
        ));
    }
    match (&game.launch, game.source) {
        (
            LaunchTarget::Manual {
                executable,
                args,
                working_dir,
            },
            Source::Manual,
        ) => {
            let executable = installed_executable(install, executable)?;
            if !executable
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                || args.len() > 32
                || args
                    .iter()
                    .any(|argument| argument.len() > 2048 || argument.contains('\0'))
            {
                return Err(LibraryError::Invalid(
                    "Executable veya başlatma argümanları geçersiz".into(),
                ));
            }
            let working_dir = discovery::absolute_path(working_dir)?;
            if !working_dir.is_dir() {
                return Err(LibraryError::Invalid("Çalışma klasörü erişilemiyor".into()));
            }
            Ok(LaunchPlan::Process {
                executable,
                args: args.clone(),
                working_dir,
            })
        }
        (LaunchTarget::Steam { app_id }, Source::Steam) => Ok(LaunchPlan::Uri {
            uri: steam::launch_uri(*app_id)?,
        }),
        (
            LaunchTarget::Epic {
                namespace_id,
                catalog_id,
                app_name,
                executable,
            },
            Source::Epic,
        ) => {
            if !executable.is_empty() {
                installed_executable(install, executable)?;
            }
            if !discovery::identifier(app_name) {
                return Err(LibraryError::Invalid("Epic AppName geçersiz".into()));
            }
            let identifier = if namespace_id.is_empty() && catalog_id.is_empty() {
                app_name.clone()
            } else {
                if !discovery::identifier(namespace_id) || !discovery::identifier(catalog_id) {
                    return Err(LibraryError::Invalid(
                        "Epic namespace/catalog kimliği geçersiz".into(),
                    ));
                }
                format!("{namespace_id}%3A{catalog_id}%3A{app_name}")
            };
            Ok(LaunchPlan::Uri {
                uri: format!(
                    "com.epicgames.launcher://apps/{identifier}?action=launch&silent=true"
                ),
            })
        }
        (
            LaunchTarget::Riot {
                client_path,
                product,
                patchline,
                executable,
            },
            Source::Riot,
        ) => {
            installed_executable(install, executable)?;
            if riot::product(product).is_none() || !discovery::identifier(patchline) {
                return Err(LibraryError::Invalid(
                    "Riot ürün/patchline kimliği geçersiz".into(),
                ));
            }
            let client = discovery::absolute_path(client_path).map_err(|_| {
                LibraryError::Invalid(
                    "Riot Client bulunamadı. Riot Client kurulumunu kontrol edip yeniden tara."
                        .into(),
                )
            })?;
            if !client.is_file()
                || !client
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("RiotClientServices.exe"))
            {
                return Err(LibraryError::Invalid(
                    "Riot Client executable dosyası bulunamadı. Yeniden tara.".into(),
                ));
            }
            let working_dir = client
                .parent()
                .ok_or_else(|| LibraryError::Invalid("Riot Client klasörü geçersiz".into()))?
                .to_path_buf();
            Ok(LaunchPlan::Process {
                executable: client,
                args: vec![
                    format!("--launch-product={product}"),
                    format!("--launch-patchline={patchline}"),
                ],
                working_dir,
            })
        }
        _ => Err(LibraryError::Invalid(
            "Başlatma tanımı oyun kaynağıyla uyuşmuyor".into(),
        )),
    }
}
