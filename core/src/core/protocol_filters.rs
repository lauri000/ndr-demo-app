use super::*;

pub(super) fn sorted_hexes(values: HashSet<String>) -> Vec<String> {
    let mut sorted = values.into_iter().collect::<Vec<_>>();
    sorted.sort();
    sorted.dedup();
    sorted
}

pub(super) fn build_protocol_filters(plan: &ProtocolSubscriptionPlan) -> Vec<Filter> {
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
                .authors(roster_authors.clone()),
        );
        filters.push(Filter::new().kind(Kind::Metadata).authors(roster_authors));
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

pub(super) fn build_protocol_state_catch_up_filters(
    plan: &ProtocolSubscriptionPlan,
    now: UnixSeconds,
) -> Vec<Filter> {
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
                .authors(roster_authors.clone()),
        );
        filters.push(Filter::new().kind(Kind::Metadata).authors(roster_authors));
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
                .authors(message_authors)
                .since(Timestamp::from(
                    now.get().saturating_sub(CATCH_UP_LOOKBACK_SECS),
                )),
        );
    }

    filters
}

pub(super) fn summarize_protocol_plan(plan: Option<&ProtocolSubscriptionPlan>) -> String {
    let Some(plan) = plan else {
        return "none".to_string();
    };
    format!(
        "rosters={} invites={} invite_response={} messages={}",
        plan.roster_authors.join(","),
        plan.invite_authors.join(","),
        plan.invite_response_recipient
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        plan.message_authors.join(","),
    )
}

pub(super) fn summarize_relay_gaps(gaps: &[RelayGap]) -> String {
    if gaps.is_empty() {
        return "none".to_string();
    }

    gaps.iter()
        .map(|gap| match gap {
            RelayGap::MissingRoster { owner_pubkey } => {
                format!("MissingRoster({owner_pubkey})")
            }
            RelayGap::MissingDeviceInvite {
                owner_pubkey,
                device_pubkey,
            } => format!("MissingDeviceInvite({owner_pubkey},{device_pubkey})"),
        })
        .collect::<Vec<_>>()
        .join("|")
}
