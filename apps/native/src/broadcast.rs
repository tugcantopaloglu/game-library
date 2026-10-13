//! NVIDIA Broadcast integration through its installed app and Windows virtual devices.
//! Inspection reads metadata only; it does not activate audio/video capture or poll the GPU.

use serde::Serialize;
use std::path::{Path, PathBuf};

pub const DOWNLOAD_URL: &str = "https://www.nvidia.com/en-us/geforce/broadcasting/broadcast-app/";
pub const SETUP_URL: &str =
    "https://www.nvidia.com/en-us/geforce/guides/broadcast-app-setup-guide/";
pub const REQUIREMENTS: &str = "RTX 2060 / Quadro RTX 3000 / TITAN RTX veya üzeri, 8 GB RAM ve güncel NVIDIA sürücüsü gerekir. RTX bulunması tam uyum doğrulaması değildir; Broadcast kurulumunun donanım/sürücü kontrolünü tamamla.";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub nvidia: bool,
    /// Display driver name evidence only, not a guarantee of supported hardware/driver.
    pub rtx_candidate: bool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Microphone,
    Speaker,
    Camera,
}

impl DeviceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Microphone => "Mikrofon",
            Self::Speaker => "Hoparlör",
            Self::Camera => "Kamera",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Audio MMDevice endpoint ID or a DirectShow camera moniker, not a display-name alias.
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub installed: bool,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub gpus: Vec<Gpu>,
    pub rtx_detected: bool,
    pub devices: Vec<Device>,
    /// Enumeration failures must remain distinguishable from an empty device list.
    pub errors: Vec<String>,
}

impl Report {
    pub fn device(&self, kind: DeviceKind) -> Option<&Device> {
        self.devices.iter().find(|device| device.kind == kind)
    }

    pub fn summary(&self) -> String {
        let installation = if self.installed {
            self.version
                .as_deref()
                .map(|version| format!("NVIDIA Broadcast {version} kurulu."))
                .unwrap_or_else(|| "NVIDIA Broadcast kurulu.".into())
        } else {
            "NVIDIA Broadcast kurulumu bulunamadı.".into()
        };
        let hardware = if self.rtx_detected {
            "RTX adaptörü bulundu; son uyumu Broadcast doğrular."
        } else if self.gpus.is_empty() {
            "GPU bilgisi doğrulanamadı."
        } else {
            "Windows görüntü adaptörlerinde RTX doğrulanamadı."
        };
        let names = self
            .devices
            .iter()
            .map(|device| format!("{}: {}", device.kind.label(), device.name))
            .collect::<Vec<_>>()
            .join(" · ");
        let devices = if names.is_empty() {
            "Sanal cihaz bulunamadı; Broadcast'u açıp kurulumu tamamla, ardından yenile.".into()
        } else {
            names
        };
        let errors = if self.errors.is_empty() {
            String::new()
        } else {
            format!(" Cihaz keşfi: {}", self.errors.join(" · "))
        };
        format!("{installation} {hardware}\n{devices}{errors}")
    }
}

pub fn inspect() -> Report {
    let installation = installed_executable();
    let mut report = Report {
        installed: installation.is_some(),
        executable: installation
            .as_ref()
            .map(|path| path.to_string_lossy().into()),
        version: installation.as_deref().and_then(installed_version),
        ..Report::default()
    };
    #[cfg(windows)]
    {
        report.gpus = native::gpus();
        report.rtx_detected = report.gpus.iter().any(|gpu| gpu.rtx_candidate);
        native::devices(&mut report);
    }
    #[cfg(not(windows))]
    report
        .errors
        .push("NVIDIA Broadcast Windows gerektirir.".into());
    report
}

/// Launch only an installed executable discovered afresh, never an arbitrary UI-supplied path.
pub fn open() -> Result<String, String> {
    let path = installed_executable()
        .ok_or("NVIDIA Broadcast kurulu değil. NVIDIA sayfasından kurup yeniden dene.")?;
    let directory = path
        .parent()
        .ok_or("NVIDIA Broadcast kurulum yolu geçersiz.")?;
    platform_windows::shell::start_process(&path, &[], directory)
        .map_err(|error| format!("NVIDIA Broadcast açılamadı: {error}"))?;
    Ok("NVIDIA Broadcast açılıyor. Fiziksel cihazlarını Broadcast'ta seçip sanal cihaz listesini yenile.".into())
}

pub fn open_download() -> Result<String, String> {
    platform_windows::shell::open_target(DOWNLOAD_URL)?;
    Ok("NVIDIA Broadcast indirme sayfası açıldı.".into())
}

pub fn open_setup() -> Result<String, String> {
    platform_windows::shell::open_target(SETUP_URL)?;
    Ok("NVIDIA Broadcast kurulum rehberi açıldı.".into())
}

fn installed_version(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        native::installed_version(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

fn installed_executable() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        for root in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(std::env::var_os)
        {
            let path = PathBuf::from(root)
                .join("NVIDIA Corporation/NVIDIA Broadcast/NVIDIA Broadcast.exe");
            if let Some(path) = valid_executable(path) {
                return Some(path);
            }
        }
        native::install_candidates()
            .into_iter()
            .find_map(valid_executable)
    }
    #[cfg(not(windows))]
    None
}

fn valid_executable(path: PathBuf) -> Option<PathBuf> {
    if !path.is_absolute()
        || !path
            .file_name()?
            .to_str()?
            .eq_ignore_ascii_case("NVIDIA Broadcast.exe")
    {
        return None;
    }
    let path = std::fs::canonicalize(path).ok()?;
    // Symlinks cannot redirect the launch to a differently named executable.
    (path.is_file()
        && path
            .file_name()?
            .to_str()?
            .eq_ignore_ascii_case("NVIDIA Broadcast.exe"))
    .then_some(path)
}

fn is_broadcast_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("nvidia broadcast")
}

fn gpu(name: String, device_id: &str) -> Gpu {
    let nvidia = device_id.to_ascii_uppercase().contains("VEN_10DE");
    let upper = name.to_ascii_uppercase();
    let rtx_candidate = nvidia && upper.split_whitespace().any(|part| part == "RTX");
    Gpu {
        name,
        nvidia,
        rtx_candidate,
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows::core::{w, BSTR, HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, DISPLAY_DEVICEW, DISPLAY_DEVICE_MIRRORING_DRIVER,
    };
    use windows::Win32::Media::Audio::{
        eCapture, eRender, EDataFlow, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::Media::DirectShow::ICreateDevEnum;
    use windows::Win32::Media::MediaFoundation::{
        CLSID_SystemDeviceEnum, CLSID_VideoInputDeviceCategory,
    };
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };
    use windows::Win32::System::Com::StructuredStorage::IPropertyBag;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CreateBindCtx,
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, STGM_READ,
    };
    use windows::Win32::System::Variant::VARIANT;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };
    use winreg::RegKey;

    pub fn installed_version(path: &Path) -> Option<String> {
        // Query the executable's Windows product version, not NVIDIA's buildinfo.txt build ID.
        let path = HSTRING::from(path.as_os_str());
        unsafe {
            let size = GetFileVersionInfoSizeW(&path, None);
            if size == 0 || size > 1024 * 1024 {
                return None;
            }
            let mut data = vec![0_u8; size as usize];
            GetFileVersionInfoW(&path, None, size, data.as_mut_ptr().cast()).ok()?;
            let mut fixed = std::ptr::null_mut();
            let mut length = 0;
            if !VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut fixed, &mut length).as_bool()
                || fixed.is_null()
                || (length as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
            {
                return None;
            }
            let offset = (fixed as usize).checked_sub(data.as_ptr() as usize)?;
            if offset.checked_add(std::mem::size_of::<VS_FIXEDFILEINFO>())? > data.len() {
                return None;
            }
            let fixed = std::ptr::read_unaligned(fixed.cast::<VS_FIXEDFILEINFO>());
            if fixed.dwSignature != 0xfeef04bd {
                return None;
            }
            Some(format!(
                "{}.{}.{}.{}",
                fixed.dwProductVersionMS >> 16,
                fixed.dwProductVersionMS & 0xffff,
                fixed.dwProductVersionLS >> 16,
                fixed.dwProductVersionLS & 0xffff
            ))
        }
    }

    pub fn install_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let root = RegKey::predef(hive);
            for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
                if let Ok(key) = root.open_subkey_with_flags(
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\NVIDIA Broadcast.exe",
                    KEY_READ | view,
                ) {
                    if let Ok(path) = key.get_value::<String, _>("") {
                        candidates.push(PathBuf::from(path.trim().trim_matches('"')));
                    }
                }
                if let Ok(uninstall) = root.open_subkey_with_flags(
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                    KEY_READ | view,
                ) {
                    for key in uninstall.enum_keys().flatten().take(2048) {
                        let Ok(key) = uninstall.open_subkey(key) else {
                            continue;
                        };
                        let name = key
                            .get_value::<String, _>("DisplayName")
                            .unwrap_or_default();
                        if name.trim().eq_ignore_ascii_case("NVIDIA Broadcast") {
                            if let Ok(path) = key.get_value::<String, _>("InstallLocation") {
                                candidates.push(PathBuf::from(path).join("NVIDIA Broadcast.exe"));
                            }
                        }
                    }
                }
            }
        }
        candidates
    }

    fn wide_string(value: &[u16]) -> String {
        let length = value
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(value.len());
        String::from_utf16_lossy(&value[..length])
    }

    pub fn gpus() -> Vec<Gpu> {
        let mut result = Vec::new();
        for index in 0..64 {
            let mut display = DISPLAY_DEVICEW {
                cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
                ..DISPLAY_DEVICEW::default()
            };
            if !unsafe { EnumDisplayDevicesW(PCWSTR::null(), index, &mut display, 0) }.as_bool() {
                break;
            }
            if display.StateFlags.0 & DISPLAY_DEVICE_MIRRORING_DRIVER.0 != 0 {
                continue;
            }
            let candidate = gpu(
                wide_string(&display.DeviceString),
                &wide_string(&display.DeviceID),
            );
            if !candidate.name.is_empty() && !result.contains(&candidate) {
                result.push(candidate);
            }
        }
        result
    }

    struct ComGuard(bool);
    impl ComGuard {
        fn acquire() -> windows::core::Result<Self> {
            let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if result == RPC_E_CHANGED_MODE {
                // The calling thread already owns an apartment; do not uninitialize it.
                Ok(Self(false))
            } else {
                result.ok()?;
                Ok(Self(true))
            }
        }
    }
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() }
            }
        }
    }

    struct TaskString(PWSTR);
    impl TaskString {
        fn string(&self) -> windows::core::Result<String> {
            unsafe { Ok(self.0.to_string()?) }
        }
    }
    impl Drop for TaskString {
        fn drop(&mut self) {
            unsafe { CoTaskMemFree(Some(self.0.as_ptr().cast())) }
        }
    }

    pub fn devices(report: &mut Report) {
        let _com = match ComGuard::acquire() {
            Ok(guard) => guard,
            Err(error) => {
                report
                    .errors
                    .push(format!("Windows cihaz arabirimi açılamadı: {error}"));
                return;
            }
        };
        for (flow, kind) in [
            (eCapture, DeviceKind::Microphone),
            (eRender, DeviceKind::Speaker),
        ] {
            match audio_devices(flow, kind) {
                Ok(scan) => {
                    report.devices.extend(scan.devices);
                    report.errors.extend(
                        scan.errors.into_iter().map(|error| {
                            format!("{} keşfi kısmen tamamlandı: {error}", kind.label())
                        }),
                    );
                }
                Err(error) => report
                    .errors
                    .push(format!("{} listesi okunamadı: {error}", kind.label())),
            }
        }
        match camera_devices() {
            Ok(scan) => {
                report.devices.extend(scan.devices);
                report.errors.extend(
                    scan.errors
                        .into_iter()
                        .map(|error| format!("Kamera keşfi kısmen tamamlandı: {error}")),
                );
            }
            Err(error) => report
                .errors
                .push(format!("Kamera listesi okunamadı: {error}")),
        }
    }

    #[derive(Default)]
    struct Scan {
        devices: Vec<Device>,
        errors: Vec<String>,
    }

    impl Scan {
        fn item(&mut self, item: windows::core::Result<Option<Device>>) {
            match item {
                Ok(Some(device)) => self.devices.push(device),
                Ok(None) => {}
                // One broken unrelated device must not hide valid Broadcast endpoints.
                Err(error) if self.errors.len() < 4 => self.errors.push(error.to_string()),
                Err(_) => {}
            }
        }
    }

    fn audio_devices(flow: EDataFlow, kind: DeviceKind) -> windows::core::Result<Scan> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)?;
            let collection = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
            let mut result = Scan::default();
            for index in 0..collection.GetCount()?.min(4096) {
                result.item((|| {
                    let device = collection.Item(index)?;
                    let properties = device.OpenPropertyStore(STGM_READ)?;
                    let name = BSTR::try_from(&properties.GetValue(&PKEY_Device_FriendlyName)?)?
                        .to_string();
                    if !is_broadcast_name(&name) {
                        return Ok(None);
                    }
                    let id = TaskString(device.GetId()?).string()?;
                    Ok(Some(Device { id, name, kind }))
                })());
            }
            Ok(result)
        }
    }

    fn camera_devices() -> windows::core::Result<Scan> {
        // Broadcast's virtual camera is also registered as a DirectShow capture source.
        // Binding its property bag reads metadata; we deliberately never bind its media source.
        unsafe {
            let enumerator: ICreateDevEnum =
                CoCreateInstance(&CLSID_SystemDeviceEnum, None, CLSCTX_INPROC_SERVER)?;
            let mut monikers = None;
            enumerator.CreateClassEnumerator(&CLSID_VideoInputDeviceCategory, &mut monikers, 0)?;
            let Some(monikers) = monikers else {
                return Ok(Scan::default());
            };
            let context = CreateBindCtx(0)?;
            let mut result = Scan::default();
            for _ in 0..4096 {
                let mut next = [None];
                let mut count = 0;
                if let Err(error) = monikers.Next(&mut next, Some(&mut count)).ok() {
                    result.errors.push(error.to_string());
                    break;
                }
                if count == 0 {
                    break;
                }
                let Some(moniker) = next[0].take() else {
                    continue;
                };
                result.item((|| {
                    let properties: IPropertyBag = moniker.BindToStorage(&context, None)?;
                    let mut name = VARIANT::default();
                    properties.Read(w!("FriendlyName"), &mut name, None)?;
                    let name = BSTR::try_from(&name)?.to_string();
                    if !is_broadcast_name(&name) {
                        return Ok(None);
                    }
                    let id = TaskString(moniker.GetDisplayName(&context, None)?).string()?;
                    Ok(Some(Device {
                        id,
                        name,
                        kind: DeviceKind::Camera,
                    }))
                })());
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtx_evidence_requires_nvidia_vendor_and_a_driver_rtx_token() {
        assert!(
            gpu(
                "NVIDIA GeForce RTX 4070 Laptop GPU".into(),
                r"PCI\VEN_10DE&DEV_2820"
            )
            .rtx_candidate
        );
        assert!(gpu("NVIDIA TITAN RTX".into(), r"PCI\VEN_10DE&DEV_1E02").rtx_candidate);
        assert!(!gpu("NVIDIA GeForce GTX 1080".into(), r"PCI\VEN_10DE&DEV_1B80").rtx_candidate);
        assert!(!gpu("NVIDIA GeForce RTX 4070".into(), r"PCI\VEN_1002&DEV_0000").rtx_candidate);
        assert!(
            !gpu(
                "Microsoft Basic Display Adapter".into(),
                r"PCI\VEN_10DE&DEV_2820"
            )
            .rtx_candidate
        );
    }

    #[test]
    fn discovery_does_not_confuse_unrelated_nvidia_devices_with_broadcast() {
        assert!(is_broadcast_name("Microphone (NVIDIA Broadcast)"));
        assert!(is_broadcast_name("Camera (nvidia broadcast)"));
        assert!(!is_broadcast_name("NVIDIA High Definition Audio"));
        assert!(!is_broadcast_name("Microphone (NVIDIA RTX Voice)"));
    }

    #[test]
    fn installation_is_not_reported_as_a_working_device_connection() {
        let report = Report {
            installed: true,
            ..Report::default()
        };
        assert!(report.summary().contains("Sanal cihaz bulunamadı"));
        assert!(report.device(DeviceKind::Microphone).is_none());
        let mut report = report;
        report.errors.push("Test keşif hatası".into());
        assert!(report.summary().contains("Test keşif hatası"));
    }

    #[test]
    fn launching_requires_a_real_installed_broadcast_executable() {
        assert!(valid_executable(PathBuf::from("NVIDIA Broadcast.exe")).is_none());
        assert!(valid_executable(PathBuf::from(r"C:\Windows\System32\cmd.exe")).is_none());
        assert!(valid_executable(PathBuf::from(r"C:\missing\NVIDIA Broadcast.exe")).is_none());
    }
}
