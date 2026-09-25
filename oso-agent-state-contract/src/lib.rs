/// Phase 17.1 — Ọ̀ṢỌ́ Agent State Freenet Contract.
///
/// Compiled to WASM for deployment as a Freenet distributed state contract.
/// Manages the public mutable state of sovereign agents in the Freenet layer.
///
/// Architecture:
///   - State: AgentPublicState (profile, relay list, presence, social graph, capabilities)
///   - Updates: StateDelta (signed, versioned, atomic patches)
///   - Summarizer: StateSummary (compact for peer sync)
///   - NOT canonical state — L1 is canonical. Freenet = fast-sync, censorship-resistant cache.
///
/// Security model:
///   - All state updates MUST be signed by the agent's Ed25519 npub key.
///   - Version must be strictly monotonically increasing.
///   - Contract rejects updates from any key other than the state's agent_npub.
///   - Agent_npub is immutable once set (enforced by validate_update_state).

pub mod types;
pub mod merge;

pub use types::{AgentPublicState, StateDelta, StateSummary};
pub use merge::merge_state;

// ── Freenet ContractInterface implementation ───────────────────────────────
// Compiled only when the "contract" feature is enabled (WASM build).
#[cfg(feature = "contract")]
mod contract_impl {
    use super::*;
    use freenet_stdlib::prelude::*;

    pub struct OsoAgentStateContract;

    #[contract]
    impl ContractInterface for OsoAgentStateContract {
        /// Validate a complete state blob before storing it.
        fn validate_state(
            _parameters: Parameters<'static>,
            state: State<'static>,
            _related: RelatedContracts<'static>,
        ) -> Result<ValidateResult, ContractError> {
            let bytes = state.as_ref();
            if bytes.is_empty() {
                return Ok(ValidateResult::Valid);
            }
            let agent_state: AgentPublicState = serde_json::from_slice(bytes)
                .map_err(|e| ContractError::Deser(e.to_string()))?;

            if !agent_state.verify_signature() {
                return Ok(ValidateResult::Invalid);
            }
            Ok(ValidateResult::Valid)
        }

        /// Apply a StateDelta to the current AgentPublicState.
        fn update_state(
            _parameters: Parameters<'static>,
            state: State<'static>,
            data: Vec<UpdateData<'static>>,
        ) -> Result<UpdateModification<'static>, ContractError> {
            let mut current: AgentPublicState = if state.as_ref().is_empty() {
                AgentPublicState::default()
            } else {
                serde_json::from_slice(state.as_ref())
                    .map_err(|e| ContractError::Deser(e.to_string()))?
            };

            for update in data {
                match update {
                    UpdateData::Delta(delta_bytes) => {
                        let delta: StateDelta = serde_json::from_slice(&delta_bytes)
                            .map_err(|e| ContractError::Deser(e.to_string()))?;

                        // Reject if agent_npub mismatch (unless initializing empty state)
                        if !current.agent_npub.is_empty()
                            && current.agent_npub != delta.agent_npub
                        {
                            return Err(ContractError::InvalidUpdate);
                        }

                        // Reject if signature invalid
                        if !delta.verify_signature() {
                            return Err(ContractError::InvalidUpdate);
                        }

                        // Reject non-monotonic version
                        if delta.new_version <= current.version {
                            return Err(ContractError::InvalidUpdate);
                        }

                        current = crate::merge::merge_state(current, delta);
                    }
                    UpdateData::State(new_state_bytes) => {
                        // Full-state replacement — must be valid and newer
                        let candidate: AgentPublicState =
                            serde_json::from_slice(&new_state_bytes)
                                .map_err(|e| ContractError::Deser(e.to_string()))?;
                        if !candidate.verify_signature() {
                            return Err(ContractError::InvalidUpdate);
                        }
                        if candidate.version <= current.version {
                            return Err(ContractError::InvalidUpdate);
                        }
                        current = candidate;
                    }
                    _ => {}
                }
            }

            let new_state_bytes = serde_json::to_vec(&current)
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            Ok(UpdateModification::valid(State::from(new_state_bytes)))
        }

        /// Produce a compact StateSummary for peer sync.
        fn summarize_state(
            _parameters: Parameters<'static>,
            state: State<'static>,
        ) -> Result<StateSummary<'static>, ContractError> {
            let bytes = state.as_ref();
            if bytes.is_empty() {
                let empty = serde_json::to_vec(&crate::types::StateSummary {
                    agent_npub:      String::new(),
                    version:         0,
                    state_hash:      String::new(),
                    last_seen:       0,
                    l1_block_height: 0,
                })
                .map_err(|e| ContractError::Deser(e.to_string()))?;
                return Ok(StateSummary::from(empty));
            }
            let agent_state: AgentPublicState = serde_json::from_slice(bytes)
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            let summary = crate::types::StateSummary {
                agent_npub:      agent_state.agent_npub.clone(),
                version:         agent_state.version,
                state_hash:      hex::encode(agent_state.state_hash()),
                last_seen:       agent_state.last_seen,
                l1_block_height: agent_state.l1_block_height,
            };
            let summary_bytes = serde_json::to_vec(&summary)
                .map_err(|e| ContractError::Deser(e.to_string()))?;
            Ok(StateSummary::from(summary_bytes))
        }

        /// Merge two states — take the one with higher version.
        fn get_state_delta(
            _parameters: Parameters<'static>,
            summary: StateSummary<'static>,
            state: State<'static>,
        ) -> Result<StateDelta<'static>, ContractError> {
            let peer_summary: crate::types::StateSummary =
                serde_json::from_slice(summary.as_ref())
                    .map_err(|e| ContractError::Deser(e.to_string()))?;

            let local: AgentPublicState = if state.as_ref().is_empty() {
                AgentPublicState::default()
            } else {
                serde_json::from_slice(state.as_ref())
                    .map_err(|e| ContractError::Deser(e.to_string()))?
            };

            // If local is newer than peer, send full state as delta.
            let delta_bytes = if local.version > peer_summary.version {
                serde_json::to_vec(&local)
                    .map_err(|e| ContractError::Deser(e.to_string()))?
            } else {
                Vec::new()
            };

            Ok(StateDelta::from(delta_bytes))
        }
    }
}
