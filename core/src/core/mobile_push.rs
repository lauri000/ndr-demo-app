use super::*;
use std::collections::HashMap;

const MOBILE_PUSH_REACTION_KIND: u64 = 7;
const MOBILE_PUSH_RECEIPT_KIND: u64 = 15;
const MOBILE_PUSH_TYPING_KIND: u64 = 25;
const MOBILE_PUSH_GROUP_METADATA_KIND: u64 = 40;
const MOBILE_PUSH_SETTINGS_KIND: u64 = 30_078;

impl AppCore {
    pub(super) fn build_mobile_push_sync_snapshot(&self) -> MobilePushSyncSnapshot {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return MobilePushSyncSnapshot::default();
        };

        let mut message_author_pubkeys = HashSet::new();
        message_author_pubkeys.extend(self.known_message_author_hexes());
        let message_author_pubkeys = sorted_hexes(message_author_pubkeys);

        let mut sessions = Vec::new();
        let mut seen_state_json = HashSet::new();
        let session_snapshot = logged_in.session_manager.snapshot();
        let mut users_by_owner = session_snapshot
            .users
            .into_iter()
            .map(|user| (user.owner_pubkey.to_string(), user))
            .collect::<HashMap<_, _>>();
        for owner_hex in sorted_hexes(self.protocol_owner_hexes()) {
            let Some(user) = users_by_owner.remove(&owner_hex) else {
                continue;
            };
            let display_name = self.owner_display_label(&owner_hex);
            for device in user.devices {
                if let Some(session) = device.active_session {
                    push_mobile_push_session_snapshot(
                        &mut sessions,
                        &mut seen_state_json,
                        &owner_hex,
                        display_name.clone(),
                        session,
                    );
                }
                for session in device.inactive_sessions {
                    push_mobile_push_session_snapshot(
                        &mut sessions,
                        &mut seen_state_json,
                        &owner_hex,
                        display_name.clone(),
                        session,
                    );
                }
            }
        }

        MobilePushSyncSnapshot {
            owner_pubkey_hex: Some(logged_in.owner_pubkey.to_string()),
            message_author_pubkeys,
            sessions,
        }
    }
}

fn push_mobile_push_session_snapshot(
    sessions: &mut Vec<MobilePushSessionSnapshot>,
    seen_state_json: &mut HashSet<String>,
    owner_hex: &str,
    display_name: String,
    session: SessionState,
) {
    let Ok(state_json) = serde_json::to_string(&session) else {
        return;
    };
    if state_json.trim().is_empty() || !seen_state_json.insert(state_json.clone()) {
        return;
    }
    let mut tracked = HashSet::new();
    collect_expected_senders(&session, &mut tracked);
    sessions.push(MobilePushSessionSnapshot {
        recipient_pubkey_hex: owner_hex.to_string(),
        display_name,
        state_json,
        tracked_sender_pubkeys: sorted_hexes(tracked),
        has_receiving_capability: session.receiving_chain_key.is_some()
            || session.receiving_chain_message_number > 0
            || !session.skipped_keys.is_empty(),
    });
}

pub(crate) fn resolve_mobile_push_notification(
    raw_payload_json: String,
) -> MobilePushNotificationResolution {
    let payload = normalized_payload(&raw_payload_json);
    let title = resolved_title(&payload);
    let body = normalized_value(payload.get("body")).unwrap_or_else(|| "New activity".to_string());
    let inner_kind = payload
        .get("inner_kind")
        .and_then(|value| value.trim().parse::<u64>().ok())
        .or_else(|| event_kind(payload.get("inner_event_json")))
        .or_else(|| event_kind(payload.get("inner_event")))
        .or_else(|| event_kind(payload.get("event")));

    if inner_kind.is_some_and(should_suppress_mobile_push_kind) {
        return MobilePushNotificationResolution {
            should_show: false,
            title: String::new(),
            body: String::new(),
            payload_json: "{}".to_string(),
        };
    }

    let body = if inner_kind == Some(MOBILE_PUSH_REACTION_KIND) {
        let emoji = normalized_value(payload.get("body"))
            .or_else(|| event_content(payload.get("inner_event_json")))
            .or_else(|| event_content(payload.get("inner_event")))
            .unwrap_or_default();
        if emoji.is_empty() {
            "Reacted".to_string()
        } else if emoji.to_lowercase().starts_with("reacted") {
            emoji
        } else {
            format!("Reacted {emoji}")
        }
    } else {
        body
    };

    let mut resolved_payload = payload;
    resolved_payload.insert("title".to_string(), title.clone());
    resolved_payload.insert("body".to_string(), body.clone());
    if let Some(kind) = inner_kind {
        resolved_payload.insert("inner_kind".to_string(), kind.to_string());
    }

    MobilePushNotificationResolution {
        should_show: true,
        title,
        body,
        payload_json: serde_json::to_string(&resolved_payload).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn normalized_payload(raw_payload_json: &str) -> BTreeMap<String, String> {
    let mut payload = BTreeMap::new();
    let Ok(decoded) = serde_json::from_str::<serde_json::Value>(raw_payload_json) else {
        return payload;
    };
    let Some(object) = decoded.as_object() else {
        return payload;
    };
    for (key, value) in object {
        if value.is_null() {
            continue;
        }
        let value = value
            .as_str()
            .map(ToString::to_string)
            .unwrap_or_else(|| value.to_string());
        if !value.trim().is_empty() {
            payload.insert(key.clone(), value);
        }
    }
    payload
}

fn resolved_title(payload: &BTreeMap<String, String>) -> String {
    for value in [payload.get("sender_name"), payload.get("title")] {
        if let Some(title) = normalized_sender_title(value) {
            if !is_generic_sender_title(&title) {
                return title;
            }
        }
    }
    "Iris Chat".to_string()
}

fn normalized_sender_title(value: Option<&String>) -> Option<String> {
    let normalized = normalized_value(value)?;
    if normalized.to_lowercase().starts_with("dm by ") && normalized.len() > 6 {
        let stripped = normalized[6..].trim().to_string();
        return (!stripped.is_empty()).then_some(stripped);
    }
    Some(normalized)
}

fn normalized_value(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_generic_sender_title(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "" | "someone" | "new message" | "new activity" | "iris chat"
    )
}

fn event_kind(value: Option<&String>) -> Option<u64> {
    let decoded = serde_json::from_str::<serde_json::Value>(value?).ok()?;
    decoded.get("kind")?.as_u64()
}

fn event_content(value: Option<&String>) -> Option<String> {
    let decoded = serde_json::from_str::<serde_json::Value>(value?).ok()?;
    let content = decoded.get("content")?.as_str()?.to_string();
    normalized_value(Some(&content))
}

fn should_suppress_mobile_push_kind(kind: u64) -> bool {
    matches!(
        kind,
        MOBILE_PUSH_RECEIPT_KIND
            | MOBILE_PUSH_TYPING_KIND
            | MOBILE_PUSH_GROUP_METADATA_KIND
            | MOBILE_PUSH_SETTINGS_KIND
    )
}
