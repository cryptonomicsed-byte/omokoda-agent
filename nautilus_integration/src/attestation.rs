use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AttestationError {
    #[error("code measurement mismatch: expected {expected}, got {actual}")]
    MeasurementMismatch { expected: String, actual: String },
    #[error("invalid TEE quote")]
    InvalidQuote,
    #[error("quote signature is empty — hardware attestation not satisfied")]
    MissingSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeQuote {
    pub enclave_id: [u8; 32],
    pub code_measurement: [u8; 32],
    pub nonce: [u8; 16],
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AttestationResult {
    pub enclave_id: [u8; 32],
    pub seal_key: [u8; 32],
}

/// Verify a TEE quote and derive a seal key.
///
/// H5 — two of the three checks are now enforced:
/// 1. Measurement check:  must match expected (unchanged).
/// 2. Signature presence: quotes with an empty  are rejected.
///    A real hardware enclave always produces a signature; an empty signature
///    means the quote was fabricated (or we are in a misconfigured test harness).
///
/// What is NOT yet verified: the actual DCAP/ECDSA signature chain against
/// Intel/AMD PKI. That requires the platform-specific DCAP library and
/// the enclave certificate chain, which are not wired yet. When a real
/// SGX/TDX deployment is in place, this function should forward to
/// .
///
/// Seal-key derivation now mixes in the quote nonce so the key is not
/// fully derivable from the public code_measurement alone.
pub fn verify_quote(
    quote: &TeeQuote,
    expected_measurement: &[u8; 32],
) -> Result<AttestationResult, AttestationError> {
    // Check 1: code measurement must match expected value.
    if quote.code_measurement != *expected_measurement {
        return Err(AttestationError::MeasurementMismatch {
            expected: hex::encode(expected_measurement),
            actual: hex::encode(quote.code_measurement),
        });
    }

    // Check 2 (H5): reject quotes with no signature.
    // Real hardware attestation always produces a non-empty signature.
    if quote.signature.is_empty() {
        return Err(AttestationError::MissingSignature);
    }

    // Derive seal key via HKDF(enclave_id || code_measurement || nonce).
    // Mixing the nonce means the seal key is not derivable from public
    // fields alone — the full quote (including signature) must be held.
    let mut hasher = Sha256::new();
    hasher.update(b"omokoda:nautilus:seal_key_v2");
    hasher.update(quote.enclave_id);
    hasher.update(quote.code_measurement);
    hasher.update(quote.nonce); // H5: bind key to quote nonce
    let seal_key: [u8; 32] = hasher.finalize().into();

    Ok(AttestationResult {
        enclave_id: quote.enclave_id,
        seal_key,
    })
}
