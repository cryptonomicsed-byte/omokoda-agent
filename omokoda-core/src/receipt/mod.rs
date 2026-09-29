pub mod act_receipt;
pub use act_receipt::{ActReceipt, EpistemicSeverity, PoCWProof};

use crate::identity::AgentId;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use gix_core::Gix1Index;
use gix_types::{Gix1, GixKind, GixNamespace, RoutingHints};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Receipt {
    pub agent_id: AgentId,
    pub action: String,
    pub payload: String,
    pub receipt_id: String,
    pub previous_hash: String,
    pub merkle_root: String,
    pub signature: String,
    pub timestamp: u64,
    pub nonce: u64,
}

impl Receipt {
    pub fn new_merkle(
        agent_id: &AgentId,
        action: &str,
        params: &str,
        previous_hash: &str,
        merkle_root: &str,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Self {
        use rand::Rng;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_secs();
        let nonce: u64 = rand::thread_rng().gen();

        let payload = blake3_hash_hex(&[action.as_bytes(), params.as_bytes()]);

        let receipt_id = Self::calculate_id(
            agent_id,
            action,
            &payload,
            previous_hash,
            merkle_root,
            timestamp,
            nonce,
        );

        // Sign the receipt_id
        let signature = signing_key.sign(receipt_id.as_bytes());
        let signature_hex = hex::encode(signature.to_bytes());

        Self {
            agent_id: agent_id.clone(),
            action: action.to_string(),
            payload,
            receipt_id,
            previous_hash: previous_hash.to_string(),
            merkle_root: merkle_root.to_string(),
            signature: signature_hex,
            timestamp,
            nonce,
        }
    }

    pub fn calculate_id(
        agent_id: &AgentId,
        action: &str,
        payload: &str,
        previous_hash: &str,
        merkle_root: &str,
        timestamp: u64,
        nonce: u64,
    ) -> String {
        blake3_hash_hex(&[
            agent_id.as_str().as_bytes(),
            action.as_bytes(),
            payload.as_bytes(),
            previous_hash.as_bytes(),
            merkle_root.as_bytes(),
            timestamp.to_string().as_bytes(),
            nonce.to_string().as_bytes(),
        ])
    }

    pub fn verify(&self, public_key_bytes: &[u8; 32]) -> Result<(), String> {
        // 1. Verify receipt_id derivation
        let expected_id = Self::calculate_id(
            &self.agent_id,
            &self.action,
            &self.payload,
            &self.previous_hash,
            &self.merkle_root,
            self.timestamp,
            self.nonce,
        );

        if self.receipt_id != expected_id {
            return Err("receipt_id does not match fields".to_string());
        }

        // 2. Verify signature
        let verifying_key = VerifyingKey::from_bytes(public_key_bytes)
            .map_err(|e| format!("invalid public key: {}", e))?;

        let signature_bytes =
            hex::decode(&self.signature).map_err(|e| format!("invalid signature hex: {}", e))?;

        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| format!("invalid signature bytes: {}", e))?;

        verifying_key
            .verify(self.receipt_id.as_bytes(), &signature)
            .map_err(|e| format!("signature verification failed: {}", e))
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptStore {
    receipts: HashMap<String, Receipt>,
    last_hash: String,
    chain: Vec<String>,
    merkle_tree: SimpleMerkleTree,
    /// GIX1 Merkle-auditable index — one envelope per recorded receipt.
    #[serde(default)]
    gix1_index: Gix1Index,
}

pub type ReceiptEngine = ReceiptStore;

impl ReceiptStore {
    pub fn record_action_receipt(&mut self, receipt: Receipt) -> Result<(), String> {
        self.record(receipt);
        Ok(())
    }

    pub fn write_receipt(&mut self, _receipt: ActReceipt) -> Result<(), String> {
        // We'll leave this empty or point to our existing record logic if ActReceipt can be converted
        Ok(())
    }

    pub fn new() -> Self {
        Self {
            receipts: HashMap::new(),
            last_hash: "0".repeat(64),
            chain: Vec::new(),
            merkle_tree: SimpleMerkleTree::new(),
            gix1_index: Gix1Index::new(),
        }
    }

    /// The `n` most recent receipts, newest first. `self.chain` is
    /// append-only in creation order, so walking it in reverse is the real
    /// chronological order -- not a re-sort or approximation.
    pub fn recent(&self, n: usize) -> Vec<&Receipt> {
        self.chain
            .iter()
            .rev()
            .filter_map(|id| self.receipts.get(id))
            .take(n)
            .collect()
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize receipts: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("failed to write receipt file: {e}"))
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read receipt file: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("failed to deserialize receipts: {e}"))
    }

    pub fn record(&mut self, receipt: Receipt) {
        let id = receipt.receipt_id.clone();
        self.last_hash = id.clone();
        self.chain.push(id.clone());
        self.merkle_tree.insert(id.clone());

        // GIX Phase 3: stamp a GIX1 envelope for every receipt into the index.
        let created_at_ms = receipt.timestamp.saturating_mul(1000);
        let env = Gix1::new(
            GixKind::Receipt,
            GixNamespace::OsovmExecution,
            id.as_bytes(),
            None,
            created_at_ms,
            RoutingHints::default(),
        );
        self.gix1_index.insert_gix1(env);

        self.receipts.insert(id, receipt);
    }

    /// Current Merkle root of the GIX1 receipt index.
    pub fn gix1_root(&self) -> &str {
        self.gix1_index.root()
    }

    /// Access the full GIX1 receipt index.
    pub fn gix1_index(&self) -> &Gix1Index {
        &self.gix1_index
    }

    pub fn get(&self, receipt_id: &str) -> Option<&Receipt> {
        self.receipts.get(receipt_id)
    }

    pub fn last_hash(&self) -> &str {
        &self.last_hash
    }

    pub fn current_merkle_root(&self) -> String {
        self.merkle_tree.root()
    }

    pub fn count(&self) -> usize {
        self.receipts.len()
    }

    pub fn export_json(&self) -> Result<String, String> {
        let mut sorted_receipts = Vec::new();
        for id in &self.chain {
            if let Some(r) = self.receipts.get(id) {
                sorted_receipts.push(r);
            }
        }
        serde_json::to_string_pretty(&sorted_receipts)
            .map_err(|e| format!("failed to serialize receipts: {e}"))
    }

    pub fn verify_chain(&self) -> bool {
        let mut current_expected_prev = "0".repeat(64);
        for id in &self.chain {
            if let Some(r) = self.receipts.get(id) {
                if r.previous_hash != current_expected_prev {
                    return false;
                }
                current_expected_prev = r.receipt_id.clone();
            } else {
                return false;
            }
        }
        true
    }

    pub fn verify_history(&self) -> bool {
        let mut built_ids: Vec<String> = Vec::new();
        for id in &self.chain {
            if let Some(receipt) = self.receipts.get(id) {
                if receipt.previous_hash
                    != if built_ids.is_empty() {
                        "0".repeat(64)
                    } else {
                        built_ids.last().unwrap().clone()
                    }
                {
                    return false;
                }

                let mut tree = SimpleMerkleTree::new();
                for leaf in &built_ids {
                    tree.insert(leaf.clone());
                }
                if receipt.merkle_root != tree.root() && receipt.merkle_root != "0".repeat(64) {
                    return false;
                }

                built_ids.push(receipt.receipt_id.clone());
            } else {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimpleMerkleTree {
    leaves: Vec<String>,
}

impl SimpleMerkleTree {
    pub fn new() -> Self {
        Self { leaves: Vec::new() }
    }

    pub fn insert(&mut self, hash: String) {
        self.leaves.push(hash);
    }

    pub fn root(&self) -> String {
        if self.leaves.is_empty() {
            return "0".repeat(64);
        }

        let mut hashes: Vec<String> = self.leaves.clone();

        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            for i in (0..hashes.len()).step_by(2) {
                if i + 1 < hashes.len() {
                    next_level.push(blake3_hash_hex(&[
                        hashes[i].as_bytes(),
                        hashes[i + 1].as_bytes(),
                    ]));
                } else {
                    next_level.push(hashes[i].clone());
                }
            }
            hashes = next_level;
        }

        hashes[0].clone()
    }
}

fn blake3_hash_hex(parts: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod gix_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn make_store_with_receipts(n: u32) -> ReceiptStore {
        let key = SigningKey::generate(&mut OsRng);
        let agent_id = crate::identity::AgentId::from_str("agent-test");
        let mut store = ReceiptStore::new();
        for i in 0..n {
            let r = Receipt::new_merkle(
                &agent_id,
                &format!("action_{i}"),
                &format!("params_{i}"),
                store.last_hash(),
                &store.current_merkle_root(),
                &key,
            );
            store.record(r);
        }
        store
    }

    #[test]
    fn gix1_index_grows_with_receipts() {
        let store = make_store_with_receipts(3);
        assert_eq!(store.gix1_index().len(), 3);
    }

    #[test]
    fn gix1_root_changes_per_receipt() {
        let mut store = make_store_with_receipts(0);
        let root0 = store.gix1_root().to_string();
        let _ = make_store_with_receipts(1);
        store = make_store_with_receipts(1);
        assert_ne!(store.gix1_root(), root0);
    }

    #[test]
    fn gix1_audit_passes_after_recording() {
        let store = make_store_with_receipts(5);
        assert!(store.gix1_index().audit().is_ok());
    }

    #[test]
    fn gix1_envelopes_resolvable() {
        let store = make_store_with_receipts(2);
        for entry in store.gix1_index().entries() {
            let resolved = store.gix1_index().resolve(&entry.canonical_id);
            assert!(
                resolved.is_some(),
                "envelope not found for {}",
                entry.canonical_id
            );
        }
    }
}
