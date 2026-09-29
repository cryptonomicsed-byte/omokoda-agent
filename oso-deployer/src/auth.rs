//! AuthorizationGate — tier-based deploy authorization.
//!
//! Tiers:
//!   0–1: guest / observer
//!   2–3: agent-level
//!   4:   trusted agent / operator
//!   5–6: governance participant
//!   7:   sovereign / root

use serde::{Deserialize, Serialize};

/// Deployment targets supported by the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployTarget {
    Local,
    Testnet,
    Mainnet,
}

impl DeployTarget {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "local"   => Some(Self::Local),
            "testnet" => Some(Self::Testnet),
            "mainnet" => Some(Self::Mainnet),
            _         => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local   => "local",
            Self::Testnet => "testnet",
            Self::Mainnet => "mainnet",
        }
    }

    /// Minimum signer tier required to deploy to this target.
    pub fn required_tier(&self) -> u8 {
        match self {
            Self::Local   => 0,
            Self::Testnet => 2,
            Self::Mainnet => 5,
        }
    }
}

pub struct AuthorizationGate;

impl AuthorizationGate {
    /// Check whether `signer_tier` is authorized for `target`.
    /// Returns `Ok(())` on success or `Err(required_tier)` on failure.
    pub fn check(target: DeployTarget, signer_tier: u8) -> Result<(), u8> {
        let required = target.required_tier();
        if signer_tier >= required {
            Ok(())
        } else {
            Err(required)
        }
    }

    /// Returns true if the target is mainnet (requires governance approval flow).
    pub fn requires_governance_approval(target: DeployTarget) -> bool {
        target == DeployTarget::Mainnet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_any_tier_authorized() {
        assert!(AuthorizationGate::check(DeployTarget::Local, 0).is_ok());
        assert!(AuthorizationGate::check(DeployTarget::Local, 7).is_ok());
    }

    #[test]
    fn testnet_tier_1_rejected() {
        assert!(AuthorizationGate::check(DeployTarget::Testnet, 1).is_err());
    }

    #[test]
    fn testnet_tier_2_passes() {
        assert!(AuthorizationGate::check(DeployTarget::Testnet, 2).is_ok());
    }

    #[test]
    fn mainnet_tier_4_rejected() {
        let result = AuthorizationGate::check(DeployTarget::Mainnet, 4);
        assert_eq!(result, Err(5));
    }

    #[test]
    fn mainnet_tier_5_passes() {
        assert!(AuthorizationGate::check(DeployTarget::Mainnet, 5).is_ok());
    }
}
