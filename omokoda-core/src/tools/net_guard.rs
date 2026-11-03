//! Outbound request guard — the wiring for `security.ssrf_guard`.
//!
//! `omokoda_hermetic::network_guard::NetworkGuard` existed but had no call
//! sites, so the `security.ssrf_guard` config key (and the `ssrf_guard` toggle
//! the onboarding wizard offers) was a no-op: every tool could fetch any URL,
//! including the agent's own loopback API and cloud metadata endpoints.
//! `guard_url` is the single entry point tools must call before an outbound
//! request.

use omokoda_hermetic::network_guard::{NetworkGuard, NetworkGuardConfig, NetworkGuardDecision};
use std::sync::LazyLock;

/// `security.ssrf_guard` from `~/.omokoda/config.toml` (default true), with an
/// `OMOKODA_SSRF_GUARD` env override for tests and one-off runs.
static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    if let Ok(v) = std::env::var("OMOKODA_SSRF_GUARD") {
        return matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        );
    }
    let Some(home) = std::env::var_os("HOME") else {
        return true;
    };
    let path = std::path::Path::new(&home)
        .join(".omokoda")
        .join("config.toml");
    crate::config_toml::OmokodaConfig::load_or_default(&path)
        .security
        .ssrf_guard
});

static GUARD: LazyLock<NetworkGuard> = LazyLock::new(|| NetworkGuard::new(NetworkGuardConfig::default()));

/// Reject a URL that the SSRF guard blocks. Returns `Ok(())` when the guard is
/// disabled by config, or when the target is a public address.
pub fn guard_url(url: &str) -> Result<(), String> {
    if !*ENABLED {
        return Ok(());
    }
    match GUARD.check(url) {
        NetworkGuardDecision::Block(reason) => Err(reason),
        NetworkGuardDecision::Warn(reason) => {
            tracing::warn!(url = %url, reason = %reason, "network guard warning");
            Ok(())
        }
        NetworkGuardDecision::Allow => Ok(()),
    }
}

/// Whether the guard is active (exposed for tool descriptions and diagnostics).
pub fn enabled() -> bool {
    *ENABLED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_loopback_and_metadata() {
        std::env::set_var("OMOKODA_SSRF_GUARD", "1");
        assert!(guard_url("http://127.0.0.1:8001/v1/reveal-seed").is_err());
        assert!(guard_url("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(guard_url("https://example.com/").is_ok());
    }
}
