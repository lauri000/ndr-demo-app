use crate::actions::AppAction;
use crate::state::AppState;
use nostr_sdk::prelude::Event;

#[derive(uniffi::Enum, Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum AppUpdate {
    FullState(AppState),
    AccountCreated {
        rev: u64,
        nsec: String,
        pubkey: String,
        npub: String,
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
    SyncComplete,
    Toast(String),
}
