//! Explicit, short-lived OBS WebSocket v5 operations. No background polling.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tungstenite::client::{client_with_config, IntoClientRequest};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

pub const DEFAULT_PORT: u16 = 4455;
const IO_TIMEOUT: Duration = Duration::from_secs(3);
const VERIFY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_MESSAGE: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub obs_version: String,
    pub websocket_version: String,
    pub current_scene: String,
    pub scenes: Vec<String>,
    pub streaming: bool,
    pub stream_reconnecting: bool,
    pub recording: bool,
    pub recording_paused: bool,
}

/// Construct these only in response to the corresponding explicit UI action.
/// Deliberately omit Toggle requests: stale UI state must not reverse an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    SetScene(String),
    StartStream,
    StopStream,
    StartRecording,
    StopRecording,
}

#[derive(Clone, Debug)]
pub struct ActionResult {
    pub snapshot: Snapshot,
    /// False means OBS acknowledged the command but has not reached its target state.
    pub verified: bool,
    pub message: String,
}

/// A successful return represents an authenticated, observed OBS state at this instant.
/// The password is borrowed only during the operation, never stored by this module.
pub fn snapshot(port: u16, password: Option<&str>) -> Result<Snapshot, String> {
    Session::connect(port, password)?.snapshot()
}

pub fn perform(port: u16, password: Option<&str>, action: Action) -> Result<ActionResult, String> {
    if let Action::SetScene(name) = &action {
        if name.is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
            return Err("OBS sahne adı geçersiz".into());
        }
    }
    let mut session = Session::connect(port, password)?;
    let (request, data) = match &action {
        Action::SetScene(name) => ("SetCurrentProgramScene", json!({"sceneName": name})),
        Action::StartStream => ("StartStream", json!({})),
        Action::StopStream => ("StopStream", json!({})),
        Action::StartRecording => ("StartRecord", json!({})),
        Action::StopRecording => ("StopRecord", json!({})),
    };
    session.request(request, data)?;
    let mut observed = session.snapshot().map_err(|_| {
        "OBS komutu kabul etti; işlem sonrası durum okunamadı. Yenile ile doğrula; komutu otomatik tekrarlama."
            .to_string()
    })?;
    let verification_deadline = Instant::now() + VERIFY_TIMEOUT;
    session.deadline_limit = Some(verification_deadline);
    while !action.matches(&observed) && Instant::now() < verification_deadline {
        std::thread::sleep(
            Duration::from_millis(150)
                .min(verification_deadline.saturating_duration_since(Instant::now())),
        );
        if Instant::now() >= verification_deadline {
            break;
        }
        // Only inspect the affected state during this user action. Never repeat the command.
        let result = match &action {
            Action::SetScene(_) => session.scene_state(&mut observed),
            Action::StartStream | Action::StopStream => session.stream_state(&mut observed),
            Action::StartRecording | Action::StopRecording => session.record_state(&mut observed),
        };
        if result.is_err() {
            if Instant::now() >= verification_deadline {
                break;
            }
            return Err("OBS komutu kabul etti; doğrulama bağlantısı kesildi. Yenile ile gerçek durumu kontrol et.".into());
        }
    }
    let verified = action.matches(&observed);
    let message = if verified {
        match action {
            Action::SetScene(_) => "OBS sahnesi değiştirildi ve doğrulandı.",
            Action::StartStream => "OBS yayını başladı; durum OBS üzerinden doğrulandı.",
            Action::StopStream => "OBS yayını durdu; durum OBS üzerinden doğrulandı.",
            Action::StartRecording => "OBS kaydı başladı; durum OBS üzerinden doğrulandı.",
            Action::StopRecording => "OBS kaydı durdu; durum OBS üzerinden doğrulandı.",
        }
    } else {
        "OBS komutu kabul etti; hedef durum henüz doğrulanmadı. Yenile ile kontrol et."
    };
    Ok(ActionResult {
        snapshot: observed,
        verified,
        message: message.into(),
    })
}

impl Action {
    fn matches(&self, state: &Snapshot) -> bool {
        match self {
            Self::SetScene(name) => state.current_scene == *name,
            Self::StartStream => state.streaming && !state.stream_reconnecting,
            Self::StopStream => !state.streaming && !state.stream_reconnecting,
            Self::StartRecording => state.recording,
            Self::StopRecording => !state.recording,
        }
    }
}

/// TCP timeouts alone allow slow partial frames to reset their timeout repeatedly.
/// Enforce an absolute deadline on every underlying read/write as well.
struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
}
impl DeadlineStream {
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|time| !time.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "OBS deadline"))
    }
}
impl Read for DeadlineStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.stream.read(bytes)
    }
}
impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        self.stream.flush()
    }
}

struct Session {
    socket: WebSocket<DeadlineStream>,
    sequence: u64,
    obs_version: String,
    websocket_version: String,
    available_requests: std::collections::HashSet<String>,
    deadline_limit: Option<Instant>,
}

impl Session {
    fn connect(port: u16, password: Option<&str>) -> Result<Self, String> {
        if port == 0 {
            return Err("OBS portu 1 ile 65535 arasında olmalı".into());
        }
        if password.is_some_and(|password| password.len() > 256 || password.contains('\0')) {
            return Err("OBS parolası geçersiz veya 256 bayt sınırını aşıyor".into());
        }
        // Direct numeric loopback: no DNS, proxy, remote endpoint or credential-bearing URL.
        let stream = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, port)), Duration::from_secs(2),
        ).map_err(|_| "OBS bağlantısı açılamadı. OBS'de Araçlar > WebSocket Sunucu Ayarları'nı ve portu kontrol et.")?;
        let _ = stream.set_nodelay(true);
        let mut request = format!("ws://127.0.0.1:{port}/")
            .into_client_request()
            .map_err(|_| "OBS bağlantı adresi oluşturulamadı")?;
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            "obswebsocket.json".parse().unwrap(),
        );
        let config = WebSocketConfig::default()
            .read_buffer_size(8192)
            .write_buffer_size(0)
            .max_write_buffer_size(MAX_MESSAGE)
            .max_message_size(Some(MAX_MESSAGE))
            .max_frame_size(Some(MAX_MESSAGE));
        let (socket, _) = client_with_config(
            request,
            DeadlineStream {
                stream,
                deadline: Instant::now() + IO_TIMEOUT,
            },
            Some(config),
        )
        .map_err(|_| "OBS WebSocket v5 görüşmesi başarısız; v5 sunucusunu etkinleştir.")?;
        let mut session = Self {
            socket,
            sequence: 0,
            obs_version: String::new(),
            websocket_version: String::new(),
            available_requests: std::collections::HashSet::new(),
            deadline_limit: None,
        };
        let hello = session.read_json()?;
        if hello["op"] != 0
            || hello["d"]["rpcVersion"]
                .as_u64()
                .is_none_or(|version| version < 1)
        {
            return Err("OBS WebSocket v5 Hello yanıtı geçersiz".into());
        }
        let mut identify = json!({"rpcVersion": 1, "eventSubscriptions": 0});
        if let Some(authentication) = hello["d"].get("authentication") {
            let password = password
                .filter(|password| !password.is_empty())
                .ok_or("OBS WebSocket parolası gerekiyor. OBS sunucu parolasını kaydet.")?;
            let salt = bounded_string(authentication, "salt", 256)?;
            let challenge = bounded_string(authentication, "challenge", 256)?;
            identify["authentication"] =
                Value::String(authentication_response(password, &salt, &challenge));
        }
        session.send_json(json!({"op": 1, "d": identify}))?;
        let identified = session.read_json()?;
        if identified["op"] != 2 || identified["d"]["negotiatedRpcVersion"] != 1 {
            return Err("OBS WebSocket kimlik doğrulamasını tamamlamadı".into());
        }
        let version = session.request("GetVersion", json!({}))?;
        session.obs_version = bounded_string(&version, "obsVersion", 128)?;
        session.websocket_version = bounded_string(&version, "obsWebSocketVersion", 128)?;
        let requests = version["availableRequests"]
            .as_array()
            .filter(|items| items.len() <= 4096)
            .ok_or("OBS desteklenen komut listesini göndermedi")?;
        for request in requests {
            let request = request
                .as_str()
                .filter(|request| request.len() <= 128)
                .ok_or("OBS komut listesi geçersiz")?;
            session.available_requests.insert(request.into());
        }
        Ok(session)
    }

    fn send_json(&mut self, data: Value) -> Result<(), String> {
        self.socket
            .send(Message::Text(data.to_string().into()))
            .map_err(socket_error)
    }

    fn read_json(&mut self) -> Result<Value, String> {
        // Even unexpected control/event floods cannot consume an unbounded loop.
        for _ in 0..64 {
            if Instant::now() >= self.socket.get_ref().deadline {
                return Err("OBS yanıtı zaman aşımına uğradı".into());
            }
            match self.socket.read().map_err(socket_error)? {
                Message::Text(text) => {
                    return serde_json::from_str(&text)
                        .map_err(|_| "OBS JSON yanıtı geçersiz".into())
                }
                Message::Ping(_) => self.socket.flush().map_err(socket_error)?,
                Message::Pong(_) => {}
                Message::Close(frame) => {
                    return Err(
                        if frame.is_some_and(|frame| u16::from(frame.code) == 4009) {
                            "OBS parolası kabul edilmedi. Sunucu parolasını kontrol et."
                        } else {
                            "OBS bağlantısı kapandı"
                        }
                        .into(),
                    );
                }
                _ => return Err("OBS beklenmeyen WebSocket mesajı gönderdi".into()),
            }
        }
        Err("OBS yanıtı kontrol mesajı sınırını aşıyor".into())
    }

    fn request(&mut self, kind: &str, data: Value) -> Result<Value, String> {
        if kind != "GetVersion" && !self.available_requests.contains(kind) {
            return Err(format!("OBS bu komutu desteklemiyor: {kind}"));
        }
        self.sequence += 1;
        let id = format!("game-library-{}", self.sequence);
        let deadline = Instant::now() + IO_TIMEOUT;
        self.socket.get_mut().deadline = self
            .deadline_limit
            .map_or(deadline, |limit| limit.min(deadline));
        self.send_json(
            json!({"op": 6, "d": {"requestType": kind, "requestId": id, "requestData": data}}),
        )?;
        for _ in 0..64 {
            let message = self.read_json()?;
            if message["op"] == 5 {
                continue;
            }
            if message["op"] != 7
                || message["d"]["requestId"] != id
                || message["d"]["requestType"] != kind
            {
                return Err("OBS yanıtı istekle eşleşmiyor".into());
            }
            let status = &message["d"]["requestStatus"];
            if status["result"] != true || status["code"] != 100 {
                // Do not echo arbitrary response text or credentials from the server.
                return Err(format!(
                    "OBS komutu reddetti: {kind} (kod {})",
                    status["code"].as_u64().unwrap_or(0)
                ));
            }
            return Ok(message["d"]
                .get("responseData")
                .cloned()
                .unwrap_or_else(|| json!({})));
        }
        Err("OBS olay mesajı sınırını aşıyor".into())
    }

    fn snapshot(&mut self) -> Result<Snapshot, String> {
        let mut state = Snapshot {
            obs_version: self.obs_version.clone(),
            websocket_version: self.websocket_version.clone(),
            ..Snapshot::default()
        };
        self.scene_state(&mut state)?;
        self.stream_state(&mut state)?;
        self.record_state(&mut state)?;
        Ok(state)
    }
    fn scene_state(&mut self, state: &mut Snapshot) -> Result<(), String> {
        let data = self.request("GetSceneList", json!({}))?;
        let current_scene = data
            .get("currentProgramSceneName")
            .ok_or("OBS yanıtında currentProgramSceneName alanı eksik")?;
        state.current_scene = match current_scene {
            Value::Null => String::new(),
            _ => bounded_string(&data, "currentProgramSceneName", 1024)?,
        };
        let scenes = data["scenes"]
            .as_array()
            .filter(|scenes| scenes.len() <= 4096)
            .ok_or("OBS sahne listesi geçersiz")?;
        state.scenes = scenes
            .iter()
            .map(|scene| bounded_string(scene, "sceneName", 1024))
            .collect::<Result<_, _>>()?;
        Ok(())
    }
    fn stream_state(&mut self, state: &mut Snapshot) -> Result<(), String> {
        let data = self.request("GetStreamStatus", json!({}))?;
        state.streaming = required_bool(&data, "outputActive")?;
        state.stream_reconnecting = required_bool(&data, "outputReconnecting")?;
        Ok(())
    }
    fn record_state(&mut self, state: &mut Snapshot) -> Result<(), String> {
        let data = self.request("GetRecordStatus", json!({}))?;
        state.recording = required_bool(&data, "outputActive")?;
        state.recording_paused = required_bool(&data, "outputPaused")?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.socket.get_mut().deadline = Instant::now() + Duration::from_millis(100);
        let _ = self.socket.close(None);
        // Dropping the TCP stream guarantees no resources remain after a manual operation.
    }
}

fn authentication_response(password: &str, salt: &str, challenge: &str) -> String {
    let mut first = Sha256::new();
    first.update(password.as_bytes());
    first.update(salt.as_bytes());
    let mut secret = STANDARD.encode(first.finalize()).into_bytes();
    let mut second = Sha256::new();
    second.update(&secret);
    second.update(challenge.as_bytes());
    secret.fill(0);
    STANDARD.encode(second.finalize())
}

fn bounded_string(data: &Value, key: &str, limit: usize) -> Result<String, String> {
    data[key]
        .as_str()
        .filter(|value| {
            !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or_else(|| format!("OBS yanıtındaki {key} alanı geçersiz"))
}
fn required_bool(data: &Value, key: &str) -> Result<bool, String> {
    data[key]
        .as_bool()
        .ok_or_else(|| format!("OBS yanıtındaki {key} alanı geçersiz"))
}
fn socket_error(error: tungstenite::Error) -> String {
    match error {
        tungstenite::Error::Io(error)
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            "OBS yanıtı zaman aşımına uğradı".into()
        }
        tungstenite::Error::Capacity(_) => "OBS yanıtı boyut sınırını aşıyor".into(),
        _ => "OBS WebSocket bağlantısı kesildi veya geçersiz yanıt alındı".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const REQUESTS: &[&str] = &[
        "GetVersion",
        "GetSceneList",
        "GetStreamStatus",
        "GetRecordStatus",
        "SetCurrentProgramScene",
        "StartStream",
        "StopStream",
        "StartRecord",
        "StopRecord",
    ];

    // tungstenite's callback requires its unboxed HTTP ErrorResponse type.
    #[allow(clippy::result_large_err)]
    fn test_server(
        requires_auth: bool,
        malformed_status: bool,
        mismatched_id: bool,
        deny_action: bool,
        deferred_action: bool,
    ) -> (u16, Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::clone(&captured);
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut socket = tungstenite::accept_hdr(
                stream,
                |_: &tungstenite::handshake::server::Request,
                 mut response: tungstenite::handshake::server::Response| {
                    response.headers_mut().insert(
                        "Sec-WebSocket-Protocol",
                        "obswebsocket.json".parse().unwrap(),
                    );
                    Ok(response)
                },
            )
            .unwrap();
            let mut hello = json!({"rpcVersion": 1});
            if requires_auth {
                hello["authentication"] = json!({"salt":"lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=", "challenge":"+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="});
            }
            socket
                .send(Message::Text(json!({"op":0,"d":hello}).to_string().into()))
                .unwrap();
            let identify = match socket.read() {
                Ok(Message::Text(text)) => serde_json::from_str::<Value>(&text).unwrap(),
                _ => return,
            };
            requests.lock().unwrap().push(identify.clone());
            assert_eq!(identify["op"], 1);
            assert_eq!(identify["d"]["eventSubscriptions"], 0);
            if requires_auth {
                assert_eq!(
                    identify["d"]["authentication"],
                    "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4="
                );
            }
            socket
                .send(Message::Text(
                    json!({"op":2,"d":{"negotiatedRpcVersion":1}})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            let mut streaming = false;
            let mut recording = false;
            let mut current_scene = "Game".to_string();
            loop {
                let message = match socket.read() {
                    Ok(Message::Text(text)) => serde_json::from_str::<Value>(&text).unwrap(),
                    _ => break,
                };
                requests.lock().unwrap().push(message.clone());
                assert_eq!(message["op"], 6);
                let kind = message["d"]["requestType"].as_str().unwrap();
                let mut denied = false;
                let data = match kind {
                    "GetVersion" => {
                        json!({"obsVersion":"32.1.0", "obsWebSocketVersion":"5.6.0", "availableRequests":REQUESTS})
                    }
                    "GetSceneList" => {
                        json!({"currentProgramSceneName":current_scene, "scenes":[{"sceneName":"Game"},{"sceneName":"Break"}]})
                    }
                    "GetStreamStatus" if malformed_status => {
                        json!({"outputActive":"yes", "outputReconnecting":false})
                    }
                    "GetStreamStatus" => {
                        json!({"outputActive":streaming, "outputReconnecting":false})
                    }
                    "GetRecordStatus" => json!({"outputActive":recording,"outputPaused":false}),
                    action => {
                        denied = deny_action;
                        if !denied && !deferred_action {
                            match action {
                                "StartStream" => streaming = true,
                                "StopStream" => streaming = false,
                                "StartRecord" => recording = true,
                                "StopRecord" => recording = false,
                                "SetCurrentProgramScene" => {
                                    current_scene = message["d"]["requestData"]["sceneName"]
                                        .as_str()
                                        .unwrap()
                                        .into()
                                }
                                _ => panic!("Unexpected request: {action}"),
                            }
                        }
                        json!({})
                    }
                };
                let id = if mismatched_id {
                    json!("unrelated-request")
                } else {
                    message["d"]["requestId"].clone()
                };
                let reply = json!({"op":7,"d":{"requestType":kind,"requestId":id,"requestStatus":{"result": !denied,"code":if denied {500} else {100},"comment":"sensitive server text"},"responseData":data}});
                if socket
                    .send(Message::Text(reply.to_string().into()))
                    .is_err()
                {
                    break;
                }
            }
        });
        (port, captured, server)
    }

    #[test]
    fn authenticated_snapshot_is_observed_without_output_commands() {
        let (port, captured, server) = test_server(true, false, false, false, false);
        let state = snapshot(port, Some("supersecretpassword")).unwrap();
        server.join().unwrap();
        assert_eq!(state.scenes, ["Game", "Break"]);
        assert_eq!(state.current_scene, "Game");
        assert!(!state.streaming && !state.recording);
        let sent = captured.lock().unwrap();
        assert_eq!(sent.len(), 5);
        assert!(sent.iter().skip(1).all(|item| item["d"]["requestType"]
            .as_str()
            .unwrap()
            .starts_with("Get")));
        assert!(!sent
            .iter()
            .any(|item| item.to_string().contains("supersecretpassword")));
    }

    #[test]
    fn missing_password_fails_before_identification() {
        let (port, captured, server) = test_server(true, false, false, false, false);
        assert!(snapshot(port, None)
            .unwrap_err()
            .contains("parolası gerekiyor"));
        server.join().unwrap();
        assert!(captured.lock().unwrap().is_empty());
    }

    #[test]
    fn scene_and_output_actions_are_verified_without_toggles_or_retry() {
        for action in [
            Action::SetScene("Break".into()),
            Action::StartStream,
            Action::StopStream,
            Action::StartRecording,
            Action::StopRecording,
        ] {
            let (port, captured, server) = test_server(false, false, false, false, false);
            let result = perform(port, None, action.clone()).unwrap();
            server.join().unwrap();
            assert!(result.verified && action.matches(&result.snapshot));
            let sent = captured.lock().unwrap();
            let mutations: Vec<_> = sent
                .iter()
                .skip(1)
                .filter(|item| {
                    !item["d"]["requestType"]
                        .as_str()
                        .unwrap()
                        .starts_with("Get")
                })
                .collect();
            assert_eq!(mutations.len(), 1);
            assert!(!mutations[0]["d"]["requestType"]
                .as_str()
                .unwrap()
                .starts_with("Toggle"));
        }
    }

    #[test]
    fn denied_action_does_not_claim_success_or_echo_server_text() {
        let (port, captured, server) = test_server(false, false, false, true, false);
        let error = perform(port, None, Action::StartStream).unwrap_err();
        server.join().unwrap();
        assert!(error.contains("reddetti") && error.contains("500"));
        assert!(!error.contains("sensitive"));
        assert_eq!(captured.lock().unwrap().len(), 3);
    }

    #[test]
    fn acknowledged_but_deferred_action_stays_unverified_and_is_not_repeated() {
        let (port, captured, server) = test_server(false, false, false, false, true);
        let started = Instant::now();
        let result = perform(port, None, Action::StartStream).unwrap();
        server.join().unwrap();
        assert!(!result.verified && !result.snapshot.streaming);
        assert!(result.message.contains("henüz doğrulanmadı"));
        assert!(started.elapsed() < Duration::from_secs(3));
        let sent = captured.lock().unwrap();
        assert_eq!(
            sent.iter()
                .filter(|item| item["d"]["requestType"] == "StartStream")
                .count(),
            1
        );
    }

    #[test]
    fn unrelated_response_is_rejected() {
        let (port, _, server) = test_server(false, false, true, false, false);
        assert!(snapshot(port, None).unwrap_err().contains("eşleşmiyor"));
        server.join().unwrap();
    }

    #[test]
    fn malformed_output_state_cannot_be_displayed_as_inactive() {
        let (port, _, server) = test_server(false, true, false, false, false);
        assert!(snapshot(port, None).unwrap_err().contains("outputActive"));
        server.join().unwrap();
    }

    #[test]
    fn deadline_and_capacity_are_bounded() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        let mut stream = DeadlineStream {
            stream: peer,
            deadline: Instant::now() + Duration::from_millis(50),
        };
        let started = Instant::now();
        assert!(stream.read(&mut [0]).is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        let oversized = "x".repeat(1025);
        assert!(bounded_string(&json!({"sceneName":oversized}), "sceneName", 1024).is_err());
        assert!(snapshot(0, None).is_err());
        assert!(snapshot(DEFAULT_PORT, Some(&"x".repeat(257))).is_err());
    }
}
