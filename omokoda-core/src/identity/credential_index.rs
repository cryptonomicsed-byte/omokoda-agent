use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What kind of credential is stored.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    NostrNpub,
    SuiAddress,
    EthAddress,
    BtcXpub,
    DidDocument,
    Vantage,
    MinipaeNpub,
    GitSignKey,
    Custom(String),
}

/// A single credential entry — the index only stores public-facing identifiers,
/// never secrets (those stay in IdentityVault / Tier0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    pub kind: CredentialKind,
    pub identifier: String,
    pub label: Option<String>,
    pub verified: bool,
    pub added_at: u64,
}

/// Lookup-only index of all credential kinds for one agent.
/// Purpose: given agent_id, find the correct identifier for any protocol.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CredentialIndex {
    entries: HashMap<CredentialKind, Vec<CredentialEntry>>,
}

impl CredentialIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, entry: CredentialEntry) {
        self.entries
            .entry(entry.kind.clone())
            .or_default()
            .push(entry);
    }

    /// Return all entries of a given kind.
    pub fn get(&self, kind: &CredentialKind) -> &[CredentialEntry] {
        self.entries.get(kind).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return the primary (first verified, or first) identifier for a kind.
    pub fn primary(&self, kind: &CredentialKind) -> Option<&str> {
        let entries = self.get(kind);
        entries
            .iter()
            .find(|e| e.verified)
            .or_else(|| entries.first())
            .map(|e| e.identifier.as_str())
    }

    pub fn all_kinds(&self) -> Vec<&CredentialKind> {
        self.entries.keys().collect()
    }

    pub fn count(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }
}
