#![allow(clippy::double_must_use)]
use super::receipt::*;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenesisError {
    #[error("BIPỌ̀N39 genesis failed: {0}")]
    Bipon(String),
    #[error("Koodu time provider failed: {0}")]
    KooduTime(String),
    #[error("Soul cast failed: {0}")]
    Soul(String),
    #[error("Memory initialization failed: {0}")]
    Memory(String),
    #[error("Network announcement failed: {0}")]
    Network(String),
    #[error("Device binding failed: {0}")]
    Device(String),
    #[error("Witness failed: {0}")]
    Witness(String),
    #[error("Ceremony aborted: required provider failed: {0}")]
    Required(String),
}

/// Generates the agent's sovereign genesis root from entropy.
/// The master seed produced here is the one root all other identities derive from.
#[async_trait]
pub trait BiponProvider: Send + Sync {
    async fn genesis(&self, request: &GenesisRequest) -> Result<BiponProof, GenesisError>;
}

/// Returns the agent's cryptographically-anchored birth position in time.
/// Uses Bitcoin block height if available; falls back to Gregorian.
#[async_trait]
pub trait KooduProvider: Send + Sync {
    async fn birth_time(&self) -> Result<KooduTimeProof, GenesisError>;
}

/// Casts the agent's Odù soul from entropy + Koodu temporal state.
/// This is the semantic genome: primary Odù, temperament, destiny threads.
#[async_trait]
pub trait SoulProvider: Send + Sync {
    async fn cast(&self, entropy: &[u8], koodu: &KooduTimeProof)
        -> Result<SoulProof, GenesisError>;
}

/// Initializes the agent's sovereign memory namespace (Minipae identity + birth glyph).
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    async fn initialize(
        &self,
        agent_id: &str,
        seed: &[u8],
        genesis_fact: &str,
    ) -> Result<MemoryProof, GenesisError>;
}

/// Publishes the agent's IP Root to the network (Nostr kind 31900). Fail-open.
#[async_trait]
pub trait NetworkProvider: Send + Sync {
    async fn announce(
        &self,
        agent_id: &str,
        mnemonic: &str,
        name: &str,
    ) -> Result<NetworkProof, GenesisError>;
}

/// Optionally binds a physical device (Agent-Phone) to the newborn agent.
#[async_trait]
pub trait DeviceProvider: Send + Sync {
    async fn bind(
        &self,
        agent_id: &str,
        device_id: Option<&str>,
        device_kind: Option<&str>,
    ) -> Result<Option<DeviceBinding>, GenesisError>;
}
