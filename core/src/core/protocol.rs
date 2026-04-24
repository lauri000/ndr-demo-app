use super::*;

impl AppCore {
    pub(super) fn prune_recent_handshake_peers(&mut self, now_secs: u64) {
        self.reconcile_recent_handshake_peers();
        self.recent_handshake_peers.retain(|_, peer| {
            let within_ttl =
                now_secs.saturating_sub(peer.observed_at_secs) <= RECENT_HANDSHAKE_TTL_SECS;
            within_ttl && !self.threads.contains_key(&peer.owner_hex)
        });
    }

    pub(super) fn remember_recent_handshake_peer(
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

    pub(super) fn clear_recent_handshake_peer(&mut self, owner_hex: &str) {
        self.recent_handshake_peers
            .retain(|_, peer| peer.owner_hex != owner_hex);
    }

    pub(super) fn tracked_peer_owner_hexes(&self) -> HashSet<String> {
        let mut owners = self
            .threads
            .keys()
            .filter(|chat_id| !is_group_chat_id(chat_id))
            .cloned()
            .collect::<HashSet<_>>();
        if let Some(chat_id) = self.active_chat_id.as_ref() {
            if !is_group_chat_id(chat_id) {
                owners.insert(chat_id.clone());
            }
        }
        for pending in &self.pending_outbound {
            if !is_group_chat_id(&pending.chat_id) {
                owners.insert(pending.chat_id.clone());
            }
        }
        for pending in &self.pending_group_controls {
            owners.extend(pending.target_owner_hexes.iter().cloned());
        }
        for pending in &self.pending_inbound {
            if let PendingInbound::Decrypted {
                sender_owner_hex, ..
            } = pending
            {
                owners.insert(sender_owner_hex.clone());
            }
        }
        if let Some(logged_in) = self.logged_in.as_ref() {
            for group in logged_in.group_manager.groups() {
                for member in group.members {
                    if member != logged_in.owner_pubkey {
                        owners.insert(member.to_string());
                    }
                }
            }
        }
        owners
    }

    pub(super) fn protocol_owner_hexes(&self) -> HashSet<String> {
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

    pub(super) fn schedule_pending_outbound_retry(&self, after: Duration) {
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            sleep(after).await;
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::RetryPendingOutbound,
            )));
        });
    }

    pub(super) fn schedule_tracked_peer_catch_up(&self, after: Duration) {
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            sleep(after).await;
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::FetchTrackedPeerCatchUp,
            )));
        });
    }

    pub(super) fn schedule_pending_device_invite_poll(&mut self, after: Duration) {
        if !self.can_poll_pending_device_invites() {
            return;
        }
        self.device_invite_poll_token = self.device_invite_poll_token.saturating_add(1);
        let token = self.device_invite_poll_token;
        let tx = self.core_sender.clone();
        self.runtime.spawn(async move {
            sleep(after).await;
            let _ = tx.send(CoreMsg::Internal(Box::new(
                InternalEvent::PollPendingDeviceInvites { token },
            )));
        });
    }

    pub(super) fn schedule_next_pending_retry(&self, now_secs: u64) {
        let next_retry_at_secs = self
            .pending_outbound
            .iter()
            .map(|pending| pending.next_retry_at_secs)
            .chain(
                self.pending_group_controls
                    .iter()
                    .map(|pending| pending.next_retry_at_secs),
            )
            .min();
        let Some(next_retry_at_secs) = next_retry_at_secs else {
            return;
        };
        let delay_secs = next_retry_at_secs.saturating_sub(now_secs).max(1);
        self.schedule_pending_outbound_retry(Duration::from_secs(delay_secs));
    }

    pub(super) fn fetch_recent_messages_for_owner(
        &self,
        owner_pubkey: OwnerPubkey,
        now: UnixSeconds,
    ) {
        self.fetch_recent_messages_for_owner_with_lookback(
            owner_pubkey,
            now,
            CATCH_UP_LOOKBACK_SECS,
        );
    }

    pub(super) fn fetch_recent_messages_for_owner_with_lookback(
        &self,
        owner_pubkey: OwnerPubkey,
        now: UnixSeconds,
        lookback_secs: u64,
    ) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            return;
        };

        let filters = self.message_filters_for_owner(owner_pubkey, now, lookback_secs);
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

    pub(super) fn fetch_recent_protocol_state(&mut self) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            return;
        };

        let Some(plan) = self.compute_protocol_subscription_plan() else {
            return;
        };
        self.push_debug_log(
            "protocol.catch_up.fetch",
            summarize_protocol_plan(Some(&plan)),
        );

        let filters = build_protocol_state_catch_up_filters(&plan, unix_now());
        if filters.is_empty() {
            return;
        }

        let tx = self.core_sender.clone();
        let plan_summary = summarize_protocol_plan(Some(&plan));
        self.runtime.spawn(async move {
            client.connect_with_timeout(Duration::from_secs(5)).await;
            match client
                .fetch_events(filters, Some(Duration::from_secs(5)))
                .await
            {
                Ok(events) => {
                    let collected = events.into_iter().collect::<Vec<_>>();
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::DebugLog {
                        category: "protocol.catch_up.result".to_string(),
                        detail: format!("{} events={}", plan_summary, collected.len(),),
                    })));
                    if !collected.is_empty() {
                        let _ = tx.send(CoreMsg::Internal(Box::new(
                            InternalEvent::FetchCatchUpEvents(collected),
                        )));
                    }
                }
                Err(error) => {
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::DebugLog {
                        category: "protocol.catch_up.error".to_string(),
                        detail: format!("{plan_summary} error={error}"),
                    })));
                }
            }
        });
    }

    pub(super) fn fetch_pending_device_invites_for_local_owner(&mut self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        if logged_in.owner_keys.is_none() {
            return;
        }

        let owner_pubkey = logged_in.owner_pubkey;
        let device_keys = logged_in.device_keys.clone();
        let relay_urls = logged_in.relay_urls.clone();
        let since = unix_now()
            .get()
            .saturating_sub(DEVICE_INVITE_DISCOVERY_LOOKBACK_SECS);
        self.push_debug_log(
            "device.invite.fetch",
            format!(
                "owner={} since={} limit={}",
                owner_pubkey, since, DEVICE_INVITE_DISCOVERY_LIMIT
            ),
        );
        let tx = self.core_sender.clone();
        let filters = vec![Filter::new()
            .kind(Kind::from(codec::INVITE_EVENT_KIND as u16))
            .since(Timestamp::from(since))
            .limit(DEVICE_INVITE_DISCOVERY_LIMIT)];

        self.runtime.spawn(async move {
            let client = Client::new(device_keys);
            ensure_session_relays_configured(&client, &relay_urls).await;
            client.connect_with_timeout(Duration::from_secs(5)).await;
            match client
                .fetch_events(filters, Some(Duration::from_secs(5)))
                .await
            {
                Ok(events) => {
                    let collected = events.into_iter().collect::<Vec<_>>();
                    let _ = tx.send(CoreMsg::Internal(Box::new(
                        InternalEvent::FetchPendingDeviceInvites(collected),
                    )));
                }
                Err(error) => {
                    let _ = tx.send(CoreMsg::Internal(Box::new(InternalEvent::DebugLog {
                        category: "device.invite.fetch.error".to_string(),
                        detail: error.to_string(),
                    })));
                }
            }
            let _ = client.shutdown().await;
        });
    }

    pub(super) fn handle_pending_device_invite_events(&mut self, events: Vec<Event>) {
        let Some((local_owner, local_device)) = self.logged_in.as_ref().and_then(|logged_in| {
            logged_in.owner_keys.as_ref().map(|_| {
                (
                    logged_in.owner_pubkey,
                    local_device_from_keys(&logged_in.device_keys),
                )
            })
        }) else {
            return;
        };

        let mut observed = 0usize;
        let mut last_error = None;

        for event in events {
            let event_id = event.id.to_string();
            if self.has_seen_event(&event_id) {
                continue;
            }

            let Ok(invite) = codec::parse_invite_event(&event) else {
                continue;
            };
            if invite.inviter_owner_pubkey != Some(local_owner)
                || invite.inviter_device_pubkey == local_device
            {
                continue;
            }

            match self
                .logged_in
                .as_mut()
                .expect("logged-in state checked above")
                .session_manager
                .observe_device_invite(local_owner, invite)
            {
                Ok(()) => {
                    observed += 1;
                    self.remember_event(event_id);
                }
                Err(error) => {
                    last_error = Some(error.to_string());
                }
            }
        }

        if let Some(error) = last_error {
            self.push_debug_log("device.invite.observe.error", error);
        }

        self.push_debug_log(
            "device.invite.observe",
            format!("owner={} observed={}", local_owner, observed),
        );

        if observed > 0 {
            self.persist_best_effort();
            self.rebuild_state();
            self.emit_state();
        }
    }

    pub(super) fn nudge_protocol_state_for_pending_reason(&mut self, reason: &PendingSendReason) {
        self.push_debug_log("protocol.nudge", format!("reason={reason:?}"));
        match reason {
            PendingSendReason::MissingRoster => {
                self.republish_local_identity_artifacts();
                self.request_protocol_subscription_refresh();
                self.fetch_recent_protocol_state();
            }
            PendingSendReason::MissingDeviceInvite => {
                self.request_protocol_subscription_refresh();
                self.fetch_recent_protocol_state();
            }
            PendingSendReason::PublishingFirstContact | PendingSendReason::PublishRetry => {}
        }
    }

    pub(super) fn fetch_recent_messages_for_tracked_peers(&self, now: UnixSeconds) {
        for owner_hex in self.tracked_peer_owner_hexes() {
            let Ok(pubkey) = PublicKey::parse(&owner_hex) else {
                continue;
            };
            self.fetch_recent_messages_for_owner(OwnerPubkey::from_bytes(pubkey.to_bytes()), now);
        }
    }

    pub(super) fn message_filters_for_owner(
        &self,
        owner_pubkey: OwnerPubkey,
        now: UnixSeconds,
        lookback_secs: u64,
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
            .since(Timestamp::from(now.get().saturating_sub(lookback_secs)))]
    }

    pub(super) fn start_notifications_loop(&self, client: Client) {
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

    pub(super) fn schedule_session_connect(&self) {
        let Some(logged_in) = self.logged_in.as_ref() else {
            return;
        };
        let client = logged_in.client.clone();
        let relay_urls = logged_in.relay_urls.clone();
        self.runtime.spawn(async move {
            ensure_session_relays_configured(&client, &relay_urls).await;
            client
                .connect_with_timeout(Duration::from_secs(RELAY_CONNECT_TIMEOUT_SECS))
                .await;
        });
    }

    pub(super) fn request_protocol_subscription_refresh(&mut self) {
        self.request_protocol_subscription_refresh_inner(false);
    }

    pub(super) fn request_protocol_subscription_refresh_forced(&mut self) {
        self.request_protocol_subscription_refresh_inner(true);
    }

    pub(super) fn request_protocol_subscription_refresh_inner(&mut self, force: bool) {
        let Some(client) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.client.clone())
        else {
            self.protocol_subscription_runtime = ProtocolSubscriptionRuntime::default();
            return;
        };

        if self.protocol_subscription_runtime.refresh_in_flight {
            self.push_debug_log("protocol.subscription.defer", "refresh already in flight");
            self.protocol_subscription_runtime.refresh_dirty = true;
            self.protocol_subscription_runtime.force_refresh_dirty |= force;
            return;
        }

        let plan = self.compute_protocol_subscription_plan();
        self.push_debug_log(
            "protocol.subscription.compute",
            summarize_protocol_plan(plan.as_ref()),
        );
        if !force && self.protocol_subscription_runtime.current_plan == plan {
            self.push_debug_log("protocol.subscription.noop", "plan unchanged");
            return;
        }

        let subscription_id = SubscriptionId::new(PROTOCOL_SUBSCRIPTION_ID);
        self.protocol_subscription_runtime.refresh_in_flight = true;
        self.protocol_subscription_runtime.refresh_dirty = false;
        self.protocol_subscription_runtime.force_refresh_dirty = false;
        self.protocol_subscription_runtime.refresh_token = self
            .protocol_subscription_runtime
            .refresh_token
            .wrapping_add(1);
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
            client
                .connect_with_timeout(Duration::from_secs(RELAY_CONNECT_TIMEOUT_SECS))
                .await;
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

    pub(super) fn compute_protocol_subscription_plan(&self) -> Option<ProtocolSubscriptionPlan> {
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

    pub(super) fn can_poll_pending_device_invites(&self) -> bool {
        self.logged_in
            .as_ref()
            .map(|logged_in| logged_in.owner_keys.is_some())
            .unwrap_or(false)
    }

    pub(super) fn known_roster_owner_hexes(&self) -> HashSet<String> {
        let mut owners = self.protocol_owner_hexes();
        if let Some(logged_in) = self.logged_in.as_ref() {
            owners.insert(logged_in.owner_pubkey.to_string());
        }
        owners
    }

    pub(super) fn known_invite_author_hexes(&self) -> HashSet<String> {
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
                    if owner_hex == logged_in.owner_pubkey.to_string()
                        && device_hex == local_device_hex
                    {
                        continue;
                    }
                    authors.insert(device_hex);
                }
            }
        }

        authors
    }

    pub(super) fn known_message_author_hexes(&self) -> HashSet<String> {
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

    pub(super) fn has_seen_event(&self, event_id: &str) -> bool {
        self.seen_event_ids.contains(event_id)
    }

    pub(super) fn remember_event(&mut self, event_id: String) {
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
