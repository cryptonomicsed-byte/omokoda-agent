//! Duress protocol — panic phrase triggers controlled response.
//! From vanity-cloakseed. Wipes sensitive data or redirects to decoy wallet.
//!
//! M6: panic phrase is hashed with Argon2id + per-agent salt (not unsalted BLAKE3).
//! Argon2id resists brute-force and GPU attacks; the per-agent salt ensures
//! identical phrases produce different hashes across agents.

use argon2::{Algorithm, Argon2, Params, Version};
use hex;

/// Sentinel salt: all-zeros signals legacy blake3 path (pre-Argon2id).
/// Used when loading old snapshots that have no salt stored.
const LEGACY_SALT: [u8; 16] = [0u8; 16];

/// The duress response: what happens when the panic phrase is entered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuressResponse {
    /// Wipe sensitive keys from memory immediately.
    Wipe,
    /// Redirect to a decoy wallet (empty or with small balance).
    Decoy { decoy_seed_hash: String },
    /// Log silent alert and continue as if normal.
    SilentAlert,
}

/// Duress handler — registered at birth with a secret panic phrase.
#[derive(Debug, Clone)]
pub struct DuressHandler {
    panic_phrase_hash: [u8; 32],
    salt: [u8; 16],
    response: DuressResponse,
}

/// Argon2id parameters for duress phrase hashing.
/// m=65536 (64 MiB), t=3 iterations, p=1 thread — balanced for registration
/// (happens once at birth) while remaining expensive for attackers.
const ARGON2_M_COST: u32 = 65_536;
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 1;

fn argon2id_hash(phrase: &str, salt: &[u8; 16]) -> [u8; 32] {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .expect("valid Argon2id params");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = [0u8; 32];
    argon2
        .hash_password_into(phrase.as_bytes(), salt, &mut output)
        .expect("Argon2id hash failed");
    output
}

impl DuressHandler {
    /// Register a duress handler with a random per-agent salt.
    /// The panic phrase is hashed with Argon2id — never stored plaintext.
    pub fn new(panic_phrase: &str, response: DuressResponse) -> Self {
        use rand::RngCore;
        let mut salt = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        let hash = argon2id_hash(panic_phrase, &salt);
        Self {
            panic_phrase_hash: hash,
            salt,
            response,
        }
    }

    /// Reconstruct a handler from a stored hash hex.
    ///
    ///  is  for Argon2id hashes (new agents) or
    /// for legacy blake3 hashes (pre-M6 snapshots).  The all-zeros
    /// sentinel is stored internally so  knows which path to use.
    pub fn from_stored_hash(
        hash_hex: &str,
        salt_hex: Option<&str>,
        response: DuressResponse,
    ) -> Option<Self> {
        let hash_bytes = hex::decode(hash_hex).ok()?;
        if hash_bytes.len() != 32 {
            return None;
        }
        let mut phrase_hash = [0u8; 32];
        phrase_hash.copy_from_slice(&hash_bytes);
        let salt = match salt_hex {
            Some(s) => {
                let sb = hex::decode(s).ok()?;
                if sb.len() != 16 {
                    return None;
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&sb);
                arr
            }
            None => LEGACY_SALT,
        };
        Some(Self {
            panic_phrase_hash: phrase_hash,
            salt,
            response,
        })
    }

    /// Return the salt as hex (store alongside the hash for future verification).
    pub fn salt_hex(&self) -> String {
        hex::encode(self.salt)
    }

    /// Return the hash as hex (for persistence).
    pub fn hash_hex(&self) -> String {
        hex::encode(self.panic_phrase_hash)
    }

    /// Check if input matches the panic phrase.
    /// Uses Argon2id + constant-time compare (new agents) or blake3 (legacy).
    pub fn is_duress(&self, input: &str) -> bool {
        let input_hash = if self.salt == LEGACY_SALT {
            // Legacy path: blake3 of the raw phrase (pre-Argon2id snapshots)
            *blake3::hash(input.as_bytes()).as_bytes()
        } else {
            argon2id_hash(input, &self.salt)
        };
        // Constant-time compare
        self.panic_phrase_hash
            .iter()
            .zip(input_hash.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }

    /// Execute the duress response if triggered.
    pub fn check_and_respond(&self, input: &str) -> Option<&DuressResponse> {
        if self.is_duress(input) {
            Some(&self.response)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_phrase_triggers_duress() {
        let handler = DuressHandler::new("orange sunset seven", DuressResponse::Wipe);
        assert!(handler.is_duress("orange sunset seven"));
        assert!(!handler.is_duress("wrong phrase"));
    }

    #[test]
    fn duress_response_returned_on_match() {
        let handler = DuressHandler::new("panic123", DuressResponse::SilentAlert);
        let resp = handler.check_and_respond("panic123");
        assert_eq!(resp, Some(&DuressResponse::SilentAlert));
    }

    #[test]
    fn no_response_on_normal_input() {
        let handler = DuressHandler::new("secret", DuressResponse::Wipe);
        assert!(handler.check_and_respond("normal input").is_none());
    }

    #[test]
    fn same_phrase_different_agents_different_hashes() {
        let h1 = DuressHandler::new("same phrase", DuressResponse::Wipe);
        let h2 = DuressHandler::new("same phrase", DuressResponse::Wipe);
        // Different salts → different hashes
        assert_ne!(h1.panic_phrase_hash, h2.panic_phrase_hash);
        // But both still match the original phrase
        assert!(h1.is_duress("same phrase"));
        assert!(h2.is_duress("same phrase"));
    }

    #[test]
    fn from_stored_hash_roundtrip() {
        let handler = DuressHandler::new("roundtrip", DuressResponse::SilentAlert);
        let hash_hex = handler.hash_hex();
        let salt_hex = handler.salt_hex();
        let restored = DuressHandler::from_stored_hash(
            &hash_hex,
            Some(&salt_hex),
            DuressResponse::SilentAlert,
        )
        .expect("restore failed");
        assert!(restored.is_duress("roundtrip"));
        assert!(!restored.is_duress("wrong"));
    }
}
