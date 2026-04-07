use crate::actions::AppAction;
use crate::state::{
    AccountSnapshot, AppState, ChatMessageSnapshot, ChatThreadSnapshot, CurrentChatSnapshot,
    DeliveryState, Router, Screen,
};
use crate::updates::{AppUpdate, CoreMsg, InternalEvent};
use flume::Sender;
use nostr_double_ratchet::{
    AuthorizedDevice, DeviceRoster, DomainError, Error, MessageEnvelope, OwnerPubkey,
    ProtocolContext, SessionManager, SessionManagerSnapshot, UnixSeconds,
};
use nostr_double_ratchet_nostr::nostr as codec;
use nostr_sdk::prelude::{Client, Event, Filter, Keys, Kind, PublicKey, RelayPoolNotification, ToBech32};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

const RELAYS: &[&str] = &[
    "ws://127.0.0.1:4848",
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
];

pub struct AppCore {
    update_tx: Sender<AppUpdate>,
    core_sender: Sender<CoreMsg>,
    shared_state: Arc<RwLock<AppState>>,
    runtime: tokio::runtime::Runtime,
    data_dir: PathBuf,
    state: AppState,
    logged_in: Option<LoggedInState>,
    threads: BTreeMap<String, ThreadRecord>,
    active_peer_hex: Option<String>,
    next_message_id: u64,
    pending_inbound: Vec<PendingInbound>,
}

struct LoggedInState {
    keys: Keys,
    client: Client,
    session_manager: SessionManager,
}

#[derive(Clone)]
struct ThreadRecord {
    peer_hex: String,
    unread_count: u64,
    messages: Vec<ChatMessageSnapshot>,
}

struct PendingInbound {
    sender_owner_hex: String,
    envelope: MessageEnvelope,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    version: u32,
    active_peer_hex: Option<String>,
    next_message_id: u64,
    session_manager: Option<SessionManagerSnapshot>,
    threads: Vec<PersistedThread>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedThread {
    peer_hex: String,
    unread_count: u64,
    messages: Vec<PersistedMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedMessage {
    id: String,
    peer_input: String,
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
            active_peer_hex: None,
            next_message_id: 1,
            pending_inbound: Vec::new(),
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
            AppAction::OpenChat { peer_input } => self.open_chat(&peer_input),
            AppAction::CloseChat => {
                self.active_peer_hex = None;
                self.rebuild_state();
                self.persist_best_effort();
                self.resubscribe_for_active_peer();
                self.emit_state();
            }
            AppAction::SendMessage { peer_input, text } => self.send_message(&peer_input, &text),
        }
    }

    fn handle_internal(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::RelayEvent(event) => {
                self.handle_relay_event(event);
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

        if let Err(error) = self.start_session(keys, true) {
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
            .and_then(|keys| self.start_session(keys, false));

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
        self.active_peer_hex = None;
        self.pending_inbound.clear();
        self.next_message_id = 1;
        self.state = AppState::empty();
        self.clear_persistence_best_effort();
        self.emit_state();
    }

    fn open_chat(&mut self, peer_input: &str) {
        let Ok((peer_hex, _pubkey)) = normalize_peer_input(peer_input) else {
            self.state.toast = Some("Invalid peer key.".to_string());
            self.emit_state();
            return;
        };

        self.threads
            .entry(peer_hex.clone())
            .or_insert_with(|| ThreadRecord {
                peer_hex: peer_hex.clone(),
                unread_count: 0,
                messages: Vec::new(),
            })
            .unread_count = 0;

        self.active_peer_hex = Some(peer_hex);
        self.republish_local_identity_artifacts();
        self.rebuild_state();
        self.persist_best_effort();
        self.resubscribe_for_active_peer();
        self.emit_state();
    }

    fn send_message(&mut self, peer_input: &str, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        let Ok((peer_hex, peer_pubkey)) = normalize_peer_input(peer_input) else {
            self.state.toast = Some("Invalid peer key.".to_string());
            self.emit_state();
            return;
        };

        if self.logged_in.is_none() {
            self.state.toast = Some("Create or restore an account first.".to_string());
            self.emit_state();
            return;
        }

        self.active_peer_hex = Some(peer_hex.clone());
        self.threads.entry(peer_hex.clone()).or_insert_with(|| ThreadRecord {
            peer_hex: peer_hex.clone(),
            unread_count: 0,
            messages: Vec::new(),
        });
        self.state.busy.sending_message = true;
        self.rebuild_state();
        self.emit_state();

        let owner = OwnerPubkey::from_bytes(peer_pubkey.to_bytes());
        let now = unix_now();
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
                if prepared.deliveries.is_empty() && prepared.invite_responses.is_empty() {
                    self.state.toast =
                        Some("Waiting for the peer roster and invite on relays.".to_string());
                } else {
                    let message = self.push_outgoing_message(&peer_hex, trimmed.to_string(), now.get());
                    let events = {
                        let mut out = Vec::new();
                        for response in &prepared.invite_responses {
                            match codec::invite_response_event(response) {
                                Ok(event) => out.push(event),
                                Err(error) => {
                                    self.state.toast = Some(error.to_string());
                                }
                            }
                        }
                        for delivery in &prepared.deliveries {
                            match codec::message_event(&delivery.envelope) {
                                Ok(event) => out.push(event),
                                Err(error) => {
                                    self.state.toast = Some(error.to_string());
                                }
                            }
                        }
                        out
                    };
                    if !events.is_empty() {
                        let client = self
                            .logged_in
                            .as_ref()
                            .expect("logged in checked above")
                            .client
                            .clone();
                        let tx = self.core_sender.clone();
                        self.runtime.spawn(async move {
                            for relay in RELAYS {
                                let _ = client.add_relay(*relay).await;
                            }
                            client.connect_with_timeout(Duration::from_secs(5)).await;
                            for event in events {
                                if let Err(error) =
                                    publish_event_with_retry(&client, event, "message").await
                                {
                                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::Toast(
                                        format!("Publish failed: {error}"),
                                    ))));
                                }
                            }
                        });
                    } else if let Some(thread) = self.threads.get_mut(&peer_hex) {
                        if let Some(last) = thread.messages.last_mut() {
                            if last.id == message.id {
                                last.delivery = DeliveryState::Failed;
                            }
                        }
                    }
                }
            }
            Err(error) => {
                self.state.toast = Some(error.to_string());
            }
        }

        self.state.busy.sending_message = false;
        self.rebuild_state();
        self.persist_best_effort();
        self.emit_state();
    }

    fn handle_relay_event(&mut self, event: Event) {
        let Some(logged_in) = self.logged_in.as_mut() else {
            return;
        };

        let kind = event.kind.as_u16() as u32;
        let now = unix_now();
        match kind {
            codec::ROSTER_EVENT_KIND => {
                if let Ok(decoded) = codec::parse_roster_event(&event) {
                    if decoded.owner_pubkey != local_owner_from_keys(&logged_in.keys) {
                        logged_in
                            .session_manager
                            .observe_peer_roster(decoded.owner_pubkey, decoded.roster);
                        self.retry_pending_inbound(now);
                        self.persist_best_effort();
                        self.rebuild_state();
                        self.emit_state();
                        return;
                    }
                }

                if let Ok(invite) = codec::parse_invite_event(&event) {
                    let owner = invite.owner_public_key.unwrap_or(invite.inviter);
                    if owner != local_owner_from_keys(&logged_in.keys) {
                        if let Err(error) = logged_in.session_manager.observe_device_invite(owner, invite) {
                            self.state.toast = Some(error.to_string());
                        } else {
                            self.retry_pending_inbound(now);
                            self.persist_best_effort();
                        }
                        self.rebuild_state();
                        self.emit_state();
                    }
                }
            }
            codec::INVITE_RESPONSE_KIND => {
                let Some(local_invite_recipient) = logged_in
                    .session_manager
                    .snapshot()
                    .local_invite
                    .as_ref()
                    .map(|invite| invite.inviter_ephemeral_public_key)
                else {
                    return;
                };

                let Ok(envelope) = codec::parse_invite_response_event(&event) else {
                    return;
                };
                if envelope.recipient != local_invite_recipient {
                    return;
                }

                let mut rng = OsRng;
                let mut ctx = ProtocolContext::new(now, &mut rng);
                if let Err(error) = logged_in
                    .session_manager
                    .observe_invite_response(&mut ctx, &envelope)
                {
                    let should_ignore = matches!(
                        error,
                        Error::Domain(DomainError::InviteAlreadyUsed)
                            | Error::Domain(DomainError::InviteExhausted)
                    );
                    if !should_ignore {
                        self.state.toast = Some(error.to_string());
                    }
                } else {
                    self.retry_pending_inbound(now);
                    self.persist_best_effort();
                }
                self.rebuild_state();
                self.emit_state();
            }
            codec::MESSAGE_EVENT_KIND => {
                if let Ok(envelope) = codec::parse_message_event(&event) {
                    let sender_owner = envelope.sender.as_owner();
                    let mut rng = OsRng;
                    let mut ctx = ProtocolContext::new(now, &mut rng);
                    match logged_in.session_manager.receive(&mut ctx, sender_owner, &envelope) {
                        Ok(Some(message)) => {
                            self.push_incoming_message(
                                &message.owner_pubkey.to_string(),
                                String::from_utf8_lossy(&message.payload).into_owned(),
                                now.get(),
                            );
                            self.persist_best_effort();
                            self.rebuild_state();
                            self.emit_state();
                        }
                        Ok(None) => {
                            self.pending_inbound.push(PendingInbound {
                                sender_owner_hex: sender_owner.to_string(),
                                envelope,
                            });
                        }
                        Err(error) => {
                            self.state.toast = Some(error.to_string());
                            self.emit_state();
                        }
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
        self.active_peer_hex = None;
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
            self.active_peer_hex = persisted.active_peer_hex.clone();
            self.next_message_id = persisted.next_message_id.max(1);
            self.threads = persisted
                .threads
                .iter()
                .map(|thread| {
                    (
                        thread.peer_hex.clone(),
                        ThreadRecord {
                            peer_hex: thread.peer_hex.clone(),
                            unread_count: thread.unread_count,
                            messages: thread
                                .messages
                                .iter()
                                .map(|message| ChatMessageSnapshot {
                                    id: message.id.clone(),
                                    peer_input: message.peer_input.clone(),
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
            .and_then(|persisted| persisted.session_manager)
            .map(|snapshot| SessionManager::from_snapshot(snapshot, secret_bytes))
            .transpose()?
            .unwrap_or_else(|| {
                SessionManager::new(local_owner, secret_bytes)
            });

        let local_roster = DeviceRoster::new(
            now,
            vec![AuthorizedDevice::new(local_device, now)],
        );
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
        self.resubscribe_for_active_peer();
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
            let sender_owner = match normalize_peer_input(&item.sender_owner_hex) {
                Ok((_, sender_pubkey)) => OwnerPubkey::from_bytes(sender_pubkey.to_bytes()),
                Err(_) => {
                    still_pending.push(item);
                    continue;
                }
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

    fn push_outgoing_message(
        &mut self,
        peer_hex: &str,
        body: String,
        created_at_secs: u64,
    ) -> ChatMessageSnapshot {
        let message = ChatMessageSnapshot {
            id: self.allocate_message_id(),
            peer_input: peer_hex.to_string(),
            author: self
                .state
                .account
                .as_ref()
                .map(|account| account.npub.clone())
                .unwrap_or_else(|| "me".to_string()),
            body,
            is_outgoing: true,
            created_at_secs,
            delivery: DeliveryState::Sent,
        };
        self.threads
            .entry(peer_hex.to_string())
            .or_insert_with(|| ThreadRecord {
                peer_hex: peer_hex.to_string(),
                unread_count: 0,
                messages: Vec::new(),
            })
            .messages
            .push(message.clone());
        message
    }

    fn push_incoming_message(&mut self, peer_hex: &str, body: String, created_at_secs: u64) {
        let message_id = self.allocate_message_id();
        let author = owner_npub(peer_hex).unwrap_or_else(|| peer_hex.to_string());
        let thread = self
            .threads
            .entry(peer_hex.to_string())
            .or_insert_with(|| ThreadRecord {
                peer_hex: peer_hex.to_string(),
                unread_count: 0,
                messages: Vec::new(),
            });
        if self.active_peer_hex.as_deref() != Some(peer_hex) {
            thread.unread_count = thread.unread_count.saturating_add(1);
        }
        thread.messages.push(ChatMessageSnapshot {
            id: message_id,
            peer_input: peer_hex.to_string(),
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
            Screen::Account
        } else {
            Screen::Welcome
        };

        let mut threads: Vec<&ThreadRecord> = self.threads.values().collect();
        threads.sort_by_key(|thread| {
            thread
                .messages
                .last()
                .map(|message| std::cmp::Reverse(message.created_at_secs))
                .unwrap_or(std::cmp::Reverse(0))
        });

        self.state.chat_list = threads
            .iter()
            .map(|thread| ChatThreadSnapshot {
                peer_input: owner_npub(&thread.peer_hex).unwrap_or_else(|| thread.peer_hex.clone()),
                title: owner_npub(&thread.peer_hex).unwrap_or_else(|| thread.peer_hex.clone()),
                last_message: thread.messages.last().map(|message| message.body.clone()),
                unread_count: thread.unread_count,
            })
            .collect();

        self.state.current_chat = self
            .active_peer_hex
            .as_ref()
            .and_then(|peer_hex| self.threads.get(peer_hex))
            .map(|thread| CurrentChatSnapshot {
                peer_input: owner_npub(&thread.peer_hex).unwrap_or_else(|| thread.peer_hex.clone()),
                title: owner_npub(&thread.peer_hex).unwrap_or_else(|| thread.peer_hex.clone()),
                messages: thread.messages.clone(),
            });

        self.state.router = Router {
            default_screen,
            screen_stack: if self.state.current_chat.is_some() {
                vec![Screen::Chat]
            } else {
                Vec::new()
            },
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
            version: 1,
            active_peer_hex: self.active_peer_hex.clone(),
            next_message_id: self.next_message_id,
            session_manager: Some(logged_in.session_manager.snapshot()),
            threads: self
                .threads
                .values()
                .map(|thread| PersistedThread {
                    peer_hex: thread.peer_hex.clone(),
                    unread_count: thread.unread_count,
                    messages: thread
                        .messages
                        .iter()
                        .map(|message| PersistedMessage {
                            id: message.id.clone(),
                            peer_input: message.peer_input.clone(),
                            author: message.author.clone(),
                            body: message.body.clone(),
                            is_outgoing: message.is_outgoing,
                            created_at_secs: message.created_at_secs,
                            delivery: (&message.delivery).into(),
                        })
                        .collect(),
                })
                .collect(),
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
                    .send(CoreMsg::Internal(Box::new(InternalEvent::Toast(error.to_string()))));
                return;
            }
        };
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            for relay in RELAYS {
                let _ = client.add_relay(*relay).await;
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

    fn resubscribe_for_active_peer(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        let client = logged_in.client.clone();
        let peer_hex = self.active_peer_hex.clone();
        self.runtime.spawn(async move {
            client.unsubscribe_all().await;
            let Some(peer_hex) = peer_hex else {
                return;
            };
            let Ok(peer_pubkey) = PublicKey::parse(&peer_hex) else {
                return;
            };

            let roster_or_invite = Filter::new()
                .author(peer_pubkey)
                .kind(Kind::from(codec::ROSTER_EVENT_KIND as u16));
            // Invite responses are authored by a random one-time sender key, so
            // filtering by the peer owner pubkey would drop the bootstrap event.
            let invite_responses = Filter::new().kind(Kind::from(codec::INVITE_RESPONSE_KIND as u16));
            let messages = Filter::new()
                .author(peer_pubkey)
                .kind(Kind::from(codec::MESSAGE_EVENT_KIND as u16));

            client.connect_with_timeout(Duration::from_secs(5)).await;
            let _ = client.subscribe(vec![roster_or_invite], None).await;
            let _ = client.subscribe(vec![invite_responses], None).await;
            let _ = client.subscribe(vec![messages], None).await;
        });
    }
}

fn normalize_peer_input(input: &str) -> anyhow::Result<(String, PublicKey)> {
    let mut normalized = input.trim().to_ascii_lowercase();
    if let Some(stripped) = normalized.strip_prefix("nostr:") {
        normalized = stripped.to_string();
    }
    let pubkey = PublicKey::parse(&normalized)?;
    Ok((pubkey.to_hex(), pubkey))
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

async fn publish_event_with_retry(client: &Client, event: Event, label: &str) -> anyhow::Result<()> {
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
            format!("no connected relays ({})", relay_status_summary(client).await)
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
