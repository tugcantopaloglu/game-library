use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Default)]
pub struct Query {
    pub text: String,
    pub favorites_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub activity: String,
    #[serde(default)]
    pub last_message_at: Option<u64>,
}

pub fn friends(snapshot: &Value, favorites: &[String], query: &Query, now_ms: u64) -> Vec<Friend> {
    if snapshot["connected"] != true {
        return vec![];
    }
    let selected: HashSet<&str> = favorites.iter().map(String::as_str).collect();
    let query_text = query.text.trim().to_lowercase();
    let mut seen = HashSet::new();
    let mut friends: Vec<Friend> = snapshot["friends"]
        .as_array()
        .into_iter()
        .flatten()
        .take(200)
        .filter_map(|value| serde_json::from_value::<Friend>(value.clone()).ok())
        .filter(|friend| {
            (15..=22).contains(&friend.id.len())
                && friend.id.bytes().all(|byte| byte.is_ascii_digit())
                && seen.insert(friend.id.clone())
                && (!query.favorites_only || selected.contains(friend.id.as_str()))
                && (query_text.is_empty()
                    || friend.name.to_lowercase().contains(&query_text)
                    || friend.activity.to_lowercase().contains(&query_text))
        })
        .collect();
    for friend in &mut friends {
        friend.last_message_at = friend
            .last_message_at
            .filter(|at| *at >= 1_420_070_400_000 && *at <= now_ms.saturating_add(300_000));
    }
    friends.sort_by(|first, second| {
        second
            .last_message_at
            .cmp(&first.last_message_at)
            .then_with(|| first.name.to_lowercase().cmp(&second.name.to_lowercase()))
            .then_with(|| first.id.cmp(&second.id))
    });
    friends
}

pub fn last_conversation(at: Option<u64>, now_ms: u64) -> String {
    let Some(at) = at else {
        return "".into();
    };
    let minutes = now_ms.saturating_sub(at) / 60_000;
    if minutes < 1 {
        "Şimdi".into()
    } else if minutes < 60 {
        format!("{minutes} dk")
    } else if minutes < 1440 {
        format!("{} sa", minutes / 60)
    } else {
        format!("{} gün", minutes / 1440)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn friends_sort_by_last_conversation_and_filter_favorites_without_inventing_recency() {
        let now = 1_800_000_000_000;
        let snapshot = json!({"connected":true,"friends":[
            {"id":"123456789012345678","name":"Older","lastMessageAt":now-120_000},
            {"id":"223456789012345678","name":"Recent","lastMessageAt":now-60_000},
            {"id":"323456789012345678","name":"Unknown"},
            {"id":"423456789012345678","name":"Future","lastMessageAt":now+600_000},
            {"id":"123456789012345678","name":"Duplicate"}
        ]});
        let favorites = vec!["123456789012345678".into()];
        let all = friends(&snapshot, &favorites, &Query::default(), now);
        assert_eq!(
            all.iter()
                .map(|friend| friend.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Recent", "Older", "Future", "Unknown"]
        );
        assert!(all[2].last_message_at.is_none());
        assert_eq!(last_conversation(all[0].last_message_at, now), "1 dk");
        let only = friends(
            &snapshot,
            &favorites,
            &Query {
                favorites_only: true,
                ..Query::default()
            },
            now,
        );
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].name, "Older");
        assert_eq!(
            friends(
                &snapshot,
                &favorites,
                &Query {
                    text: "recent".into(),
                    ..Query::default()
                },
                now
            )
            .len(),
            1
        );
        assert!(friends(
            &json!({"connected":false,"friends":snapshot["friends"]}),
            &favorites,
            &Query::default(),
            now
        )
        .is_empty());
    }
}
