//! Duress protocol — panic phrase triggers controlled response.
//! From vanity-cloakseed. Wipes sensitive data or redirects to decoy wallet.

use blake3;
use hex;

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
    response: DuressResponse,
}

impl DuressHandler {
    /// Register a duress handler. The panic phrase is hashed — never stored plaintext.
    pub fn new(panic_phrase: &str, response: DuressResponse) -> Self {
        let hash = blake3::hash(panic_phrase.as_bytes());
        let mut phrase_hash = [0u8; 32];
        phrase_hash.copy_from_slice(hash.as_bytes());
        Self {
            panic_phrase_hash: phrase_hash,
            response,
        }
    }

    /// Reconstruct a handler from a stored blake3 hash hex (for agents loaded from disk).
    /// `response` is the desired action when triggered.
    pub fn from_stored_hash(hash_hex: &str, response: DuressResponse) -> Option<Self> {
        let bytes = hex::decode(hash_hex).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let mut phrase_hash = [0u8; 32];
        phrase_hash.copy_from_slice(&bytes);
        Some(Self {
            panic_phrase_hash: phrase_hash,
            response,
        })
    }

    /// Check if input matches the panic phrase (hash comparison).
    pub fn is_duress(&self, input: &str) -> bool {
        let hash = blake3::hash(input.as_bytes());
        let mut input_hash = [0u8; 32];
        input_hash.copy_from_slice(hash.as_bytes());
        self.panic_phrase_hash == input_hash
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
}
