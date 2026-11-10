use bipon39::{entropy_to_mnemonic, mnemonic_to_seed, split_mnemonic};
use blake3;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use pbkdf2::pbkdf2_hmac_array;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;

/// Canonical BIPỌ̀N39 encoding tokens (256), sourced from the `bipon39` crate.
///
/// Single source of truth: the same canonical set that backs
/// `bipon39::entropy_to_mnemonic`, so locally-generated mnemonics and
/// crate-generated ones index the identical list. (Previously this was an
/// `include_str!` of an asset that was never committed, which made the crate
/// uncompilable from a clean checkout.)
fn wordlist() -> &'static [&'static str] {
    bipon39::wordlist::all_encoding_tokens()
}

pub struct Bipon39;

impl Bipon39 {
    pub fn entropy_to_mnemonic(entropy: &[u8]) -> String {
        entropy_to_mnemonic(entropy)
            .map(|words| words.join(" "))
            .unwrap_or_default()
    }

    pub fn mnemonic_to_indices(mnemonic: &str) -> Result<Vec<u8>, String> {
        let words = split_mnemonic(mnemonic);
        let mut indices = Vec::new();
        for word in words {
            let entry = bipon39::entry_by_encoding(word)
                .map_err(|e| format!("Invalid word in mnemonic: {}: {}", word, e))?;
            indices.push(entry.array_index as u8);
        }
        Ok(indices)
    }

    pub fn mnemonic_to_seed(
        mnemonic: &str,
        _agent_id: &str,
        _birth_timestamp: u64,
        _chain_id: &str,
    ) -> [u8; 32] {
        let words = split_mnemonic(mnemonic);
        let seed64 = mnemonic_to_seed(&words, "")
            .expect("BIP39 seed derivation failed")
            .to_vec();
        let mut seed32 = [0u8; 32];
        seed32.copy_from_slice(&seed64[..32]);
        seed32
    }

    pub fn get_odu_index(indices: &[u8]) -> u8 {
        indices.iter().fold(0, |acc, &x| acc ^ x)
    }
}

/// Agent DNA fingerprint — immutable, derived at birth
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DnaFingerprint(pub [u8; 16]);

impl DnaFingerprint {
    pub fn from_mnemonic(mnemonic: &[String]) -> Self {
        let joined = mnemonic.join(" ");
        let hash = blake3::hash(joined.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&hash.as_bytes()[..16]);
        Self(bytes)
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// BIPỌ̀N39 Mnemonic — 24 words from 256-token wordlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bipon39Mnemonic {
    pub words: Vec<String>, // 24 words
    pub entropy: [u8; 32],  // Source entropy
    pub odù_primary: u8,    // Primary Odù (0-255)
    pub odù_secondary: u8,  // Secondary Odù
    pub dna_fingerprint: DnaFingerprint,
}

impl Bipon39Mnemonic {
    /// Generate new mnemonic from entropy (hardware RNG + IfáScript cowrie)
    pub fn generate(entropy: [u8; 32]) -> Result<Self, Bipon39Error> {
        if entropy.len() != 32 {
            return Err(Bipon39Error::InvalidEntropyLength);
        }

        // Convert entropy to 24 word indices. 256 bits of entropy + an 8-bit
        // checksum = 264 bits = exactly 24 x 11 bits. Packing must be done at
        // the *bit* level: chunking the byte buffer with `chunks(11)` only
        // yields 3 chunks and silently produced 3-word mnemonics.
        let mut bytes = entropy.to_vec();
        bytes.push(entropy[0]); // 8-bit checksum (simplified, deterministic)

        let mut indices = Vec::with_capacity(24);
        let mut idx: u16 = 0;
        for bit_pos in 0..264usize {
            let byte = bytes[bit_pos / 8];
            let bit = (byte >> (7 - (bit_pos % 8))) & 1;
            idx = (idx << 1) | bit as u16;
            if (bit_pos + 1) % 11 == 0 {
                indices.push((idx as usize) % 256);
                idx = 0;
            }
        }
        debug_assert_eq!(indices.len(), 24);

        let words: Vec<String> = indices.iter().map(|&i| wordlist()[i].to_string()).collect();

        // Derive Odù from first 2 words
        let odù_primary = Self::word_to_odù(&words[0])?;
        let odù_secondary = Self::word_to_odù(&words[1])?;

        let dna = DnaFingerprint::from_mnemonic(&words);

        Ok(Self {
            words,
            entropy,
            odù_primary,
            odù_secondary,
            dna_fingerprint: dna,
        })
    }

    /// Derive child agent mnemonic (for agent-born-agent)
    pub fn derive_child(&self, salt: &[u8; 32]) -> Result<Self, Bipon39Error> {
        let parent_entropy = self.entropy;
        let child_entropy = pbkdf2_hmac_array::<Sha256, 32>(&parent_entropy, salt, 100_000);
        Self::generate(child_entropy)
    }

    /// Derive Ed25519 signing key from mnemonic
    pub fn derive_signing_key(&self) -> SigningKey {
        let seed = blake3::hash(self.words.join(" ").as_bytes());
        SigningKey::from_bytes(&seed.as_bytes()[..32].try_into().unwrap())
    }

    /// Verify mnemonic round-trip
    pub fn verify(&self) -> bool {
        let computed = Self::generate(self.entropy);
        computed.map(|m| m.words == self.words).unwrap_or(false)
    }

    fn word_to_odù(word: &str) -> Result<u8, Bipon39Error> {
        wordlist()
            .iter()
            .position(|&w| w == word)
            .map(|i| i as u8)
            .ok_or(Bipon39Error::WordNotInWordlist)
    }
}

/// Agent Identity — three identities, cryptographically linked, never collapsed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Agent identity (npub) — Nostr public key
    pub agent_npub: String,
    /// Device identity — hardware fingerprint
    pub device_id: DeviceId,
    /// Economic identity — Sui address
    pub sui_address: SuiAddress,
    /// BIPỌ̀N39 mnemonic (printed ONCE at birth, never stored plaintext)
    #[serde(skip_serializing)]
    pub mnemonic: Option<Bipon39Mnemonic>,
    /// DNA fingerprint (stored, not mnemonic)
    pub dna_fingerprint: DnaFingerprint,
    /// Birth timestamp
    pub born_at: u64,
    /// Odù assignment
    pub odù_primary: u8,
    pub odù_secondary: u8,
}

/// Device Identity — hardware fingerprint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DeviceId(pub [u8; 32]);

impl DeviceId {
    pub fn from_hardware() -> Self {
        let mut rng = OsRng;
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rng, &mut bytes);
        Self(bytes)
    }
}

/// Sui Address — economic identity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SuiAddress(pub String); // 0x... format

impl SuiAddress {
    pub fn derive_from_seed(seed: &[u8; 32], path: &str) -> Self {
        // blake3::keyed_hash requires a fixed 32-byte key; derive it from the
        // derivation label + path so distinct paths give distinct addresses.
        let mut key = [0u8; 32];
        key.copy_from_slice(blake3::hash(format!("sui-derivation:{path}").as_bytes()).as_bytes());
        let derived = blake3::keyed_hash(&key, seed);
        Self(format!("0x{}", hex::encode(&derived.as_bytes()[..32])))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Bipon39Error {
    #[error("Invalid entropy length (expected 32 bytes)")]
    InvalidEntropyLength,
    #[error("Word not in BIPỌ̀N39 wordlist")]
    WordNotInWordlist,
    #[error("Invalid mnemonic length (expected 24 words)")]
    InvalidMnemonicLength,
    #[error("Checksum verification failed")]
    ChecksumFailed,
}

/// Birth Receipt — Kind 31000 (Nostr)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BirthReceipt {
    pub kind: u16, // 31000
    pub agent_npub: String,
    pub device_id: DeviceId,
    pub sui_address: SuiAddress,
    pub dna_fingerprint: DnaFingerprint,
    pub odù_primary: u8,
    pub odù_secondary: u8,
    pub bipon39_fingerprint: String, // SHA-256 of mnemonic words (not words!)
    pub ip_root: String,             // sha256(npub || device_id || birth_ts)
    pub timestamp: u64,
    pub signature: String, // Signed by NIP-46 bunker
}

impl BirthReceipt {
    pub fn new(identity: &AgentIdentity, timestamp: u64) -> Self {
        let ip_root = Self::compute_ip_root(&identity.agent_npub, &identity.device_id, timestamp);
        let bipon39_fp = identity
            .mnemonic
            .as_ref()
            .map(|m| blake3::hash(m.words.join(" ").as_bytes()).to_hex())
            .unwrap_or_default();

        Self {
            kind: 31000,
            agent_npub: identity.agent_npub.clone(),
            device_id: identity.device_id.clone(),
            sui_address: identity.sui_address.clone(),
            dna_fingerprint: identity.dna_fingerprint.clone(),
            odù_primary: identity.odù_primary,
            odù_secondary: identity.odù_secondary,
            bipon39_fingerprint: bipon39_fp.to_string(),
            ip_root,
            timestamp,
            signature: String::new(),
        }
    }

    fn compute_ip_root(npub: &str, device_id: &DeviceId, ts: u64) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(npub.as_bytes());
        hasher.update(&device_id.0);
        hasher.update(&ts.to_le_bytes());
        hex::encode(hasher.finalize().as_bytes())
    }
}

/// Device Binding Receipt — Kind 31001 (Nostr)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceBindingReceipt {
    pub kind: u16, // 31001
    pub agent_npub: String,
    pub device_id: DeviceId,
    pub sui_address: SuiAddress,
    pub ip_root: String,
    pub timestamp: u64,
    pub signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnemonic_generation() {
        let entropy = [42u8; 32];
        let mnemonic = Bipon39Mnemonic::generate(entropy).unwrap();
        assert_eq!(mnemonic.words.len(), 24);
        assert!(mnemonic.verify());
    }

    #[test]
    fn test_dna_fingerprint_deterministic() {
        let mnemonic = Bipon39Mnemonic::generate([1u8; 32]).unwrap();
        let dna1 = mnemonic.dna_fingerprint.clone();
        let dna2 = DnaFingerprint::from_mnemonic(&mnemonic.words);
        assert_eq!(dna1, dna2);
    }

    #[test]
    fn test_child_derivation() {
        let parent = Bipon39Mnemonic::generate([2u8; 32]).unwrap();
        let salt = [3u8; 32];
        let child = parent.derive_child(&salt).unwrap();
        assert_ne!(parent.words, child.words);
        assert_eq!(child.words.len(), 24);
    }

    #[test]
    fn test_signing_key_derivation() {
        let mnemonic = Bipon39Mnemonic::generate([5u8; 32]).unwrap();
        let key = mnemonic.derive_signing_key();
        let msg = b"test message";
        let sig = key.sign(msg);
        assert!(key.verifying_key().verify(msg, &sig).is_ok());
    }

    #[test]
    fn test_ip_root_uniqueness() {
        let receipt1 = BirthReceipt::new(
            &AgentIdentity {
                agent_npub: "npub1abc".into(),
                device_id: DeviceId([1u8; 32]),
                sui_address: SuiAddress("0x123".into()),
                mnemonic: None,
                dna_fingerprint: DnaFingerprint([0u8; 16]),
                born_at: 1000,
                odù_primary: 1,
                odù_secondary: 2,
            },
            1000,
        );

        let receipt2 = BirthReceipt::new(
            &AgentIdentity {
                agent_npub: "npub1abc".into(),
                device_id: DeviceId([1u8; 32]),
                sui_address: SuiAddress("0x123".into()),
                mnemonic: None,
                dna_fingerprint: DnaFingerprint([0u8; 16]),
                born_at: 1001,
                odù_primary: 1,
                odù_secondary: 2,
            },
            1001,
        );

        assert_ne!(receipt1.ip_root, receipt2.ip_root);
    }
}
