pub mod ase;
pub mod bipon39;
pub mod buzz;
pub mod buzz_relay;
pub mod cloak;
pub mod credential_index;
pub mod dna;
pub mod duress;
pub mod fork;
pub mod git_sign;
pub mod hive;
pub mod machine_vault;
pub mod merkle;
pub mod nip06;
pub mod oauth;
pub mod odu;
pub mod pet;
pub mod poison_radar;
pub mod safety;
pub mod user;
pub mod vault;
pub mod wallet;
pub mod x402;

pub use credential_index::{CredentialEntry, CredentialIndex, CredentialKind};
pub use fork::{derive_fork_entropy, fork_agent, fork_index_hmac_key, ForkResult};
pub use vault::{CapabilityToken, IdentityVault, SealVault};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(dna_fingerprint: &str) -> Self {
        Self(format!("agent-{}", &dna_fingerprint[..16]))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self(s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for AgentId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
