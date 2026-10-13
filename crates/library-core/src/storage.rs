use crate::models::{
    Collection, CompletionTimes, Game, GameDetail, GamePage, LaunchTarget, LibraryStatus,
    PlaySession, ScanResult, Source, SourceRoot, SourceStatus,
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
        if version > 3 {
            return Err(LibraryError::Invalid(
                "Veritabanı bu uygulamadan daha yeni bir sürüm kullanıyor".into(),
            ));
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-2048; PRAGMA foreign_keys=ON;")?;
        if version == 3 {
            connection.execute_batch(
                "CREATE INDEX IF NOT EXISTS games_source_id ON games(source,source_id);",
            )?;
            return Ok(Self { connection });
        }
        if version == 2 {
            upgrade_user_data(&mut connection, path, true)?;
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
        upgrade_user_data(&mut connection, path, false)?;
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
        self.connection.execute_batch(
            "CREATE INDEX IF NOT EXISTS games_source_id ON games(source,source_id);",
        )?;
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
                installed=excluded.installed,size_bytes=excluded.size_bytes,artwork_path=coalesce(excluded.artwork_path,games.artwork_path),source=excluded.source,source_id=excluded.source_id,launch_json=excluded.launch_json")?;
            for game in &root.games {
                if game.source != root.source {
                    return Err(LibraryError::Invalid(
                        "Oyun ve tarama kaynağı uyuşmuyor".into(),
                    ));
                }
                let launch = serde_json::to_string(&game.launch)
                    .map_err(|error| LibraryError::Invalid(error.to_string()))?;
                let account_id: Option<String> = transaction.query_row("SELECT id FROM games WHERE source=?1 AND source_id=?2 AND (library_path LIKE 'account://%' OR id LIKE '%:account') LIMIT 1",params![game.source.as_str(),game.source_id],|row|row.get(0)).ok();
                let id = account_id.as_deref().unwrap_or(&game.id);
                insert.execute(params![
                    id,
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
        transaction.execute("DELETE FROM games WHERE source='steam' AND app_id=228980 AND favorite=0 AND NOT EXISTS(SELECT 1 FROM game_overrides o WHERE o.game_id=games.id) AND NOT EXISTS(SELECT 1 FROM collection_games c WHERE c.game_id=games.id)",[])?;
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

    pub fn needs_source_scan(&self) -> Result<bool> {
        Ok(self.setting("source_scan_version")?.as_deref() != Some("3"))
    }

    pub fn apply_source_scan(&mut self, result: &ScanResult) -> Result<()> {
        self.apply_scan(result)?;
        if !result.report.cancelled {
            self.connection.execute("INSERT INTO settings(key,value) VALUES('source_scan_version','3') ON CONFLICT(key) DO UPDATE SET value='3'",[])?;
        }
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
        self.list_filtered(query, filter, offset, limit, source, None)
    }

    pub fn list_filtered(
        &self,
        query: &str,
        filter: &str,
        offset: u32,
        limit: u32,
        source: Option<Source>,
        collection: Option<i64>,
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
        let predicate = "search_name LIKE ?1 ESCAPE '\\' AND (?2='all' OR (?2='installed' AND installed=1) OR (?2='favorites' AND favorite=1) OR (?2='favorites-installed' AND favorite=1 AND installed=1)) AND (?5 IS NULL OR source=?5) AND (?6 IS NULL OR id IN (SELECT game_id FROM collection_games WHERE collection_id=?6))";
        let total = self.connection.query_row(
            &format!("SELECT count(*) FROM library_games WHERE {predicate}"),
            params![
                query,
                filter,
                limit.clamp(1, 200),
                offset,
                source,
                collection
            ],
            |row| row.get::<_, u32>(0),
        )?;
        let mut statement = self.connection.prepare_cached(&format!("SELECT id,app_id,name,library_path,install_path,installed,favorite,size_bytes,artwork_path,source,source_id,launch_json FROM library_games WHERE {predicate} ORDER BY search_name,id LIMIT ?3 OFFSET ?4"))?;
        let games = statement
            .query_map(
                params![
                    query,
                    filter,
                    limit.clamp(1, 200),
                    offset,
                    source,
                    collection
                ],
                row_game,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(GamePage { games, total })
    }

    pub fn game(&self, id: &str) -> Result<Game> {
        self.connection.query_row("SELECT id,app_id,name,library_path,install_path,installed,favorite,size_bytes,artwork_path,source,source_id,launch_json FROM library_games WHERE id=?1", [id], row_game).map_err(Into::into)
    }

    pub fn apply_owned_games(&mut self, source: Source, games: &[Game]) -> Result<()> {
        self.connection.execute_batch(
            "CREATE INDEX IF NOT EXISTS games_source_id ON games(source,source_id);",
        )?;
        if !matches!(source, Source::Steam | Source::Epic) || games.len() > 20_000 {
            return Err(LibraryError::Invalid(
                "Hesap kütüphanesi sınırı veya kaynak geçersiz".into(),
            ));
        }
        let transaction = self.connection.transaction()?;
        for game in games {
            if game.source != source
                || !game.library_path.starts_with("account://")
                || game.installed
                || game.name.trim().is_empty()
                || game.name.len() > 512
            {
                return Err(LibraryError::Invalid("Hesap oyun kaydı geçersiz".into()));
            }
            if source == Source::Steam && game.app_id == 228980 {
                continue;
            }
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM games WHERE source=?1 AND source_id=?2)",
                params![source.as_str(), game.source_id],
                |row| row.get(0),
            )?;
            if !exists {
                let launch = serde_json::to_string(&game.launch)
                    .map_err(|error| LibraryError::Invalid(error.to_string()))?;
                transaction.execute("INSERT INTO games(id,app_id,name,search_name,library_path,install_path,installed,size_bytes,artwork_path,source,source_id,launch_json) VALUES(?1,?2,?3,?4,?5,'',0,0,NULL,?6,?7,?8)",params![game.id,game.app_id,game.name,game.name.to_lowercase(),game.library_path,source.as_str(),game.source_id,launch])?;
            }
        }
        transaction.commit()?;
        self.set_setting(
            &format!("account_sync:{}", source.as_str()),
            &format!("{} oyun · {}", games.len(), now_ms()),
        )?;
        Ok(())
    }

    pub fn save_completion(&self, id: &str, completion: &CompletionTimes) -> Result<()> {
        let game = self.game(id)?;
        if completion.hltb_id == 0
            || completion.name.is_empty()
            || completion.name.len() > 512
            || completion.name.contains('\0')
            || [
                completion.main_seconds,
                completion.extra_seconds,
                completion.completionist_seconds,
            ]
            .iter()
            .flatten()
            .any(|seconds| *seconds > 36_000_000)
        {
            return Err(LibraryError::Invalid("HowLongToBeat kaydı geçersiz".into()));
        }
        let value = serde_json::to_string(completion)
            .map_err(|error| LibraryError::Invalid(error.to_string()))?;
        self.set_metadata("hltb", game.source, &game.source_id, &value)
    }

    pub fn completion(&self, game: &Game) -> Result<Option<CompletionTimes>> {
        Ok(self
            .setting(&format!("hltb:{}:{}", game.source.as_str(), game.source_id))?
            .and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub fn artwork_url(&self, game: &Game) -> Result<Option<String>> {
        self.setting(&format!(
            "cover-url:{}:{}",
            game.source.as_str(),
            game.source_id
        ))
    }

    pub fn set_artwork_url(&self, source: Source, source_id: &str, url: &str) -> Result<()> {
        self.set_metadata("cover-url", source, source_id, url)
    }

    fn set_metadata(&self, kind: &str, source: Source, source_id: &str, value: &str) -> Result<()> {
        if source_id.is_empty()
            || source_id.len() > 4096
            || source_id.contains('\0')
            || value.len() > 16_384
            || value.contains('\0')
        {
            return Err(LibraryError::Invalid(
                "Oyun metadata boyut sınırını aşıyor".into(),
            ));
        }
        let key = format!("{kind}:{}:{source_id}", source.as_str());
        self.connection.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
        Ok(())
    }
    pub fn set_source_artwork(&self, id: &str, path: &str) -> Result<()> {
        if path.len() > 4096 || path.contains('\0') {
            return Err(LibraryError::Invalid("Kapak yolu geçersiz".into()));
        }
        self.connection.execute(
            "UPDATE games SET artwork_path=?2 WHERE id=?1",
            params![id, path],
        )?;
        Ok(())
    }

    pub fn add_manual(
        &mut self,
        executable: &Path,
        title: &str,
        args: Vec<String>,
    ) -> Result<String> {
        let game = crate::manual::game(executable, title, args)?;
        let id = game.id.clone();
        let launch = serde_json::to_string(&game.launch)
            .map_err(|error| LibraryError::Invalid(error.to_string()))?;
        self.connection.execute("INSERT INTO games(id,app_id,name,search_name,library_path,install_path,installed,size_bytes,artwork_path,source,source_id,launch_json) VALUES(?1,0,?2,?3,?4,?4,1,0,NULL,'manual',?5,?6) ON CONFLICT(id) DO UPDATE SET name=excluded.name,search_name=excluded.search_name,installed=1,launch_json=excluded.launch_json",params![id,game.name,game.name.to_lowercase(),game.install_path,game.source_id,launch])?;
        Ok(id)
    }

    pub fn set_override(
        &self,
        id: &str,
        title: Option<&str>,
        notes: &str,
        artwork: Option<&str>,
    ) -> Result<()> {
        self.game(id)?;
        if title.is_some_and(|title| {
            title.trim().is_empty() || title.len() > 512 || title.contains('\0')
        }) || notes.len() > 16_384
            || notes.contains('\0')
            || artwork.is_some_and(|path| path.len() > 4096 || path.contains('\0'))
        {
            return Err(LibraryError::Invalid(
                "Oyun düzeltmesi boyut veya karakter sınırını aşıyor".into(),
            ));
        }
        let search = title.map(|title| title.trim().to_lowercase());
        self.connection.execute("INSERT INTO game_overrides(game_id,title,search_title,notes,artwork_path) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(game_id) DO UPDATE SET title=excluded.title,search_title=excluded.search_title,notes=excluded.notes,artwork_path=excluded.artwork_path",
            params![id,title.map(str::trim),search,notes,artwork])?;
        Ok(())
    }

    pub fn set_details(&self, id: &str, title: &str, notes: &str) -> Result<()> {
        if title.trim().is_empty()
            || title.len() > 512
            || title.contains('\0')
            || notes.len() > 16_384
            || notes.contains('\0')
        {
            return Err(LibraryError::Invalid(
                "Oyun düzeltmesi boyut veya karakter sınırını aşıyor".into(),
            ));
        }
        let original: String =
            self.connection
                .query_row("SELECT name FROM games WHERE id=?1", [id], |row| row.get(0))?;
        let title = (title.trim() != original).then(|| title.trim());
        let search = title.map(str::to_lowercase);
        self.connection.execute("INSERT INTO game_overrides(game_id,title,search_title,notes) VALUES(?1,?2,?3,?4) ON CONFLICT(game_id) DO UPDATE SET title=excluded.title,search_title=excluded.search_title,notes=excluded.notes",
            params![id,title,search,notes])?;
        Ok(())
    }

    pub fn set_artwork_override(&self, id: &str, artwork: Option<&str>) -> Result<()> {
        self.game(id)?;
        if artwork.is_some_and(|path| path.len() > 4096 || path.contains('\0')) {
            return Err(LibraryError::Invalid("Kapak yolu geçersiz".into()));
        }
        self.connection.execute("INSERT INTO game_overrides(game_id,artwork_path) VALUES(?1,?2) ON CONFLICT(game_id) DO UPDATE SET artwork_path=excluded.artwork_path",
            params![id,artwork])?;
        Ok(())
    }

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut statement = self.connection.prepare("SELECT c.id,c.name,count(cg.game_id) FROM collections c LEFT JOIN collection_games cg ON c.id=cg.collection_id GROUP BY c.id ORDER BY c.name,c.id")?;
        let collections = statement
            .query_map([], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    games: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(collections)
    }

    pub fn create_collection(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() || name.len() > 120 || name.contains('\0') {
            return Err(LibraryError::Invalid(
                "Koleksiyon adı 1–120 karakter olmalı".into(),
            ));
        }
        self.connection
            .execute("INSERT OR IGNORE INTO collections(name) VALUES(?1)", [name])?;
        Ok(self.connection.query_row(
            "SELECT id FROM collections WHERE name=?1",
            [name],
            |row| row.get(0),
        )?)
    }

    pub fn collection_member(&self, collection: i64, game: &str, member: bool) -> Result<()> {
        self.game(game)?;
        if member {
            self.connection.execute(
                "INSERT OR IGNORE INTO collection_games(collection_id,game_id) VALUES(?1,?2)",
                params![collection, game],
            )?;
        } else {
            self.connection.execute(
                "DELETE FROM collection_games WHERE collection_id=?1 AND game_id=?2",
                params![collection, game],
            )?;
        }
        Ok(())
    }

    pub fn delete_collection(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM collections WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn detail(&self, id: &str) -> Result<GameDetail> {
        let game = self.game(id)?;
        use rusqlite::OptionalExtension;
        let notes = self
            .connection
            .query_row(
                "SELECT notes FROM game_overrides WHERE game_id=?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default();
        let mut memberships = self.connection.prepare("SELECT c.id,c.name,1 FROM collections c JOIN collection_games cg ON c.id=cg.collection_id WHERE cg.game_id=?1 ORDER BY c.name,c.id")?;
        let collections = memberships
            .query_map([id], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    games: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut history = self.connection.prepare("SELECT id,started_ms,ended_ms,status,pid FROM play_sessions WHERE game_id=?1 ORDER BY id DESC LIMIT 20")?;
        let sessions = history
            .query_map([id], |row| {
                Ok(PlaySession {
                    id: row.get(0)?,
                    started_ms: row.get(1)?,
                    ended_ms: row.get(2)?,
                    status: row.get(3)?,
                    pid: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(GameDetail {
            completion: self.completion(&game)?,
            game,
            notes,
            collections,
            sessions,
        })
    }

    pub fn session_start(&self, game: &str) -> Result<i64> {
        let active: u32 = self.connection.query_row(
            "SELECT count(*) FROM play_sessions WHERE ended_ms IS NULL",
            [],
            |row| row.get(0),
        )?;
        if active >= 8 {
            return Err(LibraryError::Invalid(
                "Aynı anda en fazla sekiz oyun oturumu izlenebilir".into(),
            ));
        }
        self.game(game)?;
        self.connection.execute(
            "INSERT INTO play_sessions(game_id,started_ms,status) VALUES(?1,?2,'requested')",
            params![game, now_ms()],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    pub fn tracking_executable(&self, id: &str) -> Result<Option<String>> {
        self.setting(&format!("tracking:{id}"))
    }

    pub fn set_tracking_executable(&self, id: &str, executable: &Path) -> Result<()> {
        let game = self.game(id)?;
        let root = std::fs::canonicalize(&game.install_path)?;
        let executable = std::fs::canonicalize(executable)?;
        if !executable.starts_with(root)
            || !executable.is_file()
            || !executable
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Err(LibraryError::Invalid(
                "İzlenecek oyun executable dosyası kurulum içinde olmalı".into(),
            ));
        }
        // Installation IDs can contain long paths, so this setting is written directly with a fixed prefix.
        self.connection.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("tracking:{id}"),executable.to_string_lossy()])?;
        Ok(())
    }

    pub fn session_update(
        &self,
        id: i64,
        status: &str,
        pid: Option<u32>,
        finished: bool,
    ) -> Result<()> {
        if ![
            "requested",
            "running",
            "ended",
            "failed",
            "unconfirmed",
            "interrupted",
        ]
        .contains(&status)
        {
            return Err(LibraryError::Invalid("Geçersiz oyun oturumu durumu".into()));
        }
        self.connection.execute(
            "UPDATE play_sessions SET status=?1,pid=coalesce(?2,pid),ended_ms=?3 WHERE id=?4",
            params![status, pid, finished.then(now_ms), id],
        )?;
        Ok(())
    }

    pub fn recover_sessions(&self) -> Result<()> {
        self.connection.execute(
            "UPDATE play_sessions SET status='interrupted',ended_ms=?1 WHERE ended_ms IS NULL",
            [now_ms()],
        )?;
        Ok(())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        if key.len() > 128 || value.len() > 16_384 {
            return Err(LibraryError::Invalid("Ayar boyut sınırını aşıyor".into()));
        }
        self.connection.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }

    pub fn friend_favorites(&self, account: &str) -> Result<Vec<String>> {
        validate_discord_id(account)?;
        let ids: Vec<String> = self
            .setting(&format!("discord_favorites:{account}"))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|_| LibraryError::Invalid("Arkadaş favorileri okunamadı".into()))?
            .unwrap_or_default();
        if ids.len() > 200 || ids.iter().any(|id| validate_discord_id(id).is_err()) {
            return Err(LibraryError::Invalid("Arkadaş favorileri geçersiz".into()));
        }
        Ok(ids)
    }

    pub fn set_friend_favorite(&self, account: &str, id: &str, favorite: bool) -> Result<()> {
        validate_discord_id(id)?;
        let mut ids = self.friend_favorites(account)?;
        ids.retain(|saved| saved != id);
        if favorite {
            if ids.len() >= 200 {
                return Err(LibraryError::Invalid(
                    "En fazla 200 arkadaş favorilere eklenebilir".into(),
                ));
            }
            ids.push(id.to_string());
        }
        ids.sort();
        ids.dedup();
        self.set_setting(
            &format!("discord_favorites:{account}"),
            &serde_json::to_string(&ids)
                .map_err(|error| LibraryError::Invalid(error.to_string()))?,
        )
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

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn validate_discord_id(id: &str) -> Result<()> {
    if !(15..=22).contains(&id.len())
        || !id.bytes().all(|byte| byte.is_ascii_digit())
        || !id.parse::<u64>().is_ok_and(|value| value > 0)
    {
        return Err(LibraryError::Invalid("Discord kimliği geçersiz".into()));
    }
    Ok(())
}

fn upgrade_user_data(connection: &mut Connection, path: &Path, backup: bool) -> Result<()> {
    if backup {
        let backup = path.with_file_name(format!("library.schema-2-backup-{}.db", now_ms()));
        connection.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
    }
    let transaction = connection.transaction()?;
    transaction.execute_batch("CREATE TABLE game_overrides(game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,title TEXT,search_title TEXT,notes TEXT NOT NULL DEFAULT '',artwork_path TEXT);
        CREATE TABLE collections(id INTEGER PRIMARY KEY,name TEXT NOT NULL UNIQUE);
        CREATE TABLE collection_games(collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,PRIMARY KEY(collection_id,game_id));
        CREATE TABLE play_sessions(id INTEGER PRIMARY KEY,game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,started_ms INTEGER NOT NULL,ended_ms INTEGER,status TEXT NOT NULL,pid INTEGER);
        CREATE INDEX sessions_game ON play_sessions(game_id,id);
        CREATE VIEW library_games AS SELECT g.id,g.app_id,coalesce(o.title,g.name) AS name,coalesce(o.search_title,g.search_name) AS search_name,g.library_path,g.install_path,g.installed,g.favorite,g.size_bytes,coalesce(o.artwork_path,g.artwork_path) AS artwork_path,g.source,g.source_id,g.launch_json FROM games g LEFT JOIN game_overrides o ON g.id=o.game_id;")?;
    transaction.pragma_update(None, "user_version", 3)?;
    transaction.commit()?;
    Ok(())
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
