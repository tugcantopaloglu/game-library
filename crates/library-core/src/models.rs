use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Riot,
    Manual,
}

impl Source {
    pub const ALL: [Self; 4] = [Self::Steam, Self::Epic, Self::Riot, Self::Manual];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Steam => "steam",
            Self::Epic => "epic",
            Self::Riot => "riot",
            Self::Manual => "manual",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Steam => "Steam",
            Self::Epic => "Epic Games",
            Self::Riot => "Riot Games",
            Self::Manual => "Elle eklenen",
        }
    }
    pub fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "steam" => Ok(Self::Steam),
            "epic" => Ok(Self::Epic),
            "riot" => Ok(Self::Riot),
            "manual" => Ok(Self::Manual),
            _ => Err(crate::LibraryError::Invalid("Geçersiz oyun kaynağı".into())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LaunchTarget {
    Steam {
        app_id: u32,
    },
    Epic {
        namespace_id: String,
        catalog_id: String,
        app_name: String,
        executable: String,
    },
    Riot {
        client_path: String,
        product: String,
        patchline: String,
        executable: String,
    },
    Manual {
        executable: String,
        args: Vec<String>,
        working_dir: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub games: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaySession {
    pub id: i64,
    pub started_ms: i64,
    pub ended_ms: Option<i64>,
    pub status: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    pub game: Game,
    pub notes: String,
    pub collections: Vec<Collection>,
    pub sessions: Vec<PlaySession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceRoot {
    pub source: Source,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub source: Source,
    pub total: u32,
    pub installed: u32,
    pub roots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub app_id: u32,
    pub source: Source,
    pub source_id: String,
    pub launch: LaunchTarget,
    pub name: String,
    pub library_path: String,
    pub install_path: String,
    pub installed: bool,
    pub favorite: bool,
    pub size_bytes: f64,
    pub artwork_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GamePage {
    pub games: Vec<Game>,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub sources: Vec<SourceStatus>,
    pub total: u32,
    pub installed: u32,
    pub favorites: u32,
    pub roots: Vec<String>,
    pub warnings: Vec<String>,
    pub last_scan_unix_ms: Option<f64>,
    pub last_scan_duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub roots: Vec<String>,
    pub found: u32,
    pub warnings: Vec<String>,
    pub duration_ms: f64,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
pub struct RootScan {
    pub source: Source,
    pub path: String,
    pub games: Vec<Game>,
    pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub report: ScanReport,
    pub libraries: Vec<RootScan>,
}
