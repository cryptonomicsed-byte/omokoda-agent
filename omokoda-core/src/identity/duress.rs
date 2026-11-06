//! Duress protocol -- panic phrase triggers controlled response.
//! M6: Uses Argon2id (memory-hard KDF) instead of BLAKE3 for hash storage.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuressResponse {
    Wipe,
    Decoy { decoy_seed_hash: String },
    SilentAlert,
}

#[derive(Debug)]
pub enum DuressError {
    Hash(String),
}

impl std::fmt::Display for DuressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_'>) -> std::fmt::Result {
        match self {
            DuressError::Hash(e) => write!(f, "hash error: {}", e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DuressHandler {
    panic_phrase_hash: String,
    response: DuressResponse,
}

impl DuressHandler {
    pub fn new(panic_phrase: &str, response: DuressResponse) -> Self {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(panic_phrase.as_bytes(), &salt)
            .expect("argon2 hash failed")
            .to_string();
        Self {
            panic_phrase_hash: hash,
            response,
        }
    }

    pub fn from_stored_hash(hash_phc: &str, response: DuressResponse) -> Option<Self> {
        PasswordHash::new(hash_phc).ok()?;
        Some(Self {
            panic_phrase_hash: hash_phc.to_string(),
            response,
        })
    }

    pub fn is_duress(&self, input: &str) -> bool {
        let parsed = match PasswordHash::new(&self.panic_phrase_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        Argon2::default()
            .verify_password(input.as_bytes(), &parsed)
            .is_ok()
    }

    pub fn check_and_respond(&self, input: &str) -> Option<&DuressResponse> {
        if self.is_duress(input) {
            Some(&self.response)
        } else {
            None
        }
    }

    pub fn stored_hash(&self) -> &str {
        &self.panic_phrase_hash
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
    fn from_stored_hash_roundtrip() {
        let handler = DuressHandler::new("test phrase", DuressResponse::Wipe);
        let stored = handler.stored_hash().to_string();
        let reloaded = DuressHandler::from_stored_hash(&stored, DuressResponse::Wipe)
            .expect("from_stored_hash failed");
        assert!(reloaded.is_duress("test phrase"));
        assert!(!reloaded.is_duress("wrong"));
    }
}
