use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, INFINITE, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn processes() -> Vec<(u32, PathBuf)> {
    let mut result = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map(Handle) else {
            return result;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot.0, &mut entry).is_err() {
            return result;
        }
        loop {
            if let Ok(process) = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                entry.th32ProcessID,
            )
            .map(Handle)
            {
                let mut name = vec![0u16; 32768];
                let mut length = name.len() as u32;
                if QueryFullProcessImageNameW(
                    process.0,
                    PROCESS_NAME_WIN32,
                    PWSTR(name.as_mut_ptr()),
                    &mut length,
                )
                .is_ok()
                {
                    result.push((
                        entry.th32ProcessID,
                        PathBuf::from(String::from_utf16_lossy(&name[..length as usize])),
                    ));
                }
            }
            if Process32NextW(snapshot.0, &mut entry).is_err() {
                break;
            }
        }
    }
    result
}

pub fn baseline() -> Vec<u32> {
    processes().into_iter().map(|process| process.0).collect()
}

pub fn running_named(name: &str) -> bool {
    processes().iter().any(|(_, executable)| executable.file_name().is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name)))
}

pub fn wait_for_exit(pid: u32, timeout: u32) -> Result<bool, String> {
    unsafe {
        match OpenProcess(PROCESS_SYNCHRONIZE, false, pid).map(Handle) {
            Ok(process) => Ok(WaitForSingleObject(process.0, timeout) == WAIT_OBJECT_0),
            Err(error) if error.code().0 as u32 == 0x80070057 => Ok(true),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn same_path(first: &Path, second: &Path) -> bool {
    first
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .eq_ignore_ascii_case(second.to_string_lossy().trim_start_matches("\\\\?\\"))
}

/// Observes a newly started, exact executable and then waits on its OS handle.
pub fn watch(
    executable: PathBuf,
    before: Vec<u32>,
    on_state: impl Fn(&str, Option<u32>, bool) + Send + 'static,
) {
    std::thread::spawn(move || {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(60) {
            if let Some((pid, _)) = processes()
                .into_iter()
                .find(|(pid, path)| !before.contains(pid) && same_path(path, &executable))
            {
                unsafe {
                    if let Ok(process) = OpenProcess(
                        PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                        false,
                        pid,
                    )
                    .map(Handle)
                    {
                        on_state("running", Some(pid), false);
                        if WaitForSingleObject(process.0, INFINITE) == WAIT_OBJECT_0 {
                            on_state("ended", Some(pid), true);
                        } else {
                            on_state("unconfirmed", Some(pid), true);
                        }
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        on_state("unconfirmed", None, true);
    });
}
