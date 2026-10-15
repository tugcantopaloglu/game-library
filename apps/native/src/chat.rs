use serde_json::{json, Value};
use std::collections::HashSet;

const MAX_FRIENDS: usize = 200;
const MAX_MESSAGES: usize = 150;
const MAX_VIEW_BYTES: usize = 96 * 1024;
const MAX_SEND_CHARS: usize = 2000;
const MAX_SEND_BYTES: usize = 8000;
const MAX_RECEIVE_CHARS: usize = 4000;
const MAX_RECEIVE_BYTES: usize = 16000;
const MAX_JS_INTEGER: u64 = 9_007_199_254_740_991;
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// A native open is optimistic only until Discord acknowledges a newer revision.
pub struct OpenAttempt {
    user: String,
    account: String,
    revision: u64,
    deadline: u64,
    error: Option<String>,
}

impl OpenAttempt {
    pub fn new(snapshot: &Value, user: &str, now: u64) -> Self {
        Self {
            user: user.into(),
            account: snapshot["accountId"].as_str().unwrap_or("").into(),
            revision: snapshot["chat"]["revision"].as_u64().unwrap_or(0),
            deadline: now.saturating_add(35_000),
            error: None,
        }
    }

    pub fn fail(&mut self, error: &str) {
        self.error = Some(label(error, 256));
    }

    pub fn expired(&self, now: u64) -> bool {
        self.error.is_none() && now >= self.deadline
    }
}

pub struct PanelState {
    pub loading: bool,
    pub retry: bool,
    pub status: String,
}

pub fn panel_state(
    snapshot: &Value,
    user: &str,
    attempt: &mut Option<OpenAttempt>,
    now: u64,
) -> PanelState {
    let chat = view(snapshot);
    let connected = snapshot["connected"] == true;
    if !connected
        || attempt
            .as_ref()
            .is_some_and(|attempt| attempt.user != user || snapshot["accountId"] != attempt.account)
    {
        *attempt = None;
    }
    let selected = !user.is_empty() && chat.user_id == user;
    let fallback = label(
        snapshot["chat"]["status"]
            .as_str()
            .unwrap_or("Sohbet açılamadı. Tekrar dene."),
        256,
    );
    let mut waiting = false;
    let mut error = None;
    if let Some(open) = attempt {
        if selected {
            *attempt = None;
        } else if let Some(message) = &open.error {
            error = Some(message.clone());
        } else if snapshot["chat"]["revision"].as_u64().unwrap_or(0) > open.revision
            && snapshot["chat"]["loading"] == false
            && snapshot["chat"]["userId"] == ""
        {
            open.fail(&fallback);
            error = open.error.clone();
        } else if now >= open.deadline {
            open.fail("Sohbet açma işlemi zaman aşımına uğradı. Tekrar dene.");
            error = open.error.clone();
        } else {
            waiting = true;
        }
    }
    let loading = connected && (waiting || selected && chat.loading);
    PanelState {
        loading,
        retry: connected
            && !user.is_empty()
            && !loading
            && (!selected || chat.channel_id.is_empty()),
        status: if !connected {
            "Discord bağlantısı kapalı. Bağlantı kurulduktan sonra tekrar dene.".into()
        } else if let Some(error) = error {
            error
        } else if waiting {
            "Birebir sohbet açılıyor…".into()
        } else if selected {
            chat.status
        } else if snapshot["chat"]["userId"] == "" {
            fallback
        } else {
            "Bu görüşme henüz açık değil. Tekrar dene.".into()
        },
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub author: String,
    pub content: String,
    /// Discord server milliseconds formatted in the user's current Istanbul UTC+3 offset.
    pub time: String,
    pub own: bool,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub url: String,
    pub size: u64,
    pub image: bool,
}

pub fn attachment_url(value: &str, channel: &str, id: &str) -> bool {
    valid_id(channel) && valid_id(id) && value.len() <= 2048 &&
    url::Url::parse(value).is_ok_and(|url|
        url.scheme() == "https" && matches!(url.host_str(), Some("cdn.discordapp.com" | "media.discordapp.net"))
        && url.username().is_empty() && url.password().is_none() && url.port().is_none() && url.fragment().is_none() && url.path().starts_with(&format!("/attachments/{channel}/{id}/")))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub user_id: String,
    pub channel_id: String,
    pub name: String,
    pub status: String,
    pub messages: Vec<Message>,
    pub loading: bool,
    pub can_load_older: bool,
    pub history_loading: bool,
    pub pending: bool,
    pub can_send: bool,
    /// Only the backend's confirmed MessageStore nonce may clear a draft.
    pub sent_nonce: String,
}

fn valid_id(id: &str) -> bool {
    (15..=22).contains(&id.len())
        && !id.starts_with('0')
        && id.bytes().all(|byte| byte.is_ascii_digit())
        && id.parse::<u64>().is_ok_and(|id| id > 0)
}

fn plain_text(value: &str) -> bool {
    value
        .chars()
        .all(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
}

fn label(value: &str, limit: usize) -> String {
    value
        .chars()
        .take(limit)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

struct Session<'a> {
    account: &'a str,
    revision: u64,
    chat: &'a Value,
}

fn session(snapshot: &Value) -> Option<Session<'_>> {
    if snapshot["connected"] != true {
        return None;
    }
    let account = snapshot["accountId"].as_str().filter(|id| valid_id(id))?;
    let chat = snapshot.get("chat").filter(|chat| chat.is_object())?;
    // Chat data cached for another Discord account must never be rendered or sent back.
    if chat["accountId"].as_str() != Some(account) {
        return None;
    }
    let revision = chat["revision"]
        .as_u64()
        .filter(|revision| (1..=MAX_JS_INTEGER).contains(revision))?;
    Some(Session {
        account,
        revision,
        chat,
    })
}

fn friend<'a>(snapshot: &'a Value, account: &str, user: &str) -> Option<&'a Value> {
    if !valid_id(user) || user == account {
        return None;
    }
    snapshot["friends"]
        .as_array()?
        .iter()
        .take(MAX_FRIENDS)
        .find(|friend| friend["id"].as_str() == Some(user))
}

fn clock(timestamp: u64) -> String {
    // Istanbul's current UTC+3 display offset is explicit; the execution host timezone is unused.
    let minutes = timestamp / 60_000;
    format!("{:02}:{:02}", (minutes / 60 + 3) % 24, minutes % 60)
}

fn message(value: &Value, peer_name: &str, channel: &str) -> Option<Message> {
    let id = value["id"].as_str().filter(|id| valid_id(id))?;
    let content = value["content"].as_str()?;
    if content.len() > MAX_RECEIVE_BYTES
        || content.chars().count() > MAX_RECEIVE_CHARS
        || !plain_text(content)
    {
        return None;
    }
    let timestamp = value["timestamp"]
        .as_u64()
        .filter(|timestamp| (DISCORD_EPOCH_MS..=4_102_444_800_000).contains(timestamp))?;
    let own = value["own"].as_bool()?;
    let author = label(value["author"].as_str()?, 64);
    let author = if author.trim().is_empty() {
        if own {
            "Sen".into()
        } else {
            peer_name.into()
        }
    } else {
        author
    };
    let attachments: Vec<_> = value["attachments"].as_array().into_iter().flatten().take(4).filter_map(|item| {
        let id = item["id"].as_str()?;
        let url = item["url"].as_str()?;
        if !attachment_url(url, channel, id) { return None; }
        let size = item["size"].as_u64().unwrap_or(0);
        Some(Attachment { id: id.into(), url: url.into(), name: label(item["name"].as_str().unwrap_or("Dosya"), 128), size, image: item["image"] == true && size > 0 && size <= 8*1024*1024 })
    }).collect();
    if content.is_empty() && attachments.is_empty() { return None; }
    Some(Message {
        id: id.into(),
        author,
        content: content.into(),
        time: clock(timestamp),
        own,
        attachments,
    })
}

/// Pure mapping from the latest authenticated bridge snapshot; it keeps no offline chat cache.
pub fn view(snapshot: &Value) -> View {
    let Some(session) = session(snapshot) else {
        return View::default();
    };
    let Some(user) = session.chat["userId"].as_str() else {
        return View::default();
    };
    let Some(peer) = friend(snapshot, session.account, user) else {
        return View::default();
    };
    let Some(channel) = session.chat["channelId"].as_str() else {
        return View::default();
    };
    if !channel.is_empty() && !valid_id(channel) {
        return View::default();
    }
    let name = label(peer["name"].as_str().unwrap_or(user), 64);
    let status = label(session.chat["status"].as_str().unwrap_or(""), 256);
    let loading = session.chat["loading"].as_bool().unwrap_or(true);
    // An unknown pending state cannot enable outbound messages.
    let pending = session.chat["pending"].as_bool().unwrap_or(true);
    let mut messages = Vec::new();
    if !channel.is_empty() {
        let mut seen = HashSet::new();
        let mut bytes = name.len() + status.len() + user.len() + channel.len();
        for value in session.chat["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .take(MAX_MESSAGES)
        {
            let Some(message) = message(value, &name, channel) else {
                continue;
            };
            if !seen.insert(message.id.clone()) {
                continue;
            }
            // Include row metadata overhead so text + display rows stay under the 96 KiB budget.
            let row_bytes = message.id.len()
                + message.author.len()
                + message.content.len()
                + message.attachments.iter().map(|item| item.id.len() + item.name.len() + item.url.len() + 128).sum::<usize>()
                + message.time.len()
                + 64;
            if bytes.saturating_add(row_bytes) > MAX_VIEW_BYTES {
                break;
            }
            bytes += row_bytes;
            messages.push(message);
        }
        messages.reverse();
    }
    let sent_nonce = session.chat["sentNonce"]
        .as_str()
        .filter(|nonce| {
            !nonce.is_empty()
                && nonce.len() <= 128
                && nonce.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
                })
        })
        .unwrap_or("")
        .into();
    View {
        user_id: user.into(),
        channel_id: channel.into(),
        name,
        status,
        messages,
        loading,
        pending,
        can_load_older: session.chat["canLoadOlder"] == true && snapshot["capabilities"]["chat_older"] == true,
        history_loading: session.chat["historyLoading"] == true,
        can_send: !channel.is_empty()
            && !loading
            && !pending
            && session.chat["canSend"] == true
            && snapshot["capabilities"]["chat_send"] == true,
        sent_nonce,
    }
}

/// Build a bounded command for the exact account/selection represented by the live snapshot.
pub fn request(
    snapshot: &Value,
    action: &str,
    user: &str,
    channel: &str,
    content: &str,
) -> Result<Value, String> {
    if !matches!(
        action,
        "chat_open" | "chat_close" | "chat_refresh" | "chat_older" | "chat_send"
    ) || snapshot["capabilities"][action] != true
    {
        return Err("Sohbet işlemi bu Discord sürümünde kullanılamıyor.".into());
    }
    let session =
        session(snapshot).ok_or("Discord sohbet durumu güncel değil; bağlantıyı yenile.")?;
    friend(snapshot, session.account, user)
        .ok_or("Seçili Discord arkadaşı artık kullanılamıyor.")?;
    if action != "chat_send" && !content.is_empty() {
        return Err("Sohbet işlemi mesaj içeriği kabul etmiyor.".into());
    }
    if action == "chat_open" {
        if !channel.is_empty() || session.chat["pending"].as_bool() != Some(false) {
            return Err(
                "Sohbet kanalı veya bekleyen mesaj değişti; güncel arkadaş listesini kullan."
                    .into(),
            );
        }
        // A new selection must not depend on the previous chat's snapshot revision.
        return Ok(json!({"userId":user,"accountId":session.account}));
    }
    let observed = view(snapshot);
    if observed.user_id != user || observed.channel_id != channel {
        return Err("Seçili sohbet veya DM kanalı değişti; sohbeti yeniden aç.".into());
    }
    // Closing also cancels an in-flight channel open, whose observed channel is still empty.
    if action == "chat_close" {
        return Ok(
            json!({"userId":user,"channelId":channel,"accountId":session.account,"revision":session.revision}),
        );
    }
    if !valid_id(channel) || observed.loading || observed.pending {
        return Err("Sohbetin yüklenmesi veya bekleyen mesajın sonucu bekleniyor.".into());
    }
    if action == "chat_refresh" || action == "chat_older" {
        return Ok(
            json!({"userId":user,"channelId":channel,"accountId":session.account,"revision":session.revision}),
        );
    }
    if !observed.can_send {
        return Err("Discord bu sohbet için mesaj göndermeyi doğrulamadı.".into());
    }
    if content.trim().is_empty()
        || content.len() > MAX_SEND_BYTES
        || content.chars().count() > MAX_SEND_CHARS
        || !plain_text(content)
    {
        return Err("Mesaj 1–2000 karakter olmalı; denetim karakterleri kullanılamaz.".into());
    }
    Ok(
        json!({"userId":user,"channelId":channel,"content":content,"accountId":session.account,"revision":session.revision}),
    )
}

pub fn attachment_request(snapshot: &Value, message_id: &str, attachment_id: &str) -> Result<String, String> {
    let chat = view(snapshot);
    chat.messages.iter().find(|message| message.id == message_id)
        .and_then(|message| message.attachments.iter().find(|item| item.id == attachment_id))
        .map(|item| item.url.clone()).ok_or_else(|| "Ek artık seçili görüşmede bulunmuyor. Sohbeti yenile.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "123456789012345678";
    const PEER: &str = "223456789012345678";
    const OTHER: &str = "323456789012345678";
    const CHANNEL: &str = "423456789012345678";

    fn fixture() -> Value {
        json!({"connected":true,"accountId":ACCOUNT,"friends":[{"id":PEER,"name":"Arkadaş"}],
            "capabilities":{"chat_open":true,"chat_close":true,"chat_refresh":true,"chat_send":true},
            "chat":{"accountId":ACCOUNT,"revision":3,"userId":PEER,"channelId":CHANNEL,"loading":false,"pending":false,"canSend":true,"status":"Sohbet açık","sentNonce":null,
                "messages":[{"id":"523456789012345678","author":"Arkadaş","content":"Merhaba\nNasılsın?","timestamp":1_800_000_000_000_u64,"own":false}]}})
    }

    #[test]
    fn attachment_only_messages_are_scoped_and_foreign_urls_never_open() {
        let mut snapshot=fixture();let id="623456789012345678";
        let url=format!("https://cdn.discordapp.com/attachments/{CHANNEL}/{id}/photo.png?ex=123&hm=abc");
        snapshot["chat"]["messages"][0]["content"]=json!("");
        snapshot["chat"]["messages"][0]["attachments"]=json!([{"id":id,"name":"photo.png","url":url,"size":512,"image":true},{"id":id,"name":"foreign","url":"https://example.com/no.png"}]);
        let chat=view(&snapshot);assert_eq!(chat.messages[0].attachments.len(),1);
        assert_eq!(attachment_request(&snapshot,"523456789012345678",id).unwrap(),url);
        snapshot["accountId"]=json!(OTHER);assert!(attachment_request(&snapshot,"523456789012345678",id).is_err());
        for bad in [format!("http://cdn.discordapp.com/attachments/{CHANNEL}/{id}/a.png"),format!("https://cdn.discordapp.com.evil.test/attachments/{CHANNEL}/{id}/a.png"),format!("https://cdn.discordapp.com/attachments/{OTHER}/{id}/a.png")] {assert!(!attachment_url(&bad,CHANNEL,id));}
    }
    #[test]
    fn older_requests_require_current_session_and_capability() {
        let mut snapshot=fixture();snapshot["capabilities"]["chat_older"]=json!(true);
        assert_eq!(request(&snapshot,"chat_older",PEER,CHANNEL,"").unwrap()["revision"],3);
        snapshot["chat"]["loading"]=json!(true);assert!(request(&snapshot,"chat_older",PEER,CHANNEL,"").is_err());
    }
    #[test]
    fn offline_and_account_mismatched_snapshots_clear_every_message_and_nonce() {
        let mut snapshot = fixture();
        snapshot["chat"]["sentNonce"] = json!("confirmed-1");
        assert_eq!(view(&snapshot).messages.len(), 1);
        snapshot["connected"] = json!(false);
        assert_eq!(view(&snapshot), View::default());
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, "Hello").is_err());
        snapshot["connected"] = json!(true);
        snapshot["accountId"] = json!(OTHER);
        assert_eq!(view(&snapshot), View::default());
        assert!(request(&snapshot, "chat_open", PEER, "", "").is_err());
    }

    #[test]
    fn only_current_bounded_friend_ids_and_exact_live_dm_channels_are_accepted() {
        let mut snapshot = fixture();
        assert!(request(&snapshot, "chat_open", PEER, "", "").is_ok());
        assert!(request(&snapshot, "chat_open", OTHER, "", "").is_err());
        assert!(request(&snapshot, "chat_open", PEER, CHANNEL, "").is_err());
        assert!(request(&snapshot, "chat_send", PEER, OTHER, "Hello").is_err());
        assert!(request(&snapshot, "chat_send", OTHER, CHANNEL, "Hello").is_err());
        let mut friends = vec![json!({"id":OTHER}); MAX_FRIENDS];
        friends.push(json!({"id":PEER,"name":"Out of bounds"}));
        snapshot["friends"] = json!(friends);
        assert_eq!(view(&snapshot), View::default());
        assert!(request(&snapshot, "chat_open", PEER, "", "").is_err());
    }

    #[test]
    fn requests_are_bound_to_the_current_account_and_chat_revision() {
        let snapshot = fixture();
        let open = request(&snapshot, "chat_open", PEER, "", "").unwrap();
        assert_eq!(open["accountId"], ACCOUNT);
        assert!(open.get("revision").is_none());
        let request = request(&snapshot, "chat_send", PEER, CHANNEL, "Hi").unwrap();
        assert_eq!(request["accountId"], ACCOUNT);
        assert_eq!(request["revision"], 3);
        assert_eq!(request["channelId"], CHANNEL);
        for invalid in [json!(0), json!(-1), json!("3"), json!(MAX_JS_INTEGER + 1)] {
            let mut malformed = snapshot.clone();
            malformed["chat"]["revision"] = invalid;
            assert_eq!(view(&malformed), View::default());
        }
    }

    #[test]
    fn failed_and_timed_out_opens_release_loading_and_allow_the_same_friend_to_retry() {
        let mut snapshot = fixture();
        snapshot["chat"]["userId"] = json!("");
        snapshot["chat"]["channelId"] = json!("");
        let mut attempt = Some(OpenAttempt::new(&snapshot, PEER, 100));
        assert!(panel_state(&snapshot, PEER, &mut attempt, 101).loading);
        snapshot["chat"]["revision"] = json!(4);
        snapshot["chat"]["loading"] = json!(false);
        snapshot["chat"]["status"] =
            json!("Birebir sohbet açılamadı. Discord bağlantısını kontrol et.");
        let failed = panel_state(&snapshot, PEER, &mut attempt, 102);
        assert!(!failed.loading);
        assert!(failed.retry);
        assert!(failed.status.contains("açılamadı"));
        // A new request for the same friend is allowed and has a fresh deadline.
        attempt = Some(OpenAttempt::new(&snapshot, PEER, 200));
        assert!(panel_state(&snapshot, PEER, &mut attempt, 201).loading);
        assert!(attempt.as_ref().unwrap().expired(35_200));
        let timeout = panel_state(&snapshot, PEER, &mut attempt, 35_200);
        assert!(!timeout.loading);
        assert!(timeout.retry);
        assert!(timeout.status.contains("zaman aşımına"));
        assert!(!attempt.as_ref().unwrap().expired(35_201));
    }

    #[test]
    fn a_rejected_open_keeps_its_error_until_retry_and_ignores_the_previous_conversation() {
        let mut snapshot = fixture();
        snapshot["chat"]["pending"] = json!(true);
        snapshot["friends"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":OTHER,"name":"Other"}));
        let mut attempt = OpenAttempt::new(&snapshot, OTHER, 100);
        let error = request(&snapshot, "chat_open", OTHER, "", "").unwrap_err();
        attempt.fail(&error);
        let mut attempt = Some(attempt);
        let failed = panel_state(&snapshot, OTHER, &mut attempt, 101);
        assert!(!failed.loading);
        assert!(failed.retry);
        assert_eq!(failed.status, error);
        snapshot["chat"]["pending"] = json!(false);
        assert_eq!(
            panel_state(&snapshot, OTHER, &mut attempt, 102).status,
            error
        );
        // A late response for the old user cannot keep the new user's attempt alive.
        let selected = panel_state(&snapshot, PEER, &mut attempt, 103);
        assert!(!selected.retry);
        assert!(attempt.is_none());
    }

    #[test]
    fn successful_opens_and_account_changes_clear_native_attempts() {
        let mut snapshot = fixture();
        let mut attempt = Some(OpenAttempt::new(&snapshot, PEER, 100));
        let ready = panel_state(&snapshot, PEER, &mut attempt, 101);
        assert!(!ready.loading);
        assert!(!ready.retry);
        assert!(attempt.is_none());
        attempt = Some(OpenAttempt::new(&snapshot, PEER, 100));
        snapshot["accountId"] = json!(OTHER);
        assert!(panel_state(&snapshot, PEER, &mut attempt, 102).retry);
        assert!(attempt.is_none());
        snapshot["connected"] = json!(false);
        let disconnected = panel_state(&snapshot, PEER, &mut attempt, 103);
        assert!(!disconnected.loading);
        assert!(!disconnected.retry);
    }

    #[test]
    fn an_older_open_acknowledgement_cannot_finish_the_new_selection() {
        let mut snapshot = fixture();
        let mut attempt = Some(OpenAttempt::new(&snapshot, OTHER, 100));
        snapshot["chat"]["revision"] = json!(4);
        let pending = panel_state(&snapshot, OTHER, &mut attempt, 101);
        assert!(pending.loading);
        assert!(!pending.retry);
        assert_eq!(pending.status, "Birebir sohbet açılıyor…");
    }

    #[test]
    fn unicode_message_limits_count_characters_without_splitting_utf8() {
        let snapshot = fixture();
        let content = "🦊".repeat(2000);
        assert_eq!(content.len(), MAX_SEND_BYTES);
        assert_eq!(
            request(&snapshot, "chat_send", PEER, CHANNEL, &content).unwrap()["content"],
            content
        );
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, &"🦊".repeat(2001)).is_err());
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, &"a".repeat(2001)).is_err());
        for content in ["", " \n ", "hello\0world", "hello\u{7f}"] {
            assert!(request(&snapshot, "chat_send", PEER, CHANNEL, content).is_err());
        }
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, "Hello\nworld").is_ok());
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, "hello\tworld").is_ok());
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, "hello\r\nworld").is_ok());
    }

    #[test]
    fn pending_loading_and_unknown_capabilities_disable_message_sends() {
        for field in ["loading", "pending"] {
            let mut snapshot = fixture();
            snapshot["chat"][field] = json!(true);
            assert!(!view(&snapshot).can_send);
            assert!(request(&snapshot, "chat_send", PEER, CHANNEL, "Hello").is_err());
            assert!(request(&snapshot, "chat_refresh", PEER, CHANNEL, "").is_err());
        }
        let mut snapshot = fixture();
        snapshot["capabilities"]["chat_send"] = json!(false);
        assert!(!view(&snapshot).can_send);
        assert!(request(&snapshot, "chat_send", PEER, CHANNEL, "Hello").is_err());
        snapshot["chat"].as_object_mut().unwrap().remove("pending");
        assert!(view(&snapshot).pending);
        snapshot["chat"].as_object_mut().unwrap().remove("loading");
        assert!(view(&snapshot).loading);
    }

    #[test]
    fn opening_channels_can_be_cancelled_but_never_receive_outbound_messages() {
        let mut snapshot = fixture();
        snapshot["chat"]["channelId"] = json!("");
        snapshot["chat"]["loading"] = json!(true);
        let observed = view(&snapshot);
        assert_eq!(observed.user_id, PEER);
        assert!(observed.messages.is_empty());
        assert!(!observed.can_send);
        assert!(request(&snapshot, "chat_close", PEER, "", "").is_ok());
        assert!(request(&snapshot, "chat_send", PEER, "", "Hello").is_err());
        assert!(request(&snapshot, "chat_refresh", PEER, "", "").is_err());
        assert!(request(&snapshot, "chat_close", PEER, CHANNEL, "").is_err());
    }

    #[test]
    fn newest_plain_messages_are_bounded_and_untrusted_extra_fields_are_ignored() {
        let mut snapshot = fixture();
        let messages = (0..40_u64).map(|index| json!({"id":(623_456_789_012_345_678_u64+index).to_string(),"author":"🦊".repeat(200),"content":"🦊".repeat(4000),"timestamp":1_800_000_000_000_u64+index,"own":false,"attachments":[{"url":"file:///private"}],"accessToken":"do-not-map"})).collect::<Vec<_>>();
        snapshot["chat"]["messages"] = json!(messages);
        let observed = view(&snapshot);
        assert!(!observed.messages.is_empty());
        assert!(observed.messages.len() <= MAX_MESSAGES);
        assert!(
            observed
                .messages
                .iter()
                .map(|message| message.content.len()
                    + message.author.len()
                    + message.id.len()
                    + message.attachments.iter().map(|item| item.id.len() + item.name.len() + item.url.len() + 128).sum::<usize>()
                + message.time.len()
                    + 64)
                .sum::<usize>()
                <= MAX_VIEW_BYTES
        );
        assert!(observed
            .messages
            .iter()
            .all(|message| message.author.chars().count() <= 64));
        assert_eq!(
            observed.messages.last().unwrap().id,
            (623_456_789_012_345_678_u64 + 39).to_string()
        );
        assert!(!format!("{observed:?}").contains("do-not-map"));
        snapshot["chat"]["messages"] = json!((0..200_u64).map(|index| json!({"id":(623_456_789_012_345_678_u64+index).to_string(),"author":"A","content":"<b>Plain</b>","timestamp":1_800_000_000_000_u64,"own":false})).collect::<Vec<_>>());
        let observed = view(&snapshot);
        assert_eq!(observed.messages.len(), MAX_MESSAGES);
        assert_eq!(observed.messages[0].content, "<b>Plain</b>");
    }

    #[test]
    fn invalid_message_metadata_and_control_text_never_reach_the_panel() {
        let mut snapshot = fixture();
        let valid = snapshot["chat"]["messages"][0].clone();
        let mut duplicate = valid.clone();
        duplicate["content"] = json!("Newest edit");
        let mut bad_id = valid.clone();
        bad_id["id"] = json!("bad");
        let mut bad_time = valid.clone();
        bad_time["timestamp"] = json!(u64::MAX);
        let mut bad_control = valid.clone();
        bad_control["content"] = json!("bad\0text");
        snapshot["chat"]["messages"] = json!([bad_id, bad_time, bad_control, valid, duplicate]);
        let observed = view(&snapshot);
        assert_eq!(observed.messages.len(), 1);
        assert_eq!(observed.messages[0].content, "Newest edit");
    }

    #[test]
    fn nonce_and_clock_are_bounded_confirmation_metadata() {
        let mut snapshot = fixture();
        assert_eq!(clock(DISCORD_EPOCH_MS), "03:00");
        snapshot["chat"]["sentNonce"] = json!("confirmed-123");
        assert_eq!(view(&snapshot).sent_nonce, "confirmed-123");
        snapshot["chat"]["sentNonce"] = json!("secret\0value");
        assert!(view(&snapshot).sent_nonce.is_empty());
        snapshot["chat"]["sentNonce"] = json!("a".repeat(129));
        assert!(view(&snapshot).sent_nonce.is_empty());
    }
}
