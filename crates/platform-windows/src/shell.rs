use std::path::Path;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject, INFINITE,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOWNORMAL,
};

pub fn activate_window(handle: isize) {
    unsafe {
        let window = HWND(handle as *mut core::ffi::c_void);
        let _ = ShowWindow(window, SW_RESTORE);
        let _ = SetForegroundWindow(window);
    }
}

struct OwnedHandle(HANDLE);
unsafe impl Send for OwnedHandle {}

impl OwnedHandle {
    fn wait(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, INFINITE) == WAIT_OBJECT_0 }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub struct InstanceGuard {
    _mutex: OwnedHandle,
    event: OwnedHandle,
    pub primary: bool,
}

impl InstanceGuard {
    fn wait(&self) -> bool {
        self.event.wait()
    }
    pub fn acquire(data_dir: &Path) -> Result<Self, String> {
        let key = data_dir
            .to_string_lossy()
            .to_lowercase()
            .bytes()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        let mutex_name = HSTRING::from(format!("Local\\GameLibrary.Native.{key:016x}"));
        let event_name = HSTRING::from(format!("Local\\GameLibrary.Native.Open.{key:016x}"));
        unsafe {
            let mutex = OwnedHandle(
                CreateMutexW(None, false, &mutex_name).map_err(|error| error.to_string())?,
            );
            let primary = GetLastError() != ERROR_ALREADY_EXISTS;
            let event = OwnedHandle(
                CreateEventW(None, false, false, &event_name).map_err(|error| error.to_string())?,
            );
            if !primary {
                SetEvent(event.0).map_err(|error| error.to_string())?;
            }
            Ok(Self {
                _mutex: mutex,
                event,
                primary,
            })
        }
    }

    pub fn listen(self, on_open: impl Fn() -> bool + Send + 'static) {
        std::thread::spawn(move || loop {
            if !self.wait() || !on_open() {
                break;
            }
        });
    }
}

pub fn open_target(target: &str) -> Result<(), String> {
    let target = HSTRING::from(target);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize <= 32 {
        Err(format!("Windows başlatma isteğini kabul etmedi ({}). Launcher kurulumunu ve protokol kaydını kontrol et.", result.0 as usize))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::InstanceGuard;

    #[test]
    fn instance_ownership_survives_secondary_exit() {
        let directory = tempfile::tempdir().unwrap();
        let primary = InstanceGuard::acquire(directory.path()).unwrap();
        assert!(primary.primary);
        let secondary = InstanceGuard::acquire(directory.path()).unwrap();
        assert!(!secondary.primary);
        drop(secondary);
        assert!(!InstanceGuard::acquire(directory.path()).unwrap().primary);
        drop(primary);
        assert!(InstanceGuard::acquire(directory.path()).unwrap().primary);
    }
}
