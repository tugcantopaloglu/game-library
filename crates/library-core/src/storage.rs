use crate::models::{
    Game, GamePage, LaunchTarget, LibraryStatus, ScanResult, Source, SourceRoot, SourceStatus,
};
use crate::{LibraryError, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > 2 {
            return Err(LibraryError::Invalid(
                "Veritabanı bu uygulamadan daha yeni bir sürüm kullanıyor".into(),
            ));
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-2048; PRAGMA foreign_keys=ON;")?;
        if version == 2 {
            return Ok(Self { connection });
        }
        if version == 1 {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let backup = path.with_file_name(format!("library.schema-1-backup-{stamp}.db"));
            connection.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-2048; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY, app_id INTEGER NOT NULL, name TEXT NOT NULL, search_name TEXT NOT NULL,
                library_path TEXT NOT NULL, install_path TEXT NOT NULL, installed INTEGER NOT NULL,
                favorite INTEGER NOT NULL DEFAULT 0, size_bytes REAL NOT NULL, artwork_path TEXT
            );
            CREATE INDEX IF NOT EXISTS games_sort ON games(search_name, id);
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            ")?;
        let transaction = connection.transaction()?;
        transaction.execute_batch("ALTER TABLE games ADD COLUMN source TEXT NOT NULL DEFAULT 'steam';
            ALTER TABLE games ADD COLUMN source_id TEXT NOT NULL DEFAULT '';
            ALTER TABLE games ADD COLUMN launch_json TEXT NOT NULL DEFAULT '';
            UPDATE games SET source_id=CAST(app_id AS TEXT),launch_json=json_object('kind','steam','appId',app_id);
            CREATE INDEX games_source_sort ON games(source,search_name,id);
            CREATE TABLE source_roots(source TEXT NOT NULL,path TEXT NOT NULL,path_key TEXT NOT NULL,PRIMARY KEY(source,path_key));")?;
        let roots =
            transaction.query_row("SELECT value FROM settings WHERE key='roots'", [], |row| {
                row.get::<_, String>(0)
            });
        match roots {
            Ok(roots) => {
                let roots: Vec<String> = serde_json::from_str(&roots).map_err(|error| {
                    LibraryError::Invalid(format!("Steam kaynakları taşınamadı: {error}"))
                })?;
                for root in roots {
                    transaction.execute(
                        "INSERT OR IGNORE INTO source_roots VALUES('steam',?1,?2)",
                        params![root, root.to_lowercase()],
                    )?;
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(error) => return Err(error.into()),
        }
        transaction.pragma_update(None, "user_version", 2)?;
        transaction.commit()?;
        Ok(Self { connection })
    }

    pub fn roots(&self) -> Result<Vec<String>> {
        Ok(self
            .source_roots()?
            .into_iter()
            .filter(|root| root.source == Source::Steam)
            .map(|root| root.path)
            .collect())
    }

    pub fn add_root(&self, path: &str) -> Result<()> {
        self.add_source_root(Source::Steam, path)
    }

    pub fn source_roots(&self) -> Result<Vec<SourceRoot>> {
        let mut statement = self
            .connection
            .prepare("SELECT source,path FROM source_roots ORDER BY source,path_key")?;
        let roots = statement
            .query_map([], |row| {
                let source: String = row.get(0)?;
                let source = decode_source(&source)?;
                Ok(SourceRoot {
                    source,
                    path: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(roots)
    }

    pub fn add_source_root(&self, source: Source, path: &str) -> Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO source_roots VALUES(?1,?2,?3)",
            params![source.as_str(), path, path.to_lowercase()],
        )?;
        Ok(())
    }

    pub fn apply_scan(&mut self, result: &ScanResult) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for root in &result.libraries {
            if root.complete {
                transaction.execute(
                    "UPDATE games SET installed=0 WHERE source=?1 AND library_path=?2",
                    params![root.source.as_str(), root.path],
                )?;
            }
            let mut insert = transaction.prepare_cached("INSERT INTO games (id,app_id,name,search_name,library_path,install_path,installed,size_bytes,artwork_path,source,source_id,launch_json)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,search_name=excluded.search_name,install_path=excluded.install_path,library_path=excluded.library_path,
                installed=excluded.installed,size_bytes=excluded.size_bytes,artwork_path=excluded.artwork_path,source=excluded.source,source_id=excluded.source_id,launch_json=excluded.launch_json")?;
            for game in &root.games {
                if game.source != root.source {
                    return Err(LibraryError::Invalid(
                        "Oyun ve tarama kaynağı uyuşmuyor".into(),
                    ));
                }
                let launch = serde_json::to_string(&game.launch)
                    .map_err(|error| LibraryError::Invalid(error.to_string()))?;
                insert.execute(params![
                    game.id,
                    game.app_id,
                    game.name,
                    game.name.to_lowercase(),
                    game.library_path,
                    game.install_path,
                    game.installed,
                    game.size_bytes,
                    game.artwork_path,
                    game.source.as_str(),
                    game.source_id,
                    launch
                ])?;
            }
        }
        let mut discovered: Vec<SourceRoot> = transaction
            .query_row(
                "SELECT value FROM settings WHERE key='discovered_sources'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        for root in &result.libraries {
            if !discovered.iter().any(|saved| {
                saved.source == root.source && saved.path.eq_ignore_ascii_case(&root.path)
            }) {
                discovered.push(SourceRoot {
                    source: root.source,
                    path: root.path.clone(),
                });
            }
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        for (key, value) in [
            (
                "discovered_sources",
                serde_json::to_string(&discovered)
                    .map_err(|error| LibraryError::Invalid(error.to_string()))?,
            ),
            ("last_scan_unix_ms", now),
            (
                "last_scan_duration_ms",
                result.report.duration_ms.to_string(),
            ),
            (
                "warnings",
                serde_json::to_string(&result.report.warnings).unwrap_or_default(),
            ),
            (
                "discovered_roots",
                serde_json::to_string(&result.report.roots).unwrap_or_default(),
            ),
        ] {
            transaction.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list(&self, query: &str, filter: &str, offset: u32, limit: u32) -> Result<GamePage> {
        self.list_for_source(query, filter, offset, limit, None)
    }

    pub fn list_for_source(
        &self,
        query: &str,
        filter: &str,
        offset: u32,
        limit: u32,
        source: Option<Source>,
    ) -> Result<GamePage> {
        let query = format!(
            "%{}%",
            query
                .to_lowercase()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let filter = match filter {
            "all" | "installed" | "favorites" | "favorites-installed" => filter,
            _ => return Err(LibraryError::Invalid("Geçersiz kütüphane filtresi".into())),
        };
        let source = source.map(Source::as_str);
        let predicate = "search_name LIKE ?1 ESCAPE '\\' AND (?2='all' OR (?2='installed' AND installed=1) OR (?2='favorites' AND favorite=1) OR (?2='favorites-installed' AND favorite=1 AND installed=1)) AND (?5 IS NULL OR source=?5)";
        let total = self.connection.query_row(
            &format!("SELECT count(*) FROM games WHERE {predicate}"),
            params![query, filter, limit.clamp(1, 200), offset, source],
            |row| row.get::<_, u32>(0),
        )?;
        let mut statement = self.connection.prepare_cached(&format!("SELECT id,app_id,name,library_path,install_path,installed,favorite,size_bytes,artwork_path,source,source_id,launch_json FROM games WHERE {predicate} ORDER BY search_name,id LIMIT ?3 OFFSET ?4"))?;
        let games = statement
            .query_map(
                params![query, filter, limit.clamp(1, 200), offset, source],
                row_game,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(GamePage { games, total })
    }

    pub fn game(&self, id: &str) -> Result<Game> {
        self.connection.query_row("SELECT id,app_id,name,library_path,install_path,installed,favorite,size_bytes,artwork_path,source,source_id,launch_json FROM games WHERE id=?1", [id], row_game).map_err(Into::into)
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<()> {
        if self.connection.execute(
            "UPDATE games SET favorite=?1 WHERE id=?2",
            params![favorite, id],
        )? == 0
        {
            return Err(LibraryError::Invalid("Oyun kaydı bulunamadı".into()));
        }
        Ok(())
    }

    pub fn status(&self) -> Result<LibraryStatus> {
        let (total, installed, favorites) = self.connection.query_row(
            "SELECT count(*),coalesce(sum(installed),0),coalesce(sum(favorite),0) FROM games",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let mut roots = self.roots()?;
        let discovered: Vec<String> = self
            .setting("discovered_roots")?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        for root in discovered {
            if !roots.iter().any(|saved| saved.eq_ignore_ascii_case(&root)) {
                roots.push(root);
            }
        }
        let mut all_roots = self.source_roots()?;
        let mut known = self
            .connection
            .prepare("SELECT DISTINCT source,library_path FROM games")?;
        all_roots.extend(
            known
                .query_map([], |row| {
                    Ok(SourceRoot {
                        source: decode_source(&row.get::<_, String>(0)?)?,
                        path: row.get(1)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        );
        let discovered: Vec<SourceRoot> = self
            .setting("discovered_sources")?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        all_roots.extend(discovered);
        let mut sources = Vec::new();
        for source in Source::ALL {
            let (total, installed) = self.connection.query_row(
                "SELECT count(*),coalesce(sum(installed),0) FROM games WHERE source=?1",
                [source.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let mut source_paths = Vec::new();
            for root in all_roots.iter().filter(|root| root.source == source) {
                if !source_paths
                    .iter()
                    .any(|path: &String| path.eq_ignore_ascii_case(&root.path))
                {
                    source_paths.push(root.path.clone());
                }
            }
            sources.push(SourceStatus {
                source,
                total,
                installed,
                roots: source_paths,
            });
        }
        Ok(LibraryStatus {
            sources,
            total,
            installed,
            favorites,
            roots,
            warnings: self
                .setting("warnings")?
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_default(),
            last_scan_unix_ms: self
                .setting("last_scan_unix_ms")?
                .and_then(|value| value.parse().ok()),
            last_scan_duration_ms: self
                .setting("last_scan_duration_ms")?
                .and_then(|value| value.parse().ok()),
        })
    }

    fn setting(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }
}

fn decode_source(value: &str) -> rusqlite::Result<Source> {
    Source::parse(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn row_game(row: &rusqlite::Row<'_>) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get(0)?,
        app_id: row.get(1)?,
        source: decode_source(&row.get::<_, String>(9)?)?,
        source_id: row.get(10)?,
        launch: serde_json::from_str::<LaunchTarget>(&row.get::<_, String>(11)?).map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            },
        )?,
        name: row.get(2)?,
        library_path: row.get(3)?,
        install_path: row.get(4)?,
        installed: row.get(5)?,
        favorite: row.get(6)?,
        size_bytes: row.get(7)?,
        artwork_path: row.get(8)?,
    })
}
