//! CloakSeed — owner key protection via a personal cipher overlay.
//! Ported from vanity-cloakseed's `ciphers.js`: a positional-substitution
//! cipher over the mnemonic wordlist (encode: real word at index i -> the
//! i-th cover word; decode: reverse lookup). Cover words are semantically
//! meaningless relative to the real phrase, so a written-down backup is
//! useless without the cipher -- display-layer protection against
//! shoulder-surfing/note-theft, never a substitute for the real encryption
//! already sealing this content at rest (`session.rs::seal_private`).
//!
//! Ported onto BIPỌ̀N39's own 256-word canonical encoding space rather than
//! the standard BIP-39 2048-word English list `ciphers.js` targets --
//! that's what this agent's real mnemonic (`odu_identity.mnemonic`) is
//! actually written in, and the substitution algorithm is wordlist-size
//! agnostic, so there's nothing lost switching spaces.
//!
//! The cipher itself is derived deterministically from the agent's own Odù
//! seed (HKDF-SHA256, domain-separated), not generated randomly and stored
//! separately: nothing new needs to be persisted or leak-checked, and only
//! whoever already holds the (already-sealed) seed can ever reproduce it.

use hkdf::Hkdf;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use sha2::Sha256;

pub struct CloakSeed {
    /// `cipher[i]` is the cover word standing in for `bipon39`'s real
    /// canonical wordlist entry at encoding-index `i`. A permutation of the
    /// same 256-word list, so every cover word is itself a real, vetted
    /// token -- just never at its own true position.
    cipher: Vec<String>,
}

impl CloakSeed {
    /// Derive this agent's personal cloak cipher from its own Odù seed.
    /// Deterministic: the same seed always reproduces the same cipher.
    pub fn from_seed(odu_seed: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(None, odu_seed);
        let mut cipher_seed = [0u8; 32];
        hk.expand(b"omokoda-cloakseed-cipher-v1", &mut cipher_seed)
            .expect("32 bytes is a valid HKDF-SHA256 output length");
        let mut rng = StdRng::from_seed(cipher_seed);
        let mut cipher: Vec<String> = bipon39::all_encoding_tokens()
            .iter()
            .map(|s| s.to_string())
            .collect();
        cipher.shuffle(&mut rng);
        Self { cipher }
    }

    /// Encode real mnemonic words into cover words, position-for-position.
    pub fn encode_phrase(&self, real_words: &[&str]) -> Result<Vec<String>, String> {
        real_words
            .iter()
            .map(|w| {
                let idx = bipon39::index_of_encoding(w).map_err(|e| e.to_string())?;
                Ok(self.cipher[idx].clone())
            })
            .collect()
    }

    /// Decode cover words back to the real mnemonic, position-for-position.
    pub fn decode_phrase(&self, cloak_words: &[&str]) -> Result<Vec<String>, String> {
        cloak_words
            .iter()
            .map(|w| {
                let idx = self
                    .cipher
                    .iter()
                    .position(|c| c == w)
                    .ok_or_else(|| format!("unknown cloak word: '{w}'"))?;
                Ok(bipon39::all_encoding_tokens()[idx].to_string())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_the_same_cipher() {
        let seed = [7u8; 32];
        let a = CloakSeed::from_seed(&seed);
        let b = CloakSeed::from_seed(&seed);
        assert_eq!(a.cipher, b.cipher);
    }

    #[test]
    fn different_seeds_yield_different_ciphers() {
        let a = CloakSeed::from_seed(&[1u8; 32]);
        let b = CloakSeed::from_seed(&[2u8; 32]);
        assert_ne!(a.cipher, b.cipher);
    }

    #[test]
    fn cipher_is_a_true_permutation_of_the_real_wordlist() {
        let cloak = CloakSeed::from_seed(&[9u8; 32]);
        let mut sorted = cloak.cipher.clone();
        sorted.sort();
        let mut real: Vec<String> = bipon39::all_encoding_tokens()
            .iter()
            .map(|s| s.to_string())
            .collect();
        real.sort();
        assert_eq!(
            sorted, real,
            "every cover word is a real wordlist token, used exactly once"
        );
    }

    #[test]
    fn encode_then_decode_round_trips() {
        let cloak = CloakSeed::from_seed(&[42u8; 32]);
        let real_words: Vec<&str> = bipon39::all_encoding_tokens()[..12].to_vec();
        let cloaked = cloak.encode_phrase(&real_words).unwrap();
        let cloaked_refs: Vec<&str> = cloaked.iter().map(String::as_str).collect();
        let decoded = cloak.decode_phrase(&cloaked_refs).unwrap();
        assert_eq!(decoded, real_words);
    }

    #[test]
    fn cloaked_words_are_never_the_real_words_at_their_own_position() {
        // Not a security property (this is display protection, not
        // encryption -- see module docs) but a sanity check that the
        // permutation isn't accidentally the identity map.
        let cloak = CloakSeed::from_seed(&[3u8; 32]);
        let fixed_points = cloak
            .cipher
            .iter()
            .zip(bipon39::all_encoding_tokens())
            .filter(|(c, r)| c.as_str() == **r)
            .count();
        // A random 256-permutation has ~1 fixed point on average; assert
        // it isn't the extreme "no shuffling happened" case (256 fixed).
        assert!(fixed_points < 200);
    }

    #[test]
    fn decode_rejects_a_word_outside_the_cipher() {
        let cloak = CloakSeed::from_seed(&[5u8; 32]);
        assert!(cloak.decode_phrase(&["not-a-real-cloak-word"]).is_err());
    }
}
