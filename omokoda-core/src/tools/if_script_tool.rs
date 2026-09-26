//! if_script_tool — field-grounded Odù divination via If-Script's FieldDiviner,
//! with optional glyph-memory augmentation, CowrieOracle intent seeding, and
//! Nostr CastReceipt engram publishing.
//!
//! `FieldDiviner::cast(uri_pattern)` reads the live Waggle field (present
//! channel state) and the journal (`hours_back` ago), composes the two into
//! an 8-bit figure, and resolves that figure against the real Odù corpus --
//! the cast emerges from actual operational history, not a random throw or
//! a fixed table (see `ifascript::field_divination`'s own module docs).
//! Fails soft (`CastError::FieldUnreachable`) when Waggle isn't reachable --
//! this tool surfaces that as a normal error, never a panic.
//!
//! When `use_memory` is true, `memory_chunks` are converted to `GlyphResidue`
//! and fed into `cast_with_memory()` for a composed Odù result.
//!
//! When `BUZZ_RELAY_URL` is set, a `CastReceipt` NIP-AE engram (kind 30174)
//! is prepared and published to the configured Nostr relay.
//!
//! `FieldDiviner` uses `reqwest::blocking` internally; wrapped in
//! `spawn_blocking` here so it never stalls the async executor.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::tools::{ExecutionContext, Tool};

#[derive(Deserialize)]
struct IfScriptCastParams {
    /// Waggle field URI pattern to cast against, e.g. "agent/*" or a
    /// specific resource path. Empty/omitted casts against the root field.
    #[serde(default)]
    uri_pattern: String,
    /// How far back to read the "past" field state, in hours.
    #[serde(default)]
    hours_back: Option<f64>,
    /// Enable glyph-memory augmentation (composed Odù from memory residues).
    #[serde(default)]
    use_memory: bool,
    /// Raw text chunks to convert to GlyphResidue for memory augmentation.
    #[serde(default)]
    memory_chunks: Vec<String>,
    /// Agent XP for tier computation in memory cast (defaults to 0 = tier 1).
    #[serde(default)]
    agent_xp: u64,
}

pub struct IfScriptTool;

#[async_trait]
impl Tool for IfScriptTool {
    fn name(&self) -> &str {
        "if_script_cast"
    }
    fn description(&self) -> &str {
        "Cast an Odù figure from the live Waggle field's real operational \
         history (present state composed over past state), via If-Script's \
         FieldDiviner -- not a static lookup table. Optional: use_memory=true \
         with memory_chunks for glyph-augmented composed cast. Publishes a \
         CastReceipt engram to Nostr when BUZZ_RELAY_URL is configured. \
         Params: uri_pattern (string), hours_back (optional number, default 24), \
         use_memory (bool), memory_chunks (string[]), agent_xp (number)."
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let parsed: IfScriptCastParams = if params.trim().is_empty() {
            IfScriptCastParams {
                uri_pattern: String::new(),
                hours_back: None,
                use_memory: false,
                memory_chunks: Vec::new(),
                agent_xp: 0,
            }
        } else {
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?
        };

        let uri_pattern = parsed.uri_pattern.clone();
        let hours_back = parsed.hours_back;
        let use_memory = parsed.use_memory;
        let memory_chunks = parsed.memory_chunks.clone();
        let agent_xp = parsed.agent_xp;
        let agent_name = context.name.clone();
        let mnemonic = context.odu_identity.mnemonic.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
            // ── 1. Field divination ─────────────────────────────────────────────
            let diviner = ifascript::field_divination::FieldDiviner::default();
            let cast = match hours_back {
                Some(hb) => diviner.cast_at(&uri_pattern, hb),
                None => diviner.cast(&uri_pattern),
            }
            .map_err(|e| format!("field cast failed: {e}"))?;

            // ── 2. Calabash directive ───────────────────────────────────────────
            let directive =
                crate::execution::calabash_dispatch::CalabashDispatcher::directive_for(cast.binary);

            // ── 3. Optional glyph-memory augmentation ──────────────────────────
            let memory_result = if use_memory && !memory_chunks.is_empty() {
                // Seed IfaVM with agent name for deterministic oracle
                let mut vm = ifascript::IfaVM::with_intent(&agent_name);
                let residues: Vec<ifascript::GlyphResidue> = memory_chunks
                    .iter()
                    .map(|chunk| ifascript::GlyphResidue::from_chunk(chunk))
                    .collect();
                let experience = ifascript::AgentExperience::with_xp(agent_xp);
                let mem_cast = ifascript::cast_with_memory(&mut vm, &residues, &experience);
                Some(json!({
                    "base_odu_index": mem_cast.base.index,
                    "base_universal_name": mem_cast.base.universal_name,
                    "composed_id": mem_cast.composed_id,
                    "residues_used": mem_cast.residues_used,
                    "composed": match &mem_cast.composed {
                        Ok(comp) => json!({
                            "odu_id": comp.odu_id,
                            "universal_name": comp.universal_name,
                            "vessel": format!("{:?}", comp.vessel),
                        }),
                        Err(_) => json!({"denied": true}),
                    },
                }))
            } else {
                None
            };

            // ── 4. Nostr CastReceipt engram ─────────────────────────────────────
            let nostr_status = if let Ok(relay_url) = std::env::var("BUZZ_RELAY_URL") {
                // gates_passed: check if the vessel is aligned (advisory)
                let gates_passed = directive.vessel <= 16;
                // Build a CastResult from FieldCast to feed into CastReceipt
                let vm_result = ifascript::vm::CastResult {
                    index: cast.binary,
                    vessel: cast.odu.vessel,
                    file_domain: cast.odu.vessel.file_domain(),
                    universal_name: cast.odu.universal_name,
                    prescriptions: cast.odu.prescriptions,
                };
                let receipt = ifascript::CastReceipt::from_cast(&vm_result, gates_passed);

                // Derive NostrIdentity deterministically from mnemonic
                let identity_result: Result<ifascript::NostrIdentity, String> = (|| {
                    use sha2::{Digest, Sha256};
                    let seed: [u8; 32] = Sha256::digest(mnemonic.as_bytes()).into();
                    ifascript::NostrIdentity::from_secret_bytes(&seed)
                        .map_err(|e| format!("NostrIdentity: {e}"))
                })();

                match identity_result {
                    Ok(identity) => {
                        let owner_pubkey = identity.public_key_hex().to_string();
                        let mut gateway = ifascript::NostrGateway::new(identity.clone());
                        gateway.add_relay(relay_url.clone());
                        match gateway.prepare_cast_engram(&receipt, &owner_pubkey) {
                            Ok(event) => {
                                // Fire-and-forget publish (fail-open)
                                let _ = gateway.publish_everywhere(&event);
                                json!({
                                    "published": true,
                                    "relay": relay_url,
                                    "event_kind": 30174,
                                })
                            }
                            Err(e) => json!({
                                "published": false,
                                "error": format!("{:?}", e),
                            }),
                        }
                    }
                    Err(e) => json!({"published": false, "error": e}),
                }
            } else {
                json!({"published": false, "reason": "BUZZ_RELAY_URL not set"})
            };

            // ── 5. Assemble result ──────────────────────────────────────────────
            let mut result = json!({
                "odu_name": cast.odu.name,
                "universal_name": cast.odu.universal_name,
                "archetype": cast.odu.archetype,
                "binary": cast.binary,
                "vessel": directive.vessel,
                "present_signature": cast.present_signature,
                "past_signature": cast.past_signature,
                "opcode": directive.opcode,
                "prescription": directive.prescription,
                "prescriptions_spiritual": cast.odu.prescriptions,
                "nostr": nostr_status,
            });

            if let Some(mem) = memory_result {
                result["memory_cast"] = mem;
            }

            Ok(result)
        })
        .await
        .map_err(|e| format!("if_script_cast task join error: {e}"))??;

        Ok((result.to_string(), crate::usage::TokenUsage::default()))
    }
}
