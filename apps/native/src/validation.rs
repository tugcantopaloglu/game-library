use crate::LibraryWindow;
use library_core::storage::Store;
use serde_json::json;
use slint::ComponentHandle;
use std::path::Path;
use std::time::Instant;

pub fn search_benchmark(store: &Store, output: &Path) -> Result<(), String> {
    let mut samples = Vec::new();
    for index in 0..100 {
        let query = if index % 3 == 0 {
            "9999"
        } else if index % 3 == 1 {
            "Fixture Game"
        } else {
            "no-such-title"
        };
        let start = Instant::now();
        let page = store
            .list(query, "all", 0, 100)
            .map_err(|error| error.to_string())?;
        samples.push((start.elapsed().as_secs_f64() * 1000.0, page.total));
    }
    let mut times: Vec<f64> = samples.iter().map(|sample| sample.0).collect();
    times.sort_by(f64::total_cmp);
    std::fs::write(output,serde_json::to_vec_pretty(&json!({"games":store.status().map_err(|error|error.to_string())?.total,"runs":samples.len(),"searchRustP50Ms":times[49],"searchRustP95Ms":times[94],"searchRustMaxMs":times[99],"includesUiOrIpc":false})).map_err(|error|error.to_string())?).map_err(|error|error.to_string())
}

pub fn screenshot(ui: &LibraryWindow, output: &Path) -> Result<(), String> {
    let pixels = ui
        .window()
        .take_snapshot()
        .map_err(|error| error.to_string())?;
    image::save_buffer(
        output,
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
    )
    .map_err(|error| error.to_string())
}

/// Isolated profile acceptance: real Slint scrolling, cancellation, hide and restore.
pub fn cover_lifecycle(ui: slint::Weak<LibraryWindow>, output: std::path::PathBuf) {
    use slint::{Model, Timer, TimerMode};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;
    let timer = Rc::new(Timer::default());
    let ticking = timer.clone();
    let phase = Cell::new(0);
    let samples = RefCell::new(Vec::new());
    let started = Instant::now();
    let changed = Cell::new(Instant::now());
    timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
        let Some(ui) = ui.upgrade() else { ticking.stop(); return; };
        if started.elapsed() > Duration::from_secs(40) {
            let _ = std::fs::write(&output, b"{\"passed\":false,\"reason\":\"timeout\"}");
            ticking.stop(); let _ = slint::quit_event_loop(); return;
        }
        if ui.get_loading() || ui.get_covers_loading() || changed.get().elapsed() < Duration::from_millis(300) { return; }
        let rows = ui.get_game_rows();
        let range = crate::covers::visible_rows(ui.get_first_cover_row(), ui.get_visible_cover_rows(), rows.row_count());
        let mut count = 0;
        let mut bytes = 0;
        let mut outside = 0;
        let mut full_quality = true;
        for (index, row) in rows.iter().enumerate() {
            for tile in row.iter() {
                if !tile.has_artwork { continue; }
                count += 1;
                let size = tile.artwork.size();
                bytes += u64::from(size.width) * u64::from(size.height) * 4;
                full_quality &= size.width == 480 && size.height == 720;
                if !range.contains(&index) || !ui.get_library_visible() { outside += 1; }
            }
        }
        samples.borrow_mut().push(json!({"phase":phase.get(),"firstRow":range.start,"rows":range.len(),"covers":count,"rgbaBytes":bytes,"outsideViewport":outside,"all480x720":full_quality}));
        match phase.get() {
            0 => ui.invoke_validation_scroll(rows.row_count().saturating_sub(1) as i32),
            1 => ui.invoke_validation_scroll(0),
            2 => {
                // Replace queued work before it can publish into the final viewport.
                ui.invoke_validation_scroll((rows.row_count() / 2) as i32);
                ui.invoke_validation_scroll(rows.row_count().saturating_sub(1) as i32);
            }
            3 => ui.invoke_hide_window(),
            4 => { ui.set_library_visible(true); let _ = ui.show(); ui.invoke_cover_viewport(); }
            _ => {
                let samples = samples.borrow();
                let passed = samples.iter().all(|sample| sample["outsideViewport"] == 0 && sample["all480x720"] == true)
                    && samples[0]["covers"].as_u64().unwrap_or(0) > 0
                    && samples[1]["firstRow"].as_u64().unwrap_or(0) > 0
                    && samples[4]["covers"] == 0
                    && count > 0;
                let _ = screenshot(&ui, &output.with_extension("png"));
                let _ = std::fs::write(&output, serde_json::to_vec_pretty(&json!({"passed":passed,"samples":*samples,"previousFullPageRgbaBytes":100*480*720*4})).unwrap_or_default());
                ticking.stop(); let _ = slint::quit_event_loop(); return;
            }
        }
        phase.set(phase.get() + 1);
        changed.set(Instant::now());
    });
}

pub fn stability(output: std::path::PathBuf, seconds: u32) -> Result<(), String> {
    if !(60..=28_800).contains(&seconds) {
        return Err("Kararlılık denemesi 60 saniye ile sekiz saat arasında olmalı".into());
    }
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            std::thread::sleep(std::time::Duration::from_secs(8));
            let start = Instant::now();
            let mut sampler = platform_windows::resources::Sampler::default();
            let mut samples = Vec::new();
            let mut interrupted = false;
            loop {
                let sample = sampler.sample()?;
                if let Some(previous) = samples.last() {
                    let previous: &platform_windows::resources::ResourceSample = previous;
                    if sample.elapsed_ms - previous.elapsed_ms > 90_000.0 {
                        interrupted = true;
                    }
                }
                samples.push(sample);
                let completed = start.elapsed().as_secs() >= u64::from(seconds);
                let report = json!({"kind":"long-idle-stability","targetSeconds":seconds,"elapsedSeconds":start.elapsed().as_secs_f64(),"completed":completed,"interrupted":interrupted,"sampleCadenceSeconds":60,"pid":std::process::id(),"build":if cfg!(debug_assertions){"debug"}else{"release"},"samples":samples});
                let temporary = output.with_extension("part");
                std::fs::write(
                    &temporary,
                    serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                std::fs::rename(&temporary, &output).map_err(|error| error.to_string())?;
                if completed {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(
                    60.min(u64::from(seconds).saturating_sub(start.elapsed().as_secs())),
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = std::fs::write(output.with_extension("error.txt"), error);
        }
        let _ = slint::quit_event_loop();
    });
    Ok(())
}

pub fn launch_acceptance(database: std::path::PathBuf, id: String, output: std::path::PathBuf) {
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let store = Store::open(&database).map_err(|error| error.to_string())?;
            let start = Instant::now();
            loop {
                let detail = store.detail(&id).map_err(|error| error.to_string())?;
                let status = detail
                    .sessions
                    .first()
                    .map(|session| session.status.as_str())
                    .unwrap_or("waiting");
                if status == "running"
                    || status == "failed"
                    || status == "unconfirmed"
                    || start.elapsed().as_secs() >= 70
                {
                    std::fs::write(&output,serde_json::to_vec_pretty(&json!({"kind":"real-game-launch-acceptance","game":detail.game.name,"source":detail.game.source,"processObserved":status == "running","elapsedSeconds":start.elapsed().as_secs_f64(),"sessions":detail.sessions})).map_err(|error|error.to_string())?).map_err(|error|error.to_string())?;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = std::fs::write(output.with_extension("error.txt"), error);
        }
        let _ = slint::quit_event_loop();
    });
}
