use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub const PROTOCOL: u64 = 1;
const MAX_SNAPSHOT_AGE: Duration = Duration::from_secs(6);

struct LiveState {
    value: Value,
    received_at: Option<Instant>,
    change: u64,
}

impl LiveState {
    fn disconnected(status: &str) -> Self {
        Self {
            value: json!({"connected":false,"status":status}),
            received_at: None,
            change: 0,
        }
    }

    fn snapshot(&self, now: Instant) -> Value {
        if self
            .received_at
            .is_some_and(|at| now.saturating_duration_since(at) > MAX_SNAPSHOT_AGE)
        {
            return json!({"connected":false,"status":"Discord verisi güncel değil. Yeni durum bekleniyor."});
        }
        self.value.clone()
    }

    fn confirm(&mut self, message: &Value, now: Instant) -> bool {
        let revision = message["revision"].as_u64();
        if message["protocol"] != PROTOCOL
            || message["version"] != PROTOCOL
            || !revision.is_some_and(|revision| revision > 0 && revision <= 9_007_199_254_740_991)
            || revision != self.value["revision"].as_u64()
            || self.received_at.is_none()
        {
            return false;
        }
        self.received_at = Some(now);
        true
    }

    fn view_token(&self) -> (u64, bool) {
        (
            self.change,
            self.value["connected"] == true
                && self
                    .received_at
                    .is_some_and(|at| at.elapsed() <= MAX_SNAPSHOT_AGE),
        )
    }
}

pub struct Bridge {
    state: Arc<Mutex<LiveState>>,
    commands: mpsc::SyncSender<Value>,
    stop: Arc<AtomicBool>,
    address: std::net::SocketAddr,
    config: std::path::PathBuf,
    token: String,
}

fn read_message(reader: &mut BufReader<TcpStream>) -> Result<Value, String> {
    let mut bytes = Vec::new();
    reader
        .take(256 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() > 256 * 1024 || bytes.last() != Some(&b'\n') {
        return Err("Köprü mesajı geçersiz".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Köprü JSON mesajı geçersiz".into())
}

fn write_message(stream: &mut TcpStream, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    stream.write_all(&bytes).map_err(|error| error.to_string())
}

impl Bridge {
    pub fn start(data: &Path) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|error| error.to_string())?;
        let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        let owner_token = token.clone();
        let directory = data.join("integrations/vencord");
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        std::fs::write(directory.join("bridge.json"),serde_json::to_vec(&json!({"protocol":PROTOCOL,"port":listener.local_addr().map_err(|error|error.to_string())?.port(),"token":token})).map_err(|error|error.to_string())?).map_err(|error|error.to_string())?;
        let state = Arc::new(Mutex::new(LiveState::disconnected(
            "Vencord köprüsü bekleniyor",
        )));
        let state_copy = state.clone();
        let (commands, receiver) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(mut stream) = stream else {
                    break;
                };
                let result = (|| -> Result<(), String> {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .map_err(|error| error.to_string())?;
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .map_err(|error| error.to_string())?;
                    let mut reader =
                        BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
                    let hello = read_message(&mut reader)?;
                    if hello["protocol"] != PROTOCOL
                        || hello["token"] != token
                        || hello["type"] != "hello"
                    {
                        return Err("Köprü kimliği veya sürümü uyumsuz".into());
                    }
                    write_message(&mut stream, &json!({"type":"welcome","protocol":PROTOCOL}))?;
                    while receiver.try_recv().is_ok() {}
                    loop {
                        if stopping.load(Ordering::Relaxed) {
                            break;
                        }
                        match read_message(&mut reader) {
                            Ok(value) => {
                                if value["type"] == "snapshot" {
                                    if value["protocol"] != PROTOCOL
                                        || value["version"] != PROTOCOL
                                        || !value["capabilities"].is_object()
                                        || !value["friends"].is_array()
                                        || !value["rooms"].is_array()
                                    {
                                        return Err("Köprü durum verisi veya sürümü uyumsuz".into());
                                    }
                                    if let Ok(mut state) = state_copy.lock() {
                                        state.value = value;
                                        state.value["connected"] = json!(true);
                                        state.received_at = Some(Instant::now());
                                        state.change = state.change.wrapping_add(1);
                                    }
                                } else if value["type"] == "snapshot_unchanged" {
                                    if let Ok(mut state) = state_copy.lock() {
                                        if !state.confirm(&value, Instant::now()) {
                                            return Err("Köprü durum onayı uyumsuz".into());
                                        }
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                        while let Ok(command) = receiver.try_recv() {
                            write_message(&mut stream, &command)?;
                        }
                        write_message(&mut stream, &json!({"type":"ping"}))?;
                    }
                    Ok(())
                })();
                let _ = result;
                if let Ok(mut state) = state_copy.lock() {
                    let change = state.change.wrapping_add(1);
                    *state = LiveState::disconnected("Vencord bağlantısı kapalı veya uyumsuz");
                    state.change = change;
                }
            }
        });
        Ok(Self {
            state,
            commands,
            stop,
            address,
            config: directory.join("bridge.json"),
            token: owner_token,
        })
    }

    pub fn snapshot(&self) -> Value {
        self.state
            .lock()
            .map(|state| state.snapshot(Instant::now()))
            .unwrap_or_else(|_| json!({"connected":false}))
    }

    pub fn view_token(&self) -> (u64, bool) {
        self.state
            .lock()
            .map(|state| state.view_token())
            .unwrap_or((0, false))
    }

    pub fn command(&self, action: &str, data: Value) -> Result<(), String> {
        let snapshot = self.snapshot();
        if snapshot["connected"] != true || snapshot["capabilities"][action] != true {
            return Err("Bu işlem bağlı Discord/Vencord sürümünde desteklenmiyor".into());
        }
        self.commands
            .try_send(json!({"type":"command","action":action,"data":data}))
            .map_err(|_| "Discord işlem kuyruğu dolu".into())
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let current = std::fs::read(&self.config)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        if current.is_some_and(|current| current["token"] == self.token) {
            let _ = std::fs::remove_file(&self.config);
        }
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_confirmation_requires_the_current_snapshot_revision() {
        let now = Instant::now();
        let mut state = LiveState::disconnected("test");
        state.value = json!({"connected":true,"revision":7,"friends":[],"rooms":[]});
        state.received_at = Some(now - MAX_SNAPSHOT_AGE - Duration::from_secs(1));
        let token = state.view_token();
        assert!(!state.confirm(&json!({"protocol":1,"version":1,"revision":6}), now));
        assert_eq!(state.snapshot(now)["connected"], false);
        assert!(state.confirm(&json!({"protocol":1,"version":1,"revision":7}), now));
        assert_eq!(state.snapshot(now)["connected"], true);
        assert_eq!(state.view_token().0, token.0);
        assert!(!state.confirm(&json!({"protocol":1,"version":2,"revision":7}), now));
        state.received_at = None;
        assert!(!state.confirm(&json!({"protocol":1,"version":1,"revision":7}), now));
    }
    #[test]
    fn bridge_requires_pairing_and_clears_live_data_on_disconnect() {
        let directory = tempfile::tempdir().unwrap();
        let bridge = Bridge::start(directory.path()).unwrap();
        assert!(bridge.command("mute", json!({})).is_err());
        let config: Value = serde_json::from_slice(
            &std::fs::read(directory.path().join("integrations/vencord/bridge.json")).unwrap(),
        )
        .unwrap();
        let mut stream = TcpStream::connect(bridge.address).unwrap();
        write_message(
            &mut stream,
            &json!({"type":"hello","protocol":PROTOCOL,"token":config["token"]}),
        )
        .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        assert_eq!(read_message(&mut reader).unwrap()["type"], "welcome");
        write_message(&mut stream,&json!({"type":"snapshot","protocol":PROTOCOL,"version":PROTOCOL,"capabilities":{"mute":true},"friends":[{"name":"Test friend"}],"rooms":[]})).unwrap();
        let start = std::time::Instant::now();
        while bridge.snapshot()["connected"] != true && start.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(bridge.snapshot()["connected"], true);
        assert!(bridge.command("mute", json!({})).is_ok());
        assert!(bridge.command("stream_start", json!({})).is_err());
        drop(reader);
        drop(stream);
        let start = std::time::Instant::now();
        while bridge.snapshot()["connected"] == true && start.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(bridge.snapshot()["connected"], false);
        assert!(bridge.snapshot()["friends"].is_null());
    }

    #[test]
    fn heartbeats_do_not_keep_stale_social_data_or_controls_live() {
        let directory = tempfile::tempdir().unwrap();
        let bridge = Bridge::start(directory.path()).unwrap();
        let mut stream = TcpStream::connect(bridge.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        write_message(
            &mut stream,
            &json!({"type":"hello","protocol":PROTOCOL,"token":bridge.token}),
        )
        .unwrap();
        assert_eq!(read_message(&mut reader).unwrap()["type"], "welcome");
        let snapshot = json!({"type":"snapshot","protocol":PROTOCOL,"version":PROTOCOL,"capabilities":{"mute":true},"friends":[{"name":"Test friend"}],"rooms":[]});
        write_message(&mut stream, &snapshot).unwrap();
        assert_eq!(read_message(&mut reader).unwrap()["type"], "ping");
        assert_eq!(bridge.snapshot()["connected"], true);
        bridge.state.lock().unwrap().received_at =
            Some(Instant::now() - MAX_SNAPSHOT_AGE - Duration::from_secs(1));
        write_message(&mut stream, &json!({"type":"heartbeat"})).unwrap();
        assert_eq!(read_message(&mut reader).unwrap()["type"], "ping");
        assert_eq!(bridge.snapshot()["connected"], false);
        assert!(bridge.snapshot()["friends"].is_null());
        assert!(bridge.command("mute", json!({})).is_err());
        write_message(&mut stream, &snapshot).unwrap();
        assert_eq!(read_message(&mut reader).unwrap()["type"], "ping");
        assert_eq!(bridge.snapshot()["connected"], true);
        assert!(bridge.command("mute", json!({})).is_ok());
    }

    #[test]
    fn incompatible_snapshot_clears_social_state_after_a_valid_handshake() {
        let directory = tempfile::tempdir().unwrap();
        let bridge = Bridge::start(directory.path()).unwrap();
        let mut stream = TcpStream::connect(bridge.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        write_message(
            &mut stream,
            &json!({"type":"hello","protocol":PROTOCOL,"token":bridge.token}),
        )
        .unwrap();
        assert_eq!(read_message(&mut reader).unwrap()["type"], "welcome");
        write_message(&mut stream, &json!({"type":"snapshot","protocol":PROTOCOL,"version":PROTOCOL,"capabilities":{"mute":true},"friends":[],"rooms":[]})).unwrap();
        assert_eq!(read_message(&mut reader).unwrap()["type"], "ping");
        write_message(&mut stream, &json!({"type":"snapshot","protocol":PROTOCOL,"version":2,"capabilities":{"mute":true},"friends":[{"name":"Stale friend"}],"rooms":[]})).unwrap();
        assert!(read_message(&mut reader).is_err());
        assert_eq!(bridge.snapshot()["connected"], false);
        assert!(bridge.snapshot()["friends"].is_null());
        assert!(bridge.command("mute", json!({})).is_err());
    }
}
