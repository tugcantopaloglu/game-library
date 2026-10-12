use crate::LibraryWindow;
use platform_windows::resources::{report, ResourceSample, Sampler};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct Measurements {
    pub sampling: AtomicBool,
    pub running: AtomicBool,
    pub cancel: AtomicBool,
    pub exit_error: Mutex<Option<String>>,
    pub sampler: Mutex<Sampler>,
}

fn present(ui: &LibraryWindow, sample: &ResourceSample) {
    ui.set_ram(
        sample
            .private_working_set_mib
            .map(|value| format!("{value:.2} MiB"))
            .unwrap_or_else(|| "Bilinmiyor".into())
            .into(),
    );
    ui.set_cpu(
        sample
            .cpu_percent
            .map(|value| format!("%{value:.3}"))
            .unwrap_or_else(|| "Bilinmiyor".into())
            .into(),
    );
    ui.set_commit_size(format!("{:.2} MiB", sample.private_bytes_mib).into());
    if sample.partial {
        ui.set_notice("Ölçüm kısmi: bazı süreç sayaçları okunamadı.".into());
    }
}

pub fn sample(state: Arc<Measurements>, ui: slint::Weak<LibraryWindow>) {
    if state.sampling.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let result = state
            .sampler
            .lock()
            .map_err(|_| "Ölçüm kilidi alınamadı".to_owned())
            .and_then(|mut sampler| sampler.sample());
        state.sampling.store(false, Ordering::SeqCst);
        let _ = ui.upgrade_in_event_loop(move |ui| match result {
            Ok(sample) => present(&ui, &sample),
            Err(error) => ui.set_notice(error.into()),
        });
    });
}

pub fn lifecycle(state: Arc<Measurements>, ui: slint::Weak<LibraryWindow>, path: PathBuf) {
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            std::thread::sleep(Duration::from_secs(3));
            let mut sampler = Sampler::default();
            let mut cycles = Vec::with_capacity(50);
            for index in 0..50 {
                ui.upgrade_in_event_loop(|ui| {
                    crate::show(&ui);
                    ui.invoke_focus_idle();
                })
                .map_err(|error| error.to_string())?;
                std::thread::sleep(Duration::from_millis(500));
                let open = sampler.sample()?;
                ui.upgrade_in_event_loop(|ui| {
                    let _ = ui.hide();
                })
                .map_err(|error| error.to_string())?;
                std::thread::sleep(Duration::from_millis(500));
                let tray = sampler.sample()?;
                cycles.push(serde_json::json!({"cycle": index + 1, "open": open, "tray": tray}));
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let bytes = serde_json::to_vec_pretty(&serde_json::json!({"cycles": cycles, "build": if cfg!(debug_assertions) { "debug" } else { "release" }})).map_err(|error| error.to_string())?;
            std::fs::write(path, bytes).map_err(|error| error.to_string())?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Ok(mut failure) = state.exit_error.lock() {
                *failure = Some(error);
            }
        }
        let _ = slint::quit_event_loop();
    });
}

pub fn start(
    state: Arc<Measurements>,
    ui: slint::Weak<LibraryWindow>,
    directory: PathBuf,
    output: Option<PathBuf>,
    seconds: u32,
) -> Result<(), String> {
    if !(10..=120).contains(&seconds) {
        return Err("Ölçüm 10–120 saniye arasında olmalı.".into());
    }
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("Ölçüm zaten sürüyor.".into());
    }
    state.cancel.store(false, Ordering::Relaxed);
    let automatic = output.is_some();
    if !automatic {
        let _ = ui.upgrade_in_event_loop(|ui| {
            ui.set_measuring(true);
            ui.set_measurement_progress(0);
        });
    }
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            if automatic {
                std::thread::sleep(Duration::from_secs(8));
            }
            let mut sampler = Sampler::default();
            let mut samples = Vec::with_capacity(seconds as usize + 1);
            for index in 0..=seconds {
                if state.cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let sample = sampler.sample()?;
                if !automatic {
                    let displayed = sample.clone();
                    let _ = ui.upgrade_in_event_loop(move |ui| {
                        if ui.window().is_visible() && ui.get_section() == 2 {
                            present(&ui, &displayed);
                            ui.set_measurement_progress(index as i32);
                        }
                    });
                }
                samples.push(sample);
                if index < seconds {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_millis();
            let path = output.unwrap_or_else(|| {
                directory
                    .join("measurements")
                    .join(format!("native-{timestamp}.json"))
            });
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let measured = report(samples, seconds, path.to_string_lossy().into_owned());
            let interrupted = measured.interrupted;
            let bytes = serde_json::to_vec_pretty(&measured).map_err(|error| error.to_string())?;
            std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
            if !automatic {
                if interrupted {
                    let _=ui.upgrade_in_event_loop(|ui|ui.set_notice("Ölçüm kesintiye uğradı. Rapor kaydedildi; karşılaştırma için yeniden ölç.".into()));
                }
                let summary = format!(
                    "Ölçüm kaydedildi · RAM {} · CPU {}\n{}",
                    measured
                        .average_private_working_set_mib
                        .map(|value| format!("{value:.2} MiB"))
                        .unwrap_or_else(|| "bilinmiyor".into()),
                    measured
                        .average_cpu_percent
                        .map(|value| format!("%{value:.3}"))
                        .unwrap_or_else(|| "bilinmiyor".into()),
                    path.display()
                );
                let _ =
                    ui.upgrade_in_event_loop(move |ui| ui.set_measurement_summary(summary.into()));
            }
            Ok(())
        })();
        state.running.store(false, Ordering::SeqCst);
        if automatic {
            if let Err(error) = result {
                if let Ok(mut failure) = state.exit_error.lock() {
                    *failure = Some(error);
                }
            }
            let _ = slint::quit_event_loop();
        } else {
            let cancelled = state.cancel.load(Ordering::Relaxed);
            let _ = ui.upgrade_in_event_loop(move |ui| {
                ui.set_measuring(false);
                if let Err(error) = result {
                    ui.set_notice(error.into());
                } else if cancelled {
                    ui.set_measurement_summary("Ölçüm iptal edildi; rapor kaydedilmedi.".into());
                }
            });
        }
    });
    Ok(())
}
