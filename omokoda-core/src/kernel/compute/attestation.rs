/// Cryptographic proof that a specific hardware device performed real work.
/// Produced by the device driver / TEE and verified by OSOVM before Dopamine credit.
#[derive(Debug, Clone)]
pub struct HardwareAttestation {
    pub attest_id: String,
    pub device_id: String,   // /devices/gpu/0001
    pub hardware_id: String, // PCIe UUID from driver
    pub agent_id: String,
    pub workload_hash: String,      // SHA-256 of workload descriptor
    pub tpm_quote: Option<String>,  // TPM 2.0 PCR quote (hex)
    pub tee_report: Option<String>, // SGX/TDX attestation report (hex)
    pub driver_sig: String,         // driver-signed nonce (hex)
    pub nonce: String,              // challenge nonce issued by OSOVM
    pub timestamp: u64,
    pub verified: bool,
}

impl HardwareAttestation {
    pub fn new(
        device_id: impl Into<String>,
        hardware_id: impl Into<String>,
        agent_id: impl Into<String>,
        workload_hash: impl Into<String>,
        nonce: impl Into<String>,
    ) -> Self {
        Self {
            attest_id: uuid_v4(),
            device_id: device_id.into(),
            hardware_id: hardware_id.into(),
            agent_id: agent_id.into(),
            workload_hash: workload_hash.into(),
            tpm_quote: None,
            tee_report: None,
            driver_sig: String::new(),
            nonce: nonce.into(),
            timestamp: now_secs(),
            verified: false,
        }
    }

    /// Mark verified after OSOVM confirmation
    pub fn mark_verified(&mut self) {
        self.verified = true;
    }
}

fn uuid_v4() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;
    let mut h = DefaultHasher::new();
    SystemTime::now().hash(&mut h);
    std::thread::current().id().hash(&mut h);
    format!(
        "{:016x}{:016x}",
        h.finish(),
        h.finish().wrapping_mul(0xcafe_babe)
    )
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
