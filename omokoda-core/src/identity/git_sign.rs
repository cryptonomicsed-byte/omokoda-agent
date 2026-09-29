//! Git-signing sub-identity -- a secp256k1/BIP340 keypair, domain-separated
//! from the Buzz/Nostr chat identity, used to sign real Gitea commits via
//! git-sign-nostr (NIP-GS) as an accountability layer on top of the
//! existing Gitea/SkillForge pipeline.
//!
//! Hybrid design (2026-07-29): Gitea remains the primary CI/build/push
//! pipeline, untouched. This adds a second, independent signature on every
//! commit proving *which agent* authored it -- separate from Gitea's own
//! push auth (a shared/operator credential today), so multiple agents
//! collaborating on one repo each carry cryptographic, per-agent
//! provenance regardless of what Gitea credential actually pushed the ref.
//!
//! Deliberately a DIFFERENT sub-identity from `/buzz` rather than reusing
//! it: git-sign-nostr does accept BUZZ_PRIVATE_KEY as a fallback, but a
//! compromised git-signing key (used non-interactively by CI on a build
//! box) should never be able to impersonate this agent in Buzz chat, and
//! vice versa -- the same "one root, many per-purpose keys" discipline
//! already applied to the wallet and Buzz identities.

use hkdf::Hkdf;
use nostr::prelude::*;
use sha2::Sha256;

/// Derive this agent's git-signing keypair from its own Odù seed.
/// Deterministic: the same seed always reproduces the same keypair.
pub fn derive_git_sign_keys(odu_seed: &[u8]) -> Result<Keys, String> {
    let hk = Hkdf::<Sha256>::new(None, odu_seed);
    let mut secret = [0u8; 32];
    hk.expand(b"omokoda-git-sign-nostr-v1", &mut secret)
        .map_err(|e| format!("HKDF expand failed: {e}"))?;
    Keys::parse(&hex::encode(secret)).map_err(|e| format!("invalid derived nostr key: {e}"))
}

/// This agent's git-signing pubkey as raw hex -- the value for
/// `git config user.signingkey` and for anyone verifying her commits.
/// Never exposes the secret half.
pub fn git_sign_pubkey_hex(odu_seed: &[u8]) -> Result<String, String> {
    let keys = derive_git_sign_keys(odu_seed)?;
    Ok(keys.public_key().to_hex())
}

/// This agent's git-signing secret key as raw hex -- the value for
/// `NOSTR_PRIVATE_KEY` when actually signing commits via git-sign-nostr.
pub fn git_sign_privkey_hex(odu_seed: &[u8]) -> Result<String, String> {
    let keys = derive_git_sign_keys(odu_seed)?;
    Ok(keys.secret_key().to_secret_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_the_same_keypair() {
        let seed = [11u8; 32];
        let a = derive_git_sign_keys(&seed).unwrap();
        let b = derive_git_sign_keys(&seed).unwrap();
        assert_eq!(
            a.secret_key().to_secret_bytes(),
            b.secret_key().to_secret_bytes()
        );
    }

    #[test]
    fn different_seeds_yield_different_keypairs() {
        let a = derive_git_sign_keys(&[1u8; 32]).unwrap();
        let b = derive_git_sign_keys(&[2u8; 32]).unwrap();
        assert_ne!(a.public_key(), b.public_key());
    }

    #[test]
    fn git_sign_identity_is_independent_of_buzz_identity() {
        // Same domain-separation sanity check pattern as buzz.rs -- proves
        // this is really a distinct sub-identity, not an accidental alias
        // of the Buzz chat key derived from the same seed.
        let seed = [4u8; 32];
        let git_sign = derive_git_sign_keys(&seed).unwrap();
        let buzz = crate::identity::buzz::derive_buzz_keys(&seed).unwrap();
        assert_ne!(git_sign.public_key(), buzz.public_key());
    }

    #[test]
    fn pubkey_hex_matches_the_keypair() {
        let seed = [7u8; 32];
        let keys = derive_git_sign_keys(&seed).unwrap();
        assert_eq!(
            git_sign_pubkey_hex(&seed).unwrap(),
            keys.public_key().to_hex()
        );
    }

    #[test]
    fn privkey_hex_round_trips_into_the_same_public_key() {
        let seed = [9u8; 32];
        let sk_hex = git_sign_privkey_hex(&seed).unwrap();
        let reparsed = Keys::parse(&sk_hex).unwrap();
        assert_eq!(
            reparsed.public_key(),
            derive_git_sign_keys(&seed).unwrap().public_key()
        );
    }
}
