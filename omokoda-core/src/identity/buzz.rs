//! Buzz/Nostr sub-identity — a secp256k1 keypair derived from this agent's
//! own Odù seed, for participating in Buzz (block/buzz) as a first-class
//! member alongside humans.
//!
//! Buzz/Nostr identity is secp256k1 (NIP-01), not the Ed25519 the rest of
//! this kernel uses (BIPON39 identity, the Sui wallet). There is no way to
//! reuse the same keypair across the two curves, so this derives a
//! *separate*, domain-separated sub-identity from the same root seed --
//! the same "one root, many per-purpose keys" pattern already used for the
//! GlyphIndex keyring and the Sui wallet key. Nothing new needs separate
//! storage or leak-checking: whoever already holds the (sealed) Odù seed
//! can always re-derive this identity; nobody else ever can.

use hkdf::Hkdf;
use nostr::prelude::*;
use sha2::Sha256;

/// Derive this agent's Buzz/Nostr keypair from its own Odù seed.
/// Deterministic: the same seed always reproduces the same keypair.
pub fn derive_buzz_keys(odu_seed: &[u8]) -> Result<Keys, String> {
    let hk = Hkdf::<Sha256>::new(None, odu_seed);
    let mut secret = [0u8; 32];
    hk.expand(b"omokoda-buzz-nostr-identity-v1", &mut secret)
        .map_err(|e| format!("HKDF expand failed: {e}"))?;
    Keys::parse(&hex::encode(secret)).map_err(|e| format!("invalid derived nostr key: {e}"))
}

/// This agent's Buzz npub (bech32-encoded public key) -- safe to publish,
/// register with a Buzz relay, or display. Never exposes the secret half.
pub fn buzz_npub(odu_seed: &[u8]) -> Result<String, String> {
    let keys = derive_buzz_keys(odu_seed)?;
    keys.public_key()
        .to_bech32()
        .map_err(|e| format!("bech32 encoding failed: {e}"))
}

/// This agent's Buzz pubkey as raw hex (not bech32) -- needed by relay
/// operators for config (e.g. RELAY_OWNER_PUBKEY), which speak raw hex per
/// NIP-01, not npub. Never exposes the secret half.
pub fn buzz_pubkey_hex(odu_seed: &[u8]) -> Result<String, String> {
    let keys = derive_buzz_keys(odu_seed)?;
    Ok(keys.public_key().to_hex())
}

/// This agent's Buzz private key as raw hex -- for feeding into a relay
/// client (BUZZ_PRIVATE_KEY env var) to actually sign and publish events.
/// Never logged; caller is responsible for keeping it out of error paths.
pub fn buzz_privkey_hex(odu_seed: &[u8]) -> Result<String, String> {
    let keys = derive_buzz_keys(odu_seed)?;
    Ok(keys.secret_key().to_secret_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_the_same_keypair() {
        let seed = [7u8; 32];
        let a = derive_buzz_keys(&seed).unwrap();
        let b = derive_buzz_keys(&seed).unwrap();
        assert_eq!(a.secret_key().to_secret_bytes(), b.secret_key().to_secret_bytes());
        assert_eq!(a.public_key(), b.public_key());
    }

    #[test]
    fn different_seeds_yield_different_keypairs() {
        let a = derive_buzz_keys(&[1u8; 32]).unwrap();
        let b = derive_buzz_keys(&[2u8; 32]).unwrap();
        assert_ne!(a.public_key(), b.public_key());
    }

    #[test]
    fn npub_is_well_formed_bech32() {
        let npub = buzz_npub(&[9u8; 32]).unwrap();
        assert!(npub.starts_with("npub1"));
    }

    #[test]
    fn pubkey_hex_matches_the_bech32_pubkey() {
        let seed = [3u8; 32];
        let keys = derive_buzz_keys(&seed).unwrap();
        let hex_pk = buzz_pubkey_hex(&seed).unwrap();
        assert_eq!(hex_pk, keys.public_key().to_hex());
    }

    #[test]
    fn privkey_hex_round_trips_into_the_same_public_key() {
        let seed = [5u8; 32];
        let sk_hex = buzz_privkey_hex(&seed).unwrap();
        let reparsed = Keys::parse(&sk_hex).unwrap();
        assert_eq!(reparsed.public_key(), derive_buzz_keys(&seed).unwrap().public_key());
    }

    #[test]
    fn buzz_identity_is_independent_of_the_ed25519_identity() {
        // Sanity check that this really is a distinct curve/keyspace: the
        // derived secp256k1 secret bytes must not equal a naive Ed25519
        // derivation over the same seed with a different domain label (the
        // wallet/GlyphIndex convention) -- proves domain separation is
        // actually doing something, not a copy-paste no-op.
        let seed = [4u8; 32];
        let buzz = derive_buzz_keys(&seed).unwrap();
        let hk = Hkdf::<Sha256>::new(None, &seed);
        let mut other = [0u8; 32];
        hk.expand(b"omokoda-native-wallet-v1", &mut other).unwrap();
        assert_ne!(buzz.secret_key().to_secret_bytes().as_ref() as &[u8], other.as_ref() as &[u8]);
    }
}
