use super::*;

impl AppCore {
    pub(super) fn set_local_profile_name(&mut self, name: &str) {
        let Some(local_owner_hex) = self
            .logged_in
            .as_ref()
            .map(|logged_in| logged_in.owner_pubkey.to_string())
        else {
            return;
        };

        let Some(record) = build_owner_profile_record(name) else {
            return;
        };

        self.owner_profiles.insert(local_owner_hex.clone(), record);
        self.push_debug_log("profile.local.set", format!("owner={local_owner_hex}"));
        self.persist_best_effort();
    }

    pub(super) fn apply_profile_metadata_event(&mut self, event: &Event) -> bool {
        let owner_hex = event.pubkey.to_hex();
        let Some(record) = parse_owner_profile_record(&event.content, event.created_at.as_u64())
        else {
            return false;
        };

        if let Some(existing) = self.owner_profiles.get(&owner_hex) {
            if existing.updated_at_secs > record.updated_at_secs {
                return false;
            }
        }

        self.owner_profiles.insert(owner_hex.clone(), record);
        self.push_debug_log("relay.metadata", format!("owner={owner_hex}"));
        true
    }

    pub(super) fn owner_display_name(&self, owner_hex: &str) -> Option<String> {
        self.owner_profiles
            .get(owner_hex)
            .and_then(OwnerProfileRecord::preferred_label)
    }

    pub(super) fn owner_display_label(&self, owner_hex: &str) -> String {
        self.owner_display_name(owner_hex)
            .or_else(|| owner_npub(owner_hex))
            .unwrap_or_else(|| owner_hex.to_string())
    }

    pub(super) fn owner_secondary_identifier(&self, owner_hex: &str) -> Option<String> {
        let npub = owner_npub(owner_hex)?;
        match self.owner_display_name(owner_hex) {
            Some(label) if label != npub => Some(npub),
            Some(_) => None,
            None => Some(npub),
        }
    }
}
