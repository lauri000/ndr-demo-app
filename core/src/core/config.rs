use super::*;

pub(super) const FALLBACK_DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
];
pub(super) const APP_VERSION: &str = env!("NDR_APP_VERSION");
pub(super) const BUILD_CHANNEL: &str = env!("NDR_BUILD_CHANNEL");
pub(super) const BUILD_GIT_SHA: &str = env!("NDR_BUILD_GIT_SHA");
pub(super) const BUILD_TIMESTAMP_UTC: &str = env!("NDR_BUILD_TIMESTAMP_UTC");
pub(super) const COMPILED_DEFAULT_RELAYS_CSV: &str = env!("NDR_DEFAULT_RELAYS");
pub(super) const RELAY_SET_ID: &str = env!("NDR_RELAY_SET_ID");
pub(super) const TRUSTED_TEST_BUILD: &str = env!("NDR_TRUSTED_TEST_BUILD");
pub(super) const MAX_SEEN_EVENT_IDS: usize = 2048;
pub(super) const RECENT_HANDSHAKE_TTL_SECS: u64 = 10 * 60;
pub(super) const PENDING_RETRY_DELAY_SECS: u64 = 2;
pub(super) const FIRST_CONTACT_STAGE_DELAY_MS: u64 = 1500;
pub(super) const FIRST_CONTACT_RETRY_DELAY_SECS: u64 = 5;
pub(super) const CATCH_UP_LOOKBACK_SECS: u64 = 30;
pub(super) const UNKNOWN_GROUP_RECOVERY_LOOKBACK_SECS: u64 = 24 * 60 * 60;
pub(super) const DEVICE_INVITE_DISCOVERY_LOOKBACK_SECS: u64 = 30 * 24 * 60 * 60;
pub(super) const DEVICE_INVITE_DISCOVERY_LIMIT: usize = 256;
pub(super) const DEVICE_INVITE_DISCOVERY_POLL_SECS: u64 = 5;
pub(super) const RELAY_CONNECT_TIMEOUT_SECS: u64 = 5;
pub(super) const RESUBSCRIBE_CATCH_UP_DELAY_SECS: u64 = 5;
pub(super) const PROTOCOL_SUBSCRIPTION_ID: &str = "ndr-protocol";
pub(super) const APP_DIRECT_MESSAGE_PAYLOAD_VERSION: u8 = 1;
pub(super) const APP_GROUP_MESSAGE_PAYLOAD_VERSION: u8 = 1;
pub(super) const GROUP_CHAT_PREFIX: &str = "group:";
pub(super) const DEBUG_SNAPSHOT_FILENAME: &str = "ndr_demo_runtime_debug.json";
pub(super) const MAX_DEBUG_LOG_ENTRIES: usize = 128;
pub(super) const PERSISTED_STATE_VERSION: u32 = 11;

pub(super) fn configured_relays() -> Vec<String> {
    let compiled_defaults = compiled_default_relays();
    match std::env::var("NDR_DEMO_RELAYS") {
        Ok(value) => {
            let custom: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if custom.is_empty() {
                compiled_defaults
            } else {
                custom
            }
        }
        Err(_) => compiled_defaults,
    }
}

pub(super) fn configured_relay_urls() -> Vec<RelayUrl> {
    let parsed: Vec<RelayUrl> = configured_relays()
        .into_iter()
        .filter_map(|relay| RelayUrl::parse(relay).ok())
        .collect();
    if parsed.is_empty() {
        FALLBACK_DEFAULT_RELAYS
            .iter()
            .filter_map(|relay| RelayUrl::parse(*relay).ok())
            .collect()
    } else {
        parsed
    }
}

pub(super) fn compiled_default_relays() -> Vec<String> {
    let compiled = COMPILED_DEFAULT_RELAYS_CSV
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if compiled.is_empty() {
        FALLBACK_DEFAULT_RELAYS
            .iter()
            .map(|relay| (*relay).to_string())
            .collect()
    } else {
        compiled
    }
}

pub(super) fn trusted_test_build() -> bool {
    matches!(TRUSTED_TEST_BUILD, "1" | "true" | "TRUE" | "True")
}

pub(crate) fn build_summary() -> String {
    format!("{APP_VERSION} ({BUILD_GIT_SHA})")
}

pub(crate) fn relay_set_id() -> &'static str {
    RELAY_SET_ID
}

pub(crate) fn trusted_test_build_flag() -> bool {
    trusted_test_build()
}

pub(super) async fn ensure_session_relays_configured(client: &Client, relay_urls: &[RelayUrl]) {
    for relay in relay_urls {
        let _ = client.add_relay(relay.clone()).await;
    }
}
