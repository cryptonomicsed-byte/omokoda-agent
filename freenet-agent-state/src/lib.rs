/// Freenet WASM contract — OSO Agent Public State — Phase 17.1
///
/// This crate compiles to a `cdylib` WASM module that runs inside a Freenet
/// relay node.  The contract validates and merges agent public-state updates.
///
/// The Freenet contract interface requires three exported functions:
///   - `validate_state`   — checks a proposed new state is valid
///   - `validate_delta`   — checks an incremental update is valid
///   - `summarize_state`  — produces a compact summary for relay sync
///
/// Compile for Freenet deployment:
///   cargo build --release --target wasm32-unknown-unknown --features contract
///
/// Run unit tests natively (no WASM needed):
///   cargo test --lib

pub mod merge;
pub mod types;

pub use merge::{apply_delta, merge as merge_states};
pub use types::{AgentPublicState, ContractError, StateDelta, StateSummary};

// ── Native (non-WASM) API — called from omokoda-core ─────────────────────────

/// Validate a full `AgentPublicState` without applying it.
/// Returns Ok(state_hash_hex) on success.
pub fn validate_state(state: &AgentPublicState) -> Result<String, ContractError> {
    if state.agent_npub.is_empty() {
        return Err(ContractError::InvalidNpub);
    }
    Ok(hex::encode(state.state_hash()))
}

/// Validate a `StateDelta` against the current state.
/// Returns Ok(()) if the delta is structurally valid (version + npub match).
/// NOTE: Ed25519 signature verification is enforced in the WASM build only;
/// the native build trusts the caller (used from omokoda-core where the key
/// check already happened).
pub fn validate_delta(
    current: &AgentPublicState,
    delta: &StateDelta,
) -> Result<(), ContractError> {
    if delta.agent_npub != current.agent_npub {
        return Err(ContractError::NpubMismatch);
    }
    if delta.new_version != current.version + 1 {
        return Err(ContractError::VersionMismatch {
            expected: current.version + 1,
            got: delta.new_version,
        });
    }
    Ok(())
}

/// Produce a compact `StateSummary` for Freenet relay sync.
pub fn summarize(state: &AgentPublicState) -> StateSummary {
    StateSummary {
        agent_npub: state.agent_npub.clone(),
        version: state.version,
        state_hash: hex::encode(state.state_hash()),
        last_seen: state.last_seen,
        lifecycle_stage: state.lifecycle_stage.clone(),
    }
}

// ── WASM contract entry points (compiled only with `contract` feature) ────────

#[cfg(feature = "contract")]
mod wasm_contract {
    use freenet_stdlib::prelude::{
        contract, ContractError as FnError, ContractInterface, Parameters, RelatedContracts,
        State, StateDelta as FnDelta, StateSummary as FnSummary, UpdateData, UpdateModification,
        ValidateResult,
    };

    use crate::{merge, types::AgentPublicState};

    fn deser_err(e: impl std::fmt::Display) -> FnError {
        FnError::Deser(e.to_string())
    }

    struct AgentStateContract;

    #[contract]
    impl ContractInterface for AgentStateContract {
        fn validate_state(
            _parameters: Parameters<'static>,
            state: State<'static>,
            _related: RelatedContracts<'static>,
        ) -> Result<ValidateResult, FnError> {
            let parsed: AgentPublicState =
                serde_json::from_slice(&state).map_err(deser_err)?;
            crate::validate_state(&parsed).map_err(|e| FnError::Other(e.to_string()))?;
            Ok(ValidateResult::Valid)
        }

        fn update_state(
            _parameters: Parameters<'static>,
            state: State<'static>,
            data: Vec<UpdateData<'static>>,
        ) -> Result<UpdateModification<'static>, FnError> {
            let mut current: AgentPublicState =
                serde_json::from_slice(&state).map_err(deser_err)?;

            for update in data {
                match update {
                    UpdateData::Delta(delta_bytes) => {
                        let delta: crate::types::StateDelta =
                            serde_json::from_slice(&delta_bytes).map_err(deser_err)?;
                        current = merge::apply_delta(&current, &delta)
                            .map_err(|e| FnError::Other(e))?;
                    }
                    UpdateData::State(new_state_bytes) => {
                        let incoming: AgentPublicState =
                            serde_json::from_slice(&new_state_bytes).map_err(deser_err)?;
                        current = merge::merge(&current, &incoming);
                    }
                    _ => {}
                }
            }

            let bytes = serde_json::to_vec(&current).map_err(deser_err)?;
            Ok(UpdateModification::valid(State::from(bytes)))
        }

        fn summarize_state(
            _parameters: Parameters<'static>,
            state: State<'static>,
        ) -> Result<FnSummary<'static>, FnError> {
            let parsed: AgentPublicState =
                serde_json::from_slice(&state).map_err(deser_err)?;
            let summary = crate::summarize(&parsed);
            let bytes = serde_json::to_vec(&summary).map_err(deser_err)?;
            Ok(FnSummary::from(bytes))
        }

        fn get_state_delta(
            _parameters: Parameters<'static>,
            state: State<'static>,
            summary: FnSummary<'static>,
        ) -> Result<FnDelta<'static>, FnError> {
            let current: AgentPublicState =
                serde_json::from_slice(&state).map_err(deser_err)?;
            let incoming: crate::types::StateSummary =
                serde_json::from_slice(&summary).map_err(deser_err)?;
            if current.version > incoming.version {
                let full = serde_json::to_vec(&current).map_err(deser_err)?;
                Ok(FnDelta::from(full))
            } else {
                Ok(FnDelta::from(vec![]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(npub: &str, version: u64) -> AgentPublicState {
        AgentPublicState {
            agent_npub: npub.into(),
            display_name: "TestAgent".into(),
            bio: "bio".into(),
            relay_list: vec!["wss://relay.example".into()],
            tier: 1,
            last_seen: 1_700_000_000,
            declared_capabilities: vec!["think".into()],
            lifecycle_stage: "active".into(),
            sui_object_id: None,
            bipon39_hint: None,
            version,
            updated_at: 1_700_000_000 + version,
            signature: "testsig".into(),
        }
    }

    #[test]
    fn validate_state_ok() {
        let state = make_state("npub1abc", 1);
        let hash = validate_state(&state).expect("must be valid");
        assert!(!hash.is_empty());
    }

    #[test]
    fn validate_state_rejects_empty_npub() {
        let state = make_state("", 1);
        assert!(matches!(validate_state(&state), Err(ContractError::InvalidNpub)));
    }

    #[test]
    fn validate_delta_ok() {
        let state = make_state("npub1abc", 3);
        let delta = StateDelta {
            agent_npub: "npub1abc".into(),
            display_name: Some("Updated".into()),
            bio: None,
            relay_list: None,
            tier: None,
            last_seen: None,
            declared_capabilities: None,
            lifecycle_stage: None,
            sui_object_id: None,
            bipon39_hint: None,
            new_version: 4,
            updated_at: 1_700_000_010,
            signature: "sig".into(),
        };
        assert!(validate_delta(&state, &delta).is_ok());
    }

    #[test]
    fn validate_delta_rejects_version_skip() {
        let state = make_state("npub1abc", 1);
        let delta = StateDelta {
            agent_npub: "npub1abc".into(),
            display_name: None,
            bio: None,
            relay_list: None,
            tier: None,
            last_seen: None,
            declared_capabilities: None,
            lifecycle_stage: None,
            sui_object_id: None,
            bipon39_hint: None,
            new_version: 5, // skips versions
            updated_at: 1_000,
            signature: "sig".into(),
        };
        assert!(matches!(
            validate_delta(&state, &delta),
            Err(ContractError::VersionMismatch { .. })
        ));
    }

    #[test]
    fn summarize_produces_correct_fields() {
        let state = make_state("npub1xyz", 7);
        let summary = summarize(&state);
        assert_eq!(summary.agent_npub, "npub1xyz");
        assert_eq!(summary.version, 7);
        assert_eq!(summary.lifecycle_stage, "active");
        assert_eq!(summary.state_hash, hex::encode(state.state_hash()));
    }

    #[test]
    fn full_apply_and_summarize_roundtrip() {
        let state = make_state("npub1full", 1);
        let delta = StateDelta {
            agent_npub: "npub1full".into(),
            display_name: Some("Renamed".into()),
            bio: None,
            relay_list: Some(vec!["wss://new-relay.example".into()]),
            tier: Some(2),
            last_seen: Some(1_700_001_000),
            declared_capabilities: None,
            lifecycle_stage: Some("hibernating".into()),
            sui_object_id: None,
            bipon39_hint: None,
            new_version: 2,
            updated_at: 1_700_001_000,
            signature: "sig2".into(),
        };
        let next = apply_delta(&state, &delta).expect("apply ok");
        assert_eq!(next.version, 2);
        assert_eq!(next.display_name, "Renamed");
        assert_eq!(next.lifecycle_stage, "hibernating");
        assert_eq!(next.tier, 2);

        let summary = summarize(&next);
        assert_eq!(summary.version, 2);
        assert_eq!(summary.lifecycle_stage, "hibernating");
    }
}
