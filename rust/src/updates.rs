use crate::actions::AppAction;
use crate::state::AppState;
use nostr_sdk::prelude::Event;

#[derive(uniffi::Enum, Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum AppUpdate {
    FullState(AppState),
    PersistAccountBundle {
        rev: u64,
        owner_nsec: Option<String>,
        owner_pubkey_hex: String,
        device_nsec: String,
    },
}

#[derive(Debug)]
pub enum CoreMsg {
    Action(AppAction),
    Internal(Box<InternalEvent>),
}

#[derive(Debug)]
pub enum InternalEvent {
    RelayEvent(Event),
    RetryPendingOutbound,
    FetchTrackedPeerCatchUp,
    FetchCatchUpEvents(Vec<Event>),
    StagedSendFinished {
        message_id: String,
        chat_id: String,
        success: bool,
    },
    SyncComplete,
    Toast(String),
}
