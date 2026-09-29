use crate::identity::odu::{OduIdentity, OduSeed};
use crate::identity::AgentId;
use argon2::{Algorithm, Argon2, Params, Version};
use blake3;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use zeroize::Zeroize;

pub const SESSION_VERSION: u32 = 2;
pub const ENCRYPTED_SESSION_VERSION: u32 = 1;
pub const ARGON2_MEMORY_KB: u32 = 65536;
pub const ARGON2_ITERATIONS: u32 = 3;
pub const ARGON2_PARALLELISM: u32 = 1;
pub const ARGON2_OUTPUT_LEN: u32 = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub version: u32,
    pub agent_id: AgentId,
    pub name: String,
    pub birth_timestamp: u64,
    pub reputation: f64,
    pub config: SessionConfig,
    pub public_messages: Vec<ConversationMessage>,
    /// Legacy single-blob vault — kept for backward compat + migration reads.
    #[serde(default)]
    pub encrypted_private: Option<EncryptedSession>,
    /// Gap #1: identity vault — keys only, sealed separately from memory.
    #[serde(default)]
    pub encrypted_identity_vault: Option<EncryptedSession>,
    /// Gap #1: memory vault — agent-authored entries, no key material.
    #[serde(default)]
    pub encrypted_memory_vault: Option<EncryptedSession>,
    pub warn_count: u32,
    pub cooldown_active: bool,
    pub think_history: Vec<String>,
    pub swarm_agents: Vec<AgentId>,
}

pub type SessionState = Session;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    pub default_provider: String,
    pub default_privacy: bool,
    pub default_sandbox: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default_provider: "default".to_string(),
            default_privacy: true,
            default_sandbox: true,
        }
    }
}

/// Versioned encrypted private session envelope.
///
/// Security invariants:
/// - `private_ciphertext` is the only persisted representation of private messages and Odu private data.
/// - `nonce` is generated randomly for every seal/rotation.
/// - `salt` is public KDF salt; it is not a secret.
/// - Argon2id parameters are persisted so future migrations can reject or upgrade old envelopes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedSession {
    pub version: u32,
    pub private_ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub salt: [u8; 16],
    pub key_version: u32,
    pub kdf: KdfParams,
}

pub type EncryptedData = EncryptedSession;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KdfParams {
    pub algorithm: String,
    pub memory_kb: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub output_len: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            algorithm: "argon2id-v0x13".to_string(),
            memory_kb: ARGON2_MEMORY_KB,
            iterations: ARGON2_ITERATIONS,
            parallelism: ARGON2_PARALLELISM,
            output_len: ARGON2_OUTPUT_LEN,
        }
    }
}

/// Zeroizing wrapper for passphrase-derived unlock keys held by the Steward.
#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveKey([u8; 32]);

impl SensitiveKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn zeroize_now(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for SensitiveKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SensitiveKey([redacted])")
    }
}

impl Drop for SensitiveKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMessage {
    pub role: MessageRole,
    pub blocks: Vec<ContentBlock>,
    pub is_private: bool,
    pub timestamp: u64,
    /// Token usage for this message, if recorded. Defaults to None on old data (backward-compatible).
    #[serde(default)]
    pub usage: Option<crate::usage::TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivateSessionData {
    pub odu_seed: OduSeed,
    pub odu_identity: OduIdentity,
    pub private_messages: Vec<ConversationMessage>,
    /// This agent's Vantage API key -- captured at birth/registration and
    /// never stored anywhere outside this sealed blob. See
    /// omokoda-agent-vault-secrets design (2026-07-26).
    #[serde(default)]
    pub vantage_api_key: Option<String>,
    /// Hex-encoded Ed25519 secret key bytes, the same wallet key already
    /// derivable via Wallet::derive_from_mnemonic -- stored here so it
    /// never needs to be re-derived through an unsealed mnemonic for
    /// routine use, and so it lives inside the self-sealed vault like any
    /// other agent-owned secret.
    #[serde(default)]
    pub wallet_private_key_hex: Option<String>,
    /// Additional chain keys derived from the same `odu_identity.mnemonic`
    /// at birth, alongside the Sui key above -- one child key per chain,
    /// same self-seal boundary. See `crate::identity::wallet` for the
    /// per-chain derivation paths (ported from vanity-cloakseed's
    /// chains.ts) and `Steward::birth()` for where these are minted.
    #[serde(default)]
    pub eth_private_key_hex: Option<String>,
    #[serde(default)]
    pub eth_address: Option<String>,
    #[serde(default)]
    pub btc_private_key_hex: Option<String>,
    #[serde(default)]
    pub btc_address: Option<String>,
    #[serde(default)]
    pub sol_private_key_hex: Option<String>,
    #[serde(default)]
    pub sol_address: Option<String>,
    #[serde(default)]
    pub cosmos_private_key_hex: Option<String>,
    #[serde(default)]
    pub cosmos_address: Option<String>,
    #[serde(default)]
    pub aptos_private_key_hex: Option<String>,
    #[serde(default)]
    pub aptos_address: Option<String>,
    #[serde(default)]
    pub nostr_private_key_hex: Option<String>,
    #[serde(default)]
    pub nostr_address: Option<String>,
    /// minipae NIP-AE agent identity (m/44'/30174'/<agent_index>'/<owner_index>'),
    /// a sibling derivation from the same mnemonic -- not a second unrelated
    /// key. See `crate::identity::wallet::derive_minipae_key`.
    #[serde(default)]
    pub minipae_private_key_hex: Option<String>,
    #[serde(default)]
    pub minipae_npub: Option<String>,
    /// Optional vanity address mined at birth (standalone keypair, not HD-derived).
    /// Present only when `vanity_prefix`/`vanity_suffix` birth metadata was supplied.
    #[serde(default)]
    pub vanity_private_key_hex: Option<String>,
    #[serde(default)]
    pub vanity_address: Option<String>,
    /// CREATE2 vanity contract: the 32-byte salt (hex) whose deployment address
    /// matches the requested prefix/suffix. The contract address itself is stored
    /// as a wallet binding (chain = "create2_contract") in the public manifest.
    #[serde(default)]
    pub create2_salt_hex: Option<String>,
    #[serde(default)]
    pub create2_contract_address: Option<String>,
    /// EIP-2307 keystore v3 JSON for the ETH private key, encrypted with the
    /// `keystore_password` birth metadata. None if no password was supplied at birth.
    /// Store this somewhere safe — it is the only recovery path if the vault is lost.
    #[serde(default)]
    pub eth_keystore_v3_json: Option<String>,

    // ── Inference / compute credentials ──────────────────────────────────────
    #[serde(default)]
    pub inference_endpoint: Option<String>,
    #[serde(default)]
    pub inference_provider: Option<String>,
    #[serde(default)]
    pub inference_model: Option<String>,
    #[serde(default)]
    pub gpu_ai_api_key: Option<String>,
    #[serde(default)]
    pub kaggle_username: Option<String>,
    #[serde(default)]
    pub kaggle_api_key: Option<String>,
}

/// Gap #1 — Tier 1 vault: key material only. No memory entries. Separate seal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityVaultData {
    pub odu_seed: OduSeed,
    pub odu_identity: OduIdentity,
    #[serde(default)]
    pub vantage_api_key: Option<String>,
    #[serde(default)]
    pub wallet_private_key_hex: Option<String>,
    #[serde(default)]
    pub eth_private_key_hex: Option<String>,
    #[serde(default)]
    pub eth_address: Option<String>,
    #[serde(default)]
    pub btc_private_key_hex: Option<String>,
    #[serde(default)]
    pub btc_address: Option<String>,
    #[serde(default)]
    pub sol_private_key_hex: Option<String>,
    #[serde(default)]
    pub sol_address: Option<String>,
    #[serde(default)]
    pub cosmos_private_key_hex: Option<String>,
    #[serde(default)]
    pub cosmos_address: Option<String>,
    #[serde(default)]
    pub aptos_private_key_hex: Option<String>,
    #[serde(default)]
    pub aptos_address: Option<String>,
    #[serde(default)]
    pub nostr_private_key_hex: Option<String>,
    #[serde(default)]
    pub nostr_address: Option<String>,
    #[serde(default)]
    pub minipae_private_key_hex: Option<String>,
    #[serde(default)]
    pub minipae_npub: Option<String>,

    // ── Inference / compute credentials ──────────────────────────────────────
    /// The agent's primary inference endpoint (larql, GPU.ai, RunPod, etc.).
    /// Set at birth via UCX provisioning; updated when the agent upgrades its
    /// compute tier or self-hosts a model after Mycelium fine-tune completes.
    #[serde(default)]
    pub inference_endpoint: Option<String>,
    /// Provider tag: "larql" | "gpu_ai" | "runpod" | "openai" | "kaggle"
    #[serde(default)]
    pub inference_provider: Option<String>,
    /// Model name served at inference_endpoint (e.g. "mycelium-q4_k_m").
    #[serde(default)]
    pub inference_model: Option<String>,
    /// GPU.ai API key assigned to this agent at birth (sub-keyed from ecosystem key).
    /// Stored here so the agent can independently provision GPU time via UCX.
    #[serde(default)]
    pub gpu_ai_api_key: Option<String>,
    /// Kaggle API credentials — assigned from a pre-provisioned agent account pool.
    /// Used to run fine-tuning notebooks, consume Kaggle datasets, submit simulations.
    #[serde(default)]
    pub kaggle_username: Option<String>,
    #[serde(default)]
    pub kaggle_api_key: Option<String>,

    // ── Phase 7.1 world keys ──────────────────────────────────────────────────
    /// libp2p Ed25519 peer id — DIP mesh identity, distinct derivation path
    #[serde(default)]
    pub libp2p_peer_id: Option<String>,
    #[serde(default)]
    pub libp2p_private_key_hex: Option<String>,
    /// HMAC-derived local part of agent email address (first 16 hex chars)
    #[serde(default)]
    pub agent_email_local: Option<String>,
    /// Full email address once mailbox is provisioned (local@domain)
    #[serde(default)]
    pub agent_email: Option<String>,
    #[serde(default)]
    pub agent_email_password: Option<String>,
    #[serde(default)]
    pub email_jmap_url: Option<String>,
    #[serde(default)]
    pub email_imap_host: Option<String>,
    #[serde(default)]
    pub email_smtp_host: Option<String>,
    #[serde(default)]
    pub agent_email_verified_at: Option<u64>,
    /// Nostr relay list this agent subscribes to
    #[serde(default)]
    pub relay_list: Option<Vec<String>>,
    /// Sui on-chain object IDs assigned at birth (fail-open — None if Sui unavailable)
    #[serde(default)]
    pub sui_soul_object_id: Option<String>,
    #[serde(default)]
    pub sui_agent_object_id: Option<String>,
}

impl Drop for IdentityVaultData {
    fn drop(&mut self) {
        self.odu_seed.0.zeroize();
        self.odu_identity.mnemonic.zeroize();
    }
}

/// Gap #1 — Tier 2 vault: agent-authored memory only. Zero key material.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryVaultData {
    /// Conversation history (retained for backward compat with PrivateSessionData).
    #[serde(default)]
    pub private_messages: Vec<ConversationMessage>,
    /// Structured Tier-2 private memory entries (thoughts, relations, etc.).
    #[serde(default)]
    pub entries: Vec<crate::memory::private_schema::PrivateMemoryEntry>,
    /// Agent compute wallet — Dopamine + Synapse balances and ledger.
    /// Populated at birth and updated on every GPU contribution + inference spend.
    #[serde(default)]
    pub compute_wallet: Option<crate::kernel::compute::wallet::AgentComputeWallet>,
}

impl Session {
    pub fn new(agent_id: AgentId, name: String, birth_timestamp: u64) -> Self {
        Self {
            version: SESSION_VERSION,
            agent_id,
            name,
            birth_timestamp,
            reputation: 0.0,
            config: SessionConfig::default(),
            public_messages: Vec::new(),
            encrypted_private: None,
            encrypted_identity_vault: None,
            encrypted_memory_vault: None,
            warn_count: 0,
            cooldown_active: false,
            think_history: Vec::new(),
            swarm_agents: Vec::new(),
        }
    }

    pub fn apply_metadata(&mut self, key: &str, value: &str) {
        match key {
            "provider" => self.config.default_provider = value.to_string(),
            "privacy" => self.config.default_privacy = value == "true",
            "sandbox" => self.config.default_sandbox = value == "true",
            _ => {}
        }
    }

    pub fn warn_count_this_session(&self) -> u32 {
        self.warn_count
    }

    pub fn increment_warn_count(&mut self) {
        self.warn_count += 1;
    }

    pub fn is_in_cooldown(&self) -> bool {
        self.cooldown_active
    }

    pub fn set_cooldown(&mut self, active: bool) {
        self.cooldown_active = active;
    }

    pub fn recent_thinks(&self) -> &[String] {
        &self.think_history
    }

    pub fn swarm_size(&self) -> usize {
        self.swarm_agents.len()
    }

    pub fn add_message(&mut self, message: ConversationMessage, reputation: f64) {
        if !message.is_private {
            self.push_public(message, reputation);
        }
    }

    pub fn push_public(&mut self, message: ConversationMessage, reputation: f64) {
        self.public_messages.push(message);
        let engine = crate::memory::MemoryEngine::new();
        engine.compress(&mut self.public_messages, reputation);
    }

    pub fn seal_private(
        &mut self,
        private_data: &PrivateSessionData,
        password_key: &[u8; 32],
    ) -> Result<(), String> {
        self.seal_private_with_version(private_data, password_key, 1)
    }

    fn seal_private_with_version(
        &mut self,
        private_data: &PrivateSessionData,
        password_key: &[u8; 32],
        key_version: u32,
    ) -> Result<(), String> {
        let salt = generate_salt(&self.agent_id, self.birth_timestamp);
        let mut key = derive_session_key(&salt, password_key, key_version);

        let mut json = serde_json::to_string(private_data)
            .map_err(|e| format!("failed to serialize private data: {e}"))?;

        let cipher = ChaCha20Poly1305::new(&key.into());
        key.zeroize();
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, json.as_bytes())
            .map_err(|e| format!("encryption failed: {e}"))?;
        json.zeroize();

        self.encrypted_private = Some(EncryptedSession {
            version: ENCRYPTED_SESSION_VERSION,
            private_ciphertext: ciphertext,
            nonce: nonce_bytes,
            salt,
            key_version,
            kdf: KdfParams::default(),
        });

        Ok(())
    }

    pub fn unseal_private(&self, password_key: &[u8; 32]) -> Result<PrivateSessionData, String> {
        let data = self
            .encrypted_private
            .as_ref()
            .ok_or_else(|| "no encrypted private data found".to_string())?;

        let mut key = derive_session_key(&data.salt, password_key, data.key_version);
        let cipher = ChaCha20Poly1305::new(&key.into());
        key.zeroize();
        let nonce = Nonce::from_slice(&data.nonce);

        let mut plaintext = cipher
            .decrypt(nonce, data.private_ciphertext.as_slice())
            .map_err(|e| format!("decryption failed: {e}"))?;

        let private_data: PrivateSessionData = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("failed to deserialize private data: {e}"))?;
        plaintext.zeroize();

        Ok(private_data)
    }

    pub fn rotate_key(
        &mut self,
        old_password_key: &[u8; 32],
        new_password_key: &[u8; 32],
    ) -> Result<(), String> {
        let private_data = self.unseal_private(old_password_key)?;
        let new_version = self
            .encrypted_private
            .as_ref()
            .map(|d| d.key_version)
            .unwrap_or(0)
            + 1;
        self.seal_private_with_version(&private_data, new_password_key, new_version)?;
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize session: {e}"))?;
        secure_write(path, json.as_bytes())?;
        Ok(())
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let session: Self = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        session.migrate()
    }

    /// Seal the identity vault (keys only) into its own encrypted blob.
    pub fn seal_identity_vault(
        &mut self,
        vault: &IdentityVaultData,
        password_key: &[u8; 32],
    ) -> Result<(), String> {
        self.encrypted_identity_vault = Some(Self::seal_blob(
            vault,
            password_key,
            &self.agent_id,
            self.birth_timestamp,
        )?);
        Ok(())
    }

    /// Unseal the identity vault.
    pub fn unseal_identity_vault(
        &self,
        password_key: &[u8; 32],
    ) -> Result<IdentityVaultData, String> {
        let blob = self
            .encrypted_identity_vault
            .as_ref()
            .ok_or("no identity vault")?;
        Self::unseal_blob(blob, password_key)
    }

    /// Seal the memory vault (entries only, no keys) into its own encrypted blob.
    pub fn seal_memory_vault(
        &mut self,
        vault: &MemoryVaultData,
        password_key: &[u8; 32],
    ) -> Result<(), String> {
        self.encrypted_memory_vault = Some(Self::seal_blob(
            vault,
            password_key,
            &self.agent_id,
            self.birth_timestamp,
        )?);
        Ok(())
    }

    /// Unseal the memory vault.
    pub fn unseal_memory_vault(&self, password_key: &[u8; 32]) -> Result<MemoryVaultData, String> {
        let blob = self
            .encrypted_memory_vault
            .as_ref()
            .ok_or("no memory vault")?;
        Self::unseal_blob(blob, password_key)
    }

    fn seal_blob<T: Serialize>(
        data: &T,
        password_key: &[u8; 32],
        agent_id: &AgentId,
        birth_ts: u64,
    ) -> Result<EncryptedSession, String> {
        let salt = generate_salt(agent_id, birth_ts);
        let mut key = derive_session_key(&salt, password_key, 1);
        let mut json = serde_json::to_string(data).map_err(|e| format!("serialize failed: {e}"))?;
        let cipher = ChaCha20Poly1305::new(&key.into());
        key.zeroize();
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, json.as_bytes())
            .map_err(|e| format!("encrypt failed: {e}"))?;
        json.zeroize();
        Ok(EncryptedSession {
            version: ENCRYPTED_SESSION_VERSION,
            private_ciphertext: ciphertext,
            nonce: nonce_bytes,
            salt,
            key_version: 1,
            kdf: KdfParams::default(),
        })
    }

    fn unseal_blob<T: serde::de::DeserializeOwned>(
        blob: &EncryptedSession,
        password_key: &[u8; 32],
    ) -> Result<T, String> {
        let mut key = derive_session_key(&blob.salt, password_key, blob.key_version);
        let cipher = ChaCha20Poly1305::new(&key.into());
        key.zeroize();
        let nonce = Nonce::from_slice(&blob.nonce);
        let mut plaintext = cipher
            .decrypt(nonce, blob.private_ciphertext.as_slice())
            .map_err(|e| format!("decrypt failed: {e}"))?;
        let data =
            serde_json::from_slice(&plaintext).map_err(|e| format!("deserialize failed: {e}"))?;
        plaintext.zeroize();
        Ok(data)
    }

    pub fn migrate(self) -> Result<Self, String> {
        match self.version {
            1 | SESSION_VERSION => Ok(Self {
                version: SESSION_VERSION,
                ..self
            }),
            other => Err(format!(
                "unsupported session version {other}; expected {SESSION_VERSION}"
            )),
        }
    }

    pub fn export_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("failed to export session: {e}"))
    }
}

impl PrivateSessionData {
    pub fn push_private(&mut self, message: ConversationMessage, reputation: f64) {
        self.private_messages.push(message);
        let engine = crate::memory::MemoryEngine::new();
        engine.compress(&mut self.private_messages, reputation);
    }
}

impl Drop for PrivateSessionData {
    fn drop(&mut self) {
        self.odu_seed.0.zeroize();
        self.odu_identity.mnemonic.zeroize();
        for message in &mut self.private_messages {
            message.zeroize_contents();
        }
        self.private_messages.clear();
    }
}

impl ConversationMessage {
    pub fn zeroize_contents(&mut self) {
        for block in &mut self.blocks {
            match block {
                ContentBlock::Text { text } => text.zeroize(),
                ContentBlock::ToolUse { id, name, input } => {
                    id.zeroize();
                    name.zeroize();
                    input.zeroize();
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    output,
                    is_error: _,
                } => {
                    tool_use_id.zeroize();
                    output.zeroize();
                }
            }
        }
        self.blocks.clear();
    }
}

impl ConversationMessage {
    pub fn new_user(content: String, is_private: bool) -> Self {
        Self {
            role: MessageRole::User,
            blocks: vec![ContentBlock::Text { text: content }],
            is_private,
            timestamp: current_unix_timestamp(),
            usage: None,
        }
    }

    pub fn new_assistant(content: String, is_private: bool) -> Self {
        Self {
            role: MessageRole::Assistant,
            blocks: vec![ContentBlock::Text { text: content }],
            is_private,
            timestamp: current_unix_timestamp(),
            usage: None,
        }
    }

    pub fn new_system(content: String, is_private: bool) -> Self {
        Self {
            role: MessageRole::System,
            blocks: vec![ContentBlock::Text { text: content }],
            is_private,
            timestamp: current_unix_timestamp(),
            usage: None,
        }
    }

    pub fn user_text(text: &str) -> Self {
        Self::new_user(text.to_string(), false)
    }

    pub fn assistant_text(text: &str) -> Self {
        Self::new_assistant(text.to_string(), false)
    }
}

const SESSION_CHAIN_ID: &str = "omokoda-main";

pub fn generate_salt(agent_id: &AgentId, birth_timestamp: u64) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(agent_id.as_str().as_bytes());
    hasher.update(&birth_timestamp.to_be_bytes());
    hasher.update(SESSION_CHAIN_ID.as_bytes());
    let result = hasher.finalize();
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&result.as_bytes()[..16]);
    salt
}

/// Derive the AEAD key with Argon2id using frozen session parameters.
///
/// Bound to the born agent via `salt` (derived from agent_id + birth_timestamp,
/// see `generate_salt`) -- never the Odu seed itself. The seed used to also be
/// mixed into this salt, which meant it had to stay readable in plaintext
/// outside the sealed blob just so `unseal_private` could be called at all --
/// a real bug (2026-07-26 finding): it defeated sealing for the seed and
/// everything derived from it (wallet keys, Buzz identity, etc.), since a
/// secret that must always be plaintext-readable to unlock its own vault
/// isn't actually sealed. Salt doesn't need to be secret, only bound to the
/// agent -- agent_id + birth_timestamp already provides that with no
/// circularity. The passphrase-derived `password_key` remains the ownership
/// secret and must be zeroized by callers.
fn derive_session_key(salt: &[u8; 16], password_key: &[u8; 32], key_version: u32) -> [u8; 32] {
    let params = Params::new(
        ARGON2_MEMORY_KB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(ARGON2_OUTPUT_LEN as usize),
    )
    .expect("invalid Argon2 parameters");

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut okm = [0u8; 32];

    let mut combined_salt = Vec::with_capacity(20);
    combined_salt.extend_from_slice(salt);
    combined_salt.extend_from_slice(&key_version.to_be_bytes());

    argon2
        .hash_password_into(password_key, &combined_salt, &mut okm)
        .expect("Argon2 key derivation failed");
    okm
}

pub fn derive_unlock_key(
    password: &str,
    agent_public_key: &[u8; 32],
) -> Result<SensitiveKey, String> {
    let params = Params::new(
        ARGON2_MEMORY_KB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(ARGON2_OUTPUT_LEN as usize),
    )
    .map_err(|e| format!("invalid Argon2 parameters: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut salt = [0u8; 32];
    let mut hasher = blake3::Hasher::new();
    hasher.update(agent_public_key);
    hasher.update(b"omokoda-unlock-key-v1");
    salt.copy_from_slice(hasher.finalize().as_bytes());

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| format!("unlock key derivation failed: {e}"))?;
    salt.zeroize();
    Ok(SensitiveKey::new(key))
}

pub fn secure_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("failed to create session dir: {e}"))?;
            set_strict_dir_permissions(parent)?;
        }
    }
    std::fs::write(path, bytes).map_err(|e| format!("failed to write session file: {e}"))?;
    std::fs::File::open(path)
        .map_err(|e| format!("failed to open file for sync: {e}"))?
        .sync_all()
        .map_err(|e| format!("failed to sync file: {e}"))?;
    set_strict_file_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
pub fn set_strict_dir_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("failed to set session dir permissions: {e}"))
}

#[cfg(not(unix))]
pub fn set_strict_dir_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
pub fn set_strict_file_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("failed to set session file permissions: {e}"))
}

#[cfg(not(unix))]
pub fn set_strict_file_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

// ── Session Lifecycle ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SessionPhase {
    #[default]
    Birth,
    Active,
    Hibernating,
    Dead,
}

impl std::fmt::Display for SessionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionPhase::Birth => write!(f, "Birth"),
            SessionPhase::Active => write!(f, "Active"),
            SessionPhase::Hibernating => write!(f, "Hibernating"),
            SessionPhase::Dead => write!(f, "Dead"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePoint {
    pub id: String,
    pub created_at: u64,
    pub label: String,
    pub message_count: usize,
    pub reputation_snapshot: f64,
    pub messages_snapshot: Vec<ConversationMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalSession {
    pub agent_id: AgentId,
    pub name: String,
    pub birth_timestamp: u64,
    pub last_seen: u64,
    pub phase: SessionPhase,
    pub message_count: usize,
    pub reputation: f64,
}

#[derive(Debug)]
pub struct SessionHistory {
    pub sessions: Vec<HistoricalSession>,
}

impl SessionHistory {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    pub fn record(&mut self, session: &Session, last_seen: u64) {
        let entry = HistoricalSession {
            agent_id: session.agent_id.clone(),
            name: session.name.clone(),
            birth_timestamp: session.birth_timestamp,
            last_seen,
            phase: SessionPhase::Dead,
            message_count: session.public_messages.len(),
            reputation: session.reputation,
        };
        self.sessions.push(entry);
    }

    pub fn list(&self) -> &[HistoricalSession] {
        &self.sessions
    }

    pub fn find_by_name(&self, name: &str) -> Option<&HistoricalSession> {
        self.sessions.iter().find(|s| s.name == name)
    }

    pub fn active_sessions(&self) -> Vec<&HistoricalSession> {
        self.sessions
            .iter()
            .filter(|s| s.phase == SessionPhase::Active)
            .collect()
    }
}

impl Default for SessionHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct SessionManager {
    pub session: Session,
    pub phase: SessionPhase,
    pub history: SessionHistory,
    restore_points: Vec<RestorePoint>,
}

impl SessionManager {
    pub fn new(session: Session) -> Self {
        Self {
            session,
            phase: SessionPhase::Birth,
            history: SessionHistory::new(),
            restore_points: Vec::new(),
        }
    }

    pub fn activate(&mut self) -> Result<(), String> {
        match self.phase {
            SessionPhase::Birth | SessionPhase::Hibernating => {
                self.phase = SessionPhase::Active;
                Ok(())
            }
            ref p => Err(format!("Cannot activate from phase {}", p)),
        }
    }

    pub fn hibernate(&mut self) -> Result<(), String> {
        match self.phase {
            SessionPhase::Active => {
                self.phase = SessionPhase::Hibernating;
                Ok(())
            }
            ref p => Err(format!("Cannot hibernate from phase {}", p)),
        }
    }

    pub fn resume(&mut self) -> Result<(), String> {
        match self.phase {
            SessionPhase::Hibernating => {
                self.phase = SessionPhase::Active;
                Ok(())
            }
            ref p => Err(format!("Cannot resume from phase {}", p)),
        }
    }

    pub fn terminate(&mut self) -> Result<(), String> {
        let now = current_unix_timestamp();
        self.history.record(&self.session, now);
        self.phase = SessionPhase::Dead;
        Ok(())
    }

    pub fn create_restore_point(&mut self, label: &str) -> String {
        let now = current_unix_timestamp();
        let mut hasher = blake3::Hasher::new();
        hasher.update(label.as_bytes());
        hasher.update(&now.to_be_bytes());
        let id = hasher.finalize().to_hex().to_string();

        let rp = RestorePoint {
            id: id.clone(),
            created_at: now,
            label: label.to_string(),
            message_count: self.session.public_messages.len(),
            reputation_snapshot: self.session.reputation,
            messages_snapshot: self.session.public_messages.clone(),
        };
        self.restore_points.push(rp);
        id
    }

    pub fn restore_to(&mut self, id: &str) -> Result<(), String> {
        if self.phase == SessionPhase::Dead {
            return Err("Cannot restore a Dead session".to_string());
        }
        let rp = self
            .restore_points
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Restore point '{}' not found", id))?;
        self.session.public_messages = rp.messages_snapshot.clone();
        self.session.reputation = rp.reputation_snapshot;
        Ok(())
    }

    pub fn restore_points(&self) -> &[RestorePoint] {
        &self.restore_points
    }

    /// Session serialization: persists live transcript to JSONL for cross-session reload.
    /// Each public message is one line; the session header is line 0.
    /// Writes only the live transcript so the file can be appended on each turn rather than rewritten entirely.
    pub fn save_to_disk(&self, base_dir: &Path) -> Result<(), String> {
        let sessions_dir = base_dir.join(".omokoda").join("sessions");
        fs::create_dir_all(&sessions_dir).map_err(|e| format!("create sessions dir: {}", e))?;

        let path = sessions_dir.join(format!("{}.jsonl", self.session.agent_id));
        let header = serde_json::to_string(&self.session)
            .map_err(|e| format!("serialize session: {}", e))?;

        let mut lines = vec![header];
        for msg in &self.session.public_messages {
            let line =
                serde_json::to_string(msg).map_err(|e| format!("serialize message: {}", e))?;
            lines.push(line);
        }

        fs::write(&path, lines.join("\n"))
            .map_err(|e| format!("write {}: {}", path.display(), e))?;
        Ok(())
    }

    /// Restore a session from a `.omokoda/sessions/<agent_id>.jsonl` file written by
    /// `save_to_disk`. Returns an error if the file does not exist or is malformed.
    pub fn load_from_disk(agent_id: &AgentId, base_dir: &Path) -> Result<Self, String> {
        let path = base_dir
            .join(".omokoda")
            .join("sessions")
            .join(format!("{}.jsonl", agent_id));

        let raw =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path.display(), e))?;

        let mut lines = raw.lines();
        let header_line = lines.next().ok_or("empty session file")?;
        let mut session: Session = serde_json::from_str(header_line)
            .map_err(|e| format!("parse session header: {}", e))?;

        session.public_messages.clear();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let msg: ConversationMessage =
                serde_json::from_str(line).map_err(|e| format!("parse message line: {}", e))?;
            session.public_messages.push(msg);
        }

        Ok(Self::new(session))
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::identity::AgentId;

    fn make_manager() -> SessionManager {
        let id = AgentId::new("test-fingerprint-lifecycle");
        let session = Session::new(id, "lifecycle-test".to_string(), 0);
        SessionManager::new(session)
    }

    #[test]
    fn test_activate_from_birth() {
        let mut mgr = make_manager();
        assert_eq!(mgr.phase, SessionPhase::Birth);
        mgr.activate().unwrap();
        assert_eq!(mgr.phase, SessionPhase::Active);
    }

    #[test]
    fn test_hibernate_and_resume() {
        let mut mgr = make_manager();
        mgr.activate().unwrap();
        mgr.hibernate().unwrap();
        assert_eq!(mgr.phase, SessionPhase::Hibernating);
        mgr.resume().unwrap();
        assert_eq!(mgr.phase, SessionPhase::Active);
    }

    #[test]
    fn test_terminate_records_history() {
        let mut mgr = make_manager();
        mgr.activate().unwrap();
        mgr.terminate().unwrap();
        assert_eq!(mgr.phase, SessionPhase::Dead);
        assert_eq!(mgr.history.list().len(), 1);
    }

    #[test]
    fn test_restore_point_roundtrip() {
        let mut mgr = make_manager();
        mgr.activate().unwrap();
        // Push 3 messages
        for i in 0..3 {
            mgr.session
                .public_messages
                .push(ConversationMessage::user_text(&format!("msg {}", i)));
        }
        assert_eq!(mgr.session.public_messages.len(), 3);
        let rp_id = mgr.create_restore_point("test-snapshot");
        // Push 3 more
        for i in 3..6 {
            mgr.session
                .public_messages
                .push(ConversationMessage::user_text(&format!("msg {}", i)));
        }
        assert_eq!(mgr.session.public_messages.len(), 6);
        mgr.restore_to(&rp_id).unwrap();
        assert_eq!(mgr.session.public_messages.len(), 3);
    }

    #[test]
    fn test_save_and_load_from_disk() {
        let tmp = std::env::temp_dir().join(format!(
            "omokoda_session_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let id = AgentId::new("disk-persist-fingerprint");
        let mut mgr = SessionManager::new(Session::new(id.clone(), "disk-test".to_string(), 0));
        mgr.session
            .public_messages
            .push(ConversationMessage::user_text("hello from disk"));
        mgr.session
            .public_messages
            .push(ConversationMessage::user_text("second message"));

        mgr.save_to_disk(&tmp).unwrap();

        let loaded = SessionManager::load_from_disk(&id, &tmp).unwrap();
        assert_eq!(loaded.session.name, "disk-test");
        assert_eq!(loaded.session.public_messages.len(), 2);
        if let crate::session::ContentBlock::Text { text } =
            &loaded.session.public_messages[0].blocks[0]
        {
            assert_eq!(text, "hello from disk");
        } else {
            panic!("unexpected block type");
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_from_disk_missing_file_errors() {
        let id = AgentId::new("no-such-agent-abcd");
        let result = SessionManager::load_from_disk(
            &id,
            std::path::Path::new("/tmp/does_not_exist_omokoda"),
        );
        assert!(result.is_err());
    }
}
