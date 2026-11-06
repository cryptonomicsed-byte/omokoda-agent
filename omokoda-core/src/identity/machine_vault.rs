//! Machine-held vault master secret -- the key that makes self-seal-at-birth
//! real. Generated once on this host, read only by this internal module,
//! NEVER exposed through any HTTP handler, CLI argument, log line, or
//! tool_output. Every agent's individual seal key is derived from it via
//! HKDF, domain-separated by agent id, so no two agents share a key and
//! compromising one derived key never reveals the master secret itself.
//!
//! Honest limit (already discussed and still true): this closes the
//! remote/API/log leak entirely -- no password for this path ever transits
//! any request, so nobody who only has network/API access, and nobody
//! reading the kernel's own logs, can ever recover it. It does NOT defeat
//! an operator with root on this same machine, who can read the master
//! secret file directly. That's a separate infrastructure-level question
//! (HSM/enclave/separate box), not something a software design can close.

use hkdf::Hkdf;
use sha2::Sha256;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;

fn master_secret_path() -> PathBuf {
    std::env::var("OMOKODA_VAULT_MASTER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/etc/ares-env/omokoda-vault-master.key"))
}

/// Read this host's vault master secret, generating it on first use if it
/// doesn't exist yet. Root-only permissions (0600), hex-encoded on disk.
fn machine_master_secret() -> Result<[u8; 32], String> {
    let path = master_secret_path();

    if let Ok(hex_str) = std::fs::read_to_string(&path) {
        let bytes = hex::decode(hex_str.trim())
            .map_err(|e| format!("vault master secret file is corrupt: {e}"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "vault master secret file has the wrong length".to_string())?;
        return Ok(arr);
    }

    let mut secret = [0u8; 32];
    rand::Rng::fill(&mut rand::thread_rng(), &mut secret);

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Use create_new + mode(0o600) atomically to avoid a write-then-chmod race window.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("failed to create vault master secret file: {e}"))?;
    file.write_all(hex::encode(secret).as_bytes())
        .map_err(|e| format!("failed to write vault master secret: {e}"))?;

    Ok(secret)
}

/// This agent's individual seal key -- HKDF over the machine master secret,
/// domain-separated by agent id. Deterministic: always re-derivable by this
/// host without any external input, never returned to any caller.
pub fn derive_agent_vault_key(agent_id: &str) -> Result<[u8; 32], String> {
    let master = machine_master_secret()?;
    let hk = Hkdf::<Sha256>::new(None, &master);
    let mut key = [0u8; 32];
    hk.expand(
        format!("omokoda-agent-vault-v1:{agent_id}").as_bytes(),
        &mut key,
    )
    .map_err(|e| format!("HKDF expand failed: {e}"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Single test: env::set_var is process-global, so two tests racing on
    // different paths could read each other's path mid-flight if run in
    // parallel threads (the default). One test, one path, no race.
    #[test]
    fn vault_key_derivation_is_deterministic_and_agent_specific() {
        // Use TMPDIR (respects Android/Termux) instead of hardcoded /tmp.
        let tmp = std::env::var("TMPDIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
        let path = format!("{}/omokoda-test-vault-master.key", tmp);
        let _ = std::fs::remove_file(&path); // clean up any stale file
        std::env::set_var("OMOKODA_VAULT_MASTER_PATH", &path);
        let a1 = derive_agent_vault_key("agent-1").unwrap();
        let a2 = derive_agent_vault_key("agent-1").unwrap();
        let b = derive_agent_vault_key("agent-2").unwrap();
        assert_eq!(a1, a2, "same agent id must yield the same key");
        assert_ne!(a1, b, "different agent ids must yield different keys");
        let _ = std::fs::remove_file(&path);
    }
}
