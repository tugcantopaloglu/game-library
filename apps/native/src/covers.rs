//! Decode only the viewport; the UI and bounded cache share the same RGBA allocation.
use crate::LibraryWindow;
use slint::{Model, Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

const CACHE_BYTES: usize = 32 * 1024 * 1024;
type Pixels = SharedPixelBuffer<Rgba8Pixel>;
type Key = (String, u64, u128);

pub fn thumbnail(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let resized = if decoded.width() > 480 || decoded.height() > 720 {
        decoded.resize(480, 720, image::imageops::FilterType::Lanczos3)
    } else {
        decoded
    }
    .into_rgba8();
    Some((resized.width(), resized.height(), resized.into_raw()))
}

#[derive(Default)]
struct Cache {
    entries: HashMap<Key, Pixels>,
    order: VecDeque<Key>,
    bytes: usize,
}
impl Cache {
    fn retain(&mut self, paths: &HashSet<&str>) {
        self.entries.retain(|key, _| paths.contains(key.0.as_str()));
        self.order.retain(|key| self.entries.contains_key(key));
        self.bytes = self
            .entries
            .values()
            .map(|pixels| pixels.as_bytes().len())
            .sum();
    }
    fn get(&mut self, path: &str) -> Option<Pixels> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        let key = (path.to_owned(), metadata.len(), modified);
        if let Some(pixels) = self.entries.get(&key).cloned() {
            self.order.retain(|entry| entry != &key);
            self.order.push_back(key);
            return Some(pixels);
        }
        let (width, height, bytes) = thumbnail(path)?;
        let pixels = Pixels::clone_from_slice(&bytes, width, height);
        self.insert(key, pixels.clone());
        Some(pixels)
    }
    fn insert(&mut self, key: Key, pixels: Pixels) {
        // A changed file at the same path must not keep its previous decode alive.
        self.entries.retain(|entry, _| entry.0 != key.0);
        self.order.retain(|entry| self.entries.contains_key(entry));
        self.bytes = self
            .entries
            .values()
            .map(|pixels| pixels.as_bytes().len())
            .sum();
        let size = pixels.as_bytes().len();
        while self.bytes + size > CACHE_BYTES {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(pixels) = self.entries.remove(&old) {
                self.bytes -= pixels.as_bytes().len();
            }
        }
        if size <= CACHE_BYTES {
            self.bytes += size;
            self.order.push_back(key.clone());
            self.entries.insert(key, pixels);
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Slot {
    row: usize,
    column: usize,
    id: String,
    path: String,
}
struct Job {
    generation: u64,
    slots: Vec<Slot>,
}
#[derive(Default)]
struct Shared {
    job: Mutex<Option<Job>>,
    changed: Condvar,
    generation: AtomicU64,
    pending: AtomicU64,
    stopped: AtomicBool,
}

pub struct Controller {
    shared: Arc<Shared>,
    desired: RefCell<Vec<Slot>>,
}

/// Include each partially visible row, without keeping the rest of the page decoded.
pub fn visible_rows(first: i32, count: i32, total: usize) -> std::ops::Range<usize> {
    let start = (first.max(0) as usize).min(total);
    start..start.saturating_add(count.max(0) as usize).min(total)
}

impl Controller {
    pub fn start(ui: slint::Weak<LibraryWindow>) -> Self {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::spawn(move || {
            let mut cache = Cache::default();
            loop {
                let job = {
                    let mut next = worker.job.lock().unwrap();
                    while next.is_none() && !worker.stopped.load(Ordering::Relaxed) {
                        next = worker.changed.wait(next).unwrap();
                    }
                    if worker.stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    next.take().unwrap()
                };
                cache.retain(&job.slots.iter().map(|slot| slot.path.as_str()).collect());
                let mut decoded = Vec::new();
                for slot in job.slots {
                    if worker.generation.load(Ordering::Relaxed) != job.generation {
                        break;
                    }
                    let pixels = cache.get(&slot.path);
                    decoded.push((slot, pixels));
                }
                if worker.generation.load(Ordering::Relaxed) != job.generation {
                    continue;
                }
                let state = worker.clone();
                let _ = ui.upgrade_in_event_loop(move |ui| {
                    if state.generation.load(Ordering::Relaxed) != job.generation {
                        return;
                    }
                    let rows = ui.get_game_rows();
                    for (slot, pixels) in decoded {
                        let Some(row) = rows.row_data(slot.row) else {
                            continue;
                        };
                        let Some(mut tile) = row.row_data(slot.column) else {
                            continue;
                        };
                        if tile.id != slot.id || tile.artwork_path != slot.path {
                            continue;
                        }
                        tile.artwork = pixels.map(slint::Image::from_rgba8).unwrap_or_default();
                        tile.has_artwork = tile.artwork.size().width > 0;
                        tile.artwork_loaded = true;
                        if !tile.has_artwork { ui.invoke_cover_failed(tile.id.clone()); }
                        row.set_row_data(slot.column, tile);
                    }
                    state.pending.store(0, Ordering::Relaxed);
                    ui.set_covers_loading(false);
                    ui.invoke_covers_ready();
                });
            }
        });
        Self {
            shared,
            desired: RefCell::new(Vec::new()),
        }
    }

    pub fn update(&self, ui: &LibraryWindow) {
        let rows = ui.get_game_rows();
        let visible = if ui.get_library_visible() && ui.get_section() < 2 {
            visible_rows(
                ui.get_first_cover_row(),
                ui.get_visible_cover_rows(),
                rows.row_count(),
            )
        } else {
            0..0
        };
        let mut desired = Vec::new();
        let mut missing = false;
        for (index, row) in rows.iter().enumerate() {
            for column in 0..row.row_count() {
                let Some(mut tile) = row.row_data(column) else {
                    continue;
                };
                if visible.contains(&index) && !tile.artwork_path.is_empty() {
                    missing |= !tile.artwork_loaded;
                    desired.push(Slot {
                        row: index,
                        column,
                        id: tile.id.to_string(),
                        path: tile.artwork_path.to_string(),
                    });
                } else if tile.has_artwork || tile.artwork_loaded {
                    tile.artwork = Default::default();
                    tile.has_artwork = false;
                    tile.artwork_loaded = false;
                    row.set_row_data(column, tile);
                }
            }
        }
        if *self.desired.borrow() == desired
            && (!missing || self.shared.pending.load(Ordering::Relaxed) != 0)
        {
            if self.shared.pending.load(Ordering::Relaxed) == 0 {
                ui.set_covers_loading(false);
                ui.invoke_covers_ready();
            }
            return;
        }
        *self.desired.borrow_mut() = desired.clone();
        let generation = self.shared.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.shared.pending.store(generation, Ordering::Relaxed);
        ui.set_covers_loading(true);
        // One replaceable job: rapid scrolling never queues a page of stale decodes.
        *self.shared.job.lock().unwrap() = Some(Job {
            generation,
            slots: desired,
        });
        self.shared.changed.notify_one();
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Relaxed);
        self.shared.changed.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_quality_decodes_are_shared_and_the_cache_stays_within_budget() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("cover.png");
        let source = image::RgbaImage::from_fn(600, 900, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 127, 255])
        });
        source.save(&path).unwrap();
        let expected = image::DynamicImage::ImageRgba8(source)
            .resize(480, 720, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        let mut cache = Cache::default();
        let first = cache.get(path.to_str().unwrap()).unwrap();
        let second = cache.get(path.to_str().unwrap()).unwrap();
        assert_eq!((first.width(), first.height()), (480, 720));
        assert_eq!(first.as_bytes(), expected.as_raw());
        assert_eq!(first.as_bytes().as_ptr(), second.as_bytes().as_ptr());
        for index in 0..100 {
            cache.insert((index.to_string(), 1, 1), Pixels::new(480, 720));
        }
        assert!(cache.bytes <= CACHE_BYTES);
        assert_eq!(cache.entries.len(), 24);
        cache.retain(&HashSet::new());
        assert_eq!(cache.bytes, 0);
        // UI ownership survives eviction without another allocation or quality change.
        assert_eq!(first.as_bytes(), expected.as_raw());
    }

    #[test]
    fn a_viewport_never_decodes_the_entire_hundred_cover_page() {
        assert_eq!(visible_rows(0, 3, 25), 0..3);
        assert_eq!(visible_rows(23, 3, 25), 23..25);
        assert_eq!(visible_rows(-3, 0, 25), 0..0);
        let visible_bytes = visible_rows(0, 3, 25).len() * 4 * 480 * 720 * 4;
        assert_eq!(visible_bytes, 16_588_800);
        assert!(visible_bytes < CACHE_BYTES);
    }
}
