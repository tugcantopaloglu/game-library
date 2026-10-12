use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub app_id: u32,
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
    pub path: String,
    pub games: Vec<Game>,
    pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub report: ScanReport,
    pub libraries: Vec<RootScan>,
}
