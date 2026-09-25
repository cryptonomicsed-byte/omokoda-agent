/// Phase 17.1 — State merge logic.
///
/// Applies a StateDelta to an AgentPublicState.
/// Merge is deterministic: same inputs always produce same output.
/// Concurrent edit resolution: higher version wins (caller already validated version monotonicity).

use crate::types::{AgentPublicState, StateDelta};

/// Apply `delta` to `current`, returning the merged state.
///
/// The caller (contract update_state) must verify:
///   - delta.verify_signature() is true
///   - delta.new_version > current.version
/// before calling this function.
pub fn merge_state(mut current: AgentPublicState, delta: StateDelta) -> AgentPublicState {
    // Set agent_npub on first init
    if current.agent_npub.is_empty() {
        current.agent_npub = delta.agent_npub.clone();
    }

    if let Some(v) = delta.relay_list {
        current.relay_list = v;
    }
    if let Some(v) = delta.display_name {
        current.display_name = v;
    }
    if let Some(v) = delta.about {
        current.about = v;
    }
    if let Some(v) = delta.walrus_profile_url {
        current.walrus_profile_url = v;
    }
    if let Some(v) = delta.last_seen {
        current.last_seen = v;
    }
    if let Some(v) = delta.presence {
        current.presence = v;
    }
    if let Some(v) = delta.capabilities {
        current.capabilities = v;
    }
    if let Some(v) = delta.l1_state_root {
        current.l1_state_root = Some(v);
    }
    if let Some(v) = delta.l1_block_height {
        current.l1_block_height = v;
    }

    // Social graph: apply additions
    if let Some(add) = delta.social_edges_add {
        for (npub, rel) in add {
            current.social_edges.insert(npub, rel);
        }
    }
    // Social graph: apply removals
    if let Some(remove) = delta.social_edges_remove {
        for npub in remove {
            current.social_edges.remove(&npub);
        }
    }

    // Advance version
    current.version = delta.new_version;

    // Signature from delta is the authorizing sig for this version.
    current.signature = delta.signature;

    current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StateDelta;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn make_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    fn sign_delta(delta: &mut StateDelta, key: &SigningKey) {
        use ed25519_dalek::Signer;
        let bytes = delta.delta_signing_bytes();
        let sig = key.sign(&bytes);
        delta.signature = hex::encode(sig.to_bytes());
    }

    fn npub_hex(key: &SigningKey) -> String {
        hex::encode(key.verifying_key().to_bytes())
    }

    #[test]
    fn merge_applies_all_fields() {
        let key = make_signing_key();
        let npub = npub_hex(&key);

        let current = AgentPublicState {
            agent_npub: npub.clone(),
            version: 0,
            ..Default::default()
        };

        let mut delta = StateDelta {
            agent_npub: npub.clone(),
            new_version: 1,
            display_name: Some("Ọ̀ṢỌ́ Agent".to_string()),
            about: Some("Sovereign compute unit".to_string()),
            presence: Some("ACTIVE".to_string()),
            relay_list: Some(vec!["wss://relay.test".to_string()]),
            capabilities: Some(vec!["text_generation".to_string()]),
            last_seen: Some(1_700_000_000),
            ..Default::default()
        };
        sign_delta(&mut delta, &key);

        let merged = merge_state(current, delta);
        assert_eq!(merged.version, 1);
        assert_eq!(merged.display_name, "Ọ̀ṢỌ́ Agent");
        assert_eq!(merged.presence, "ACTIVE");
        assert_eq!(merged.relay_list, vec!["wss://relay.test"]);
        assert_eq!(merged.capabilities, vec!["text_generation"]);
        assert_eq!(merged.last_seen, 1_700_000_000);
    }

    #[test]
    fn merge_social_graph_add_and_remove() {
        let key = make_signing_key();
        let npub = npub_hex(&key);

        let current = AgentPublicState {
            agent_npub: npub.clone(),
            version: 2,
            social_edges: {
                let mut m = std::collections::BTreeMap::new();
                m.insert("npub_existing".to_string(), "follows".to_string());
                m
            },
            ..Default::default()
        };

        let mut delta = StateDelta {
            agent_npub: npub.clone(),
            new_version: 3,
            social_edges_add: Some({
                let mut m = std::collections::BTreeMap::new();
                m.insert("npub_new".to_string(), "delegates_to".to_string());
                m
            }),
            social_edges_remove: Some(vec!["npub_existing".to_string()]),
            ..Default::default()
        };
        sign_delta(&mut delta, &key);

        let merged = merge_state(current, delta);
        assert!(merged.social_edges.contains_key("npub_new"));
        assert!(!merged.social_edges.contains_key("npub_existing"));
        assert_eq!(merged.version, 3);
    }

    #[test]
    fn merge_initializes_agent_npub_on_empty_state() {
        let key = make_signing_key();
        let npub = npub_hex(&key);

        let current = AgentPublicState::default(); // agent_npub is empty

        let mut delta = StateDelta {
            agent_npub: npub.clone(),
            new_version: 1,
            display_name: Some("New Agent".to_string()),
            ..Default::default()
        };
        sign_delta(&mut delta, &key);

        let merged = merge_state(current, delta);
        assert_eq!(merged.agent_npub, npub);
        assert_eq!(merged.display_name, "New Agent");
    }

    #[test]
    fn state_signing_bytes_are_deterministic() {
        let state = AgentPublicState {
            agent_npub: "pub123".to_string(),
            display_name: "Test Agent".to_string(),
            version: 5,
            ..Default::default()
        };
        let b1 = state.state_signing_bytes();
        let b2 = state.state_signing_bytes();
        assert_eq!(b1, b2, "signing bytes must be deterministic");
    }

    #[test]
    fn state_hash_changes_with_version() {
        let state_v1 = AgentPublicState {
            agent_npub: "pub123".to_string(),
            version: 1,
            ..Default::default()
        };
        let state_v2 = AgentPublicState {
            agent_npub: "pub123".to_string(),
            version: 2,
            ..Default::default()
        };
        assert_ne!(state_v1.state_hash(), state_v2.state_hash());
    }
}
