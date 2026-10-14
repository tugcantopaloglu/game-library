use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub audio_available: bool,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct View {
    pub known: bool,
    pub active: bool,
    pub key: String,
    pub source_name: String,
    pub source_id: String,
    pub resolution: Option<u32>,
    pub fps: Option<u32>,
    pub audio: Option<bool>,
    pub resolutions: Vec<u32>,
    pub fps_options: Vec<u32>,
    pub pending: bool,
    pub result: String,
    pub sources: Vec<Source>,
    pub source_revision: i32,
    pub source_channel: String,
    pub source_pending: bool,
    pub source_open: bool,
    pub source_error: String,
}

fn valid_source_id(id: &str, kind: &str) -> bool {
    if !matches!(kind, "window" | "screen") || id.len() > 128 { return false; }
    if let Some(handle) = id.strip_prefix(&format!("{kind}-handle:")) {
        return !handle.is_empty() && handle.bytes().all(|byte| byte.is_ascii_digit()) && handle.parse::<u64>().is_ok();
    }
    let Some(suffix) = id.strip_prefix(&format!("{kind}:")) else { return false; };
    let fields = suffix.split(':').collect::<Vec<_>>();
    (1..=2).contains(&fields.len()) && fields.iter().all(|field| !field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit()) && field.parse::<u64>().is_ok())
}

pub fn view(snapshot: &Value) -> View {
    if snapshot["connected"] != true {
        return View::default();
    }
    let mut view: View =
        serde_json::from_value(snapshot["streamControl"].clone()).unwrap_or_default();
    if view.key.len() > 256 || view.key.contains('\0') {
        return View::default();
    }
    view.resolutions.retain(|value| *value <= 4320);
    view.fps_options.retain(|value| (1..=120).contains(value));
    view.resolutions.truncate(8);
    view.fps_options.truncate(8);
    view.sources.retain(|source| {
        valid_source_id(&source.id, &source.kind)
    });
    view.sources.truncate(100);
    view
}

pub fn selection_request(
    snapshot: &Value,
    action: &str,
    key: &str,
    channel: &str,
    revision: i32,
    index: i32,
    audio: bool,
) -> Result<Value, String> {
    let view = view(snapshot);
    if !matches!(action, "stream_start" | "stream_source")
        || snapshot["capabilities"][action] != true
        || !view.known
        || (action == "stream_start" && (view.active || !key.is_empty()))
        || (action == "stream_source" && (!view.active || key.is_empty() || key != view.key))
        || view.pending
        || view.source_pending
        || !view.source_open
        || revision <= 0
        || revision != view.source_revision
        || channel.is_empty()
        || channel != view.source_channel
    {
        return Err("Ses odası veya kaynak seçimi değişti; listeyi yenile".into());
    }
    let source = usize::try_from(index)
        .ok()
        .and_then(|index| view.sources.get(index))
        .ok_or("Kaynak seçimi artık kullanılamıyor")?;
    if action == "stream_start" && audio && !source.audio_available {
        return Err("Bu kaynak yayın sesi paylaşımını desteklemiyor".into());
    }
    Ok(
        json!({"streamKey":key,"channelId":channel,"sourceRevision":revision,"sourceId":source.id,"audio":audio}),
    )
}

pub fn request(snapshot: &Value, action: &str, key: &str, value: i32) -> Result<Value, String> {
    let view = view(snapshot);
    if snapshot["capabilities"][action] != true
        || !view.active
        || view.key.is_empty()
        || view.key != key
        || view.pending
    {
        return Err("Yayın değişti veya bu kontrol şu anda kullanılamıyor".into());
    }
    let options = match action {
        "stream_quality" => &view.resolutions,
        "stream_fps" => &view.fps_options,
        "stream_stop" => return Ok(json!({"streamKey":key})),
        "stream_audio" if matches!(value, 0 | 1) => {
            return Ok(json!({"streamKey":key,"audio":value == 1}))
        }
        _ => return Err("Desteklenmeyen yayın işlemi".into()),
    };
    let value = u32::try_from(value)
        .ok()
        .filter(|value| options.contains(value))
        .ok_or("Yayın seçeneği artık kullanılamıyor")?;
    Ok(if action == "stream_quality" {
        json!({"streamKey":key,"resolution":value})
    } else {
        json!({"streamKey":key,"fps":value})
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_requests_require_the_observed_key_capability_and_valid_option() {
        let snapshot = json!({"connected":true,"capabilities":{"stream_quality":true,"stream_fps":true,"stream_stop":true},"streamControl":{"active":true,"key":"live","resolutions":[480,720],"fpsOptions":[15,30]}});
        assert_eq!(
            request(&snapshot, "stream_quality", "live", 720).unwrap()["resolution"],
            720
        );
        assert!(request(&snapshot, "stream_quality", "other", 720).is_err());
        assert!(request(&snapshot, "stream_fps", "live", -1).is_err());
        assert!(request(&snapshot, "stream_fps", "live", 2).is_err());
        assert!(request(&snapshot, "stream_start", "live", 0).is_err());
        let mut stale = snapshot.clone();
        stale["connected"] = json!(false);
        assert!(request(&stale, "stream_stop", "live", 0).is_err());
        let mut pending = snapshot;
        pending["streamControl"]["pending"] = json!(true);
        assert!(request(&pending, "stream_stop", "live", 0).is_err());
    }

    #[test]
    fn changing_option_order_never_changes_the_requested_quality_value() {
        let snapshot = json!({"connected":true,"capabilities":{"stream_quality":true,"stream_fps":true},"streamControl":{"active":true,"key":"live","resolutions":[0,480,720,1080],"fpsOptions":[5,15,30,60]}});
        assert_eq!(request(&snapshot, "stream_quality", "live", 720).unwrap()["resolution"],720);
        assert_eq!(request(&snapshot, "stream_fps", "live", 60).unwrap()["fps"],60);
        assert!(request(&snapshot, "stream_quality", "live", 2).is_err());
    }

    #[test]
    fn native_capture_handles_are_preserved_and_invalid_sources_are_rejected() {
        let snapshot = json!({"connected":true,"streamControl":{"sources":[
            {"id":"screen-handle:327932","name":"Screen 1","kind":"screen"},
            {"id":"window:919562","name":"Window","kind":"window"},
            {"id":"window:919562:1","name":"Legacy window","kind":"window"},
            {"id":"screen-handle:bad","kind":"screen"},
            {"id":"window:12:3:4","kind":"window"},
            {"id":"file:///private","kind":"window"}
        ]}});
        assert_eq!(view(&snapshot).sources.len(),3);
        assert_eq!(view(&snapshot).sources[0].id,"screen-handle:327932");
    }

    #[test]
    fn source_selection_requires_the_current_room_catalog_and_stream() {
        let mut snapshot = json!({"connected":true,"capabilities":{"stream_start":true,"stream_source":true},"streamControl":{"known":true,"sourceOpen":true,"sourceRevision":2,"sourceChannel":"room","sources":[{"id":"window:1:0","name":"Game","kind":"window","audioAvailable":true}]}});
        assert_eq!(
            selection_request(&snapshot, "stream_start", "", "room", 2, 0, true).unwrap()
                ["sourceId"],
            "window:1:0"
        );
        assert!(selection_request(&snapshot, "stream_start", "", "room", 1, 0, false).is_err());
        assert!(selection_request(&snapshot, "stream_start", "", "other", 2, 0, false).is_err());
        assert!(selection_request(&snapshot, "stream_start", "", "room", 2, -1, false).is_err());
        snapshot["streamControl"]["active"] = json!(true);
        snapshot["streamControl"]["key"] = json!("live");
        assert!(selection_request(&snapshot, "stream_start", "", "room", 2, 0, false).is_err());
        assert!(selection_request(
            &snapshot,
            "stream_source",
            "replacement",
            "room",
            2,
            0,
            false
        )
        .is_err());
        assert!(selection_request(&snapshot, "stream_source", "live", "room", 2, 0, false).is_ok());
        snapshot["connected"] = json!(false);
        assert!(
            selection_request(&snapshot, "stream_source", "live", "room", 2, 0, false).is_err()
        );
    }

    #[test]
    fn audio_requests_accept_only_boolean_choices_for_the_current_stream() {
        let mut snapshot = json!({"connected":true,"capabilities":{"stream_audio":true},"streamControl":{"active":true,"key":"live"}});
        assert_eq!(
            request(&snapshot, "stream_audio", "live", 1).unwrap()["audio"],
            true
        );
        assert!(request(&snapshot, "stream_audio", "live", 2).is_err());
        assert!(request(&snapshot, "stream_audio", "replacement", 0).is_err());
        snapshot["streamControl"]["pending"] = json!(true);
        assert!(request(&snapshot, "stream_audio", "live", 0).is_err());
    }
}
