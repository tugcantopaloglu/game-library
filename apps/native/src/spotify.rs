//! Token-free Spotify links. Windows opens the registered Spotify URI handler.
//! These actions are invoked only by explicit UI buttons; no playback API is used.

pub fn valid_track_id(value: &str) -> bool {
    value.len() == 22 && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub fn track_uri(track_id: &str) -> Result<String, String> {
    if !valid_track_id(track_id) {
        return Err("Spotify parça kimliği geçersiz".into());
    }
    Ok(format!("spotify:track:{track_id}"))
}

pub fn open_track(track_id: &str) -> Result<(), String> {
    platform_windows::shell::open_target(&track_uri(track_id)?)
        .map_err(|_| "Spotify açılamadı. Spotify uygulama kurulumunu kontrol et.".into())
}

pub fn open_app() -> Result<(), String> {
    platform_windows::shell::open_target("spotify:")
        .map_err(|_| "Spotify açılamadı. Spotify uygulama kurulumunu kontrol et.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_targets_are_fixed_spotify_uris() {
        assert_eq!(
            track_uri("4iV5W9uYEdYUVa79Axb7Rh").unwrap(),
            "spotify:track:4iV5W9uYEdYUVa79Axb7Rh"
        );
        for invalid in [
            "spotify:track:4iV5W9uYEdYUVa79Axb7Rh",
            "https://example.com/a",
            "4iV5W9uYEdYUVa79Axb7R/",
            "4iV5W9uYEdYUVa79Axb7Rh\n",
            "4iV5W9uYEdYUVa79Axb7R",
            "4iV5W9uYEdYUVa79Axb7Ré",
        ] {
            assert!(track_uri(invalid).is_err());
        }
    }
}
