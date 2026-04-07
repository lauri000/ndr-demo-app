use crate::actions::AppAction;
use crate::state::{
    AccountSnapshot, AppState, ChatMessageSnapshot, ChatThreadSnapshot, CurrentChatSnapshot,
    DeliveryState, Router, Screen,
};
use crate::updates::{AppUpdate, CoreMsg, InternalEvent};
use flume::Sender;
use nostr_double_ratchet::{
    AuthorizedDevice, DeviceRoster, DomainError, Error, MessageEnvelope, OwnerPubkey,
    ProtocolContext, RelayGap, SessionManager, SessionManagerSnapshot, SessionState, UnixSeconds,
};
use nostr_double_ratchet_nostr::nostr as codec;
use nostr_sdk::prelude::{
    Client, Event, Filter, Keys, Kind, PublicKey, RelayPoolNotification, Timestamp, ToBech32,
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
    recent_handshake_peers: BTreeMap<String, u64>,
    seen_event_ids: HashSet<String>,
    seen_event_order: VecDeque<String>,
}

struct LoggedInState {
    keys: Keys,
    client: Client,
    session_manager: SessionManager,
}

#[derive(Clone)]
struct ThreadRecord {
    chat_id: String,
    unread_count: u64,
    updated_at_secs: u64,
    messages: Vec<ChatMessageSnapshot>,
}

struct PendingInbound {
    envelope: MessageEnvelope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingOutbound {
    message_id: String,
    chat_id: String,
    body: String,
    #[serde(default)]
    reason: PendingSendReason,
    #[serde(default)]
    next_retry_at_secs: u64,
    #[serde(default)]
    in_flight: bool,
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

#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    version: u32,
    #[serde(alias = "active_peer_hex")]
    active_chat_id: Option<String>,
    next_message_id: u64,
    session_manager: Option<SessionManagerSnapshot>,
    threads: Vec<PersistedThread>,
    #[serde(default)]
    pending_outbound: Vec<PendingOutbound>,
    #[serde(default)]
    seen_event_ids: Vec<String>,
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
            AppAction::RestoreSession { nsec } => self.restore_session(&nsec),
            AppAction::Logout => self.logout(),
            AppAction::CreateChat { peer_input } => self.create_chat(&peer_input),
            AppAction::OpenChat { chat_id } => self.open_chat(&chat_id),
            AppAction::SendMessage { chat_id, text } => self.send_message(&chat_id, &text),
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
            InternalEvent::StagedSendFinished {
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
                    pending.next_retry_at_secs =
                        unix_now().get().saturating_add(FIRST_CONTACT_RETRY_DELAY_SECS);
                    self.schedule_pending_outbound_retry(Duration::from_secs(
                        FIRST_CONTACT_RETRY_DELAY_SECS,
                    ));
                }
                self.schedule_next_pending_retry(unix_now().get());
                self.rebuild_state();
                self.persist_best_effort();
                self.emit_state();
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

        let keys = Keys::generate();
        let nsec = keys
            .secret_key()
            .to_bech32()
            .unwrap_or_else(|_| keys.secret_key().to_secret_hex());

        if let Err(error) = self.start_session(keys, false) {
            self.state.toast = Some(error.to_string());
        } else if let Some(account) = self.state.account.clone() {
            let _ = self.update_tx.send(AppUpdate::AccountCreated {
                rev: self.state.rev,
                nsec,
                pubkey: account.public_key_hex,
                npub: account.npub,
            });
        }

        self.state.busy.creating_account = false;
        self.rebuild_state();
        self.emit_state();
    }

    fn restore_session(&mut self, nsec: &str) {
        self.state.busy.restoring_session = true;
        self.emit_state();

        let result = Keys::parse(nsec.trim())
            .map_err(|error| anyhow::anyhow!(error.to_string()))
            .and_then(|keys| self.start_session(keys, true));

        if let Err(error) = result {
            self.state.toast = Some(error.to_string());
        }

        self.state.busy.restoring_session = false;
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
        self.resubscribe_to_protocol_events();
        self.schedule_tracked_peer_catch_up(Duration::from_secs(
            RESUBSCRIBE_CATCH_UP_DELAY_SECS,
        ));
        self.state.busy.creating_chat = false;
        self.emit_state();
    }

    fn open_chat(&mut self, chat_id: &str) {
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
        self.resubscribe_to_protocol_events();
        self.schedule_tracked_peer_catch_up(Duration::from_secs(
            RESUBSCRIBE_CATCH_UP_DELAY_SECS,
        ));
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

        let owner = OwnerPubkey::from_bytes(peer_pubkey.to_bytes());
        let prepared = {
            let logged_in = self.logged_in.as_mut().expect("logged in checked above");
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(now, &mut rng);
            logged_in
                .session_manager
                .prepare_send(&mut ctx, owner, trimmed.as_bytes().to_vec())
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
                        reason,
                        now.get().saturating_add(PENDING_RETRY_DELAY_SECS),
                    );
                    if republish_identity {
                        self.republish_local_identity_artifacts();
                    }
                    self.resubscribe_to_protocol_events();
                    self.schedule_pending_outbound_retry(Duration::from_secs(
                        PENDING_RETRY_DELAY_SECS,
                    ));
                } else {
                    let invite_events = prepared
                        .invite_responses
                        .iter()
                        .map(codec::invite_response_event)
                        .collect::<std::result::Result<Vec<_>, _>>();
                    let message_events = prepared
                        .deliveries
                        .iter()
                        .map(|delivery| codec::message_event(&delivery.envelope))
                        .collect::<std::result::Result<Vec<_>, _>>();

                    match (invite_events, message_events) {
                        (Ok(invite_events), Ok(message_events))
                            if !invite_events.is_empty() && !message_events.is_empty() =>
                        {
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
                                PendingSendReason::PublishingFirstContact,
                                now.get().saturating_add(FIRST_CONTACT_RETRY_DELAY_SECS),
                            );
                            self.set_pending_outbound_in_flight(&message.id, true);
                            self.start_staged_first_contact_send(StagedOutboundSend {
                                message_id: message.id,
                                chat_id,
                                invite_events,
                                message_events,
                            });
                        }
                        (Ok(_), Ok(message_events)) if !message_events.is_empty() => {
                            let _message = self.push_outgoing_message(
                                &chat_id,
                                trimmed.to_string(),
                                now.get(),
                                DeliveryState::Sent,
                            );
                            self.resubscribe_to_protocol_events();
                            self.publish_events(message_events, "message");
                        }
                        (Err(error), _) | (_, Err(error)) => {
                            self.state.toast = Some(error.to_string());
                        }
                        _ => {
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
                self.screen_stack = vec![Screen::NewChat];
                self.active_chat_id = None;
            }
            Screen::Chat { chat_id } => {
                self.open_chat(&chat_id);
                return;
            }
            Screen::Welcome => return,
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
                Screen::Welcome | Screen::ChatList => {}
                Screen::NewChat => normalized_stack.push(Screen::NewChat),
                Screen::Chat { chat_id } => {
                    if let Ok((chat_id, _)) = parse_peer_input(&chat_id) {
                        normalized_stack.push(Screen::Chat { chat_id });
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
                    let is_foreign_roster = {
                        let logged_in = self.logged_in.as_ref().expect("checked above");
                        decoded.owner_pubkey != local_owner_from_keys(&logged_in.keys)
                    };
                    if is_foreign_roster {
                        self.logged_in
                            .as_mut()
                            .expect("checked above")
                            .session_manager
                            .observe_peer_roster(decoded.owner_pubkey, decoded.roster);
                        self.remember_event(event_id);
                        self.retry_pending_inbound(now);
                        self.retry_pending_outbound(now);
                        self.resubscribe_to_protocol_events();
                        self.persist_best_effort();
                        self.rebuild_state();
                        self.emit_state();
                        return;
                    }
                }

                if let Ok(invite) = codec::parse_invite_event(&event) {
                    let owner = invite.owner_public_key.unwrap_or(invite.inviter);
                    let is_foreign_invite = {
                        let logged_in = self.logged_in.as_ref().expect("checked above");
                        owner != local_owner_from_keys(&logged_in.keys)
                    };
                    if is_foreign_invite {
                        if let Err(error) = self
                            .logged_in
                            .as_mut()
                            .expect("checked above")
                            .session_manager
                            .observe_device_invite(owner, invite)
                        {
                            self.state.toast = Some(error.to_string());
                        } else {
                            self.remember_event(event_id.clone());
                            self.retry_pending_inbound(now);
                            self.retry_pending_outbound(now);
                            self.resubscribe_to_protocol_events();
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
                        self.remember_recent_handshake_peer(owner_hex, now.get());
                        self.retry_pending_inbound(now);
                        self.retry_pending_outbound(now);
                        self.resubscribe_to_protocol_events();
                        self.fetch_recent_messages_for_owner(processed.owner_pubkey, now);
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
                        self.push_incoming_message(
                            &owner_hex,
                            String::from_utf8_lossy(&message.payload).into_owned(),
                            now.get(),
                        );
                        self.resubscribe_to_protocol_events();
                        self.persist_best_effort();
                        self.rebuild_state();
                        self.emit_state();
                    }
                    Ok(None) => {
                        self.remember_event(event_id.clone());
                        self.pending_inbound.push(PendingInbound { envelope });
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

    fn start_session(&mut self, keys: Keys, allow_restore: bool) -> anyhow::Result<()> {
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
        self.next_message_id = 1;

        let secret_bytes = keys.secret_key().to_secret_bytes();
        let now = unix_now();
        let local_owner = local_owner_from_keys(&keys);
        let local_device = local_owner.as_device();

        let persisted = if allow_restore {
            self.load_persisted().ok().flatten()
        } else {
            None
        };

        if let Some(persisted) = &persisted {
            self.active_chat_id = persisted.active_chat_id.clone();
            self.next_message_id = persisted.next_message_id.max(1);
            self.pending_outbound = persisted.pending_outbound.clone();
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

        if let Some(chat_id) = self.active_chat_id.clone() {
            self.screen_stack = vec![Screen::Chat { chat_id }];
        }

        let mut session_manager = persisted
            .and_then(|persisted| persisted.session_manager)
            .map(|snapshot| SessionManager::from_snapshot(snapshot, secret_bytes))
            .transpose()?
            .unwrap_or_else(|| SessionManager::new(local_owner, secret_bytes));

        let local_roster = DeviceRoster::new(now, vec![AuthorizedDevice::new(local_device, now)]);
        session_manager.apply_local_roster(local_roster.clone());

        let invite_url = {
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(now, &mut rng);
            let invite = session_manager.ensure_local_invite(&mut ctx)?.clone();
            codec::invite_url(&invite, "https://chat.iris.to")?
        };

        let client = Client::new(keys.clone());
        self.start_notifications_loop(client.clone());
        self.publish_local_identity_artifacts(
            client.clone(),
            keys.clone(),
            local_owner,
            local_roster.clone(),
            invite_url.clone(),
        );

        self.logged_in = Some(LoggedInState {
            keys: keys.clone(),
            client,
            session_manager,
        });

        self.retry_pending_outbound(now);

        self.state.account = Some(AccountSnapshot {
            public_key_hex: keys.public_key().to_hex(),
            npub: keys
                .public_key()
                .to_bech32()
                .unwrap_or_else(|_| keys.public_key().to_hex()),
            invite_url,
        });
        self.state.busy.syncing_network = true;
        self.rebuild_state();
        self.persist_best_effort();
        self.resubscribe_to_protocol_events();
        self.schedule_tracked_peer_catch_up(Duration::from_secs(
            RESUBSCRIBE_CATCH_UP_DELAY_SECS,
        ));
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
                    self.push_incoming_message(
                        &message.owner_pubkey.to_string(),
                        String::from_utf8_lossy(&message.payload).into_owned(),
                        now.get(),
                    );
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

            let prepared = {
                let logged_in = self.logged_in.as_mut().expect("checked above");
                let mut rng = OsRng;
                let mut ctx = ProtocolContext::new(now, &mut rng);
                logged_in.session_manager.prepare_send(
                    &mut ctx,
                    owner,
                    pending_message.body.as_bytes().to_vec(),
                )
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
                        still_pending.push(pending_message);
                    } else {
                        let invite_events = prepared
                            .invite_responses
                            .iter()
                            .map(codec::invite_response_event)
                            .collect::<std::result::Result<Vec<_>, _>>();
                        let message_events = prepared
                            .deliveries
                            .iter()
                            .map(|delivery| codec::message_event(&delivery.envelope))
                            .collect::<std::result::Result<Vec<_>, _>>();

                        match (invite_events, message_events) {
                            (Ok(invite_events), Ok(message_events))
                                if !invite_events.is_empty() && !message_events.is_empty() =>
                            {
                                pending_message.reason = PendingSendReason::PublishingFirstContact;
                                pending_message.next_retry_at_secs =
                                    now.get().saturating_add(FIRST_CONTACT_RETRY_DELAY_SECS);
                                pending_message.in_flight = true;
                                self.start_staged_first_contact_send(StagedOutboundSend {
                                    message_id: pending_message.message_id.clone(),
                                    chat_id: pending_message.chat_id.clone(),
                                    invite_events,
                                    message_events,
                                });
                                still_pending.push(pending_message);
                            }
                            (Ok(_), Ok(message_events)) if !message_events.is_empty() => {
                                pending_message.reason = PendingSendReason::PublishRetry;
                                pending_message.next_retry_at_secs =
                                    now.get().saturating_add(FIRST_CONTACT_RETRY_DELAY_SECS);
                                pending_message.in_flight = true;
                                self.start_pending_message_publish(
                                    pending_message.message_id.clone(),
                                    pending_message.chat_id.clone(),
                                    message_events,
                                );
                                still_pending.push(pending_message);
                            }
                            (Err(error), _) | (_, Err(error)) => {
                                self.state.toast = Some(error.to_string());
                                self.update_message_delivery(
                                    &pending_message.chat_id,
                                    &pending_message.message_id,
                                    DeliveryState::Failed,
                                );
                            }
                            _ => {
                                pending_message.reason = PendingSendReason::MissingDeviceInvite;
                                pending_message.next_retry_at_secs =
                                    now.get().saturating_add(PENDING_RETRY_DELAY_SECS);
                                still_pending.push(pending_message);
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
        reason: PendingSendReason,
        next_retry_at_secs: u64,
    ) {
        self.pending_outbound.push(PendingOutbound {
            message_id,
            chat_id,
            body,
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
        self.recent_handshake_peers
            .retain(|owner, observed_at_secs| {
                let within_ttl =
                    now_secs.saturating_sub(*observed_at_secs) <= RECENT_HANDSHAKE_TTL_SECS;
                within_ttl && !self.threads.contains_key(owner)
            });
    }

    fn remember_recent_handshake_peer(&mut self, owner_hex: String, now_secs: u64) {
        if self.threads.contains_key(&owner_hex) {
            self.recent_handshake_peers.remove(&owner_hex);
            return;
        }
        self.recent_handshake_peers.insert(owner_hex, now_secs);
    }

    fn clear_recent_handshake_peer(&mut self, owner_hex: &str) {
        self.recent_handshake_peers.remove(owner_hex);
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
        owners.extend(self.recent_handshake_peers.keys().cloned());
        owners
    }

    fn schedule_pending_outbound_retry(&self, after: Duration) {
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            sleep(after).await;
            let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::RetryPendingOutbound)));
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
        self.resubscribe_to_protocol_events();
        self.publish_message_batch(message_id, chat_id, message_events);
    }

    fn start_staged_first_contact_send(&mut self, staged: StagedOutboundSend) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            return;
        };

        self.resubscribe_to_protocol_events();
        for event in staged
            .invite_events
            .iter()
            .chain(staged.message_events.iter())
        {
            self.remember_event(event.id.to_string());
        }

        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            for relay in configured_relays() {
                let _ = client.add_relay(relay).await;
            }
            client.connect_with_timeout(Duration::from_secs(5)).await;

            let invite_publish = publish_events_with_retry(
                &client,
                staged.invite_events,
                "invite response",
            )
            .await;
            if invite_publish.is_err() {
                let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::StagedSendFinished {
                    message_id: staged.message_id,
                    chat_id: staged.chat_id,
                    success: false,
                })));
                return;
            }

            sleep(Duration::from_millis(FIRST_CONTACT_STAGE_DELAY_MS)).await;

            let success = publish_events_with_retry(&client, staged.message_events, "message")
                .await
                .is_ok();
            let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::StagedSendFinished {
                message_id: staged.message_id,
                chat_id: staged.chat_id,
                success,
            })));
        });
    }

    fn publish_message_batch(&mut self, message_id: String, chat_id: String, events: Vec<Event>) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            return;
        };

        for event in &events {
            self.remember_event(event.id.to_string());
        }

        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            for relay in configured_relays() {
                let _ = client.add_relay(relay).await;
            }
            client.connect_with_timeout(Duration::from_secs(5)).await;
            let success = publish_events_with_retry(&client, events, "message")
                .await
                .is_ok();
            let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::StagedSendFinished {
                message_id,
                chat_id,
                success,
            })));
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

    fn message_filters_for_owner(&self, owner_pubkey: OwnerPubkey, now: UnixSeconds) -> Vec<Filter> {
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

        vec![
            Filter::new()
                .kind(Kind::from(codec::MESSAGE_EVENT_KIND as u16))
                .authors(authors)
                .since(Timestamp::from(
                    now.get().saturating_sub(CATCH_UP_LOOKBACK_SECS),
                )),
        ]
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

    fn allocate_message_id(&mut self) -> String {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.saturating_add(1);
        id.to_string()
    }

    fn rebuild_state(&mut self) {
        let default_screen = if self.state.account.is_some() {
            Screen::ChatList
        } else {
            Screen::Welcome
        };

        let mut threads: Vec<&ThreadRecord> = self.threads.values().collect();
        threads.sort_by_key(|thread| std::cmp::Reverse(thread.updated_at_secs));

        self.state.chat_list = threads
            .iter()
            .map(|thread| ChatThreadSnapshot {
                chat_id: thread.chat_id.clone(),
                display_name: owner_npub(&thread.chat_id).unwrap_or_else(|| thread.chat_id.clone()),
                peer_npub: owner_npub(&thread.chat_id).unwrap_or_else(|| thread.chat_id.clone()),
                last_message_preview: thread.messages.last().map(|message| message.body.clone()),
                unread_count: thread.unread_count,
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
            version: 3,
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
            pending_outbound: self.pending_outbound.clone(),
            seen_event_ids: self.seen_event_order.iter().cloned().collect(),
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

    fn publish_events(&mut self, events: Vec<Event>, label: &'static str) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            return;
        };

        for event in &events {
            self.remember_event(event.id.to_string());
        }

        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            for relay in configured_relays() {
                let _ = client.add_relay(relay).await;
            }
            client.connect_with_timeout(Duration::from_secs(5)).await;
            for event in events {
                if let Err(error) = publish_event_with_retry(&client, event, label).await {
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(format!(
                        "Publish failed: {error}"
                    )))));
                }
            }
        });
    }

    fn publish_local_identity_artifacts(
        &self,
        client: Client,
        keys: Keys,
        owner: OwnerPubkey,
        roster: DeviceRoster,
        invite_url: String,
    ) {
        let invite = match normalize_invite_from_url(&invite_url) {
            Ok(invite) => invite,
            Err(error) => {
                let _ = self
                    .core_sender
                    .send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                        error.to_string(),
                    ))));
                return;
            }
        };
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            for relay in configured_relays() {
                let _ = client.add_relay(relay).await;
            }
            client.connect_with_timeout(Duration::from_secs(5)).await;

            let roster_event = match codec::roster_unsigned_event(owner, &roster)
                .and_then(|unsigned| unsigned.sign_with_keys(&keys).map_err(Into::into))
            {
                Ok(event) => event,
                Err(error) => {
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                        error.to_string(),
                    ))));
                    return;
                }
            };

            let invite_event = match codec::invite_unsigned_event(&invite)
                .and_then(|unsigned| unsigned.sign_with_keys(&keys).map_err(Into::into))
            {
                Ok(event) => event,
                Err(error) => {
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                        error.to_string(),
                    ))));
                    return;
                }
            };

            if let Err(error) = publish_event_with_retry(&client, roster_event, "roster").await {
                let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(format!(
                    "Roster publish failed: {error}"
                )))));
            }
            if let Err(error) = publish_event_with_retry(&client, invite_event, "invite").await {
                let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(format!(
                    "Invite publish failed: {error}"
                )))));
            }

            let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::SyncComplete)));
        });
    }

    fn republish_local_identity_artifacts(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        let Some(account) = self.state.account.as_ref() else {
            return;
        };

        let now = unix_now();
        let owner = local_owner_from_keys(&logged_in.keys);
        let roster = DeviceRoster::new(now, vec![AuthorizedDevice::new(owner.as_device(), now)]);

        self.publish_local_identity_artifacts(
            logged_in.client.clone(),
            logged_in.keys.clone(),
            owner,
            roster,
            account.invite_url.clone(),
        );
    }

    fn resubscribe_to_protocol_events(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        let filters = self.protocol_filters();
        let client = logged_in.client.clone();
        self.runtime.spawn(async move {
            client.unsubscribe_all().await;
            client.connect_with_timeout(Duration::from_secs(5)).await;
            if !filters.is_empty() {
                let _ = client.subscribe(filters, None).await;
            }
        });
    }

    fn protocol_filters(&self) -> Vec<Filter> {
        let mut filters = Vec::new();

        let peer_authors = self
            .known_peer_owner_hexes()
            .into_iter()
            .filter_map(|hex| PublicKey::parse(&hex).ok())
            .collect::<Vec<_>>();
        if !peer_authors.is_empty() {
            filters.push(
                Filter::new()
                    .kind(Kind::from(codec::ROSTER_EVENT_KIND as u16))
                    .authors(peer_authors),
            );
        }

        let invite_response_filter = self
            .logged_in
            .as_ref()
            .and_then(|logged_in| logged_in.session_manager.snapshot().local_invite)
            .and_then(|invite| {
                PublicKey::parse(invite.inviter_ephemeral_public_key.to_string()).ok()
            })
            .map(|recipient| {
                Filter::new()
                    .kind(Kind::from(codec::INVITE_RESPONSE_KIND as u16))
                    .pubkey(recipient)
            })
            .unwrap_or_else(|| Filter::new().kind(Kind::from(codec::INVITE_RESPONSE_KIND as u16)));
        filters.push(invite_response_filter);

        let message_authors = self
            .known_message_author_hexes()
            .into_iter()
            .filter_map(|hex| PublicKey::parse(&hex).ok())
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

    fn known_peer_owner_hexes(&self) -> HashSet<String> {
        self.protocol_owner_hexes()
    }

    fn known_message_author_hexes(&self) -> HashSet<String> {
        let mut authors = HashSet::new();
        if let Some(logged_in) = self.logged_in.as_ref() {
            let selected_owners = self.protocol_owner_hexes();
            for user in logged_in
                .session_manager
                .snapshot()
                .users
                .into_iter()
                .filter(|user| selected_owners.contains(&user.owner_pubkey.to_string()))
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

fn local_owner_from_keys(keys: &Keys) -> OwnerPubkey {
    OwnerPubkey::from_bytes(keys.public_key().to_bytes())
}

fn owner_npub(peer_hex: &str) -> Option<String> {
    PublicKey::parse(peer_hex).ok()?.to_bech32().ok()
}

fn normalize_invite_from_url(url: &str) -> anyhow::Result<nostr_double_ratchet::Invite> {
    Ok(codec::parse_invite_url(url)?)
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
    event: Event,
    label: &str,
) -> anyhow::Result<()> {
    let mut last_error = "no relays available".to_string();

    for attempt in 0..5 {
        client.connect_with_timeout(Duration::from_secs(8)).await;
        wait_for_connected_relays(client, Duration::from_secs(4)).await;

        let relays = client.relays().await;
        let mut connected = 0usize;
        let mut accepted = 0usize;
        let mut failures: Vec<String> = Vec::new();

        for (url, relay) in relays {
            if !relay.is_connected() {
                failures.push(format!("{url}=status:{}", relay.status()));
                continue;
            }

            connected += 1;
            match relay.send_event(event.clone()).await {
                Ok(_) => accepted += 1,
                Err(error) => failures.push(format!("{url}={error}")),
            }
        }

        if accepted > 0 {
            return Ok(());
        }

        last_error = if connected == 0 {
            format!(
                "no connected relays ({})",
                relay_status_summary(client).await
            )
        } else if failures.is_empty() {
            "connected relays did not accept event".to_string()
        } else {
            failures.join("; ")
        };

        if attempt < 4 {
            sleep(Duration::from_millis(750 * (attempt + 1) as u64)).await;
        }
    }

    Err(anyhow::anyhow!("{label}: {last_error}"))
}

async fn publish_events_with_retry(
    client: &Client,
    events: Vec<Event>,
    label: &str,
) -> anyhow::Result<()> {
    for event in events {
        publish_event_with_retry(client, event, label).await?;
    }
    Ok(())
}

async fn wait_for_connected_relays(client: &Client, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let relays = client.relays().await;
        if relays.values().any(|relay| relay.is_connected()) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(250)).await;
    }
}

async fn relay_status_summary(client: &Client) -> String {
    let relays = client.relays().await;
    if relays.is_empty() {
        return "no relays added".to_string();
    }

    let mut states: Vec<String> = relays
        .into_iter()
        .map(|(url, relay)| format!("{url}={}", relay.status()))
        .collect();
    states.sort();
    states.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
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
            let previous = std::env::var("NDR_DEMO_RELAYS").ok();
            std::env::set_var("NDR_DEMO_RELAYS", "ws://127.0.0.1:4848");
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
                tag_values.iter().skip(1).filter_map(Value::as_str).any(|tag_value| {
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
            nsec: nsec_for_fill(secret_fill),
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
        let bytes = fs::read(data_dir.join("ndr_demo_core_state.json"))
            .expect("read persisted state");
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

    fn keys_for_fill(secret_fill: u8) -> Keys {
        Keys::new(SecretKey::from_slice(&[secret_fill; 32]).expect("secret key"))
    }

    fn publish_local_relay_event(keys: &Keys, event: Event) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("publish runtime");
        runtime.block_on(async {
            let client = Client::new(keys.clone());
            client
                .add_relay("ws://127.0.0.1:4848")
                .await
                .expect("add relay");
            client.connect_with_timeout(Duration::from_secs(5)).await;
            publish_event_with_retry(&client, event, "test publish")
                .await
                .expect("publish event");
            let _ = client.shutdown().await;
        });
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
        let keys = Keys::new(SecretKey::from_slice(&[21; 32]).expect("secret key"));

        let mut seeded = test_core(data_dir.path());
        seeded
            .start_session(keys.clone(), false)
            .expect("start session");
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
        restored
            .start_session(keys.clone(), true)
            .expect("restore session");
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
        fresh.start_session(keys, false).expect("fresh session");
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
        let keys = Keys::new(SecretKey::from_slice(&[31; 32]).expect("secret key"));
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
        core.start_session(keys, true)
            .expect("restore legacy state");

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
        let chat_id = parse_peer_input(&npub_for_fill(41))
            .expect("peer chat id")
            .0;

        core.state.account = Some(AccountSnapshot {
            public_key_hex: parse_peer_input(&npub_for_fill(40)).expect("account hex").0,
            npub: npub_for_fill(40),
            invite_url: "https://chat.iris.to".to_string(),
        });
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
            state.chat_list.iter().any(|chat| {
                chat.last_message_preview.as_deref() == Some("waiting on roster")
            })
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

        let bob_keys = keys_for_fill(92);
        let bob_owner = local_owner_from_keys(&bob_keys);
        let now = unix_now();
        let bob_roster = DeviceRoster::new(
            now,
            vec![AuthorizedDevice::new(bob_owner.as_device(), now)],
        );
        let roster_event = codec::roster_unsigned_event(bob_owner, &bob_roster)
            .expect("bob roster event")
            .sign_with_keys(&bob_keys)
            .expect("sign bob roster");
        publish_local_relay_event(&bob_keys, roster_event);

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
            SessionManager::new(bob_owner, bob_keys.secret_key().to_secret_bytes());
        session_manager.apply_local_roster(bob_roster);
        let mut rng = OsRng;
        let mut ctx = ProtocolContext::new(unix_now(), &mut rng);
        let invite = session_manager
            .ensure_local_invite(&mut ctx)
            .expect("ensure bob invite")
            .clone();
        let invite_event = codec::invite_unsigned_event(&invite)
            .expect("bob invite event")
            .sign_with_keys(&bob_keys)
            .expect("sign bob invite");
        publish_local_relay_event(&bob_keys, invite_event);

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
}
