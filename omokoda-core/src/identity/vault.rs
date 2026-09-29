use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub agent_id: String,
    pub tool: String,
    pub expiry: u64,
    pub signature: Vec<u8>,
}

impl CapabilityToken {
    pub fn sign(agent_id: &str, tool: &str, k_root: &[u8; 32]) -> Self {
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600; // 1 hour

        let message = format!("{}:{}:{}", agent_id, tool, expiry);
        let mut hmac =
            Hmac::<Sha256>::new_from_slice(k_root).expect("HMAC can take key of any size");
        hmac.update(message.as_bytes());
        let signature = hmac.finalize().into_bytes().to_vec();

        Self {
            agent_id: agent_id.to_string(),
            tool: tool.to_string(),
            expiry,
            signature,
        }
    }

    pub fn verify(&self, k_root: &[u8; 32]) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if self.expiry < now {
            return false;
        }

        let message = format!("{}:{}:{}", self.agent_id, self.tool, self.expiry);
        let mut hmac =
            Hmac::<Sha256>::new_from_slice(k_root).expect("HMAC can take key of any size");
        hmac.update(message.as_bytes());

        hmac.verify_slice(&self.signature).is_ok()
    }
}

/// Tier 1 identity vault: all derived keys and operational credentials.
/// Logically separate from MemoryVault (private knowledge) — keys vs. thoughts.
/// Constructed from PrivateSessionData at birth; can be re-derived from mnemonic.
#[derive(Debug, Clone)]
pub struct IdentityVault {
    /// Raw entropy seed (Tier 0 — never transmitted, never backed up).
    pub odu_seed_bytes: [u8; 32],
    /// Mnemonic (Tier 0 — same constraints).
    pub mnemonic: String,
    /// Vantage operational API key.
    pub vantage_api_key: Option<String>,
    // ── Chain keys (Tier 1 — regenerable from mnemonic) ─────────────────────
    pub wallet_private_key_hex: Option<String>, // Sui
    pub eth_private_key_hex: Option<String>,
    pub eth_address: Option<String>,
    pub btc_private_key_hex: Option<String>,
    pub btc_address: Option<String>,
    pub sol_private_key_hex: Option<String>,
    pub sol_address: Option<String>,
    pub cosmos_private_key_hex: Option<String>,
    pub cosmos_address: Option<String>,
    pub aptos_private_key_hex: Option<String>,
    pub aptos_address: Option<String>,
    pub nostr_private_key_hex: Option<String>,
    pub nostr_address: Option<String>,
    pub minipae_private_key_hex: Option<String>,
    pub minipae_npub: Option<String>,
}

impl IdentityVault {
    /// Convenience constructor from the sealed session blob.
    pub fn from_session(psd: &crate::session::PrivateSessionData) -> Self {
        Self {
            odu_seed_bytes: *psd.odu_seed.as_bytes(),
            mnemonic: psd.odu_identity.mnemonic.clone(),
            vantage_api_key: psd.vantage_api_key.clone(),
            wallet_private_key_hex: psd.wallet_private_key_hex.clone(),
            eth_private_key_hex: psd.eth_private_key_hex.clone(),
            eth_address: psd.eth_address.clone(),
            btc_private_key_hex: psd.btc_private_key_hex.clone(),
            btc_address: psd.btc_address.clone(),
            sol_private_key_hex: psd.sol_private_key_hex.clone(),
            sol_address: psd.sol_address.clone(),
            cosmos_private_key_hex: psd.cosmos_private_key_hex.clone(),
            cosmos_address: psd.cosmos_address.clone(),
            aptos_private_key_hex: psd.aptos_private_key_hex.clone(),
            aptos_address: psd.aptos_address.clone(),
            nostr_private_key_hex: psd.nostr_private_key_hex.clone(),
            nostr_address: psd.nostr_address.clone(),
            minipae_private_key_hex: psd.minipae_private_key_hex.clone(),
            minipae_npub: psd.minipae_npub.clone(),
        }
    }
}

pub struct SealVault;

impl SealVault {
    /// Generates an internal secret (K_root) inside the vault.
    pub fn generate_internal_secret() -> [u8; 32] {
        let mut k_root = [0u8; 32];
        rand::thread_rng().fill(&mut k_root);
        k_root
    }

    /// Generates a deterministic internal secret (K_root) for a given name and seed.
    pub fn generate_deterministic_secret(name: &str, master_seed: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        hasher.update(master_seed);
        hasher.finalize().into()
    }

    /// Reconstructs K_root from threshold shares.
    pub fn reconstruct_secret(_shares: Vec<Vec<u8>>) -> Result<[u8; 32], String> {
        Err("Threshold reconstruction not implemented in stub".to_string())
    }
}
