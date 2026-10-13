use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: String,
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

pub fn view(snapshot: &Value) -> View {
    if snapshot["connected"] != true {
        return View::default();
    }
    let mut view: View =
        serde_json::from_value(snapshot["streamControl"].clone()).unwrap_or_default();
    if view.key.len() > 256 || view.key.contains('\0') {
        return View::default();
    }
    view.resolutions.retain(|value| (1..=4320).contains(value));
    view.fps_options.retain(|value| (1..=120).contains(value));
    view.resolutions.truncate(8);
    view.fps_options.truncate(8);
    view.sources.retain(|source| {
        source.id.len() <= 128
            && !source.id.contains('\0')
            && matches!(source.kind.as_str(), "window" | "screen")
            && source.id.starts_with(&format!("{}:", source.kind))
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
) -> Result<Value, String> {
    let view = view(snapshot);
    if action != "stream_start"
        || snapshot["capabilities"][action] != true
        || !view.known
        || view.active
        || !key.is_empty()
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
    Ok(json!({"streamKey":key,"channelId":channel,"sourceRevision":revision,"sourceId":source.id}))
}

pub fn request(snapshot: &Value, action: &str, key: &str, index: i32) -> Result<Value, String> {
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
        _ => return Err("Desteklenmeyen yayın işlemi".into()),
    };
    let value = usize::try_from(index)
        .ok()
        .and_then(|index| options.get(index))
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
            request(&snapshot, "stream_quality", "live", 1).unwrap()["resolution"],
            720
        );
        assert!(request(&snapshot, "stream_quality", "other", 1).is_err());
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
}
