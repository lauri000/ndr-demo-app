use crate::actions::AppAction;
use crate::state::{
    AccountSnapshot, AppState, ChatMessageSnapshot, ChatThreadSnapshot, CurrentChatSnapshot,
    DeliveryState, DeviceAuthorizationState, DeviceEntrySnapshot, DeviceRosterSnapshot, Router,
    Screen,
};
use crate::updates::{AppUpdate, CoreMsg, InternalEvent};
use flume::Sender;
use nostr_double_ratchet::{
    DevicePubkey, DeviceRoster, DomainError, Error, MessageEnvelope, OwnerPubkey,
    ProtocolContext, RelayGap, RosterEditor, SessionManager, SessionManagerSnapshot,
    SessionState, UnixSeconds,
};
use nostr_double_ratchet_nostr::nostr as codec;
use nostr_sdk::prelude::{
    Client, Event, Filter, Keys, Kind, PublicKey, RelayPoolNotification, RelayUrl,
    SubscriptionId, Timestamp, ToBech32,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
];
const MAX_SEEN_EVENT_IDS: usize = 2048;
const RECENT_HANDSHAKE_TTL_SECS: u64 = 10 * 60;
const PENDING_RETRY_DELAY_SECS: u64 = 2;
const FIRST_CONTACT_STAGE_DELAY_MS: u64 = 1500;
const FIRST_CONTACT_RETRY_DELAY_SECS: u64 = 5;
const CATCH_UP_LOOKBACK_SECS: u64 = 30;
const RESUBSCRIBE_CATCH_UP_DELAY_SECS: u64 = 5;
const PROTOCOL_SUBSCRIPTION_ID: &str = "ndr-protocol";
const APP_MESSAGE_PAYLOAD_VERSION: u8 = 1;

pub struct AppCore {
    update_tx: Sender<AppUpdate>,
    core_sender: Sender<CoreMsg>,
    shared_state: Arc<RwLock<AppState>>,
    runtime: tokio::runtime::Runtime,
    data_dir: PathBuf,
    state: AppState,
    logged_in: Option<LoggedInState>,
    threads: BTreeMap<String, ThreadRecord>,
    active_chat_id: Option<String>,
    screen_stack: Vec<Screen>,
    next_message_id: u64,
    pending_inbound: Vec<PendingInbound>,
    pending_outbound: Vec<PendingOutbound>,
    recent_handshake_peers: BTreeMap<String, RecentHandshakePeer>,
    seen_event_ids: HashSet<String>,
    seen_event_order: VecDeque<String>,
    protocol_subscription_runtime: ProtocolSubscriptionRuntime,
}

struct LoggedInState {
    owner_pubkey: OwnerPubkey,
    owner_keys: Option<Keys>,
    device_keys: Keys,
    client: Client,
    relay_urls: Vec<RelayUrl>,
    session_manager: SessionManager,
    authorization_state: LocalAuthorizationState,
}

#[derive(Clone)]
struct ThreadRecord {
    chat_id: String,
    unread_count: u64,
    updated_at_secs: u64,
    messages: Vec<ChatMessageSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct PendingInbound {
    envelope: MessageEnvelope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PreparedPublishBatch {
    #[serde(default)]
    invite_events: Vec<Event>,
    #[serde(default)]
    message_events: Vec<Event>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
enum OutboundPublishMode {
    FirstContactStaged,
    OrdinaryFirstAck,
    #[default]
    WaitForPeer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingOutbound {
    message_id: String,
    chat_id: String,
    body: String,
    #[serde(default)]
    prepared_publish: Option<PreparedPublishBatch>,
    #[serde(default)]
    publish_mode: OutboundPublishMode,
    #[serde(default)]
    reason: PendingSendReason,
    #[serde(default)]
    next_retry_at_secs: u64,
    #[serde(default)]
    in_flight: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AppMessagePayload {
    version: u8,
    chat_id: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RoutedChatMessage {
    chat_id: String,
    body: String,
    is_outgoing: bool,
}

#[derive(Clone, Debug)]
struct RecentHandshakePeer {
    owner_hex: String,
    device_hex: String,
    observed_at_secs: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalAuthorizationState {
    Authorized,
    AwaitingApproval,
    Revoked,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
enum PendingSendReason {
    #[default]
    MissingRoster,
    MissingDeviceInvite,
    PublishingFirstContact,
    PublishRetry,
}

#[derive(Debug, Clone)]
struct StagedOutboundSend {
    message_id: String,
    chat_id: String,
    invite_events: Vec<Event>,
    message_events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProtocolSubscriptionPlan {
    roster_authors: Vec<String>,
    invite_authors: Vec<String>,
    invite_response_recipient: Option<String>,
    message_authors: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct ProtocolSubscriptionRuntime {
    current_plan: Option<ProtocolSubscriptionPlan>,
    applying_plan: Option<ProtocolSubscriptionPlan>,
    refresh_in_flight: bool,
    refresh_dirty: bool,
    refresh_token: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    version: u32,
    #[serde(alias = "active_peer_hex")]
    active_chat_id: Option<String>,
    next_message_id: u64,
    session_manager: Option<SessionManagerSnapshot>,
    threads: Vec<PersistedThread>,
    #[serde(default)]
    pending_inbound: Vec<PendingInbound>,
    #[serde(default)]
    pending_outbound: Vec<PendingOutbound>,
    #[serde(default)]
    seen_event_ids: Vec<String>,
    #[serde(default)]
    authorization_state: Option<PersistedAuthorizationState>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedThread {
    #[serde(alias = "peer_hex")]
    chat_id: String,
    unread_count: u64,
    #[serde(default)]
    updated_at_secs: u64,
    messages: Vec<PersistedMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedMessage {
    id: String,
    #[serde(alias = "peer_input")]
    chat_id: String,
    author: String,
    body: String,
    is_outgoing: bool,
    created_at_secs: u64,
    delivery: PersistedDeliveryState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PersistedDeliveryState {
    Pending,
    Sent,
    Received,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PersistedAuthorizationState {
    Authorized,
    AwaitingApproval,
    Revoked,
}

impl From<PersistedDeliveryState> for DeliveryState {
    fn from(value: PersistedDeliveryState) -> Self {
        match value {
            PersistedDeliveryState::Pending => DeliveryState::Pending,
            PersistedDeliveryState::Sent => DeliveryState::Sent,
            PersistedDeliveryState::Received => DeliveryState::Received,
            PersistedDeliveryState::Failed => DeliveryState::Failed,
        }
    }
}

impl From<&DeliveryState> for PersistedDeliveryState {
    fn from(value: &DeliveryState) -> Self {
        match value {
            DeliveryState::Pending => Self::Pending,
            DeliveryState::Sent => Self::Sent,
            DeliveryState::Received => Self::Received,
            DeliveryState::Failed => Self::Failed,
        }
    }
}

impl From<LocalAuthorizationState> for PersistedAuthorizationState {
    fn from(value: LocalAuthorizationState) -> Self {
        match value {
            LocalAuthorizationState::Authorized => Self::Authorized,
            LocalAuthorizationState::AwaitingApproval => Self::AwaitingApproval,
            LocalAuthorizationState::Revoked => Self::Revoked,
        }
    }
}

impl From<PersistedAuthorizationState> for LocalAuthorizationState {
    fn from(value: PersistedAuthorizationState) -> Self {
        match value {
            PersistedAuthorizationState::Authorized => Self::Authorized,
            PersistedAuthorizationState::AwaitingApproval => Self::AwaitingApproval,
            PersistedAuthorizationState::Revoked => Self::Revoked,
        }
    }
}

impl AppCore {
    pub fn new(
        update_tx: Sender<AppUpdate>,
        core_sender: Sender<CoreMsg>,
        data_dir: String,
        shared_state: Arc<RwLock<AppState>>,
    ) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        let state = AppState::empty();
        match shared_state.write() {
            Ok(mut slot) => *slot = state.clone(),
            Err(poison) => *poison.into_inner() = state.clone(),
        }

        Self {
            update_tx,
            core_sender,
            shared_state,
            runtime,
            data_dir: PathBuf::from(data_dir),
            state,
            logged_in: None,
            threads: BTreeMap::new(),
            active_chat_id: None,
            screen_stack: Vec::new(),
            next_message_id: 1,
            pending_inbound: Vec::new(),
            pending_outbound: Vec::new(),
            recent_handshake_peers: BTreeMap::new(),
            seen_event_ids: HashSet::new(),
            seen_event_order: VecDeque::new(),
            protocol_subscription_runtime: ProtocolSubscriptionRuntime::default(),
        }
    }

    pub fn handle_message(&mut self, msg: CoreMsg) {
        match msg {
            CoreMsg::Action(action) => self.handle_action(action),
            CoreMsg::Internal(event) => self.handle_internal(*event),
        }
    }

    fn handle_action(&mut self, action: AppAction) {
        self.state.toast = None;
        match action {
            AppAction::CreateAccount => self.create_account(),
            AppAction::RestoreSession { owner_nsec } => self.restore_primary_session(&owner_nsec),
            AppAction::RestoreAccountBundle {
                owner_nsec,
                owner_pubkey_hex,
                device_nsec,
            } => self.restore_account_bundle(owner_nsec, &owner_pubkey_hex, &device_nsec),
            AppAction::StartLinkedDevice { owner_input } => self.start_linked_device(&owner_input),
            AppAction::Logout => self.logout(),
            AppAction::CreateChat { peer_input } => self.create_chat(&peer_input),
            AppAction::OpenChat { chat_id } => self.open_chat(&chat_id),
            AppAction::SendMessage { chat_id, text } => self.send_message(&chat_id, &text),
            AppAction::AddAuthorizedDevice { device_input } => {
                self.add_authorized_device(&device_input)
            }
            AppAction::RemoveAuthorizedDevice { device_pubkey_hex } => {
                self.remove_authorized_device(&device_pubkey_hex)
            }
            AppAction::AcknowledgeRevokedDevice => self.acknowledge_revoked_device(),
            AppAction::PushScreen { screen } => self.push_screen(screen),
            AppAction::UpdateScreenStack { stack } => self.update_screen_stack(stack),
        }
    }

    fn handle_internal(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::RelayEvent(event) => {
                self.handle_relay_event(event);
            }
            InternalEvent::RetryPendingOutbound => {
                let now = unix_now();
                self.retry_pending_outbound(now);
                self.rebuild_state();
                self.persist_best_effort();
                self.emit_state();
            }
            InternalEvent::FetchTrackedPeerCatchUp => {
                let now = unix_now();
                self.fetch_recent_messages_for_tracked_peers(now);
            }
            InternalEvent::FetchCatchUpEvents(events) => {
                for event in events {
                    self.handle_relay_event(event);
                }
            }
            InternalEvent::PublishFinished {
                message_id,
                chat_id,
                success,
            } => {
                if success {
                    self.pending_outbound
                        .retain(|pending| pending.message_id != message_id);
                    self.update_message_delivery(&chat_id, &message_id, DeliveryState::Sent);
                } else if let Some(pending) = self
                    .pending_outbound
                    .iter_mut()
                    .find(|pending| pending.message_id == message_id)
                {
                    pending.in_flight = false;
                    pending.reason = PendingSendReason::PublishRetry;
                    let retry_after_secs = retry_delay_for_publish_mode(&pending.publish_mode);
                    pending.next_retry_at_secs =
                        unix_now().get().saturating_add(retry_after_secs);
                    self.schedule_pending_outbound_retry(Duration::from_secs(retry_after_secs));
                }
                self.schedule_next_pending_retry(unix_now().get());
                self.rebuild_state();
                self.persist_best_effort();
                self.emit_state();
            }
            InternalEvent::ProtocolSubscriptionRefreshCompleted {
                token,
                applied,
                plan,
            } => {
                if token != self.protocol_subscription_runtime.refresh_token {
                    return;
                }
                self.protocol_subscription_runtime.refresh_in_flight = false;
                self.protocol_subscription_runtime.applying_plan = None;
                if applied {
                    self.protocol_subscription_runtime.current_plan = plan;
                }
                if self.protocol_subscription_runtime.refresh_dirty {
                    self.protocol_subscription_runtime.refresh_dirty = false;
                    self.request_protocol_subscription_refresh();
                }
            }
            InternalEvent::SyncComplete => {
                self.state.busy.syncing_network = false;
                self.emit_state();
            }
            InternalEvent::Toast(message) => {
                self.state.toast = Some(message);
                self.emit_state();
            }
        }
    }

    fn create_account(&mut self) {
        self.state.busy.creating_account = true;
        self.emit_state();

        let owner_keys = Keys::generate();
        let device_keys = Keys::generate();

        if let Err(error) = self.start_primary_session(owner_keys, device_keys, false, false) {
            self.state.toast = Some(error.to_string());
        }

        self.state.busy.creating_account = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn restore_primary_session(&mut self, owner_nsec: &str) {
        self.state.busy.restoring_session = true;
        self.emit_state();

        let result = Keys::parse(owner_nsec.trim())
            .map_err(|error| anyhow::anyhow!(error.to_string()))
            .and_then(|owner_keys| self.start_primary_session(owner_keys, Keys::generate(), true, false));

        if let Err(error) = result {
            self.state.toast = Some(error.to_string());
        }

        self.state.busy.restoring_session = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn restore_account_bundle(
        &mut self,
        owner_nsec: Option<String>,
        owner_pubkey_hex: &str,
        device_nsec: &str,
    ) {
        self.state.busy.restoring_session = true;
        self.emit_state();

        let result = (|| -> anyhow::Result<()> {
            let owner_pubkey = parse_owner_input(owner_pubkey_hex)?;
            let owner_keys = match owner_nsec {
                Some(secret) => {
                    let keys =
                        Keys::parse(secret.trim()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
                    let derived_owner = OwnerPubkey::from_bytes(keys.public_key().to_bytes());
                    if derived_owner != owner_pubkey {
                        return Err(anyhow::anyhow!(
                            "stored owner secret does not match stored owner pubkey"
                        ));
                    }
                    Some(keys)
                }
                None => None,
            };
            let device_keys = Keys::parse(device_nsec.trim())
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            self.start_session(owner_pubkey, owner_keys, device_keys, true, true)
        })();

        if let Err(error) = result {
            self.state.toast = Some(error.to_string());
        }

        self.state.busy.restoring_session = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn start_linked_device(&mut self, owner_input: &str) {
        self.state.busy.linking_device = true;
        self.emit_state();

        let result = parse_owner_input(owner_input)
            .and_then(|owner_pubkey| self.start_session(owner_pubkey, None, Keys::generate(), false, false));
        if let Err(error) = result {
            self.state.toast = Some(error.to_string());
        }

        self.state.busy.linking_device = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn logout(&mut self) {
        if let Some(logged_in) = self.logged_in.take() {
            let client = logged_in.client.clone();
            self.runtime.spawn(async move {
                client.unsubscribe_all().await;
                let _ = client.shutdown().await;
            });
        }

        self.threads.clear();
        self.active_chat_id = None;
        self.screen_stack.clear();
        self.pending_inbound.clear();
        self.pending_outbound.clear();
        self.recent_handshake_peers.clear();
        self.seen_event_ids.clear();
        self.seen_event_order.clear();
        self.protocol_subscription_runtime = ProtocolSubscriptionRuntime::default();
        self.next_message_id = 1;
        self.state = AppState::empty();
        self.clear_persistence_best_effort();
        self.emit_state();
    }

    fn create_chat(&mut self, peer_input: &str) {
        if self.logged_in.is_none() {
            self.state.toast = Some("Create or restore an account first.".to_string());
            self.emit_state();
            return;
        }
        if !self.can_use_chats() {
            self.state.toast = Some(chat_unavailable_message(self.logged_in.as_ref()).to_string());
            self.emit_state();
            return;
        }

        self.state.busy.creating_chat = true;
        self.emit_state();

        let Ok((chat_id, _pubkey)) = parse_peer_input(peer_input) else {
            self.state.toast = Some("Invalid peer key.".to_string());
            self.state.busy.creating_chat = false;
            self.emit_state();
            return;
        };

        let now = unix_now().get();
        self.prune_recent_handshake_peers(now);
        let thread = self
            .threads
            .entry(chat_id.clone())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.clone(),
                unread_count: 0,
                updated_at_secs: now,
                messages: Vec::new(),
            });
        if thread.updated_at_secs == 0 {
            thread.updated_at_secs = now;
        }
        thread.unread_count = 0;

        self.active_chat_id = Some(chat_id.clone());
        self.screen_stack = vec![Screen::Chat {
            chat_id: chat_id.clone(),
        }];
        self.republish_local_identity_artifacts();
        self.rebuild_state();
        self.persist_best_effort();
        self.request_protocol_subscription_refresh();
        self.schedule_tracked_peer_catch_up(Duration::from_secs(RESUBSCRIBE_CATCH_UP_DELAY_SECS));
        self.state.busy.creating_chat = false;
        self.emit_state();
    }

    fn open_chat(&mut self, chat_id: &str) {
        if !self.can_use_chats() {
            self.state.toast = Some(chat_unavailable_message(self.logged_in.as_ref()).to_string());
            self.emit_state();
            return;
        }

        let Ok((chat_id, _pubkey)) = parse_peer_input(chat_id) else {
            self.state.toast = Some("Invalid peer key.".to_string());
            self.emit_state();
            return;
        };

        let now = unix_now().get();
        self.prune_recent_handshake_peers(now);
        self.threads
            .entry(chat_id.clone())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.clone(),
                unread_count: 0,
                updated_at_secs: now,
                messages: Vec::new(),
            })
            .unread_count = 0;

        self.active_chat_id = Some(chat_id.clone());
        self.screen_stack = vec![Screen::Chat {
            chat_id: chat_id.clone(),
        }];
        self.republish_local_identity_artifacts();
        self.rebuild_state();
        self.persist_best_effort();
        self.request_protocol_subscription_refresh();
        self.schedule_tracked_peer_catch_up(Duration::from_secs(RESUBSCRIBE_CATCH_UP_DELAY_SECS));
        self.emit_state();
    }

    fn send_message(&mut self, chat_id: &str, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        let Ok((chat_id, peer_pubkey)) = parse_peer_input(chat_id) else {
            self.state.toast = Some("Invalid peer key.".to_string());
            self.emit_state();
            return;
        };

        if self.logged_in.is_none() {
            self.state.toast = Some("Create or restore an account first.".to_string());
            self.emit_state();
            return;
        }
        if !self.can_use_chats() {
            self.state.toast = Some(chat_unavailable_message(self.logged_in.as_ref()).to_string());
            self.emit_state();
            return;
        }

        let now = unix_now();
        self.prune_recent_handshake_peers(now.get());
        self.active_chat_id = Some(chat_id.clone());
        self.screen_stack = vec![Screen::Chat {
            chat_id: chat_id.clone(),
        }];
        self.threads
            .entry(chat_id.clone())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.clone(),
                unread_count: 0,
                updated_at_secs: now.get(),
                messages: Vec::new(),
            });
        self.state.busy.sending_message = true;
        self.rebuild_state();
        self.emit_state();

        let payload = match encode_app_message_payload(&chat_id, trimmed) {
            Ok(payload) => payload,
            Err(error) => {
                self.state.busy.sending_message = false;
                self.state.toast = Some(error.to_string());
                self.rebuild_state();
                self.emit_state();
                return;
            }
        };
        let owner = OwnerPubkey::from_bytes(peer_pubkey.to_bytes());
        let prepared = {
            let logged_in = self.logged_in.as_mut().expect("logged in checked above");
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(now, &mut rng);
            logged_in
                .session_manager
                .prepare_send(&mut ctx, owner, payload)
        };

        match prepared {
            Ok(prepared) => {
                if let Some(reason) = pending_reason_from_prepared(&prepared) {
                    let republish_identity = matches!(reason, PendingSendReason::MissingRoster);
                    let message = self.push_outgoing_message(
                        &chat_id,
                        trimmed.to_string(),
                        now.get(),
                        DeliveryState::Pending,
                    );
                    self.queue_pending_outbound(
                        message.id,
                        chat_id.clone(),
                        trimmed.to_string(),
                        None,
                        OutboundPublishMode::WaitForPeer,
                        reason,
                        now.get().saturating_add(PENDING_RETRY_DELAY_SECS),
                    );
                    if republish_identity {
                        self.republish_local_identity_artifacts();
                    }
                    self.request_protocol_subscription_refresh();
                    self.schedule_pending_outbound_retry(Duration::from_secs(
                        PENDING_RETRY_DELAY_SECS,
                    ));
                } else {
                    match build_prepared_publish_batch(&prepared) {
                        Ok(Some(batch)) => {
                            let publish_mode = publish_mode_for_batch(&batch);
                            let reason = pending_reason_for_publish_mode(&publish_mode);
                            let next_retry_at_secs = retry_deadline_for_publish_mode(now.get(), &publish_mode);
                            let message = self.push_outgoing_message(
                                &chat_id,
                                trimmed.to_string(),
                                now.get(),
                                DeliveryState::Pending,
                            );
                            self.queue_pending_outbound(
                                message.id.clone(),
                                chat_id.clone(),
                                trimmed.to_string(),
                                Some(batch.clone()),
                                publish_mode.clone(),
                                reason,
                                next_retry_at_secs,
                            );
                            self.set_pending_outbound_in_flight(&message.id, true);
                            self.start_publish_for_pending(message.id, chat_id, publish_mode, batch);
                        }
                        Ok(None) => {
                            let message = self.push_outgoing_message(
                                &chat_id,
                                trimmed.to_string(),
                                now.get(),
                                DeliveryState::Failed,
                            );
                            self.update_message_delivery(
                                &chat_id,
                                &message.id,
                                DeliveryState::Failed,
                            );
                        }
                        Err(error) => {
                            self.state.toast = Some(error.to_string());
                        }
                    }
                }
            }
            Err(error) => {
                self.state.toast = Some(error.to_string());
            }
        }

        self.schedule_next_pending_retry(now.get());
        self.state.busy.sending_message = false;
        self.rebuild_state();
        self.persist_best_effort();
        self.emit_state();
    }

    fn push_screen(&mut self, screen: Screen) {
        if self.state.account.is_none() {
            return;
        }

        match screen {
            Screen::ChatList => {
                self.screen_stack.clear();
                self.active_chat_id = None;
            }
            Screen::NewChat => {
                if !self.can_use_chats() {
                    self.state.toast =
                        Some(chat_unavailable_message(self.logged_in.as_ref()).to_string());
                    self.emit_state();
                    return;
                }
                self.screen_stack = vec![Screen::NewChat];
                self.active_chat_id = None;
            }
            Screen::Chat { chat_id } => {
                self.open_chat(&chat_id);
                return;
            }
            Screen::DeviceRoster => {
                self.screen_stack = vec![Screen::DeviceRoster];
                self.active_chat_id = None;
            }
            Screen::AwaitingDeviceApproval | Screen::DeviceRevoked | Screen::Welcome => return,
        }

        self.rebuild_state();
        self.persist_best_effort();
        self.emit_state();
    }

    fn update_screen_stack(&mut self, stack: Vec<Screen>) {
        if self.state.account.is_none() {
            return;
        }

        let mut normalized_stack = Vec::new();
        for screen in stack {
            match screen {
                Screen::Welcome
                | Screen::ChatList
                | Screen::AwaitingDeviceApproval
                | Screen::DeviceRevoked => {}
                Screen::NewChat => {
                    if self.can_use_chats() {
                        normalized_stack.push(Screen::NewChat);
                    }
                }
                Screen::DeviceRoster => normalized_stack.push(Screen::DeviceRoster),
                Screen::Chat { chat_id } => {
                    if self.can_use_chats() {
                        if let Ok((chat_id, _)) = parse_peer_input(&chat_id) {
                            normalized_stack.push(Screen::Chat { chat_id });
                        }
                    }
                }
            }
        }

        self.screen_stack = normalized_stack;
        self.sync_active_chat_from_router();
        self.rebuild_state();
        self.persist_best_effort();
        self.emit_state();
    }

    fn add_authorized_device(&mut self, device_input: &str) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            self.state.toast = Some("Create or restore an account first.".to_string());
            self.emit_state();
            return;
        };
        if logged_in.owner_keys.is_none() {
            self.state.toast = Some("Only the primary device can manage devices.".to_string());
            self.emit_state();
            return;
        }

        let Ok(device_pubkey) = parse_device_input(device_input) else {
            self.state.toast = Some("Invalid device key.".to_string());
            self.emit_state();
            return;
        };
        if device_pubkey == local_device_from_keys(&logged_in.device_keys) {
            self.state.toast = Some("The current device is already authorized.".to_string());
            self.emit_state();
            return;
        }

        self.state.busy.updating_roster = true;
        self.emit_state();

        let now = unix_now();
        let updated_roster = {
            let logged_in = self.logged_in.as_mut().expect("checked above");
            let current_roster = local_roster_from_session_manager(&logged_in.session_manager);
            let mut editor = RosterEditor::from_roster(current_roster.as_ref());
            editor.authorize_device(device_pubkey, now);
            let roster = editor.build(now);
            logged_in.session_manager.apply_local_roster(roster.clone());
            logged_in.authorization_state = derive_local_authorization_state(
                logged_in.owner_keys.is_some(),
                logged_in.owner_pubkey,
                local_device_from_keys(&logged_in.device_keys),
                &logged_in.session_manager,
                Some(logged_in.authorization_state),
            );
            roster
        };

        self.publish_roster_update(updated_roster);
        self.request_protocol_subscription_refresh();
        self.persist_best_effort();
        self.state.busy.updating_roster = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn remove_authorized_device(&mut self, device_pubkey_hex: &str) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            self.state.toast = Some("Create or restore an account first.".to_string());
            self.emit_state();
            return;
        };
        if logged_in.owner_keys.is_none() {
            self.state.toast = Some("Only the primary device can manage devices.".to_string());
            self.emit_state();
            return;
        }

        let Ok(device_pubkey) = parse_device_input(device_pubkey_hex) else {
            self.state.toast = Some("Invalid device key.".to_string());
            self.emit_state();
            return;
        };
        if device_pubkey == local_device_from_keys(&logged_in.device_keys) {
            self.state.toast = Some("The current device cannot remove itself.".to_string());
            self.emit_state();
            return;
        }

        self.state.busy.updating_roster = true;
        self.emit_state();

        let now = unix_now();
        let updated_roster = {
            let logged_in = self.logged_in.as_mut().expect("checked above");
            let current_roster = local_roster_from_session_manager(&logged_in.session_manager);
            let mut editor = RosterEditor::from_roster(current_roster.as_ref());
            editor.revoke_device(device_pubkey);
            let roster = editor.build(now);
            logged_in.session_manager.apply_local_roster(roster.clone());
            logged_in.authorization_state = derive_local_authorization_state(
                logged_in.owner_keys.is_some(),
                logged_in.owner_pubkey,
                local_device_from_keys(&logged_in.device_keys),
                &logged_in.session_manager,
                Some(logged_in.authorization_state),
            );
            roster
        };

        self.publish_roster_update(updated_roster);
        self.request_protocol_subscription_refresh();
        self.persist_best_effort();
        self.state.busy.updating_roster = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn acknowledge_revoked_device(&mut self) {
        if matches!(
            self.logged_in.as_ref().map(|logged_in| logged_in.authorization_state),
            Some(LocalAuthorizationState::Revoked)
        ) {
            self.screen_stack.clear();
            self.rebuild_state();
            self.emit_state();
        }
    }

    fn handle_relay_event(&mut self, event: Event) {
        let event_id = event.id.to_string();
        if self.has_seen_event(&event_id) {
            return;
        }

        if self.logged_in.is_none() {
            return;
        }

        let kind = event.kind.as_u16() as u32;
        let now = unix_now();
        self.prune_recent_handshake_peers(now.get());
        match kind {
            codec::ROSTER_EVENT_KIND => {
                if let Ok(decoded) = codec::parse_roster_event(&event) {
                    let is_local_owner = self
                        .logged_in
                        .as_ref()
                        .map(|logged_in| decoded.owner_pubkey == logged_in.owner_pubkey)
                        .unwrap_or(false);

                    {
                        let logged_in = self.logged_in.as_mut().expect("checked above");
                        if is_local_owner {
                            logged_in.session_manager.apply_local_roster(decoded.roster);
                            let previous = logged_in.authorization_state;
                            logged_in.authorization_state = derive_local_authorization_state(
                                logged_in.owner_keys.is_some(),
                                logged_in.owner_pubkey,
                                local_device_from_keys(&logged_in.device_keys),
                                &logged_in.session_manager,
                                Some(previous),
                            );
                            match (previous, logged_in.authorization_state) {
                                (
                                    LocalAuthorizationState::AwaitingApproval,
                                    LocalAuthorizationState::Authorized,
                                ) => {
                                    self.state.toast =
                                        Some("This device has been approved.".to_string());
                                }
                                (_, LocalAuthorizationState::Revoked) => {
                                    self.state.toast =
                                        Some("This device was removed from the roster.".to_string());
                                    self.active_chat_id = None;
                                    self.screen_stack.clear();
                                    self.pending_inbound.clear();
                                    self.pending_outbound.clear();
                                }
                                _ => {}
                            }
                        } else {
                            logged_in
                                .session_manager
                                .observe_peer_roster(decoded.owner_pubkey, decoded.roster);
                        }
                    }

                    let migrated_owner_hexes = self.reconcile_recent_handshake_peers();
                    self.apply_owner_migrations(&migrated_owner_hexes);
                    self.remember_event(event_id);
                    self.retry_pending_inbound(now);
                    self.retry_pending_outbound(now);
                    self.request_protocol_subscription_refresh();
                    if is_local_owner
                        && matches!(
                            self.logged_in
                                .as_ref()
                                .map(|logged_in| logged_in.authorization_state),
                            Some(LocalAuthorizationState::Authorized)
                        )
                    {
                        self.schedule_tracked_peer_catch_up(Duration::from_secs(
                            RESUBSCRIBE_CATCH_UP_DELAY_SECS,
                        ));
                    }
                    for (_, owner_hex) in migrated_owner_hexes {
                        if let Ok(pubkey) = PublicKey::parse(&owner_hex) {
                            self.fetch_recent_messages_for_owner(
                                OwnerPubkey::from_bytes(pubkey.to_bytes()),
                                now,
                            );
                        }
                    }
                    self.persist_best_effort();
                    self.rebuild_state();
                    self.emit_state();
                    return;
                }

                if let Ok(invite) = codec::parse_invite_event(&event) {
                    let invite_owner =
                        invite
                            .inviter_owner_pubkey
                            .unwrap_or_else(|| OwnerPubkey::from_bytes(invite.inviter_device_pubkey.to_bytes()));
                    let (local_owner, local_device) = {
                        let logged_in = self.logged_in.as_ref().expect("checked above");
                        (
                            logged_in.owner_pubkey,
                            local_device_from_keys(&logged_in.device_keys),
                        )
                    };
                    let should_observe = if invite.inviter_device_pubkey == local_device {
                        false
                    } else if invite_owner == local_owner {
                        local_roster_from_session_manager(
                            &self
                                .logged_in
                                .as_ref()
                                .expect("checked above")
                                .session_manager,
                        )
                        .and_then(|roster| roster.get_device(&invite.inviter_device_pubkey).copied())
                        .is_some()
                    } else {
                        true
                    };
                    if should_observe {
                        if let Err(error) = self
                            .logged_in
                            .as_mut()
                            .expect("checked above")
                            .session_manager
                            .observe_device_invite(invite_owner, invite)
                        {
                            self.state.toast = Some(error.to_string());
                        } else {
                            self.remember_event(event_id.clone());
                            self.retry_pending_inbound(now);
                            self.retry_pending_outbound(now);
                            self.request_protocol_subscription_refresh();
                            self.persist_best_effort();
                        }
                        self.rebuild_state();
                        self.emit_state();
                        return;
                    }
                }
                self.remember_event(event_id);
            }
            codec::INVITE_RESPONSE_KIND => {
                let Some(local_invite_recipient) = self
                    .logged_in
                    .as_ref()
                    .expect("checked above")
                    .session_manager
                    .snapshot()
                    .local_invite
                    .as_ref()
                    .map(|invite| invite.inviter_ephemeral_public_key)
                else {
                    self.remember_event(event_id);
                    return;
                };

                let Ok(envelope) = codec::parse_invite_response_event(&event) else {
                    self.remember_event(event_id);
                    return;
                };
                if envelope.recipient != local_invite_recipient {
                    self.remember_event(event_id);
                    return;
                }

                let mut rng = OsRng;
                let mut ctx = ProtocolContext::new(now, &mut rng);
                let invite_response = self
                    .logged_in
                    .as_mut()
                    .expect("checked above")
                    .session_manager
                    .observe_invite_response(&mut ctx, &envelope);
                match invite_response {
                    Ok(Some(processed)) => {
                        let owner_hex = processed.owner_pubkey.to_string();
                        self.remember_recent_handshake_peer(
                            owner_hex.clone(),
                            processed.device_pubkey.to_string(),
                            now.get(),
                        );
                        let migrated_owner_hexes = self.reconcile_recent_handshake_peers();
                        self.apply_owner_migrations(&migrated_owner_hexes);
                        self.retry_pending_inbound(now);
                        self.retry_pending_outbound(now);
                        self.request_protocol_subscription_refresh();
                        self.fetch_recent_messages_for_owner(processed.owner_pubkey, now);
                        for (_, migrated_owner_hex) in migrated_owner_hexes {
                            if migrated_owner_hex != owner_hex {
                                if let Ok(pubkey) = PublicKey::parse(&migrated_owner_hex) {
                                    self.fetch_recent_messages_for_owner(
                                        OwnerPubkey::from_bytes(pubkey.to_bytes()),
                                        now,
                                    );
                                }
                            }
                        }
                        self.persist_best_effort();
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let should_ignore = matches!(
                            error,
                            Error::Domain(DomainError::InviteAlreadyUsed)
                                | Error::Domain(DomainError::InviteExhausted)
                        );
                        if !should_ignore {
                            self.state.toast = Some(error.to_string());
                        }
                    }
                }
                self.remember_event(event_id);
                self.rebuild_state();
                self.emit_state();
            }
            codec::MESSAGE_EVENT_KIND => {
                let Ok(envelope) = codec::parse_message_event(&event) else {
                    self.remember_event(event_id);
                    return;
                };

                let sender_owner = self.logged_in.as_ref().and_then(|logged_in| {
                    resolve_message_sender_owner(&logged_in.session_manager, &envelope, now)
                });
                let Some(sender_owner) = sender_owner else {
                    self.remember_event(event_id.clone());
                    self.pending_inbound.push(PendingInbound { envelope });
                    self.persist_best_effort();
                    return;
                };

                let mut rng = OsRng;
                let mut ctx = ProtocolContext::new(now, &mut rng);
                match self
                    .logged_in
                    .as_mut()
                    .expect("checked above")
                    .session_manager
                    .receive(&mut ctx, sender_owner, &envelope)
                {
                    Ok(Some(message)) => {
                        self.remember_event(event_id);
                        let owner_hex = message.owner_pubkey.to_string();
                        self.clear_recent_handshake_peer(&owner_hex);
                        let routed = route_received_message(
                            self.logged_in
                                .as_ref()
                                .expect("checked above")
                                .owner_pubkey,
                            message.owner_pubkey,
                            &message.payload,
                        );
                        self.apply_routed_chat_message(routed, now.get());
                        self.request_protocol_subscription_refresh();
                        self.persist_best_effort();
                        self.rebuild_state();
                        self.emit_state();
                    }
                    Ok(None) => {
                        self.remember_event(event_id.clone());
                        self.pending_inbound.push(PendingInbound { envelope });
                        self.persist_best_effort();
                    }
                    Err(error) => {
                        self.remember_event(event_id);
                        self.state.toast = Some(error.to_string());
                        self.emit_state();
                    }
                }
            }
            _ => {}
        }
    }

    fn start_primary_session(
        &mut self,
        owner_keys: Keys,
        device_keys: Keys,
        allow_restore: bool,
        allow_protocol_restore: bool,
    ) -> anyhow::Result<()> {
        let owner_pubkey = OwnerPubkey::from_bytes(owner_keys.public_key().to_bytes());
        self.start_session(
            owner_pubkey,
            Some(owner_keys),
            device_keys,
            allow_restore,
            allow_protocol_restore,
        )
    }

    fn start_session(
        &mut self,
        owner_pubkey: OwnerPubkey,
        owner_keys: Option<Keys>,
        device_keys: Keys,
        allow_restore: bool,
        allow_protocol_restore: bool,
    ) -> anyhow::Result<()> {
        if let Some(existing) = self.logged_in.take() {
            let client = existing.client;
            self.runtime.spawn(async move {
                client.unsubscribe_all().await;
                let _ = client.shutdown().await;
            });
        }

        self.threads.clear();
        self.pending_inbound.clear();
        self.active_chat_id = None;
        self.screen_stack.clear();
        self.pending_outbound.clear();
        self.recent_handshake_peers.clear();
        self.seen_event_ids.clear();
        self.seen_event_order.clear();
        self.protocol_subscription_runtime = ProtocolSubscriptionRuntime::default();
        self.next_message_id = 1;

        let device_secret_bytes = device_keys.secret_key().to_secret_bytes();
        let local_device = DevicePubkey::from_bytes(device_keys.public_key().to_bytes());
        let now = unix_now();

        let persisted = if allow_restore {
            self.load_persisted().ok().flatten()
        } else {
            None
        };
        let persisted_authorization_state = persisted
            .as_ref()
            .and_then(|persisted| persisted.authorization_state.clone())
            .map(Into::into);

        if let Some(persisted) = &persisted {
            self.active_chat_id = persisted.active_chat_id.clone();
            self.next_message_id = persisted.next_message_id.max(1);
            if allow_protocol_restore {
                self.pending_outbound = persisted.pending_outbound.clone();
                for pending in &mut self.pending_outbound {
                    pending.publish_mode = migrate_publish_mode(
                        pending.publish_mode.clone(),
                        pending.prepared_publish.as_ref(),
                    );
                }
                self.pending_inbound = persisted.pending_inbound.clone();
                self.seen_event_order = persisted
                    .seen_event_ids
                    .iter()
                    .rev()
                    .take(MAX_SEEN_EVENT_IDS)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                self.seen_event_ids = self.seen_event_order.iter().cloned().collect();
            }
            self.threads = persisted
                .threads
                .iter()
                .map(|thread| {
                    let updated_at_secs = thread.updated_at_secs.max(
                        thread
                            .messages
                            .iter()
                            .map(|message| message.created_at_secs)
                            .max()
                            .unwrap_or(0),
                    );
                    (
                        thread.chat_id.clone(),
                        ThreadRecord {
                            chat_id: thread.chat_id.clone(),
                            unread_count: thread.unread_count,
                            updated_at_secs,
                            messages: thread
                                .messages
                                .iter()
                                .map(|message| ChatMessageSnapshot {
                                    id: message.id.clone(),
                                    chat_id: message.chat_id.clone(),
                                    author: message.author.clone(),
                                    body: message.body.clone(),
                                    is_outgoing: message.is_outgoing,
                                    created_at_secs: message.created_at_secs,
                                    delivery: message.delivery.clone().into(),
                                })
                                .collect(),
                        },
                    )
                })
                .collect();
        }

        let mut session_manager = persisted
            .and_then(|persisted| {
                if allow_protocol_restore {
                    persisted.session_manager
                } else {
                    None
                }
            })
            .filter(|snapshot| {
                snapshot.local_owner_pubkey == owner_pubkey && snapshot.local_device_pubkey == local_device
            })
            .map(|snapshot| SessionManager::from_snapshot(snapshot, device_secret_bytes))
            .transpose()?
            .unwrap_or_else(|| SessionManager::new(owner_pubkey, device_secret_bytes));

        let existing_local_roster = session_manager
            .snapshot()
            .users
            .into_iter()
            .find(|user| user.owner_pubkey == owner_pubkey)
            .and_then(|user| user.roster);
        if owner_keys.is_some() && existing_local_roster.is_none() {
            let mut roster_editor = RosterEditor::new();
            roster_editor.authorize_device(local_device, now);
            session_manager.apply_local_roster(roster_editor.build(now));
        }

        let authorization_state =
            derive_local_authorization_state(
                owner_keys.is_some(),
                owner_pubkey,
                local_device,
                &session_manager,
                persisted_authorization_state,
            );

        if authorization_state != LocalAuthorizationState::Revoked {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(now, &mut rng);
            session_manager.ensure_local_invite(&mut ctx)?;
        }

        if authorization_state != LocalAuthorizationState::Authorized {
            self.active_chat_id = None;
            self.screen_stack.clear();
            self.pending_inbound.clear();
            self.pending_outbound.clear();
        } else if let Some(chat_id) = self.active_chat_id.clone() {
            self.screen_stack = vec![Screen::Chat { chat_id }];
        }

        let client = Client::new(device_keys.clone());
        let relay_urls = configured_relay_urls();
        self.runtime
            .block_on(ensure_session_relays_configured(&client, &relay_urls));
        self.start_notifications_loop(client.clone());

        self.logged_in = Some(LoggedInState {
            owner_pubkey,
            owner_keys: owner_keys.clone(),
            device_keys: device_keys.clone(),
            client,
            relay_urls,
            session_manager,
            authorization_state,
        });
        self.schedule_session_connect();

        self.emit_account_bundle_update(owner_keys.as_ref(), &device_keys);
        self.republish_local_identity_artifacts();
        self.reconcile_recent_handshake_peers();
        self.retry_pending_inbound(now);
        self.retry_pending_outbound(now);
        self.state.busy.syncing_network = true;
        self.rebuild_state();
        self.persist_best_effort();
        self.request_protocol_subscription_refresh();
        if authorization_state == LocalAuthorizationState::Authorized {
            self.schedule_tracked_peer_catch_up(Duration::from_secs(
                RESUBSCRIBE_CATCH_UP_DELAY_SECS,
            ));
        }
        self.emit_state();
        Ok(())
    }

    fn retry_pending_inbound(&mut self, now: UnixSeconds) {
        if self.logged_in.is_none() {
            return;
        }

        let pending = std::mem::take(&mut self.pending_inbound);
        let mut still_pending = Vec::new();
        for item in pending {
            let sender_owner = self.logged_in.as_ref().and_then(|logged_in| {
                resolve_message_sender_owner(&logged_in.session_manager, &item.envelope, now)
            });
            let Some(sender_owner) = sender_owner else {
                still_pending.push(item);
                continue;
            };
            let receive_result = {
                let logged_in = self.logged_in.as_mut().expect("checked above");
                let mut rng = OsRng;
                let mut ctx = ProtocolContext::new(now, &mut rng);
                logged_in
                    .session_manager
                    .receive(&mut ctx, sender_owner, &item.envelope)
            };
            match receive_result {
                Ok(Some(message)) => {
                    let routed = route_received_message(
                        self.logged_in
                            .as_ref()
                            .expect("checked above")
                            .owner_pubkey,
                        message.owner_pubkey,
                        &message.payload,
                    );
                    self.apply_routed_chat_message(routed, now.get());
                }
                Ok(None) => still_pending.push(item),
                Err(_) => still_pending.push(item),
            }
        }
        self.pending_inbound = still_pending;
    }

    fn retry_pending_outbound(&mut self, now: UnixSeconds) {
        if self.logged_in.is_none() || self.pending_outbound.is_empty() {
            return;
        }

        self.prune_recent_handshake_peers(now.get());
        let pending = std::mem::take(&mut self.pending_outbound);
        let mut still_pending = Vec::new();

        for mut pending_message in pending {
            if pending_message.next_retry_at_secs > now.get() {
                still_pending.push(pending_message);
                continue;
            }

            if pending_message.in_flight {
                still_pending.push(pending_message);
                continue;
            }

            if let Some(batch) = pending_message.prepared_publish.clone() {
                pending_message.publish_mode =
                    migrate_publish_mode(pending_message.publish_mode.clone(), Some(&batch));
                pending_message.reason =
                    pending_reason_for_publish_mode(&pending_message.publish_mode);
                pending_message.next_retry_at_secs =
                    retry_deadline_for_publish_mode(now.get(), &pending_message.publish_mode);
                pending_message.in_flight = true;
                self.start_publish_for_pending(
                    pending_message.message_id.clone(),
                    pending_message.chat_id.clone(),
                    pending_message.publish_mode.clone(),
                    batch,
                );
                still_pending.push(pending_message);
                continue;
            }

            let owner = match parse_peer_input(&pending_message.chat_id) {
                Ok((_, peer_pubkey)) => OwnerPubkey::from_bytes(peer_pubkey.to_bytes()),
                Err(_) => {
                    self.update_message_delivery(
                        &pending_message.chat_id,
                        &pending_message.message_id,
                        DeliveryState::Failed,
                    );
                    continue;
                }
            };

            let payload = match encode_app_message_payload(
                &pending_message.chat_id,
                &pending_message.body,
            ) {
                Ok(payload) => payload,
                Err(error) => {
                    self.state.toast = Some(error.to_string());
                    self.update_message_delivery(
                        &pending_message.chat_id,
                        &pending_message.message_id,
                        DeliveryState::Failed,
                    );
                    continue;
                }
            };

            let prepared = {
                let logged_in = self.logged_in.as_mut().expect("checked above");
                let mut rng = OsRng;
                let mut ctx = ProtocolContext::new(now, &mut rng);
                logged_in
                    .session_manager
                    .prepare_send(&mut ctx, owner, payload)
            };

            match prepared {
                Ok(prepared) => {
                    if let Some(reason) = pending_reason_from_prepared(&prepared) {
                        pending_message.reason = reason.clone();
                        pending_message.next_retry_at_secs =
                            now.get().saturating_add(PENDING_RETRY_DELAY_SECS);
                        if matches!(reason, PendingSendReason::MissingRoster) {
                            self.republish_local_identity_artifacts();
                        }
                        pending_message.publish_mode = OutboundPublishMode::WaitForPeer;
                        still_pending.push(pending_message);
                    } else {
                        match build_prepared_publish_batch(&prepared) {
                            Ok(Some(batch)) => {
                                pending_message.publish_mode =
                                    publish_mode_for_batch(&batch);
                                pending_message.prepared_publish = Some(batch.clone());
                                pending_message.reason = pending_reason_for_publish_mode(
                                    &pending_message.publish_mode,
                                );
                                pending_message.next_retry_at_secs = retry_deadline_for_publish_mode(
                                    now.get(),
                                    &pending_message.publish_mode,
                                );
                                pending_message.in_flight = true;
                                self.start_publish_for_pending(
                                    pending_message.message_id.clone(),
                                    pending_message.chat_id.clone(),
                                    pending_message.publish_mode.clone(),
                                    batch,
                                );
                                still_pending.push(pending_message);
                            }
                            Ok(None) => {
                                pending_message.publish_mode = OutboundPublishMode::WaitForPeer;
                                pending_message.reason = PendingSendReason::MissingDeviceInvite;
                                pending_message.next_retry_at_secs =
                                    now.get().saturating_add(PENDING_RETRY_DELAY_SECS);
                                still_pending.push(pending_message);
                            }
                            Err(error) => {
                                self.state.toast = Some(error.to_string());
                                self.update_message_delivery(
                                    &pending_message.chat_id,
                                    &pending_message.message_id,
                                    DeliveryState::Failed,
                                );
                            }
                        }
                    }
                }
                Err(error) => {
                    self.state.toast = Some(error.to_string());
                    self.update_message_delivery(
                        &pending_message.chat_id,
                        &pending_message.message_id,
                        DeliveryState::Failed,
                    );
                }
            }
        }

        self.pending_outbound = still_pending;
        self.schedule_next_pending_retry(now.get());
    }

    fn queue_pending_outbound(
        &mut self,
        message_id: String,
        chat_id: String,
        body: String,
        prepared_publish: Option<PreparedPublishBatch>,
        publish_mode: OutboundPublishMode,
        reason: PendingSendReason,
        next_retry_at_secs: u64,
    ) {
        self.pending_outbound.push(PendingOutbound {
            message_id,
            chat_id,
            body,
            prepared_publish,
            publish_mode,
            reason,
            next_retry_at_secs,
            in_flight: false,
        });
    }

    fn set_pending_outbound_in_flight(&mut self, message_id: &str, in_flight: bool) {
        if let Some(pending) = self
            .pending_outbound
            .iter_mut()
            .find(|pending| pending.message_id == message_id)
        {
            pending.in_flight = in_flight;
        }
    }

    fn prune_recent_handshake_peers(&mut self, now_secs: u64) {
        self.reconcile_recent_handshake_peers();
        self.recent_handshake_peers.retain(|_, peer| {
            let within_ttl =
                now_secs.saturating_sub(peer.observed_at_secs) <= RECENT_HANDSHAKE_TTL_SECS;
            within_ttl && !self.threads.contains_key(&peer.owner_hex)
        });
    }

    fn remember_recent_handshake_peer(
        &mut self,
        owner_hex: String,
        device_hex: String,
        now_secs: u64,
    ) {
        if self.threads.contains_key(&owner_hex) {
            self.recent_handshake_peers
                .retain(|_, peer| peer.owner_hex != owner_hex);
            return;
        }
        self.recent_handshake_peers.insert(
            device_hex.clone(),
            RecentHandshakePeer {
                owner_hex,
                device_hex,
                observed_at_secs: now_secs,
            },
        );
    }

    fn clear_recent_handshake_peer(&mut self, owner_hex: &str) {
        self.recent_handshake_peers
            .retain(|_, peer| peer.owner_hex != owner_hex);
    }

    fn tracked_peer_owner_hexes(&self) -> HashSet<String> {
        let mut owners = self.threads.keys().cloned().collect::<HashSet<_>>();
        if let Some(chat_id) = self.active_chat_id.as_ref() {
            owners.insert(chat_id.clone());
        }
        for pending in &self.pending_outbound {
            owners.insert(pending.chat_id.clone());
        }
        owners
    }

    fn protocol_owner_hexes(&self) -> HashSet<String> {
        let mut owners = self.tracked_peer_owner_hexes();
        owners.extend(
            self.recent_handshake_peers
                .values()
                .map(|peer| peer.owner_hex.clone()),
        );
        if let Some(logged_in) = self.logged_in.as_ref() {
            for user in logged_in.session_manager.snapshot().users {
                for device in user.devices {
                    if let Some(claimed_owner_pubkey) = device.claimed_owner_pubkey {
                        owners.insert(claimed_owner_pubkey.to_string());
                    }
                }
            }
        }
        owners
    }

    fn schedule_pending_outbound_retry(&self, after: Duration) {
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            sleep(after).await;
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::RetryPendingOutbound,
            )));
        });
    }

    fn schedule_tracked_peer_catch_up(&self, after: Duration) {
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            sleep(after).await;
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::FetchTrackedPeerCatchUp,
            )));
        });
    }

    fn schedule_next_pending_retry(&self, now_secs: u64) {
        let Some(next_retry_at_secs) = self
            .pending_outbound
            .iter()
            .map(|pending| pending.next_retry_at_secs)
            .min()
        else {
            return;
        };
        let delay_secs = next_retry_at_secs.saturating_sub(now_secs).max(1);
        self.schedule_pending_outbound_retry(Duration::from_secs(delay_secs));
    }

    fn start_pending_message_publish(
        &mut self,
        message_id: String,
        chat_id: String,
        message_events: Vec<Event>,
    ) {
        self.start_ordinary_publish(message_id, chat_id, message_events);
    }

    fn start_publish_for_pending(
        &mut self,
        message_id: String,
        chat_id: String,
        publish_mode: OutboundPublishMode,
        batch: PreparedPublishBatch,
    ) {
        self.request_protocol_subscription_refresh();
        if batch.message_events.is_empty() {
            return;
        }

        match publish_mode {
            OutboundPublishMode::OrdinaryFirstAck => {
                self.start_pending_message_publish(message_id, chat_id, batch.message_events);
            }
            OutboundPublishMode::FirstContactStaged => {
                self.start_staged_first_contact_send(StagedOutboundSend {
                    message_id,
                    chat_id,
                    invite_events: batch.invite_events,
                    message_events: batch.message_events,
                });
            }
            OutboundPublishMode::WaitForPeer => {}
        }
    }

    fn reconcile_recent_handshake_peers(&mut self) -> Vec<(String, String)> {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return Vec::new();
        };

        let mut session_owners_by_device = BTreeMap::new();
        for user in logged_in.session_manager.snapshot().users {
            let owner_hex = user.owner_pubkey.to_string();
            for device in user.devices {
                if device.active_session.is_none()
                    && device.inactive_sessions.is_empty()
                    && device.claimed_owner_pubkey.is_none()
                {
                    continue;
                }
                session_owners_by_device
                    .insert(device.device_pubkey.to_string(), owner_hex.clone());
            }
        }

        let mut migrated_owner_hexes = Vec::new();
        for peer in self.recent_handshake_peers.values_mut() {
            let Some(owner_hex) = session_owners_by_device.get(&peer.device_hex) else {
                continue;
            };
            if *owner_hex != peer.owner_hex {
                let previous_owner_hex = peer.owner_hex.clone();
                peer.owner_hex = owner_hex.clone();
                migrated_owner_hexes.push((previous_owner_hex, owner_hex.clone()));
            }
        }

        migrated_owner_hexes
    }

    fn apply_owner_migrations(&mut self, migrations: &[(String, String)]) {
        for (old_owner_hex, new_owner_hex) in migrations {
            if old_owner_hex == new_owner_hex {
                continue;
            }

            if let Some(mut old_thread) = self.threads.remove(old_owner_hex) {
                old_thread.chat_id = new_owner_hex.clone();
                for message in &mut old_thread.messages {
                    message.chat_id = new_owner_hex.clone();
                }

                match self.threads.get_mut(new_owner_hex) {
                    Some(existing) => {
                        existing.unread_count =
                            existing.unread_count.saturating_add(old_thread.unread_count);
                        existing.updated_at_secs =
                            existing.updated_at_secs.max(old_thread.updated_at_secs);
                        existing.messages.extend(old_thread.messages);
                        existing.messages.sort_by(|left, right| {
                            left.created_at_secs
                                .cmp(&right.created_at_secs)
                                .then_with(|| left.id.cmp(&right.id))
                        });
                    }
                    None => {
                        self.threads.insert(new_owner_hex.clone(), old_thread);
                    }
                }
            }

            if self.active_chat_id.as_deref() == Some(old_owner_hex.as_str()) {
                self.active_chat_id = Some(new_owner_hex.clone());
            }
            for pending in &mut self.pending_outbound {
                if pending.chat_id == *old_owner_hex {
                    pending.chat_id = new_owner_hex.clone();
                }
            }
            for screen in &mut self.screen_stack {
                if let Screen::Chat { chat_id } = screen {
                    if *chat_id == *old_owner_hex {
                        *chat_id = new_owner_hex.clone();
                    }
                }
            }
        }
    }

    fn start_staged_first_contact_send(&mut self, staged: StagedOutboundSend) {
        let Some((client, relay_urls)) = self
            .logged_in
            .as_ref()
            .map(|logged_in| (logged_in.client.clone(), logged_in.relay_urls.clone()))
        else {
            return;
        };

        for event in staged
            .invite_events
            .iter()
            .chain(staged.message_events.iter())
        {
            self.remember_event(event.id.to_string());
        }

        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            let invite_publish = publish_events_with_retry(
                &client,
                &relay_urls,
                staged.invite_events,
                "invite response",
            )
            .await;
            if invite_publish.is_err() {
                let _ = tx.send(CoreMsg::Internal(Box::new(
                    InternalEvent::PublishFinished {
                        message_id: staged.message_id,
                        chat_id: staged.chat_id,
                        success: false,
                    },
                )));
                return;
            }

            sleep(Duration::from_millis(FIRST_CONTACT_STAGE_DELAY_MS)).await;

            let success = publish_events_with_retry(
                &client,
                &relay_urls,
                staged.message_events,
                "message",
            )
            .await
            .is_ok();
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::PublishFinished {
                    message_id: staged.message_id,
                    chat_id: staged.chat_id,
                    success,
                },
            )));
        });
    }

    fn start_ordinary_publish(&mut self, message_id: String, chat_id: String, events: Vec<Event>) {
        let Some((client, relay_urls)) = self
            .logged_in
            .as_ref()
            .map(|logged_in| (logged_in.client.clone(), logged_in.relay_urls.clone()))
        else {
            return;
        };

        for event in &events {
            self.remember_event(event.id.to_string());
        }

        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            let success = publish_events_first_ack(&client, &relay_urls, &events, "message")
                .await
                .is_ok();
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::PublishFinished {
                    message_id,
                    chat_id,
                    success,
                },
            )));
        });
    }

    fn fetch_recent_messages_for_owner(&self, owner_pubkey: OwnerPubkey, now: UnixSeconds) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            return;
        };

        let filters = self.message_filters_for_owner(owner_pubkey, now);
        if filters.is_empty() {
            return;
        }

        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            client.connect_with_timeout(Duration::from_secs(5)).await;
            if let Ok(events) = client
                .fetch_events(filters, Some(Duration::from_secs(5)))
                .await
            {
                let collected = events.into_iter().collect::<Vec<_>>();
                if !collected.is_empty() {
                    let _ = tx.send(CoreMsg::Internal(Box::new(
                        InternalEvent::FetchCatchUpEvents(collected),
                    )));
                }
            }
        });
    }

    fn fetch_recent_messages_for_tracked_peers(&self, now: UnixSeconds) {
        for owner_hex in self.tracked_peer_owner_hexes() {
            let Ok(pubkey) = PublicKey::parse(&owner_hex) else {
                continue;
            };
            self.fetch_recent_messages_for_owner(OwnerPubkey::from_bytes(pubkey.to_bytes()), now);
        }
    }

    fn message_filters_for_owner(
        &self,
        owner_pubkey: OwnerPubkey,
        now: UnixSeconds,
    ) -> Vec<Filter> {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return Vec::new();
        };

        let Some(user) = logged_in
            .session_manager
            .snapshot()
            .users
            .into_iter()
            .find(|user| user.owner_pubkey == owner_pubkey)
        else {
            return Vec::new();
        };

        let authors = user
            .devices
            .into_iter()
            .flat_map(|device| {
                let mut senders = HashSet::new();
                if let Some(session) = device.active_session.as_ref() {
                    collect_expected_senders(session, &mut senders);
                }
                for session in &device.inactive_sessions {
                    collect_expected_senders(session, &mut senders);
                }
                senders.into_iter().collect::<Vec<_>>()
            })
            .filter_map(|hex| PublicKey::parse(&hex).ok())
            .collect::<Vec<_>>();

        if authors.is_empty() {
            return Vec::new();
        }

        vec![Filter::new()
            .kind(Kind::from(codec::MESSAGE_EVENT_KIND as u16))
            .authors(authors)
            .since(Timestamp::from(
                now.get().saturating_sub(CATCH_UP_LOOKBACK_SECS),
            ))]
    }

    fn update_message_delivery(
        &mut self,
        chat_id: &str,
        message_id: &str,
        delivery: DeliveryState,
    ) {
        let Some(thread) = self.threads.get_mut(chat_id) else {
            return;
        };
        if let Some(message) = thread
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            message.delivery = delivery;
        }
    }

    fn push_outgoing_message(
        &mut self,
        chat_id: &str,
        body: String,
        created_at_secs: u64,
        delivery: DeliveryState,
    ) -> ChatMessageSnapshot {
        let message = ChatMessageSnapshot {
            id: self.allocate_message_id(),
            chat_id: chat_id.to_string(),
            author: self
                .state
                .account
                .as_ref()
                .map(|account| account.npub.clone())
                .unwrap_or_else(|| "me".to_string()),
            body,
            is_outgoing: true,
            created_at_secs,
            delivery,
        };
        self.threads
            .entry(chat_id.to_string())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.to_string(),
                unread_count: 0,
                updated_at_secs: created_at_secs,
                messages: Vec::new(),
            })
            .messages
            .push(message.clone());
        if let Some(thread) = self.threads.get_mut(chat_id) {
            thread.updated_at_secs = created_at_secs;
        }
        message
    }

    fn push_incoming_message(&mut self, chat_id: &str, body: String, created_at_secs: u64) {
        let message_id = self.allocate_message_id();
        let author = owner_npub(chat_id).unwrap_or_else(|| chat_id.to_string());
        let thread = self
            .threads
            .entry(chat_id.to_string())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.to_string(),
                unread_count: 0,
                updated_at_secs: created_at_secs,
                messages: Vec::new(),
            });
        if self.active_chat_id.as_deref() != Some(chat_id) {
            thread.unread_count = thread.unread_count.saturating_add(1);
        }
        thread.updated_at_secs = created_at_secs;
        thread.messages.push(ChatMessageSnapshot {
            id: message_id,
            chat_id: chat_id.to_string(),
            author,
            body,
            is_outgoing: false,
            created_at_secs,
            delivery: DeliveryState::Received,
        });
    }

    fn apply_routed_chat_message(
        &mut self,
        routed: RoutedChatMessage,
        created_at_secs: u64,
    ) {
        if routed.is_outgoing {
            self.push_outgoing_message(
                &routed.chat_id,
                routed.body,
                created_at_secs,
                DeliveryState::Sent,
            );
        } else {
            self.push_incoming_message(&routed.chat_id, routed.body, created_at_secs);
        }
    }

    fn allocate_message_id(&mut self) -> String {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.saturating_add(1);
        id.to_string()
    }

    fn rebuild_state(&mut self) {
        self.state.account = self.build_account_snapshot();
        self.state.device_roster = self.build_device_roster_snapshot();

        let default_screen = match self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.authorization_state)
        {
            None => Screen::Welcome,
            Some(LocalAuthorizationState::Authorized) => Screen::ChatList,
            Some(LocalAuthorizationState::AwaitingApproval) => Screen::AwaitingDeviceApproval,
            Some(LocalAuthorizationState::Revoked) => Screen::DeviceRevoked,
        };

        let mut threads: Vec<&ThreadRecord> = self.threads.values().collect();
        threads.sort_by_key(|thread| std::cmp::Reverse(thread.updated_at_secs));

        self.state.chat_list = threads
            .iter()
            .map(|thread| {
                let last_message = thread.messages.last();
                ChatThreadSnapshot {
                    chat_id: thread.chat_id.clone(),
                    display_name: owner_npub(&thread.chat_id)
                        .unwrap_or_else(|| thread.chat_id.clone()),
                    peer_npub: owner_npub(&thread.chat_id)
                        .unwrap_or_else(|| thread.chat_id.clone()),
                    last_message_preview: last_message.map(|message| message.body.clone()),
                    last_message_at_secs: last_message.map(|message| message.created_at_secs),
                    last_message_is_outgoing: last_message.map(|message| message.is_outgoing),
                    last_message_delivery: last_message.map(|message| message.delivery.clone()),
                    unread_count: thread.unread_count,
                }
            })
            .collect();

        self.state.current_chat = self
            .active_chat_id
            .as_ref()
            .and_then(|chat_id| self.threads.get(chat_id))
            .map(|thread| CurrentChatSnapshot {
                chat_id: thread.chat_id.clone(),
                display_name: owner_npub(&thread.chat_id).unwrap_or_else(|| thread.chat_id.clone()),
                peer_npub: owner_npub(&thread.chat_id).unwrap_or_else(|| thread.chat_id.clone()),
                messages: thread.messages.clone(),
            });

        self.state.router = Router {
            default_screen,
            screen_stack: self.screen_stack.clone(),
        };
    }

    fn build_account_snapshot(&self) -> Option<AccountSnapshot> {
        let logged_in = self.logged_in.as_ref()?;
        let owner_public_key_hex = logged_in.owner_pubkey.to_string();
        let owner_npub =
            owner_npub_from_owner(logged_in.owner_pubkey).unwrap_or_else(|| owner_public_key_hex.clone());
        let device_public_key_hex = logged_in.device_keys.public_key().to_hex();
        let device_npub = logged_in
            .device_keys
            .public_key()
            .to_bech32()
            .unwrap_or_else(|_| device_public_key_hex.clone());

        Some(AccountSnapshot {
            public_key_hex: owner_public_key_hex,
            npub: owner_npub,
            device_public_key_hex,
            device_npub,
            has_owner_signing_authority: logged_in.owner_keys.is_some(),
            authorization_state: public_authorization_state(logged_in.authorization_state),
        })
    }

    fn build_device_roster_snapshot(&self) -> Option<DeviceRosterSnapshot> {
        let logged_in = self.logged_in.as_ref()?;
        let account = self.build_account_snapshot()?;
        let current_device_pubkey_hex = account.device_public_key_hex.clone();
        let current_device_npub = account.device_npub.clone();
        let mut entries = BTreeMap::<String, DeviceEntrySnapshot>::new();

        if let Some(user) = logged_in
            .session_manager
            .snapshot()
            .users
            .into_iter()
            .find(|user| user.owner_pubkey == logged_in.owner_pubkey)
        {
            if let Some(roster) = user.roster.as_ref() {
                for authorized_device in roster.devices() {
                    let device_pubkey_hex = authorized_device.device_pubkey.to_string();
                    entries.entry(device_pubkey_hex.clone()).or_insert(DeviceEntrySnapshot {
                        device_pubkey_hex: device_pubkey_hex.clone(),
                        device_npub: device_npub(&device_pubkey_hex)
                            .unwrap_or_else(|| device_pubkey_hex.clone()),
                        is_current_device: device_pubkey_hex == current_device_pubkey_hex,
                        is_authorized: true,
                        is_stale: false,
                        last_activity_secs: None,
                    });
                }
            }

            for device in user.devices {
                let device_pubkey_hex = device.device_pubkey.to_string();
                let entry = entries
                    .entry(device_pubkey_hex.clone())
                    .or_insert(DeviceEntrySnapshot {
                        device_pubkey_hex: device_pubkey_hex.clone(),
                        device_npub: device_npub(&device_pubkey_hex)
                            .unwrap_or_else(|| device_pubkey_hex.clone()),
                        is_current_device: device_pubkey_hex == current_device_pubkey_hex,
                        is_authorized: device.authorized,
                        is_stale: device.is_stale,
                        last_activity_secs: device.last_activity.map(UnixSeconds::get),
                    });
                entry.is_authorized = device.authorized;
                entry.is_stale = device.is_stale;
                entry.last_activity_secs = device.last_activity.map(UnixSeconds::get);
            }
        }

        entries.entry(current_device_pubkey_hex.clone()).or_insert(DeviceEntrySnapshot {
            device_pubkey_hex: current_device_pubkey_hex.clone(),
            device_npub: current_device_npub.clone(),
            is_current_device: true,
            is_authorized: matches!(logged_in.authorization_state, LocalAuthorizationState::Authorized),
            is_stale: matches!(logged_in.authorization_state, LocalAuthorizationState::Revoked),
            last_activity_secs: None,
        });

        let mut devices = entries.into_values().collect::<Vec<_>>();
        devices.sort_by(|left, right| {
            right
                .is_current_device
                .cmp(&left.is_current_device)
                .then_with(|| left.device_pubkey_hex.cmp(&right.device_pubkey_hex))
        });

        Some(DeviceRosterSnapshot {
            owner_public_key_hex: account.public_key_hex,
            owner_npub: account.npub,
            current_device_public_key_hex: current_device_pubkey_hex,
            current_device_npub,
            can_manage_devices: logged_in.owner_keys.is_some(),
            authorization_state: public_authorization_state(logged_in.authorization_state),
            devices,
        })
    }

    fn can_use_chats(&self) -> bool {
        matches!(
            self.logged_in
                .as_ref()
                .map(|logged_in| logged_in.authorization_state),
            Some(LocalAuthorizationState::Authorized)
        )
    }

    fn emit_account_bundle_update(&self, owner_keys: Option<&Keys>, device_keys: &Keys) {
        let device_nsec = device_keys
            .secret_key()
            .to_bech32()
            .unwrap_or_else(|_| device_keys.secret_key().to_secret_hex());
        let owner_nsec = owner_keys.map(|keys| {
            keys.secret_key()
                .to_bech32()
                .unwrap_or_else(|_| keys.secret_key().to_secret_hex())
        });
        let owner_pubkey_hex = owner_keys
            .map(|keys| keys.public_key().to_hex())
            .or_else(|| self.logged_in.as_ref().map(|logged_in| logged_in.owner_pubkey.to_string()))
            .unwrap_or_default();
        let _ = self.update_tx.send(AppUpdate::PersistAccountBundle {
            rev: self.state.rev,
            owner_nsec,
            owner_pubkey_hex,
            device_nsec,
        });
    }

    fn emit_state(&mut self) {
        self.state.rev = self.state.rev.saturating_add(1);
        let snapshot = self.state.clone();
        match self.shared_state.write() {
            Ok(mut slot) => *slot = snapshot.clone(),
            Err(poison) => *poison.into_inner() = snapshot.clone(),
        }
        let _ = self.update_tx.send(AppUpdate::FullState(snapshot));
    }

    fn persistence_path(&self) -> PathBuf {
        self.data_dir.join("ndr_demo_core_state.json")
    }

    fn load_persisted(&self) -> anyhow::Result<Option<PersistedState>> {
        let path = self.persistence_path();
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    fn persist_best_effort(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };

        let persisted = PersistedState {
            version: 8,
            active_chat_id: self.active_chat_id.clone(),
            next_message_id: self.next_message_id,
            session_manager: Some(logged_in.session_manager.snapshot()),
            threads: self
                .threads
                .values()
                .map(|thread| PersistedThread {
                    chat_id: thread.chat_id.clone(),
                    unread_count: thread.unread_count,
                    updated_at_secs: thread.updated_at_secs,
                    messages: thread
                        .messages
                        .iter()
                        .map(|message| PersistedMessage {
                            id: message.id.clone(),
                            chat_id: message.chat_id.clone(),
                            author: message.author.clone(),
                            body: message.body.clone(),
                            is_outgoing: message.is_outgoing,
                            created_at_secs: message.created_at_secs,
                            delivery: (&message.delivery).into(),
                        })
                        .collect(),
                })
                .collect(),
            pending_inbound: self.pending_inbound.clone(),
            pending_outbound: self.pending_outbound.clone(),
            seen_event_ids: self.seen_event_order.iter().cloned().collect(),
            authorization_state: Some(logged_in.authorization_state.into()),
        };

        if let Ok(bytes) = serde_json::to_vec_pretty(&persisted) {
            let _ = fs::create_dir_all(&self.data_dir);
            let _ = fs::write(self.persistence_path(), bytes);
        }
    }

    fn clear_persistence_best_effort(&self) {
        let path = self.persistence_path();
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    fn start_notifications_loop(&self, client: Client) {
        let mut notifications = client.notifications();
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            loop {
                match notifications.recv().await {
                    Ok(RelayPoolNotification::Event { event, .. }) => {
                        let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::RelayEvent(
                            (*event).clone(),
                        ))));
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    fn schedule_session_connect(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        let client = logged_in.client.clone();
        let relay_urls = logged_in.relay_urls.clone();
        self.runtime.spawn(async move {
            ensure_session_relays_configured(&client, &relay_urls).await;
            client.connect().await;
        });
    }

    fn publish_local_identity_artifacts(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        if logged_in.authorization_state == LocalAuthorizationState::Revoked {
            return;
        }

        let snapshot = logged_in.session_manager.snapshot();
        let local_roster = snapshot
            .users
            .iter()
            .find(|user| user.owner_pubkey == logged_in.owner_pubkey)
            .and_then(|user| user.roster.clone());
        let local_invite = snapshot.local_invite.clone();
        let owner_keys = logged_in.owner_keys.clone();
        let device_keys = logged_in.device_keys.clone();
        let owner_pubkey = logged_in.owner_pubkey;
        let client = logged_in.client.clone();
        let relay_urls = logged_in.relay_urls.clone();
        let tx = self.core_sender.clone();

        self.runtime.spawn(async move {
            if let (Some(keys), Some(roster)) = (owner_keys, local_roster) {
                let roster_event = match codec::roster_unsigned_event(owner_pubkey, &roster)
                    .and_then(|unsigned| unsigned.sign_with_keys(&keys).map_err(Into::into))
                {
                    Ok(event) => Some(event),
                    Err(error) => {
                        let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                            error.to_string(),
                        ))));
                        None
                    }
                };
                if let Some(roster_event) = roster_event {
                    if let Err(error) =
                        publish_event_with_retry(&client, &relay_urls, roster_event, "roster").await
                    {
                        let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                            format!("Roster publish failed: {error}"),
                        ))));
                    }
                }
            }

            if let Some(invite) = local_invite {
                let invite_event = match codec::invite_unsigned_event(&invite)
                    .and_then(|unsigned| unsigned.sign_with_keys(&device_keys).map_err(Into::into))
                {
                    Ok(event) => Some(event),
                    Err(error) => {
                        let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                            error.to_string(),
                        ))));
                        None
                    }
                };
                if let Some(invite_event) = invite_event {
                    if let Err(error) =
                        publish_event_with_retry(&client, &relay_urls, invite_event, "invite").await
                    {
                        let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                            format!("Invite publish failed: {error}"),
                        ))));
                    }
                }
            }

            let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::SyncComplete)));
        });
    }

    fn publish_roster_update(&self, roster: DeviceRoster) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        let Some(owner_keys) = logged_in.owner_keys.clone() else {
            return;
        };
        let owner_pubkey = logged_in.owner_pubkey;
        let client = logged_in.client.clone();
        let relay_urls = logged_in.relay_urls.clone();
        let tx = self.core_sender.clone();

        self.runtime.spawn(async move {
            match codec::roster_unsigned_event(owner_pubkey, &roster)
                .and_then(|unsigned| unsigned.sign_with_keys(&owner_keys).map_err(Into::into))
            {
                Ok(event) => {
                    if let Err(error) =
                        publish_event_with_retry(&client, &relay_urls, event, "roster").await
                    {
                        let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                            format!("Roster publish failed: {error}"),
                        ))));
                    }
                }
                Err(error) => {
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                        error.to_string(),
                    ))));
                }
            }

            let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::SyncComplete)));
        });
    }

    fn republish_local_identity_artifacts(&self) {
        self.publish_local_identity_artifacts();
    }

    fn request_protocol_subscription_refresh(&mut self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            self.protocol_subscription_runtime = ProtocolSubscriptionRuntime::default();
            return;
        };

        if self.protocol_subscription_runtime.refresh_in_flight {
            self.protocol_subscription_runtime.refresh_dirty = true;
            return;
        }

        let plan = self.compute_protocol_subscription_plan();
        if self.protocol_subscription_runtime.current_plan == plan {
            return;
        }

        let client = logged_in.client.clone();
        let subscription_id = SubscriptionId::new(PROTOCOL_SUBSCRIPTION_ID);
        self.protocol_subscription_runtime.refresh_in_flight = true;
        self.protocol_subscription_runtime.refresh_dirty = false;
        self.protocol_subscription_runtime.refresh_token =
            self.protocol_subscription_runtime.refresh_token.wrapping_add(1);
        let token = self.protocol_subscription_runtime.refresh_token;
        self.protocol_subscription_runtime.applying_plan = plan.clone();
        let had_previous = self.protocol_subscription_runtime.current_plan.is_some();
        let filters = plan
            .as_ref()
            .map(build_protocol_filters)
            .unwrap_or_default();
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            let mut applied = true;
            if had_previous {
                let _ = client.unsubscribe(subscription_id.clone()).await;
            }
            if !filters.is_empty() {
                applied = client
                    .subscribe_with_id(subscription_id, filters, None)
                    .await
                    .is_ok();
            }
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::ProtocolSubscriptionRefreshCompleted {
                    token,
                    applied,
                    plan,
                },
            )));
        });
    }

    fn compute_protocol_subscription_plan(&self) -> Option<ProtocolSubscriptionPlan> {
        let roster_authors = sorted_hexes(self.known_roster_owner_hexes());
        let invite_authors = sorted_hexes(self.known_invite_author_hexes());
        let message_authors = sorted_hexes(self.known_message_author_hexes());
        let invite_response_recipient = self
            .logged_in
            .as_ref()
            .and_then(|logged_in| logged_in.session_manager.snapshot().local_invite)
            .map(|invite| invite.inviter_ephemeral_public_key.to_string());

        if roster_authors.is_empty()
            && invite_authors.is_empty()
            && message_authors.is_empty()
            && invite_response_recipient.is_none()
        {
            return None;
        }

        Some(ProtocolSubscriptionPlan {
            roster_authors,
            invite_authors,
            invite_response_recipient,
            message_authors,
        })
    }

    fn known_roster_owner_hexes(&self) -> HashSet<String> {
        let mut owners = self.protocol_owner_hexes();
        if let Some(logged_in) = self.logged_in.as_ref() {
            owners.insert(logged_in.owner_pubkey.to_string());
        }
        owners
    }

    fn known_invite_author_hexes(&self) -> HashSet<String> {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return HashSet::new();
        };

        let tracked_owners = self.protocol_owner_hexes();
        let local_device_hex = local_device_from_keys(&logged_in.device_keys).to_string();
        let mut authors = HashSet::new();

        for user in logged_in.session_manager.snapshot().users {
            let owner_hex = user.owner_pubkey.to_string();
            let should_include = owner_hex == logged_in.owner_pubkey.to_string()
                || tracked_owners.contains(&owner_hex);
            if !should_include {
                continue;
            }
            if let Some(roster) = user.roster {
                for device in roster.devices() {
                    let device_hex = device.device_pubkey.to_string();
                    if owner_hex == logged_in.owner_pubkey.to_string() && device_hex == local_device_hex {
                        continue;
                    }
                    authors.insert(device_hex);
                }
            }
        }

        authors
    }

    fn known_message_author_hexes(&self) -> HashSet<String> {
        let mut authors = HashSet::new();
        if let Some(logged_in) = self.logged_in.as_ref() {
            let selected_owners = self.protocol_owner_hexes();
            let local_owner_hex = logged_in.owner_pubkey.to_string();
            for user in logged_in
                .session_manager
                .snapshot()
                .users
                .into_iter()
                .filter(|user| {
                    let owner_hex = user.owner_pubkey.to_string();
                    owner_hex == local_owner_hex || selected_owners.contains(&owner_hex)
                })
            {
                for device in user.devices {
                    if let Some(session) = device.active_session.as_ref() {
                        collect_expected_senders(session, &mut authors);
                    }
                    for session in &device.inactive_sessions {
                        collect_expected_senders(session, &mut authors);
                    }
                }
            }
        }
        authors
    }

    fn sync_active_chat_from_router(&mut self) {
        match self.screen_stack.last() {
            Some(Screen::Chat { chat_id }) => {
                self.active_chat_id = Some(chat_id.clone());
                if let Some(thread) = self.threads.get_mut(chat_id) {
                    thread.unread_count = 0;
                }
            }
            _ => {
                self.active_chat_id = None;
            }
        }
    }

    fn has_seen_event(&self, event_id: &str) -> bool {
        self.seen_event_ids.contains(event_id)
    }

    fn remember_event(&mut self, event_id: String) {
        if !self.seen_event_ids.insert(event_id.clone()) {
            return;
        }

        self.seen_event_order.push_back(event_id);
        while self.seen_event_order.len() > MAX_SEEN_EVENT_IDS {
            if let Some(expired) = self.seen_event_order.pop_front() {
                self.seen_event_ids.remove(&expired);
            }
        }
    }
}

fn configured_relays() -> Vec<String> {
    match std::env::var("NDR_DEMO_RELAYS") {
        Ok(value) => {
            let custom: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if custom.is_empty() {
                DEFAULT_RELAYS
                    .iter()
                    .map(|relay| (*relay).to_string())
                    .collect()
            } else {
                custom
            }
        }
        Err(_) => DEFAULT_RELAYS
            .iter()
            .map(|relay| (*relay).to_string())
            .collect(),
    }
}

fn configured_relay_urls() -> Vec<RelayUrl> {
    let parsed: Vec<RelayUrl> = configured_relays()
        .into_iter()
        .filter_map(|relay| RelayUrl::parse(relay).ok())
        .collect();
    if parsed.is_empty() {
        DEFAULT_RELAYS
            .iter()
            .filter_map(|relay| RelayUrl::parse(*relay).ok())
            .collect()
    } else {
        parsed
    }
}

async fn ensure_session_relays_configured(client: &Client, relay_urls: &[RelayUrl]) {
    for relay in relay_urls {
        let _ = client.add_relay(relay.clone()).await;
    }
}

fn sorted_hexes(values: HashSet<String>) -> Vec<String> {
    let mut sorted = values.into_iter().collect::<Vec<_>>();
    sorted.sort();
    sorted.dedup();
    sorted
}

fn build_protocol_filters(plan: &ProtocolSubscriptionPlan) -> Vec<Filter> {
    let mut filters = Vec::new();

    let roster_authors = plan
        .roster_authors
        .iter()
        .filter_map(|hex| PublicKey::parse(hex).ok())
        .collect::<Vec<_>>();
    if !roster_authors.is_empty() {
        filters.push(
            Filter::new()
                .kind(Kind::from(codec::ROSTER_EVENT_KIND as u16))
                .authors(roster_authors),
        );
    }

    let invite_authors = plan
        .invite_authors
        .iter()
        .filter_map(|hex| PublicKey::parse(hex).ok())
        .collect::<Vec<_>>();
    if !invite_authors.is_empty() {
        filters.push(
            Filter::new()
                .kind(Kind::from(codec::INVITE_EVENT_KIND as u16))
                .authors(invite_authors),
        );
    }

    if let Some(recipient_hex) = plan.invite_response_recipient.as_ref() {
        if let Ok(recipient) = PublicKey::parse(recipient_hex) {
            filters.push(
                Filter::new()
                    .kind(Kind::from(codec::INVITE_RESPONSE_KIND as u16))
                    .pubkey(recipient),
            );
        }
    }

    let message_authors = plan
        .message_authors
        .iter()
        .filter_map(|hex| PublicKey::parse(hex).ok())
        .collect::<Vec<_>>();
    if !message_authors.is_empty() {
        filters.push(
            Filter::new()
                .kind(Kind::from(codec::MESSAGE_EVENT_KIND as u16))
                .authors(message_authors),
        );
    }

    filters
}

fn resolve_message_sender_owner(
    session_manager: &SessionManager,
    envelope: &MessageEnvelope,
    now: UnixSeconds,
) -> Option<OwnerPubkey> {
    let owners: Vec<OwnerPubkey> = session_manager
        .snapshot()
        .users
        .into_iter()
        .map(|user| user.owner_pubkey)
        .collect();

    for owner in owners {
        let mut candidate = session_manager.clone();
        let mut rng = OsRng;
        let mut ctx = ProtocolContext::new(now, &mut rng);
        match candidate.receive(&mut ctx, owner, envelope) {
            Ok(Some(_)) => return Some(owner),
            Ok(None) => {}
            Err(_) => {}
        }
    }

    None
}

fn encode_app_message_payload(chat_id: &str, body: &str) -> anyhow::Result<Vec<u8>> {
    let (normalized_chat_id, _) = parse_peer_input(chat_id)?;
    Ok(serde_json::to_vec(&AppMessagePayload {
        version: APP_MESSAGE_PAYLOAD_VERSION,
        chat_id: normalized_chat_id,
        body: body.to_string(),
    })?)
}

fn decode_app_message_payload(payload: &[u8]) -> Option<AppMessagePayload> {
    let decoded = serde_json::from_slice::<AppMessagePayload>(payload).ok()?;
    if decoded.version != APP_MESSAGE_PAYLOAD_VERSION {
        return None;
    }
    Some(decoded)
}

fn route_received_message(
    local_owner: OwnerPubkey,
    sender_owner: OwnerPubkey,
    payload: &[u8],
) -> RoutedChatMessage {
    if let Some(decoded) = decode_app_message_payload(payload) {
        if sender_owner == local_owner {
            if let Ok((chat_id, _)) = parse_peer_input(&decoded.chat_id) {
                if chat_id != local_owner.to_string() {
                    return RoutedChatMessage {
                        chat_id,
                        body: decoded.body,
                        is_outgoing: true,
                    };
                }
            }
        }

        return RoutedChatMessage {
            chat_id: sender_owner.to_string(),
            body: decoded.body,
            is_outgoing: false,
        };
    }

    RoutedChatMessage {
        chat_id: sender_owner.to_string(),
        body: String::from_utf8_lossy(payload).into_owned(),
        is_outgoing: false,
    }
}

fn collect_expected_senders(session: &SessionState, out: &mut HashSet<String>) {
    if let Some(current) = session.their_current_nostr_public_key {
        out.insert(current.to_string());
    }
    if let Some(next) = session.their_next_nostr_public_key {
        out.insert(next.to_string());
    }
    out.extend(session.skipped_keys.keys().map(ToString::to_string));
}

fn pending_reason_from_prepared(
    prepared: &nostr_double_ratchet::PreparedSend,
) -> Option<PendingSendReason> {
    if prepared
        .relay_gaps
        .iter()
        .any(|gap| matches!(gap, RelayGap::MissingRoster { .. }))
    {
        return Some(PendingSendReason::MissingRoster);
    }
    if prepared
        .relay_gaps
        .iter()
        .any(|gap| matches!(gap, RelayGap::MissingDeviceInvite { .. }))
    {
        return Some(PendingSendReason::MissingDeviceInvite);
    }
    if prepared.deliveries.is_empty() && prepared.invite_responses.is_empty() {
        return Some(PendingSendReason::MissingDeviceInvite);
    }
    None
}

fn build_prepared_publish_batch(
    prepared: &nostr_double_ratchet::PreparedSend,
) -> anyhow::Result<Option<PreparedPublishBatch>> {
    let invite_events = prepared
        .invite_responses
        .iter()
        .map(codec::invite_response_event)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let message_events = prepared
        .deliveries
        .iter()
        .map(|delivery| codec::message_event(&delivery.envelope))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if message_events.is_empty() {
        return Ok(None);
    }

    Ok(Some(PreparedPublishBatch {
        invite_events,
        message_events,
    }))
}

fn publish_mode_for_batch(batch: &PreparedPublishBatch) -> OutboundPublishMode {
    if batch.invite_events.is_empty() {
        OutboundPublishMode::OrdinaryFirstAck
    } else {
        OutboundPublishMode::FirstContactStaged
    }
}

fn migrate_publish_mode(
    current: OutboundPublishMode,
    batch: Option<&PreparedPublishBatch>,
) -> OutboundPublishMode {
    match current {
        OutboundPublishMode::WaitForPeer => batch
            .map(publish_mode_for_batch)
            .unwrap_or(OutboundPublishMode::WaitForPeer),
        other => other,
    }
}

fn pending_reason_for_publish_mode(mode: &OutboundPublishMode) -> PendingSendReason {
    match mode {
        OutboundPublishMode::FirstContactStaged => PendingSendReason::PublishingFirstContact,
        OutboundPublishMode::OrdinaryFirstAck => PendingSendReason::PublishRetry,
        OutboundPublishMode::WaitForPeer => PendingSendReason::MissingDeviceInvite,
    }
}

fn retry_delay_for_publish_mode(mode: &OutboundPublishMode) -> u64 {
    match mode {
        OutboundPublishMode::FirstContactStaged => FIRST_CONTACT_RETRY_DELAY_SECS,
        OutboundPublishMode::OrdinaryFirstAck | OutboundPublishMode::WaitForPeer => {
            PENDING_RETRY_DELAY_SECS
        }
    }
}

fn retry_deadline_for_publish_mode(now_secs: u64, mode: &OutboundPublishMode) -> u64 {
    now_secs.saturating_add(retry_delay_for_publish_mode(mode))
}

pub(crate) fn parse_peer_input(input: &str) -> anyhow::Result<(String, PublicKey)> {
    let mut normalized = input.trim().to_ascii_lowercase();
    if let Some(stripped) = normalized.strip_prefix("nostr:") {
        normalized = stripped.to_string();
    }
    let pubkey = PublicKey::parse(&normalized)?;
    Ok((pubkey.to_hex(), pubkey))
}

pub(crate) fn normalize_peer_input_for_display(input: &str) -> String {
    let mut normalized = input.trim().to_ascii_lowercase();
    if let Some(stripped) = normalized.strip_prefix("nostr:") {
        normalized = stripped.to_string();
    }

    match PublicKey::parse(&normalized) {
        Ok(pubkey) if normalized.starts_with("npub1") => {
            pubkey.to_bech32().unwrap_or_else(|_| normalized.clone())
        }
        Ok(pubkey) => pubkey.to_hex(),
        Err(_) => normalized,
    }
}

fn parse_owner_input(input: &str) -> anyhow::Result<OwnerPubkey> {
    let (_, pubkey) = parse_peer_input(input)?;
    Ok(OwnerPubkey::from_bytes(pubkey.to_bytes()))
}

fn parse_device_input(input: &str) -> anyhow::Result<DevicePubkey> {
    let (_, pubkey) = parse_peer_input(input)?;
    Ok(DevicePubkey::from_bytes(pubkey.to_bytes()))
}

#[cfg(test)]
fn local_owner_from_keys(keys: &Keys) -> OwnerPubkey {
    OwnerPubkey::from_bytes(keys.public_key().to_bytes())
}

fn local_device_from_keys(keys: &Keys) -> DevicePubkey {
    DevicePubkey::from_bytes(keys.public_key().to_bytes())
}

fn owner_npub(peer_hex: &str) -> Option<String> {
    PublicKey::parse(peer_hex).ok()?.to_bech32().ok()
}

fn owner_npub_from_owner(owner_pubkey: OwnerPubkey) -> Option<String> {
    PublicKey::parse(owner_pubkey.to_string()).ok()?.to_bech32().ok()
}

fn device_npub(device_hex: &str) -> Option<String> {
    PublicKey::parse(device_hex).ok()?.to_bech32().ok()
}

fn local_roster_from_session_manager(session_manager: &SessionManager) -> Option<DeviceRoster> {
    let snapshot = session_manager.snapshot();
    let owner = snapshot.local_owner_pubkey;
    snapshot
        .users
        .into_iter()
        .find(|user| user.owner_pubkey == owner)
        .and_then(|user| user.roster)
}

fn public_authorization_state(
    state: LocalAuthorizationState,
) -> DeviceAuthorizationState {
    match state {
        LocalAuthorizationState::Authorized => DeviceAuthorizationState::Authorized,
        LocalAuthorizationState::AwaitingApproval => DeviceAuthorizationState::AwaitingApproval,
        LocalAuthorizationState::Revoked => DeviceAuthorizationState::Revoked,
    }
}

fn derive_local_authorization_state(
    has_owner_signing_authority: bool,
    owner_pubkey: OwnerPubkey,
    local_device_pubkey: DevicePubkey,
    session_manager: &SessionManager,
    previous_state: Option<LocalAuthorizationState>,
) -> LocalAuthorizationState {
    let local_roster = session_manager
        .snapshot()
        .users
        .into_iter()
        .find(|user| user.owner_pubkey == owner_pubkey)
        .and_then(|user| user.roster);
    match local_roster {
        Some(roster) => {
            if roster.get_device(&local_device_pubkey).is_some() {
                LocalAuthorizationState::Authorized
            } else if has_owner_signing_authority {
                LocalAuthorizationState::Authorized
            } else if matches!(
                previous_state,
                Some(LocalAuthorizationState::Authorized)
                    | Some(LocalAuthorizationState::Revoked)
            ) {
                LocalAuthorizationState::Revoked
            } else {
                LocalAuthorizationState::AwaitingApproval
            }
        }
        None if has_owner_signing_authority => LocalAuthorizationState::Authorized,
        None => LocalAuthorizationState::AwaitingApproval,
    }
}

fn chat_unavailable_message(logged_in: Option<&LoggedInState>) -> &'static str {
    match logged_in.map(|logged_in| logged_in.authorization_state) {
        Some(LocalAuthorizationState::AwaitingApproval) => {
            "This device is still waiting for approval."
        }
        Some(LocalAuthorizationState::Revoked) => {
            "This device has been removed from the roster. Log out to continue."
        }
        _ => "Create or restore an account first.",
    }
}

fn unix_now() -> UnixSeconds {
    UnixSeconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

async fn publish_event_with_retry(
    client: &Client,
    relay_urls: &[RelayUrl],
    event: Event,
    label: &str,
) -> anyhow::Result<()> {
    let mut last_error = "no relays configured".to_string();

    for attempt in 0..5 {
        client.connect().await;
        match publish_event_once(client, relay_urls, &event).await {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error.to_string(),
        }

        if attempt < 4 {
            sleep(Duration::from_millis(750 * (attempt + 1) as u64)).await;
        }
    }

    Err(anyhow::anyhow!("{label}: {last_error}"))
}

async fn publish_events_with_retry(
    client: &Client,
    relay_urls: &[RelayUrl],
    events: Vec<Event>,
    label: &str,
) -> anyhow::Result<()> {
    for event in events {
        publish_event_with_retry(client, relay_urls, event, label).await?;
    }
    Ok(())
}

async fn publish_events_first_ack(
    client: &Client,
    relay_urls: &[RelayUrl],
    events: &[Event],
    label: &str,
) -> anyhow::Result<()> {
    for event in events {
        publish_event_first_ack(client, relay_urls, event, label).await?;
    }
    Ok(())
}

async fn publish_event_first_ack(
    client: &Client,
    relay_urls: &[RelayUrl],
    event: &Event,
    label: &str,
) -> anyhow::Result<()> {
    if relay_urls.is_empty() {
        return Err(anyhow::anyhow!("{label}: no relays configured"));
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<(), String>>(relay_urls.len().max(1));

    for relay_url in relay_urls.iter().cloned() {
        let client = client.clone();
        let event = event.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = match client.send_event_to([relay_url.clone()], event).await {
                Ok(output) if !output.success.is_empty() => Ok(()),
                Ok(output) => Err(output
                    .failed
                    .values()
                    .flatten()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "no relay accepted event".to_string())),
                Err(error) => Err(error.to_string()),
            };
            let _ = tx.send(result).await;
        });
    }
    drop(tx);

    let mut first_error = None;
    while let Some(result) = rx.recv().await {
        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "{label}: {}",
        first_error.unwrap_or_else(|| "publish failed".to_string())
    ))
}

async fn publish_event_once(
    client: &Client,
    relay_urls: &[RelayUrl],
    event: &Event,
) -> anyhow::Result<()> {
    if relay_urls.is_empty() {
        return Err(anyhow::anyhow!("no relays configured"));
    }

    let output = client
        .send_event_to(relay_urls.to_vec(), event.clone())
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if output.success.is_empty() {
        let reasons = output
            .failed
            .values()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        Err(anyhow::anyhow!(
            if reasons.is_empty() {
                "no relay accepted event".to_string()
            } else {
                reasons.join("; ")
            }
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr_double_ratchet::AuthorizedDevice;
    use crate::FfiApp;
    use futures_util::{SinkExt, StreamExt};
    use nostr_sdk::prelude::SecretKey;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::mpsc as std_mpsc;
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration as StdDuration, Instant};
    use tempfile::TempDir;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    fn relay_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct RelayEnvGuard {
        previous: Option<String>,
    }

    impl RelayEnvGuard {
        fn local_only() -> Self {
            Self::custom("ws://127.0.0.1:4848")
        }

        fn custom(value: &str) -> Self {
            let previous = std::env::var("NDR_DEMO_RELAYS").ok();
            std::env::set_var("NDR_DEMO_RELAYS", value);
            Self { previous }
        }
    }

    impl Drop for RelayEnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var("NDR_DEMO_RELAYS", previous);
            } else {
                std::env::remove_var("NDR_DEMO_RELAYS");
            }
        }
    }

    #[derive(Default)]
    struct RelayState {
        events_by_id: BTreeMap<String, Value>,
        subscriptions: HashMap<usize, HashMap<String, Vec<Value>>>,
        clients: HashMap<usize, mpsc::UnboundedSender<Message>>,
    }

    enum RelayControl {
        ReplayStored,
        Shutdown,
    }

    struct TestRelay {
        control_tx: mpsc::UnboundedSender<RelayControl>,
        join: Option<thread::JoinHandle<()>>,
    }

    impl TestRelay {
        fn start() -> Self {
            let (control_tx, mut control_rx) = mpsc::unbounded_channel();
            let (ready_tx, ready_rx) = std_mpsc::channel();

            let join = thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("relay runtime");

                runtime.block_on(async move {
                    let listener = TcpListener::bind("127.0.0.1:4848")
                        .await
                        .expect("bind relay listener");
                    let state = Arc::new(Mutex::new(RelayState::default()));
                    let next_client_id = Arc::new(std::sync::atomic::AtomicUsize::new(1));
                    ready_tx.send(()).expect("signal relay ready");

                    loop {
                        tokio::select! {
                            Some(control) = control_rx.recv() => {
                                match control {
                                    RelayControl::ReplayStored => replay_stored_events(&state),
                                    RelayControl::Shutdown => break,
                                }
                            }
                            accept_result = listener.accept() => {
                                let (stream, _) = accept_result.expect("accept relay client");
                                let websocket = accept_async(stream).await.expect("accept websocket");
                                let state = state.clone();
                                let client_id = next_client_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                tokio::spawn(async move {
                                    handle_connection(client_id, websocket, state).await;
                                });
                            }
                        }
                    }
                });
            });

            ready_rx
                .recv_timeout(StdDuration::from_secs(5))
                .expect("relay ready");

            Self {
                control_tx,
                join: Some(join),
            }
        }

        fn replay_stored(&self) {
            let _ = self.control_tx.send(RelayControl::ReplayStored);
        }
    }

    impl Drop for TestRelay {
        fn drop(&mut self) {
            let _ = self.control_tx.send(RelayControl::Shutdown);
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    async fn handle_connection(
        client_id: usize,
        websocket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        state: Arc<Mutex<RelayState>>,
    ) {
        let (mut sink, mut stream) = websocket.split();
        let (client_tx, mut client_rx) = mpsc::unbounded_channel::<Message>();

        {
            let mut relay = state.lock().expect("relay state lock");
            relay.clients.insert(client_id, client_tx);
        }

        let writer = tokio::spawn(async move {
            while let Some(message) = client_rx.recv().await {
                if sink.send(message).await.is_err() {
                    break;
                }
            }
        });

        while let Some(message) = stream.next().await {
            let Ok(message) = message else {
                break;
            };
            match message {
                Message::Text(text) => handle_client_message(client_id, &text, &state),
                Message::Ping(payload) => {
                    let sender = {
                        let relay = state.lock().expect("relay state lock");
                        relay.clients.get(&client_id).cloned()
                    };
                    if let Some(sender) = sender {
                        let _ = sender.send(Message::Pong(payload));
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        {
            let mut relay = state.lock().expect("relay state lock");
            relay.clients.remove(&client_id);
            relay.subscriptions.remove(&client_id);
        }

        writer.abort();
    }

    fn handle_client_message(client_id: usize, raw_message: &str, state: &Arc<Mutex<RelayState>>) {
        let Ok(message) = serde_json::from_str::<Value>(raw_message) else {
            return;
        };
        let Some(parts) = message.as_array() else {
            return;
        };
        let Some(kind) = parts.first().and_then(Value::as_str) else {
            return;
        };

        match kind {
            "REQ" if parts.len() >= 2 => {
                let Some(subscription_id) = parts[1].as_str() else {
                    return;
                };
                let filters: Vec<Value> = parts
                    .iter()
                    .skip(2)
                    .filter(|value| value.is_object())
                    .cloned()
                    .collect();
                let (sender, events) = {
                    let mut relay = state.lock().expect("relay state lock");
                    relay
                        .subscriptions
                        .entry(client_id)
                        .or_default()
                        .insert(subscription_id.to_string(), filters.clone());
                    (
                        relay.clients.get(&client_id).cloned(),
                        relay.events_by_id.values().cloned().collect::<Vec<_>>(),
                    )
                };

                if let Some(sender) = sender {
                    for event in events {
                        if matches_any_filter(&event, &filters) {
                            let payload = Message::Text(
                                json!(["EVENT", subscription_id, event]).to_string().into(),
                            );
                            let _ = sender.send(payload);
                        }
                    }
                    let _ = sender.send(Message::Text(
                        json!(["EOSE", subscription_id]).to_string().into(),
                    ));
                }
            }
            "CLOSE" if parts.len() >= 2 => {
                let Some(subscription_id) = parts[1].as_str() else {
                    return;
                };
                let mut relay = state.lock().expect("relay state lock");
                if let Some(subscriptions) = relay.subscriptions.get_mut(&client_id) {
                    subscriptions.remove(subscription_id);
                }
            }
            "EVENT" if parts.len() >= 2 && parts[1].is_object() => {
                let event = parts[1].clone();
                let Some(event_id) = event.get("id").and_then(Value::as_str) else {
                    return;
                };
                let (sender, deliveries) = {
                    let mut relay = state.lock().expect("relay state lock");
                    relay
                        .events_by_id
                        .insert(event_id.to_string(), event.clone());
                    let sender = relay.clients.get(&client_id).cloned();
                    let deliveries = matching_deliveries(&relay, &event);
                    (sender, deliveries)
                };
                if let Some(sender) = sender {
                    let _ = sender.send(Message::Text(
                        json!(["OK", event_id, true, ""]).to_string().into(),
                    ));
                }

                for (target, payload) in deliveries {
                    let _ = target.send(payload);
                }
            }
            _ => {}
        }
    }

    fn replay_stored_events(state: &Arc<Mutex<RelayState>>) {
        let deliveries = {
            let relay = state.lock().expect("relay state lock");
            relay
                .events_by_id
                .values()
                .flat_map(|event| matching_deliveries(&relay, event))
                .collect::<Vec<_>>()
        };

        for (target, payload) in deliveries {
            let _ = target.send(payload);
        }
    }

    fn matching_deliveries(
        relay: &RelayState,
        event: &Value,
    ) -> Vec<(mpsc::UnboundedSender<Message>, Message)> {
        let mut deliveries = Vec::new();
        for (client_id, subscriptions) in &relay.subscriptions {
            let Some(target) = relay.clients.get(client_id).cloned() else {
                continue;
            };
            for (subscription_id, filters) in subscriptions {
                if matches_any_filter(event, filters) {
                    deliveries.push((
                        target.clone(),
                        Message::Text(json!(["EVENT", subscription_id, event]).to_string().into()),
                    ));
                }
            }
        }
        deliveries
    }

    fn matches_any_filter(event: &Value, filters: &[Value]) -> bool {
        if filters.is_empty() {
            return true;
        }

        filters.iter().any(|filter| matches_filter(event, filter))
    }

    fn matches_filter(event: &Value, filter: &Value) -> bool {
        let Some(filter_object) = filter.as_object() else {
            return false;
        };

        if let Some(authors) = filter_object.get("authors").and_then(Value::as_array) {
            let Some(pubkey) = event.get("pubkey").and_then(Value::as_str) else {
                return false;
            };
            if !authors
                .iter()
                .filter_map(Value::as_str)
                .any(|author| author == pubkey)
            {
                return false;
            }
        }

        if let Some(kinds) = filter_object.get("kinds").and_then(Value::as_array) {
            let Some(kind) = event.get("kind").and_then(Value::as_u64) else {
                return false;
            };
            if !kinds
                .iter()
                .filter_map(Value::as_u64)
                .any(|value| value == kind)
            {
                return false;
            }
        }

        if let Some(since) = filter_object.get("since").and_then(Value::as_u64) {
            let Some(created_at) = event.get("created_at").and_then(Value::as_u64) else {
                return false;
            };
            if created_at < since {
                return false;
            }
        }

        if let Some(until) = filter_object.get("until").and_then(Value::as_u64) {
            let Some(created_at) = event.get("created_at").and_then(Value::as_u64) else {
                return false;
            };
            if created_at > until {
                return false;
            }
        }

        for (key, value) in filter_object {
            let Some(tag_name) = key.strip_prefix('#') else {
                continue;
            };

            let Some(expected_values) = value.as_array() else {
                return false;
            };
            if expected_values.is_empty() {
                continue;
            }

            let Some(tags) = event.get("tags").and_then(Value::as_array) else {
                return false;
            };
            let matched = tags.iter().any(|tag| {
                let Some(tag_values) = tag.as_array() else {
                    return false;
                };
                if tag_values.first().and_then(Value::as_str) != Some(tag_name) {
                    return false;
                }
                tag_values
                    .iter()
                    .skip(1)
                    .filter_map(Value::as_str)
                    .any(|tag_value| {
                        expected_values
                            .iter()
                            .filter_map(Value::as_str)
                            .any(|expected| expected == tag_value)
                    })
            });
            if !matched {
                return false;
            }
        }

        true
    }

    fn nsec_for_fill(secret_fill: u8) -> String {
        let keys = Keys::new(SecretKey::from_slice(&[secret_fill; 32]).expect("secret key"));
        keys.secret_key().to_bech32().expect("nsec")
    }

    fn npub_for_fill(secret_fill: u8) -> String {
        let keys = Keys::new(SecretKey::from_slice(&[secret_fill; 32]).expect("secret key"));
        keys.public_key().to_bech32().expect("npub")
    }

    fn wait_for_state(
        app: &Arc<FfiApp>,
        label: &str,
        predicate: impl Fn(&AppState) -> bool,
    ) -> AppState {
        let deadline = Instant::now() + StdDuration::from_secs(15);
        let mut last = app.state();

        while Instant::now() < deadline {
            last = app.state();
            if predicate(&last) {
                return last;
            }
            thread::sleep(StdDuration::from_millis(100));
        }

        panic!("timed out waiting for {label}: last state = {:?}", last);
    }

    fn app_with_dir(data_dir: &Path, secret_fill: u8) -> Arc<FfiApp> {
        let app = FfiApp::new(
            data_dir.to_string_lossy().into_owned(),
            String::new(),
            "test".to_string(),
        );
        app.dispatch(AppAction::RestoreSession {
            owner_nsec: nsec_for_fill(secret_fill),
        });
        wait_for_state(&app, "account restore", |state| {
            state.account.is_some() && !state.busy.restoring_session
        });
        app
    }

    fn app(secret_fill: u8) -> (TempDir, Arc<FfiApp>) {
        let temp_dir = TempDir::new().expect("temp dir");
        let app = app_with_dir(temp_dir.path(), secret_fill);
        (temp_dir, app)
    }

    fn test_core(data_dir: &Path) -> AppCore {
        let (update_tx, _update_rx) = flume::unbounded();
        let (core_tx, _core_rx) = flume::unbounded();
        let shared_state = Arc::new(RwLock::new(AppState::empty()));
        AppCore::new(
            update_tx,
            core_tx,
            data_dir.to_string_lossy().into_owned(),
            shared_state,
        )
    }

    fn persisted_state(data_dir: &Path) -> PersistedState {
        let bytes =
            fs::read(data_dir.join("ndr_demo_core_state.json")).expect("read persisted state");
        serde_json::from_slice(&bytes).expect("parse persisted state")
    }

    fn wait_for_persisted_state(
        data_dir: &Path,
        label: &str,
        predicate: impl Fn(&PersistedState) -> bool,
    ) -> PersistedState {
        let deadline = Instant::now() + StdDuration::from_secs(15);
        while Instant::now() < deadline {
            if data_dir.join("ndr_demo_core_state.json").exists() {
                let state = persisted_state(data_dir);
                if predicate(&state) {
                    return state;
                }
            }
            thread::sleep(StdDuration::from_millis(100));
        }
        panic!("timed out waiting for persisted state: {label}");
    }

    fn pending_publish_event_ids(core: &AppCore, message_id: &str) -> (Vec<String>, Vec<String>) {
        let pending = core
            .pending_outbound
            .iter()
            .find(|pending| pending.message_id == message_id)
            .expect("pending outbound");
        let batch = pending
            .prepared_publish
            .as_ref()
            .expect("prepared publish batch");
        (
            batch
                .invite_events
                .iter()
                .map(|event| event.id.to_string())
                .collect(),
            batch
                .message_events
                .iter()
                .map(|event| event.id.to_string())
                .collect(),
        )
    }

    fn keys_for_fill(secret_fill: u8) -> Keys {
        Keys::new(SecretKey::from_slice(&[secret_fill; 32]).expect("secret key"))
    }

    fn device_keys_for_fill(secret_fill: u8) -> Keys {
        Keys::new(SecretKey::from_slice(&[secret_fill.wrapping_add(100); 32]).expect("secret key"))
    }

    fn start_primary_test_session(
        core: &mut AppCore,
        owner_fill: u8,
        allow_restore: bool,
        allow_protocol_restore: bool,
    ) -> anyhow::Result<()> {
        core.start_primary_session(
            keys_for_fill(owner_fill),
            device_keys_for_fill(owner_fill),
            allow_restore,
            allow_protocol_restore,
        )
    }

    fn established_session_manager_pair(
        alice_fill: u8,
        bob_fill: u8,
        base_secs: u64,
    ) -> (SessionManager, SessionManager, String) {
        let alice_owner_keys = keys_for_fill(alice_fill);
        let bob_owner_keys = keys_for_fill(bob_fill);
        let alice_device_keys = device_keys_for_fill(alice_fill);
        let bob_device_keys = device_keys_for_fill(bob_fill);
        let alice_owner = local_owner_from_keys(&alice_owner_keys);
        let bob_owner = local_owner_from_keys(&bob_owner_keys);
        let alice_device = local_device_from_keys(&alice_device_keys);
        let bob_device = local_device_from_keys(&bob_device_keys);
        let now = UnixSeconds(base_secs);

        let mut alice_manager =
            SessionManager::new(alice_owner, alice_device_keys.secret_key().to_secret_bytes());
        let mut bob_manager =
            SessionManager::new(bob_owner, bob_device_keys.secret_key().to_secret_bytes());

        let alice_roster = DeviceRoster::new(
            now,
            vec![AuthorizedDevice::new(alice_device, now)],
        );
        let bob_roster =
            DeviceRoster::new(now, vec![AuthorizedDevice::new(bob_device, now)]);
        alice_manager.apply_local_roster(alice_roster.clone());
        bob_manager.apply_local_roster(bob_roster.clone());
        alice_manager.observe_peer_roster(bob_owner, bob_roster.clone());
        bob_manager.observe_peer_roster(alice_owner, alice_roster);

        let bob_invite = {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(base_secs + 1), &mut rng);
            bob_manager
                .ensure_local_invite(&mut ctx)
                .expect("ensure bob invite")
                .clone()
        };
        alice_manager
            .observe_device_invite(bob_owner, bob_invite)
            .expect("observe bob invite");

        let prepared = {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(base_secs + 2), &mut rng);
            alice_manager
                .prepare_send(&mut ctx, bob_owner, b"bootstrap".to_vec())
                .expect("prepare bootstrap message")
        };
        assert_eq!(prepared.invite_responses.len(), 1);
        assert_eq!(prepared.deliveries.len(), 1);

        {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(base_secs + 3), &mut rng);
            bob_manager
                .observe_invite_response(&mut ctx, &prepared.invite_responses[0])
                .expect("observe invite response")
                .expect("processed invite response");
        }
        {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(base_secs + 4), &mut rng);
            bob_manager
                .receive(&mut ctx, alice_owner, &prepared.deliveries[0].envelope)
                .expect("receive bootstrap delivery")
                .expect("bootstrap message");
        }

        (alice_manager, bob_manager, bob_owner.to_string())
    }

    fn logged_in_core_with_manager(
        data_dir: &Path,
        secret_fill: u8,
        session_manager: SessionManager,
    ) -> AppCore {
        let owner_keys = keys_for_fill(secret_fill);
        let device_keys = device_keys_for_fill(secret_fill);
        let mut core = test_core(data_dir);
        core.state.account = Some(AccountSnapshot {
            public_key_hex: owner_keys.public_key().to_hex(),
            npub: owner_keys.public_key().to_bech32().expect("npub"),
            device_public_key_hex: device_keys.public_key().to_hex(),
            device_npub: device_keys.public_key().to_bech32().expect("device npub"),
            has_owner_signing_authority: true,
            authorization_state: DeviceAuthorizationState::Authorized,
        });
        core.logged_in = Some(LoggedInState {
            owner_pubkey: local_owner_from_keys(&owner_keys),
            owner_keys: Some(owner_keys),
            device_keys: device_keys.clone(),
            client: Client::new(device_keys),
            relay_urls: configured_relay_urls(),
            session_manager,
            authorization_state: LocalAuthorizationState::Authorized,
        });
        core.rebuild_state();
        core
    }

    fn publish_local_relay_event(keys: &Keys, event: Event) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("publish runtime");
        runtime.block_on(async {
            let client = Client::new(keys.clone());
            let relay_urls = vec![RelayUrl::parse("ws://127.0.0.1:4848").expect("relay url")];
            ensure_session_relays_configured(&client, &relay_urls).await;
            client.connect().await;
            publish_event_with_retry(&client, &relay_urls, event, "test publish")
                .await
                .expect("publish event");
            let _ = client.shutdown().await;
        });
    }

    fn fetch_local_relay_events(filters: Vec<Filter>) -> Vec<Event> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("fetch runtime");
        runtime.block_on(async move {
            let client = Client::new(keys_for_fill(200));
            client
                .add_relay("ws://127.0.0.1:4848")
                .await
                .expect("add relay");
            client.connect_with_timeout(Duration::from_secs(5)).await;
            let events = client
                .fetch_events(filters, Some(Duration::from_secs(5)))
                .await
                .expect("fetch events");
            let collected = events.into_iter().collect::<Vec<_>>();
            let _ = client.shutdown().await;
            collected
        })
    }

    #[test]
    fn relay_filter_matches_pubkey_tags_and_time_bounds() {
        let event = json!({
            "pubkey": "aaaaaaaa",
            "kind": codec::INVITE_RESPONSE_KIND,
            "created_at": 100,
            "tags": [["p", "deadbeef"], ["d", "double-ratchet/invites/test"]],
        });

        assert!(matches_filter(
            &event,
            &json!({
                "kinds": [codec::INVITE_RESPONSE_KIND],
                "#p": ["deadbeef"],
                "since": 90,
                "until": 110,
            }),
        ));
        assert!(!matches_filter(
            &event,
            &json!({
                "kinds": [codec::INVITE_RESPONSE_KIND],
                "#p": ["cafebabe"],
            }),
        ));
        assert!(!matches_filter(
            &event,
            &json!({
                "kinds": [codec::INVITE_RESPONSE_KIND],
                "#p": ["deadbeef"],
                "since": 101,
            }),
        ));
    }

    #[test]
    fn normalize_and_validate_peer_input_accepts_expected_forms() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let npub = npub_for_fill(11);
        let (hex, _) = parse_peer_input(&npub).expect("parse npub");

        assert_eq!(normalize_peer_input_for_display(&npub), npub);
        assert_eq!(
            normalize_peer_input_for_display(&format!("nostr:{npub}")),
            npub
        );
        assert_eq!(normalize_peer_input_for_display(&hex), hex);
        assert!(parse_peer_input(&format!("nostr:{hex}")).is_ok());
        assert!(parse_peer_input("not-a-key").is_err());
    }

    #[test]
    fn start_session_restores_persisted_threads_only_when_enabled() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let data_dir = TempDir::new().expect("temp dir");
        let chat_id = parse_peer_input(&npub_for_fill(22))
            .expect("peer chat id")
            .0;

        let mut seeded = test_core(data_dir.path());
        start_primary_test_session(&mut seeded, 21, false, false).expect("start session");
        seeded.remember_event("event-1".to_string());
        seeded.threads.insert(
            chat_id.clone(),
            ThreadRecord {
                chat_id: chat_id.clone(),
                unread_count: 2,
                updated_at_secs: 55,
                messages: vec![ChatMessageSnapshot {
                    id: "1".to_string(),
                    chat_id: chat_id.clone(),
                    author: "peer".to_string(),
                    body: "restored".to_string(),
                    is_outgoing: false,
                    created_at_secs: 55,
                    delivery: DeliveryState::Received,
                }],
            },
        );
        seeded.active_chat_id = Some(chat_id.clone());
        seeded.screen_stack = vec![Screen::Chat {
            chat_id: chat_id.clone(),
        }];
        seeded.rebuild_state();
        seeded.persist_best_effort();

        let mut restored = test_core(data_dir.path());
        start_primary_test_session(&mut restored, 21, true, true).expect("restore session");
        assert_eq!(restored.active_chat_id.as_deref(), Some(chat_id.as_str()));
        assert_eq!(restored.state.chat_list.len(), 1);
        assert_eq!(
            restored
                .state
                .current_chat
                .as_ref()
                .expect("current chat")
                .messages[0]
                .body,
            "restored"
        );
        assert!(restored.has_seen_event("event-1"));

        let mut fresh = test_core(data_dir.path());
        start_primary_test_session(&mut fresh, 21, false, false).expect("fresh session");
        assert!(fresh.state.chat_list.is_empty());
        assert!(fresh.active_chat_id.is_none());
        assert!(!fresh.has_seen_event("event-1"));
    }

    #[test]
    fn legacy_peer_hex_persistence_migrates_to_chat_ids() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let data_dir = TempDir::new().expect("temp dir");
        let peer_npub = npub_for_fill(32);
        let peer_hex = parse_peer_input(&peer_npub).expect("peer hex").0;

        let legacy = json!({
            "version": 1,
            "active_peer_hex": peer_hex,
            "next_message_id": 2,
            "session_manager": null,
            "threads": [{
                "peer_hex": peer_hex,
                "unread_count": 1,
                "messages": [{
                    "id": "1",
                    "peer_input": peer_hex,
                    "author": peer_npub,
                    "body": "legacy",
                    "is_outgoing": false,
                    "created_at_secs": 7,
                    "delivery": "Received"
                }]
            }]
        });
        fs::write(
            data_dir.path().join("ndr_demo_core_state.json"),
            serde_json::to_vec(&legacy).expect("legacy json"),
        )
        .expect("write legacy persistence");

        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 31, true, false).expect("restore legacy state");

        assert_eq!(core.active_chat_id.as_deref(), Some(peer_hex.as_str()));
        assert_eq!(core.state.chat_list[0].chat_id, peer_hex);
        assert_eq!(
            core.state
                .current_chat
                .as_ref()
                .expect("current chat")
                .messages[0]
                .chat_id,
            core.state.chat_list[0].chat_id
        );
    }

    #[test]
    fn update_screen_stack_opens_chat_and_clears_unread() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 40, false, false).expect("start session");
        let chat_id = parse_peer_input(&npub_for_fill(41))
            .expect("peer chat id")
            .0;
        core.threads.insert(
            chat_id.clone(),
            ThreadRecord {
                chat_id: chat_id.clone(),
                unread_count: 3,
                updated_at_secs: 100,
                messages: vec![ChatMessageSnapshot {
                    id: "1".to_string(),
                    chat_id: chat_id.clone(),
                    author: "peer".to_string(),
                    body: "hello".to_string(),
                    is_outgoing: false,
                    created_at_secs: 100,
                    delivery: DeliveryState::Received,
                }],
            },
        );

        core.update_screen_stack(vec![Screen::Chat {
            chat_id: chat_id.clone(),
        }]);

        assert_eq!(core.active_chat_id.as_deref(), Some(chat_id.as_str()));
        assert_eq!(core.threads.get(&chat_id).expect("thread").unread_count, 0);
        assert_eq!(
            core.state
                .current_chat
                .as_ref()
                .expect("current chat")
                .chat_id,
            chat_id
        );
    }

    #[test]
    fn protocol_subscription_uses_stable_id() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 69, false, false).expect("start session");

        let client = core.logged_in.as_ref().expect("logged in").client.clone();
        let protocol_id = SubscriptionId::new(PROTOCOL_SUBSCRIPTION_ID);

        let wait_for_subscription =
            |runtime: &tokio::runtime::Runtime, client: &Client, protocol_id: &SubscriptionId| {
                let deadline = Instant::now() + StdDuration::from_secs(5);
                while Instant::now() < deadline {
                    let subscriptions = runtime.block_on(client.subscriptions());
                    if subscriptions.contains_key(&protocol_id) {
                        return subscriptions;
                    }
                    thread::sleep(StdDuration::from_millis(50));
                }
                runtime.block_on(client.subscriptions())
            };

        let initial = wait_for_subscription(&core.runtime, &client, &protocol_id);
        assert_eq!(initial.len(), 1);
        assert!(initial.contains_key(&protocol_id));

        core.create_chat(&npub_for_fill(70));

        let after_create = wait_for_subscription(&core.runtime, &client, &protocol_id);
        assert_eq!(after_create.len(), 1);
        assert!(after_create.contains_key(&protocol_id));
    }

    #[test]
    fn protocol_subscription_plan_sorts_roster_authors_stably() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 71, false, false).expect("start session");

        let owner_a = local_owner_from_keys(&keys_for_fill(90)).to_string();
        let owner_b = local_owner_from_keys(&keys_for_fill(89)).to_string();

        core.threads.insert(
            owner_a.clone(),
            ThreadRecord {
                chat_id: owner_a.clone(),
                unread_count: 0,
                updated_at_secs: 10,
                messages: Vec::new(),
            },
        );
        core.threads.insert(
            owner_b.clone(),
            ThreadRecord {
                chat_id: owner_b.clone(),
                unread_count: 0,
                updated_at_secs: 20,
                messages: Vec::new(),
            },
        );
        core.pending_outbound.push(PendingOutbound {
            message_id: "dup-owner-a".to_string(),
            chat_id: owner_a.clone(),
            body: "pending".to_string(),
            prepared_publish: None,
            publish_mode: OutboundPublishMode::WaitForPeer,
            reason: PendingSendReason::MissingRoster,
            next_retry_at_secs: 123,
            in_flight: false,
        });
        core.recent_handshake_peers.insert(
            "device-b".to_string(),
            RecentHandshakePeer {
                owner_hex: owner_b.clone(),
                device_hex: "device-b".to_string(),
                observed_at_secs: 50,
            },
        );

        let plan = core
            .compute_protocol_subscription_plan()
            .expect("subscription plan");
        let mut expected = vec![
            core.logged_in
                .as_ref()
                .expect("logged in")
                .owner_pubkey
                .to_string(),
            owner_a,
            owner_b,
        ];
        expected.sort();
        expected.dedup();

        assert_eq!(plan.roster_authors, expected);
    }

    #[test]
    fn protocol_subscription_refresh_marks_dirty_while_refresh_is_in_flight() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 72, false, false).expect("start session");

        core.protocol_subscription_runtime.refresh_in_flight = true;
        core.protocol_subscription_runtime.refresh_dirty = false;
        let token_before = core.protocol_subscription_runtime.refresh_token;

        core.request_protocol_subscription_refresh();

        assert!(core.protocol_subscription_runtime.refresh_dirty);
        assert!(core.protocol_subscription_runtime.refresh_in_flight);
        assert_eq!(core.protocol_subscription_runtime.refresh_token, token_before);
    }

    #[test]
    fn established_send_stays_pending_until_publish_ack() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::custom("ws://127.0.0.1:59999");
        let data_dir = TempDir::new().expect("temp dir");
        let (alice_manager, _bob_manager, chat_id) =
            established_session_manager_pair(73, 74, 1_900_000_000);
        let mut core = logged_in_core_with_manager(data_dir.path(), 73, alice_manager);

        core.send_message(&chat_id, "offline direct send");

        let last_message = core
            .state
            .current_chat
            .as_ref()
            .expect("current chat")
            .messages
            .last()
            .expect("outgoing message");
        assert_eq!(last_message.body, "offline direct send");
        assert!(matches!(last_message.delivery, DeliveryState::Pending));
        assert!(!core.state.busy.sending_message);
        assert_eq!(core.pending_outbound.len(), 1);
        assert_eq!(
            core.pending_outbound[0].publish_mode,
            OutboundPublishMode::OrdinaryFirstAck
        );
        assert_eq!(
            core.pending_outbound[0].reason,
            PendingSendReason::PublishRetry
        );
    }

    #[test]
    fn retry_pending_outbound_reuses_same_prepared_events_without_advancing_session() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::custom("ws://127.0.0.1:59999");
        let data_dir = TempDir::new().expect("temp dir");
        let (alice_manager, _bob_manager, chat_id) =
            established_session_manager_pair(77, 78, 1_900_000_100);
        let mut core = logged_in_core_with_manager(data_dir.path(), 77, alice_manager);

        core.send_message(&chat_id, "retry me");

        let message_id = core.pending_outbound[0].message_id.clone();
        let event_ids_before = pending_publish_event_ids(&core, &message_id);

        core.handle_internal(InternalEvent::PublishFinished {
            message_id: message_id.clone(),
            chat_id: chat_id.clone(),
            success: false,
        });

        let pending_after_failure = core
            .pending_outbound
            .iter()
            .find(|pending| pending.message_id == message_id)
            .expect("pending outbound after failure");
        assert_eq!(pending_after_failure.publish_mode, OutboundPublishMode::OrdinaryFirstAck);
        assert_eq!(pending_after_failure.reason, PendingSendReason::PublishRetry);
        assert!(!pending_after_failure.in_flight);
        assert!(pending_after_failure.next_retry_at_secs > 0);

        let snapshot_before_retry = core
            .logged_in
            .as_ref()
            .expect("logged in")
            .session_manager
            .snapshot();
        if let Some(pending) = core
            .pending_outbound
            .iter_mut()
            .find(|pending| pending.message_id == message_id)
        {
            pending.next_retry_at_secs = 0;
        }

        core.retry_pending_outbound(UnixSeconds(1_900_000_200));

        let snapshot_after_retry = core
            .logged_in
            .as_ref()
            .expect("logged in")
            .session_manager
            .snapshot();
        let event_ids_after = pending_publish_event_ids(&core, &message_id);

        assert_eq!(event_ids_after, event_ids_before);
        assert_eq!(snapshot_after_retry, snapshot_before_retry);
        assert!(
            core.pending_outbound
                .iter()
                .find(|pending| pending.message_id == message_id)
                .expect("pending outbound after retry")
                .in_flight
        );
    }

    #[test]
    fn publish_finished_success_marks_message_sent_and_clears_pending() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::custom("ws://127.0.0.1:59999");
        let data_dir = TempDir::new().expect("temp dir");
        let (alice_manager, _bob_manager, chat_id) =
            established_session_manager_pair(79, 80, 1_900_000_300);
        let mut core = logged_in_core_with_manager(data_dir.path(), 79, alice_manager);

        core.send_message(&chat_id, "mark me sent");
        let message_id = core.pending_outbound[0].message_id.clone();

        core.handle_internal(InternalEvent::PublishFinished {
            message_id: message_id.clone(),
            chat_id: chat_id.clone(),
            success: true,
        });

        assert!(core.pending_outbound.is_empty());
        let last_message = core
            .state
            .current_chat
            .as_ref()
            .expect("current chat")
            .messages
            .last()
            .expect("message");
        assert_eq!(last_message.id, message_id);
        assert!(matches!(last_message.delivery, DeliveryState::Sent));
    }

    #[test]
    fn publish_events_first_ack_succeeds_when_any_relay_accepts() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::custom("ws://127.0.0.1:4848,ws://127.0.0.1:59999");
        let _relay = TestRelay::start();
        let owner_keys = keys_for_fill(81);
        let device_keys = device_keys_for_fill(81);
        let owner = local_owner_from_keys(&owner_keys);
        let roster = DeviceRoster::new(
            UnixSeconds(1_900_000_400),
            vec![AuthorizedDevice::new(
                local_device_from_keys(&device_keys),
                UnixSeconds(1_900_000_400),
            )],
        );
        let event = codec::roster_unsigned_event(owner, &roster)
            .expect("roster event")
            .sign_with_keys(&owner_keys)
            .expect("sign roster");
        let client = Client::new(device_keys);
        let relay_urls = configured_relay_urls();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        runtime.block_on(async {
            ensure_session_relays_configured(&client, &relay_urls).await;
            client.connect().await;
            publish_events_first_ack(&client, &relay_urls, &[event.clone()], "test first ack")
                .await
                .expect("first ack publish");
            let _ = client.shutdown().await;
        });

        let events = fetch_local_relay_events(vec![Filter::new().kind(Kind::from(
            codec::ROSTER_EVENT_KIND as u16,
        ))]);
        assert!(events.iter().any(|candidate| candidate.id == event.id));
    }

    #[test]
    fn publish_events_first_ack_fails_when_all_relays_fail() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::custom("ws://127.0.0.1:59999");
        let owner_keys = keys_for_fill(82);
        let device_keys = device_keys_for_fill(82);
        let owner = local_owner_from_keys(&owner_keys);
        let roster = DeviceRoster::new(
            UnixSeconds(1_900_000_500),
            vec![AuthorizedDevice::new(
                local_device_from_keys(&device_keys),
                UnixSeconds(1_900_000_500),
            )],
        );
        let event = codec::roster_unsigned_event(owner, &roster)
            .expect("roster event")
            .sign_with_keys(&owner_keys)
            .expect("sign roster");
        let client = Client::new(device_keys);
        let relay_urls = configured_relay_urls();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let result = runtime.block_on(async {
            ensure_session_relays_configured(&client, &relay_urls).await;
            client.connect().await;
            let result =
                publish_events_first_ack(&client, &relay_urls, &[event], "test first ack").await;
            let _ = client.shutdown().await;
            result
        });

        assert!(result.is_err());
    }

    #[test]
    fn prepared_pending_outbound_persists_across_restore() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::custom("ws://127.0.0.1:59999");
        let data_dir = TempDir::new().expect("temp dir");
        let (alice_manager, _bob_manager, chat_id) =
            established_session_manager_pair(79, 80, 1_900_000_300);
        let mut core = logged_in_core_with_manager(data_dir.path(), 79, alice_manager);

        core.send_message(&chat_id, "persist me");
        let message_id = core.pending_outbound[0].message_id.clone();
        let event_ids_before = pending_publish_event_ids(&core, &message_id);
        core.persist_best_effort();

        let mut restored = test_core(data_dir.path());
        start_primary_test_session(&mut restored, 79, true, true)
            .expect("restore session with prepared pending outbound");

        let restored_message_id = restored.pending_outbound[0].message_id.clone();
        let event_ids_after = pending_publish_event_ids(&restored, &restored_message_id);

        assert_eq!(restored.pending_outbound.len(), 1);
        assert_eq!(event_ids_after, event_ids_before);
    }

    #[test]
    fn pending_inbound_persists_across_restore() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let owner_keys = keys_for_fill(75);
        let device_keys = device_keys_for_fill(75);
        let owner = local_owner_from_keys(&owner_keys);
        let now = UnixSeconds(500);
        let local_device = local_device_from_keys(&device_keys);

        let mut session_manager =
            SessionManager::new(owner, device_keys.secret_key().to_secret_bytes());
        session_manager.apply_local_roster(DeviceRoster::new(
            now,
            vec![AuthorizedDevice::new(local_device, now)],
        ));

        let mut core = logged_in_core_with_manager(data_dir.path(), 75, session_manager);
        core.pending_inbound.push(PendingInbound {
            envelope: MessageEnvelope {
                sender: local_device_from_keys(&device_keys_for_fill(76)),
                signer_secret_key: [9; 32],
                created_at: UnixSeconds(501),
                encrypted_header: "header".to_string(),
                ciphertext: "ciphertext".to_string(),
            },
        });
        core.persist_best_effort();

        let persisted = persisted_state(data_dir.path());
        assert_eq!(persisted.pending_inbound.len(), 1);
        assert_eq!(
            persisted.pending_inbound[0].envelope.ciphertext,
            "ciphertext"
        );

        let mut restored = test_core(data_dir.path());
        start_primary_test_session(&mut restored, 75, true, true)
            .expect("restore session with pending inbound");

        assert_eq!(restored.pending_inbound.len(), 1);
        assert_eq!(
            restored.pending_inbound[0].envelope.encrypted_header,
            "header"
        );
    }

    #[test]
    fn recent_handshake_peer_tracks_claimed_owner_after_roster_verification() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");

        let alice_device_keys = device_keys_for_fill(120);
        let alice_claimed_owner_keys = keys_for_fill(121);
        let bob_owner_keys = keys_for_fill(122);
        let bob_device_keys = device_keys_for_fill(122);

        let alice_device_owner = local_owner_from_keys(&alice_device_keys);
        let alice_claimed_owner = local_owner_from_keys(&alice_claimed_owner_keys);
        let bob_owner = local_owner_from_keys(&bob_owner_keys);
        let now = UnixSeconds(1_900_000_400);

        let mut alice_manager = SessionManager::new(
            alice_claimed_owner,
            alice_device_keys.secret_key().to_secret_bytes(),
        );
        let mut bob_manager =
            SessionManager::new(bob_owner, bob_device_keys.secret_key().to_secret_bytes());

        let bob_roster =
            DeviceRoster::new(
                now,
                vec![AuthorizedDevice::new(local_device_from_keys(&bob_device_keys), now)],
            );
        bob_manager.apply_local_roster(bob_roster.clone());
        alice_manager.observe_peer_roster(bob_owner, bob_roster);

        let bob_invite = {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(now.get() + 1), &mut rng);
            bob_manager
                .ensure_local_invite(&mut ctx)
                .expect("ensure bob invite")
                .clone()
        };
        alice_manager
            .observe_device_invite(bob_owner, bob_invite)
            .expect("observe bob invite");

        let prepared = {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(now.get() + 2), &mut rng);
            alice_manager
                .prepare_send(&mut ctx, bob_owner, b"claim migration".to_vec())
                .expect("prepare first-contact send")
        };
        assert_eq!(prepared.invite_responses.len(), 1);

        let mut core = logged_in_core_with_manager(data_dir.path(), 122, bob_manager);
        let processed = {
            let logged_in = core.logged_in.as_mut().expect("logged in");
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(UnixSeconds(now.get() + 3), &mut rng);
            logged_in
                .session_manager
                .observe_invite_response(&mut ctx, &prepared.invite_responses[0])
                .expect("observe invite response")
                .expect("processed invite response")
        };
        assert_eq!(processed.owner_pubkey, alice_device_owner);

        core.remember_recent_handshake_peer(
            processed.owner_pubkey.to_string(),
            processed.device_pubkey.to_string(),
            now.get(),
        );
        assert!(core
            .protocol_owner_hexes()
            .contains(&alice_device_owner.to_string()));
        assert!(core
            .protocol_owner_hexes()
            .contains(&alice_claimed_owner.to_string()));

        {
            let logged_in = core.logged_in.as_mut().expect("logged in");
            let alice_roster = DeviceRoster::new(
                UnixSeconds(now.get() + 4),
                vec![AuthorizedDevice::new(
                    local_device_from_keys(&alice_device_keys),
                    UnixSeconds(now.get() + 4),
                )],
            );
            logged_in
                .session_manager
                .observe_peer_roster(alice_claimed_owner, alice_roster);
        }

        let migrated_owner_hexes = core.reconcile_recent_handshake_peers();
        assert!(migrated_owner_hexes
            .iter()
            .any(|(_, owner_hex)| owner_hex == &alice_claimed_owner.to_string()));
        assert!(core
            .protocol_owner_hexes()
            .contains(&alice_claimed_owner.to_string()));
        assert!(!core
            .protocol_owner_hexes()
            .contains(&alice_device_owner.to_string()));
    }

    #[test]
    fn invite_response_delivery_requires_matching_pubkey_tag() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let _relay = TestRelay::start();
        let (_alice_dir, alice) = app(71);
        let (_bob_dir, bob) = app(72);
        let (charlie_dir, _charlie) = app(73);

        alice.dispatch(AppAction::CreateChat {
            peer_input: npub_for_fill(72),
        });
        let alice_chat_id = wait_for_state(&alice, "alice chat create", |state| {
            state.current_chat.is_some()
        })
        .current_chat
        .expect("alice chat")
        .chat_id;

        alice.dispatch(AppAction::SendMessage {
            chat_id: alice_chat_id,
            text: "hello bob".to_string(),
        });

        wait_for_state(&bob, "bob receives hello", |state| {
            state
                .chat_list
                .iter()
                .any(|chat| chat.last_message_preview.as_deref() == Some("hello bob"))
        });

        let charlie_persisted = wait_for_persisted_state(
            charlie_dir.path(),
            "charlie stays scoped to local invite responses",
            |persisted| {
                persisted
                    .session_manager
                    .as_ref()
                    .map(|snapshot| snapshot.users.len() == 1)
                    .unwrap_or(false)
            },
        );
        assert_eq!(
            charlie_persisted
                .session_manager
                .expect("charlie session manager")
                .users
                .len(),
            1
        );
    }

    #[test]
    fn pending_send_waits_for_missing_roster_then_delivers() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let _relay = TestRelay::start();
        let (alice_dir, alice) = app(81);

        alice.dispatch(AppAction::CreateChat {
            peer_input: npub_for_fill(82),
        });
        let chat_id = wait_for_state(&alice, "alice chat create", |state| {
            state.current_chat.is_some()
        })
        .current_chat
        .expect("alice chat")
        .chat_id;

        alice.dispatch(AppAction::SendMessage {
            chat_id,
            text: "waiting on roster".to_string(),
        });

        let alice_pending = wait_for_persisted_state(
            alice_dir.path(),
            "alice waits for missing roster",
            |persisted| {
                persisted.pending_outbound.len() == 1
                    && persisted.pending_outbound[0].reason == PendingSendReason::MissingRoster
            },
        );
        assert_eq!(alice_pending.pending_outbound.len(), 1);

        let (_bob_dir, bob) = app(82);
        wait_for_state(&bob, "bob receives queued first message", |state| {
            state
                .chat_list
                .iter()
                .any(|chat| chat.last_message_preview.as_deref() == Some("waiting on roster"))
        });
        let alice_cleared = wait_for_persisted_state(
            alice_dir.path(),
            "alice clears pending outbound after roster arrives",
            |persisted| persisted.pending_outbound.is_empty(),
        );
        assert!(alice_cleared.pending_outbound.is_empty());
    }

    #[test]
    fn pending_send_waits_for_missing_device_invite_then_publishes() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let _relay = TestRelay::start();

        let bob_owner_keys = keys_for_fill(92);
        let bob_device_keys = device_keys_for_fill(92);
        let bob_owner = local_owner_from_keys(&bob_owner_keys);
        let now = unix_now();
        let bob_roster =
            DeviceRoster::new(
                now,
                vec![AuthorizedDevice::new(local_device_from_keys(&bob_device_keys), now)],
            );
        let roster_event = codec::roster_unsigned_event(bob_owner, &bob_roster)
            .expect("bob roster event")
            .sign_with_keys(&bob_owner_keys)
            .expect("sign bob roster");
        publish_local_relay_event(&bob_owner_keys, roster_event);

        let (_alice_dir, alice) = app(91);
        alice.dispatch(AppAction::CreateChat {
            peer_input: npub_for_fill(92),
        });
        let chat_id = wait_for_state(&alice, "alice chat create", |state| {
            state.current_chat.is_some()
        })
        .current_chat
        .expect("alice chat")
        .chat_id;

        alice.dispatch(AppAction::SendMessage {
            chat_id: chat_id.clone(),
            text: "waiting on invite".to_string(),
        });

        wait_for_state(&alice, "alice send pending on missing invite", |state| {
            state
                .current_chat
                .as_ref()
                .and_then(|chat| chat.messages.last())
                .map(|message| matches!(message.delivery, DeliveryState::Pending))
                .unwrap_or(false)
        });

        let mut session_manager =
            SessionManager::new(bob_owner, bob_device_keys.secret_key().to_secret_bytes());
        session_manager.apply_local_roster(bob_roster);
        let mut rng = OsRng;
        let mut ctx = ProtocolContext::new(unix_now(), &mut rng);
        let invite = session_manager
            .ensure_local_invite(&mut ctx)
            .expect("ensure bob invite")
            .clone();
        let invite_event = codec::invite_unsigned_event(&invite)
            .expect("bob invite event")
            .sign_with_keys(&bob_device_keys)
            .expect("sign bob invite");
        publish_local_relay_event(&bob_device_keys, invite_event);

        let alice_after_invite =
            wait_for_state(&alice, "alice publishes after invite appears", |state| {
                state
                    .current_chat
                    .as_ref()
                    .and_then(|chat| chat.messages.last())
                    .map(|message| matches!(message.delivery, DeliveryState::Sent))
                    .unwrap_or(false)
            });
        assert_eq!(
            alice_after_invite
                .current_chat
                .expect("alice current chat")
                .messages
                .last()
                .expect("alice last message")
                .body,
            "waiting on invite"
        );
    }

    #[test]
    fn local_relay_round_trip_and_reverse_initiation_work() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let _relay = TestRelay::start();
        let (_alice_dir, alice) = app(51);
        let (_bob_dir, bob) = app(52);

        alice.dispatch(AppAction::CreateChat {
            peer_input: npub_for_fill(52),
        });
        let alice_state = wait_for_state(&alice, "alice chat create", |state| {
            state.current_chat.is_some() && state.chat_list.len() == 1
        });
        let alice_chat_id = alice_state
            .current_chat
            .as_ref()
            .expect("alice chat")
            .chat_id
            .clone();

        alice.dispatch(AppAction::SendMessage {
            chat_id: alice_chat_id.clone(),
            text: "hello bob".to_string(),
        });
        let _alice_after_send = wait_for_state(&alice, "alice outbound queued", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| !chat.messages.is_empty())
                .unwrap_or(false)
        });

        let bob_state = wait_for_state(&bob, "bob receives first message", |state| {
            state.chat_list.iter().any(|chat| {
                chat.last_message_preview.as_deref() == Some("hello bob") && chat.unread_count == 1
            })
        });
        let bob_chat_id = bob_state.chat_list[0].chat_id.clone();
        bob.dispatch(AppAction::OpenChat {
            chat_id: bob_chat_id.clone(),
        });
        let bob_open = wait_for_state(&bob, "bob opens chat", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| chat.messages.len() == 1)
                .unwrap_or(false)
                && state
                    .chat_list
                    .first()
                    .map(|chat| chat.unread_count == 0)
                    .unwrap_or(false)
        });
        assert_eq!(
            bob_open
                .current_chat
                .as_ref()
                .expect("bob current chat")
                .messages[0]
                .body,
            "hello bob"
        );

        bob.dispatch(AppAction::SendMessage {
            chat_id: bob_chat_id.clone(),
            text: "hi alice".to_string(),
        });
        let _bob_after_reply = wait_for_state(&bob, "bob sends reply", |state| {
            state
                .current_chat
                .as_ref()
                .and_then(|chat| chat.messages.last())
                .map(|message| {
                    message.is_outgoing &&
                        message.body == "hi alice" &&
                        matches!(message.delivery, DeliveryState::Sent)
                })
                .unwrap_or(false)
        });
        let alice_with_reply = wait_for_state(&alice, "alice receives reply", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| chat.messages.len() >= 2)
                .unwrap_or(false)
        });
        assert_eq!(
            alice_with_reply
                .current_chat
                .as_ref()
                .expect("alice current chat")
                .messages
                .last()
                .expect("alice last message")
                .body,
            "hi alice"
        );
        assert!(alice_with_reply.toast.is_none());

        let (_charlie_dir, charlie) = app(53);
        let (_dana_dir, dana) = app(54);
        dana.dispatch(AppAction::CreateChat {
            peer_input: npub_for_fill(53),
        });
        let dana_chat_id = wait_for_state(&dana, "dana chat create", |state| {
            state.current_chat.is_some()
        })
        .current_chat
        .expect("dana chat")
        .chat_id;
        dana.dispatch(AppAction::SendMessage {
            chat_id: dana_chat_id,
            text: "reverse first".to_string(),
        });
        let charlie_state = wait_for_state(&charlie, "charlie receives reverse message", |state| {
            state
                .chat_list
                .iter()
                .any(|chat| chat.last_message_preview.as_deref() == Some("reverse first"))
        });
        assert_eq!(
            charlie_state.chat_list[0].last_message_preview.as_deref(),
            Some("reverse first")
        );
    }

    #[test]
    fn primary_bootstrap_uses_separate_device_identity_and_one_device_roster() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());

        start_primary_test_session(&mut core, 140, false, false).expect("start primary session");

        let account = core.state.account.as_ref().expect("account");
        assert_ne!(account.public_key_hex, account.device_public_key_hex);
        assert!(account.has_owner_signing_authority);
        assert!(matches!(
            account.authorization_state,
            DeviceAuthorizationState::Authorized
        ));

        let snapshot = core
            .logged_in
            .as_ref()
            .expect("logged in")
            .session_manager
            .snapshot();
        assert_eq!(snapshot.local_owner_pubkey.to_string(), account.public_key_hex);
        assert_eq!(snapshot.local_device_pubkey.to_string(), account.device_public_key_hex);

        let local_roster = snapshot
            .users
            .into_iter()
            .find(|user| user.owner_pubkey == snapshot.local_owner_pubkey)
            .and_then(|user| user.roster)
            .expect("local roster");
        assert_eq!(local_roster.devices().len(), 1);
        assert_eq!(
            local_roster.devices()[0].device_pubkey.to_string(),
            account.device_public_key_hex
        );
    }

    #[test]
    fn linked_device_starts_awaiting_approval_with_device_invite() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());

        core.start_linked_device(&npub_for_fill(141));

        let account = core.state.account.as_ref().expect("account");
        assert!(!account.has_owner_signing_authority);
        assert!(matches!(
            account.authorization_state,
            DeviceAuthorizationState::AwaitingApproval
        ));
        assert!(matches!(
            core.state.router.default_screen,
            Screen::AwaitingDeviceApproval
        ));
        assert!(core
            .logged_in
            .as_ref()
            .expect("logged in")
            .session_manager
            .snapshot()
            .local_invite
            .is_some());
        let roster = core.state.device_roster.as_ref().expect("device roster");
        assert_eq!(roster.devices.len(), 1);
        assert!(!roster.devices[0].is_authorized);
        assert!(roster.devices[0].is_current_device);
    }

    #[test]
    fn linked_device_becomes_authorized_when_owner_roster_arrives() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let owner_keys = keys_for_fill(142);
        let owner = local_owner_from_keys(&owner_keys);
        let mut core = test_core(data_dir.path());

        core.start_linked_device(&owner_keys.public_key().to_bech32().expect("owner npub"));
        let device_pubkey = parse_device_input(
            &core.state
                .account
                .as_ref()
                .expect("account")
                .device_public_key_hex,
        )
        .expect("device pubkey");
        let now = unix_now();
        let roster = DeviceRoster::new(now, vec![AuthorizedDevice::new(device_pubkey, now)]);
        let roster_event = codec::roster_unsigned_event(owner, &roster)
            .expect("roster event")
            .sign_with_keys(&owner_keys)
            .expect("sign roster");

        core.handle_relay_event(roster_event);

        assert!(matches!(
            core.state
                .account
                .as_ref()
                .expect("account")
                .authorization_state,
            DeviceAuthorizationState::Authorized
        ));
        assert!(matches!(core.state.router.default_screen, Screen::ChatList));
        assert!(core
            .state
            .device_roster
            .as_ref()
            .expect("roster")
            .devices
            .iter()
            .any(|device| device.device_pubkey_hex == device_pubkey.to_string() && device.is_authorized));
    }

    #[test]
    fn linked_device_transitions_to_revoked_when_owner_roster_excludes_it() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let owner_keys = keys_for_fill(143);
        let owner = local_owner_from_keys(&owner_keys);
        let mut core = test_core(data_dir.path());

        core.start_linked_device(&owner_keys.public_key().to_bech32().expect("owner npub"));
        let device_pubkey = parse_device_input(
            &core.state
                .account
                .as_ref()
                .expect("account")
                .device_public_key_hex,
        )
        .expect("device pubkey");

        let authorized_now = unix_now();
        let authorized_roster =
            DeviceRoster::new(authorized_now, vec![AuthorizedDevice::new(device_pubkey, authorized_now)]);
        let authorized_event = codec::roster_unsigned_event(owner, &authorized_roster)
            .expect("authorized roster event")
            .sign_with_keys(&owner_keys)
            .expect("sign authorized roster");
        core.handle_relay_event(authorized_event);

        let revoked_roster = DeviceRoster::new(UnixSeconds(authorized_now.get() + 1), Vec::new());
        let revoked_event = codec::roster_unsigned_event(owner, &revoked_roster)
            .expect("revoked roster event")
            .sign_with_keys(&owner_keys)
            .expect("sign revoked roster");
        core.handle_relay_event(revoked_event);

        assert!(matches!(
            core.state
                .account
                .as_ref()
                .expect("account")
                .authorization_state,
            DeviceAuthorizationState::Revoked
        ));
        assert!(matches!(
            core.state.router.default_screen,
            Screen::DeviceRevoked
        ));
    }

    #[test]
    fn add_and_remove_authorized_device_updates_local_roster_snapshot() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 144, false, false).expect("start primary session");

        let added_device_npub = device_keys_for_fill(145)
            .public_key()
            .to_bech32()
            .expect("device npub");
        core.add_authorized_device(&added_device_npub);
        assert!(core
            .state
            .device_roster
            .as_ref()
            .expect("device roster")
            .devices
            .iter()
            .any(|device| device.device_npub == added_device_npub && device.is_authorized));

        let added_device_hex = normalize_peer_input_for_display(&added_device_npub);
        core.remove_authorized_device(&added_device_hex);
        assert!(!core
            .state
            .device_roster
            .as_ref()
            .expect("device roster")
            .devices
            .iter()
            .any(|device| device.device_pubkey_hex == added_device_hex));
    }

    #[test]
    fn primary_identity_publishes_owner_signed_roster_and_device_signed_invite() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let _relay = TestRelay::start();
        let data_dir = TempDir::new().expect("temp dir");
        let mut core = test_core(data_dir.path());
        start_primary_test_session(&mut core, 146, false, false).expect("start primary session");

        let account = core.state.account.as_ref().expect("account");
        let owner_key = PublicKey::parse(&account.public_key_hex).expect("owner key");
        let device_key = PublicKey::parse(&account.device_public_key_hex).expect("device key");

        let deadline = Instant::now() + StdDuration::from_secs(5);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            events = fetch_local_relay_events(vec![Filter::new().kind(Kind::from(codec::ROSTER_EVENT_KIND as u16))]);
            let has_roster = events.iter().any(|event| {
                codec::parse_roster_event(event).is_ok() && event.pubkey == owner_key
            });
            let has_invite = events.iter().any(|event| {
                codec::parse_invite_event(event).is_ok() && event.pubkey == device_key
            });
            if has_roster && has_invite {
                break;
            }
            thread::sleep(StdDuration::from_millis(100));
        }
        assert!(events.iter().any(|event| {
            codec::parse_roster_event(event).is_ok() && event.pubkey == owner_key
        }));
        assert!(events.iter().any(|event| {
            codec::parse_invite_event(event).is_ok() && event.pubkey == device_key
        }));
    }

    #[test]
    fn local_relay_duplicate_replay_is_ignored_and_state_restores() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let relay = TestRelay::start();
        let alice_dir = TempDir::new().expect("alice dir");
        let bob_dir = TempDir::new().expect("bob dir");
        let alice = app_with_dir(alice_dir.path(), 61);
        let bob = app_with_dir(bob_dir.path(), 62);

        alice.dispatch(AppAction::CreateChat {
            peer_input: npub_for_fill(62),
        });
        let chat_id = wait_for_state(&alice, "alice chat create", |state| {
            state.current_chat.is_some()
        })
        .current_chat
        .expect("alice chat")
        .chat_id;
        alice.dispatch(AppAction::SendMessage {
            chat_id: chat_id.clone(),
            text: "dedupe".to_string(),
        });

        let bob_chat_id = wait_for_state(&bob, "bob first delivery", |state| {
            state
                .chat_list
                .iter()
                .any(|chat| chat.last_message_preview.as_deref() == Some("dedupe"))
        })
        .chat_list[0]
            .chat_id
            .clone();
        bob.dispatch(AppAction::OpenChat {
            chat_id: bob_chat_id.clone(),
        });
        wait_for_state(&bob, "bob chat open", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| chat.messages.len() == 1)
                .unwrap_or(false)
        });

        relay.replay_stored();
        thread::sleep(StdDuration::from_secs(1));
        let bob_after_replay = bob.state();
        assert_eq!(
            bob_after_replay
                .current_chat
                .as_ref()
                .expect("bob current chat")
                .messages
                .len(),
            1
        );
        assert!(bob_after_replay.toast.is_none());

        drop(bob);
        let restored_bob = app_with_dir(bob_dir.path(), 62);
        let restored_state = wait_for_state(&restored_bob, "restored bob thread", |state| {
            !state.chat_list.is_empty()
        });
        assert_eq!(
            restored_state.chat_list[0].last_message_preview.as_deref(),
            Some("dedupe")
        );
        assert!(restored_state.toast.is_none());
    }

    #[test]
    fn linked_device_fanout_and_revocation_work_on_local_relay() {
        let _guard = relay_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _env = RelayEnvGuard::local_only();
        let _relay = TestRelay::start();
        let primary_dir = TempDir::new().expect("primary dir");
        let linked_dir = TempDir::new().expect("linked dir");
        let peer_dir = TempDir::new().expect("peer dir");

        let primary = app_with_dir(primary_dir.path(), 71);
        let primary_account = wait_for_state(&primary, "primary account", |state| {
            state.account.is_some()
        })
        .account
        .expect("primary account");

        let linked = FfiApp::new(
            linked_dir.path().to_string_lossy().into_owned(),
            String::new(),
            "test".to_string(),
        );
        linked.dispatch(AppAction::StartLinkedDevice {
            owner_input: primary_account.npub.clone(),
        });

        let linked_account =
            wait_for_state(&linked, "linked awaiting approval", |state| {
                state
                    .account
                    .as_ref()
                    .map(|account| {
                        matches!(
                            account.authorization_state,
                            DeviceAuthorizationState::AwaitingApproval
                        )
                    })
                    .unwrap_or(false)
            })
            .account
            .expect("linked account");

        primary.dispatch(AppAction::AddAuthorizedDevice {
            device_input: linked_account.device_npub.clone(),
        });

        wait_for_state(&linked, "linked authorized", |state| {
            state
                .account
                .as_ref()
                .map(|account| {
                    matches!(
                        account.authorization_state,
                        DeviceAuthorizationState::Authorized
                    )
                })
                .unwrap_or(false)
        });

        let peer = app_with_dir(peer_dir.path(), 72);
        let peer_account = wait_for_state(&peer, "peer account", |state| {
            state.account.is_some()
        })
        .account
        .expect("peer account");

        let peer_chat_id = peer_account.public_key_hex.clone();
        let primary_chat_id = primary_account.public_key_hex.clone();

        primary.dispatch(AppAction::CreateChat {
            peer_input: peer_account.npub.clone(),
        });
        wait_for_state(&primary, "primary chat with peer", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| chat.chat_id == peer_chat_id)
                .unwrap_or(false)
        });

        primary.dispatch(AppAction::SendMessage {
            chat_id: peer_chat_id.clone(),
            text: "m1".to_string(),
        });

        wait_for_state(&peer, "peer received m1", |state| {
            state.chat_list.iter().any(|thread| {
                thread.chat_id == primary_chat_id
                    && thread.last_message_preview.as_deref() == Some("m1")
            })
        });
        let linked_after_m1 = wait_for_state(&linked, "linked received synced m1", |state| {
            state.chat_list.iter().any(|thread| {
                thread.chat_id == peer_chat_id
                    && thread.last_message_preview.as_deref() == Some("m1")
            })
        });
        assert!(linked_after_m1.chat_list.iter().any(|thread| {
            thread.chat_id == peer_chat_id && thread.last_message_preview.as_deref() == Some("m1")
        }));

        peer.dispatch(AppAction::OpenChat {
            chat_id: primary_chat_id.clone(),
        });
        wait_for_state(&peer, "peer chat open", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| chat.chat_id == primary_chat_id)
                .unwrap_or(false)
        });
        peer.dispatch(AppAction::SendMessage {
            chat_id: primary_chat_id.clone(),
            text: "m2".to_string(),
        });

        wait_for_state(&primary, "primary received m2", |state| {
            state.chat_list.iter().any(|thread| {
                thread.chat_id == peer_chat_id
                    && thread.last_message_preview.as_deref() == Some("m2")
            })
        });
        let linked_after_m2 = wait_for_state(&linked, "linked received m2", |state| {
            state.chat_list.iter().any(|thread| {
                thread.chat_id == peer_chat_id
                    && thread.last_message_preview.as_deref() == Some("m2")
            })
        });
        linked.dispatch(AppAction::OpenChat {
            chat_id: peer_chat_id.clone(),
        });
        let linked_chat = wait_for_state(&linked, "linked chat open", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| {
                    chat.chat_id == peer_chat_id
                        && chat.messages.iter().any(|message| {
                            message.body == "m1" && message.is_outgoing
                        })
                        && chat.messages.iter().any(|message| {
                            message.body == "m2" && !message.is_outgoing
                        })
                })
                .unwrap_or(false)
        })
        .current_chat
        .expect("linked chat");
        assert_eq!(linked_chat.chat_id, peer_chat_id);
        assert!(linked_after_m2.chat_list.iter().any(|thread| {
            thread.chat_id == peer_chat_id && thread.last_message_preview.as_deref() == Some("m2")
        }));

        linked.dispatch(AppAction::SendMessage {
            chat_id: peer_chat_id.clone(),
            text: "m3".to_string(),
        });

        wait_for_state(&peer, "peer received m3", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| {
                    chat.chat_id == primary_chat_id
                        && chat.messages.iter().any(|message| {
                            message.body == "m3" && !message.is_outgoing
                        })
                })
                .unwrap_or(false)
        });
        primary.dispatch(AppAction::OpenChat {
            chat_id: peer_chat_id.clone(),
        });
        wait_for_state(&primary, "primary received synced m3", |state| {
            state
                .current_chat
                .as_ref()
                .map(|chat| {
                    chat.chat_id == peer_chat_id
                        && chat.messages.iter().any(|message| {
                            message.body == "m3" && message.is_outgoing
                        })
                })
                .unwrap_or(false)
        });

        primary.dispatch(AppAction::RemoveAuthorizedDevice {
            device_pubkey_hex: linked_account.device_public_key_hex.clone(),
        });
        wait_for_state(&linked, "linked revoked", |state| {
            state
                .account
                .as_ref()
                .map(|account| {
                    matches!(
                        account.authorization_state,
                        DeviceAuthorizationState::Revoked
                    )
                })
                .unwrap_or(false)
        });

        let before_count = linked
            .state()
            .current_chat
            .as_ref()
            .map(|chat| chat.messages.len())
            .unwrap_or(0);
        linked.dispatch(AppAction::SendMessage {
            chat_id: peer_chat_id.clone(),
            text: "blocked".to_string(),
        });
        let blocked_state = wait_for_state(&linked, "linked send blocked", |state| {
            state.toast.as_deref()
                == Some("This device has been removed from the roster. Log out to continue.")
        });
        assert_eq!(
            blocked_state
                .current_chat
                .as_ref()
                .map(|chat| chat.messages.len())
                .unwrap_or(0),
            before_count
        );
    }
}
