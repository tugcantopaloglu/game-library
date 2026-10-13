use serde_json::{json, Value};
use std::collections::HashSet;

const MAX_FRIENDS: usize = 200;
const MAX_MESSAGES: usize = 30;
const MAX_VIEW_BYTES: usize = 96 * 1024;
const MAX_SEND_CHARS: usize = 2000;
const MAX_SEND_BYTES: usize = 8000;
const MAX_RECEIVE_CHARS: usize = 4000;
const MAX_RECEIVE_BYTES: usize = 16000;
const MAX_JS_INTEGER: u64 = 9_007_199_254_740_991;
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub author: String,
    pub content: String,
    /// Discord server milliseconds formatted in the user's current Istanbul UTC+3 offset.
    pub time: String,
    pub own: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub user_id: String,
    pub channel_id: String,
    pub name: String,
    pub status: String,
    pub messages: Vec<Message>,
    pub loading: bool,
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

fn message(value: &Value, peer_name: &str) -> Option<Message> {
    let id = value["id"].as_str().filter(|id| valid_id(id))?;
    let content = value["content"].as_str()?;
    if content.is_empty()
        || content.len() > MAX_RECEIVE_BYTES
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
    Some(Message {
        id: id.into(),
        author,
        content: content.into(),
        time: clock(timestamp),
        own,
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
            let Some(message) = message(value, &name) else {
                continue;
            };
            if !seen.insert(message.id.clone()) {
                continue;
            }
            // Include row metadata overhead so text + display rows stay under the 96 KiB budget.
            let row_bytes = message.id.len()
                + message.author.len()
                + message.content.len()
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
        "chat_open" | "chat_close" | "chat_refresh" | "chat_send"
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
        return Ok(json!({"userId":user,"accountId":session.account,"revision":session.revision}));
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
    if action == "chat_refresh" {
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
        snapshot["chat"]["messages"] = json!((0..40_u64).map(|index| json!({"id":(623_456_789_012_345_678_u64+index).to_string(),"author":"A","content":"<b>Plain</b>","timestamp":1_800_000_000_000_u64,"own":false})).collect::<Vec<_>>());
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
