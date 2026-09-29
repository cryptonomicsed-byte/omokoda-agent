/// Fork derivation — Phase 13.1
///
/// Derives child-agent entropy deterministically from a parent k_root and a
/// fork index.  The derived entropy is passed to BIPON39 to produce a child
/// mnemonic; the raw bytes are never stored here.
///
/// Derivation: HMAC-SHA256(parent_k_root, "fork:v1:" || fork_index_be_bytes)
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Result of a fork derivation.
///
/// `child_entropy` is the raw 32-byte seed.  Callers must pass it to
/// `BIPON39::from_entropy(child_entropy)` to obtain the child mnemonic, then
/// zeroize the entropy.  Do NOT persist `child_entropy` directly.
#[derive(Debug)]
pub struct ForkResult {
    pub fork_index: u32,
    /// 32 bytes of derived child entropy — pass to BIPON39, then zeroize.
    pub child_entropy: [u8; 32],
    pub parent_agent_id: String,
    pub fork_timestamp: u64,
}

/// Derive child-agent entropy from `parent_k_root` and `fork_index`.
///
/// Deterministic: same parent + same index always → same 32-byte output.
/// Collision-resistant: different indices → different outputs (HMAC PRF).
pub fn derive_fork_entropy(parent_k_root: &[u8], fork_index: u32) -> [u8; 32] {
    let hmac_key = fork_index_hmac_key(fork_index);
    let mut mac =
        Hmac::<Sha256>::new_from_slice(parent_k_root).expect("HMAC accepts any key length");
    mac.update(&hmac_key);
    mac.finalize().into_bytes().into()
}

/// Build the HMAC message for a given fork index.
///
/// Format: b"fork:v1:" || fork_index.to_be_bytes()
pub fn fork_index_hmac_key(fork_index: u32) -> Vec<u8> {
    let mut key = b"fork:v1:".to_vec();
    key.extend_from_slice(&fork_index.to_be_bytes());
    key
}

/// Convenience: derive and return a full `ForkResult` for the given parent.
///
/// `parent_agent_id` — agent id string of the forking parent (for receipt).
/// `fork_index`      — monotonically increasing fork counter (0, 1, 2 …).
/// `fork_timestamp`  — Unix seconds at the moment of the fork request.
pub fn fork_agent(
    parent_k_root: &[u8],
    parent_agent_id: &str,
    fork_index: u32,
    fork_timestamp: u64,
) -> ForkResult {
    ForkResult {
        fork_index,
        child_entropy: derive_fork_entropy(parent_k_root, fork_index),
        parent_agent_id: parent_agent_id.to_string(),
        fork_timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARENT_ROOT: &[u8] = b"test_parent_k_root_32_bytes_here";
    const PARENT_ROOT_2: &[u8] = b"different_parent_k_root_32_bytes";

    #[test]
    fn same_parent_same_index_deterministic() {
        let e1 = derive_fork_entropy(PARENT_ROOT, 0);
        let e2 = derive_fork_entropy(PARENT_ROOT, 0);
        assert_eq!(e1, e2, "fork entropy must be deterministic");
    }

    #[test]
    fn different_indices_different_entropy() {
        let e0 = derive_fork_entropy(PARENT_ROOT, 0);
        let e1 = derive_fork_entropy(PARENT_ROOT, 1);
        let e2 = derive_fork_entropy(PARENT_ROOT, 2);
        assert_ne!(e0, e1, "fork index 0 and 1 must differ");
        assert_ne!(e1, e2, "fork index 1 and 2 must differ");
        assert_ne!(e0, e2, "fork index 0 and 2 must differ");
    }

    #[test]
    fn different_parents_different_entropy() {
        let e_a = derive_fork_entropy(PARENT_ROOT, 0);
        let e_b = derive_fork_entropy(PARENT_ROOT_2, 0);
        assert_ne!(e_a, e_b, "different parents must produce different entropy");
    }

    #[test]
    fn entropy_is_32_bytes() {
        let e = derive_fork_entropy(PARENT_ROOT, 42);
        assert_eq!(e.len(), 32);
    }

    #[test]
    fn fork_index_hmac_key_includes_index_bytes() {
        let k0 = fork_index_hmac_key(0);
        let k1 = fork_index_hmac_key(1);
        // Must include the "fork:v1:" prefix
        assert!(k0.starts_with(b"fork:v1:"));
        assert!(k1.starts_with(b"fork:v1:"));
        // Keys must differ for different indices
        assert_ne!(k0, k1);
    }

    #[test]
    fn fork_agent_convenience_wrapper() {
        let result = fork_agent(PARENT_ROOT, "agent-test123", 5, 1_700_000_000);
        assert_eq!(result.fork_index, 5);
        assert_eq!(result.parent_agent_id, "agent-test123");
        assert_eq!(result.fork_timestamp, 1_700_000_000);
        // Entropy must match the standalone derive function
        let expected = derive_fork_entropy(PARENT_ROOT, 5);
        assert_eq!(result.child_entropy, expected);
    }
}
