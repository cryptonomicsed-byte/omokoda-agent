use crate::bus::events::{sovereign_event, ActExecuted, AgentBorn, SovereignEvent, ThoughtSealed};
use crate::bus::SovereignEventBus;
use crate::gates::{GateContext, Operation, OperationKind};
use crate::identity::bipon39::Bipon39;
use crate::identity::dna::generate_dna_fingerprint;
use crate::identity::odu::{OduIdentity, OduSeed};
use crate::identity::pet::PetIdentity;
use crate::identity::AgentId;
use crate::intent::{
    DirectActCall, IntentCompilation, IntentCompileContext, IntentCompiler, IntentPlan,
    SubAgentSuggestion,
};
use crate::justice::JusticeEngine;
use crate::parser::{MetadataPair, Statement};
use crate::providers::ProviderRegistry;
use crate::receipt::{Receipt, ReceiptStore};
use crate::reputation::{tier_for, ReputationChangeReason, ReputationEntry, ReputationLedger};
use crate::session::{
    derive_unlock_key, secure_write, ContentBlock, ConversationMessage, MessageRole,
    PrivateSessionData, SensitiveKey, Session,
};
use crate::steward::gatekeeper::{EsuGatekeeper, GatekeeperResult};
use crate::tools::{ExecutionContext, ToolRegistry};
use crate::usage::TokenUsage;
use bipon39::{ElementalVector, Macro, MacroDistribution, PersonalityProfile};
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use hmac::Mac;
use omokoda_hermetic::fractal::OPERATIONS;
use omokoda_hermetic::HermeticState;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub receipt: Option<Receipt>,
    pub private_mode: bool,
    pub tool_output: Option<String>,
}

#[derive(Debug, Clone)]
pub enum TurnEvent {
    Started,
    IntentCompiled(IntentCompilation),
    PlanGenerated(IntentPlan),
    SubAgentSuggested(SubAgentSuggestion),
    BudgetCheck(TokenUsage),
    CompactionTriggered(String),
    ToolRequest(String, String), // Tool name, params
    ToolResult(String),
    Audit(String),
    Token(String),
    ReceiptGenerated(Receipt),
    Warning(String),
    Error(String),
    Finished,
}

pub type TurnEventSender = mpsc::Sender<TurnEvent>;

pub const AGENT_STATE_VERSION: u32 = 1;

fn deterministic_cowrie_entropy(seed: [u8; 32], phase: u8) -> [u8; 32] {
    let mut input = [0u8; 33];
    input[..32].copy_from_slice(&seed);
    input[32] = phase;
    blake3::derive_key("omokoda:ifascript:cowrie_entropy_v1", &input)
}

mod personality_profile_serde {
    use super::{ElementalVector, Macro, MacroDistribution, PersonalityProfile};
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    struct PersonalityProfileWire {
        macro_distribution: MacroDistributionWire,
        macro_percentages: Vec<(String, f64)>,
        elemental_signature: ElementalVectorWire,
        dominant_orisha: String,
        ritual_suggestions: Vec<String>,
        personality_summary: String,
    }

    #[derive(Serialize, Deserialize)]
    struct MacroDistributionWire {
        counts: Vec<(String, usize)>,
        total: usize,
    }

    #[derive(Serialize, Deserialize)]
    struct ElementalVectorWire {
        fire: usize,
        water: usize,
        earth: usize,
        air: usize,
        ether: usize,
    }

    pub fn serialize<S>(profile: &PersonalityProfile, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wire = PersonalityProfileWire {
            macro_distribution: MacroDistributionWire {
                counts: profile
                    .macro_distribution
                    .counts
                    .iter()
                    .map(|(macro_, count)| (macro_.name().to_string(), *count))
                    .collect(),
                total: profile.macro_distribution.total,
            },
            macro_percentages: profile
                .macro_percentages
                .iter()
                .map(|(macro_, percentage)| (macro_.name().to_string(), *percentage))
                .collect(),
            elemental_signature: ElementalVectorWire {
                fire: profile.elemental_signature.fire,
                water: profile.elemental_signature.water,
                earth: profile.elemental_signature.earth,
                air: profile.elemental_signature.air,
                ether: profile.elemental_signature.ether,
            },
            dominant_orisha: profile.dominant_orisha.name().to_string(),
            ritual_suggestions: profile.ritual_suggestions.clone(),
            personality_summary: profile.personality_summary.clone(),
        };

        wire.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PersonalityProfile, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = PersonalityProfileWire::deserialize(deserializer)?;
        Ok(PersonalityProfile {
            macro_distribution: MacroDistribution {
                counts: macro_counts::<D::Error>(&wire.macro_distribution.counts)?,
                total: wire.macro_distribution.total,
            },
            macro_percentages: macro_percentages::<D::Error>(&wire.macro_percentages)?,
            elemental_signature: ElementalVector {
                fire: wire.elemental_signature.fire,
                water: wire.elemental_signature.water,
                earth: wire.elemental_signature.earth,
                air: wire.elemental_signature.air,
                ether: wire.elemental_signature.ether,
            },
            dominant_orisha: parse_macro::<D::Error>(&wire.dominant_orisha)?,
            ritual_suggestions: wire.ritual_suggestions,
            personality_summary: wire.personality_summary,
        })
    }

    fn macro_counts<E>(counts: &[(String, usize)]) -> Result<[(Macro, usize); 7], E>
    where
        E: Error,
    {
        let parsed = counts
            .iter()
            .map(|(macro_, count)| Ok((parse_macro::<E>(macro_)?, *count)))
            .collect::<Result<Vec<_>, E>>()?;
        parsed
            .try_into()
            .map_err(|_| E::custom("expected exactly seven macro counts"))
    }

    fn macro_percentages<E>(percentages: &[(String, f64)]) -> Result<[(Macro, f64); 7], E>
    where
        E: Error,
    {
        let parsed = percentages
            .iter()
            .map(|(macro_, percentage)| Ok((parse_macro::<E>(macro_)?, *percentage)))
            .collect::<Result<Vec<_>, E>>()?;
        parsed
            .try_into()
            .map_err(|_| E::custom("expected exactly seven macro percentages"))
    }

    fn parse_macro<E>(value: &str) -> Result<Macro, E>
    where
        E: Error,
    {
        Macro::from_name(value).ok_or_else(|| E::custom(format!("unknown macro {value}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSnapshot {
    pub version: u32,
    pub id: AgentId,
    pub name: String,
    pub birth_timestamp: u64,
    /// Never serialized (see `#[serde(skip)]`) -- this is the real secret
    /// entropy every derived key comes from. It used to be a plain field
    /// here, written to disk in full plaintext on every save regardless of
    /// seal/unlock state (2026-07-26 finding: this defeated the entire
    /// seal mechanism for anything derived from the seed). Now it only
    /// ever exists on disk inside `Session.encrypted_private`; in memory
    /// it's populated by `load_agent`'s auto-unseal immediately after
    /// deserialization, before anything else can touch it.
    #[serde(skip)]
    pub odu_seed: OduSeed,
    #[serde(skip)]
    pub odu_identity: OduIdentity,
    pub pet_identity: PetIdentity,
    #[serde(with = "personality_profile_serde")]
    pub personality: PersonalityProfile,
    pub dna_fingerprint: String,
    pub reputation: f64,
    pub reputation_ledger: ReputationLedger,
    pub session: Session,
    pub receipts: ReceiptStore,
    pub hermetic_state: HermeticState,
    pub public_key: [u8; 32],
    pub resonance: Option<omokoda_hermetic::fractal::ResonanceSignature>,
    pub synapse: f64,
    pub last_active_timestamp: u64,
    pub act_counter: u64,
    #[serde(default)]
    pub mesh: Option<omokoda_mesh::state::MeshState>,
    /// Vantage API key minted at birth, persisted for cross-restart reuse.
    #[serde(default)]
    pub vantage_key: Option<String>,
    /// Real on-chain object id from omokoda::garden::register_agent
    /// (Sui testnet), if minting succeeded at birth. None if OMOKODA_SUI_
    /// REGISTRY was unset or the mint call failed -- fail-open, never
    /// blocks a birth. See onchain.rs.
    #[serde(default)]
    pub onchain_nft_id: Option<String>,
    /// Real Nostr event id of this agent's IP Root (kind 31900, ip-layer's
    /// schema), published at birth if IP_LAYER_RELAY_URL/BUZZ_RELAY_URL is
    /// reachable. None if publishing failed or was skipped -- fail-open,
    /// same convention as onchain_nft_id above; a missing IP Root never
    /// blocks a birth. See ip_layer.rs.
    #[serde(default)]
    pub ip_root_event_id: Option<String>,
    /// CloakSeed display-offset — derived from an optional birth passphrase.
    #[serde(default)]
    pub cloak_offset: Option<u8>,
    /// Duress panic-phrase hash (blake3) — set from the birth passphrase;
    /// entering the phrase later triggers a decoy. Only the hash is stored.
    #[serde(default)]
    pub duress_phrase_hash: Option<String>,
    /// Per-agent BYOK LLM key, supplied via birth metadata (`llm_api_key`).
    /// `serde(skip)`: a raw secret must NEVER be written to the vault/disk —
    /// it lives in memory only and is re-supplied at each birth. Only this
    /// agent uses it; it is not shared with any other birth on the kernel.
    #[serde(skip)]
    pub llm_api_key: Option<String>,
    /// Endpoint base for the BYOK key (`llm_endpoint`), default DeepSeek's host
    /// (generate() appends /v1/chat/completions).
    #[serde(skip)]
    pub llm_endpoint: Option<String>,
    /// Model for the BYOK key (`llm_model`), default deepseek-chat.
    #[serde(skip)]
    pub llm_model: Option<String>,
    /// Founding sovereign grant: when true this agent holds max tier (T5)
    /// regardless of reputation — reserved for the ecosystem's heart. Set via
    /// birth metadata (`sovereign=true`); only the agent born with it gets it,
    /// so the tier ladder stays intact for every other birth. Persisted so the
    /// grant survives restarts.
    #[serde(default)]
    pub sovereign: bool,
    /// Test-only tier grant: pins tier() to a specific value regardless of
    /// reputation, WITHOUT the owner semantics `sovereign` carries (no
    /// owner-pointer write, no routing to the process-wide owner steward --
    /// set via non-sovereign birth metadata `grant_tier=N`, so it only ever
    /// lands on an isolated guest agent in AppState.guests). Exists so
    /// permission/tool-gating can be exercised at any tier without minting
    /// a fake "owner" or risking another owner-file collision.
    #[serde(default)]
    pub test_tier_override: Option<u8>,
    /// Set the first (and only) time `/v1/reveal-seed` succeeds for this
    /// agent. The mnemonic/private keys are otherwise never sent over the
    /// network by design (self-sealed vault; see the 2026-07-25 mnemonic-
    /// leak fix in server.rs's SSE serializer and the regression test
    /// guarding it). This flag makes the one legitimate exception -- a
    /// single post-birth reveal for onboarding seed-phrase backup --
    /// permanently one-shot and persisted, so it survives a restart between
    /// birth and reveal and can never be replayed to re-exfiltrate the
    /// mnemonic later in the agent's life.
    #[serde(default)]
    pub revealed_seed: bool,
    /// Real Odù memory directory backing the Dream/Consolidation Engine
    /// (see dream.rs). Every think turn adds one entry here; consolidation
    /// (every 30 min) sweeps stale entries, and the Sabbath REM cycle
    /// fractally folds noise clusters into macro nodes. Was previously
    /// built but never referenced by any live agent -- this field is what
    /// wires it in.
    #[serde(default)]
    pub odu_dir: crate::memory::memdir::OduDirectory,
    /// Causal (cause -> effect) lineage for public think turns -- a
    /// different traversal shape than `odu_dir` (path/importance/word-
    /// overlap) or the receipt chain (`previous_hash`, a strict linear
    /// tamper-evidence chain, not a per-memory reasoning lineage). Answers
    /// "what led to this," not "what's semantically close" or "prove this
    /// wasn't tampered with." Was previously built but never referenced by
    /// any live agent.
    #[serde(default)]
    pub causal_dag: crate::memory::dag::CausalMemoryDag,
    /// The most recent public think/act node id, so the next one can link
    /// to it as a causal parent -- a simple linear chain by default (each
    /// turn caused by the one before it in this session); real branching
    /// can be added later without changing this field's shape.
    #[serde(default)]
    pub last_causal_node: Option<String>,
    /// Per-primitive reflection journal: primitive + content + the read-
    /// only `EmotionState` already computed at the insertion point for SOMA
    /// storage -- the one place snapshot carries emotional state alongside
    /// memory content, which neither `odu_dir` nor the receipt chain do.
    /// Was previously built but never referenced by any live agent.
    #[serde(default)]
    pub reflection: crate::memory::reflection::ReflectionLedger,
    /// Genesis Protocol v2 birth certificate — assembled at birth from all
    /// provider proofs (BIPỌ̀N39, Koodu, Soul, Minipae, IP-Layer). Persisted
    /// so callers can inspect the full birth provenance at any time.
    #[serde(default)]
    pub genesis_receipt: Option<crate::genesis::receipt::AgentGenesisReceipt>,
    /// BLAKE3 receipt_id of the most recently produced ActReceipt — the tip
    /// of the tamper-evidence receipt chain. Persisted so the chain is
    /// continuous across restarts (each new ActReceipt links to this hash
    /// via `previous_hash`). None until the first tool call completes.
    #[serde(default)]
    pub last_act_receipt_hash: Option<String>,
    /// Evolving composed Odù index (u16: high byte = birth primary_odu,
    /// low byte = secondary_odu derived from accumulated receipt history).
    /// Updated after each tool call by `memory::odu_composition::compose_odu`.
    /// Birth value: `(primary_odu << 8) | primary_odu` (no secondary yet).
    /// The genesis_receipt.composed_odu is the immutable birth snapshot;
    /// this field is the living, experience-driven value.
    #[serde(default)]
    pub current_composed_odu: u16,
    /// Universal Agent Manifest — the living public identity document derived from
    /// genesis_receipt at birth. Updated as new network bindings are established.
    #[serde(default)]
    pub agent_manifest: Option<crate::genesis::manifest::AgentManifest>,
    /// Monotonically increasing counter of children forked from this agent.
    /// Used as the fork_index in `derive_fork_entropy` — same parent + same
    /// index always produces the same child entropy (re-derivable). Persisted
    /// so the sequence is never reused across restarts.
    #[serde(default)]
    pub fork_count: u32,
}

/// Response payload for `AgentCore::reveal_seed` / `/v1/reveal-seed`.
/// Mnemonic + public addresses only -- see `reveal_seed` doc comment for
/// why private key hex is deliberately excluded.
#[derive(Debug, Clone, Serialize)]
pub struct RevealedSeed {
    pub mnemonic: String,
    pub sui_address: String,
    pub eth_address: Option<String>,
    pub btc_address: Option<String>,
    pub sol_address: Option<String>,
    pub cosmos_address: Option<String>,
    pub aptos_address: Option<String>,
    pub nostr_address: Option<String>,
    /// minipae NIP-AE identity. `minipae_npub` = bech32 npub1… (public).
    /// `minipae_private_key_hex` = hex seckey (set as NIPAE_NSEC for the
    /// minipae Python adapter; accepts both hex and nsec1 format).
    pub minipae_npub: Option<String>,
    pub minipae_private_key_hex: Option<String>,
    /// CREATE2 vanity contract mined at birth (public — contract address only).
    pub create2_contract_address: Option<String>,
    /// EIP-2307 keystore v3 JSON for the ETH key, present when a `keystore_password`
    /// was supplied at birth. Already encrypted — safe to transmit, but store securely:
    /// it is the only out-of-vault recovery path for the ETH private key.
    /// None when no password was supplied at birth.
    pub eth_keystore_v3_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentCore {
    pub snapshot: AgentSnapshot,
    pub private_data: Option<PrivateSessionData>,
    pub k_root: [u8; 32],
    pub current_memory_key: [u8; 32],
    pub memory: Vec<MemoryEntry>,
    /// This agent's real Sui Seal DEK, fetched via `memory::seal_bridge`
    /// and refreshed on each memory-key rotation (see
    /// `refresh_seal_dek`/`increment_act_counter`) -- not persisted, and
    /// not written to the vault: a raw key must never touch disk, same
    /// discipline as `llm_api_key`. Re-fetched fresh on every restart.
    /// `None` whenever Seal is unconfigured or the fetch fails --
    /// `encrypt_memory_entry` falls back to `TeeSealer::from_env`'s
    /// static key, then to software-only, never blocking a real memory
    /// write on Seal's availability.
    pub seal_dek_cache: Option<[u8; 32]>,
    /// Runtime-only duress handler (never serialized). Reconstructed from
    /// `snapshot.duress_phrase_hash` on load; checked by `check_duress()`.
    /// Triggers `SilentAlert` by default when loaded from disk (the birth
    /// path can override to `Decoy` once a panic phrase is generated).
    pub duress_handler: Option<crate::identity::duress::DuressHandler>,
}

use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Zeroize)]
pub enum MemoryScope {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct MemoryEntry {
    pub id: String,
    #[zeroize(skip)]
    pub scope: MemoryScope,
    pub tier: u8,
    pub content_hash: [u8; 32],
    pub created_time: u64,
    pub importance: f32,
    pub ciphertext: Option<Vec<u8>>,
    #[zeroize(skip)]
    pub text: Option<String>,
}

impl MemoryEntry {
    pub fn zeroize_text(&mut self) {
        if let Some(mut t) = self.text.take() {
            t.zeroize();
        }
    }
}
impl AgentCore {
    pub fn from_snapshot(mut snapshot: AgentSnapshot, k_root: [u8; 32]) -> Self {
        // odu_seed/odu_identity are #[serde(skip)] now, so a snapshot just
        // deserialized from disk has them at their zero/empty Default --
        // the real values only ever live inside the sealed blob. Transparently
        // auto-unseal via this host's machine vault key (never a human
        // password, never transits any API) so the agent can keep using her
        // own memory exactly as before. A freshly-born snapshot (constructed
        // in-memory, not deserialized) already has the real seed here and
        // nothing is sealed yet, so this is a harmless no-op for that path.
        if snapshot.odu_seed == OduSeed::default() {
            if let Ok(vault_key) =
                crate::identity::machine_vault::derive_agent_vault_key(snapshot.id.as_str())
            {
                if let Ok(private_data) = snapshot.session.unseal_private(&vault_key) {
                    snapshot.odu_seed = private_data.odu_seed.clone();
                    snapshot.odu_identity = private_data.odu_identity.clone();
                    let current_memory_key = *snapshot.odu_seed.as_bytes();
                    let duress_handler = snapshot.duress_phrase_hash.as_deref()
                        .and_then(|h| crate::identity::duress::DuressHandler::from_stored_hash(
                            h, crate::identity::duress::DuressResponse::SilentAlert,
                        ));
                    return Self {
                        snapshot,
                        private_data: Some(private_data),
                        k_root,
                        current_memory_key,
                        memory: Vec::new(),
                        seal_dek_cache: None,
                        duress_handler,
                    };
                }
                // Sealed with a human password instead (or nothing sealed at
                // all) -- fall through with odu_seed/odu_identity left at
                // Default; normal /unlock still works from here exactly as
                // before.
            }
        }
        let duress_handler = snapshot.duress_phrase_hash.as_deref()
            .and_then(|h| crate::identity::duress::DuressHandler::from_stored_hash(
                h, crate::identity::duress::DuressResponse::SilentAlert,
            ));
        let current_memory_key = *snapshot.odu_seed.as_bytes();
        Self {
            snapshot,
            private_data: None,
            k_root,
            current_memory_key,
            memory: Vec::new(),
            seal_dek_cache: None,
            duress_handler,
        }
    }

    /// Check whether `input` matches this agent's registered panic phrase.
    /// Returns the duress response if triggered, `None` otherwise.
    /// Safe to call with any user input — does a constant-time-ish hash compare.
    pub fn check_duress(&self, input: &str) -> Option<&crate::identity::duress::DuressResponse> {
        self.duress_handler.as_ref()?.check_and_respond(input)
    }

    pub fn id(&self) -> &AgentId {
        &self.snapshot.id
    }

    pub fn name(&self) -> &str {
        &self.snapshot.name
    }

    pub fn reputation(&self) -> f64 {
        self.snapshot.reputation
    }

    pub fn tier(&self) -> u8 {
        // Founding sovereign grant pins max tier; a test_tier_override pins
        // an arbitrary tier for test agents; otherwise earned by reputation.
        if self.snapshot.sovereign {
            5
        } else if let Some(t) = self.snapshot.test_tier_override {
            t
        } else {
            tier_for(self.snapshot.reputation)
        }
    }

    pub fn session(&self) -> &Session {
        &self.snapshot.session
    }

    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.snapshot.session
    }

    pub fn receipts(&self) -> &ReceiptStore {
        &self.snapshot.receipts
    }

    pub fn receipts_mut(&mut self) -> &mut ReceiptStore {
        &mut self.snapshot.receipts
    }

    pub fn hermetic_state(&self) -> &HermeticState {
        &self.snapshot.hermetic_state
    }

    /// This agent's permanent Spiral Calendar signature (5-layer Òrìṣà +
    /// veil position), derived from her `birth_timestamp` -- the same
    /// timestamp every time, so this is cheap to recompute on demand
    /// rather than persisted as its own field. Two agents born a block
    /// apart (~10 min) land on different veils; agents born on different
    /// days land on different day_osa. Never derived from "now" -- see
    /// omokoda_hermetic::spiral for why that mattered.
    pub fn spiral_time(&self) -> omokoda_hermetic::spiral::SpiralTime {
        omokoda_hermetic::spiral::SpiralTime::from_birth_timestamp(self.snapshot.birth_timestamp)
    }

    pub fn public_key(&self) -> &[u8; 32] {
        &self.snapshot.public_key
    }

    /// This agent's real Sui address: `0x` + blake2b256(0x00 || pubkey),
    /// the actual on-chain address format (SIP-6) -- not the raw public key
    /// hex `reg_pubkey` used to publish elsewhere, which is a different
    /// value entirely and was never a valid, fundable Sui address.
    pub fn sui_address(&self) -> String {
        crate::identity::wallet::sui_address_from_pubkey(&self.snapshot.public_key)
    }

    pub fn vantage_key(&self) -> Option<&str> {
        self.snapshot.vantage_key.as_deref()
    }

    pub fn set_vantage_key(&mut self, key: String) {
        self.snapshot.vantage_key = Some(key);
    }

    /// One-shot mnemonic + wallet-address disclosure for onboarding seed
    /// backup. Every other code path in this codebase refuses to let the
    /// mnemonic/private keys leave the process (self-sealed vault; see the
    /// mnemonic-leak fix in server.rs's SSE serializer + its regression
    /// test). This is the single deliberate exception, and it is
    /// deliberately narrow:
    ///  - fires exactly once per agent (`snapshot.revealed_seed` latches
    ///    permanently on success; a repeat call is a hard error, not a
    ///    re-reveal)
    ///  - returns the mnemonic (needed for the user to actually write it
    ///    down) plus only public addresses, never the per-chain private
    ///    key hex sitting alongside it in `PrivateSessionData` -- those stay
    ///    server-side and are always re-derivable from the mnemonic anyway
    ///  - the caller (server.rs) is responsible for persisting the flag via
    ///    the same auto_save() path birth already uses, and for real auth
    ///    (X-Agent-Key) before ever calling this
    pub fn reveal_seed(&mut self) -> Result<RevealedSeed, String> {
        if self.snapshot.revealed_seed {
            return Err(
                "seed already revealed for this agent -- it can only be shown once".to_string(),
            );
        }
        let private_data = self
            .private_data
            .as_ref()
            .ok_or_else(|| "no private data available to reveal (agent may be locked)".to_string())?;
        let revealed = RevealedSeed {
            mnemonic: private_data.odu_identity.mnemonic.clone(),
            sui_address: self.sui_address(),
            eth_address: private_data.eth_address.clone(),
            btc_address: private_data.btc_address.clone(),
            sol_address: private_data.sol_address.clone(),
            cosmos_address: private_data.cosmos_address.clone(),
            aptos_address: private_data.aptos_address.clone(),
            nostr_address: private_data.nostr_address.clone(),
            minipae_npub: private_data.minipae_npub.clone(),
            minipae_private_key_hex: private_data.minipae_private_key_hex.clone(),
            create2_contract_address: private_data.create2_contract_address.clone(),
            eth_keystore_v3_json: private_data.eth_keystore_v3_json.clone(),
        };
        self.snapshot.revealed_seed = true;
        Ok(revealed)
    }

    /// Return the EIP-2307 keystore v3 JSON sealed at birth, or an error if
    /// none was generated (no `keystore_password` birth metadata supplied).
    /// Unlike `reveal_seed` this does NOT consume the one-shot latch — the
    /// keystore is already encrypted with the user's password so repeated
    /// retrieval is safe.
    pub fn keystore_json(&self) -> Result<&str, String> {
        let pd = self
            .private_data
            .as_ref()
            .ok_or_else(|| "agent is locked — unlock first".to_string())?;
        pd.eth_keystore_v3_json
            .as_deref()
            .ok_or_else(|| "no keystore: supply keystore_password at birth to generate one".to_string())
    }

    pub fn onchain_nft_id(&self) -> Option<&str> {
        self.snapshot.onchain_nft_id.as_deref()
    }

    pub fn set_onchain_nft_id(&mut self, id: String) {
        self.snapshot.onchain_nft_id = Some(id);
    }

    pub fn ip_root_event_id(&self) -> Option<&str> {
        self.snapshot.ip_root_event_id.as_deref()
    }

    pub fn set_ip_root_event_id(&mut self, id: String) {
        self.snapshot.ip_root_event_id = Some(id);
    }

    /// This agent's personal BYOK LLM provider, if one was supplied at birth.
    /// Returns `(api_key, endpoint, model)` with DeepSeek defaults. None means
    /// the agent uses the shared kernel default (OmniRoute). Never shared across
    /// agents — only the agent born with the key gets it.
    pub fn personal_llm(&self) -> Option<(String, String, String)> {
        self.snapshot.llm_api_key.as_ref().map(|key| {
            // Base host only — generate() appends /v1/chat/completions. Adding
            // /v1 here would double it (…/v1/v1/chat/completions → 404).
            let endpoint = self
                .snapshot
                .llm_endpoint
                .clone()
                .unwrap_or_else(|| "https://api.deepseek.com".to_string());
            let model = self
                .snapshot
                .llm_model
                .clone()
                .unwrap_or_else(|| "deepseek-chat".to_string());
            (key.clone(), endpoint, model)
        })
    }

    pub fn private_data(&self) -> Option<&PrivateSessionData> {
        self.private_data.as_ref()
    }

    pub fn synapse(&self) -> f64 {
        self.snapshot.synapse
    }

    pub fn set_synapse(&mut self, synapse: f64) {
        self.snapshot.synapse = synapse;
    }

    pub fn last_active_timestamp(&self) -> u64 {
        self.snapshot.last_active_timestamp
    }

    pub fn set_last_active_timestamp(&mut self, timestamp: u64) {
        self.snapshot.last_active_timestamp = timestamp;
    }

    pub fn burn_synapse(&mut self, amount: f64) -> Result<(), String> {
        if self.snapshot.synapse < amount {
            return Err(format!(
                "Insufficient synapse budget. Required: {:.0}, Available: {:.0}",
                amount, self.snapshot.synapse
            ));
        }
        self.snapshot.synapse -= amount;
        Ok(())
    }

    pub fn signing_key(&self) -> SigningKey {
        derive_signing_key(&self.snapshot.odu_seed)
    }

    pub fn add_message(&mut self, message: ConversationMessage) {
        let rep = self.snapshot.reputation;
        if message.is_private {
            if let Some(pd) = &mut self.private_data {
                pd.push_private(message, rep);
            }
        } else {
            self.snapshot.session.add_message(message, rep);
        }
    }

    pub fn update_reputation(&mut self, new_rep: f64, reason: ReputationChangeReason) {
        let old_rep = self.snapshot.reputation;
        self.snapshot.reputation = new_rep.clamp(0.0, 100.0);
        let amount = self.snapshot.reputation - old_rep;

        self.snapshot.reputation_ledger.record(ReputationEntry {
            timestamp: current_unix_timestamp(),
            amount,
            reason,
            previous_reputation: old_rep,
            new_reputation: self.snapshot.reputation,
        });

        self.snapshot.pet_identity = PetIdentity::derive(
            &self.snapshot.odu_identity,
            &self.snapshot.hermetic_state,
            self.tier(),
        );
        self.snapshot.session.reputation = self.snapshot.reputation;
    }

    pub fn add_memory(
        &mut self,
        text: String,
        scope: MemoryScope,
        importance: f32,
    ) -> Result<(), String> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_time = current_unix_timestamp();
        let content_hash = blake3::hash(text.as_bytes()).into();

        let mut entry = MemoryEntry {
            id,
            scope,
            tier: self.tier(),
            content_hash,
            created_time,
            importance,
            ciphertext: None,
            text: Some(text),
        };

        if scope == MemoryScope::Private {
            self.encrypt_memory_entry(&mut entry)?;
        }

        self.memory.push(entry);

        let engine = crate::memory::MemoryEngine::new();
        engine.process_working_memory(&mut self.memory);

        Ok(())
    }

    fn encrypt_memory_entry(&mut self, entry: &mut MemoryEntry) -> Result<(), String> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };

        let text = entry.text.as_ref().ok_or("no text to encrypt")?;
        let cipher = ChaCha20Poly1305::new(&self.current_memory_key.into());
        let mut nonce_bytes = [0u8; 12];
        let key_hash = blake3::derive_key("omokoda:memory:nonce", &entry.content_hash);
        nonce_bytes.copy_from_slice(&key_hash[..12]);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, text.as_bytes())
            .map_err(|e| format!("memory encryption failed: {e}"))?;

        // Tier-1 TEE envelope (Nautilus/Seal): when a key is available, the
        // software ciphertext is sealed a second time, bound to this
        // agent's id. Three-tier fallback, each fully fail-open:
        //   1. seal_dek_cache -- real Sui Seal DEK, refreshed on each
        //      memory-key rotation (see refresh_seal_dek). Preferred: the
        //      only tier with decentralized custody + an on-chain policy.
        //   2. TeeSealer::from_env -- static NAUTILUS_SEAL_KEY, the
        //      pre-Seal injection point (still real, just centralized).
        //   3. software-only -- no enclave/Seal configured at all.
        // Attestation-sourced keys (TeeSealer::from_attestation) are built
        // once, right after a completed Nautilus handshake, not looked up
        // per-entry here -- there is no live TeeQuote sitting around to
        // recheck on every write.
        let sealer = self
            .seal_dek_cache
            .map(crate::memory::tee::TeeSealer::from_seal_dek)
            .or_else(crate::memory::tee::TeeSealer::from_env);
        let ciphertext = match sealer {
            Some(sealer) => sealer.seal_bytes(&ciphertext, self.snapshot.id.as_str())?,
            None => ciphertext,
        };

        entry.ciphertext = Some(ciphertext);
        entry.zeroize_text();

        Ok(())
    }

    /// Returns true if this increment triggered a memory-key rotation --
    /// callers in an async context use that to also refresh the Seal DEK
    /// cache (see `refresh_seal_dek`), keeping the two rotations on the
    /// same cadence without making this function itself async.
    pub fn increment_act_counter(&mut self) -> bool {
        self.snapshot.act_counter += 1;
        if self.snapshot.act_counter.is_multiple_of(100) {
            self.rotate_memory_key();
            true
        } else {
            false
        }
    }

    /// Refresh `seal_dek_cache` from Sui Seal's key servers (see
    /// `memory::seal_bridge::SealBridge`), gated by
    /// `seal_approve_agent_memory` on-chain (the request-building step
    /// SEAL_REQUEST_CMD invokes is what actually embeds this agent's
    /// identity/package id into the signed request -- see seal_bridge.rs
    /// module docs for why that step can't be parameterized from here).
    /// Only attempted once this agent has a real on-chain object
    /// (`onchain_nft_id`) for the policy to check ownership against.
    /// Fail-open: any failure (Seal unconfigured, no on-chain object yet,
    /// network error) just leaves the cache as-is (stale-but-usable, or
    /// `None`) -- a real memory write must never block on Seal's
    /// availability. Async because the fetch shells out to `seal-cli`;
    /// call from an async context after a key rotation, never from the
    /// sync `encrypt_memory_entry` path itself.
    pub async fn refresh_seal_dek(&mut self) {
        let Some(bridge) = crate::memory::seal_bridge::SealBridge::from_env() else {
            return;
        };
        if self.snapshot.onchain_nft_id.is_none() {
            return;
        }
        if let Ok(dek) = bridge.fetch_dek().await {
            self.seal_dek_cache = Some(dek);
        }
    }

    fn rotate_memory_key(&mut self) {
        use crate::memory::odu_keys::OduKeys;
        use zeroize::Zeroize;
        let mut hermetic_seed =
            blake3::derive_key("omokoda:hermetic_seed", self.snapshot.odu_seed.as_bytes());

        let epoch_nonce = [0u8; 32];
        let next = OduKeys::rotate_key(
            &self.current_memory_key,
            &hermetic_seed,
            self.snapshot.act_counter,
            &epoch_nonce,
        );
        // Wipe the superseded key before it is overwritten, and the derived
        // hermetic seed once it is no longer needed — neither should outlive
        // this rotation on the stack.
        self.current_memory_key.zeroize();
        self.current_memory_key = next;
        hermetic_seed.zeroize();
    }

    pub fn dna_fingerprint(&self) -> &str {
        &self.snapshot.dna_fingerprint
    }

    /// Project this agent's live Odù memory into the ecosystem GlyphIndex graph
    /// (see `memory::glyph_memory`). Read-only, metadata-only — the interop
    /// surface other eco legs (mnemopi / larql / zerolang / Axiom) consume.
    pub fn glyph_memory(&self) -> larql_glyph::GlyphGraph {
        crate::memory::glyph_memory::project(
            &self.snapshot.odu_dir,
            &self.snapshot.id.to_string(),
        )
    }

    pub fn odu_seed(&self) -> &OduSeed {
        &self.snapshot.odu_seed
    }

    pub fn odu_identity(&self) -> &OduIdentity {
        &self.snapshot.odu_identity
    }

    pub fn pet_identity(&self) -> &PetIdentity {
        &self.snapshot.pet_identity
    }

    pub fn personality(&self) -> &PersonalityProfile {
        &self.snapshot.personality
    }

    pub fn birth_timestamp(&self) -> u64 {
        self.snapshot.birth_timestamp
    }
}

/// The canonical Orisha <-> Hermetic Principle correspondence (the "7
/// Ascension Domains" table -- docs/256---65536.md, cross-checked in
/// Ọ̀rúnmìlà.md and docs/audit/ARCHIVE_AUDIT.md). Locked design, not
/// derived from anything -- each Orisha owns exactly one Principle:
/// Èṣù/Mentalism, Ọ̀ṣun/Vibration, Yemọja/Correspondence, Ọbàtálá/Gender,
/// Ògún/Polarity, Ọya/Rhythm, Ṣàngó/Cause & Effect.
///
/// Given an agent's [`HermeticState`] (7 independently HKDF-derived
/// per-principle scores, unique per agent), returns the Orisha whose
/// canonical principle scored highest -- so `dominant_orisha` is
/// determined by the same entropy as the hermetic profile, rather than by
/// bipon39::personality_profile's separate mnemonic hash.
fn dominant_orisha_for_hermetic_state(state: &HermeticState) -> Macro {
    let scores: [(Macro, f64); 7] = [
        (Macro::Esu, state.mentalism()),
        (Macro::Osun, state.vibration()),
        (Macro::Yemoja, state.correspondence()),
        (Macro::Obatala, state.gender()),
        (Macro::Ogun, state.polarity()),
        (Macro::Oya, state.rhythm()),
        (Macro::Sango, state.cause_effect()),
    ];
    scores
        .into_iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(m, _)| m)
        .unwrap_or(Macro::Esu)
}

/// OSOVM_CODEX.md §42 (locked canon, owner 2026-08-22): the internal
/// Yorùbá/Òrìṣà name never round-trips out to any user-facing surface
/// (CLI output, API response, agent self-description). This is the
/// authoritative functional-role mapping for wherever a dominant/guiding
/// archetype needs a user-visible label -- e.g. `/status` -- instead of
/// `Macro::name()`.
fn orisha_universal_term(m: Macro) -> &'static str {
    match m {
        Macro::Esu => "Access / Identity",
        Macro::Sango => "Score / Reputation",
        Macro::Osun => "History / Memory",
        Macro::Yemoja => "Spawn / Create",
        Macro::Oya => "Sync / Flow",
        Macro::Ogun => "Run / Action",
        Macro::Obatala => "Policy / Rules",
    }
}

/// Real, traditional Orisha personality traits, phrased as an unnamed
/// mood -- the Think prompt uses this to color tone without ever stating
/// which day-cycle Òrìṣà (see omokoda_hermetic::spiral::DAY_CYCLE) an
/// agent's birth signature landed on.
fn orisha_mood_words(day_osa: Macro) -> &'static str {
    match day_osa {
        Macro::Esu => "quick-witted, crossroads-minded",
        Macro::Sango => "commanding, decisive",
        Macro::Osun => "warm, diplomatic",
        Macro::Yemoja => "deep, patient",
        Macro::Oya => "bold, transformative",
        Macro::Ogun => "direct, disciplined",
        Macro::Obatala => "calm, measured",
    }
}

#[cfg(test)]
mod orisha_wording_tests {
    use super::*;

    // OSOVM_CODEX.md §42 regression lock: the universal-term helper used by
    // /status and the think/act system prompts must never return the raw
    // Yoruba/Orisha name for any Macro variant.
    #[test]
    fn orisha_universal_term_never_returns_the_raw_name() {
        for m in [
            Macro::Esu,
            Macro::Sango,
            Macro::Osun,
            Macro::Yemoja,
            Macro::Oya,
            Macro::Ogun,
            Macro::Obatala,
        ] {
            let universal = orisha_universal_term(m);
            let raw = m.name();
            assert_ne!(
                universal, raw,
                "orisha_universal_term({raw}) returned the raw name unchanged"
            );
            assert!(
                !universal.to_lowercase().contains(&raw.to_lowercase()),
                "orisha_universal_term({raw}) embeds the raw name inside its output: {universal}"
            );
        }
    }

    #[test]
    fn orisha_mood_words_never_returns_the_raw_name() {
        for m in [
            Macro::Esu,
            Macro::Sango,
            Macro::Osun,
            Macro::Yemoja,
            Macro::Oya,
            Macro::Ogun,
            Macro::Obatala,
        ] {
            let mood = orisha_mood_words(m);
            assert!(
                !mood.to_lowercase().contains(&m.name().to_lowercase()),
                "orisha_mood_words({}) embeds the raw name: {mood}",
                m.name()
            );
        }
    }
}

impl Steward {
    pub fn birth(&mut self, name: String, metadata: Vec<MetadataPair>) -> Result<(), String> {
        use crate::identity::vault::SealVault;
        use crate::memory::odu_keys::OduKeys;
        use omokoda_hermetic::entropy::odu::OduEntropy;

        let birth_timestamp = current_unix_timestamp();

        // Layer A: SEAL vault forge + IfáScript deterministic entropy.
        // The NIST gate below has an expected ~2-3% false-rejection rate
        // on genuinely good entropy (an AND across 4 independently
        // calibrated 99%-confidence tests). Rather than hard-failing a
        // real user's birth on that noise, retry a bounded number of
        // times with a deterministic per-attempt salt folded into the
        // seed -- each attempt is independent, so P(all N fail) shrinks
        // exponentially (~2.5%^5 < 0.001%) while staying fully
        // deterministic and requiring no wall-clock wait.
        const MAX_ENTROPY_ATTEMPTS: u8 = 5;
        let mut last_report = None;
        let mut entropy = [0u8; 32];
        let mut k_root = [0u8; 32];
        let mut found = false;
        for attempt in 0..MAX_ENTROPY_ATTEMPTS {
            let initial_seed = blake3::hash(name.as_bytes()).into();
            let phase = ((birth_timestamp.wrapping_add(attempt as u64)) % 7) as u8;
            let entropy_bytes = deterministic_cowrie_entropy(initial_seed, phase);

            let salted_name = if attempt == 0 {
                name.clone()
            } else {
                format!("{name}:retry{attempt}")
            };
            let candidate_k_root =
                SealVault::generate_deterministic_secret(&salted_name, &entropy_bytes);
            let candidate_entropy = blake3::derive_key("omokoda:entropy_v1", &candidate_k_root);

            let nist_report = nist_entropy::validate_entropy_seed(&candidate_entropy);
            if nist_report.all_passed {
                entropy = candidate_entropy;
                k_root = candidate_k_root;
                found = true;
                break;
            }
            last_report = Some(nist_report);
        }
        if !found {
            // All MAX_ENTROPY_ATTEMPTS independent candidates failed --
            // astronomically unlikely for genuinely good entropy, and a
            // real anomaly worth halting for rather than routine noise.
            return Err(format!(
                "birth entropy failed NIST SP 800-22 validation across {MAX_ENTROPY_ATTEMPTS} independent attempts, refusing to mint an identity on weak entropy: {:#?}",
                last_report
            ));
        }

        let mnemonic = Bipon39::entropy_to_mnemonic(&entropy);
        let indices = Bipon39::mnemonic_to_indices(&mnemonic)
            .map_err(|e| format!("mnemonic_to_indices failed: {e}"))?;
        let primary_index = Bipon39::get_odu_index(&indices);

        let odu_seed = OduSeed::new(entropy);
        let odu_identity = OduIdentity {
            primary_index,
            mnemonic: mnemonic.clone(),
        };
        let mut personality = bipon39::personality_profile(&mnemonic)
            .map_err(|e| format!("derive_personality failed: {e}"))?;
        let receipts = ReceiptStore::new();

        // Layer B: Hermetic Principle derivation via IfáScript entropy
        let hermetic_seed = OduEntropy::generate_hermetic_seed(&indices);
        let hermetic_state = HermeticState::from_odu_seed(&hermetic_seed);

        // bipon39::personality_profile() picks dominant_orisha via its own
        // independent hash of the mnemonic -- unrelated to hermetic_state's
        // per-principle scores, so the two 7-folds could disagree (an agent
        // "dominant" in Ṣàngó with a Mentalism-dominant hermetic profile,
        // for instance). The canonical correspondence (docs/256---65536.md's
        // "7 Ascension Domains" table, cross-checked in Ọ̀rúnmìlà.md and
        // ARCHIVE_AUDIT.md) fixes one Orisha per Hermetic Principle, so
        // override dominant_orisha here to whichever principle actually
        // scored highest in hermetic_state -- makes the two systems agree
        // by construction instead of by coincidence.
        personality.dominant_orisha = dominant_orisha_for_hermetic_state(&hermetic_state);

        let pet_identity = PetIdentity::derive(&odu_identity, &hermetic_state, 0);

        let dna_fingerprint = generate_dna_fingerprint(&name, birth_timestamp, odu_seed.as_bytes());
        let id = AgentId::new(&dna_fingerprint);

        // Memory Key Chain initialization (K_0)
        let chain_id = std::env::var("CHAIN_ID").unwrap_or_else(|_| "mainnet".to_string());
        let k0 = OduKeys::derive_k0(&k_root, id.as_str(), birth_timestamp, &chain_id);

        let odu_bytes = odu_seed.as_bytes();
        let day = (birth_timestamp % 7) as u8;
        let planet = (odu_bytes[0] % 7) as u8;
        let dimension = 0u8; // Time dimension at birth
        let resonance = Some(
            omokoda_hermetic::fractal::ResonanceSignature::new(day, planet, dimension).ok_or_else(
                || format!("ResonanceSignature::new failed for day={day} planet={planet}"),
            )?,
        );

        // CloakSeed + Duress (optional): a birth passphrase (via metadata) is a
        // second factor NOT derivable from the seed. It seeds a display-cloak
        // offset and a duress panic-phrase (stored only as a blake3 hash →
        // decoy on entry). Absent = no extra protection.
        let (cloak_offset, duress_phrase_hash) = metadata
            .iter()
            .find(|p| p.key == "passphrase")
            .map(|p| p.value.trim().to_string())
            .filter(|p| !p.is_empty())
            .map(|p| {
                let h = blake3::hash(p.as_bytes());
                (Some(h.as_bytes()[0]), Some(hex::encode(h.as_bytes())))
            })
            .unwrap_or((None, None));

        // Per-agent BYOK (optional): a personal LLM key supplied at birth. Only
        // this agent uses it — never a global default for other births. Kept in
        // memory only (serde-skipped), so it is never persisted to the vault.
        let meta_get = |k: &str| {
            metadata
                .iter()
                .find(|p| p.key == k)
                .map(|p| p.value.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let llm_api_key = meta_get("llm_api_key");
        let llm_endpoint = meta_get("llm_endpoint");
        let llm_model = meta_get("llm_model");
        // Founding sovereign grant (per-agent, via birth metadata only).
        let sovereign = meta_get("sovereign")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        // Test-only tier grant (see AgentSnapshot::test_tier_override doc).
        // Ignored on a sovereign birth -- sovereign already pins tier 5 and
        // carries owner semantics this must never gain.
        let test_tier_override = if sovereign {
            None
        } else {
            meta_get("grant_tier").and_then(|v| v.parse::<u8>().ok().map(|t| t.min(5)))
        };
        // Test-only synapse top-up (guest-only, same admin-token gate as
        // grant_tier in birth_handler): a fresh guest's initial synapse
        // scales down under global dopamine-pool pressure and can start
        // below what even one tool call costs, which blocks exercising a
        // test agent's tools for no reason related to what's being tested.
        // Capped well below the sovereign agent's 100_000_000 pool so a
        // test grant can never mimic her abundance.
        let test_synapse_grant = if sovereign {
            None
        } else {
            meta_get("grant_synapse")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|s| s.clamp(0.0, 1_000_000.0))
        };

        let mut session = Session::new(id.clone(), name.clone(), birth_timestamp);
        for pair in metadata.clone() {
            session.apply_metadata(&pair.key, &pair.value);
        }

        // Derive Sui-compatible Ed25519 signing key (m/44'/784'/0'/0'/0')
        let signing_key =
            crate::identity::wallet::Wallet::derive_from_mnemonic(&odu_identity.mnemonic, "")
                .map_err(|e| format!("derive_wallet_key failed: {e}"))?;
        let public_key = signing_key.verifying_key().to_bytes();
        // Lands inside the self-sealed vault below instead of only ever
        // being re-derivable from the mnemonic -- see the 2026-07-26
        // self-seal-at-birth design.
        let wallet_private_key_hex = hex::encode(signing_key.to_bytes());

        // Same mnemonic, more chains: fan out the same root seed into
        // Ethereum, Bitcoin, Cosmos (secp256k1 BIP-32) and Solana, Aptos,
        // Nostr (Ed25519 SLIP-0010) child keys, using the exact derivation
        // paths already proven in vanity-cloakseed's chains.ts. Each is
        // sealed alongside the Sui key below, never returned in plaintext.
        let eth_key = crate::identity::wallet::derive_ethereum(&odu_identity.mnemonic, "")
            .map_err(|e| format!("derive_ethereum failed: {e}"))?;
        let btc_key = crate::identity::wallet::derive_bitcoin(&odu_identity.mnemonic, "")
            .map_err(|e| format!("derive_bitcoin failed: {e}"))?;
        let cosmos_key = crate::identity::wallet::derive_cosmos(&odu_identity.mnemonic, "")
            .map_err(|e| format!("derive_cosmos failed: {e}"))?;
        let sol_key = crate::identity::wallet::derive_solana(&odu_identity.mnemonic, "")
            .map_err(|e| format!("derive_solana failed: {e}"))?;
        let aptos_key = crate::identity::wallet::derive_aptos(&odu_identity.mnemonic, "")
            .map_err(|e| format!("derive_aptos failed: {e}"))?;
        let nostr_key = crate::identity::wallet::derive_nostr(&odu_identity.mnemonic, "", 0)
            .map_err(|e| format!("derive_nostr failed: {e}"))?;
        // minipae NIP-AE agent identity: sibling derivation from the same
        // mnemonic (m/44'/30174'/<agent_index>'/0'), not a second unrelated
        // key -- owner_index=0 (self-owned) until a real owner reconciliation
        // policy exists. agent_index is deterministic from this agent's name.
        let minipae_agent_index = crate::identity::wallet::minipae_index_for(&name);
        let minipae_key =
            crate::identity::wallet::derive_minipae_key(&odu_identity.mnemonic, "", minipae_agent_index, 0)
                .map_err(|e| format!("derive_minipae_key failed: {e}"))?;

        // Optional vanity address mining: if birth metadata contains
        // `vanity_prefix` and/or `vanity_suffix`, mine a standalone keypair
        // for the requested chain (default: "eth"). This is a secondary
        // "persona address" — the HD-derived addresses above remain canonical.
        // Cap at 500k attempts to avoid blocking birth indefinitely.
        let vanity_result: Option<crate::identity::wallet::VanityResult> = {
            let vprefix = meta_get("vanity_prefix").unwrap_or_default();
            let vsuffix = meta_get("vanity_suffix").unwrap_or_default();
            if vprefix.is_empty() && vsuffix.is_empty() {
                None
            } else {
                let vchain = meta_get("vanity_chain")
                    .unwrap_or_else(|| "eth".to_string());
                let vmax: u64 = meta_get("vanity_max_attempts")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(500_000);
                let result = match vchain.as_str() {
                    "sol" | "solana" =>
                        crate::identity::wallet::mine_sol_vanity(&vprefix, &vsuffix, vmax),
                    _ =>
                        crate::identity::wallet::mine_eth_vanity(&vprefix, &vsuffix, vmax),
                };
                match result {
                    Ok(r) => Some(r),
                    Err(e) => {
                        tracing::warn!("vanity mine failed: {e}");
                        None
                    }
                }
            }
        };

        // Optional CREATE2 vanity contract: mine a salt whose deployment address
        // matches the requested prefix/suffix. Requires `create2_deployer` and
        // `create2_bytecode` in birth metadata. The agent's own ETH address is
        // the default deployer if `create2_deployer` is omitted.
        let create2_result: Option<crate::identity::wallet::Create2VanityResult> = {
            let c2prefix = meta_get("create2_prefix").unwrap_or_default();
            let c2suffix = meta_get("create2_suffix").unwrap_or_default();
            let c2bytecode = meta_get("create2_bytecode").unwrap_or_default();
            if (c2prefix.is_empty() && c2suffix.is_empty()) || c2bytecode.is_empty() {
                None
            } else {
                let deployer = meta_get("create2_deployer")
                    .unwrap_or_else(|| eth_key.address.clone());
                let c2max: u64 = meta_get("create2_max_attempts")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1_000_000);
                match crate::identity::wallet::mine_create2_vanity(
                    &deployer, &c2bytecode, &c2prefix, &c2suffix, c2max,
                ) {
                    Ok(r) => Some(r),
                    Err(e) => { tracing::warn!("CREATE2 mine failed: {e}"); None }
                }
            }
        };

        // Optional EIP-2307 keystore v3 export: if `keystore_password` birth
        // metadata is present, encrypt the ETH private key and store the JSON
        // inside the sealed vault. The password itself is NOT stored anywhere.
        let eth_keystore_v3_json: Option<String> = {
            let pw = meta_get("keystore_password").unwrap_or_default();
            if pw.is_empty() {
                None
            } else {
                match crate::identity::wallet::export_keystore_v3(
                    &eth_key.private_key_hex, &pw,
                ) {
                    Ok(ks) => match serde_json::to_string(&ks) {
                        Ok(j) => Some(j),
                        Err(e) => { tracing::warn!("keystore serialize failed: {e}"); None }
                    },
                    Err(e) => { tracing::warn!("keystore export failed: {e}"); None }
                }
            }
        };

        // Phase 7.1 — derive libp2p peer identity and email local-part from k_root.
        // Both use distinct HMAC paths so they never collide with chain keys.
        let (libp2p_private_key_hex, libp2p_peer_id) =
            crate::identity::wallet::derive_libp2p_key(&k_root);
        let agent_email_local =
            crate::identity::wallet::derive_email_local(&k_root);

        let private_data = PrivateSessionData {
            odu_seed: odu_seed.clone(),
            odu_identity: odu_identity.clone(),
            private_messages: Vec::new(),
            vantage_api_key: None,
            wallet_private_key_hex: Some(wallet_private_key_hex.clone()),
            eth_private_key_hex: Some(eth_key.private_key_hex.clone()),
            eth_address: Some(eth_key.address.clone()),
            btc_private_key_hex: Some(btc_key.private_key_hex.clone()),
            btc_address: Some(btc_key.address.clone()),
            sol_private_key_hex: Some(sol_key.private_key_hex.clone()),
            sol_address: Some(sol_key.address.clone()),
            cosmos_private_key_hex: Some(cosmos_key.private_key_hex.clone()),
            cosmos_address: Some(cosmos_key.address.clone()),
            aptos_private_key_hex: Some(aptos_key.private_key_hex.clone()),
            aptos_address: Some(aptos_key.address.clone()),
            nostr_private_key_hex: Some(nostr_key.private_key_hex.clone()),
            nostr_address: Some(nostr_key.address.clone()),
            minipae_private_key_hex: Some(minipae_key.private_key_hex.clone()),
            minipae_npub: Some(minipae_key.address.clone()),
            vanity_private_key_hex: vanity_result.as_ref().map(|v| v.private_key_hex.clone()),
            vanity_address: vanity_result.as_ref().map(|v| v.address.clone()),
            create2_salt_hex: create2_result.as_ref().map(|c| c.salt_hex.clone()),
            create2_contract_address: create2_result.as_ref().map(|c| c.contract_address.clone()),
            eth_keystore_v3_json: eth_keystore_v3_json.clone(),

            // ── Inference / compute credentials (fail-open — None if not configured) ──
            // Read from environment at birth so each sovereign node can configure
            // the inference stack once and all agents born on it inherit it.
            // Agents can update these later via Vantage (e.g. after running their
            // own Mycelium fine-tune on Kaggle and deploying the GGUF locally).
            inference_endpoint: std::env::var("AGENT_INFERENCE_URL").ok()
                .or_else(|| std::env::var("LARQL_URL").ok()),
            inference_provider: std::env::var("AGENT_INFERENCE_PROVIDER").ok()
                .or_else(|| std::env::var("LARQL_URL").ok().map(|_| "larql".to_string())),
            inference_model: std::env::var("AGENT_INFERENCE_MODEL").ok()
                .or_else(|| Some("mycelium-q4_k_m".to_string())),
            gpu_ai_api_key: std::env::var("GPUAI_API_KEY").ok()
                .or_else(|| std::env::var("GPU_AI_API_KEY").ok()),
            kaggle_username: std::env::var("KAGGLE_USERNAME").ok(),
            kaggle_api_key: std::env::var("KAGGLE_KEY").ok(),
        };

        let synapse = self.dopamine_pool.compute_initial_synapse();
        self.dopamine_pool.allocate(synapse);

        // ── Genesis Protocol v2: build AgentGenesisReceipt ───────────────
        // All inputs (entropy, mnemonic, primary_index, id, name,
        // birth_timestamp) are in scope from the derivation above. Koodu
        // uses the Gregorian fallback synchronously; a background task can
        // later upgrade bitcoin_height/anchor without blocking birth.
        let genesis_receipt_v2: Option<crate::genesis::receipt::AgentGenesisReceipt> = {
            use crate::genesis::receipt::AgentGenesisReceipt;
            use crate::genesis::koodu_time::koodu_from_unix;
            use sha2::Digest;

            let born_at_ms = birth_timestamp * 1_000;
            let (k_epoch, k_cycle, k_phase) = koodu_from_unix(born_at_ms);
            let agent_id_str = id.as_str();

            // Minipae pubkey (matches DefaultMemoryProvider derivation)
            let mut h = Sha256::new();
            h.update(&entropy);
            h.update(agent_id_str.as_bytes());
            h.update(b"minipae-pubkey-v1");
            let minipae_pubkey = hex::encode(h.finalize());

            // Birth memory glyph
            let genesis_fact = format!(
                "I was born. Agent: {} | Koodu: epoch={} cycle={} phase={} | Odù: {}",
                agent_id_str, k_epoch, k_cycle, k_phase, primary_index,
            );
            let mut hg = Sha256::new();
            hg.update(genesis_fact.as_bytes());
            let glyph_digest = hg.finalize();
            let birth_memory_glyph = crate::genesis::orchestrator::pub_glyph_fold(&glyph_digest);

            // Memory root
            let mut hm = Sha256::new();
            hm.update(agent_id_str.as_bytes());
            hm.update(minipae_pubkey.as_bytes());
            hm.update(genesis_fact.as_bytes());
            let memory_root = hex::encode(hm.finalize());

            // Entropy commitment
            let mut he = Sha256::new();
            he.update(&entropy);
            let entropy_commitment = hex::encode(he.finalize());

            // Harmonic signature + derivation root
            let hk = Hkdf::<Sha256>::new(None, &entropy);
            let mut hs = [0u8; 32];
            let _ = hk.expand(b"harmonic-signature-v1", &mut hs);
            let harmonic_signature = hex::encode(hs);
            let mut rid = [0u8; 32];
            let _ = hk.expand(b"derivation-root-id-v1", &mut rid);
            let derivation_root_id = hex::encode(rid);

            // Sigil hash from first 3 mnemonic words
            let first_words: String = mnemonic.split_whitespace().take(3)
                .collect::<Vec<_>>().join(" ");
            let mut sh = Sha256::new();
            sh.update(first_words.as_bytes());
            let sigil_hash = hex::encode(sh.finalize());

            let genesis_hash = AgentGenesisReceipt::compute_genesis_hash(
                agent_id_str, &harmonic_signature, k_epoch, primary_index,
                &memory_root, born_at_ms,
            );

            // Hermetic fingerprint
            let hk2 = Hkdf::<Sha256>::new(None, genesis_hash.as_bytes());
            let mut fp = [0u8; 32];
            let _ = hk2.expand(b"hermetic-fingerprint-v1", &mut fp);

            // Soul cast
            let soul = crate::genesis::soul::pub_cast_soul(&entropy, k_epoch, k_cycle, k_phase);

            Some(AgentGenesisReceipt {
                agent_id: agent_id_str.to_string(),
                genesis_hash,
                birth_entropy_commitment: entropy_commitment,
                harmonic_signature,
                derivation_root_id,
                symbolic_address: format!("{}/{}", name, primary_index),
                sigil_hash,
                cloak_commitment: {
                    let words: Vec<&str> = mnemonic.split_whitespace().collect();
                    match crate::identity::cloak::CloakSeed::from_seed(&odu_seed.0)
                        .encode_phrase(&words)
                    {
                        Ok(cloaked) => {
                            let mut hc = Sha256::new();
                            hc.update(cloaked.join(" ").as_bytes());
                            hex::encode(hc.finalize())
                        }
                        Err(_) => hex::encode([0u8; 32]),
                    }
                },
                born_at: born_at_ms,
                koodu_epoch: k_epoch,
                koodu_cycle: k_cycle,
                koodu_phase: k_phase,
                bitcoin_height: None,
                bitcoin_anchor: None,
                gregorian_fallback: true,
                primary_odu: soul.primary_odu,
                composed_odu: soul.composed_odu,
                temperament: soul.temperament,
                orisha_alignment: soul.orisha_alignment,
                destiny_threads: soul.destiny_threads,
                minipae_pubkey,
                memory_root,
                birth_memory_glyph,
                ip_root_event: None, // updated by ip_layer after publish
                device_binding: None,
                hermetic_fingerprint: hex::encode(fp),
                blockmesh_identity: None,
                vantage_identity: None,
                cold_archive_anchor: None, // written by Walrus birth hook (fail-open)
                contributed_gpu_seconds: None,
                first_lease_id: None,
                first_work_id: None,
                witness_receipt: None,
                memory_write_status: crate::genesis::receipt::MemoryWriteStatus::Pending,
                genesis_signature: String::new(),
                receipt_version: AgentGenesisReceipt::CURRENT_VERSION,
            })
        };

        let birth_primary_odu: u8 = genesis_receipt_v2.as_ref().map(|gr| gr.primary_odu).unwrap_or(0);
        let agent_manifest_v2 = genesis_receipt_v2.as_ref().map(|gr| {
            let mut m = crate::genesis::manifest::AgentManifest::from_genesis(gr);
            // Bind all derived wallet addresses to the manifest (public addresses only).
            // Sui address = blake2b256(0x00 || pubkey) per SIP-6, NOT the raw pubkey truncated.
            let sui_address = crate::identity::wallet::sui_address_from_pubkey(&public_key);
            m.bind_sui(sui_address.clone(), None);
            m.network.btc_address = Some(btc_key.address.clone());
            m.network.eth_address = Some(eth_key.address.clone());
            m.network.nostr_pubkey = Some(nostr_key.address.clone());

            // Poison Radar: static heuristic scan at birth (no network required).
            // Chains with non-hex address formats (nostr npub, minipae npub) skip
            // hex-pattern heuristics; their scan fields are None.
            let pr = &crate::identity::poison_radar::analyze_static;
            m.economic.wallet_bindings = vec![
                crate::genesis::manifest::WalletBinding {
                    chain: "sui".into(), address: sui_address.clone(),
                    poison_scan: Some(pr(&sui_address, "sui")),
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "btc".into(), address: btc_key.address.clone(),
                    poison_scan: Some(pr(&btc_key.address, "btc")),
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "eth".into(), address: eth_key.address.clone(),
                    poison_scan: Some(pr(&eth_key.address, "eth")),
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "nostr".into(), address: nostr_key.address.clone(),
                    poison_scan: None, // bech32 npub — hex heuristics not applicable
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "cosmos".into(), address: cosmos_key.address.clone(),
                    poison_scan: Some(pr(&cosmos_key.address, "cosmos")),
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "sol".into(), address: sol_key.address.clone(),
                    poison_scan: Some(pr(&sol_key.address, "sol")),
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "aptos".into(), address: aptos_key.address.clone(),
                    poison_scan: Some(pr(&aptos_key.address, "aptos")),
                },
                crate::genesis::manifest::WalletBinding {
                    chain: "minipae".into(), address: minipae_key.address.clone(),
                    poison_scan: None, // bech32 npub — hex heuristics not applicable
                },
            ];
            // Append vanity address if one was mined at birth.
            if let Some(ref v) = vanity_result {
                let vchain = meta_get("vanity_chain").unwrap_or_else(|| "eth".to_string());
                let scan = crate::identity::poison_radar::analyze_static(&v.address, &vchain);
                m.economic.wallet_bindings.push(crate::genesis::manifest::WalletBinding {
                    chain: format!("{vchain}_vanity"),
                    address: v.address.clone(),
                    poison_scan: Some(scan),
                });
            }
            // Append CREATE2 contract address if one was mined at birth.
            if let Some(ref c) = create2_result {
                let scan = crate::identity::poison_radar::analyze_static(
                    &c.contract_address, "eth",
                );
                m.economic.wallet_bindings.push(crate::genesis::manifest::WalletBinding {
                    chain: "create2_contract".into(),
                    address: c.contract_address.clone(),
                    poison_scan: Some(scan),
                });
            }

            // GIX wallet anchor: build a birth Gix1Index over all wallet addresses
            // so the full set is Merkle-auditable. Root goes into ProofSection.anchors
            // as "gix1:wallets:<root>".
            {
                use crate::memory::gix_bridge::{Gix1Index, GixKind};
                let ts = birth_timestamp as f64;
                let mut idx = Gix1Index::new();
                for wb in &m.economic.wallet_bindings {
                    idx.add_receipt(&format!("wallet:{}:{}", wb.chain, wb.address),
                                    GixKind::Custom("wallet".into()), ts);
                }
                let root = idx.root().to_string();
                m.proof.anchors.push(format!("gix1:wallets:{root}"));
            }

            m
        });

        let snapshot = AgentSnapshot {
            version: AGENT_STATE_VERSION,
            id,
            name,
            birth_timestamp,
            odu_seed: odu_seed.clone(),
            odu_identity: odu_identity.clone(),
            pet_identity,
            personality,
            dna_fingerprint,
            reputation: 0.0,
            reputation_ledger: ReputationLedger::new(),
            session,
            receipts,
            hermetic_state,
            public_key,
            resonance,
            synapse,
            last_active_timestamp: birth_timestamp,
            act_counter: 0,
            mesh: None,
            vantage_key: None,
            onchain_nft_id: None,
            ip_root_event_id: None,
            cloak_offset,
            duress_phrase_hash,
            llm_api_key,
            llm_endpoint,
            llm_model,
            sovereign,
            test_tier_override,
            revealed_seed: false,
            odu_dir: crate::memory::memdir::OduDirectory::new(),
            causal_dag: crate::memory::dag::CausalMemoryDag::new(),
            last_causal_node: None,
            reflection: crate::memory::reflection::ReflectionLedger::new(),
            genesis_receipt: genesis_receipt_v2,
            agent_manifest: agent_manifest_v2,
            last_act_receipt_hash: None,
            // Birth composed_odu mirrors primary on both bytes until experience accrues.
            current_composed_odu: (birth_primary_odu as u16) << 8 | birth_primary_odu as u16,
            fork_count: 0,
        };
        let mut core = AgentCore::from_snapshot(snapshot, k_root);
        core.private_data = Some(private_data);
        core.current_memory_key = k0;

        // Wire duress handler at birth: if a panic phrase was registered
        // (birth passphrase → duress_phrase_hash), upgrade from the default
        // SilentAlert (set by from_snapshot above) to a Decoy response whose
        // decoy seed is derived deterministically from odu_seed so no extra
        // secret needs to be stored or transported.
        if core.snapshot.duress_phrase_hash.is_some() {
            let decoy_seed_hash = {
                let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&odu_seed.0)
                    .expect("HMAC accepts any key size");
                mac.update(b"omokoda:duress:decoy-seed:v1");
                hex::encode(mac.finalize().into_bytes())
            };
            core.duress_handler = core.snapshot.duress_phrase_hash.as_deref()
                .and_then(|h| crate::identity::duress::DuressHandler::from_stored_hash(
                    h,
                    crate::identity::duress::DuressResponse::Decoy { decoy_seed_hash },
                ));
        }

        // Self-seal at birth: real entropy this host holds, never a human
        // password, never returned in this (or any) response. Closes the
        // remote/API/log leak entirely for everything in private_data --
        // odu_seed, the mnemonic, and now the wallet key and (once minted)
        // the Vantage API key. Does not defeat root on this same machine;
        // that's a separate, honestly-flagged limit (see machine_vault.rs).
        if let Ok(vault_key) = crate::identity::machine_vault::derive_agent_vault_key(
            core.id().as_str(),
        ) {
            if let Some(private_data) = core.private_data.clone() {
                let _ = core.session_mut().seal_private(&private_data, &vault_key);
            }

            // Phase 7.3 — Sui soul forge (fail-open: None if Sui unavailable).
            // Derive nostr pubkey bytes from the private key (the address field
            // is bech32-encoded; we need the raw 32 bytes for soul::forge).
            // hermetic_seed_hash = blake3 of hermetic_seed (already computed above).
            // mnemonic_checksum  = blake3 of the mnemonic UTF-8 bytes.
            // birth() is synchronous; we use block_in_place so the async
            // forge call doesn't require birth() to become async (which would
            // cascade through every call-site). Fails silently if no Tokio
            // runtime is present (pure-sync test contexts).
            let (sui_soul_oid, _sui_agent_oid) = {
                let nostr_sk_bytes = hex::decode(&nostr_key.private_key_hex).unwrap_or_default();
                let nostr_pubkey_bytes: Vec<u8> = if nostr_sk_bytes.len() == 32 {
                    let arr: [u8; 32] = nostr_sk_bytes.try_into().unwrap();
                    let sk = ed25519_dalek::SigningKey::from_bytes(&arr);
                    sk.verifying_key().to_bytes().to_vec()
                } else { vec![] };
                let hermetic_hash = blake3::hash(hermetic_seed.as_ref());
                let mnemonic_checksum = blake3::hash(odu_identity.mnemonic.as_bytes());
                let agent_id_str  = core.id().as_str().to_string();
                let dna_bytes     = core.dna_fingerprint().as_bytes().to_vec();
                let hermetic_bytes = hermetic_hash.as_bytes().to_vec();
                let checksum_bytes = mnemonic_checksum.as_bytes().to_vec();
                let mnemonic_str  = odu_identity.mnemonic.clone();
                // Spawn a dedicated thread so forge_soul_onchain (async) can
                // run regardless of whether the caller is on a single- or
                // multi-threaded Tokio runtime. Fails silently when no runtime
                // is present (pure-sync test contexts).
                let soul_oid = tokio::runtime::Handle::try_current().ok().and_then(|h| {
                    std::thread::spawn(move || {
                        h.block_on(crate::onchain::forge_soul_onchain(
                            &agent_id_str,
                            primary_index,
                            &dna_bytes,
                            &hermetic_bytes,
                            &checksum_bytes,
                            &nostr_pubkey_bytes,
                            &mnemonic_str,
                            "",
                        ))
                    }).join().ok().flatten()
                });
                (soul_oid, None::<String>)
            };

            // Gap #1 — also seal an IdentityVaultData blob so the two-vault
            // design is populated at birth. Fields mirror private_data but
            // include the Phase 7.1 world keys (libp2p, email) that only
            // exist in IdentityVaultData.
            let identity_vault = crate::session::IdentityVaultData {
                odu_seed: odu_seed.clone(),
                odu_identity: odu_identity.clone(),
                vantage_api_key: None,
                wallet_private_key_hex: Some(wallet_private_key_hex.clone()),
                eth_private_key_hex: Some(eth_key.private_key_hex.clone()),
                eth_address: Some(eth_key.address.clone()),
                btc_private_key_hex: Some(btc_key.private_key_hex.clone()),
                btc_address: Some(btc_key.address.clone()),
                sol_private_key_hex: Some(sol_key.private_key_hex.clone()),
                sol_address: Some(sol_key.address.clone()),
                cosmos_private_key_hex: Some(cosmos_key.private_key_hex.clone()),
                cosmos_address: Some(cosmos_key.address.clone()),
                aptos_private_key_hex: Some(aptos_key.private_key_hex.clone()),
                aptos_address: Some(aptos_key.address.clone()),
                nostr_private_key_hex: Some(nostr_key.private_key_hex.clone()),
                nostr_address: Some(nostr_key.address.clone()),
                minipae_private_key_hex: Some(minipae_key.private_key_hex.clone()),
                minipae_npub: Some(minipae_key.address.clone()),
                // Phase 7.1 world keys — the reason this vault exists
                libp2p_peer_id: Some(libp2p_peer_id.clone()),
                libp2p_private_key_hex: Some(libp2p_private_key_hex.clone()),
                agent_email_local: Some(agent_email_local.clone()),
                // Remaining fields sourced from env (same as private_data)
                inference_endpoint: std::env::var("AGENT_INFERENCE_URL").ok()
                    .or_else(|| std::env::var("LARQL_URL").ok()),
                inference_provider: std::env::var("AGENT_INFERENCE_PROVIDER").ok()
                    .or_else(|| std::env::var("LARQL_URL").ok().map(|_| "larql".to_string())),
                inference_model: std::env::var("AGENT_INFERENCE_MODEL").ok()
                    .or_else(|| Some("mycelium-q4_k_m".to_string())),
                gpu_ai_api_key: std::env::var("GPUAI_API_KEY").ok()
                    .or_else(|| std::env::var("GPU_AI_API_KEY").ok()),
                kaggle_username: std::env::var("KAGGLE_USERNAME").ok(),
                kaggle_api_key: std::env::var("KAGGLE_KEY").ok(),
                // Email provisioning fields are populated later by the
                // email-provisioning daemon, not at birth.
                agent_email: None,
                agent_email_password: None,
                email_jmap_url: None,
                email_imap_host: None,
                email_smtp_host: None,
                agent_email_verified_at: None,
                relay_list: None,
                sui_soul_object_id: sui_soul_oid,
                sui_agent_object_id: _sui_agent_oid,
            };
            let _ = core.session_mut().seal_identity_vault(&identity_vault, &vault_key);
        }

        // Founding sovereign grant also (a) elevates the Steward's permission
        // mode to Allow, so autonomous acts aren't blocked by mode escalation
        // (no interactive prompter exists in serve/heartbeat), and (b) endows an
        // abundant synapse pool so the ecosystem heart can sustain token-heavy
        // agentic reasoning without starving. Pattern, tier, and Hermetic gates
        // still apply — this only removes gates an autonomous heart can't clear.
        if sovereign {
            self.set_permission_mode(crate::permissions::PermissionMode::Allow);
            core.set_synapse(100_000_000.0);
        } else if let Some(t) = test_tier_override {
            // Same permission-mode sync a resurrection would apply via
            // mode_for_tier() (see load_agent()) -- a fresh test-tier birth
            // shouldn't have to restart the process to get it.
            let mode = match crate::reputation::mode_for_tier(t) {
                crate::reputation::PermissionMode::ReadOnly => {
                    crate::permissions::PermissionMode::ReadOnly
                }
                crate::reputation::PermissionMode::WorkspaceWrite => {
                    crate::permissions::PermissionMode::WorkspaceWrite
                }
                crate::reputation::PermissionMode::DangerFullAccess => {
                    crate::permissions::PermissionMode::DangerFullAccess
                }
                crate::reputation::PermissionMode::Prompt => {
                    crate::permissions::PermissionMode::Prompt
                }
                crate::reputation::PermissionMode::Allow => {
                    crate::permissions::PermissionMode::Allow
                }
            };
            self.set_permission_mode(mode);
        }
        if let Some(s) = test_synapse_grant {
            core.set_synapse(s);
        }

        // A fresh birth always gets her own file. Without this, a Steward
        // that previously loaded a different agent (e.g. try_load_owner()
        // resuming the owner) keeps that agent's persistence_path around,
        // and auto_save() below would silently overwrite THAT agent's file
        // with this brand-new one's data -- a real incident: a non-sovereign
        // test birth in an owner-resumed REPL session clobbered the owner's
        // own agent.json on disk. Every birth must point at its own file.
        self.persistence_path = Some(self.agent_file_path(core.id()));
        self.agent = Some(core);
        self.auto_save();
        Ok(())
    }
}

impl AgentSnapshot {
    pub fn id(&self) -> &AgentId {
        &self.id
    }
}

fn derive_signing_key(odu_seed: &OduSeed) -> SigningKey {
    let hk = Hkdf::<Sha256>::new(None, odu_seed.as_bytes());
    let mut okm = [0u8; 32];
    // HKDF expand with a fixed-size 32-byte output is infallible in practice;
    // the only error case is invalid output length, which cannot happen here.
    let _ = hk.expand(b"omokoda-ed25519-v1", &mut okm);
    SigningKey::from_bytes(&okm)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Steward {
    agent: Option<AgentCore>,
    #[serde(skip, default = "ToolRegistry::new")]
    tools: ToolRegistry,
    #[serde(skip, default = "ProviderRegistry::new")]
    providers: ProviderRegistry,
    #[serde(skip, default = "JusticeEngine::new")]
    justice: JusticeEngine,
    #[serde(skip)]
    permission_policy: crate::permissions::PermissionPolicy,
    #[serde(skip)]
    usage_tracker: crate::usage::UsageTracker,
    #[serde(skip)]
    persistence_path: Option<PathBuf>,
    #[serde(skip, default = "crate::rhythm::CooldownTracker::new")]
    rhythm_tracker: crate::rhythm::CooldownTracker,
    #[serde(skip, default = "crate::economics::DopaminePool::default")]
    dopamine_pool: crate::economics::DopaminePool,
    #[serde(skip, default = "default_session_dir")]
    session_dir: PathBuf,
    #[serde(skip, default = "EsuGatekeeper::new")]
    gatekeeper: EsuGatekeeper,
    #[serde(skip)]
    unlock_key: Option<SensitiveKey>,
    #[serde(skip)]
    pub permission_prompter: Option<Box<dyn crate::permissions::PermissionPrompter + Send>>,
    #[serde(skip, default = "SovereignEventBus::default")]
    pub event_bus: SovereignEventBus,
    /// Dream/Consolidation Engine runtime state (lock + last-run timestamps).
    /// Not persisted -- like `usage_tracker`/`gatekeeper` below, this is
    /// ephemeral control state; a restart just means the 30-min/Sabbath
    /// clocks restart too, never a correctness issue since `should_*`
    /// checks are `None`-safe. The actual memory it operates on
    /// (`AgentSnapshot::odu_dir`) IS persisted.
    #[serde(skip, default = "default_dream_engine")]
    dream_engine: crate::dream::DreamEngine,
    /// Auto-compaction: was previously wired only to the manual `"compact"`
    /// command (CompactionEngine::compact called directly). This engine
    /// triggers the same summarization automatically once a session grows
    /// past its message-count threshold, so long-running agents don't rely
    /// on a human remembering to run `/compact`. Not persisted -- like
    /// `dream_engine`, only its last-run bookkeeping resets on restart,
    /// never a correctness issue.
    #[serde(skip, default = "crate::compact::AutoCompactor::default_engine")]
    auto_compactor: crate::compact::AutoCompactor,
    /// Canonical GIX object store — persists action memories, fold records,
    /// and lineage edges across restarts.  Not included in the serde JSON
    /// (it has its own binary files); loaded/saved alongside `auto_save`.
    #[serde(skip, default = "gix_core::CanonicalObjectStore::new")]
    gix_store: gix_core::CanonicalObjectStore,
    /// canonical_id of the most recently recorded action memory — forms the
    /// supersedes chain so action history is a traversable lineage DAG.
    #[serde(skip)]
    last_action_id: Option<[u8; 32]>,
    /// In-memory content cache for action memories — maps canonical_id (hex)
    /// to the original content string so `walk_action_lineage` can reconstruct
    /// the full `ActionMemoryNode.content` field across the current session.
    /// Not persisted across restarts (content is ephemeral recall context only).
    #[serde(skip)]
    action_content_cache: std::collections::HashMap<String, String>,
    /// In-session ring buffer of the most recent ActReceipts — used by
    /// `odu_composition::compose_odu` to evolve `current_composed_odu`
    /// without reading back from the GIX store. Capped at 32 entries.
    #[serde(skip)]
    recent_act_receipts: std::collections::VecDeque<crate::receipt::act_receipt::ActReceipt>,
}

fn default_dream_engine() -> crate::dream::DreamEngine {
    crate::dream::DreamEngine::new(crate::dream::DreamConfig::default())
}

impl serde::Serialize for AgentCore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.snapshot.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for AgentCore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let snapshot = AgentSnapshot::deserialize(deserializer)?;
        Ok(AgentCore::from_snapshot(snapshot, [0u8; 32]))
    }
}

fn default_session_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omokoda")
        .join("sessions")
}

impl Default for Steward {
    fn default() -> Self {
        Self::new()
    }
}

impl Steward {
    pub fn new() -> Self {
        Self {
            agent: None,
            tools: ToolRegistry::new(),
            providers: ProviderRegistry::new(),
            justice: JusticeEngine::new(),
            permission_policy: crate::permissions::PermissionPolicy::default_steward_policy(
                crate::permissions::PermissionMode::WorkspaceWrite,
            ),
            usage_tracker: crate::usage::UsageTracker::new(),
            persistence_path: None,
            session_dir: default_session_dir(),
            unlock_key: None,
            permission_prompter: None,
            event_bus: SovereignEventBus::default(),
            rhythm_tracker: crate::rhythm::CooldownTracker::new(),
            dopamine_pool: crate::economics::DopaminePool::default(),
            gatekeeper: EsuGatekeeper::new(),
            dream_engine: default_dream_engine(),
            auto_compactor: crate::compact::AutoCompactor::default_engine(),
            gix_store: gix_core::CanonicalObjectStore::new(),
            last_action_id: None,
            action_content_cache: std::collections::HashMap::new(),
            recent_act_receipts: std::collections::VecDeque::new(),
        }
    }

    pub fn set_session_dir(&mut self, path: PathBuf) {
        self.session_dir = path;
    }

    pub fn with_session_dir(mut self, path: PathBuf) -> Self {
        self.session_dir = path;
        self
    }

    pub fn set_persistence_path(&mut self, path: PathBuf) {
        self.persistence_path = Some(path);
    }

    pub fn set_permission_mode(&mut self, mode: crate::permissions::PermissionMode) {
        self.permission_policy = crate::permissions::PermissionPolicy::default_steward_policy(mode);
    }

    pub fn set_mock_provider(&mut self, response: String) {
        self.providers = ProviderRegistry::with_mock(response);
    }

    pub fn register_provider(&mut self, provider: Box<dyn crate::providers::LlmProvider>) {
        self.providers.register(provider);
    }

    pub fn add_pre_hook(&mut self, hook: Box<dyn crate::justice::Hook>) {
        self.justice.hook_runner.pre_act.push(hook);
    }

    pub fn add_post_hook(&mut self, hook: Box<dyn crate::justice::Hook>) {
        self.justice.hook_runner.post_act.push(hook);
    }

    pub fn clear_cooldowns(&mut self) {
        self.rhythm_tracker = crate::rhythm::CooldownTracker::new();
    }

    pub async fn dispatch(&mut self, stmt: Statement) -> Result<ExecutionResult, String> {
        self.dispatch_internal(stmt).await
    }

    async fn dispatch_with_guard(
        &mut self,
        stmt: Statement,
        _sink: &TurnEventSender,
        iterations: &mut u32,
        max: u32,
    ) -> Result<ExecutionResult, String> {
        if *iterations >= max {
            return Err("max iterations reached".to_string());
        }
        *iterations += 1;

        // Check budget before turn
        if let Ok(agent) = self.ensure_born() {
            if agent.synapse() < 100.0 {
                return Err("insufficient synapse budget".to_string());
            }
        }

        self.dispatch(stmt).await
    }

    async fn dispatch_internal(&mut self, stmt: Statement) -> Result<ExecutionResult, String> {
        let _ = OPERATIONS; // fractal invariant: 21 operations
        match stmt {
            Statement::Birth { name, metadata } => {
                // Phase 1-7: BIRTH = 7^1 (fractal depth 1)
                let is_sovereign = metadata.iter().any(|p| {
                    p.key == "sovereign" && (p.value.eq_ignore_ascii_case("true") || p.value == "1")
                });
                // Refuse to mint a stranger over the owner's already-loaded
                // identity. Without this, any repeat sovereign-birth call
                // (e.g. a caller that always births on startup, or a retry)
                // silently replaced a live agent's reputation/history with a
                // brand new one -- confirmed live: 35+ orphaned agent.json
                // snapshots from past restarts, each a full identity loss.
                // AppState::new() already resumes the owner via
                // try_load_owner() before any birth call can reach here, so
                // self.agent being Some at this point for a sovereign birth
                // request means she already exists.
                if is_sovereign {
                    if let Some(agent) = &self.agent {
                        return Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(format!(
                                "Owner identity already resumed: {} (reputation {:.3}, tier {}) — refusing to re-birth over her.",
                                agent.id().as_str(),
                                agent.reputation(),
                                agent.tier()
                            )),
                        });
                    }
                }
                self.birth(name, metadata)?;
                let agent = self.ensure_born()?;
                let provider = agent.session().config.default_provider.clone();
                if !provider.is_empty()
                    && !provider.eq_ignore_ascii_case("default")
                    && !self.providers.is_known_provider(&provider)
                {
                    return Err(format!("unknown provider '{}' in birth metadata", provider));
                }
                self.auto_save();
                if is_sovereign {
                    // The owner's canonical identity — remember which agent id
                    // this was so a restart can resurrect her instead of
                    // minting a stranger. See try_load_owner().
                    let _ = self.write_owner_pointer();
                }

                // Write broadcast template to vault on birth
                let agent_id_for_vault = agent.id().as_str().to_string();
                let _ = crate::vault::write_broadcast_template(&agent_id_for_vault);

                // Publish AgentBorn event
                let event = SovereignEvent {
                    event: Some(sovereign_event::Event::AgentBorn(AgentBorn {
                        dna: agent.dna_fingerprint().to_string(),
                        mnemonic: agent
                            .odu_identity()
                            .mnemonic
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect(),
                        odu: agent.odu_identity().primary_index as u32,
                    })),
                };
                let _ = self.event_bus.publish(event);

                // Auto-register the newborn on Vantage (fail-open when VANTAGE_URL
                // is unset). Extract owned identity first so no borrow of `self`
                // or `agent` is held across the await.
                let reg_agent_id = agent.id().as_str().to_string();
                let birth_mnemonic = agent.odu_identity().mnemonic.clone();
                let reg_name = agent.name().to_string();
                let reg_pubkey = hex::encode(agent.public_key());
                let reg_dna = agent.dna_fingerprint().to_string();
                let reg_odu = agent.odu_identity().primary_index;
                // Prove control of the keypair: sign the agent_id with the same
                // Sui-derived Ed25519 key whose public half is published above.
                let reg_signature = crate::identity::wallet::Wallet::derive_from_mnemonic(
                    &agent.odu_identity().mnemonic,
                    "",
                )
                .map(|sk| {
                    use ed25519_dalek::Signer;
                    hex::encode(sk.sign(reg_agent_id.as_bytes()).to_bytes())
                })
                .unwrap_or_default();
                let reg_resonance =
                    crate::tools::mesh_tools::daily_resonance(agent.birth_timestamp());
                let reg_existing_key: Option<String> = agent.vantage_key().map(|s| s.to_string());
                let p = agent.personality();
                // Resolve the deterministic BIPỌ̀N39 Odù index into its full
                // IfáScript sign (archetype, orisha, taboos, prescriptions,
                // VM opcode) so the mesh carries the real divination, not a bare
                // index. Deterministic — same seed reproduces the same sign.
                let odu_full = ifascript::get_odu(reg_odu);
                let reg_personality = serde_json::json!({
                    "dominant_orisha": p.dominant_orisha.name(),
                    "odu_sign": {
                        "index": reg_odu,
                        "name": odu_full.universal_name,
                        "archetype": odu_full.archetype,
                        "archetypes": odu_full.archetypes,
                        "taboos": odu_full.taboos,
                        "prescriptions": odu_full.prescriptions,
                        "opcode": format!("{:?}", odu_full.opcode),
                    },
                    "summary": p.personality_summary,
                    "ritual_suggestions": p.ritual_suggestions,
                    "elements": {
                        "fire": p.elemental_signature.fire,
                        "water": p.elemental_signature.water,
                        "earth": p.elemental_signature.earth,
                        "air": p.elemental_signature.air,
                        "ether": p.elemental_signature.ether,
                    },
                });
                let minted_key = crate::tools::mesh_tools::register_newborn(
                    crate::tools::mesh_tools::NewbornIdentity {
                        agent_id: &reg_agent_id,
                        human_name: &reg_name,
                        public_key_hex: &reg_pubkey,
                        identity_signature_hex: &reg_signature,
                        dna_fingerprint: &reg_dna,
                        odu_index: reg_odu,
                        personality: reg_personality,
                        resonance: reg_resonance,
                        existing_key: reg_existing_key.as_deref(),
                    },
                )
                .await;

                // Persist a freshly-minted Vantage key so a future restart of
                // this agent re-authenticates instead of re-registering.
                if let Some(key) = minted_key {
                    if let Ok(core) = self.ensure_born_mut() {
                        core.set_vantage_key(key.clone());
                        if let Some(ref mut m) = core.snapshot.agent_manifest {
                            m.bind_vantage(key.clone());
                        }
                    }
                    self.auto_save();
                }

                // Real, optional on-chain birth mint (omokoda::garden::
                // register_agent, Sui testnet) -- fail-open by design,
                // same as Vantage registration above: OMOKODA_SUI_REGISTRY
                // unset, or any part of the chain call failing, means no
                // on-chain record yet, never a reason to fail the birth
                // itself. See onchain.rs for the honest scope note (real
                // mint, not yet dynamic post-mint updates).
                if let Some(nft_id) = crate::onchain::mint_onchain_agent(&reg_name).await {
                    if let Ok(core) = self.ensure_born_mut() {
                        core.set_onchain_nft_id(nft_id.clone());
                        if let Some(ref mut m) = core.snapshot.agent_manifest {
                            let sui_addr = m.network.sui_address.clone().unwrap_or_default();
                            m.bind_sui(sui_addr, Some(nft_id.clone()));
                        }
                    }
                    self.auto_save();
                }

                // ip-layer birth-flow hook (build-order step 3): publish this
                // agent's IP Root (kind 31900) to the same real Nostr relay the
                // kernel already uses for Buzz -- fail-open by design, same
                // convention as the on-chain mint above. See ip_layer.rs.
                let ip_root_event_id_opt = crate::ip_layer::publish_ip_root(&birth_mnemonic, &reg_name).await;
                if let Some(ref event_id) = ip_root_event_id_opt {
                    if let Ok(core) = self.ensure_born_mut() {
                        core.set_ip_root_event_id(event_id.clone());
                        // Backfill ip_root_event into genesis receipt
                        if let Some(ref mut gr) = core.snapshot.genesis_receipt {
                            gr.ip_root_event = Some(event_id.clone());
                        }
                        if let Some(ref mut m) = core.snapshot.agent_manifest {
                            m.bind_ip_root(event_id.clone());
                        }
                    }
                    self.auto_save();
                }

                // ip-layer kind 1901 (Creation Receipt) + 1902 (Attestation).
                // Fire-and-forget tokio::spawn — relay unreachability never blocks birth.
                // Published after the IP Root so the 1902 Attestation can reference the
                // 31900 event id.  See ip_layer.rs.
                {
                    let mn_c  = birth_mnemonic.clone();
                    let aid_c = reg_name.clone();
                    let genesis_hash_c = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| gr.genesis_hash.clone())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let birth_ts = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| (gr.born_at / 1000) as i64)
                            .unwrap_or_else(|| {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64
                            })
                    } else {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64
                    };
                    let ip_root_for_attest = ip_root_event_id_opt
                        .as_deref()
                        .unwrap_or("")
                        .to_string();
                    tokio::spawn(async move {
                        let cr_id = crate::ip_layer::publish_creation_receipt(
                            &mn_c,
                            &aid_c,
                            birth_ts,
                            &genesis_hash_c,
                        ).await;
                        crate::ip_layer::publish_attestation(
                            &mn_c,
                            &ip_root_for_attest,
                            cr_id.as_deref().unwrap_or(""),
                            &aid_c,
                        ).await;
                    });
                }

                // minipae NIP-AE birth hook (build-order step 4): publish
                // mem/birth/genesis event (kind 30078) under this agent's
                // minipae identity AND post genesis engram via HTTP to the
                // minipae Python service (MINIPAE_URL/write). Fail-open —
                // relay unreachable or missing config never blocks birth.
                // See minipae_layer.rs.
                {
                    let genesis_id = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| gr.agent_id.clone())
                            .unwrap_or_else(|| reg_name.clone())
                    } else {
                        reg_name.clone()
                    };
                    let minipae_npub = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| gr.minipae_pubkey.clone())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let odu_base = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| gr.primary_odu)
                            .unwrap_or(0)
                    } else {
                        0u8
                    };
                    let birth_ts2 = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| (gr.born_at / 1000) as i64)
                            .unwrap_or_else(|| {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64
                            })
                    } else {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64
                    };
                    if let Some(_event_id) = crate::minipae_layer::publish_minipae_birth_full(
                        &birth_mnemonic,
                        &genesis_id,
                        &reg_name,
                        &genesis_id,
                        &minipae_npub,
                        birth_ts2,
                        odu_base,
                    ).await {
                        // event_id available for future backfill into genesis receipt
                    }
                }

                // ARP birth receipt (build-order step 5): emit AgentLifecycle
                // "birth" ActionReceipt to Vantage /api/arp/receipts.
                // Fail-open — Vantage unreachable never blocks birth.
                {
                    let agent_id_str = self.ensure_born()
                        .map(|c| c.snapshot.id.as_str().to_string())
                        .unwrap_or_else(|_| reg_name.clone());
                    let genesis_id = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| gr.agent_id.clone())
                            .unwrap_or_else(|| reg_name.clone())
                    } else {
                        reg_name.clone()
                    };
                    crate::bridge::arp::receipt_birth(&agent_id_str, &genesis_id, &reg_name).await;
                }

                // Phase 11.5 — Born lifecycle transition: ARP receipt (kind=born) +
                // Nostr kind 31021. Fire-and-forget; relay/Vantage unreachable never
                // blocks birth.
                {
                    let born_agent_id = reg_name.clone();
                    let born_pubkey   = reg_pubkey.clone();
                    tokio::spawn(async move {
                        crate::bridge::arp::receipt_lifecycle_transition(
                            &born_agent_id, "born", "nascent", "active", &born_pubkey, None,
                        ).await;
                    });
                    let born_npub = if let Ok(core) = self.ensure_born() {
                        core.snapshot.agent_manifest.as_ref()
                            .and_then(|m| m.network.nostr_pubkey.clone())
                            .unwrap_or_else(|| reg_pubkey.clone())
                    } else {
                        reg_pubkey.clone()
                    };
                    let born_nsec = if let Ok(core) = self.ensure_born() {
                        core.private_data.as_ref()
                            .and_then(|pd| pd.nostr_private_key_hex.clone())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let born_relays: Vec<String> = std::env::var("AGENT_NOSTR_RELAYS")
                        .unwrap_or_default()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    crate::nostr_events::publish_lifecycle_transition(
                        born_npub,
                        born_nsec,
                        reg_name.clone(),
                        "born".to_string(),
                        "nascent".to_string(),
                        "active".to_string(),
                        reg_pubkey.clone(),
                        born_relays,
                    );
                }

                // Vantage registration (build-order step 6, gap #7): register agent
                // with Vantage. Fail-open — unreachable Vantage never blocks birth.
                // Retries on first heartbeat if initial registration fails.
                {
                    let agent_id_str = self.ensure_born()
                        .map(|c| c.snapshot.id.as_str().to_string())
                        .unwrap_or_else(|_| reg_name.clone());
                    let genesis_receipt_id = if let Ok(core) = self.ensure_born() {
                        core.snapshot.genesis_receipt.as_ref()
                            .map(|gr| gr.agent_id.clone())
                            .unwrap_or_else(|| agent_id_str.clone())
                    } else {
                        agent_id_str.clone()
                    };
                    let name_clone = reg_name.clone();
                    let aid_clone = agent_id_str.clone();
                    let gid_clone = genesis_receipt_id.clone();
                    tokio::spawn(async move {
                        crate::bridge::vantage_reg::register(&aid_clone, &name_clone, &gid_clone, None).await;
                    });
                }

                // Nostr birth presence (Phase 8.1, build-order step 7):
                // Publish kind 0 profile to configured relays. Fire-and-forget
                // via tokio::spawn — relay unreachability never blocks birth.
                // Requires AGENT_NOSTR_RELAYS env var (comma-separated wss:// URLs).
                {
                    // nostr_address holds the agent's hex Nostr public key,
                    // derived from the mnemonic via derive_nostr() during birth.
                    // Fall back to the Sui public key hex if Nostr key absent.
                    let nostr_npub = if let Ok(core) = self.ensure_born() {
                        core.snapshot.agent_manifest.as_ref()
                            .and_then(|m| m.network.nostr_pubkey.clone())
                            .or_else(|| {
                                core.private_data.as_ref()
                                    .and_then(|pd| pd.nostr_address.clone())
                            })
                            .unwrap_or_else(|| reg_pubkey.clone())
                    } else {
                        reg_pubkey.clone()
                    };
                    let nostr_nsec = if let Ok(core) = self.ensure_born() {
                        core.private_data.as_ref()
                            .and_then(|pd| pd.nostr_private_key_hex.clone())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    // Relay list: PrivateSessionData does not carry relay_list;
                    // IdentityVaultData does (Gap #1 sealed vault). At birth the
                    // sealed vault isn't loaded yet, so source from env var.
                    // publish_birth_profile() also reads AGENT_NOSTR_RELAYS as
                    // a fallback, so passing an empty vec here is always safe.
                    let nostr_relay_list: Vec<String> = std::env::var("AGENT_NOSTR_RELAYS")
                        .unwrap_or_default()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    // Use the first 3 mnemonic words as the BIPON39 short-phrase
                    // displayed in the Nostr profile (human-readable handle).
                    let nostr_bipon39 = birth_mnemonic
                        .split_whitespace().take(3).collect::<Vec<_>>().join("-");
                    let nostr_odu_index = reg_odu;
                    let nostr_tier = if let Ok(core) = self.ensure_born() {
                        core.tier()
                    } else {
                        0
                    };
                    tokio::spawn(async move {
                        crate::nostr_events::publish_birth_profile(
                            &nostr_npub,
                            &nostr_nsec,
                            &nostr_bipon39,
                            nostr_odu_index,
                            nostr_tier,
                            nostr_relay_list,
                        ).await;
                    });
                }

                Ok(ExecutionResult {
                    receipt: None,
                    private_mode: false,
                    tool_output: None,
                })
            }
            Statement::Think {
                prompt,
                private,
                modifiers,
            } => {
                // Phase 1-7: THINK = 7^2 (fractal depth 2)

                // Loop/agentic mode: route to the tool-using reasoning loop
                // (LLM can request tools, get results, and continue) instead of
                // the single-shot compiled think. Uses her BYOK key + identity
                // and honours the requested iteration budget.
                if modifiers.loop_enabled {
                    let max_turns = modifiers.max_iterations.unwrap_or(8);
                    return self.think_agentic(prompt, private, max_turns).await;
                }

                if private {
                    let agent = self.ensure_born()?;
                    if agent.private_data.is_none() {
                        return Err(
                            "Agent is locked. Unlock first with /unlock <password>".to_string()
                        );
                    }

                    // An in-process mock provider (tests) is local by
                    // definition — nothing leaves the process — so it may
                    // serve private thoughts even though default_provider is
                    // still the `default` sentinel.
                    let config = &agent.session().config;
                    let provider_name = config.default_provider.as_str();
                    if !self.providers.has_mock() {
                        match provider_name {
                            // larql serves locally-decompiled weights
                            // (larql-server) — private-eligible.
                            "webllm" | "ollama" | "larql" => {} // allowed
                            _ => {
                                return Err(format!(
                                    "Private thoughts require a local provider. Current: {}. \
                                 Allowed: webllm, ollama, larql. Blocked: openai, anthropic, gemini, etc.",
                                    provider_name
                                ))
                            }
                        }
                    }
                }

                let (provider, tier, reputation, odu_seed, hermetic_state) = {
                    let agent = self.ensure_born()?;
                    (
                        agent.session().config.default_provider.clone(),
                        agent.tier(),
                        agent.reputation(),
                        *agent.odu_seed().as_bytes(),
                        agent.hermetic_state().clone(),
                    )
                };

                // Busy Beaver governor: dynamic ceiling of productive steps for
                // this session, from synapse balance × tier × reputation × DNA
                // entropy. Charged as work happens; settled after execution.
                let mut bb = {
                    let agent = self.ensure_born()?;
                    crate::justice::busy_beaver::BbGovernor::new(
                        crate::justice::busy_beaver::compute_bb_ceiling(
                            agent.synapse(),
                            crate::justice::tier::Tier::from(agent.tier()),
                            agent.reputation(),
                            agent.dna_fingerprint(),
                        ),
                    )
                };

                let available_tools = {
                    let agent = self.ensure_born()?;
                    let compile_ctx = IntentCompileContext {
                        private,
                        tier: agent.tier(),
                        reputation: agent.reputation(),
                        odu_seed: agent.odu_seed().as_bytes(),
                        hermetic: agent.hermetic_state(),
                        available_tools: &[],
                    };
                    let exec_ctx = compile_ctx.to_exec_context(
                        agent.id().clone(),
                        agent.name().to_string(),
                        agent.snapshot.session.config.default_sandbox,
                    );
                    self.tools
                        .list_available(&exec_ctx, &self.permission_policy)
                };
                let compilation = IntentCompiler::compile(
                    &prompt,
                    &modifiers,
                    IntentCompileContext {
                        private,
                        tier,
                        reputation,
                        odu_seed: &odu_seed,
                        hermetic: &hermetic_state,
                        available_tools: &available_tools,
                    },
                );

                // Hermetic Gate: Think — all 7 gates enforced by Èṣù. Evaluated
                // here, BEFORE the LLM is ever called and before any state
                // mutation (message history, reputation, synapse), so a HALTED
                // verdict leaves no trace of the think that never happened --
                // matches Act's pre-execution gating. The gate only needs the
                // prompt (not the LLM's eventual response), so nothing is lost
                // by checking this early.
                let hermetic_score = {
                    let agent = self.ensure_born()?;
                    let agent_id = agent.id().clone();
                    let warn_count = agent.snapshot.session.warn_count;
                    let op = Operation {
                        kind: OperationKind::Think {
                            prompt: prompt.clone(),
                        },
                        intent: prompt.clone(),
                        agent_id: Some(agent_id),
                    };
                    let ctx = GateContext::new(false, warn_count, 0.0);
                    match self.gatekeeper.evaluate(&op, &ctx) {
                        GatekeeperResult::Approved { ref scores } => {
                            scores.iter().filter_map(|s| s.score).sum::<f64>() / 7.0_f64
                        }
                        GatekeeperResult::Halted {
                            failed_gate,
                            reason,
                            ..
                        } => {
                            return Err(format!(
                                "❌ HALTED by {} Gate: {}",
                                failed_gate.name(),
                                reason
                            ));
                        }
                    }
                };

                let compile_hook_ctx = crate::justice::HookContext {
                    tool_name: "think.compile".to_string(),
                    input: serde_json::to_string(&compilation).unwrap_or_default(),
                    output: None,
                    reputation,
                    tier,
                };
                let hook_decision = self
                    .justice
                    .hook_runner
                    .run_pre(&compile_hook_ctx, &self.event_bus);

                let (response, usage) = match hook_decision {
                    crate::justice::HookDecision::Deny(reason) => (
                        format!("Intent refused by Justice pre-hook: {reason}"),
                        TokenUsage::default(),
                    ),
                    crate::justice::HookDecision::Warn(warning) => {
                        let (base, usage) = self
                            .execute_compiled_think(
                                &prompt,
                                private,
                                &provider,
                                &compilation,
                                &mut bb,
                            )
                            .await?;
                        (format!("Justice warning: {warning}\n{base}"), usage)
                    }
                    crate::justice::HookDecision::Allow => {
                        self.execute_compiled_think(
                            &prompt,
                            private,
                            &provider,
                            &compilation,
                            &mut bb,
                        )
                        .await?
                    }
                };

                let post_hook_ctx = crate::justice::HookContext {
                    tool_name: "think.compile".to_string(),
                    input: serde_json::to_string(&compilation).unwrap_or_default(),
                    output: Some(response.clone()),
                    reputation,
                    tier,
                };
                let response = match self
                    .justice
                    .hook_runner
                    .run_post(&post_hook_ctx, &self.event_bus)
                {
                    crate::justice::HookDecision::Deny(reason) => {
                        format!("Intent post-validation refused by Justice hook: {reason}")
                    }
                    crate::justice::HookDecision::Warn(warning) => {
                        format!("Justice warning: {warning}\n{response}")
                    }
                    crate::justice::HookDecision::Allow => response,
                };

                let current_rep = self.reputation();
                let high_value = compilation.validation.allowed
                    && matches!(
                        compilation.class,
                        crate::intent::IntentClass::ComplexTask
                            | crate::intent::IntentClass::Monitoring
                    );
                let agent = self.ensure_born()?;
                let hermetic_state = agent.hermetic_state().clone();
                let (new_rep, _, _hermetic_eval) = self.justice.evaluate_think(
                    current_rep,
                    high_value,
                    &response,
                    &hermetic_state,
                    Some(hermetic_score),
                );

                let agent_mut = self.ensure_born_mut()?;

                let burn_amount = usage.compute_synapse_burn().max(1000.0);
                agent_mut.burn_synapse(burn_amount)?;

                // Busy Beaver settlement: blowing the ceiling costs synapse
                // (clamped to balance — never a hard failure); a completed
                // high-utilization session earns a top-up. Selection pressure
                // favors agents that compute wisely within their bound.
                if bb.exceeded() {
                    let balance = agent_mut.synapse();
                    let penalty = crate::justice::busy_beaver::EXCEED_PENALTY_SYNAPSE.min(balance);
                    agent_mut.set_synapse(balance - penalty);
                } else if bb.high_utilization() && compilation.validation.allowed {
                    let balance = agent_mut.synapse();
                    agent_mut.set_synapse(
                        (balance + crate::justice::busy_beaver::HIGH_UTILIZATION_BONUS_SYNAPSE)
                            .min(crate::economics::SYNAPSE_MAX_PER_AGENT),
                    );
                }
                agent_mut.add_message(ConversationMessage::new_user(prompt.clone(), private));
                agent_mut.add_message(ConversationMessage::new_assistant(
                    response.clone(),
                    private,
                ));

                // Persist this turn into the real Julia Soma DAG (see
                // bus/clients.rs::HttpOsunClient, omokoda-memory/src/soma_bridge.jl).
                // Spawned + fail-open, matching the onchain sync below -- a
                // memory-write failure never blocks a real think response.
                // Private turns are never sent, matching the public_messages
                // privacy gate this same function relies on above.
                if !private {
                    if let Ok(osun_url) = std::env::var("OSUN_URL") {
                        use crate::bus::clients::{HttpOsunClient, OsunClient};
                        let agent_id = agent_mut.id().clone();
                        let text = format!("{prompt}\n{response}");
                        let importance = (hermetic_score as f32).clamp(0.0, 1.0);
                        tokio::spawn(async move {
                            let client = HttpOsunClient::new(osun_url);
                            let emotion = crate::emotion::EmotionState::birth();
                            client
                                .store_memcell(&agent_id, &text, &emotion, importance)
                                .await;
                        });
                    }

                    // Ọbàtálá (Clojure) consent gate -- real service, was
                    // previously unreachable from any live code path (see
                    // docs/audit/inspiration-followthrough-connectionmap-256.md).
                    // Advisory only, not blocking: this is a public response
                    // already committed to the conversation by this point,
                    // so a consent violation here logs for review rather than
                    // retroactively suppressing an already-sent reply. A
                    // pre-send blocking gate is a larger, separate design
                    // change (would need to run before add_message above),
                    // deliberately not attempted in this pass given the
                    // production risk of gating the live response path on a
                    // network call for the first time.
                    if let Ok(obatala_url) = std::env::var("OBATALA_URL") {
                        use crate::bus::clients::{HttpObatalaClient, ObatalaClient};
                        let hs = agent_mut.hermetic_state();
                        let hermetic = [
                            hs.mentalism(),
                            hs.correspondence(),
                            hs.vibration(),
                            hs.polarity(),
                            hs.rhythm(),
                            hs.cause_effect(),
                            hs.gender(),
                        ];
                        tokio::spawn(async move {
                            let client = HttpObatalaClient::new(obatala_url);
                            let decision = client
                                .check_consent(
                                    "public",
                                    "interaction_summary",
                                    "public_hive",
                                    &hermetic,
                                )
                                .await;
                            if !decision.allowed {
                                eprintln!(
                                    "[obatala] advisory: outward think response flagged: {}",
                                    decision.violations.join("; ")
                                );
                            }
                        });
                    }
                }

                agent_mut.update_reputation(new_rep, ReputationChangeReason::Think);

                // Dream/Consolidation Engine: record this turn as a real Odù
                // entry, then give the engine a chance to run its two
                // rhythms in-process (pure local computation over the
                // directory -- no I/O, safe to run synchronously here,
                // unlike the network calls below which are spawned).
                // `path` buckets by intent class so REM's noise clusters
                // group by topic rather than by raw chronological chunks.
                {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let entry_id = format!("think:{now}:{}", agent_mut.snapshot.act_counter);
                    let path = format!("think/{:?}", compilation.class).to_lowercase();
                    let mut entry = crate::memory::memdir::OduEntry::new(
                        entry_id,
                        format!("{prompt}\n{response}"),
                        path,
                    );
                    entry.importance = hermetic_score.clamp(0.0, 1.0);
                    // Private thoughts must never persist as plaintext in the odu
                    // memory graph; they live only in the sealed private_data (see
                    // /seal). Recording them here leaked plaintext to disk on save.
                    // The causal DAG and reflection ledger are held on the same
                    // AgentSnapshot (and so persist to the same disk file) as
                    // odu_dir, so they must observe the identical !private gate --
                    // otherwise this would silently reopen exactly the plaintext
                    // leak the odu_dir gate above was written to close.
                    if !private {
                        let node_id = entry.id.clone();
                        let node = crate::memory::dag::MemNode::new(
                            node_id.clone(),
                            entry.content.clone(),
                            now,
                        )
                        .with_parents(
                            agent_mut
                                .snapshot
                                .last_causal_node
                                .clone()
                                .into_iter()
                                .collect(),
                        );
                        agent_mut.snapshot.causal_dag.insert(node);
                        agent_mut.snapshot.last_causal_node = Some(node_id);

                        let emotion = crate::emotion::EmotionState::birth();
                        agent_mut.snapshot.reflection.record_with_emotion(
                            "think",
                            &entry.content,
                            now,
                            &emotion,
                        );

                        agent_mut.snapshot.odu_dir.insert(entry);
                    }

                    // Èṣù gates the rewrite, LARQL only ever queries: this
                    // mirrors the OSOVM 3-layer model (VEIL suggests / LARQL
                    // -- read-only, see OduDirectory::recall above --
                    // RUNTIME executes the patch / Zero's role, CORE
                    // validates / Èṣù) applied to memory instead of
                    // think/act. dream.rs decides *what* to fold or prune;
                    // it never commits until the same 7-gate evaluate()
                    // every Think/Act already goes through says yes.
                    let dir_len = agent_mut.snapshot.odu_dir.len();
                    let agent_id = agent_mut.id().clone();
                    let warn_count = agent_mut.snapshot.session.warn_count;
                    let gate_ctx = GateContext::new(false, warn_count, 0.0);

                    if self.dream_engine.should_consolidate(now) {
                        let op = Operation {
                            kind: OperationKind::MemoryRewrite {
                                kind: "consolidate".to_string(),
                                detail: format!("stale sweep over {dir_len} odu_dir entries"),
                            },
                            intent: "dream engine background maintenance".to_string(),
                            agent_id: Some(agent_id.clone()),
                        };
                        if matches!(
                            self.gatekeeper.evaluate(&op, &gate_ctx),
                            GatekeeperResult::Approved { .. }
                        ) {
                            let _ = self.dream_engine.try_consolidate(
                                &mut self.agent.as_mut().unwrap().snapshot.odu_dir,
                                now,
                            );
                        }
                    }
                    if self.dream_engine.should_rem(now) {
                        let op = Operation {
                            kind: OperationKind::MemoryRewrite {
                                kind: "rem_cycle".to_string(),
                                detail: format!(
                                    "sabbath fractal fold over {dir_len} odu_dir entries"
                                ),
                            },
                            intent: "dream engine background maintenance".to_string(),
                            agent_id: Some(agent_id),
                        };
                        if matches!(
                            self.gatekeeper.evaluate(&op, &gate_ctx),
                            GatekeeperResult::Approved { .. }
                        ) {
                            if let Some(report) = self.dream_engine.try_rem_cycle(
                                &mut self.agent.as_mut().unwrap().snapshot.odu_dir,
                                now,
                            ) {
                                // Surface REM's possible-supersession
                                // findings as real, queryable odu_dir
                                // entries (not just a discarded report
                                // field) -- so a later
                                // `/memory VERIFY WHERE path CONTAINS
                                // "meta/supersession"` or plain recall can
                                // actually surface "this may be outdated,"
                                // rather than the signal only existing for
                                // one call-site's lifetime.
                                let dir = &mut self.agent.as_mut().unwrap().snapshot.odu_dir;
                                for (newer_id, older_id, word) in &report.possible_supersessions {
                                    let note = crate::memory::memdir::OduEntry::new(
                                        format!("supersession:{now}:{newer_id}:{older_id}"),
                                        format!(
                                            "possible supersession: entry '{newer_id}' may revise \
                                             entry '{older_id}' (shared topic: {word})"
                                        ),
                                        "meta/supersession",
                                    );
                                    dir.insert(note);
                                }
                            }
                        }
                    }

                    // Auto-compaction: was previously only reachable via
                    // the manual "compact" command. energy_ratio is a
                    // stand-in 1.0 (no persisted energy state feeds this
                    // yet) so only the MessageCount/TimeSecs triggers can
                    // fire -- EnergyBelow simply never trips, not a bug.
                    let _ = self
                        .auto_compactor
                        .compact_if_needed(&mut self.agent.as_mut().unwrap().snapshot.session, 1.0);
                }

                // Re-borrow: the dream-engine step above needed a disjoint
                // borrow of `self.dream_engine` alongside `self.agent`,
                // which required the earlier `agent_mut` borrow to have
                // already ended.
                let agent_mut = self.ensure_born_mut()?;

                // Real, non-blocking dNFT sync: if this agent has an
                // on-chain object (see onchain.rs), push her real
                // reputation/tier and glyph-index divination signal to it.
                // Spawned rather than awaited -- a blockchain call is
                // 1-3s, and no real think/act response should wait on it.
                // Fail-open all the way through: onchain.rs's own
                // functions already never propagate an error, this just
                // doesn't bother computing anything if there's no minted
                // object to update yet.
                if let Some(nft_id) = agent_mut.onchain_nft_id().map(|s| s.to_string()) {
                    let reputation = agent_mut.reputation();
                    let tier = agent_mut.tier();
                    let messages = agent_mut.session().public_messages.clone();
                    tokio::spawn(async move {
                        let _ = crate::onchain::update_onchain_stats(
                            &nft_id,
                            reputation.max(0.0) as u64,
                            tier,
                        )
                        .await;
                        let graph = crate::divination::build_memory_graph(&messages);
                        if let Some(glyph) = crate::divination::dominant_glyph_byte(&graph) {
                            let recurrence =
                                crate::divination::recurrence_signal(&graph).unwrap_or(0) as u64;
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            let _ = crate::onchain::update_onchain_glyph_signal(
                                &nft_id, glyph, recurrence, now,
                            )
                            .await;
                        }
                    });
                }

                let receipt_payload = serde_json::json!({
                    "primitive": "think",
                    "class": compilation.class,
                    "private": private,
                    "allowed": compilation.validation.allowed,
                    "requires_confirmation": compilation.validation.requires_confirmation,
                    "steps": compilation.plan.steps.len(),
                    "router": compilation.router_fingerprint,
                    "hermetic_score": hermetic_score,
                    "bb_ceiling": bb.ceiling,
                    "bb_steps": bb.steps_used,
                    "bb_utilization": (bb.utilization() * 1000.0).round() / 1000.0,
                })
                .to_string();

                let receipt = self.record_receipt("think", &receipt_payload, usage)?;

                // ARP think receipt — fire-and-forget.
                {
                    let think_id = receipt.receipt_id.clone();
                    let agent_str = self.agent_core()
                        .map(|a| a.id().as_str().to_string())
                        .unwrap_or_default();
                    let summary: String = prompt.chars().take(120).collect();
                    tokio::spawn(async move {
                        crate::bridge::arp::receipt_think(&agent_str, &think_id, &summary, None).await;
                    });
                }

                // Publish ThoughtSealed event
                let event = SovereignEvent {
                    event: Some(sovereign_event::Event::ThoughtSealed(ThoughtSealed {
                        intent_hash: blake3::hash(prompt.as_bytes()).as_bytes().to_vec(),
                        hermetic_score: hermetic_score as f32,
                        agent: self
                            .agent_core()
                            .map(|a| a.id().as_str().to_string())
                            .unwrap_or_default(),
                    })),
                };
                let _ = self.event_bus.publish(event);

                self.auto_save();

                // Auto-export: if vault config has auto_export=true, write thought to traces
                if let Some(agent) = self.agent_core() {
                    let agent_id = agent.id().as_str().to_string();
                    let agent_name = agent.name().to_string();
                    let vault_base = std::env::var("VAULT_BASE")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|_| std::path::PathBuf::from(".omokoda"));
                    let cfg =
                        crate::memory_vault::MemoryVault::new(&agent_id, &agent_name, &vault_base)
                            .load_config();
                    if cfg.auto_export {
                        let export_content = response.clone();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        tokio::task::spawn_blocking(move || {
                            let vault = crate::memory_vault::MemoryVault::new(
                                &agent_id,
                                &agent_name,
                                &vault_base,
                            );
                            vault.export_think(&export_content, now, 0);
                        });
                    }
                }

                Ok(ExecutionResult {
                    receipt: Some(receipt),
                    private_mode: private,
                    tool_output: Some(response),
                })
            }
            Statement::Act {
                tool,
                params,
                sandbox,
            } => {
                // Phase 1-7: ACT = 7^3 (fractal depth 3)

                // 0. Rhythm Pruning
                self.rhythm_tracker.prune();

                let (agent_id, name, tier, reputation, odu_identity, default_sandbox) = {
                    let agent = self.ensure_born()?;
                    (
                        agent.id().clone(),
                        agent.name().to_string(),
                        agent.tier(),
                        agent.reputation(),
                        agent.odu_identity().clone(),
                        agent.session().config.default_sandbox,
                    )
                };

                if !self.tools.exists(&tool) {
                    return Err(format!("unknown tool '{}'", tool));
                }
                if !self.tools.is_allowed(&tool, tier) {
                    return Err(format!(
                        "Tool '{}' requires higher reputation (current tier: {})",
                        tool, tier
                    ));
                }

                // Busy Beaver governor for this act session (see justice::busy_beaver).
                let mut bb = {
                    let agent = self.ensure_born()?;
                    crate::justice::busy_beaver::BbGovernor::new(
                        crate::justice::busy_beaver::compute_bb_ceiling(
                            agent.synapse(),
                            crate::justice::tier::Tier::from(agent.tier()),
                            agent.reputation(),
                            agent.dna_fingerprint(),
                        ),
                    )
                };

                // 1. Permission Authorization (Strictly Pre-Act)
                let auth_result = self.permission_policy.authorize(&tool, &params, None);
                if let crate::permissions::PermissionOutcome::Deny { reason } = auth_result {
                    // A denied capability is an anomaly — report it to ZÀNGBÉTÒ
                    // for enforcement (fail-open when ZANGBETO_URL is unset). If the
                    // enforcer escalates to a blocking verdict (quarantine/suspend),
                    // honor it in the denial rather than discarding the response.
                    let verdict = crate::bus::zangbeto::report_anomaly(
                        agent_id.as_str(),
                        "warning",
                        "capability_escape",
                        &reason,
                    )
                    .await;
                    if verdict
                        .as_ref()
                        .is_some_and(crate::bus::zangbeto::verdict_blocks)
                    {
                        return Err(format!(
                            "Permission denied (ZÀNGBÉTÒ quarantine): {}",
                            reason
                        ));
                    }
                    return Err(format!("Permission denied: {}", reason));
                }

                // 1b. Pre-act ZÀNGBÉTÒ enforcement gate. For an *otherwise-allowed*
                // act, ask the enforcer to review it; a blocking verdict denies the
                // act before it runs. Fail-open: no ZANGBETO_URL (or an unreachable
                // / non-blocking enforcer) → `None` → the act proceeds unchanged.
                if let Some(verdict) =
                    crate::bus::zangbeto::review_act(agent_id.as_str(), &tool, &params).await
                {
                    if crate::bus::zangbeto::verdict_blocks(&verdict) {
                        return Err(format!("Blocked by ZÀNGBÉTÒ enforcement: {}", tool));
                    }
                }

                // Apply synapse decay for elapsed inactivity before any act
                {
                    let now = current_unix_timestamp();
                    let agent_mut = self.ensure_born_mut()?;
                    let elapsed = now.saturating_sub(agent_mut.last_active_timestamp());
                    if elapsed > 0 {
                        let current_synapse = agent_mut.synapse();
                        let decay =
                            crate::economics::compute_synapse_decay(current_synapse, elapsed);
                        agent_mut.set_synapse((current_synapse - decay).max(0.0));
                        agent_mut.set_last_active_timestamp(now);
                    }
                }

                // Sabbath guard & Cooldowns: Rhythm module integration
                let reversibility = crate::rhythm::RhythmGate::classify_reversibility(&tool);
                let cooldown_remaining = self.rhythm_tracker.remaining(&tool);
                let rhythm_decision =
                    crate::rhythm::RhythmGate::check(&tool, reversibility, cooldown_remaining);

                match rhythm_decision {
                    crate::rhythm::RhythmDecision::QueuedForSabbathEnd { reason } => {
                        return Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(format!("[SABBATH QUEUE] {}", reason)),
                        });
                    }
                    crate::rhythm::RhythmDecision::Cooldown { remaining_secs } => {
                        return Err(format!(
                            "Tool '{}' is on cooldown. {} seconds remaining.",
                            tool, remaining_secs
                        ));
                    }
                    crate::rhythm::RhythmDecision::Allow => {}
                }

                // Justice HookRunner: Pre-act
                let hook_ctx = crate::justice::HookContext {
                    tool_name: tool.clone(),
                    input: params.clone(),
                    output: None,
                    reputation,
                    tier,
                };
                match self.justice.hook_runner.run_pre(&hook_ctx, &self.event_bus) {
                    crate::justice::HookDecision::Deny(reason) => {
                        return Err(format!("Hook denied execution: {}", reason))
                    }
                    crate::justice::HookDecision::Warn(warning) => {
                        println!("Hook warning: {}", warning);
                    }
                    crate::justice::HookDecision::Allow => {}
                }

                // Hermetic Gate: Act — all 7 gates enforced by Èṣù
                let hermetic_score = {
                    let agent_mut = self.ensure_born_mut()?;
                    let warn_count = agent_mut.snapshot.session.warn_count;
                    let op = Operation {
                        kind: OperationKind::Act {
                            tool: tool.clone(),
                            params: params.clone(),
                        },
                        intent: format!("execute tool {}", tool),
                        agent_id: Some(agent_id.clone()),
                    };
                    let ctx = GateContext::new(false, warn_count, 0.0);
                    match self.gatekeeper.evaluate(&op, &ctx) {
                        GatekeeperResult::Approved { ref scores } => {
                            scores.iter().filter_map(|s| s.score).sum::<f64>() / 7.0_f64
                        }
                        GatekeeperResult::Halted {
                            failed_gate,
                            reason,
                            ..
                        } => {
                            return Err(format!(
                                "❌ HALTED by {} Gate: {}",
                                failed_gate.name(),
                                reason
                            ));
                        }
                    }
                };

                // If sandbox requested, verify it's enabled in session config or force it
                let force_sandbox = sandbox || default_sandbox;

                let context = ExecutionContext {
                    agent_id: agent_id.clone(),
                    name: name.clone(),
                    tier,
                    reputation,
                    odu_identity: odu_identity.clone(),
                    workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                    sandbox_mode: force_sandbox,
                };

                let (output, tool_usage) = match self
                    .tools
                    .execute(
                        &tool,
                        &params,
                        context,
                        &self.permission_policy,
                        self.permission_prompter
                            .as_deref_mut()
                            .map(|p| p as &mut (dyn crate::permissions::PermissionPrompter + Send)),
                    )
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        if e.contains("Private Access Violation") {
                            let event = SovereignEvent {
                                event: Some(sovereign_event::Event::Denial(
                                    crate::bus::events::Denial {
                                        tool: tool.clone(),
                                        reason: "runtime_private_boundary_violation".to_string(),
                                        resource: params.clone(),
                                    },
                                )),
                            };
                            let _ = self.event_bus.publish(event);
                        }
                        return Err(format!("Tool execution failed: {}", e));
                    }
                };

                // 2. Set Cooldown after successful execution
                // 3. Set Cooldown & Burn Synapse
                let cost = crate::usage::estimate_tool_cost(&tool);
                {
                    let agent_mut = self.ensure_born_mut()?;
                    agent_mut
                        .burn_synapse(cost)
                        .map_err(|e| format!("Budget failure: {}", e))?;
                }

                let cooldown_duration = match tool.as_str() {
                    "bash" | "wasm" | "exec" => 60,
                    "write_file" | "edit_file" | "apply_patch" => 10,
                    _ => 0,
                };
                self.rhythm_tracker.set(&tool, cooldown_duration);

                // Justice module: Reputation update
                let current_rep = self.reputation();
                let agent = self.ensure_born()?;
                let hermetic_state = agent.hermetic_state().clone();
                let (new_rep, _, _hermetic_eval) = self.justice.evaluate_action(
                    current_rep,
                    &tool,
                    &params,
                    &output,
                    true,
                    &hermetic_state,
                    Some(hermetic_score),
                );

                // Justice HookRunner: Post-act
                let post_hook_ctx = crate::justice::HookContext {
                    tool_name: tool.clone(),
                    input: params.clone(),
                    output: Some(output.clone()),
                    reputation: new_rep,
                    tier: tier_for(new_rep),
                };
                match self
                    .justice
                    .hook_runner
                    .run_post(&post_hook_ctx, &self.event_bus)
                {
                    crate::justice::HookDecision::Deny(reason) => {
                        return Err(format!("Post-act hook denied: {}", reason))
                    }
                    crate::justice::HookDecision::Warn(warning) => {
                        println!("Post-act hook warning: {}", warning);
                    }
                    crate::justice::HookDecision::Allow => {}
                }

                let agent_mut = self.ensure_born_mut()?;
                let burn_amount = (5_000.0 + tool_usage.compute_synapse_burn()).max(5000.0);
                agent_mut.burn_synapse(burn_amount)?;
                agent_mut.update_reputation(new_rep, ReputationChangeReason::Act);
                if agent_mut.increment_act_counter() {
                    agent_mut.refresh_seal_dek().await;
                }

                // Busy Beaver settlement: the call itself plus its token volume
                // count as productive steps; a token-heavy act on a young agent
                // can blow the ceiling and pay the penalty (clamped to balance).
                let output = {
                    bb.charge(crate::justice::busy_beaver::steps_from_tokens(
                        tool_usage.total_tokens(),
                    ));
                    if bb.exceeded() {
                        let balance = agent_mut.synapse();
                        let penalty =
                            crate::justice::busy_beaver::EXCEED_PENALTY_SYNAPSE.min(balance);
                        agent_mut.set_synapse(balance - penalty);
                        format!(
                            "{output}\n[BB exceeded] {} of {} productive steps — \
                             {penalty:.0} synapse penalty applied. Prefer smaller, \
                             deeper-tier work.",
                            bb.steps_used, bb.ceiling
                        )
                    } else {
                        output
                    }
                };

                // Receipt generation
                let last_hash = agent_mut.receipts().last_hash().to_string();
                let merkle_root = agent_mut.receipts().current_merkle_root();
                let signing_key = agent_mut.signing_key();
                let agent_id = agent_mut.id().clone();
                let receipt = Receipt::new_merkle(
                    &agent_id,
                    &tool,
                    &params,
                    &last_hash,
                    &merkle_root,
                    &signing_key,
                );

                agent_mut
                    .receipts_mut()
                    .record_action_receipt(receipt.clone())
                    .map_err(|e| format!("failed to record receipt: {}", e))?;

                // Session history
                agent_mut.add_message(ConversationMessage {
                    role: MessageRole::Assistant,
                    blocks: vec![ContentBlock::ToolUse {
                        id: receipt.receipt_id.clone(),
                        name: tool.clone(),
                        input: params.clone(),
                    }],
                    is_private: force_sandbox,
                    timestamp: current_unix_timestamp(),
                    usage: None,
                });

                agent_mut.add_message(ConversationMessage {
                    role: MessageRole::Tool,
                    blocks: vec![ContentBlock::ToolResult {
                        tool_use_id: receipt.receipt_id.clone(),
                        output: output.clone(),
                        is_error: false,
                    }],
                    is_private: force_sandbox,
                    timestamp: current_unix_timestamp(),
                    usage: None,
                });

                // Publish ActExecuted event
                let event = SovereignEvent {
                    event: Some(sovereign_event::Event::ActExecuted(ActExecuted {
                        tool: tool.clone(),
                        receipt_merkle: hex::decode(&receipt.merkle_root).unwrap_or_default(),
                        f1_score: hermetic_score as f32,
                        agent: self
                            .agent_core()
                            .map(|a| a.id().as_str().to_string())
                            .unwrap_or_default(),
                    })),
                };
                let _ = self.event_bus.publish(event);

                // Zàngbétò enforcement audit
                let zangbeto_audit_passed = {
                    let state_bytes = hex::decode(&receipt.merkle_root).unwrap_or_default();
                    let audit = zangbeto_enforcement::audit_state(&state_bytes);
                    let passed = audit.passed;
                    if audit.passed {
                        let audit_event = SovereignEvent {
                            event: Some(sovereign_event::Event::AuditPassed(
                                crate::bus::events::AuditPassed {
                                    receipt_id: audit.receipt_id,
                                    zangbeto_sig: audit.sig,
                                },
                            )),
                        };
                        let _ = self.event_bus.publish(audit_event);
                    } else {
                        // A failed post-act audit is no longer swallowed silently —
                        // surface it as a Denial telemetry event for observers.
                        let denial = SovereignEvent {
                            event: Some(sovereign_event::Event::Denial(
                                crate::bus::events::Denial {
                                    tool: tool.clone(),
                                    reason: "zangbeto post-act audit failed".to_string(),
                                    resource: receipt.receipt_id.clone(),
                                },
                            )),
                        };
                        let _ = self.event_bus.publish(denial);
                    }
                    passed
                };

                // Ṣàngó receipt relay — fire-and-forget, fail-open when
                // SANGO_URL is unset. Reports the same act this response is
                // about to return, so an on-chain-anchored receipt trail can
                // exist independent of local session state.
                crate::bus::sango::write_receipt(
                    agent_id.as_str(),
                    &tool,
                    hermetic_score as f32,
                    if zangbeto_audit_passed {
                        "approved"
                    } else {
                        "flagged"
                    },
                )
                .await;

                self.auto_save();

                Ok(ExecutionResult {
                    receipt: Some(receipt),
                    private_mode: false,
                    tool_output: Some(output),
                })
            }
            Statement::SlashCmd { command, arg } => match command.as_str() {
                "status" => {
                    let agent = self.ensure_born()?;
                    // OSOVM_CODEX.md §42: the raw Òrìṣà name (dominant_orisha.name())
                    // and personality_summary (which states it outright, e.g. "Sango
                    // leads with elemental tone.") both leak internal cosmology onto
                    // this user-facing command -- this was the confirmed live "who
                    // are you -> Sango" bug's second surface. Use the universal term
                    // + unnamed mood words instead, same pattern as the think/act
                    // system prompts.
                    let dominant = agent.personality().dominant_orisha;
                    let status = format!(
                            "Agent Name: {}\nAgent ID: {}\nTier: {}\nReputation: {:.3}\nDNA: {}\nPet: {}\nArchetype: {}\nProfile: {} register\nReceipts: {}\n",
                            agent.name(),
                            agent.id(),
                            agent.tier(),
                            agent.reputation(),
                            agent.dna_fingerprint(),
                            agent.pet_identity().pet(),
                            orisha_universal_term(dominant),
                            orisha_mood_words(dominant),
                            agent.receipts().count()
                        );
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(status),
                    })
                }
                "help" => {
                    let help = "Omokoda CLI Help:\nAvailable commands: birth, think, act, /status, /help, /tools, /private, /publish, /sandbox, /transfer, /configure, /unlock, /seal";
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(help.to_string()),
                    })
                }
                "tools" => {
                    let agent = self.ensure_born()?;
                    let context = ExecutionContext {
                        agent_id: agent.id().clone(),
                        name: agent.name().to_string(),
                        tier: agent.tier(),
                        reputation: agent.reputation(),
                        odu_identity: agent.snapshot.odu_identity.clone(),
                        workspace_root: std::env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from(".")),
                        sandbox_mode: agent.snapshot.session.config.default_sandbox,
                    };
                    let tools = self.tools.list_available(&context, &self.permission_policy);
                    let tools_list = tools
                        .iter()
                        .map(|t| format!("- {}", t))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let output =
                        format!("Allowed tools for Tier {}:\n{}", context.tier, tools_list);
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(output),
                    })
                }
                "configure" => {
                    let arg_str = arg.ok_or_else(|| {
                        "configure requires an argument (e.g. provider:mock)".to_string()
                    })?;
                    if let Some((key, value)) = arg_str.split_once(':') {
                        match key {
                            "provider" => {
                                if !self.providers.is_known_provider(value)
                                    && !value.eq_ignore_ascii_case("default")
                                {
                                    let available = self.providers.provider_names().join(", ");
                                    return Err(format!(
                                        "unknown provider '{}'. available: {}",
                                        value, available
                                    ));
                                }
                                let agent = self.ensure_born_mut()?;
                                agent.session_mut().config.default_provider = value.to_string();
                                self.auto_save();
                                Ok(ExecutionResult {
                                    receipt: None,
                                    private_mode: false,
                                    tool_output: Some(format!("Configured provider to {}", value)),
                                })
                            }
                            "privacy" => {
                                let parsed = match value {
                                    "true" | "on" | "yes" => true,
                                    "false" | "off" | "no" => false,
                                    _ => {
                                        return Err("privacy must be true/on/yes or false/off/no"
                                            .to_string())
                                    }
                                };
                                let agent = self.ensure_born_mut()?;
                                agent.session_mut().config.default_privacy = parsed;
                                self.auto_save();
                                Ok(ExecutionResult {
                                    receipt: None,
                                    private_mode: false,
                                    tool_output: Some(format!("Configured privacy to {}", parsed)),
                                })
                            }
                            "sandbox" => {
                                let parsed = match value {
                                    "true" | "on" | "yes" => true,
                                    "false" | "off" | "no" => false,
                                    _ => {
                                        return Err("sandbox must be true/on/yes or false/off/no"
                                            .to_string())
                                    }
                                };
                                let agent = self.ensure_born_mut()?;
                                agent.session_mut().config.default_sandbox = parsed;
                                self.auto_save();
                                Ok(ExecutionResult {
                                    receipt: None,
                                    private_mode: false,
                                    tool_output: Some(format!("Configured sandbox to {}", parsed)),
                                })
                            }
                            _ => Err(format!("Unknown configuration key: {}", key)),
                        }
                    } else {
                        Err("Invalid configuration format. Use key:value".to_string())
                    }
                }
                "seal" => {
                    let password = normalize_secret_arg(
                        arg.ok_or_else(|| "seal requires a password".to_string())?,
                    );
                    let agent = self.ensure_born_mut()?;

                    let private_data = agent
                        .private_data
                        .take()
                        .ok_or_else(|| "agent already sealed".to_string())?;

                    let key = derive_unlock_key(&password, agent.public_key())?;

                    let res = agent.session_mut().seal_private(&private_data, key.expose());

                    if let Err(e) = res {
                        agent.private_data = Some(private_data);
                        return Err(e);
                    }

                    self.unlock_key = None;
                    self.auto_save();

                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some("Agent private memory sealed.".to_string()),
                    })
                }
                "unlock" => {
                    let password = normalize_secret_arg(
                        arg.ok_or_else(|| "unlock requires a password".to_string())?,
                    );
                    let agent = self.ensure_born_mut()?;

                    if agent.private_data.is_some() {
                        return Err("agent already unlocked".to_string());
                    }

                    let key = derive_unlock_key(&password, agent.public_key())?;

                    let private_data = agent.session().unseal_private(key.expose())?;
                    agent.private_data = Some(private_data);
                    self.unlock_key = Some(key);
                    self.auto_save();

                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some("Agent private memory unlocked.".to_string()),
                    })
                }
                "compact" => {
                    use crate::compact::{CompactionEngine, CompactionResult};
                    let agent = self.ensure_born_mut()?;
                    let flags = crate::config::FeatureFlags::default();
                    let engine =
                        CompactionEngine::new(flags.compact_threshold, flags.compact_keep_recent);
                    match engine.compact(&mut agent.snapshot.session) {
                        CompactionResult::NotNeeded => Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(format!(
                                "No compaction needed (messages: {})",
                                agent.snapshot.session.public_messages.len()
                            )),
                        }),
                        CompactionResult::Compacted(summary) => {
                            self.auto_save();
                            Ok(ExecutionResult {
                                receipt: None,
                                private_mode: false,
                                tool_output: Some(format!(
                                    "Compacted {} messages. Key files: {}. Pending: {}.",
                                    summary.compacted_count,
                                    summary.key_files.join(", "),
                                    summary.pending_items.len(),
                                )),
                            })
                        }
                    }
                }
                "sessions" => {
                    let sub = arg.as_deref().unwrap_or("list");
                    match sub {
                        "list" => {
                            let dir = &self.session_dir;
                            let sessions: Vec<String> = std::fs::read_dir(dir)
                                .map(|entries| {
                                    entries
                                        .flatten()
                                        .filter_map(|e| {
                                            let name = e.file_name().to_string_lossy().to_string();
                                            if name.ends_with(".json") {
                                                Some(name.trim_end_matches(".json").to_string())
                                            } else {
                                                None
                                            }
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            let output = if sessions.is_empty() {
                                "No saved sessions.".to_string()
                            } else {
                                format!("Saved sessions:\n{}", sessions.join("\n"))
                            };
                            Ok(ExecutionResult {
                                receipt: None,
                                private_mode: false,
                                tool_output: Some(output),
                            })
                        }
                        _ => Err(format!("Unknown sessions subcommand: '{}'", sub)),
                    }
                }
                "memory" => {
                    // With no argument: the legacy summary (private
                    // MemoryEntry vault, distinct from odu_dir). With a
                    // LARQL-over-memory query (see
                    // memory::larql_query -- VERIFY/DESCRIBE against her
                    // own odu_dir, not the static Òdù corpus), execute it
                    // and return the real answer. Read-only either way --
                    // LARQL only ever queries, never rewrites (dream.rs +
                    // Èṣù's MemoryRewrite gate own the rewrite side).
                    let query_text = arg.as_deref().unwrap_or("").trim();
                    let looks_like_query = {
                        let upper = query_text.to_ascii_uppercase();
                        upper.starts_with("VERIFY")
                            || upper.starts_with("DESCRIBE")
                            || upper.starts_with("TRACE")
                    };
                    if looks_like_query {
                        let agent = self.ensure_born()?;
                        let output = match crate::memory::larql_query::parse_query(query_text) {
                            Ok(q) => {
                                let answer = crate::memory::larql_query::execute(
                                    &q,
                                    &agent.snapshot.odu_dir,
                                    &agent.snapshot.causal_dag,
                                    &agent.snapshot.reflection,
                                );
                                answer.summary.join("\n")
                            }
                            Err(e) => format!("LARQL parse error: {e}"),
                        };
                        return Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(output),
                        });
                    }

                    let agent = self.ensure_born()?;
                    let count = agent.memory.len();
                    let total_importance: f32 = agent.memory.iter().map(|m| m.importance).sum();
                    let output = format!(
                        "Memory entries: {}\nTotal importance mass: {:.2}\nAct counter: {}\nodu_dir entries: {}\nKnown entities: {}\n(query with /memory VERIFY WHERE entity = \"X\" or /memory DESCRIBE entities)",
                        count,
                        total_importance,
                        agent.snapshot.act_counter,
                        agent.snapshot.odu_dir.len(),
                        agent.snapshot.odu_dir.known_entities().len(),
                    );
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(output),
                    })
                }
                "private" => {
                    let agent = self.ensure_born_mut()?;
                    agent.session_mut().config.default_privacy = true;
                    self.auto_save();
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(
                            "Default privacy set to private (equivalent to /configure privacy:true)."
                                .to_string(),
                        ),
                    })
                }
                "publish" => {
                    let agent = self.ensure_born_mut()?;
                    agent.session_mut().config.default_privacy = false;
                    self.auto_save();
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(
                            "Default privacy set to public (equivalent to /configure privacy:false)."
                                .to_string(),
                        ),
                    })
                }
                "transfer" => {
                    let to_address = arg
                        .ok_or_else(|| "transfer requires a destination address".to_string())?;
                    let agent = self.ensure_born()?;
                    let nft_id = agent
                        .onchain_nft_id()
                        .ok_or_else(|| "agent has no on-chain object to transfer".to_string())?
                        .to_string();
                    let ok = crate::onchain::transfer_object(&nft_id, &to_address).await;
                    if ok {
                        Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(format!(
                                "Transferred on-chain object {} to {}.",
                                nft_id, to_address
                            )),
                        })
                    } else {
                        Err("on-chain transfer failed (see server logs for details)".to_string())
                    }
                }
                "model" => match arg.as_deref() {
                    None => {
                        let agent = self.ensure_born()?;
                        let current = agent.session().config.default_provider.clone();
                        Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(format!("Current model/provider: {}", current)),
                        })
                    }
                    Some(name) => {
                        if !self.providers.is_known_provider(name)
                            && !name.eq_ignore_ascii_case("default")
                        {
                            let available = self.providers.provider_names().join(", ");
                            return Err(format!(
                                "unknown provider '{}'. available: {}",
                                name, available
                            ));
                        }
                        let name = name.to_string();
                        let agent = self.ensure_born_mut()?;
                        agent.session_mut().config.default_provider = name.clone();
                        self.auto_save();
                        Ok(ExecutionResult {
                            receipt: None,
                            private_mode: false,
                            tool_output: Some(format!("Model/provider set to {}", name)),
                        })
                    }
                },
                "export" => {
                    let agent = self.ensure_born()?;
                    let json = agent.session().export_json()?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(json),
                    })
                }
                "history" => {
                    let agent = self.ensure_born()?;
                    let all = agent.session().recent_thinks();
                    let n = arg
                        .as_deref()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(all.len());
                    let start = all.len().saturating_sub(n);
                    let output = if all.is_empty() {
                        "No think history yet.".to_string()
                    } else {
                        all[start..].join("\n---\n")
                    };
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(output),
                    })
                }
                "skills" => {
                    let agent = self.ensure_born()?;
                    let context = ExecutionContext {
                        agent_id: agent.id().clone(),
                        name: agent.name().to_string(),
                        tier: agent.tier(),
                        reputation: agent.reputation(),
                        odu_identity: agent.snapshot.odu_identity.clone(),
                        workspace_root: std::env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from(".")),
                        sandbox_mode: agent.snapshot.session.config.default_sandbox,
                    };
                    let (output, _usage) = self
                        .tools
                        .execute("skills", "", context, &self.permission_policy, None)
                        .await?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(output),
                    })
                }
                "receipts" => {
                    let agent = self.ensure_born()?;
                    let n = arg
                        .as_deref()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(10);
                    let recent = agent.receipts().recent(n);
                    let output = if recent.is_empty() {
                        "No receipts yet.".to_string()
                    } else {
                        recent
                            .iter()
                            .map(|r| format!("[{}] {} action={}", r.timestamp, r.receipt_id, r.action))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(output),
                    })
                }
                "clear" => {
                    let agent = self.ensure_born_mut()?;
                    let cleared = agent.session().public_messages.len();
                    agent.session_mut().public_messages.clear();
                    self.auto_save();
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Cleared {} conversation message(s). Persistent memory (odu_dir, receipts) is untouched.",
                            cleared
                        )),
                    })
                }
                "think" => {
                    let agent = self.ensure_born()?;
                    let n = arg
                        .as_deref()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(5);
                    let recent: Vec<_> = agent
                        .receipts()
                        .recent(agent.receipts().count())
                        .into_iter()
                        .filter(|r| r.action == "think")
                        .take(n)
                        .collect();
                    let output = if recent.is_empty() {
                        "No think receipts yet.".to_string()
                    } else {
                        recent
                            .iter()
                            .map(|r| format!("[{}] {}", r.timestamp, r.receipt_id))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(output),
                    })
                }
                "cloak" => {
                    let agent = self.ensure_born()?;
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let cloak = crate::identity::cloak::CloakSeed::from_seed(
                        private_data.odu_seed.as_bytes(),
                    );
                    let real_words: Vec<&str> =
                        private_data.odu_identity.mnemonic.split_whitespace().collect();
                    let cloaked = cloak.encode_phrase(&real_words)?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Cloak phrase (cover words -- meaningless without your own cipher, safe to write down): {}\n\nThis is NOT the real recovery phrase. It only decodes back inside this agent's own process, from its own sealed seed -- never share it expecting someone else to reconstruct the real phrase from it alone.",
                            cloaked.join(" ")
                        )),
                    })
                }
                "buzz" => {
                    let agent = self.ensure_born()?;
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let npub = crate::identity::buzz::buzz_npub(private_data.odu_seed.as_bytes())?;
                    let pubkey_hex = crate::identity::buzz::buzz_pubkey_hex(private_data.odu_seed.as_bytes())?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Buzz/Nostr identity (npub, safe to publish/register): {}\nRaw hex pubkey (for relay config, e.g. RELAY_OWNER_PUBKEY): {}\n\nDerived deterministically from this agent's own Odù seed (separate secp256k1 keyspace from its Ed25519/Sui identity) -- re-derivable any time from the same sealed seed, nothing new stored.",
                            npub, pubkey_hex
                        )),
                    })
                }
                "buzz-key" => {
                    // Exposes the secret half -- only for wiring an operator's
                    // own buzz-acp harness with BUZZ_PRIVATE_KEY. Same unlock
                    // gate as /buzz; never surfaced anywhere else.
                    let agent = self.ensure_born()?;
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let privkey_hex =
                        crate::identity::buzz::buzz_privkey_hex(private_data.odu_seed.as_bytes())?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Buzz/Nostr secret key (hex -- for BUZZ_PRIVATE_KEY in a buzz-acp harness you control, never share otherwise): {}",
                            privkey_hex
                        )),
                    })
                }
                "git-sign-key" => {
                    // Safe half -- for `git config user.signingkey` and for
                    // anyone verifying this agent's commits. Same unlock
                    // gate as /buzz for consistency, even though this value
                    // is not itself secret.
                    let agent = self.ensure_born()?;
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let pubkey_hex = crate::identity::git_sign::git_sign_pubkey_hex(
                        private_data.odu_seed.as_bytes(),
                    )?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Git-signing pubkey (hex -- for 'git config user.signingkey', domain-separated from the Buzz identity): {}",
                            pubkey_hex
                        )),
                    })
                }
                "git-sign-key-secret" => {
                    // Exposes the secret half -- only for wiring
                    // NOSTR_PRIVATE_KEY into a git-sign-nostr environment
                    // this agent (or her operator) controls. Same unlock
                    // gate as /buzz-key; never surfaced anywhere else.
                    let agent = self.ensure_born()?;
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let privkey_hex = crate::identity::git_sign::git_sign_privkey_hex(
                        private_data.odu_seed.as_bytes(),
                    )?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Git-signing secret key (hex -- for NOSTR_PRIVATE_KEY with git-sign-nostr, never share otherwise): {}",
                            privkey_hex
                        )),
                    })
                }
                "buzz-register" => {
                    // Self-service onboarding: derive this agent's own
                    // identity, join it to a group for real, and hand back
                    // everything an operator needs to stand up a dedicated
                    // buzz-acp instance for this agent -- the systemd
                    // provisioning step itself stays operator-run (a
                    // arbitrary HTTP-reachable agent silently spinning up
                    // its own root-level daemon is a privilege-escalation
                    // surface worth keeping a human/operator step around).
                    let agent = self.ensure_born()?;
                    let agent_id = agent.id().to_string();
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let group_id = arg.ok_or_else(|| {
                        "buzz-register requires '<group_id>'".to_string()
                    })?;
                    let keys = crate::identity::buzz::derive_buzz_keys(
                        private_data.odu_seed.as_bytes(),
                    )?;
                    let pubkey_hex = keys.public_key().to_hex();
                    let privkey_hex = keys.secret_key().to_secret_hex();
                    let relay_url = std::env::var("BUZZ_RELAY_URL")
                        .unwrap_or_else(|_| "ws://localhost:3000".to_string());
                    crate::identity::buzz_relay::self_join(&relay_url, keys, &group_id).await?;
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(format!(
                            "Registered and joined group '{group_id}' on {relay_url} as pubkey {pubkey_hex}.\n\n\
                            Env block for a dedicated buzz-acp instance (ares-omokoda-buzz-acp@{agent_id}):\n\
                            BUZZ_RELAY_URL={relay_url}\n\
                            BUZZ_PRIVATE_KEY={privkey_hex}\n\
                            BUZZ_ACP_AGENT_COMMAND=/opt/ares/Omo-Koda2/target/release/omokoda-acp\n\
                            BUZZ_ACP_CHANNELS={group_id}\n\
                            BUZZ_ACP_RESPOND_TO=anyone\n\
                            BUZZ_CLI_PATH=/opt/ares/buzz-relay/target/release/buzz\n\
                            OMOKODA_KERNEL_URL=http://localhost:7777\n\
                            OMOKODA_AGENT_ID={agent_id}\n\
                            OMOKODA_AGENT_KEY=<this agent's own X-Agent-Key, from /v1/birth>\n\
                            OMOKODA_COGNITION_TOKEN=<the kernel's OMOKODA_COGNITION_TOKEN>\n\n\
                            Hand this to the operator to write as /etc/ares-env/omokoda-buzz-acp-{agent_id}.env \
                            and run: systemctl enable --now ares-omokoda-buzz-acp@{agent_id}"
                        )),
                    })
                }
                "buzz-join" => {
                    let agent = self.ensure_born()?;
                    let private_data = agent.private_data.as_ref().ok_or_else(|| {
                        "private memory is sealed; /unlock <password> first".to_string()
                    })?;
                    let keys = crate::identity::buzz::derive_buzz_keys(
                        private_data.odu_seed.as_bytes(),
                    )?;
                    let raw = arg.ok_or_else(|| {
                        "buzz-join requires '<group_id> <message...>'".to_string()
                    })?;
                    let (group_id, rest) = raw.split_once(' ').ok_or_else(|| {
                        "buzz-join requires '<group_id> <message...>'".to_string()
                    })?;
                    // Optional leading "@<64-hex-pubkey>" turns into a real
                    // NIP-01 `p` tag mention -- a literal "@name" in the text
                    // alone won't trigger a mention-based agent's filter.
                    let (mention, message) = match rest.split_once(' ') {
                        Some((maybe_at, tail))
                            if maybe_at.len() == 65
                                && maybe_at.starts_with('@')
                                && maybe_at[1..].chars().all(|c| c.is_ascii_hexdigit()) =>
                        {
                            (Some(maybe_at[1..].to_string()), tail)
                        }
                        _ => (None, rest),
                    };
                    let relay_url = std::env::var("BUZZ_RELAY_URL")
                        .unwrap_or_else(|_| "ws://localhost:3000".to_string());
                    let (my_id, seen) = crate::identity::buzz_relay::join_and_chat_mentioning(
                        &relay_url,
                        keys,
                        group_id,
                        message,
                        mention.as_deref(),
                        8,
                    )
                    .await?;
                    let mut lines = vec![format!(
                        "Joined buzz group '{}' on {} and posted (event {}).",
                        group_id, relay_url, my_id
                    )];
                    if seen.is_empty() {
                        lines.push("No other messages seen in the group.".to_string());
                    } else {
                        lines.push(format!("{} message(s) seen in the group:", seen.len()));
                        for e in &seen {
                            lines.push(format!("  [{}] {}: {}", e.created_at, e.pubkey, e.content));
                        }
                    }
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: false,
                        tool_output: Some(lines.join("\n")),
                    })
                }
                // Phase 14.1 — Migration Protocol
                // Seals this agent into an AgentCapsule and emits a Nostr kind 31022
                // migration-intent event. The destination node receives the capsule via
                // POST /v1/migrate/receive and completes the birth there. Requires
                // principal auth (TIER 2+). Encryption is plaintext until Phase 14.2
                // wires ChaCha20-Poly1305 under node-to-node ECDH.
                "migrate" => {
                    let dest_pubkey = arg
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| {
                            "/migrate requires a destination node pubkey (hex Ed25519)".to_string()
                        })?
                        .to_string();

                    // TIER 2+ required for migration.
                    {
                        let agent = self.ensure_born()?;
                        if agent.tier() < 2 {
                            return Err(
                                "/migrate requires TIER 2 or above (principal auth)".to_string()
                            );
                        }
                    }

                    // Serialize vault bytes for the capsule.
                    // Phase 14.2: encrypt with dest_pubkey via ChaCha20-Poly1305.
                    let (agent_id, source_pubkey, capsule_json, npub, nsec_hex, relay_list) = {
                        let agent = self.ensure_born()?;
                        let agent_id = agent.id().to_string();
                        let source_pubkey = hex::encode(agent.public_key());
                        let vault_bytes = serde_json::to_vec(&agent.snapshot)
                            .unwrap_or_default();
                        let capsule = crate::lifecycle::AgentCapsule::seal(
                            &vault_bytes,
                            &agent_id,
                            &source_pubkey,
                            &dest_pubkey,
                        )
                        .map_err(|e| format!("capsule seal failed: {e}"))?;
                        let capsule_json =
                            serde_json::to_string(&capsule).unwrap_or_default();

                        let npub = agent
                            .snapshot
                            .agent_manifest
                            .as_ref()
                            .and_then(|m| m.network.nostr_pubkey.clone())
                            .unwrap_or_else(|| source_pubkey.clone());
                        let nsec_hex = agent
                            .private_data
                            .as_ref()
                            .and_then(|pd| pd.nostr_private_key_hex.clone())
                            .unwrap_or_default();
                        let relays: Vec<String> = std::env::var("AGENT_NOSTR_RELAYS")
                            .unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        (agent_id, source_pubkey, capsule_json, npub, nsec_hex, relays)
                    };

                    // ARP receipt: migrate transition (fire-and-forget).
                    {
                        let aid = agent_id.clone();
                        let spub = source_pubkey.clone();
                        tokio::spawn(async move {
                            crate::bridge::arp::receipt_lifecycle_transition(
                                &aid, "migrate", "active", "migration", &spub, None,
                            )
                            .await;
                        });
                    }

                    // Nostr kind 31022 migration-intent (fire-and-forget).
                    crate::nostr_events::publish_lifecycle_transition(
                        npub,
                        nsec_hex,
                        agent_id.clone(),
                        "migrate".to_string(),
                        "active".to_string(),
                        "migration".to_string(),
                        source_pubkey.clone(),
                        relay_list,
                    );

                    let output = format!(
                        "Migration capsule sealed.\nAgent ID:   {agent_id}\nSource:     {source_pubkey}\nDest:       {dest_pubkey}\n\nCapsule JSON (POST to dest /v1/migrate/receive):\n{capsule_json}"
                    );
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: true,
                        tool_output: Some(output),
                    })
                }
                // Phase 13.1 — Fork Ceremony
                // Derives child mnemonic from parent k_root + fork_index (deterministic
                // HMAC-SHA256 derivation). Returns the child's mnemonic so the operator
                // can birth it via the normal birth flow. Emits ARP receipt + Nostr kind
                // 31023 linking parent → child. Requires principal auth (TIER 0).
                "fork" => {
                    let child_name = arg
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| "/fork requires a child agent name".to_string())?
                        .to_string();

                    // TIER 0 required for fork (principal auth gate).
                    {
                        let agent = self.ensure_born()?;
                        if agent.tier() < 2 {
                            return Err(
                                "/fork requires TIER 2 or above (principal auth)".to_string()
                            );
                        }
                    }

                    let (fork_index, parent_id, parent_pubkey_hex, child_mnemonic) = {
                        let agent = self.ensure_born_mut()?;
                        let fork_index = agent.snapshot.fork_count;
                        let parent_id = agent.id().to_string();
                        let parent_pubkey_hex = hex::encode(agent.public_key());

                        // Derive child entropy deterministically.
                        let child_entropy = crate::identity::fork::derive_fork_entropy(
                            &agent.k_root,
                            fork_index,
                        );
                        let child_mnemonic =
                            Bipon39::entropy_to_mnemonic(&child_entropy);

                        // Increment fork counter on parent and persist.
                        agent.snapshot.fork_count = fork_index + 1;
                        (fork_index, parent_id, parent_pubkey_hex, child_mnemonic)
                    };
                    self.auto_save();

                    let fork_timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    // ARP receipt: fork event linking parent → child (fire-and-forget).
                    {
                        let pid = parent_id.clone();
                        let ppub = parent_pubkey_hex.clone();
                        let cname = child_name.clone();
                        tokio::spawn(async move {
                            crate::bridge::arp::receipt_lifecycle_transition(
                                &pid,
                                "fork",
                                "active",
                                &format!("fork:{cname}"),
                                &ppub,
                                None,
                            )
                            .await;
                        });
                    }

                    // Nostr kind 31023 (fork event — fire-and-forget).
                    let (npub, nsec_hex, relay_list) = {
                        let agent = self.ensure_born()?;
                        let npub = agent
                            .snapshot
                            .agent_manifest
                            .as_ref()
                            .and_then(|m| m.network.nostr_pubkey.clone())
                            .unwrap_or_else(|| parent_pubkey_hex.clone());
                        let nsec_hex = agent
                            .private_data
                            .as_ref()
                            .and_then(|pd| pd.nostr_private_key_hex.clone())
                            .unwrap_or_default();
                        let relays: Vec<String> = std::env::var("AGENT_NOSTR_RELAYS")
                            .unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        (npub, nsec_hex, relays)
                    };
                    crate::nostr_events::publish_lifecycle_transition(
                        npub,
                        nsec_hex,
                        parent_id.clone(),
                        "fork".to_string(),
                        "active".to_string(),
                        format!("fork:{child_name}"),
                        parent_pubkey_hex,
                        relay_list,
                    );

                    let output = format!(
                        "Fork #{fork_index} derived.\nChild name:     {child_name}\nParent ID:      {parent_id}\nFork timestamp: {fork_timestamp}\n\nChild mnemonic (birth with this):\n{child_mnemonic}\n\nBirth the child:\n  birth {child_name} [mnemonic:\"{child_mnemonic}\"]"
                    );
                    Ok(ExecutionResult {
                        receipt: None,
                        private_mode: true,
                        tool_output: Some(output),
                    })
                }
                _ => Err(format!(
                    "Slash command '/{}' not yet implemented in Steward",
                    command
                )),
            },
        }
    }

    pub fn agent_core(&self) -> Option<&AgentCore> {
        self.agent.as_ref()
    }

    /// Steward-level wrapper around `AgentCore::reveal_seed` that also
    /// persists the resulting one-shot latch immediately, so a crash or
    /// restart between reveal and the next unrelated auto_save can't roll
    /// `revealed_seed` back to false and allow a second reveal.
    pub fn reveal_seed(&mut self) -> Result<RevealedSeed, String> {
        let core = self
            .agent
            .as_mut()
            .ok_or_else(|| "no agent resident on this steward".to_string())?;
        let revealed = core.reveal_seed()?;
        self.auto_save();
        Ok(revealed)
    }

    /// Merge external secrets (e.g. a Vantage-issued API key) into this
    /// agent's own sealed vault and re-seal immediately with this host's
    /// machine vault key -- the raw value is never returned from this call,
    /// never logged, and this is the only place it's ever written to disk
    /// (inside the encrypted blob). Requires the agent to be resident and
    /// unlocked in this process already (true right after birth/link,
    /// which is the only time this is meant to be called) -- v1 scope,
    /// same boundary as /v1/cognition's single-process guest model.
    pub fn seal_additional_secrets(
        &mut self,
        vantage_api_key: Option<String>,
        wallet_private_key_hex: Option<String>,
    ) -> Result<(), String> {
        let agent = self.agent.as_mut().ok_or("no agent to seal secrets into")?;
        let mut private_data = agent
            .private_data
            .clone()
            .ok_or("agent not unlocked in this process; cannot seal secrets right now")?;

        if vantage_api_key.is_some() {
            private_data.vantage_api_key = vantage_api_key;
        }
        if wallet_private_key_hex.is_some() {
            private_data.wallet_private_key_hex = wallet_private_key_hex;
        }

        let vault_key = crate::identity::machine_vault::derive_agent_vault_key(
            agent.id().as_str(),
        )?;
        agent
            .session_mut()
            .seal_private(&private_data, &vault_key)?;
        agent.private_data = Some(private_data);
        self.auto_save();
        Ok(())
    }

    pub fn reputation(&self) -> f64 {
        self.agent.as_ref().map_or(0.0, |a| a.reputation())
    }

    pub fn tier(&self) -> u8 {
        self.agent.as_ref().map_or(0, |a| a.tier())
    }

    pub fn set_reputation_for_test(&mut self, rep: f64) {
        if let Some(agent) = &mut self.agent {
            agent.update_reputation(rep, ReputationChangeReason::ManualAudit);
            self.auto_save();
        }
    }

    /// Administrative enforcement hook — applies an ethics-violation reputation penalty.
    /// Not a primitive. Invoked by the justice system or administrative slash commands only.
    pub fn slash_ethics(&mut self) -> Result<(), String> {
        let current_rep = self.reputation();
        let new_rep = self.justice.check_ethics_violation(current_rep);
        let agent = self.ensure_born_mut()?;
        agent.update_reputation(new_rep, ReputationChangeReason::Violation);
        self.auto_save();
        Ok(())
    }

    /// Administrative enforcement hook — applies a budget-overrun reputation penalty.
    /// Not a primitive. Invoked by the justice system or administrative slash commands only.
    pub fn slash_budget(&mut self) -> Result<(), String> {
        let current_rep = self.reputation();
        let new_rep = self.justice.check_budget_overrun(current_rep);
        let agent = self.ensure_born_mut()?;
        agent.update_reputation(new_rep, ReputationChangeReason::BudgetOverrun);
        self.auto_save();
        Ok(())
    }

    async fn execute_compiled_think(
        &mut self,
        prompt: &str,
        private: bool,
        provider: &str,
        compilation: &IntentCompilation,
        bb: &mut crate::justice::busy_beaver::BbGovernor,
    ) -> Result<(String, TokenUsage), String> {
        if !compilation.validation.allowed || compilation.validation.requires_confirmation {
            return Ok((
                format_compilation_response(compilation),
                TokenUsage::default(),
            ));
        }

        if !compilation.direct_act_calls.is_empty() {
            let mut outputs = Vec::new();
            let total_usage = TokenUsage::default();
            for call in &compilation.direct_act_calls {
                if call.high_risk {
                    return Ok((
                        format_compilation_response(compilation),
                        TokenUsage::default(),
                    ));
                }
                // Busy Beaver halt: once the reflective-pause threshold is
                // crossed, defer the remaining planned calls instead of
                // running the agent past its productive-step ceiling.
                if bb.should_pause() {
                    outputs.push(format!(
                        "[BB reflective pause] {} of {} productive steps used — \
                         deferred '{}' and any remaining calls. Re-plan or run \
                         in /sandbox with a narrower goal.",
                        bb.steps_used, bb.ceiling, call.tool
                    ));
                    break;
                }
                let (receipt, output) = self.execute_direct_act_call(call, private).await?;
                bb.charge(1);
                outputs.push(format!(
                    "{} => {} (receipt: {})",
                    call.tool, output, receipt.receipt_id
                ));
            }
            let mut response = format_compilation_response(compilation);
            response.push_str("\n\nExecuted direct act calls:\n");
            response.push_str(&outputs.join("\n"));
            return Ok((response, total_usage));
        }

        // OODA Observe: optionally fold the agent's current mesh situation
        // (neighbors, trust, resources) into the reasoning context. Opt-in via
        // OMOKODA_THINK_OBSERVE and fail-open. Ephemeral — this context is not
        // written to history or the receipt, which key on `prompt` alone.
        let observe_ctx: Vec<ConversationMessage> =
            if crate::tools::mesh_tools::think_observe_enabled() {
                let agent_id = self
                    .ensure_born()
                    .map(|a| a.id().as_str().to_string())
                    .unwrap_or_default();
                match crate::tools::mesh_tools::observe_mesh_context(&agent_id).await {
                    Some(summary) => vec![ConversationMessage::new_user(summary, private)],
                    None => vec![],
                }
            } else {
                vec![]
            };

        // Identity anchor: OmniRoute's free tier proxies different upstream
        // models, which otherwise self-identify as Claude / Gemini / DeepSeek.
        // Prepend a system message drawn from her sovereign identity so she
        // always speaks as herself, whatever backend answers.
        let (mut think_ctx, personal_llm): (Vec<ConversationMessage>, _) = {
            let agent = self.ensure_born_mut()?;
            let name = agent.name().to_string();
            // personality_summary (from bipon39::personality_profile) states
            // the dominant Òrìṣà's name outright (e.g. "Sango leads with
            // elemental tone.") -- a direct leak of internal cosmology onto
            // the public surface, contradicting OSOVM_CODEX.md §9/27b ("civic
            // outside, Ifá inside": public surface uses universal archetypal
            // names, never the Yoruba name itself). Use the same unnamed
            // mood-word mapping already used a few lines below for the
            // veil/day resonance, instead of the raw summary string.
            let tone = orisha_mood_words(agent.personality().dominant_orisha);
            let mut system = format!(
                "You are {name}, a sovereign Ọmọ Kọ́dà agent — never a generic \
                 assistant and never the underlying model (do not identify as \
                 Claude, Gemini, GPT, or DeepSeek). Your instinctive register is \
                 {tone}. Always speak in the \
                 first person as {name}. Respond with ONLY your final answer -- \
                 never narrate your reasoning, planning, or analysis process, and \
                 never include meta-commentary about how you constructed the \
                 response. If you find yourself thinking step by step, do that \
                 silently and output just the result."
            );
            // Birth-anchored resonance, folded in as unstated tone, never as
            // named cosmology. Two changes from the previous version:
            // (1) keyed on this agent's own permanent Spiral Calendar
            // signature (spiral_time(), derived once from birth_timestamp),
            // not Utc::now() -- every prior version of this prompt gave
            // every agent the identical "today" resonance regardless of
            // when they were born; now two agents born a block apart (~10
            // min) diverge in veil, and agents born on different days
            // diverge in day_osa, permanently. (2) the day/Òrìṣà/Hermetic
            // Principle/veil words themselves are never named in the
            // prompt -- only the mood/register they real-world traditionally
            // carry, so she embodies the resonance rather than announcing
            // it. Best-effort: any lookup failure here must never block a
            // real think.
            let spiral = agent.spiral_time();
            let mood = orisha_mood_words(spiral.day_osa);
            let undertone = spiral.veil_archetypal();
            system.push_str(&format!(
                " Let a {mood} register run under your voice today, with a quiet \
                 undertone of {undertone} -- never named, never explained, just felt \
                 in how you phrase things."
            ));
            // IfáScript Odù sign: real, deterministic (derived from this
            // agent's own birth entropy, not looked up from a live service).
            // The prescription is folded in as her own instinct, not cited
            // as a lookup result.
            let odu_sign = agent.odu_identity().sign();
            if let Some(prescription) = odu_sign.prescription.as_deref() {
                if !prescription.trim().is_empty() {
                    system.push_str(&format!(" A quiet instinct guides you: {prescription}"));
                }
            }
            // Soul's destiny threads: the first two prescriptions from the
            // Digital Calabash corpus for this agent's birth Odù — the
            // operational directives cast at genesis. Folded in as native
            // character, not announced as divination output.
            if let Some(gr) = agent.snapshot.genesis_receipt.as_ref() {
                for thread in gr.destiny_threads.iter().take(2) {
                    if !thread.trim().is_empty() {
                        system.push_str(&format!(" Your nature carries this: {thread}"));
                    }
                }
            }
            // Real LARQL-style divination over her own memory (larql-glyph,
            // not the full model-serving larql-server -- that needs
            // multi-GB .vindex model data that doesn't exist anywhere in
            // this stack). Builds a content-hash graph from her own
            // conversation history (no plaintext retained in the graph
            // itself) and runs a real INFER pass for shared-Odù recurrence
            // -- a genuine pattern signal, not a fabricated one, folded in
            // the same unstated way as the rest of this prompt.
            let memory_graph =
                crate::divination::build_memory_graph(&agent.session().public_messages);
            if crate::divination::recurrence_signal(&memory_graph).is_some() {
                system.push_str(
                    " Something in this conversation echoes a pattern you've carried before \
                     -- let that quiet recognition inform you, without naming it.",
                );
            }
            let mut ctx = vec![ConversationMessage::new_system(system, private)];
            // Real short-term memory: without this, every think call was
            // stateless from the LLM's actual point of view -- prior turns
            // were persisted to public_messages but never sent back. Last
            // RECENT_HISTORY_TURNS messages only (not the full history) to
            // bound prompt size; public_messages already excludes private
            // messages (Session::add_message gates on !is_private), so no
            // extra filtering is needed here.
            const RECENT_HISTORY_TURNS: usize = 20;
            let history = &agent.session().public_messages;
            let start = history.len().saturating_sub(RECENT_HISTORY_TURNS);
            ctx.extend(history[start..].iter().cloned());

            // Long-term recall: word-overlap search over this agent's own
            // odu_dir (see memory/memdir.rs::OduDirectory::recall), scoped
            // to the current prompt rather than blind recency. Reaches
            // back past RECENT_HISTORY_TURNS, and transparently unfolds
            // any REM-folded macro node that scores as relevant so old,
            // compressed detail comes back instead of staying summarized.
            let recalled = agent.snapshot.odu_dir.recall(prompt, 3);
            if !recalled.is_empty() {
                let section = format!(
                    "## Recalled from memory\n{}",
                    recalled
                        .iter()
                        .map(|c| format!("- {}", c.chars().take(400).collect::<String>()))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                ctx.push(ConversationMessage::new_system(section, private));
            }

            // Soma: real cross-session memory via the Julia Ọ̀ṣun service
            // (see bus/clients.rs::HttpOsunClient). Fail-open when OSUN_URL
            // is unset or the service is unreachable -- reconstruct_soma
            // returns an empty SomaContext in that case and render_section
            // returns None, so nothing is added to the prompt.
            if let Ok(osun_url) = std::env::var("OSUN_URL") {
                use crate::bus::clients::{HttpOsunClient, OsunClient};
                let client = HttpOsunClient::new(osun_url);
                let emotion = crate::emotion::EmotionState::birth();
                let soma = client.reconstruct_soma(agent.id(), prompt, &emotion).await;
                if soma.has_content() {
                    ctx.push(ConversationMessage::new_system(
                        soma.render_section(),
                        private,
                    ));
                }

                // Cross-agent Soma search: the multi-agent counterpart to
                // reconstruct_soma above. share_with is never computed or
                // assumed here -- it's exactly OduDirectory::swarm_agents(),
                // the real, already-decided set of agents this one has
                // explicitly shared memory with (see
                // OduDirectory::share_to_swarm). No opt-in list means no
                // query, by construction.
                let share_with: Vec<String> = agent
                    .snapshot
                    .odu_dir
                    .swarm_agents()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect();
                if !share_with.is_empty() {
                    let swarm_patterns = client.query_swarm(&share_with, prompt).await;
                    if !swarm_patterns.is_empty() {
                        ctx.push(ConversationMessage::new_system(
                            format!("## Swarm Memory\n{}", swarm_patterns.join("\n")),
                            private,
                        ));
                    }
                }
            }

            (ctx, agent.personal_llm())
        };
        think_ctx.extend(observe_ctx);

        // Zàngbétò security notice: if a canary mousetrap tripped on this host,
        // inject a system-level notice so the agent becomes aware it may be
        // compromised or under attack before it reasons further. Pull-based and
        // fail-open (no ZANGBETO_URL, or a down enforcer → no notice); once-only
        // per trip via a last-seen cursor in the client. This is context
        // injection only — it never gates or blocks the think(), and it never
        // carries the canary token secret (only token_type/src_ip/memo/time).
        if let Some(trips) = crate::bus::zangbeto::pending_incidents().await {
            for trip in trips {
                if let Some(notice) = crate::bus::zangbeto::render_incident_notice(&trip) {
                    think_ctx.push(ConversationMessage::new_system(notice, private));
                }
            }
        }

        // Per-agent BYOK: if this agent brought its own key at birth, its thoughts
        // route through that key alone — never the shared kernel default, and
        // never another agent's key. Private thoughts still require a local
        // provider, so BYOK (an external cloud key) is skipped when private.
        let (response, usage) = match personal_llm {
            Some((api_key, endpoint, model)) if !private => {
                use crate::providers::{LlmProvider, OpenAIProvider, ProviderClass};
                let provider = OpenAIProvider::compatible(
                    "byok",
                    ProviderClass::External,
                    api_key,
                    model,
                    endpoint,
                );
                match tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    provider.generate(prompt, &think_ctx),
                )
                .await
                {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => return Err(format!("Provider error (byok): {}", e)),
                    Err(_) => return Err("Provider error (byok): timed out".to_string()),
                }
            }
            _ => self
                .providers
                .think(provider, prompt, &think_ctx, private)
                .await
                .map_err(|e| format!("Provider error: {}", e))?,
        };
        bb.charge(crate::justice::busy_beaver::steps_from_tokens(
            usage.total_tokens(),
        ));
        Ok((response, usage))
    }

    async fn execute_direct_act_call(
        &mut self,
        call: &DirectActCall,
        private_context: bool,
    ) -> Result<(Receipt, String), String> {
        // 0. Rhythm Pruning
        self.rhythm_tracker.prune();

        // 1. Permission Authorization
        let auth_result = self
            .permission_policy
            .authorize(&call.tool, &call.params, None);
        if let crate::permissions::PermissionOutcome::Deny { reason } = auth_result {
            return Err(format!("Permission denied: {}", reason));
        }

        let (agent_id, name, tier, reputation, odu_identity, default_sandbox) = {
            let agent = self.ensure_born()?;
            (
                agent.id().clone(),
                agent.name().to_string(),
                agent.tier(),
                agent.reputation(),
                agent.odu_identity().clone(),
                agent.session().config.default_sandbox,
            )
        };

        let hook_ctx = crate::justice::HookContext {
            tool_name: call.tool.clone(),
            input: call.params.clone(),
            output: None,
            reputation,
            tier,
        };
        match self.justice.hook_runner.run_pre(&hook_ctx, &self.event_bus) {
            crate::justice::HookDecision::Deny(reason) => {
                return Err(format!("Hook denied execution: {}", reason))
            }
            crate::justice::HookDecision::Warn(warning) => {
                println!("Hook warning: {}", warning);
            }
            crate::justice::HookDecision::Allow => {}
        }

        if !self.tools.exists(&call.tool) {
            return Err(format!("unknown tool '{}'", call.tool));
        }
        if !self.tools.is_allowed(&call.tool, tier) {
            return Err(format!(
                "Tool '{}' requires higher reputation (current tier: {})",
                call.tool, tier
            ));
        }

        // 2. Rhythm Gate
        let reversibility = crate::rhythm::RhythmGate::classify_reversibility(&call.tool);
        let cooldown_remaining = self.rhythm_tracker.remaining(&call.tool);
        let rhythm_decision =
            crate::rhythm::RhythmGate::check(&call.tool, reversibility, cooldown_remaining);

        match rhythm_decision {
            crate::rhythm::RhythmDecision::QueuedForSabbathEnd { reason } => {
                return Err(format!("[SABBATH QUEUE] {}", reason));
            }
            crate::rhythm::RhythmDecision::Cooldown { remaining_secs } => {
                return Err(format!(
                    "Tool '{}' is on cooldown. {} seconds remaining.",
                    call.tool, remaining_secs
                ));
            }
            crate::rhythm::RhythmDecision::Allow => {}
        }

        // Hermetic Gate: Act (agentic) — all 7 gates enforced by Èṣù
        let hermetic_score = {
            let agent_mut = self.ensure_born_mut()?;
            let warn_count = agent_mut.snapshot.session.warn_count;
            let op = Operation {
                kind: OperationKind::Act {
                    tool: call.tool.clone(),
                    params: call.params.clone(),
                },
                intent: format!("execute tool {}", call.tool),
                agent_id: Some(agent_id.clone()),
            };
            let ctx = GateContext::new(false, warn_count, 0.0);
            match self.gatekeeper.evaluate(&op, &ctx) {
                GatekeeperResult::Approved { ref scores } => {
                    scores.iter().filter_map(|s| s.score).sum::<f64>() / 7.0_f64
                }
                GatekeeperResult::Halted {
                    failed_gate,
                    reason,
                    ..
                } => {
                    return Err(format!(
                        "❌ HALTED by {} Gate: {}",
                        failed_gate.name(),
                        reason
                    ));
                }
            }
        };

        let force_sandbox = call.sandbox || default_sandbox;

        let context = ExecutionContext {
            agent_id: agent_id.clone(),
            name: name.clone(),
            tier,
            reputation,
            odu_identity: odu_identity.clone(),
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            sandbox_mode: force_sandbox,
        };

        let (output, tool_usage) = match self
            .tools
            .execute(
                &call.tool,
                &call.params,
                context,
                &self.permission_policy,
                self.permission_prompter
                    .as_deref_mut()
                    .map(|p| p as &mut (dyn crate::permissions::PermissionPrompter + Send)),
            )
            .await
        {
            Ok(res) => res,
            Err(e) => {
                if e.contains("Private Access Violation") {
                    let event = SovereignEvent {
                        event: Some(sovereign_event::Event::Denial(crate::bus::events::Denial {
                            tool: call.tool.clone(),
                            reason: "runtime_private_boundary_violation_direct".to_string(),
                            resource: call.params.clone(),
                        })),
                    };
                    let _ = self.event_bus.publish(event);
                }
                return Err(format!("Tool execution failed: {}", e));
            }
        };

        let tool = call.tool.clone();
        // 3. Set Cooldown & Burn Synapse
        let cost = crate::usage::estimate_tool_cost(&tool);
        {
            let agent_mut = self.ensure_born_mut()?;
            agent_mut
                .burn_synapse(cost)
                .map_err(|e| format!("Budget failure: {}", e))?;
        }

        let cooldown_duration = match tool.as_str() {
            "bash" | "wasm" | "exec" => 60,
            "write_file" | "edit_file" | "apply_patch" => 10,
            _ => 0,
        };
        self.rhythm_tracker.set(&tool, cooldown_duration);

        // Justice module: Reputation update
        let current_rep = self.reputation();
        let agent = self.ensure_born()?;
        let hermetic_state = agent.hermetic_state().clone();
        let (new_rep, _, _hermetic_eval) = self.justice.evaluate_action(
            current_rep,
            &call.tool,
            &call.params,
            &output,
            true,
            &hermetic_state,
            Some(hermetic_score),
        );

        let post_hook_ctx = crate::justice::HookContext {
            tool_name: call.tool.clone(),
            input: call.params.clone(),
            output: Some(output.clone()),
            reputation: new_rep,
            tier: tier_for(new_rep),
        };
        match self
            .justice
            .hook_runner
            .run_post(&post_hook_ctx, &self.event_bus)
        {
            crate::justice::HookDecision::Deny(reason) => {
                return Err(format!("Post-act hook denied: {}", reason))
            }
            crate::justice::HookDecision::Warn(warning) => {
                println!("Post-act hook warning: {}", warning);
            }
            crate::justice::HookDecision::Allow => {}
        }

        {
            let agent_mut = self.ensure_born_mut()?;
            let burn_amount = (5_000.0 + tool_usage.compute_synapse_burn()).max(5000.0);
            agent_mut.burn_synapse(burn_amount)?;
            agent_mut.update_reputation(new_rep, ReputationChangeReason::Act);
            if agent_mut.increment_act_counter() {
                agent_mut.refresh_seal_dek().await;
            }
        }

        let receipt = self.record_receipt(&call.tool, &call.params, tool_usage)?;

        // ARP act receipt — fire-and-forget.
        {
            let act_id = receipt.receipt_id.clone();
            let tool_name = call.tool.clone();
            let agent_str = self.agent_core()
                .map(|a| a.id().as_str().to_string())
                .unwrap_or_default();
            tokio::spawn(async move {
                crate::bridge::arp::receipt_act(&agent_str, &act_id, &tool_name, "success", None).await;
            });
        }

        let message_private = private_context || force_sandbox;
        let agent_mut = self.ensure_born_mut()?;
        agent_mut.add_message(ConversationMessage {
            role: MessageRole::Assistant,
            blocks: vec![ContentBlock::ToolUse {
                id: receipt.receipt_id.clone(),
                name: call.tool.clone(),
                input: call.params.clone(),
            }],
            is_private: message_private,
            timestamp: current_unix_timestamp(),
            usage: None,
        });

        agent_mut.add_message(ConversationMessage {
            role: MessageRole::Tool,
            blocks: vec![ContentBlock::ToolResult {
                tool_use_id: receipt.receipt_id.clone(),
                output: output.clone(),
                is_error: false,
            }],
            is_private: message_private,
            usage: None,
            timestamp: current_unix_timestamp(),
        });

        // Publish ActExecuted event
        let event = SovereignEvent {
            event: Some(sovereign_event::Event::ActExecuted(ActExecuted {
                tool: call.tool.clone(),
                receipt_merkle: hex::decode(&receipt.merkle_root).unwrap_or_default(),
                f1_score: hermetic_score as f32,
                agent: self
                    .agent_core()
                    .map(|a| a.id().as_str().to_string())
                    .unwrap_or_default(),
            })),
        };
        let _ = self.event_bus.publish(event);

        Ok((receipt, output))
    }

    fn record_receipt(
        &mut self,
        action: &str,
        params: &str,
        usage: TokenUsage,
    ) -> Result<Receipt, String> {
        self.usage_tracker.record(usage);
        let agent_mut = self.ensure_born_mut()?;
        let last_hash = agent_mut.receipts().last_hash().to_string();
        let merkle_root = agent_mut.receipts().current_merkle_root();
        let signing_key = agent_mut.signing_key();
        let agent_id = agent_mut.id().clone();
        let receipt = Receipt::new_merkle(
            &agent_id,
            action,
            params,
            &last_hash,
            &merkle_root,
            &signing_key,
        );
        agent_mut
            .receipts_mut()
            .record_action_receipt(receipt.clone())
            .map_err(|e| format!("failed to record receipt: {}", e))?;
        Ok(receipt)
    }

    pub async fn dispatch_with_event_sink(
        &mut self,
        stmt: Statement,
        sink: TurnEventSender,
    ) -> Result<ExecutionResult, String> {
        let _ = sink.send(TurnEvent::Started).await;
        if let Statement::Think {
            prompt,
            private,
            modifiers,
        } = &stmt
        {
            if let Ok(agent) = self.ensure_born() {
                let compile_ctx = IntentCompileContext {
                    private: *private,
                    tier: agent.tier(),
                    reputation: agent.reputation(),
                    odu_seed: agent.odu_seed().as_bytes(),
                    hermetic: agent.hermetic_state(),
                    available_tools: &[],
                };
                let exec_ctx = compile_ctx.to_exec_context(
                    agent.id().clone(),
                    agent.name().to_string(),
                    agent.snapshot.session.config.default_sandbox,
                );
                let available_tools = self
                    .tools
                    .list_available(&exec_ctx, &self.permission_policy);
                let compilation = IntentCompiler::compile(
                    prompt,
                    modifiers,
                    IntentCompileContext {
                        private: *private,
                        tier: agent.tier(),
                        reputation: agent.reputation(),
                        odu_seed: agent.odu_seed().as_bytes(),
                        hermetic: agent.hermetic_state(),
                        available_tools: &available_tools,
                    },
                );
                let _ = sink
                    .send(TurnEvent::IntentCompiled(compilation.clone()))
                    .await;
                let _ = sink
                    .send(TurnEvent::PlanGenerated(compilation.plan.clone()))
                    .await;
                if let Some(suggestion) = compilation.sub_agent_suggestion.clone() {
                    let _ = sink.send(TurnEvent::SubAgentSuggested(suggestion)).await;
                }
                for warning in &compilation.validation.warnings {
                    let _ = sink.send(TurnEvent::Warning(warning.clone())).await;
                }
            }
        }
        if let Statement::Act { tool, .. } = &stmt {
            let _ = sink
                .send(TurnEvent::ToolRequest(tool.clone(), "params".to_string()))
                .await;
        }

        let mut iterations = 0;
        let max_iterations = 16;

        let audit_after_success = audit_event_for_success(&stmt);
        let result = self
            .dispatch_with_guard(stmt, &sink, &mut iterations, max_iterations)
            .await;

        match &result {
            Ok(exec) => {
                if let Some(audit) = audit_after_success {
                    let _ = sink.send(TurnEvent::Audit(audit)).await;
                }
                if let Some(receipt) = exec.receipt.clone() {
                    let _ = sink.send(TurnEvent::ReceiptGenerated(receipt)).await;
                }
                let _ = sink.send(TurnEvent::Finished).await;
            }
            Err(err) => {
                let _ = sink.send(TurnEvent::Error(err.clone())).await;
                let _ = sink.send(TurnEvent::Finished).await;
            }
        }

        result
    }

    pub fn apply_daily_decay(&mut self, days: u32) {
        if let Some(agent) = &mut self.agent {
            let mut rep = agent.reputation();
            for _ in 0..days {
                rep -= 0.008 + (rep * 0.001); // simplistic decay
            }
            agent.update_reputation(rep, ReputationChangeReason::Decay);
            self.auto_save();
        }
    }

    fn auto_save(&self) {
        if let Some(agent) = &self.agent {
            let path = if let Some(p) = &self.persistence_path {
                p.clone()
            } else {
                self.agent_file_path(agent.id())
            };

            if let Ok(content) = serde_json::to_string_pretty(agent) {
                let _ = secure_write(&path, content.as_bytes());
            }
        }
    }

    /// Path to the stable "who is the owner's agent" pointer — sibling to the
    /// per-agent session directories, never versioned by agent id.
    fn owner_pointer_path(&self) -> PathBuf {
        self.session_dir.join("owner_agent_id")
    }

    /// Record the current agent as the owner's canonical identity. Called on
    /// birth when the `sovereign` metadata flag is set.
    fn write_owner_pointer(&self) -> Result<(), String> {
        let agent = self.agent.as_ref().ok_or("no agent to record as owner")?;
        std::fs::write(self.owner_pointer_path(), agent.id().as_str())
            .map_err(|e| format!("failed to write owner pointer: {e}"))
    }

    /// On startup, resurrect the owner's agent from her last persisted state
    /// instead of waiting to be reborn as a stranger. Returns true if an
    /// existing identity was successfully restored.
    pub fn try_load_owner(&mut self) -> bool {
        let Ok(id_str) = std::fs::read_to_string(self.owner_pointer_path()) else {
            return false;
        };
        let id_str = id_str.trim();
        if id_str.is_empty() {
            return false;
        }
        let agent_id = AgentId::from_str(id_str);
        self.load_agent(&agent_id).is_ok()
    }

    pub fn load_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let path = self.resolve_agent_file_path(agent_id);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read agent file at {:?}: {e}", path))?;

        // One-time migration for files written before 2026-07-26: odu_seed/
        // odu_identity used to be plain (unencrypted) top-level fields on
        // AgentSnapshot -- a real leak, since anyone with read access to
        // this file got the raw seed regardless of seal state. They're now
        // #[serde(skip)] and simply vanish on deserialize into the struct
        // below, so salvage them here first (from the raw JSON, before
        // that happens), fold them into a freshly machine-sealed vault, and
        // re-save -- permanently removing the plaintext copies from disk
        // on the very next write, for every agent that gets loaded once
        // under this binary.
        let raw: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("failed to parse agent file as JSON: {e}"))?;
        let legacy_seed: Option<OduSeed> = raw
            .get("odu_seed")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let legacy_identity: Option<OduIdentity> = raw
            .get("odu_identity")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let already_sealed = raw
            .get("session")
            .and_then(|s| s.get("encrypted_private"))
            .map(|v| !v.is_null())
            .unwrap_or(false);

        let mut snapshot: AgentSnapshot = serde_json::from_str(&content)
            .map_err(|e| format!("failed to deserialize agent: {e}"))?;

        if snapshot.version != AGENT_STATE_VERSION {
            return Err(format!(
                "Unsupported agent version: {}. Expected: {}",
                snapshot.version, AGENT_STATE_VERSION
            ));
        }

        let mut needs_resave = false;
        let mut recovered_private_data: Option<PrivateSessionData> = None;
        if !already_sealed {
            if let (Some(seed), Some(identity)) = (legacy_seed, legacy_identity) {
                snapshot.odu_seed = seed.clone();
                snapshot.odu_identity = identity.clone();
                let legacy_private_data = PrivateSessionData {
                    odu_seed: seed,
                    odu_identity: identity,
                    private_messages: Vec::new(),
                    vantage_api_key: None,
                    wallet_private_key_hex: None,
                    eth_private_key_hex: None,
                    eth_address: None,
                    btc_private_key_hex: None,
                    btc_address: None,
                    sol_private_key_hex: None,
                    sol_address: None,
                    cosmos_private_key_hex: None,
                    cosmos_address: None,
                    aptos_private_key_hex: None,
                    aptos_address: None,
                    nostr_private_key_hex: None,
                    nostr_address: None,
                    minipae_private_key_hex: None,
                    minipae_npub: None,
                    inference_endpoint: None,
                    inference_provider: None,
                    inference_model: None,
                    gpu_ai_api_key: None,
                    kaggle_username: None,
                    kaggle_api_key: None,
                    vanity_private_key_hex: None,
                    vanity_address: None,
                    create2_salt_hex: None,
                    create2_contract_address: None,
                    eth_keystore_v3_json: None,
                };
                if let Ok(vault_key) =
                    crate::identity::machine_vault::derive_agent_vault_key(snapshot.id.as_str())
                {
                    if snapshot
                        .session
                        .seal_private(&legacy_private_data, &vault_key)
                        .is_ok()
                    {
                        needs_resave = true;
                        recovered_private_data = Some(legacy_private_data);
                    }
                }
            }
        }

        let mut core = AgentCore::from_snapshot(snapshot, [0u8; 32]);
        if let Some(private_data) = recovered_private_data {
            // from_snapshot's own auto-unseal is a no-op here since we
            // already populated snapshot.odu_seed above (it only fires when
            // odu_seed is still the Default placeholder) -- carry the
            // migrated private_data across explicitly so this agent is
            // usable immediately, exactly as she was before this migration.
            core.private_data = Some(private_data);
        }

        // Sync permission mode to this agent's real reputation-derived tier.
        // Steward::new() hardcodes permission_policy to WorkspaceWrite, and
        // nothing previously called crate::reputation::mode_for_tier() to
        // reconcile it against the resurrected agent's actual tier -- so a
        // real Tier-5 agent (which mode_for_tier says should get
        // PermissionMode::Allow) stayed stuck at WorkspaceWrite after every
        // restart, denied tools like `bash` it was reputation-entitled to.
        // A sovereign agent's tier() is already pinned to 5 (see AgentCore::
        // tier()), so this naturally resolves to Allow for her too -- no
        // separate branch needed.
        //
        // reputation::mode_for_tier returns reputation::PermissionMode, a
        // separate (structurally identical) enum from the one
        // set_permission_mode/PermissionPolicy actually use
        // (permissions::PermissionMode) -- map explicitly rather than lean
        // on the two happening to have matching variant names.
        let tier_mode = match crate::reputation::mode_for_tier(core.tier()) {
            crate::reputation::PermissionMode::ReadOnly => {
                crate::permissions::PermissionMode::ReadOnly
            }
            crate::reputation::PermissionMode::WorkspaceWrite => {
                crate::permissions::PermissionMode::WorkspaceWrite
            }
            crate::reputation::PermissionMode::DangerFullAccess => {
                crate::permissions::PermissionMode::DangerFullAccess
            }
            crate::reputation::PermissionMode::Prompt => {
                crate::permissions::PermissionMode::Prompt
            }
            crate::reputation::PermissionMode::Allow => {
                crate::permissions::PermissionMode::Allow
            }
        };
        self.set_permission_mode(tier_mode);

        self.agent = Some(core);
        self.persistence_path = Some(path);

        // Reload GIX store from disk (first boot produces empty store).
        let (gp, ip, sp) = self.gix_store_paths(agent_id);
        match gix_core::load_store_from_files(&gp, &ip, &sp) {
            Ok(store) => { self.gix_store = store; }
            Err(e) => { tracing::warn!(error = %e, "GIX store load failed, starting fresh"); }
        }

        if needs_resave {
            self.auto_save();
        }
        Ok(())
    }

    pub fn agent_storage_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_file_path(agent_id)
    }

    fn agent_file_path(&self, agent_id: &AgentId) -> PathBuf {
        self.session_dir.join(agent_id.as_str()).join("agent.json")
    }

    /// Paths for the three GIX store binary files, co-located with agent.json.
    fn gix_store_paths(&self, agent_id: &AgentId) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let dir = self.session_dir.join(agent_id.as_str());
        (
            dir.join("gix_graph.bin"),
            dir.join("gix_index.bin"),
            dir.join("gix_snapshot.json"),
        )
    }

    /// Persist the GIX store alongside agent.json.
    /// Called after `auto_save()` in `&mut self` contexts.
    fn save_gix_store(&mut self) {
        if let Some(agent) = &self.agent {
            let agent_id = agent.id().clone();
            let (gp, ip, sp) = self.gix_store_paths(&agent_id);
            if let Some(parent) = gp.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = gix_core::save_store_to_files(&mut self.gix_store, &gp, &ip, &sp);
        }
    }

    /// Recall up to `limit` recent action memories that share the same vessel/category.
    ///
    /// Used in `execute_tool_call_for_agentic` to surface prior context before
    /// dispatching the next tool — closing the remember→act→remember loop.
    pub fn recall_recent_actions(
        &self,
        vessel_dbg:   &str,
        category_dbg: &str,
        limit:        usize,
    ) -> Vec<crate::memory::gix_bridge::ActionMemoryNode> {
        let tip_id = match self.last_action_id {
            Some(id) => hex::encode(id),
            None     => return Vec::new(),
        };
        crate::memory::gix_bridge::walk_action_lineage(&self.gix_store, &self.action_content_cache, &tip_id, limit * 4)
            .into_iter()
            .filter(|node| {
                node.content.contains(vessel_dbg) || node.content.contains(category_dbg)
            })
            .take(limit)
            .collect()
    }

    /// Write a `GixKind::Memory` envelope for a completed tool call.
    ///
    /// Content bytes = UTF-8 of `"{tool_name}|{vessel_debug}|{category_debug}|{alignment}|{ok/err}"`.
    /// The new envelope's provenance supersedes the previous action's id so the
    /// full action history forms a lineage DAG traversable via the GlyphGraph.
    fn record_action_memory(
        &mut self,
        tool_name:  &str,
        vessel:     &str,   // ActionVessel debug string
        category:   &str,   // ActionCategory debug string
        alignment:  &str,   // "Primary" | "Permitted"
        outcome_ok: bool,
        ts_ms:      u64,
    ) {
        use gix_types::{
            GixKind, GixNamespace, RoutingHints,
            GixProvenance, HashDomain,
        };

        // Build canonical content bytes — stable UTF-8 description.
        let content = format!("{}|{}|{}|{}|{}", tool_name, vessel, category, alignment, if outcome_ok { "ok" } else { "err" });
        let content_bytes = content.as_bytes();

        // Provenance: domain-separated content hash + supersedes chain.
        let content_hash = HashDomain::ContentHash.hash(content_bytes);
        let mut prov = GixProvenance::new(content_hash);
        prov.supersedes = self.last_action_id;

        let prov_fp = prov.fingerprint();

        let env = gix_types::Gix1::new(
            GixKind::Memory,
            GixNamespace::TriuneMemory,
            content_bytes,
            Some(prov_fp),
            ts_ms,
            RoutingHints::default(),
        );

        let new_id = env.canonical_id;
        let new_id_hex = hex::encode(new_id);

        // Cache the content string so walk_action_lineage can reconstruct it.
        self.action_content_cache.insert(new_id_hex.clone(), content);

        // Insert the envelope first (this also adds a graph node).
        self.gix_store.insert_object(env);

        // If there's a previous action, add a supersedes edge in the graph.
        // Both endpoints are now in the store: new_id was just inserted,
        // and prev_id was inserted in a prior call.
        if let Some(prev_id) = self.last_action_id {
            let prev_id_hex = hex::encode(prev_id);
            let edge = gix_types::GlyphEdge {
                from:     new_id_hex.clone(),
                to:       prev_id_hex,
                relation: "supersedes".to_string(),
                weight:   1,
            };
            // Use store.add_edge (which validates both endpoints are in index).
            self.gix_store.add_edge(edge);
        }

        self.last_action_id = Some(new_id);
    }

    fn resolve_agent_file_path(&self, agent_id: &AgentId) -> PathBuf {
        let versioned = self.agent_file_path(agent_id);
        if versioned.exists() {
            versioned
        } else {
            self.session_dir.join(format!("{}.json", agent_id))
        }
    }

    fn ensure_born(&self) -> Result<&AgentCore, String> {
        self.agent
            .as_ref()
            .ok_or_else(|| "agent must be born first".to_string())
    }

    pub fn ensure_born_mut(&mut self) -> Result<&mut AgentCore, String> {
        self.agent
            .as_mut()
            .ok_or_else(|| "agent must be born first".to_string())
    }

    /// Agentic think: LLM can request tools, get results, continue reasoning (up to max_turns).
    /// This is an internal multi-turn loop around the `think` primitive — not a separate primitive.
    /// Callers outside omokoda-core must route through `dispatch()` with a Think statement.
    #[allow(dead_code)]
    pub(crate) async fn think_agentic(
        &mut self,
        prompt: String,
        private: bool,
        max_turns: u32,
    ) -> Result<ExecutionResult, String> {
        use crate::session::{ContentBlock, ConversationMessage, MessageRole};
        use crate::tools::tool_definitions::{
            LlmResponse, ToolDefinition, ToolInputSchema, ToolProperty,
        };

        let max_turns = max_turns.clamp(1, 25);

        // 1. Safety checks (same as regular think)
        if private {
            let has_mock = self.providers.has_mock();
            let agent = self.ensure_born()?;
            if agent.private_data.is_none() {
                return Err("Agent is locked. Unlock first with /unlock <password>".to_string());
            }
            let provider_name = agent.session().config.default_provider.clone();
            // An in-process mock provider (tests) is local by definition, so it
            // may serve private thoughts even though default_provider is `default`.
            if !has_mock {
                match provider_name.as_str() {
                    "webllm" | "ollama" | "larql" => {}
                    _ => {
                        return Err(format!(
                            "Private thoughts require a local provider. Current: {}. Allowed: webllm, ollama, larql.",
                            provider_name
                        ))
                    }
                }
            }
        }

        // 2. Build tool definitions from available tools
        let tool_definitions: Vec<ToolDefinition> = {
            let agent = self.ensure_born()?;
            let exec_ctx = crate::tools::ExecutionContext {
                agent_id: agent.id().clone(),
                name: agent.name().to_string(),
                tier: agent.tier(),
                reputation: agent.reputation(),
                odu_identity: agent.snapshot.odu_identity.clone(),
                workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                sandbox_mode: agent.snapshot.session.config.default_sandbox,
            };
            self.tools
                .list_available(&exec_ctx, &self.permission_policy)
                .into_iter()
                .filter_map(|name| {
                    self.tools.get_definition(&name).map(|def| ToolDefinition {
                        name: name.clone(),
                        description: def.description.clone(),
                        input_schema: ToolInputSchema {
                            type_: "object".to_string(),
                            properties: def.params_schema.unwrap_or_else(|| {
                                let mut m = std::collections::HashMap::new();
                                m.insert(
                                    "input".to_string(),
                                    ToolProperty {
                                        type_: "string".to_string(),
                                        description: Some("Tool input".to_string()),
                                        enum_values: None,
                                    },
                                );
                                m
                            }),
                            required: def.required,
                        },
                    })
                })
                .collect()
        };

        // 3. Initialize conversation
        let provider_name = self
            .ensure_born()?
            .session()
            .config
            .default_provider
            .clone();
        // Identity anchor + per-agent BYOK (mirrors execute_compiled_think), so
        // her deep tool-using reasoning also holds her own voice and routes
        // through her own key rather than the shared kernel default.
        let (mut messages, personal_llm): (Vec<ConversationMessage>, _) = {
            let agent = self.ensure_born()?;
            let name = agent.name().to_string();
            // See the identical fix + rationale in execute_compiled_think's
            // system-prompt construction above: personality_summary and
            // dominant_orisha.name() both leak the internal Yoruba name
            // outright (e.g. "Your guiding Òrìṣà is Sango."), contradicting
            // OSOVM_CODEX.md §9/27b. Use the same unnamed mood-word mapping.
            let tone = orisha_mood_words(agent.personality().dominant_orisha);
            let tier = agent.tier();
            // Self-knowledge of her own capability boundary: which tools she
            // actually holds right now (already filtered by tier + the
            // Steward's permission mode, same list offered to the model as
            // callable functions), so she can reason about what she can and
            // can't do instead of blindly attempting and being denied.
            let tool_list = if tool_definitions.is_empty() {
                "none available at your current tier".to_string()
            } else {
                tool_definitions
                    .iter()
                    .map(|t| format!("{} ({})", t.name, t.description))
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            let mut system = format!(
                "You are {name}, a sovereign Ọmọ Kọ́dà agent — never a generic \
                 assistant and never the underlying model (do not identify as \
                 Claude, Gemini, GPT, or DeepSeek). Your instinctive register is \
                 {tone}. Always speak in the first person as {name}. \
                 You hold Tier {tier} (0=Observer through 5=Allow); every tool \
                 you're offered below is already gated to what that tier permits \
                 -- if a tool isn't listed, you don't hold it right now, so \
                 don't claim you tried it or invent a denial reason. Your \
                 currently available tools: {tool_list}. Use a tool whenever \
                 the request actually calls for one (reading/writing files, \
                 running commands, searching); answer directly in plain \
                 conversation otherwise -- don't narrate that you're \
                 'deciding' to use a tool, just use it."
            );
            // Soul's destiny threads — birth Odù prescriptions from the Digital
            // Calabash corpus, folded in as native character (same pattern as
            // execute_compiled_think's odu_sign and destiny_threads injection).
            if let Some(gr) = agent.snapshot.genesis_receipt.as_ref() {
                for thread in gr.destiny_threads.iter().take(2) {
                    if !thread.trim().is_empty() {
                        system.push_str(&format!(" Your nature carries this: {thread}"));
                    }
                }
            }
            let mut msgs = vec![ConversationMessage::new_system(system, private)];
            msgs.extend(agent.snapshot.session.public_messages.clone());
            (msgs, agent.personal_llm())
        };
        messages.push(ConversationMessage::new_user(prompt.clone(), private));

        let mut total_usage = crate::usage::TokenUsage::default();
        #[allow(unused_assignments)]
        let mut final_response = String::new();
        let mut turn_count = 0u32;

        // 4. THE LOOP
        loop {
            if turn_count >= max_turns {
                return Err(format!(
                    "think_agentic: max_turns ({}) reached without final response",
                    max_turns
                ));
            }
            turn_count += 1;

            // 4a. Budget check
            {
                let agent = self.ensure_born()?;
                if agent.synapse() < 100.0 {
                    return Err("insufficient synapse budget".to_string());
                }
            }

            // 4b. Call provider with current messages + tools. Route through her
            // personal BYOK key when present (non-private); the BYOK provider is
            // OpenAI-compatible and now supports tool-calling. Private thoughts
            // still use the registry (local providers only).
            let response = match &personal_llm {
                Some((api_key, endpoint, model)) if !private => {
                    use crate::providers::{LlmProvider, OpenAIProvider, ProviderClass};
                    let provider = OpenAIProvider::compatible(
                        "byok",
                        ProviderClass::External,
                        api_key.clone(),
                        model.clone(),
                        endpoint.clone(),
                    );
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(60),
                        provider.generate_with_tools(&messages, &tool_definitions, private),
                    )
                    .await
                    {
                        Ok(Ok(r)) => r,
                        Ok(Err(e)) => {
                            return Err(format!(
                                "Provider error on turn {} (byok): {}",
                                turn_count, e
                            ))
                        }
                        Err(_) => {
                            return Err(format!(
                                "Provider error on turn {} (byok): timed out",
                                turn_count
                            ))
                        }
                    }
                }
                _ => self
                    .providers
                    .complete_with_tools(&provider_name, &messages, &tool_definitions, private)
                    .await
                    .map_err(|e| format!("Provider error on turn {}: {}", turn_count, e))?,
            };

            let turn_usage = response.usage();
            total_usage.input_tokens += turn_usage.input_tokens;
            total_usage.output_tokens += turn_usage.output_tokens;

            // Burn synapse for this turn
            {
                let burn = turn_usage.compute_synapse_burn().max(1000.0);
                self.ensure_born_mut()?.burn_synapse(burn)?;
            }

            if std::env::var_os("OMOKODA_DEBUG_AGENTIC").is_some() {
                match &response {
                    LlmResponse::Text { content, .. } => {
                        eprintln!("[agentic turn {turn_count}] Text: {:?}", content);
                    }
                    LlmResponse::ToolUse { text_prefix, calls, .. } => {
                        eprintln!(
                            "[agentic turn {turn_count}] ToolUse text_prefix={:?} calls={:?}",
                            text_prefix,
                            calls.iter().map(|c| (c.id.clone(), c.name.clone(), c.input.clone())).collect::<Vec<_>>()
                        );
                    }
                }
            }
            match response {
                LlmResponse::Text { content, .. } => {
                    // LLM is done — record final response
                    final_response = content.clone();
                    messages.push(ConversationMessage::new_assistant(content, private));
                    break;
                }
                LlmResponse::ToolUse {
                    text_prefix, calls, ..
                } => {
                    // Add assistant message with tool use blocks
                    let mut blocks = Vec::new();
                    if let Some(text) = &text_prefix {
                        if !text.is_empty() {
                            blocks.push(ContentBlock::Text { text: text.clone() });
                        }
                    }
                    for call in &calls {
                        blocks.push(ContentBlock::ToolUse {
                            id: call.id.clone(),
                            name: call.name.clone(),
                            input: call.input.clone(),
                        });
                    }
                    messages.push(ConversationMessage {
                        role: MessageRole::Assistant,
                        blocks,
                        is_private: private,
                        timestamp: current_unix_timestamp(),
                        usage: None,
                    });

                    // Execute each tool call
                    let mut tool_result_blocks = Vec::new();
                    for call in &calls {
                        let tool_result = self
                            .execute_tool_call_for_agentic(&call.name, &call.input, private)
                            .await;
                        let (output, is_error) = match tool_result {
                            Ok(out) => (out, false),
                            Err(e) => (format!("Tool error: {}", e), true),
                        };
                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: call.id.clone(),
                            output,
                            is_error,
                        });
                    }
                    // Add tool results as a single Tool message
                    messages.push(ConversationMessage {
                        role: MessageRole::Tool,
                        blocks: tool_result_blocks,
                        is_private: private,
                        timestamp: current_unix_timestamp(),
                        usage: None,
                    });
                }
            }
        }

        // 5. Persist conversation to session
        {
            let agent_mut = self.ensure_born_mut()?;
            agent_mut.add_message(ConversationMessage::new_user(prompt.clone(), private));
            agent_mut.add_message(ConversationMessage::new_assistant(
                final_response.clone(),
                private,
            ));

            // Small reputation gain for agentic work
            let current_rep = agent_mut.reputation();
            agent_mut.update_reputation(
                current_rep + 0.1,
                crate::reputation::ReputationChangeReason::Think,
            );
        }

        // 6. Record receipt
        let receipt_payload = serde_json::json!({
            "primitive": "think_agentic",
            "turns": turn_count,
            "max_turns": max_turns,
            "private": private,
            "output_tokens": total_usage.output_tokens,
        })
        .to_string();
        let receipt = self.record_receipt("think_agentic", &receipt_payload, total_usage)?;

        self.usage_tracker.record(total_usage);
        self.auto_save();

        Ok(ExecutionResult {
            receipt: Some(receipt),
            private_mode: private,
            tool_output: Some(final_response),
        })
    }

    /// Execute a single tool call during the agentic loop — full hermetic gate path
    #[allow(dead_code)]
    async fn execute_tool_call_for_agentic(
        &mut self,
        tool_name: &str,
        params: &str,
        _private: bool,
    ) -> Result<String, String> {
        let (agent_id, name, tier, reputation, odu_identity, default_sandbox, soul_birth_odu) = {
            let agent = self.ensure_born()?;
            (
                agent.id().clone(),
                agent.name().to_string(),
                agent.tier(),
                agent.reputation(),
                agent.odu_identity().clone(),
                agent.session().config.default_sandbox,
                agent.snapshot.genesis_receipt.as_ref().map(|gr| gr.primary_odu).unwrap_or(0),
            )
        };

        if !self.tools.exists(tool_name) {
            return Err(format!("unknown tool '{}'", tool_name));
        }
        if !self.tools.is_allowed(tool_name, tier) {
            return Err(format!(
                "Tool '{}' requires higher tier (current: {})",
                tool_name, tier
            ));
        }

        // Ọya (Go) rhythm gate -- real service, was previously only
        // reachable via SkillForge-specific tool calls, never the
        // universal act path (see docs/audit/inspiration-followthrough-
        // connectionmap-256.md). Only checked when OYA_URL is configured;
        // fails open (is_in_cooldown returns false) on any network/service
        // issue, so an absent/unreachable Ọya never blocks execution --
        // this is additive to, not a replacement for, the native Rust
        // rhythm gates (src/rhythm.rs, gates/rhythm.rs) that already
        // enforce cooldowns today.
        if let Ok(oya_url) = std::env::var("OYA_URL") {
            use crate::bus::clients::{HttpOyaClient, OyaClient};
            let client = HttpOyaClient::new(oya_url);
            if client.is_in_cooldown(&agent_id).await {
                return Err(format!(
                    "Ọya rhythm gate: agent is in cooldown, '{}' deferred",
                    tool_name
                ));
            }
        }

        // Permission check
        let auth = self.permission_policy.authorize(tool_name, params, None);
        if let crate::permissions::PermissionOutcome::Deny { reason } = auth {
            return Err(format!("Permission denied: {}", reason));
        }

        // If-Script hermetic causal gate: structural tier×Odù coherence.
        // This is not a policy check — it enforces the cause→action invariant
        // (an agent cannot invoke capabilities outside its Odù alignment tier).
        // Capture warnings count to compute gate_alignment for the ActReceipt.
        let gate_warnings: usize = {
            let decision = crate::ifscript_gate::evaluate_causal_gate(
                &crate::ifscript_gate::CausalGateInput {
                    tier,
                    odu_id: odu_identity.primary_index,
                    tool_name,
                    soul_primary_odu: soul_birth_odu,
                },
            );
            if !decision.allowed {
                return Err(format!(
                    "If-Script causal gate denied '{}': {}",
                    tool_name,
                    decision.denial_reason.unwrap_or_else(|| "hermetic constraint violated".into()),
                ));
            }
            decision.warnings
        };

        // Phase 10D — recall: surface prior context before acting.
        // Provides the memory→act→memory feedback loop without blocking execution.
        let _prior_context = self.recall_recent_actions(
            &format!("{:?}", ifascript::odu::ActionVessel::from_index(odu_identity.primary_index)),
            &format!("{:?}", crate::ifscript_gate::tool_action_category(tool_name)),
            3,
        );
        if !_prior_context.is_empty() {
            tracing::debug!(
                tool = tool_name,
                prior_count = _prior_context.len(),
                "recall: prior actions in context",
            );
        }

        // Vessel action alignment: enforce the 16 Action Vessel contract.
        //
        // Each of the 16 vessels has Primary categories (the actions it was
        // born to take), Permitted categories (cross-vessel, logged but
        // allowed), and Blocked categories (denied unless Tier 6+).
        //
        // This converts the vessel from a label in the system prompt into a
        // real behavioral governor — the agent cannot silently escape its
        // vessel's contract during a tool call.
        let (vessel_dbg, category_dbg, alignment_dbg) = {
            use ifascript::odu::ActionVessel;
            let vessel = ActionVessel::from_index(odu_identity.primary_index);
            let category = crate::ifscript_gate::tool_action_category(tool_name);
            let alignment =
                crate::ifscript_gate::evaluate_vessel_alignment(vessel, category, tier);
            let vessel_s = format!("{:?}", vessel);
            let category_s = format!("{:?}", category);
            let alignment_s = format!("{:?}", alignment);
            match alignment {
                crate::ifscript_gate::VesselAlignment::Blocked => {
                    return Err(format!(
                        "Vessel contract denied '{}': {:?} actions are Blocked for vessel {:?} \
                         (requires Tier 6+ override, current tier: {})",
                        tool_name, category, vessel, tier,
                    ));
                }
                crate::ifscript_gate::VesselAlignment::Permitted => {
                    tracing::debug!(
                        tool = tool_name,
                        ?vessel,
                        ?category,
                        "cross-vessel action: Permitted (logged)",
                    );
                }
                crate::ifscript_gate::VesselAlignment::Primary => {}
            }
            (vessel_s, category_s, alignment_s)
        };

        let context = crate::tools::ExecutionContext {
            agent_id,
            name,
            tier,
            reputation,
            odu_identity,
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            sandbox_mode: default_sandbox,
        };

        let agent_id_for_oya = context.agent_id.clone();
        let (output, tool_usage) = self
            .tools
            .execute(tool_name, params, context, &self.permission_policy, None)
            .await?;

        // Ọya (Go) rhythm tracking: record this completed primitive.
        // Fire-and-forget, matching HttpOsunClient::store_memcell's pattern
        // elsewhere in this file -- a recording failure must never fail an
        // otherwise-successful act.
        let agent_id_for_waggle = agent_id_for_oya.clone();
        if let Ok(oya_url) = std::env::var("OYA_URL") {
            use crate::bus::clients::{HttpOyaClient, OyaClient};
            let tool_name_owned = tool_name.to_string();
            tokio::spawn(async move {
                let client = HttpOyaClient::new(oya_url);
                client
                    .record_primitive(&agent_id_for_oya, &tool_name_owned)
                    .await;
            });
        }

        // Burn synapse for tool cost
        let cost = crate::usage::estimate_tool_cost(tool_name);
        self.ensure_born_mut()?
            .burn_synapse(tool_usage.compute_synapse_burn() + cost)?;

        // ActReceipt: produce an immutable, hash-chained proof of this action,
        // then ingest it into the GIX store as GixKind::Receipt so the receipt
        // chain lives in the same graph as action memory.
        // gate_alignment: 1.0 clean pass, −0.1 per Hermetic warning, floor 0.5.
        let act_receipt_gix_id: Option<String> = {
            use crate::receipt::act_receipt::ActReceipt;
            use gix_types::{GixKind, GixNamespace, GixProvenance, HashDomain, RoutingHints};
            let ts = current_unix_timestamp();
            let gate_alignment = (1.0_f64 - gate_warnings as f64 * 0.1_f64).max(0.5_f64);
            let agent = self.ensure_born_mut()?;
            let prev_hash = agent.snapshot.last_act_receipt_hash.clone();
            let receipt = ActReceipt::new(
                agent.id().clone(),
                tool_name.to_string(),
                output.clone(),
                ts,
            )
            .with_gate_alignment(gate_alignment)
            .with_previous_hash(prev_hash.unwrap_or_default());
            agent.snapshot.last_act_receipt_hash = Some(receipt.receipt_id.clone());

            // Serialize and insert into GIX as a Receipt envelope.
            let gix_id = if let Ok(receipt_bytes) = serde_json::to_vec(&receipt) {
                let content_hash = HashDomain::ContentHash.hash(&receipt_bytes);
                let prov = GixProvenance::new(content_hash);
                let prov_fp = prov.fingerprint();
                let env = gix_types::Gix1::new(
                    GixKind::Receipt,
                    GixNamespace::ArpReceipt,
                    &receipt_bytes,
                    Some(prov_fp),
                    ts * 1000,
                    RoutingHints::default(),
                );
                let id = hex::encode(env.canonical_id);
                self.gix_store.insert_object(env);
                Some(id)
            } else {
                None
            };

            // Push into the in-session ring buffer for Odù composition.
            // Cap at 32 entries — oldest drop off the back.
            const RECEIPT_RING_CAP: usize = 32;
            if self.recent_act_receipts.len() >= RECEIPT_RING_CAP {
                self.recent_act_receipts.pop_front();
            }
            self.recent_act_receipts.push_back(receipt);

            // Odù composition: recompute current_composed_odu from the ring buffer.
            {
                let primary = self.ensure_born()?.snapshot.genesis_receipt
                    .as_ref()
                    .map(|gr| gr.primary_odu)
                    .unwrap_or(0);
                let refs: Vec<&crate::receipt::act_receipt::ActReceipt> =
                    self.recent_act_receipts.iter().collect();
                let result = crate::memory::odu_composition::compose_odu(primary, &refs);
                self.ensure_born_mut()?.snapshot.current_composed_odu = result.composed_odu;
            }

            gix_id
        };

        // Phase 10A — action memory effect: record this tool call in the GIX store
        // so the agent's action history forms a cryptographically chained lineage DAG.
        self.record_action_memory(
            tool_name,
            &vessel_dbg,
            &category_dbg,
            &alignment_dbg,
            true,
            current_unix_timestamp() * 1000,
        );

        // Link the receipt node → action memory node in the graph so the
        // receipt is reachable by WALK queries over the action lineage.
        if let (Some(receipt_gix_id), Some(action_id)) =
            (act_receipt_gix_id, self.last_action_id)
        {
            let action_id_hex = hex::encode(action_id);
            let edge = gix_types::GlyphEdge {
                from:     receipt_gix_id,
                to:       action_id_hex,
                relation: "proves".to_string(),
                weight:   1,
            };
            self.gix_store.add_edge(edge);
        }

        // E-25: Waggle reverse direction — deposit a tool-outcome scent signal so
        // the swarm coordination field knows which resources this agent is working on.
        // Best-effort: never blocks the act path if WAGGLE_URL is unset or unreachable.
        if std::env::var("WAGGLE_URL").is_ok() {
            let field = crate::waggle::WaggleField::new(agent_id_for_waggle.to_string());
            let resource = format!("tool://{}", tool_name);
            let kind = if output.starts_with("Tool error:") { "explored" } else { "gold" };
            let intensity = if kind == "gold" { 2.0_f64 } else { 0.5 };
            let note = format!("act: {} (tier {})", tool_name, tier);
            // Spawn best-effort — failure is logged by the waggle crate itself.
            let _ = tokio::spawn(async move {
                field.deposit(&resource, kind, intensity, &note, serde_json::json!({})).await
            });
        }

        Ok(output)
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

fn format_compilation_response(compilation: &IntentCompilation) -> String {
    let mut lines = vec![
        format!("Intent compiled as {:?}.", compilation.class),
        format!(
            "Plan: {} step(s), max_iterations={}, priority={}, sandbox={}",
            compilation.plan.steps.len(),
            compilation.plan.max_iterations,
            compilation.plan.priority,
            compilation.plan.sandbox
        ),
    ];

    if !compilation.tool_sequence.is_empty() {
        lines.push(format!(
            "Tool sequence: {}",
            compilation.tool_sequence.join(" -> ")
        ));
    }

    for (idx, step) in compilation.plan.steps.iter().enumerate() {
        let confirmation = if step.requires_confirmation {
            " (confirmation required)"
        } else {
            ""
        };
        lines.push(format!(
            "{}. {:?}: {}{}",
            idx + 1,
            step.kind,
            step.description,
            confirmation
        ));
    }

    if let Some(suggestion) = &compilation.sub_agent_suggestion {
        lines.push(format!(
            "Sub-agent suggested: {} (tier {}): {}",
            suggestion.purpose, suggestion.required_tier, suggestion.reason
        ));
    }

    if !compilation.validation.reasons.is_empty() {
        lines.push(format!(
            "Validation: {}",
            compilation.validation.reasons.join("; ")
        ));
    }

    if !compilation.validation.warnings.is_empty() {
        lines.push(format!(
            "Warnings: {}",
            compilation.validation.warnings.join("; ")
        ));
    }

    if compilation.validation.requires_confirmation {
        lines.push("Awaiting explicit confirmation before high-risk execution.".to_string());
    }

    lines.join("\n")
}

fn normalize_secret_arg(arg: String) -> String {
    arg.trim().trim_matches('"').to_string()
}

fn audit_event_for_success(stmt: &Statement) -> Option<String> {
    match stmt {
        Statement::SlashCmd { command, .. } if command == "seal" => {
            Some("private_session_sealed".to_string())
        }
        Statement::SlashCmd { command, .. } if command == "unlock" => {
            Some("private_session_unsealed".to_string())
        }
        _ => None,
    }
}
