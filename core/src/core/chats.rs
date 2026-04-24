use super::*;

impl AppCore {
    pub(super) fn create_chat(&mut self, peer_input: &str) {
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
        self.push_debug_log(
            "chat.create",
            format!("peer_input={} chat_id={chat_id}", peer_input.trim()),
        );

        let now = unix_now().get();
        self.prune_recent_handshake_peers(now);
        self.ensure_thread_record(&chat_id, now).unread_count = 0;

        self.active_chat_id = Some(chat_id.clone());
        self.screen_stack = vec![Screen::Chat { chat_id }];
        self.republish_local_identity_artifacts();
        self.rebuild_state();
        self.persist_best_effort();
        self.request_protocol_subscription_refresh();
        self.schedule_tracked_peer_catch_up(Duration::from_secs(RESUBSCRIBE_CATCH_UP_DELAY_SECS));
        self.state.busy.creating_chat = false;
        self.emit_state();
    }

    pub(super) fn ensure_thread_record(
        &mut self,
        chat_id: &str,
        updated_at_secs: u64,
    ) -> &mut ThreadRecord {
        let thread = self
            .threads
            .entry(chat_id.to_string())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.to_string(),
                unread_count: 0,
                updated_at_secs,
                messages: Vec::new(),
            });
        if thread.updated_at_secs == 0 {
            thread.updated_at_secs = updated_at_secs;
        }
        thread
    }

    pub(super) fn normalize_chat_id(&self, chat_id: &str) -> Option<String> {
        if is_group_chat_id(chat_id) {
            let group_id = parse_group_id_from_chat_id(chat_id)?;
            let group_chat_id = group_chat_id(&group_id);
            let known_group = self
                .logged_in
                .as_ref()
                .and_then(|logged_in| logged_in.group_manager.group(&group_id))
                .is_some();
            if known_group || self.threads.contains_key(&group_chat_id) {
                return Some(group_chat_id);
            }
            return None;
        }

        parse_peer_input(chat_id)
            .ok()
            .map(|(normalized, _)| normalized)
    }

    pub(super) fn open_chat(&mut self, chat_id: &str) {
        if !self.can_use_chats() {
            self.state.toast = Some(chat_unavailable_message(self.logged_in.as_ref()).to_string());
            self.emit_state();
            return;
        }

        let Some(chat_id) = self.normalize_chat_id(chat_id) else {
            self.state.toast = Some("Invalid chat id.".to_string());
            self.emit_state();
            return;
        };

        let now = unix_now().get();
        self.prune_recent_handshake_peers(now);
        self.ensure_thread_record(&chat_id, now).unread_count = 0;
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

    pub(super) fn send_message(&mut self, chat_id: &str, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

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

        let Some(normalized_chat_id) = self.normalize_chat_id(chat_id) else {
            self.state.toast = Some("Invalid chat id.".to_string());
            self.emit_state();
            return;
        };
        self.push_debug_log(
            "chat.send",
            format!(
                "chat_id={} is_group={}",
                normalized_chat_id,
                is_group_chat_id(&normalized_chat_id)
            ),
        );

        let now = unix_now();
        self.prune_recent_handshake_peers(now.get());
        self.active_chat_id = Some(normalized_chat_id.clone());
        self.screen_stack = vec![Screen::Chat {
            chat_id: normalized_chat_id.clone(),
        }];
        self.ensure_thread_record(&normalized_chat_id, now.get());
        self.state.busy.sending_message = true;
        self.rebuild_state();
        self.emit_state();

        if is_group_chat_id(&normalized_chat_id) {
            self.send_group_message(&normalized_chat_id, trimmed, now);
        } else {
            self.send_direct_message(&normalized_chat_id, trimmed, now);
        }

        self.schedule_next_pending_retry(now.get());
        self.state.busy.sending_message = false;
        self.rebuild_state();
        self.persist_best_effort();
        self.emit_state();
    }

    pub(super) fn send_direct_message(&mut self, chat_id: &str, text: &str, now: UnixSeconds) {
        let Ok((normalized_chat_id, peer_pubkey)) = parse_peer_input(chat_id) else {
            self.state.toast = Some("Invalid peer key.".to_string());
            return;
        };

        let payload = match encode_app_direct_message_payload(&normalized_chat_id, text) {
            Ok(payload) => payload,
            Err(error) => {
                self.state.toast = Some(error.to_string());
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

        self.handle_prepared_direct_send(&normalized_chat_id, text, now, prepared);
    }

    pub(super) fn send_group_message(&mut self, chat_id: &str, text: &str, now: UnixSeconds) {
        let Some(group_id) = parse_group_id_from_chat_id(chat_id) else {
            self.state.toast = Some("Invalid group id.".to_string());
            return;
        };
        let payload = match encode_app_group_message_payload(text) {
            Ok(payload) => payload,
            Err(error) => {
                self.state.toast = Some(error.to_string());
                return;
            }
        };

        let prepared = {
            let logged_in = self.logged_in.as_mut().expect("logged in checked above");
            let mut rng = OsRng;
            let mut ctx = ProtocolContext::new(now, &mut rng);
            let (session_manager, group_manager) =
                (&mut logged_in.session_manager, &mut logged_in.group_manager);
            group_manager.send_message(session_manager, &mut ctx, &group_id, payload)
        };

        match prepared {
            Ok(prepared) => {
                self.publish_group_local_sibling_best_effort(&prepared);
                if let Some(reason) = pending_reason_from_group_prepared(&prepared) {
                    self.push_debug_log(
                        "group.send.pending",
                        format!(
                            "chat_id={} reason={reason:?} gaps={}",
                            chat_id,
                            summarize_relay_gaps(&prepared.remote.relay_gaps)
                        ),
                    );
                    let pending_reason = reason.clone();
                    let message = self.push_outgoing_message(
                        chat_id,
                        text.to_string(),
                        now.get(),
                        DeliveryState::Pending,
                    );
                    self.queue_pending_outbound(
                        message.id,
                        chat_id.to_string(),
                        text.to_string(),
                        None,
                        OutboundPublishMode::WaitForPeer,
                        pending_reason.clone(),
                        now.get().saturating_add(PENDING_RETRY_DELAY_SECS),
                    );
                    self.nudge_protocol_state_for_pending_reason(&pending_reason);
                    self.request_protocol_subscription_refresh();
                    self.schedule_pending_outbound_retry(Duration::from_secs(
                        PENDING_RETRY_DELAY_SECS,
                    ));
                } else {
                    match build_group_prepared_publish_batch(&prepared) {
                        Ok(Some(batch)) => {
                            let publish_mode = publish_mode_for_batch(&batch);
                            let message = self.push_outgoing_message(
                                chat_id,
                                text.to_string(),
                                now.get(),
                                DeliveryState::Pending,
                            );
                            self.queue_pending_outbound(
                                message.id.clone(),
                                chat_id.to_string(),
                                text.to_string(),
                                Some(batch.clone()),
                                publish_mode.clone(),
                                pending_reason_for_publish_mode(&publish_mode),
                                retry_deadline_for_publish_mode(now.get(), &publish_mode),
                            );
                            self.set_pending_outbound_in_flight(&message.id, true);
                            self.start_publish_for_pending(
                                message.id,
                                chat_id.to_string(),
                                publish_mode,
                                batch,
                            );
                        }
                        Ok(None) => {
                            let message = self.push_outgoing_message(
                                chat_id,
                                text.to_string(),
                                now.get(),
                                DeliveryState::Failed,
                            );
                            self.update_message_delivery(
                                chat_id,
                                &message.id,
                                DeliveryState::Failed,
                            );
                        }
                        Err(error) => self.state.toast = Some(error.to_string()),
                    }
                }
            }
            Err(error) => {
                self.state.toast = Some(error.to_string());
            }
        }
    }

    pub(super) fn handle_prepared_direct_send(
        &mut self,
        chat_id: &str,
        text: &str,
        now: UnixSeconds,
        prepared: Result<nostr_double_ratchet::PreparedSend, Error>,
    ) {
        match prepared {
            Ok(prepared) => {
                if let Some(reason) = pending_reason_from_prepared(&prepared) {
                    self.push_debug_log(
                        "direct.send.pending",
                        format!(
                            "chat_id={} reason={reason:?} gaps={}",
                            chat_id,
                            summarize_relay_gaps(&prepared.relay_gaps)
                        ),
                    );
                    let pending_reason = reason.clone();
                    let message = self.push_outgoing_message(
                        chat_id,
                        text.to_string(),
                        now.get(),
                        DeliveryState::Pending,
                    );
                    self.queue_pending_outbound(
                        message.id,
                        chat_id.to_string(),
                        text.to_string(),
                        None,
                        OutboundPublishMode::WaitForPeer,
                        pending_reason.clone(),
                        now.get().saturating_add(PENDING_RETRY_DELAY_SECS),
                    );
                    self.nudge_protocol_state_for_pending_reason(&pending_reason);
                    self.request_protocol_subscription_refresh();
                    self.schedule_pending_outbound_retry(Duration::from_secs(
                        PENDING_RETRY_DELAY_SECS,
                    ));
                } else {
                    match build_prepared_publish_batch(&prepared) {
                        Ok(Some(batch)) => {
                            let publish_mode = publish_mode_for_batch(&batch);
                            let message = self.push_outgoing_message(
                                chat_id,
                                text.to_string(),
                                now.get(),
                                DeliveryState::Pending,
                            );
                            self.queue_pending_outbound(
                                message.id.clone(),
                                chat_id.to_string(),
                                text.to_string(),
                                Some(batch.clone()),
                                publish_mode.clone(),
                                pending_reason_for_publish_mode(&publish_mode),
                                retry_deadline_for_publish_mode(now.get(), &publish_mode),
                            );
                            self.set_pending_outbound_in_flight(&message.id, true);
                            self.start_publish_for_pending(
                                message.id,
                                chat_id.to_string(),
                                publish_mode,
                                batch,
                            );
                        }
                        Ok(None) => {
                            let message = self.push_outgoing_message(
                                chat_id,
                                text.to_string(),
                                now.get(),
                                DeliveryState::Failed,
                            );
                            self.update_message_delivery(
                                chat_id,
                                &message.id,
                                DeliveryState::Failed,
                            );
                        }
                        Err(error) => self.state.toast = Some(error.to_string()),
                    }
                }
            }
            Err(error) => {
                self.state.toast = Some(error.to_string());
            }
        }
    }

    pub(super) fn retry_pending_inbound(&mut self, now: UnixSeconds) {
        if self.logged_in.is_none() {
            return;
        }

        let mut pending = std::mem::take(&mut self.pending_inbound);
        loop {
            let mut still_pending = Vec::new();
            let mut made_progress = false;

            for item in pending {
                if let PendingInbound::Decrypted {
                    sender_owner_hex,
                    payload,
                    created_at_secs,
                } = item.clone()
                {
                    let Ok(sender_pubkey) = PublicKey::parse(&sender_owner_hex) else {
                        still_pending.push(item);
                        continue;
                    };
                    match self.apply_decrypted_payload(
                        OwnerPubkey::from_bytes(sender_pubkey.to_bytes()),
                        &payload,
                        created_at_secs,
                    ) {
                        Ok(()) => {
                            made_progress = true;
                        }
                        Err(error) if is_retryable_group_payload_error(&error) => {
                            if is_unknown_group_payload_error(&error) {
                                self.fetch_recent_messages_for_owner_with_lookback(
                                    OwnerPubkey::from_bytes(sender_pubkey.to_bytes()),
                                    now,
                                    UNKNOWN_GROUP_RECOVERY_LOOKBACK_SECS,
                                );
                            }
                            still_pending.push(item);
                        }
                        Err(error) => {
                            self.state.toast = Some(error.to_string());
                            made_progress = true;
                        }
                    }
                    continue;
                }

                let PendingInbound::Envelope { envelope } = &item else {
                    continue;
                };

                let sender_owner = self.logged_in.as_ref().and_then(|logged_in| {
                    resolve_message_sender_owner(&logged_in.session_manager, envelope, now)
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
                        .receive(&mut ctx, sender_owner, envelope)
                };
                match receive_result {
                    Ok(Some(message)) => match self.apply_decrypted_payload(
                        message.owner_pubkey,
                        &message.payload,
                        envelope.created_at.get(),
                    ) {
                        Ok(()) => {
                            made_progress = true;
                        }
                        Err(error) if is_retryable_group_payload_error(&error) => {
                            if is_unknown_group_payload_error(&error) {
                                self.fetch_recent_messages_for_owner_with_lookback(
                                    message.owner_pubkey,
                                    now,
                                    UNKNOWN_GROUP_RECOVERY_LOOKBACK_SECS,
                                );
                            }
                            still_pending.push(PendingInbound::decrypted(
                                message.owner_pubkey,
                                message.payload,
                                envelope.created_at.get(),
                            ));
                        }
                        Err(error) => {
                            self.state.toast = Some(error.to_string());
                            made_progress = true;
                        }
                    },
                    Ok(None) | Err(_) => {
                        // If the owner is now resolvable but the real session manager can no
                        // longer receive this envelope, the payload was already consumed earlier.
                        // Keeping the raw envelope would wedge the queue forever.
                        made_progress = true;
                    }
                }
            }

            if still_pending.is_empty() || !made_progress {
                self.pending_inbound = still_pending;
                break;
            }
            pending = still_pending;
        }
    }

    pub(super) fn retry_pending_outbound(&mut self, now: UnixSeconds) {
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

            if is_group_chat_id(&pending_message.chat_id) {
                let Some(group_id) = parse_group_id_from_chat_id(&pending_message.chat_id) else {
                    self.update_message_delivery(
                        &pending_message.chat_id,
                        &pending_message.message_id,
                        DeliveryState::Failed,
                    );
                    continue;
                };
                let payload = match encode_app_group_message_payload(&pending_message.body) {
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
                    let (session_manager, group_manager) =
                        (&mut logged_in.session_manager, &mut logged_in.group_manager);
                    group_manager.send_message(session_manager, &mut ctx, &group_id, payload)
                };

                match prepared {
                    Ok(prepared) => {
                        self.publish_group_local_sibling_best_effort(&prepared);
                        if let Some(reason) = pending_reason_from_group_prepared(&prepared) {
                            self.push_debug_log(
                                "retry.group.pending",
                                format!(
                                    "chat_id={} reason={reason:?} gaps={}",
                                    pending_message.chat_id,
                                    summarize_relay_gaps(&prepared.remote.relay_gaps)
                                ),
                            );
                            pending_message.reason = reason.clone();
                            pending_message.next_retry_at_secs =
                                now.get().saturating_add(PENDING_RETRY_DELAY_SECS);
                            self.nudge_protocol_state_for_pending_reason(&reason);
                            pending_message.publish_mode = OutboundPublishMode::WaitForPeer;
                            still_pending.push(pending_message);
                        } else {
                            match build_group_prepared_publish_batch(&prepared) {
                                Ok(Some(batch)) => {
                                    pending_message.publish_mode = publish_mode_for_batch(&batch);
                                    pending_message.prepared_publish = Some(batch.clone());
                                    pending_message.reason = pending_reason_for_publish_mode(
                                        &pending_message.publish_mode,
                                    );
                                    pending_message.next_retry_at_secs =
                                        retry_deadline_for_publish_mode(
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
                                    self.push_debug_log(
                                        "retry.group.pending",
                                        format!(
                                            "chat_id={} reason={:?}",
                                            pending_message.chat_id, pending_message.reason
                                        ),
                                    );
                                    self.nudge_protocol_state_for_pending_reason(
                                        &pending_message.reason,
                                    );
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
                continue;
            }

            let prepared = {
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

                let payload = match encode_app_direct_message_payload(
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
                        self.push_debug_log(
                            "retry.direct.pending",
                            format!(
                                "chat_id={} reason={reason:?} gaps={}",
                                pending_message.chat_id,
                                summarize_relay_gaps(&prepared.relay_gaps)
                            ),
                        );
                        pending_message.reason = reason.clone();
                        pending_message.next_retry_at_secs =
                            now.get().saturating_add(PENDING_RETRY_DELAY_SECS);
                        self.nudge_protocol_state_for_pending_reason(&reason);
                        pending_message.publish_mode = OutboundPublishMode::WaitForPeer;
                        still_pending.push(pending_message);
                    } else {
                        match build_prepared_publish_batch(&prepared) {
                            Ok(Some(batch)) => {
                                pending_message.publish_mode = publish_mode_for_batch(&batch);
                                pending_message.prepared_publish = Some(batch.clone());
                                pending_message.reason =
                                    pending_reason_for_publish_mode(&pending_message.publish_mode);
                                pending_message.next_retry_at_secs =
                                    retry_deadline_for_publish_mode(
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
                                self.push_debug_log(
                                    "retry.direct.pending",
                                    format!(
                                        "chat_id={} reason={:?}",
                                        pending_message.chat_id, pending_message.reason
                                    ),
                                );
                                self.nudge_protocol_state_for_pending_reason(
                                    &pending_message.reason,
                                );
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

    pub(super) fn queue_pending_outbound(
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

    pub(super) fn set_pending_outbound_in_flight(&mut self, message_id: &str, in_flight: bool) {
        if let Some(pending) = self
            .pending_outbound
            .iter_mut()
            .find(|pending| pending.message_id == message_id)
        {
            pending.in_flight = in_flight;
        }
    }

    pub(super) fn update_message_delivery(
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

    pub(super) fn push_outgoing_message(
        &mut self,
        chat_id: &str,
        body: String,
        created_at_secs: u64,
        delivery: DeliveryState,
    ) -> ChatMessageSnapshot {
        let (body, attachments) = extract_message_attachments(&body);
        let message = ChatMessageSnapshot {
            id: self.allocate_message_id(),
            chat_id: chat_id.to_string(),
            author: self
                .state
                .account
                .as_ref()
                .map(|account| account.display_name.clone())
                .unwrap_or_else(|| "me".to_string()),
            body,
            attachments,
            reactions: Vec::new(),
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
            .insert_message_sorted(message.clone());
        if let Some(thread) = self.threads.get_mut(chat_id) {
            thread.updated_at_secs = thread.updated_at_secs.max(created_at_secs);
        }
        message
    }

    pub(super) fn push_incoming_message_from(
        &mut self,
        chat_id: &str,
        body: String,
        created_at_secs: u64,
        author: Option<String>,
    ) {
        let message_id = self.allocate_message_id();
        let author = author.unwrap_or_else(|| self.owner_display_label(chat_id));
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
        thread.updated_at_secs = thread.updated_at_secs.max(created_at_secs);
        let (body, attachments) = extract_message_attachments(&body);
        thread.insert_message_sorted(ChatMessageSnapshot {
            id: message_id,
            chat_id: chat_id.to_string(),
            author,
            body,
            attachments,
            reactions: Vec::new(),
            is_outgoing: false,
            created_at_secs,
            delivery: DeliveryState::Received,
        });
    }

    pub(super) fn push_system_notice(&mut self, chat_id: &str, body: String, created_at_secs: u64) {
        let message_id = self.allocate_message_id();
        let thread = self
            .threads
            .entry(chat_id.to_string())
            .or_insert_with(|| ThreadRecord {
                chat_id: chat_id.to_string(),
                unread_count: 0,
                updated_at_secs: created_at_secs,
                messages: Vec::new(),
            });
        if thread
            .messages
            .iter()
            .any(|message| message.author == "Iris" && message.body == body)
        {
            return;
        }
        if self.active_chat_id.as_deref() != Some(chat_id) {
            thread.unread_count = thread.unread_count.saturating_add(1);
        }
        thread.updated_at_secs = thread.updated_at_secs.max(created_at_secs);
        thread.insert_message_sorted(ChatMessageSnapshot {
            id: message_id,
            chat_id: chat_id.to_string(),
            author: "Iris".to_string(),
            body,
            attachments: Vec::new(),
            reactions: Vec::new(),
            is_outgoing: false,
            created_at_secs,
            delivery: DeliveryState::Received,
        });
    }

    pub(super) fn toggle_reaction(&mut self, chat_id: &str, message_id: &str, emoji: &str) {
        let emoji = emoji.trim();
        if chat_id.is_empty() || message_id.is_empty() || emoji.is_empty() {
            return;
        }
        let Some(thread) = self.threads.get_mut(chat_id) else {
            return;
        };
        let Some(message) = thread
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        else {
            return;
        };
        toggle_local_reaction(message, emoji);
        self.persist_best_effort();
        self.rebuild_state();
        self.emit_state();
    }

    pub(super) fn delete_local_message(&mut self, chat_id: &str, message_id: &str) {
        if chat_id.is_empty() || message_id.is_empty() {
            return;
        }
        let Some(thread) = self.threads.get_mut(chat_id) else {
            return;
        };
        let original_len = thread.messages.len();
        thread.messages.retain(|message| message.id != message_id);
        if thread.messages.len() == original_len {
            return;
        }
        thread.updated_at_secs = thread
            .messages
            .last()
            .map(|message| message.created_at_secs)
            .unwrap_or(thread.updated_at_secs);
        if self.active_chat_id.as_deref() == Some(chat_id) {
            thread.unread_count = 0;
        }
        self.persist_best_effort();
        self.rebuild_state();
        self.emit_state();
    }

    pub(super) fn apply_routed_chat_message(
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
            self.push_incoming_message_from(
                &routed.chat_id,
                routed.body,
                created_at_secs,
                routed.author,
            );
        }
    }

    pub(super) fn route_received_direct_message(
        &self,
        local_owner: OwnerPubkey,
        sender_owner: OwnerPubkey,
        payload: &[u8],
    ) -> RoutedChatMessage {
        if let Some(decoded) = decode_app_direct_message_payload(payload) {
            if sender_owner == local_owner {
                if let Ok((chat_id, _)) = parse_peer_input(&decoded.chat_id) {
                    if chat_id != local_owner.to_string() {
                        return RoutedChatMessage {
                            chat_id,
                            body: decoded.body,
                            is_outgoing: true,
                            author: Some(self.owner_display_label(&local_owner.to_string())),
                        };
                    }
                }
            }

            return RoutedChatMessage {
                chat_id: sender_owner.to_string(),
                body: decoded.body,
                is_outgoing: false,
                author: Some(self.owner_display_label(&sender_owner.to_string())),
            };
        }

        RoutedChatMessage {
            chat_id: sender_owner.to_string(),
            body: String::from_utf8_lossy(payload).into_owned(),
            is_outgoing: false,
            author: Some(self.owner_display_label(&sender_owner.to_string())),
        }
    }

    pub(super) fn apply_group_metadata_update(
        &mut self,
        group: GroupSnapshot,
        previous: Option<GroupSnapshot>,
        created_at_secs: u64,
    ) {
        self.apply_group_snapshot_to_threads_with_notices(
            previous.as_ref(),
            &group,
            created_at_secs.max(group.updated_at.get()),
        );
    }

    pub(super) fn apply_decrypted_payload(
        &mut self,
        sender_owner: OwnerPubkey,
        payload: &[u8],
        created_at_secs: u64,
    ) -> anyhow::Result<()> {
        let local_owner = self.logged_in.as_ref().expect("logged in").owner_pubkey;

        let previous_groups = self
            .logged_in
            .as_ref()
            .map(|logged_in| {
                logged_in
                    .group_manager
                    .snapshot()
                    .groups
                    .into_iter()
                    .map(|group| (group.group_id.clone(), group))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();

        let group_event = {
            let logged_in = self.logged_in.as_mut().expect("logged in");
            logged_in
                .group_manager
                .handle_incoming(sender_owner, payload)?
        };

        match group_event {
            Some(GroupIncomingEvent::MetadataUpdated(group)) => {
                let previous = previous_groups.get(&group.group_id).cloned();
                self.apply_group_metadata_update(group, previous, created_at_secs);
            }
            Some(GroupIncomingEvent::Message(group_message)) => {
                let decoded = decode_app_group_message_payload(&group_message.body)
                    .ok_or_else(|| anyhow::anyhow!("Invalid group message payload."))?;
                self.apply_routed_chat_message(
                    RoutedChatMessage {
                        chat_id: group_chat_id(&group_message.group_id),
                        body: decoded.body,
                        is_outgoing: group_message.sender_owner == local_owner,
                        author: Some(
                            self.owner_display_label(&group_message.sender_owner.to_string()),
                        ),
                    },
                    created_at_secs,
                );
            }
            None => {
                let routed = self.route_received_direct_message(local_owner, sender_owner, payload);
                self.apply_routed_chat_message(routed, created_at_secs);
            }
        }

        Ok(())
    }

    pub(super) fn allocate_message_id(&mut self) -> String {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.saturating_add(1);
        id.to_string()
    }
}

pub(super) fn toggle_local_reaction(message: &mut ChatMessageSnapshot, emoji: &str) {
    let emoji = emoji.trim();
    if emoji.is_empty() {
        return;
    }
    if let Some(index) = message
        .reactions
        .iter()
        .position(|reaction| reaction.emoji == emoji)
    {
        let reaction = &mut message.reactions[index];
        if reaction.reacted_by_me {
            reaction.reacted_by_me = false;
            reaction.count = reaction.count.saturating_sub(1);
            if reaction.count == 0 {
                message.reactions.remove(index);
            }
        } else {
            reaction.reacted_by_me = true;
            reaction.count = reaction.count.saturating_add(1);
        }
    } else {
        message.reactions.push(MessageReactionSnapshot {
            emoji: emoji.to_string(),
            count: 1,
            reacted_by_me: true,
        });
    }
    sort_message_reactions(&mut message.reactions);
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn apply_incoming_reaction(message: &mut ChatMessageSnapshot, emoji: &str) -> bool {
    let emoji = emoji.trim();
    if emoji.is_empty() {
        return false;
    }
    if let Some(reaction) = message
        .reactions
        .iter_mut()
        .find(|reaction| reaction.emoji == emoji)
    {
        reaction.count = reaction.count.saturating_add(1);
    } else {
        message.reactions.push(MessageReactionSnapshot {
            emoji: emoji.to_string(),
            count: 1,
            reacted_by_me: false,
        });
    }
    sort_message_reactions(&mut message.reactions);
    true
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn reaction_notification_body(emoji: &str, target_preview: &str) -> String {
    let emoji = emoji.trim();
    let target_preview = target_preview.trim();
    if target_preview.is_empty() {
        format!("New reaction {emoji}")
    } else {
        format!(
            "Reaction {emoji} to \"{}\"",
            truncate_reaction_preview(target_preview)
        )
    }
}

fn sort_message_reactions(reactions: &mut [MessageReactionSnapshot]) {
    reactions.sort_by(|left, right| {
        left.emoji
            .cmp(&right.emoji)
            .then_with(|| right.count.cmp(&left.count))
    });
}

#[cfg_attr(not(test), allow(dead_code))]
fn truncate_reaction_preview(preview: &str) -> String {
    const MAX_CHARS: usize = 80;
    let mut chars = preview.chars();
    let truncated = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}
