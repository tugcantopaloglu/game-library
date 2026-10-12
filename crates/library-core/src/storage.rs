use crate::models::{Game, GamePage, LibraryStatus, ScanResult};
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
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-2048; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY, app_id INTEGER NOT NULL, name TEXT NOT NULL, search_name TEXT NOT NULL,
                library_path TEXT NOT NULL, install_path TEXT NOT NULL, installed INTEGER NOT NULL,
                favorite INTEGER NOT NULL DEFAULT 0, size_bytes REAL NOT NULL, artwork_path TEXT
            );
            CREATE INDEX IF NOT EXISTS games_sort ON games(search_name, id);
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            PRAGMA user_version=1;")?;
        Ok(Self { connection })
    }

    pub fn roots(&self) -> Result<Vec<String>> {
        Ok(self
            .setting("roots")?
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default())
    }

    pub fn add_root(&self, path: &str) -> Result<()> {
        let mut roots = self.roots()?;
        if !roots.iter().any(|root| root.eq_ignore_ascii_case(path)) {
            roots.push(path.to_owned());
        }
        self.set_setting(
            "roots",
            &serde_json::to_string(&roots)
                .map_err(|error| LibraryError::Invalid(error.to_string()))?,
        )
    }

    pub fn apply_scan(&mut self, result: &ScanResult) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for root in &result.libraries {
            if root.complete {
                transaction.execute(
                    "UPDATE games SET installed=0 WHERE library_path=?1",
                    [&root.path],
                )?;
            }
            let mut insert = transaction.prepare_cached("INSERT INTO games (id,app_id,name,search_name,library_path,install_path,installed,size_bytes,artwork_path)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,search_name=excluded.search_name,install_path=excluded.install_path,
                installed=excluded.installed,size_bytes=excluded.size_bytes,artwork_path=excluded.artwork_path")?;
            for game in &root.games {
                insert.execute(params![
                    game.id,
                    game.app_id,
                    game.name,
                    game.name.to_lowercase(),
                    game.library_path,
                    game.install_path,
                    game.installed,
                    game.size_bytes,
                    game.artwork_path
                ])?;
            }
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        for (key, value) in [
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
        let predicate = "search_name LIKE ?1 ESCAPE '\\' AND (?2='all' OR (?2='installed' AND installed=1) OR (?2='favorites' AND favorite=1) OR (?2='favorites-installed' AND favorite=1 AND installed=1))";
        let total = self.connection.query_row(
            &format!("SELECT count(*) FROM games WHERE {predicate}"),
            params![query, filter],
            |row| row.get::<_, u32>(0),
        )?;
        let mut statement = self.connection.prepare_cached(&format!("SELECT id,app_id,name,library_path,install_path,installed,favorite,size_bytes,artwork_path FROM games WHERE {predicate} ORDER BY search_name,id LIMIT ?3 OFFSET ?4"))?;
        let games = statement
            .query_map(
                params![query, filter, limit.clamp(1, 200), offset],
                row_game,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(GamePage { games, total })
    }

    pub fn game(&self, id: &str) -> Result<Game> {
        self.connection.query_row("SELECT id,app_id,name,library_path,install_path,installed,favorite,size_bytes,artwork_path FROM games WHERE id=?1", [id], row_game).map_err(Into::into)
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
        Ok(LibraryStatus {
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

    fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
        Ok(())
    }
}

fn row_game(row: &rusqlite::Row<'_>) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get(0)?,
        app_id: row.get(1)?,
        name: row.get(2)?,
        library_path: row.get(3)?,
        install_path: row.get(4)?,
        installed: row.get(5)?,
        favorite: row.get(6)?,
        size_bytes: row.get(7)?,
        artwork_path: row.get(8)?,
    })
}
