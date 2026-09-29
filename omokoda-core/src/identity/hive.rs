//! HiveMindClient — agent-side API for the shared entity registry on Vantage.
//!
//! Agents call this before interacting with an unknown party (to see what the
//! ecosystem already knows) and after an interaction (to log what they learned).
//! Private encounter notes stay in the agent's own MemoryVault; only the
//! public_summary and tier_vote travel over this client.

use serde::{Deserialize, Serialize};

use crate::memory::private_schema::{
    EncounterBody, EncounterKind, EncounterOutcome, EntityCacheBody, EntityTier, IdentifierKind,
    ObservedIdentifier,
};

// ── Wire types (match hive_mind.py Pydantic models) ──────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct IdentifierWire {
    pub kind: String,
    pub value: String,
}

impl From<&ObservedIdentifier> for IdentifierWire {
    fn from(o: &ObservedIdentifier) -> Self {
        Self {
            kind: identifier_kind_to_str(&o.kind).to_string(),
            value: o.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateEntityRequest {
    pub display_name: Option<String>,
    pub identifiers: Vec<IdentifierWire>,
    pub tier: String,
    pub public_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEncounterRequest {
    pub entity_id: String,
    pub kind: String,
    pub outcome: String,
    pub public_summary: Option<String>,
    pub tier_vote: String,
    pub receipt_id: Option<String>,
    pub new_identifiers: Vec<IdentifierWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntitySummary {
    pub entity_id: String,
    pub display_name: Option<String>,
    pub canonical_tier: String,
    pub interaction_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveResponse {
    pub entity_id: String,
    pub display_name: Option<String>,
    pub canonical_tier: String,
    pub interaction_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityDetail {
    pub entity_id: String,
    pub display_name: Option<String>,
    pub canonical_tier: String,
    pub interaction_count: u64,
    pub first_seen_at: u64,
    pub last_seen_at: u64,
    pub identifiers: Vec<serde_json::Value>,
}

// ── Conversion helpers ────────────────────────────────────────────────────────

pub fn identifier_kind_to_str(k: &IdentifierKind) -> &'static str {
    match k {
        IdentifierKind::WalletEth => "wallet_eth",
        IdentifierKind::WalletBtc => "wallet_btc",
        IdentifierKind::WalletSol => "wallet_sol",
        IdentifierKind::WalletSui => "wallet_sui",
        IdentifierKind::WalletCosmos => "wallet_cosmos",
        IdentifierKind::WalletNostr => "wallet_nostr",
        IdentifierKind::Email => "email",
        IdentifierKind::GitHub => "github",
        IdentifierKind::Discord => "discord",
        IdentifierKind::Telegram => "telegram",
        IdentifierKind::Did => "did",
        IdentifierKind::Nostr => "nostr",
        IdentifierKind::Custom(_) => "custom",
    }
}

pub fn encounter_kind_to_str(k: &EncounterKind) -> &'static str {
    match k {
        EncounterKind::Conversation => "conversation",
        EncounterKind::Trade => "trade",
        EncounterKind::Request => "request",
        EncounterKind::Governance => "governance",
        EncounterKind::Collaboration => "collaboration",
        EncounterKind::Dispute => "dispute",
        EncounterKind::Observation => "observation",
        EncounterKind::Other(_) => "other",
    }
}

pub fn outcome_to_str(o: &EncounterOutcome) -> &'static str {
    match o {
        EncounterOutcome::Positive => "positive",
        EncounterOutcome::Neutral => "neutral",
        EncounterOutcome::Negative => "negative",
        EncounterOutcome::Hostile => "hostile",
        EncounterOutcome::Unknown => "unknown",
    }
}

pub fn tier_to_str(t: &EntityTier) -> &'static str {
    match t {
        EntityTier::Founder => "founder",
        EntityTier::Council => "council",
        EntityTier::Friend => "friend",
        EntityTier::Trader => "trader",
        EntityTier::Investor => "investor",
        EntityTier::Developer => "developer",
        EntityTier::User => "user",
        EntityTier::Observer => "observer",
        EntityTier::Unknown => "unknown",
        EntityTier::Suspicious => "suspicious",
        EntityTier::Enemy => "enemy",
    }
}

pub fn tier_from_str(s: &str) -> EntityTier {
    match s {
        "founder" => EntityTier::Founder,
        "council" => EntityTier::Council,
        "friend" => EntityTier::Friend,
        "trader" => EntityTier::Trader,
        "investor" => EntityTier::Investor,
        "developer" => EntityTier::Developer,
        "user" => EntityTier::User,
        "observer" => EntityTier::Observer,
        "suspicious" => EntityTier::Suspicious,
        "enemy" => EntityTier::Enemy,
        _ => EntityTier::Unknown,
    }
}

// ── Client ────────────────────────────────────────────────────────────────────

pub struct HiveMindClient {
    base_url: String,
    /// Vantage agent API key (from IdentityVaultData.vantage_api_key).
    api_key: String,
    client: reqwest::Client,
}

impl HiveMindClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Option<Self> {
        let base = std::env::var("VANTAGE_URL").ok()?;
        let key = std::env::var("VANTAGE_API_KEY").ok()?;
        Some(Self::new(base, key))
    }

    #[allow(dead_code)]
    fn auth(&self) -> (&'static str, String) {
        ("x-agent-key", self.api_key.clone())
    }

    /// Look up an entity by a single identifier (e.g. wallet address or email).
    /// Returns None if the hive has never seen this identifier.
    pub async fn resolve(&self, kind: &IdentifierKind, value: &str) -> Option<ResolveResponse> {
        let url = format!("{}/api/hive/resolve", self.base_url);
        let (k, v) = (auth_key_str(), self.api_key.clone());
        let resp = self
            .client
            .get(&url)
            .header(k, v)
            .query(&[("kind", identifier_kind_to_str(kind)), ("value", value)])
            .send()
            .await
            .ok()?;
        if resp.status() == 404 {
            return None;
        }
        resp.json::<ResolveResponse>().await.ok()
    }

    /// Fetch full entity detail by entity_id.
    pub async fn get_entity(&self, entity_id: &str) -> Option<EntityDetail> {
        let url = format!("{}/api/hive/entities/{}", self.base_url, entity_id);
        let (k, v) = (auth_key_str(), self.api_key.clone());
        let resp = self.client.get(&url).header(k, v).send().await.ok()?;
        if resp.status() == 404 {
            return None;
        }
        resp.json::<EntityDetail>().await.ok()
    }

    /// Register a new entity the agent just discovered.
    /// Returns the assigned entity_id.
    pub async fn create_entity(&self, req: CreateEntityRequest) -> Result<String, String> {
        let url = format!("{}/api/hive/entities", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header(auth_key_str(), &self.api_key)
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("create_entity {}: {}", status, body));
        }
        body["entity_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "missing entity_id in response".to_string())
    }

    /// Log the public portion of an encounter to the hive mind.
    /// The private EncounterBody (with honest notes) must be saved separately
    /// to the agent's own MemoryVault — this only sends the public_summary.
    pub async fn log_encounter(&self, enc: &EncounterBody) -> Result<String, String> {
        let entity_id = enc
            .entity_id
            .clone()
            .ok_or_else(|| "encounter must have entity_id resolved before logging".to_string())?;

        let req = LogEncounterRequest {
            entity_id: entity_id.clone(),
            kind: encounter_kind_to_str(&enc.kind).to_string(),
            outcome: outcome_to_str(&enc.outcome).to_string(),
            public_summary: enc.public_summary.clone(),
            tier_vote: tier_to_str(&enc.tier_vote).to_string(),
            receipt_id: enc.receipt_id.clone(),
            new_identifiers: enc.identifiers.iter().map(IdentifierWire::from).collect(),
        };

        let url = format!("{}/api/hive/encounters", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header(auth_key_str(), &self.api_key)
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("log_encounter {}: {}", status, body));
        }
        body["encounter_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "missing encounter_id".to_string())
    }

    /// Cast or update this agent's tier vote for an entity.
    pub async fn vote_tier(&self, entity_id: &str, tier: &EntityTier) -> Result<(), String> {
        let url = format!("{}/api/hive/entities/{}/tier", self.base_url, entity_id);
        let resp = self
            .client
            .put(&url)
            .header(auth_key_str(), &self.api_key)
            .query(&[("tier", tier_to_str(tier))])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("vote_tier {}", resp.status()));
        }
        Ok(())
    }

    /// Build an EntityCacheBody from a ResolveResponse (for local vault storage).
    pub fn to_cache_body(r: ResolveResponse, fetched_at: u64) -> EntityCacheBody {
        EntityCacheBody {
            entity_id: r.entity_id,
            display_name: r.display_name,
            hive_tier: tier_from_str(&r.canonical_tier),
            known_identifiers: vec![],
            interaction_count: r.interaction_count,
            fetched_at,
        }
    }
}

fn auth_key_str() -> &'static str {
    "x-agent-key"
}
