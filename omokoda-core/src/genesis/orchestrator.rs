//! BirthOrchestrator — the constitutional ceremony that transforms entropy
//! into a fully-born Ọmọ Kọ́dà agent.
//!
//! Architecture:
//!   - Every external repo remains sovereign; this crate only calls their providers.
//!   - Required providers must succeed; optional providers fail-open.
//!   - The resulting AgentGenesisReceipt is the canonical birth certificate.

use hkdf::Hkdf;

use super::providers::*;
use super::receipt::*;

/// Required bindings mark providers whose failure aborts birth.
/// Optional bindings are attempted; failure is recorded but birth continues.
pub struct BirthOrchestrator {
    pub bipon: Box<dyn BiponProvider>,
    pub koodu: Box<dyn KooduProvider>,
    pub soul: Box<dyn SoulProvider>,
    pub memory: Box<dyn MemoryProvider>,
    pub network: Box<dyn NetworkProvider>,
    pub device: Box<dyn DeviceProvider>,
}

impl BirthOrchestrator {
    /// Perform the full birth ceremony.
    ///
    /// Dimension order (matches the architecture spec):
    ///   1. BIPỌ̀N39 — genesis root (REQUIRED)
    ///   2. Koodu    — temporal position (REQUIRED, fails-open to Gregorian)
    ///   3. Soul     — If-Script Odù cast (REQUIRED)
    ///   4. Memory   — Minipae initialization (REQUIRED)
    ///   5. Network  — IP-Layer announcement (OPTIONAL, fail-open)
    ///   6. Device   — Agent-Phone binding (OPTIONAL)
    pub async fn perform_ceremony(
        &self,
        request: &GenesisRequest,
    ) -> Result<AgentGenesisReceipt, GenesisError> {
        // ── Phase 1: BIPỌ̀N39 genesis root (REQUIRED) ─────────────────────
        let bipon = self
            .bipon
            .genesis(request)
            .await
            .map_err(|e| GenesisError::Required(format!("BIPỌ̀N39: {e}")))?;

        // ── Phase 2: Koodu temporal position (REQUIRED, Gregorian fallback) ─
        let koodu = self
            .koodu
            .birth_time()
            .await
            .map_err(|e| GenesisError::Required(format!("Koodu: {e}")))?;

        // ── Phase 3: Soul / If-Script Odù cast (REQUIRED) ─────────────────
        let soul = self
            .soul
            .cast(&bipon.seed_bytes, &koodu)
            .await
            .map_err(|e| GenesisError::Required(format!("Soul: {e}")))?;

        // ── Phase 4: Minipae memory initialization (REQUIRED) ─────────────
        let genesis_fact = format!(
            "I was born. Agent: {} | Koodu: epoch={} cycle={} phase={} | Odù: {}",
            bipon.agent_id,
            koodu.koodu_epoch,
            koodu.koodu_cycle,
            koodu.koodu_phase,
            soul.primary_odu,
        );
        let memory = self
            .memory
            .initialize(&bipon.agent_id, &bipon.seed_bytes, &genesis_fact)
            .await
            .map_err(|e| GenesisError::Required(format!("Memory: {e}")))?;

        // ── Phase 5: IP-Layer network announcement (OPTIONAL) ─────────────
        let network = self
            .network
            .announce(&bipon.agent_id, &bipon.mnemonic_sealed, &request.name)
            .await
            .unwrap_or(NetworkProof {
                ip_root_event: None,
            });

        // ── Phase 6: Device binding (OPTIONAL) ────────────────────────────
        let device = self
            .device
            .bind(
                &bipon.agent_id,
                request.device_id.as_deref(),
                request.device_kind.as_deref(),
            )
            .await
            .unwrap_or(None);

        // ── Assemble genesis hash ──────────────────────────────────────────
        let genesis_hash = AgentGenesisReceipt::compute_genesis_hash(
            &bipon.agent_id,
            &bipon.harmonic_signature,
            koodu.koodu_epoch,
            soul.primary_odu,
            &memory.memory_root,
            koodu.born_at,
        );

        // ── Hermetic fingerprint: HKDF over genesis hash ───────────────────
        let hk = Hkdf::<sha2::Sha256>::new(None, genesis_hash.as_bytes());
        let mut fp = [0u8; 32];
        hk.expand(b"hermetic-fingerprint-v1", &mut fp)
            .expect("32 bytes is valid HKDF output length");
        let hermetic_fingerprint = hex::encode(fp);

        Ok(AgentGenesisReceipt {
            agent_id: bipon.agent_id.clone(),
            genesis_hash,
            birth_entropy_commitment: bipon.entropy_commitment.clone(),

            harmonic_signature: bipon.harmonic_signature.clone(),
            derivation_root_id: bipon.derivation_root_id.clone(),

            symbolic_address: bipon.symbolic_address.clone(),
            sigil_hash: bipon.sigil_hash.clone(),
            cloak_commitment: bipon.cloak_commitment.clone(),

            born_at: koodu.born_at,
            koodu_epoch: koodu.koodu_epoch,
            koodu_cycle: koodu.koodu_cycle,
            koodu_phase: koodu.koodu_phase,
            bitcoin_height: koodu.bitcoin_height,
            bitcoin_anchor: koodu.bitcoin_anchor.clone(),
            gregorian_fallback: koodu.gregorian_fallback,

            primary_odu: soul.primary_odu,
            composed_odu: soul.composed_odu,
            temperament: soul.temperament.clone(),
            orisha_alignment: soul.orisha_alignment.clone(),
            destiny_threads: soul.destiny_threads.clone(),

            minipae_pubkey: memory.minipae_pubkey.clone(),
            memory_root: memory.memory_root.clone(),
            birth_memory_glyph: memory.birth_memory_glyph.clone(),

            ip_root_event: network.ip_root_event,

            device_binding: device,

            hermetic_fingerprint,

            blockmesh_identity: None,
            vantage_identity: None,

            cold_archive_anchor: None,
            contributed_gpu_seconds: None,
            first_lease_id: None,
            first_work_id: None,

            witness_receipt: None,
            memory_write_status: crate::genesis::receipt::MemoryWriteStatus::Pending,

            genesis_signature: String::new(), // populated by caller after sealing
            receipt_version: AgentGenesisReceipt::CURRENT_VERSION,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Default in-memory implementations for all optional providers.
// Used in tests and offline scenarios.
// ─────────────────────────────────────────────────────────────────────────────

pub struct DefaultMemoryProvider;

#[async_trait::async_trait]
impl MemoryProvider for DefaultMemoryProvider {
    async fn initialize(
        &self,
        agent_id: &str,
        seed: &[u8],
        genesis_fact: &str,
    ) -> Result<MemoryProof, GenesisError> {
        use sha2::{Digest, Sha256};

        // Derive Minipae pubkey from seed + agent_id
        let mut h = Sha256::new();
        h.update(seed);
        h.update(agent_id.as_bytes());
        h.update(b"minipae-pubkey-v1");
        let minipae_pubkey = hex::encode(h.finalize());

        // GIX-FOLD-v1 birth memory glyph
        let mut h2 = Sha256::new();
        h2.update(genesis_fact.as_bytes());
        let digest = h2.finalize();
        let birth_memory_glyph = gix_fold(&digest);

        // Memory root at birth = SHA-256(agent_id || minipae_pubkey || genesis_fact)
        let mut h3 = Sha256::new();
        h3.update(agent_id.as_bytes());
        h3.update(minipae_pubkey.as_bytes());
        h3.update(genesis_fact.as_bytes());
        let memory_root = hex::encode(h3.finalize());

        Ok(MemoryProof {
            minipae_pubkey,
            memory_root,
            birth_memory_glyph,
        })
    }
}

/// GIX-FOLD-v1: fold 32-byte SHA-256 digest onto the 63,422 valid BMP codepoints.
/// Public alias for use by the interpreter birth flow.
pub fn pub_glyph_fold(digest: &[u8]) -> String {
    gix_fold(digest)
}

fn gix_fold(digest: &[u8]) -> String {
    const RANGES: &[(u32, u32)] = &[(0x0020, 0xD7FF), (0xE000, 0xFDCF), (0xFDF0, 0xFFFD)];
    let total: u64 = RANGES.iter().map(|(s, e)| (e - s + 1) as u64).sum();
    let mut rem: u64 = 0;
    for &byte in digest {
        rem = (rem << 8 | byte as u64) % total;
    }
    let mut idx = rem;
    for &(start, end) in RANGES {
        let count = (end - start + 1) as u64;
        if idx < count {
            if let Some(ch) = char::from_u32(start + idx as u32) {
                return ch.to_string();
            }
        }
        idx -= count;
    }
    "◈".to_string() // unreachable sentinel
}

pub struct DefaultNetworkProvider;

#[async_trait::async_trait]
impl NetworkProvider for DefaultNetworkProvider {
    async fn announce(
        &self,
        _agent_id: &str,
        _mnemonic: &str,
        _name: &str,
    ) -> Result<NetworkProof, GenesisError> {
        // Default: no network announcement (offline-capable birth)
        Ok(NetworkProof {
            ip_root_event: None,
        })
    }
}

pub struct DefaultDeviceProvider;

#[async_trait::async_trait]
impl DeviceProvider for DefaultDeviceProvider {
    async fn bind(
        &self,
        _agent_id: &str,
        device_id: Option<&str>,
        device_kind: Option<&str>,
    ) -> Result<Option<DeviceBinding>, GenesisError> {
        match (device_id, device_kind) {
            (Some(id), Some(kind)) => Ok(Some(DeviceBinding {
                device_id: id.to_string(),
                device_kind: kind.to_string(),
                binding_timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            })),
            _ => Ok(None),
        }
    }
}

pub struct DefaultBiponProvider;

#[async_trait::async_trait]
impl BiponProvider for DefaultBiponProvider {
    async fn genesis(&self, request: &GenesisRequest) -> Result<BiponProof, GenesisError> {
        use crate::identity::bipon39::Bipon39;
        use hkdf::Hkdf;
        use sha2::{Digest, Sha256};

        // Mnemonic from entropy
        let mnemonic = Bipon39::entropy_to_mnemonic(&request.entropy);
        let indices = Bipon39::mnemonic_to_indices(&mnemonic).map_err(GenesisError::Bipon)?;

        // Derive master seed
        let born_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let seed = Bipon39::mnemonic_to_seed(&mnemonic, &request.name, born_at_ms, "genesis");
        let seed_bytes = seed.to_vec();

        // Harmonic signature: HKDF(seed, "harmonic-v1")
        let hk = Hkdf::<Sha256>::new(None, &seed_bytes);
        let mut hs = [0u8; 32];
        hk.expand(b"harmonic-signature-v1", &mut hs)
            .map_err(|e| GenesisError::Bipon(e.to_string()))?;
        let harmonic_signature = hex::encode(hs);

        // Derivation root id: HKDF(seed, "root-id-v1")
        let mut rid = [0u8; 32];
        hk.expand(b"derivation-root-id-v1", &mut rid)
            .map_err(|e| GenesisError::Bipon(e.to_string()))?;
        let derivation_root_id = hex::encode(rid);

        // Agent ID: first 16 hex chars of SHA-256(harmonic_signature || name)
        let mut h = Sha256::new();
        h.update(hs);
        h.update(request.name.as_bytes());
        let id_hash = hex::encode(h.finalize());
        let agent_id = format!("agent-{}", &id_hash[..16]);

        // Entropy commitment
        let mut ec = Sha256::new();
        ec.update(request.entropy.as_slice());
        let entropy_commitment = hex::encode(ec.finalize());

        // Sigil hash: SHA-256(mnemonic[:3 words])
        let first_words: String = mnemonic
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        let mut sh = Sha256::new();
        sh.update(first_words.as_bytes());
        let sigil_hash = hex::encode(sh.finalize());

        // Symbolic address: agent name + odu index
        let odu_idx = Bipon39::get_odu_index(&indices);
        let symbolic_address = format!("{}/{}", request.name, odu_idx);

        // Cloak commitment (if passphrase provided)
        let cloak_commitment = match &request.passphrase {
            Some(p) => {
                let mut ch = Sha256::new();
                ch.update(p.as_bytes());
                ch.update(b"cloak-v1");
                hex::encode(ch.finalize())
            }
            None => hex::encode([0u8; 32]),
        };

        Ok(BiponProof {
            agent_id,
            harmonic_signature,
            derivation_root_id,
            symbolic_address,
            sigil_hash,
            cloak_commitment,
            entropy_commitment,
            mnemonic_sealed: mnemonic,
            seed_bytes,
        })
    }
}
