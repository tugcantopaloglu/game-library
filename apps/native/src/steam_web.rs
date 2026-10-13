//! A temporary, dedicated Steam login browser. Main UI stays native.
//! Only game metadata crosses the local browser transport; session credentials
//! remain in the browser's own profile and never enter our database or logs.
use library_core::models::Game;
use serde_json::{json, Value};
use std::io::Read;
use std::net::{Ipv4Addr, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

const SCRIPT: &str = include_str!("../../../integrations/steam/owned-library.js");
const MAX_RESULT: usize = 16 * 1024 * 1024;
fn profile(data: &Path) -> PathBuf {
    data.join("integrations/steam/browser")
}
pub fn account_id(data: &Path) -> Option<String> {
    let path = data.join("integrations/steam/connection.json");
    if std::fs::metadata(&path).ok()?.len() > 4096 {
        return None;
    }
    let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let id = value["steamid"].as_str()?;
    valid_id(id).then(|| id.to_owned())
}
fn valid_id(id: &str) -> bool {
    id.len() == 17 && id.bytes().all(|byte| byte.is_ascii_digit())
}

fn browser_executable() -> Option<PathBuf> {
    for base in [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("LOCALAPPDATA"),
    ]
    .into_iter()
    .flatten()
    {
        for relative in [
            "Microsoft/Edge/Application/msedge.exe",
            "Google/Chrome/Application/chrome.exe",
        ] {
            let path = PathBuf::from(&base).join(relative);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

struct Browser {
    process: Child,
    port: u16,
    socket: Option<WebSocket<TcpStream>>,
    next_id: u64,
}
impl Browser {
    fn launch(data: &Path, interactive: bool) -> Result<Self, String> {
        let executable = browser_executable()
            .ok_or("Steam giriş penceresi için Microsoft Edge veya Chrome gerekli.")?;
        let directory = profile(data);
        std::fs::create_dir_all(&directory).map_err(|_| "Steam giriş profili hazırlanamadı")?;
        let active = directory.join("DevToolsActivePort");
        if active.is_file() {
            std::fs::remove_file(&active)
                .map_err(|_| "Önce açık Steam giriş penceresini kapat.")?;
        }
        let mut command = Command::new(executable);
        command
            .args([
                "--remote-debugging-port=0",
                "--remote-debugging-address=127.0.0.1",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-background-mode",
                "--disable-extensions",
            ])
            .arg(format!(
                "--user-data-dir={}",
                crate::updates::external_path(&directory)
            ))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x08000000);
        if interactive {
            command.arg("--app=https://store.steampowered.com/login/?redir=explore%2F");
        } else {
            command.args([
                "--headless=new",
                "--disable-gpu",
                "https://store.steampowered.com/explore/",
            ]);
        }
        let process = command
            .spawn()
            .map_err(|_| "Steam giriş penceresi açılamadı")?;
        let mut browser = Self {
            process,
            port: 0,
            socket: None,
            next_id: 0,
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(&active) {
                if text.len() <= 256 {
                    if let Some(port) = text
                        .lines()
                        .next()
                        .and_then(|port| port.parse::<u16>().ok())
                        .filter(|port| *port > 0)
                    {
                        browser.port = port;
                        return Ok(browser);
                    }
                }
            }
            if browser
                .process
                .try_wait()
                .map_err(|_| "Steam giriş işlemi okunamadı")?
                .is_some()
            {
                return Err("Steam giriş penceresi kapatıldı. Tekrar bağlanmayı dene.".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err("Steam giriş penceresi zaman aşımına uğradı".into())
    }
    fn attach(&mut self) -> Result<bool, String> {
        let mut response = crate::media::agent(5)
            .get(&format!("http://127.0.0.1:{}/json/list", self.port))
            .call()
            .map_err(|_| "Steam giriş penceresine bağlanılamadı")?;
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Steam giriş durumu okunamadı")?;
        if bytes.len() > 1024 * 1024 {
            return Err("Steam giriş durum sınırı aşıldı".into());
        }
        let targets: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Steam giriş yanıtı geçersiz")?;
        for target in targets
            .as_array()
            .ok_or("Steam giriş hedefi geçersiz")?
            .iter()
            .take(32)
        {
            if target["type"] != "page" || !target["url"].as_str().is_some_and(trusted_page) {
                continue;
            }
            let address = target["webSocketDebuggerUrl"]
                .as_str()
                .ok_or("Steam giriş bağlantısı eksik")?;
            if !trusted_socket(address, self.port) {
                return Err("Steam giriş bağlantısı doğrulanamadı".into());
            }
            let stream = TcpStream::connect_timeout(
                &(Ipv4Addr::LOCALHOST, self.port).into(),
                Duration::from_secs(3),
            )
            .map_err(|_| "Steam giriş bağlantısı kurulamadı")?;
            stream
                .set_read_timeout(Some(Duration::from_secs(35)))
                .map_err(|_| "Steam giriş zaman sınırı ayarlanamadı")?;
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .map_err(|_| "Steam giriş zaman sınırı ayarlanamadı")?;
            let config = tungstenite::protocol::WebSocketConfig::default()
                .max_message_size(Some(MAX_RESULT))
                .max_frame_size(Some(MAX_RESULT));
            let (socket, _) =
                tungstenite::client::client_with_config(address, stream, Some(config))
                    .map_err(|_| "Steam giriş protokolü doğrulanamadı")?;
            self.socket = Some(socket);
            return Ok(true);
        }
        Ok(false)
    }
    fn evaluate(&mut self) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let socket = self.socket.as_mut().ok_or("Steam giriş bağlantısı eksik")?;
        socket.send(Message::Text(json!({"id":id,"method":"Runtime.evaluate","params":{"expression":SCRIPT,"awaitPromise":true,"returnByValue":true}}).to_string().into())).map_err(|_|"Steam giriş isteği gönderilemedi")?;
        let deadline = Instant::now() + Duration::from_secs(35);
        while Instant::now() < deadline {
            let message = socket
                .read()
                .map_err(|_| "Steam giriş penceresi kapandı veya istek zaman aşımına uğradı")?;
            if let Message::Text(text) = message {
                if text.len() > MAX_RESULT {
                    return Err("Steam hesap yanıtı çok büyük".into());
                }
                let value: Value =
                    serde_json::from_str(&text).map_err(|_| "Steam giriş yanıtı geçersiz")?;
                if value["id"] != id {
                    continue;
                }
                if !value["error"].is_null() || !value["result"]["exceptionDetails"].is_null() {
                    return Err("Steam sayfa sözleşmesi değişti. Hesap listesi korundu.".into());
                }
                return Ok(value["result"]["result"]["value"].clone());
            }
        }
        Err("Steam hesap isteği zaman aşımına uğradı".into())
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        if let Some(socket) = &mut self.socket {
            let _ = socket.send(Message::Text(
                json!({"id":999999,"method":"Browser.close"})
                    .to_string()
                    .into(),
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if !matches!(self.process.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // This is only the dedicated helper child, never the user's normal browser.
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
fn trusted_page(address: &str) -> bool {
    url::Url::parse(address).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("store.steampowered.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443)
    })
}
fn trusted_socket(address: &str, port: u16) -> bool {
    url::Url::parse(address).is_ok_and(|url| {
        url.scheme() == "ws"
            && url.host_str() == Some("127.0.0.1")
            && url.port() == Some(port)
            && url.username().is_empty()
            && url.password().is_none()
            && url.path().starts_with("/devtools/page/")
            && url.query().is_none()
            && url.fragment().is_none()
    })
}
fn parse_catalog(value: &Value) -> Result<(String, Vec<Game>), String> {
    let id = value["steamid"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or("Steam hesabı doğrulanamadı")?;
    let bytes = serde_json::to_vec(&json!({"response":value["response"]}))
        .map_err(|_| "Steam hesap listesi okunamadı")?;
    let games = library_core::owned::steam(&bytes).map_err(|error| error.to_string())?;
    Ok((id.to_owned(), games))
}

pub fn probe(data: &Path) -> Result<Value, String> {
    let mut browser = Browser::launch(data, false)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if browser.socket.is_none() && !browser.attach()? {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let value = browser.evaluate()?;
        if value["kind"] != "waiting" {
            return Ok(
                json!({"browserTransport":true,"steamPageStatus":value["kind"],"credentialsExported":false}),
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Steam giriş sayfası okunamadı".into())
}
pub fn library(data: &Path, interactive: bool) -> Result<Vec<Game>, String> {
    if !interactive && account_id(data).is_none() {
        return Err("Kurulu olmayan Steam oyunları için Steam'e bağlan düğmesini kullan.".into());
    }
    let mut browser = Browser::launch(data, interactive)?;
    let deadline = Instant::now() + Duration::from_secs(if interactive { 600 } else { 45 });
    while Instant::now() < deadline {
        if browser.socket.is_none() && !browser.attach()? {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let value = browser.evaluate()?;
        match value["kind"].as_str() {
            Some("games") => {
                let (id,games)=parse_catalog(&value)?;
                std::fs::write(data.join("integrations/steam/connection.json"),json!({"steamid":id,"games":games.len()}).to_string()).map_err(|_|"Steam bağlantı bilgisi kaydedilemedi")?;
                return Ok(games);
            },
            Some("waiting" | "login_required") if interactive => {std::thread::sleep(Duration::from_secs(1));},
            Some("waiting") => {std::thread::sleep(Duration::from_millis(500));},
            Some("login_required") => return Err("Steam oturumu sona erdi; Steam'e bağlan ile yeniden giriş yap. Önceki oyunlar korundu.".into()),
            Some("unsupported") => return Err("Steam giriş sayfası bu sürümde desteklenmiyor. Gelişmiş bağlantı seçeneklerini kullan.".into()),
            _ => return Err("Steam hesap listesi alınamadı; girişini ve bağlantını kontrol et. Mevcut oyunlar korundu.".into()),
        }
    }
    Err("Steam girişini tamamlayıp yeniden bağlan. Önceki oyunlar korundu.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_transport_is_restricted_to_the_owned_local_session_and_steam() {
        assert!(trusted_page("https://store.steampowered.com/explore/"));
        assert!(trusted_socket(
            "ws://127.0.0.1:4455/devtools/page/ABC",
            4455
        ));
        for page in [
            "http://store.steampowered.com/",
            "https://store.steampowered.com.evil.test/",
            "https://user@store.steampowered.com/",
            "https://store.steampowered.com:444/",
        ] {
            assert!(!trusted_page(page));
        }
        for socket in [
            "ws://192.168.1.2:4455/devtools/page/ABC",
            "ws://127.0.0.1:4456/devtools/page/ABC",
            "ws://127.0.0.1:4455/devtools/browser/ABC",
            "ws://user@127.0.0.1:4455/devtools/page/ABC",
        ] {
            assert!(!trusted_socket(socket, 4455));
        }
    }
    #[test]
    fn only_complete_owned_catalogs_can_leave_the_browser() {
        let value = json!({"steamid":"76561198000000001","response":{"game_count":1,"games":[{"appid":570,"name":"Dota 2"}]}});
        let (id, games) = parse_catalog(&value).unwrap();
        assert_eq!(id, "76561198000000001");
        assert_eq!(games.len(), 1);
        assert!(!games[0].installed);
        assert!(parse_catalog(&json!({"steamid":"76561198000000001","response":{}})).is_err());
        assert!(parse_catalog(&json!({"steamid":"bad","response":{"game_count":0}})).is_err());
        assert!(parse_catalog(
            &json!({"steamid":"76561198000000001","response":{"game_count":2,"games":[]}})
        )
        .is_err());
    }
}
