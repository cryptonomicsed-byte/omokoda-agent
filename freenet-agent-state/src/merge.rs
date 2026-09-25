/// Deterministic merge logic for concurrent `AgentPublicState` edits.
///
/// Rules (in priority order):
/// 1. Higher `version` wins unconditionally.
/// 2. On equal version: higher `updated_at` wins.
/// 3. On equal version + equal timestamp: the state with the lower
///    lexicographic `state_hash` wins (deterministic tiebreak).
///
/// This ensures convergence across all Freenet relay nodes regardless of
/// message delivery order.
use crate::types::{AgentPublicState, StateDelta};

/// Apply a `StateDelta` to an existing state, returning the new state.
/// Returns `Err` if the delta's `new_version != state.version + 1`.
pub fn apply_delta(
    state: &AgentPublicState,
    delta: &StateDelta,
) -> Result<AgentPublicState, String> {
    if delta.agent_npub != state.agent_npub {
        return Err(format!(
            "npub mismatch: delta={} state={}",
            delta.agent_npub, state.agent_npub
        ));
    }
    if delta.new_version != state.version + 1 {
        return Err(format!(
            "version mismatch: expected {}, got {}",
            state.version + 1,
            delta.new_version
        ));
    }

    let mut next = state.clone();
    if let Some(v) = &delta.display_name          { next.display_name = v.clone(); }
    if let Some(v) = &delta.bio                   { next.bio = v.clone(); }
    if let Some(v) = &delta.relay_list            { next.relay_list = v.clone(); }
    if let Some(v) = delta.tier                   { next.tier = v; }
    if let Some(v) = delta.last_seen              { next.last_seen = v; }
    if let Some(v) = &delta.declared_capabilities { next.declared_capabilities = v.clone(); }
    if let Some(v) = &delta.lifecycle_stage       { next.lifecycle_stage = v.clone(); }
    if let Some(v) = &delta.sui_object_id         { next.sui_object_id = v.clone(); }
    if let Some(v) = &delta.bipon39_hint          { next.bipon39_hint = v.clone(); }

    next.version    = delta.new_version;
    next.updated_at = delta.updated_at;
    next.signature  = delta.signature.clone();
    Ok(next)
}

/// Merge two concurrent states — used when Freenet receives conflicting
/// updates from different relay paths.  Deterministic: same inputs always
/// produce the same winner.
pub fn merge(a: &AgentPublicState, b: &AgentPublicState) -> AgentPublicState {
    // Higher version always wins.
    if a.version > b.version { return a.clone(); }
    if b.version > a.version { return b.clone(); }

    // Tie: use the more recently updated.
    if a.updated_at > b.updated_at { return a.clone(); }
    if b.updated_at > a.updated_at { return b.clone(); }

    // Final tiebreak: lexicographically lower state_hash.
    let ha = hex::encode(a.state_hash());
    let hb = hex::encode(b.state_hash());
    if ha <= hb { a.clone() } else { b.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentPublicState;

    fn base() -> AgentPublicState {
        AgentPublicState {
            agent_npub: "npub1test".into(),
            display_name: "Agent".into(),
            bio: "".into(),
            relay_list: vec![],
            tier: 1,
            last_seen: 1_000,
            declared_capabilities: vec![],
            lifecycle_stage: "active".into(),
            sui_object_id: None,
            bipon39_hint: None,
            version: 1,
            updated_at: 1_000,
            signature: "sig1".into(),
        }
    }

    fn delta(new_version: u64, display_name: &str) -> StateDelta {
        StateDelta {
            agent_npub: "npub1test".into(),
            display_name: Some(display_name.into()),
            bio: None,
            relay_list: None,
            tier: None,
            last_seen: None,
            declared_capabilities: None,
            lifecycle_stage: None,
            sui_object_id: None,
            bipon39_hint: None,
            new_version,
            updated_at: 2_000,
            signature: "sig_delta".into(),
        }
    }

    #[test]
    fn apply_delta_increments_version() {
        let state = base();
        let d = delta(2, "NewName");
        let next = apply_delta(&state, &d).expect("must succeed");
        assert_eq!(next.version, 2);
        assert_eq!(next.display_name, "NewName");
    }

    #[test]
    fn apply_delta_rejects_wrong_version() {
        let state = base();
        let d = delta(5, "Skip");
        assert!(apply_delta(&state, &d).is_err());
    }

    #[test]
    fn merge_higher_version_wins() {
        let mut a = base();
        let mut b = base();
        b.version = 3;
        b.display_name = "BNewer".into();
        let winner = merge(&a, &b);
        assert_eq!(winner.display_name, "BNewer");

        a.version = 10;
        a.display_name = "ANewer".into();
        let winner2 = merge(&a, &b);
        assert_eq!(winner2.display_name, "ANewer");
    }

    #[test]
    fn merge_higher_updated_at_wins_on_same_version() {
        let a = base();
        let mut b = base();
        b.updated_at = 9_999;
        b.display_name = "BNewer".into();
        let winner = merge(&a, &b);
        assert_eq!(winner.display_name, "BNewer");
    }

    #[test]
    fn merge_is_deterministic_on_full_tie() {
        let a = base();
        let b = base();
        // Both identical — must not panic, must return a consistent winner.
        let w1 = merge(&a, &b);
        let w2 = merge(&b, &a);
        assert_eq!(w1.version, w2.version);
    }
}
