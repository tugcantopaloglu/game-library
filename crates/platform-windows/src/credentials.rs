use windows::core::{HSTRING, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

const TARGET: &str = "GameLibrary/SteamGridDB";

pub fn save_artwork_key(key: &str) -> Result<(), String> {
    save_key(TARGET, key)
}
pub fn save_steam_key(key: &str) -> Result<(), String> {
    save_key("GameLibrary/SteamWebAPI", key)
}
pub fn steam_key() -> Result<Option<String>, String> {
    read_key("GameLibrary/SteamWebAPI")
}
fn save_key(target_name: &str, key: &str) -> Result<(), String> {
    let target = HSTRING::from(target_name);
    if key.is_empty() {
        let _ = unsafe { CredDeleteW(&target, CRED_TYPE_GENERIC, None) };
        return Ok(());
    }
    if key.len() > 256
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("API anahtarı geçersiz".into());
    }
    let mut target_text: Vec<u16> = target_name.encode_utf16().chain(Some(0)).collect();
    let mut bytes = key.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target_text.as_mut_ptr()),
        CredentialBlobSize: bytes.len() as u32,
        CredentialBlob: bytes.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    let result = unsafe { CredWriteW(&credential, 0) }.map_err(|error| error.to_string());
    bytes.fill(0);
    result
}

pub fn artwork_key() -> Result<Option<String>, String> {
    read_key(TARGET)
}
fn read_key(target_name: &str) -> Result<Option<String>, String> {
    let mut credential = std::ptr::null_mut();
    unsafe {
        match CredReadW(
            &HSTRING::from(target_name),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        ) {
            Ok(()) => {
                let size = (*credential).CredentialBlobSize as usize;
                let result = if size <= 256 {
                    String::from_utf8(
                        std::slice::from_raw_parts((*credential).CredentialBlob, size).to_vec(),
                    )
                    .map(Some)
                    .map_err(|_| "API anahtarı okunamadı".into())
                } else {
                    Err("API anahtarı sınırı aşıyor".into())
                };
                CredFree(credential.cast());
                result
            }
            Err(error) if error.code().0 as u32 == 0x80070490 => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}
