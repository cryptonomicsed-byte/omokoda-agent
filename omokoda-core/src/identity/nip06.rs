//! NIP-06 secp256k1 identity, derived from the same BIPON39 birth seed as
//! the kernel's native Ed25519 identity -- a second, related key, not a
//! second unrelated secret.
//!
//! Wired per the cross-pillar minipae/NIP-AE negotiation (plan-v3 §1.6):
//! BIPON39's `DerivationMode::{Native, Bip32}` gives two independent
//! `(master_key, chain_code)` pairs off one seed via
//! `HMAC-SHA512(key, seed)` with a different key string per branch --
//! `Native` (unchanged, backs the kernel's existing Ed25519 birth
//! identity) and `Bip32` (this module, backs the secp256k1/Nostr identity
//! minipae's engrams use). One seed of truth, one backup/rotation event
//! covers both.
//!
//! `derive_child_key`'s BIP-32 compliance (both the compressed-pubkey
//! non-hardened branch and the `(IL + kpar) mod n` step) was verified
//! against official BIP-32 test vectors and an independent library
//! differential check before this module was written -- see BIPON39
//! commits f51aaab/70173aa/1c4d5c9.
//!
//! Standard NIP-06 path: `m/44'/1237'/<account>'/0/0`. This kernel only
//! ever derives account 0 -- one Nostr identity per agent, matching the
//! one-Ed25519-identity-per-agent model the kernel already has. Nothing
//! here changes birth, identity persistence, or the existing Ed25519
//! flow; this is a purely additive derived identity.

use bipon39::derivation::{derive_path, DerivationMode, MasterKey};
use bipon39::mnemonic_to_seed as bipon39_mnemonic_to_seed;
use bipon39::split_mnemonic;
use nostr::key::{Keys, SecretKey};
use nostr::nips::nip19::ToBech32;

/// NIP-06 hardened path segments: purpose(44') / coin_type(1237') / account(N').
/// change(0) and address_index(0) are non-hardened, appended per call.
const NIP06_PURPOSE: u32 = 44 | 0x8000_0000;
const NIP06_COIN_TYPE: u32 = 1237 | 0x8000_0000;

/// The derived Nostr identity: raw key material plus its bech32 forms.
/// `secret_key_hex`/`nsec` are sensitive -- callers must treat them with
/// the same care as any other private key material in this kernel (never
/// logged, never returned over an unauthenticated API).
pub struct Nip06Identity {
    pub secret_key_hex: String,
    pub public_key_hex: String,
    pub npub: String,
    pub nsec: String,
}

/// Derive this agent's NIP-06 secp256k1/Nostr identity from its birth
/// mnemonic, at account index `account`. Deterministic: the same mnemonic
/// and account always produce the same identity, exactly like the
/// existing Ed25519 derivation.
///
/// Uses the FULL 64-byte BIPON39 seed (PBKDF2-HMAC-SHA512 output), not
/// the truncated 32-byte value `identity::bipon39::Bipon39::mnemonic_to_seed`
/// produces for the native Ed25519 branch -- BIP-32 master derivation
/// requires the full 64 bytes.
pub fn derive_nip06_identity(mnemonic: &str, account: u32) -> Result<Nip06Identity, String> {
    let words = split_mnemonic(mnemonic);
    let seed = bipon39_mnemonic_to_seed(&words, "")
        .map_err(|e| format!("BIPON39 seed derivation failed: {e}"))?;

    // Confirms the seed is usable for BIP-32 master derivation before
    // walking the path (fail fast with a clear error otherwise).
    let MasterKey { .. } = bipon39::derivation::master_from_seed(&seed, DerivationMode::Bip32)
        .map_err(|e| format!("BIP-32 master key derivation failed: {e}"))?;

    let path = [
        NIP06_PURPOSE,
        NIP06_COIN_TYPE,
        account | 0x8000_0000,
        0, // change
        0, // address_index
    ];

    let (derived_key, _derived_chain) =
        derive_path(&seed, &path).map_err(|e| format!("NIP-06 path derivation failed: {e}"))?;

    let secret_key = SecretKey::from_slice(&derived_key)
        .map_err(|e| format!("invalid secp256k1 secret key: {e}"))?;
    let secret_key_hex = hex::encode(derived_key);
    let keys = Keys::new(secret_key);
    let public_key_hex = keys.public_key().to_hex();
    let npub = keys
        .public_key()
        .to_bech32()
        .map_err(|e| format!("npub bech32 encoding failed: {e}"))?;
    let nsec = keys
        .secret_key()
        .to_bech32()
        .map_err(|e| format!("nsec bech32 encoding failed: {e}"))?;

    Ok(Nip06Identity {
        secret_key_hex,
        public_key_hex,
        npub,
        nsec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic_for_the_same_mnemonic_and_account() {
        let mnemonic = bipon39::entropy_to_mnemonic(&[7u8; 32])
            .expect("test entropy should produce a valid mnemonic")
            .join(" ");
        let a = derive_nip06_identity(&mnemonic, 0).unwrap();
        let b = derive_nip06_identity(&mnemonic, 0).unwrap();
        assert_eq!(a.secret_key_hex, b.secret_key_hex);
        assert_eq!(a.public_key_hex, b.public_key_hex);
        assert_eq!(a.npub, b.npub);
        assert_eq!(a.nsec, b.nsec);
    }

    #[test]
    fn different_accounts_produce_different_identities() {
        let mnemonic = bipon39::entropy_to_mnemonic(&[7u8; 32])
            .expect("test entropy should produce a valid mnemonic")
            .join(" ");
        let a = derive_nip06_identity(&mnemonic, 0).unwrap();
        let b = derive_nip06_identity(&mnemonic, 1).unwrap();
        assert_ne!(a.secret_key_hex, b.secret_key_hex);
        assert_ne!(a.npub, b.npub);
    }

    #[test]
    fn different_mnemonics_produce_different_identities() {
        let m1 = bipon39::entropy_to_mnemonic(&[1u8; 32]).unwrap().join(" ");
        let m2 = bipon39::entropy_to_mnemonic(&[2u8; 32]).unwrap().join(" ");
        let a = derive_nip06_identity(&m1, 0).unwrap();
        let b = derive_nip06_identity(&m2, 0).unwrap();
        assert_ne!(a.secret_key_hex, b.secret_key_hex);
    }

    #[test]
    fn npub_and_nsec_have_correct_bech32_prefixes() {
        let mnemonic = bipon39::entropy_to_mnemonic(&[3u8; 32]).unwrap().join(" ");
        let identity = derive_nip06_identity(&mnemonic, 0).unwrap();
        assert!(
            identity.npub.starts_with("npub1"),
            "npub: {}",
            identity.npub
        );
        assert!(
            identity.nsec.starts_with("nsec1"),
            "nsec: {}",
            identity.nsec
        );
    }

    #[test]
    fn public_key_hex_is_32_bytes_x_only() {
        let mnemonic = bipon39::entropy_to_mnemonic(&[4u8; 32]).unwrap().join(" ");
        let identity = derive_nip06_identity(&mnemonic, 0).unwrap();
        let decoded = hex::decode(&identity.public_key_hex).unwrap();
        assert_eq!(decoded.len(), 32, "NIP-01 x-only pubkeys are 32 bytes");
    }
}
