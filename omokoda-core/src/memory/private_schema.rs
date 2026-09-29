use serde::{Deserialize, Serialize};

/// Discriminated-union body for each Tier-2 memory category.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum PrivateMemoryBody {
    Identity(IdentityBody),
    Birth(BirthBody),
    Thought(ThoughtBody),
    Relation(RelationBody),
    Preference(PreferenceBody),
    Decision(DecisionBody),
    Capability(CapabilityBody),
    Receipt(ReceiptBody),
    Note(NoteBody),
    /// Private record of a specific encounter with another entity.
    /// Stored only in this agent's MemoryVault — never transmitted to the hive.
    Encounter(EncounterBody),
    /// Cached hive-mind lookup result for a known entity.
    /// Updated each time the agent queries `/api/hive/entities`.
    EntityCache(EntityCacheBody),
}

/// A single private memory entry stored in MemoryVault.entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateMemoryEntry {
    /// UUID v4 — stable across edits.
    pub id: String,
    /// Unix seconds.
    pub created_ts: u64,
    /// Last update timestamp (None = never updated).
    pub updated_ts: Option<u64>,
    /// Structured body.
    pub body: PrivateMemoryBody,
    /// Optional free-form tags for retrieval.
    #[serde(default)]
    pub tags: Vec<String>,
}

// ── Body types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityBody {
    pub agent_id: String,
    pub name: String,
    pub odu_index: u8,
    pub odu_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BirthBody {
    pub genesis_receipt_id: String,
    pub birth_ts: u64,
    pub ip_root_kind: u16,
    pub nostr_pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtBody {
    pub content: String,
    /// Short-form summary for fast retrieval.
    pub summary: Option<String>,
    /// Confidence 0.0–1.0.
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationBody {
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub relation_kind: String,
    pub trust_score: f32,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceBody {
    pub key: String,
    pub value: serde_json::Value,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionBody {
    pub description: String,
    pub rationale: String,
    pub outcome: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityBody {
    pub skill: String,
    pub level: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptBody {
    /// ARP receipt ID or Vantage receipt reference.
    pub receipt_id: String,
    pub action: String,
    pub amount_cents: Option<u64>,
    pub currency: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteBody {
    pub content: String,
}

// ── Encounter & hive-mind types ────────────────────────────────────────────────

/// How the agent classifies a specific interaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncounterKind {
    Conversation,
    Trade,
    Request,
    Governance,
    Collaboration,
    Dispute,
    Observation,
    Other(String),
}

/// Outcome the agent privately assigns to an encounter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EncounterOutcome {
    Positive,
    Neutral,
    Negative,
    Hostile,
    Unknown,
}

/// Tier the agent assigns to an entity in the ecosystem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntityTier {
    Founder,
    Council,
    Friend,
    Trader,
    Investor,
    Developer,
    User,
    Observer,
    #[default]
    Unknown,
    Suspicious,
    Enemy,
}

/// What kind of identifier this is.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentifierKind {
    WalletEth,
    WalletBtc,
    WalletSol,
    WalletSui,
    WalletCosmos,
    WalletNostr,
    Email,
    GitHub,
    Discord,
    Telegram,
    Did,
    Nostr,
    Custom(String),
}

/// A single identifier observed for an entity (wallet address, email, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedIdentifier {
    pub kind: IdentifierKind,
    pub value: String,
}

/// Private encounter record — full honesty, never leaves this vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterBody {
    /// Hive entity_id if the agent successfully resolved this entity.
    pub entity_id: Option<String>,
    /// Identifiers the agent observed during this encounter.
    pub identifiers: Vec<ObservedIdentifier>,
    /// How this interaction happened.
    pub kind: EncounterKind,
    /// Outcome as the agent privately judges it.
    pub outcome: EncounterOutcome,
    /// Agent's private (honest, internal) note — never shared.
    pub private_note: Option<String>,
    /// Short summary the agent is willing to share with the hive.
    /// None = keep fully private. Some = will be submitted to hive mind.
    pub public_summary: Option<String>,
    /// Tier the agent assigns to this entity based on this interaction.
    pub tier_vote: EntityTier,
    /// ARP receipt or Vantage tx id proving the interaction happened.
    pub receipt_id: Option<String>,
}

/// Snapshot of a hive entity record cached locally after a lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCacheBody {
    pub entity_id: String,
    pub display_name: Option<String>,
    /// Canonical tier as voted by the hive (may differ from agent's own vote).
    pub hive_tier: EntityTier,
    /// All known identifiers for this entity from the hive.
    pub known_identifiers: Vec<ObservedIdentifier>,
    /// Total interactions the ecosystem has logged for this entity.
    pub interaction_count: u64,
    /// Unix timestamp when this cache entry was fetched.
    pub fetched_at: u64,
}
