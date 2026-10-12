use crate::models::{ScanReport, ScanResult, Source, SourceRoot};
use crate::{epic, riot, steam, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub fn normalize_root(source: Source, path: &Path) -> Result<PathBuf> {
    match source {
        Source::Steam => steam::normalize_root(path),
        Source::Epic => epic::normalize_root(path),
        Source::Riot => riot::normalize_root(path),
        Source::Manual => Err(crate::LibraryError::Invalid(
            "Elle oyun eklemek için executable seç.".into(),
        )),
    }
}

pub fn scan(roots: &[SourceRoot], cancel: &AtomicBool) -> ScanResult {
    let started = Instant::now();
    let configured = |source| {
        roots
            .iter()
            .filter(|root| root.source == source)
            .map(|root| root.path.clone())
            .collect::<Vec<_>>()
    };
    let mut result = steam::scan(&configured(Source::Steam), cancel);
    if !cancel.load(Ordering::Relaxed) {
        result.libraries.extend(epic::scan(
            &configured(Source::Epic),
            cancel,
            &mut result.report.warnings,
        ));
    }
    if !cancel.load(Ordering::Relaxed) {
        result.libraries.extend(riot::scan(
            &configured(Source::Riot),
            cancel,
            &mut result.report.warnings,
        ));
    }
    result.report = ScanReport {
        roots: result
            .libraries
            .iter()
            .map(|root| format!("{} · {}", root.source.label(), root.path))
            .collect(),
        found: result
            .libraries
            .iter()
            .map(|root| root.games.len() as u32)
            .sum(),
        warnings: result.report.warnings,
        duration_ms: started.elapsed().as_secs_f64() * 1000.0,
        cancelled: cancel.load(Ordering::Relaxed),
    };
    result
}
