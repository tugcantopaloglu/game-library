use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use ts_rs::TS;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    PROCESS_MEMORY_COUNTERS_EX2,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSample {
    pub pid: u32,
    pub name: String,
    pub working_set_mib: f64,
    pub private_working_set_mib: Option<f64>,
    pub private_bytes_mib: f64,
    pub cpu_time_ms: f64,
    pub creation_time: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSample {
    pub elapsed_ms: f64,
    pub working_set_mib: f64,
    pub private_working_set_mib: Option<f64>,
    pub private_bytes_mib: f64,
    pub cpu_percent: Option<f64>,
    pub process_count: u32,
    pub partial: bool,
    pub processes: Vec<ProcessSample>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementReport {
    pub duration_seconds: u32,
    pub measured_duration_seconds: f64,
    pub max_sample_gap_ms: f64,
    pub interrupted: bool,
    pub sample_count: u32,
    pub average_cpu_percent: Option<f64>,
    pub average_private_working_set_mib: Option<f64>,
    pub peak_private_working_set_mib: Option<f64>,
    pub average_working_set_mib: f64,
    pub average_private_bytes_mib: f64,
    pub partial: bool,
    pub logical_processors: u32,
    pub build: String,
    pub saved_path: String,
    pub samples: Vec<ResourceSample>,
}

pub struct Sampler {
    started: Instant,
    previous_at: Option<Instant>,
    previous: HashMap<(u32, String), f64>,
    tree_updated_at: Option<Instant>,
    process_tree: Vec<(u32, String)>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            previous_at: None,
            previous: HashMap::new(),
            tree_updated_at: None,
            process_tree: Vec::new(),
        }
    }
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}
fn mib(bytes: usize) -> f64 {
    bytes as f64 / 1_048_576.0
}

fn process_sample(pid: u32, name: String) -> Option<ProcessSample> {
    unsafe {
        let handle =
            OwnedHandle(OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?);
        let mut memory = PROCESS_MEMORY_COUNTERS_EX2 {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32,
            ..Default::default()
        };
        let private = if GetProcessMemoryInfo(
            handle.0,
            &mut memory as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            memory.cb,
        )
        .is_ok()
        {
            Some(mib(memory.PrivateWorkingSetSize))
        } else {
            let mut basic = PROCESS_MEMORY_COUNTERS_EX {
                cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
                ..Default::default()
            };
            GetProcessMemoryInfo(
                handle.0,
                &mut basic as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
                basic.cb,
            )
            .ok()?;
            memory.WorkingSetSize = basic.WorkingSetSize;
            memory.PrivateUsage = basic.PrivateUsage;
            None
        };
        let (mut creation, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        GetProcessTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user).ok()?;
        Some(ProcessSample {
            pid,
            name,
            working_set_mib: mib(memory.WorkingSetSize),
            private_working_set_mib: private,
            private_bytes_mib: mib(memory.PrivateUsage),
            cpu_time_ms: (ticks(kernel) + ticks(user)) as f64 / 10_000.0,
            creation_time: ticks(creation).to_string(),
        })
    }
}

fn process_tree() -> std::result::Result<Vec<(u32, String)>, String> {
    let mut entries = Vec::new();
    unsafe {
        let snapshot = OwnedHandle(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|error| error.to_string())?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot.0, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(entry.szExeFile.len());
                entries.push((
                    entry.th32ProcessID,
                    entry.th32ParentProcessID,
                    String::from_utf16_lossy(&entry.szExeFile[..end]),
                ));
                if Process32NextW(snapshot.0, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
    let root = std::process::id();
    let mut owned = HashSet::from([root]);
    loop {
        let before = owned.len();
        for (pid, parent, name) in &entries {
            if owned.contains(parent) && name.eq_ignore_ascii_case("msedgewebview2.exe") {
                owned.insert(*pid);
            }
        }
        if before == owned.len() {
            break;
        }
    }
    Ok(entries
        .into_iter()
        .filter(|(pid, _, _)| owned.contains(pid))
        .map(|(pid, _, name)| (pid, name))
        .collect())
}

fn read_tree(tree: &[(u32, String)]) -> (Vec<ProcessSample>, bool) {
    let mut processes = Vec::with_capacity(tree.len());
    let mut partial = false;
    for (pid, name) in tree {
        match process_sample(*pid, name.clone()) {
            Some(process) => processes.push(process),
            None => partial = true,
        }
    }
    (processes, partial)
}

impl Sampler {
    pub fn sample(&mut self) -> std::result::Result<ResourceSample, String> {
        let now = Instant::now();
        if self
            .tree_updated_at
            .is_none_or(|previous| now.duration_since(previous).as_secs() >= 5)
        {
            self.process_tree = process_tree()?;
            self.tree_updated_at = Some(now);
        }
        let (mut processes, mut partial) = read_tree(&self.process_tree);
        if partial {
            self.process_tree = process_tree()?;
            self.tree_updated_at = Some(now);
            (processes, partial) = read_tree(&self.process_tree);
        }
        let root = std::process::id();
        if !processes.iter().any(|process| process.pid == root) {
            return Err("Ana uygulama süreci ölçülemedi".into());
        }
        processes.sort_by_key(|process| process.pid);
        let current: HashMap<_, _> = processes
            .iter()
            .map(|process| {
                (
                    (process.pid, process.creation_time.clone()),
                    process.cpu_time_ms,
                )
            })
            .collect();
        let cpu = self.previous_at.map(|at| {
            let delta_ms: f64 = current
                .iter()
                .filter_map(|(key, time)| {
                    self.previous
                        .get(key)
                        .map(|previous| (time - previous).max(0.0))
                })
                .sum();
            delta_ms
                / now.duration_since(at).as_secs_f64().max(0.001)
                / 1000.0
                / logical_processors() as f64
                * 100.0
        });
        self.previous_at = Some(now);
        self.previous = current;
        Ok(ResourceSample {
            elapsed_ms: now.duration_since(self.started).as_secs_f64() * 1000.0,
            working_set_mib: processes
                .iter()
                .map(|process| process.working_set_mib)
                .sum(),
            private_working_set_mib: processes
                .iter()
                .map(|process| process.private_working_set_mib)
                .collect::<Option<Vec<_>>>()
                .map(|values| values.iter().sum()),
            private_bytes_mib: processes
                .iter()
                .map(|process| process.private_bytes_mib)
                .sum(),
            cpu_percent: cpu,
            process_count: processes.len() as u32,
            partial,
            processes,
        })
    }
}

pub fn logical_processors() -> u32 {
    std::thread::available_parallelism()
        .map(|count| count.get() as u32)
        .unwrap_or(1)
}

pub fn report(
    samples: Vec<ResourceSample>,
    duration_seconds: u32,
    saved_path: String,
) -> MeasurementReport {
    let count = samples.len().max(1) as f64;
    let mut cpu_weight = 0.0;
    let mut cpu_elapsed = 0.0;
    let mut max_sample_gap_ms = 0.0_f64;
    for pair in samples.windows(2) {
        let gap = (pair[1].elapsed_ms - pair[0].elapsed_ms).max(0.0);
        max_sample_gap_ms = max_sample_gap_ms.max(gap);
        if let Some(cpu) = pair[1].cpu_percent {
            cpu_weight += cpu * gap;
            cpu_elapsed += gap;
        }
    }
    let measured_duration_seconds = samples
        .first()
        .zip(samples.last())
        .map(|(first, last)| (last.elapsed_ms - first.elapsed_ms).max(0.0) / 1000.0)
        .unwrap_or(0.0);
    let interrupted = samples.len() < 2
        || max_sample_gap_ms > 2500.0
        || measured_duration_seconds > f64::from(duration_seconds) * 1.15 + 1.0
        || measured_duration_seconds < f64::from(duration_seconds) * 0.85;
    let private: Option<Vec<f64>> = samples
        .iter()
        .map(|sample| sample.private_working_set_mib)
        .collect();
    MeasurementReport {
        duration_seconds,
        measured_duration_seconds,
        max_sample_gap_ms,
        interrupted,
        sample_count: samples.len() as u32,
        average_cpu_percent: (cpu_elapsed > 0.0).then(|| cpu_weight / cpu_elapsed),
        average_private_working_set_mib: private
            .as_ref()
            .filter(|values| !values.is_empty())
            .map(|values| values.iter().sum::<f64>() / count),
        peak_private_working_set_mib: private
            .as_ref()
            .filter(|values| !values.is_empty())
            .map(|values| values.iter().copied().fold(0.0, f64::max)),
        average_working_set_mib: samples
            .iter()
            .map(|sample| sample.working_set_mib)
            .sum::<f64>()
            / count,
        average_private_bytes_mib: samples
            .iter()
            .map(|sample| sample.private_bytes_mib)
            .sum::<f64>()
            / count,
        partial: samples.is_empty() || samples.iter().any(|sample| sample.partial),
        logical_processors: logical_processors(),
        build: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .into(),
        saved_path,
        samples,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn interrupted_measurements_are_flagged_and_cpu_uses_elapsed_time() {
        let sample = |elapsed_ms, cpu_percent| super::ResourceSample {
            elapsed_ms,
            cpu_percent,
            working_set_mib: 1.0,
            private_working_set_mib: Some(1.0),
            private_bytes_mib: 1.0,
            process_count: 1,
            partial: false,
            processes: Vec::new(),
        };
        let report = super::report(
            vec![
                sample(0.0, None),
                sample(1000.0, Some(10.0)),
                sample(10000.0, Some(0.0)),
            ],
            10,
            "unused.json".into(),
        );
        assert!(report.interrupted);
        assert_eq!(report.max_sample_gap_ms, 9000.0);
        assert_eq!(report.measured_duration_seconds, 10.0);
        assert_eq!(report.average_cpu_percent, Some(1.0));
    }
    #[test]
    fn empty_measurement_is_incomplete_and_ram_is_unknown() {
        let report = super::report(Vec::new(), 60, "unused.json".into());
        assert!(report.partial);
        assert!(report.average_private_working_set_mib.is_none());
        assert!(report.peak_private_working_set_mib.is_none());
        assert!(report.average_cpu_percent.is_none());
    }
}
