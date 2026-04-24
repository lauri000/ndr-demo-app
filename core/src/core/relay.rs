use super::*;

impl AppCore {
    pub(super) fn handle_relay_event(&mut self, event: Event) {
        let event_id = event.id.to_string();
        if self.has_seen_event(&event_id) {
            return;
        }

        if self.logged_in.is_none() {
            return;
        }

        let kind = event.kind.as_u16() as u32;
        self.push_debug_log("relay.event", format!("kind_raw={} id={event_id}", kind));
        let now = unix_now();
        self.prune_recent_handshake_peers(now.get());
        match kind {
            0 => {
                if self.apply_profile_metadata_event(&event) {
                    self.remember_event(event_id);
                    self.persist_best_effort();
                    self.rebuild_state();
                    self.emit_state();
                    return;
                }
                self.remember_event(event_id);
            }
            codec::ROSTER_EVENT_KIND => {
                if let Ok(decoded) = codec::parse_roster_event(&event) {
                    self.debug_event_counters.roster_events += 1;
                    let is_local_owner = self
                        .logged_in
                        .as_ref()
                        .map(|logged_in| decoded.owner_pubkey == logged_in.owner_pubkey)
                        .unwrap_or(false);

                    let mut roster_log: Option<(&'static str, String)> = None;
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
                                    roster_log = Some((
                                        "relay.roster.local",
                                        "local device transitioned to Authorized".to_string(),
                                    ));
                                    self.state.toast =
                                        Some("This device has been approved.".to_string());
                                }
                                (_, LocalAuthorizationState::Revoked) => {
                                    roster_log = Some((
                                        "relay.roster.local",
                                        "local device transitioned to Revoked".to_string(),
                                    ));
                                    self.state.toast = Some(
                                        "This device was removed from the roster.".to_string(),
                                    );
                                    self.active_chat_id = None;
                                    self.screen_stack.clear();
                                    self.pending_inbound.clear();
                                    self.pending_outbound.clear();
                                    self.pending_group_controls.clear();
                                }
                                _ => {}
                            }
                        } else {
                            roster_log = Some((
                                "relay.roster.peer",
                                format!("observed roster for {}", decoded.owner_pubkey),
                            ));
                            logged_in
                                .session_manager
                                .observe_peer_roster(decoded.owner_pubkey, decoded.roster);
                        }
                    }
                    if let Some((category, detail)) = roster_log {
                        self.push_debug_log(category, detail);
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
                    self.debug_event_counters.invite_events += 1;
                    let invite_owner = invite.inviter_owner_pubkey.unwrap_or_else(|| {
                        OwnerPubkey::from_bytes(invite.inviter_device_pubkey.to_bytes())
                    });
                    let local_device = {
                        let logged_in = self.logged_in.as_ref().expect("checked above");
                        local_device_from_keys(&logged_in.device_keys)
                    };
                    // Pending linked-device invites for the local owner arrive before the
                    // device has been added to the owner-signed roster. Observe every
                    // non-self invite so the primary can render it as "Pending".
                    let should_observe = invite.inviter_device_pubkey != local_device;
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
                            self.push_debug_log(
                                "relay.invite",
                                format!("observed invite for owner {}", invite_owner),
                            );
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
                self.debug_event_counters.invite_response_events += 1;
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
                        self.push_debug_log(
                            "relay.invite_response",
                            format!(
                                "processed owner={} device={}",
                                processed.owner_pubkey, processed.device_pubkey
                            ),
                        );
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
                let expires_at_secs = message_expiration_from_event(&event);
                self.debug_event_counters.message_events += 1;

                let sender_owner = self.logged_in.as_ref().and_then(|logged_in| {
                    resolve_message_sender_owner(&logged_in.session_manager, &envelope, now)
                });
                let Some(sender_owner) = sender_owner else {
                    self.push_debug_log(
                        "relay.message.pending",
                        "sender owner unresolved; queued as pending inbound",
                    );
                    self.remember_event(event_id.clone());
                    self.pending_inbound
                        .push(PendingInbound::envelope(envelope, expires_at_secs));
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
                        self.push_debug_log(
                            "relay.message.received",
                            format!(
                                "owner={} bytes={}",
                                message.owner_pubkey,
                                message.payload.len()
                            ),
                        );
                        self.remember_event(event_id);
                        let owner_hex = message.owner_pubkey.to_string();
                        self.clear_recent_handshake_peer(&owner_hex);
                        if let Err(error) = self.apply_decrypted_payload(
                            message.owner_pubkey,
                            &message.payload,
                            now.get(),
                            expires_at_secs,
                        ) {
                            if is_retryable_group_payload_error(&error) {
                                if is_unknown_group_payload_error(&error) {
                                    self.fetch_recent_messages_for_owner_with_lookback(
                                        message.owner_pubkey,
                                        now,
                                        UNKNOWN_GROUP_RECOVERY_LOOKBACK_SECS,
                                    );
                                }
                                self.push_debug_log(
                                    "relay.message.pending",
                                    format!("payload apply deferred: {error}"),
                                );
                                self.pending_inbound.push(PendingInbound::decrypted(
                                    message.owner_pubkey,
                                    message.payload,
                                    now.get(),
                                    expires_at_secs,
                                ));
                            } else {
                                self.state.toast = Some(error.to_string());
                            }
                        } else {
                            self.retry_pending_inbound(now);
                        }
                        self.request_protocol_subscription_refresh();
                        self.persist_best_effort();
                        self.rebuild_state();
                        self.emit_state();
                    }
                    Ok(None) => {
                        self.push_debug_log(
                            "relay.message.pending",
                            "session_manager returned None; queued as pending inbound",
                        );
                        self.remember_event(event_id.clone());
                        self.pending_inbound
                            .push(PendingInbound::envelope(envelope, expires_at_secs));
                        self.persist_best_effort();
                    }
                    Err(error) => {
                        self.remember_event(event_id);
                        self.state.toast = Some(error.to_string());
                        self.emit_state();
                    }
                }
            }
            _ => {
                self.debug_event_counters.other_events += 1;
            }
        }
    }
}

fn message_expiration_from_event(event: &Event) -> Option<u64> {
    let raw = event
        .tags
        .iter()
        .find(|tag| tag.as_slice().first().map(|value| value.as_str()) == Some("expiration"))
        .and_then(|tag| tag.as_slice().get(1))?;
    let mut value = raw.parse::<u64>().ok()?;
    if value == 0 {
        return None;
    }
    while value > 9_999_999_999 {
        value /= 1_000;
    }
    (value > 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::Tag;

    #[test]
    fn parses_message_expiration_tag_seconds_and_milliseconds() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::from(codec::MESSAGE_EVENT_KIND as u16), "cipher")
            .tag(Tag::parse(["expiration", "1704067260123"]).expect("expiration tag"))
            .sign_with_keys(&keys)
            .expect("event");

        assert_eq!(message_expiration_from_event(&event), Some(1_704_067_260));
    }

    #[test]
    fn ignores_invalid_message_expiration_tags() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::from(codec::MESSAGE_EVENT_KIND as u16), "cipher")
            .tag(Tag::parse(["expiration", "0"]).expect("expiration tag"))
            .sign_with_keys(&keys)
            .expect("event");

        assert_eq!(message_expiration_from_event(&event), None);
    }
}
