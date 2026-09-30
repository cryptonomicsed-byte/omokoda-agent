//! Contract client — interface to the 6 native contract classes.
//!
//! Phase 25.3-25.8 will implement the concrete logic for each contract type.
//! This module defines the trait surface and dispatch enum.

use crate::error::{SdkError, SdkResult};
use serde::{Deserialize, Serialize};

/// The 6 native contract classes (from dApp layer spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractClass {
    /// AsePool, Payment, Escrow, Marketplace, Treasury.
    Financial,
    /// AgentRegistry, Hiring, Delegation, SkillRegistry, Reputation.
    Agent,
    /// JobContract (13-step lifecycle), WorkAgreement.
    Work,
    /// DeviceRegistry, DeviceManifest, SensorPolicy.
    Device,
    /// ZàngbétòProof, EvidenceBundle, WitnessAttestation.
    Evidence,
    /// Council, DAO, SectorGovernance.
    Governance,
}

/// A deployed native contract instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeContract {
    pub id: String,
    pub class: ContractClass,
    pub owner: String,
    pub metadata: serde_json::Value,
}

/// Contract call result — opaque, contract-type-specific.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResult {
    pub contract_id: String,
    pub method: String,
    pub output: serde_json::Value,
    /// ARP ActionReceipt-shaped JSON for this call.
    pub arp_payload: Option<serde_json::Value>,
}

/// Client for native contract interactions.
pub struct ContractClient {
    pub agent_id: String,
    contracts: std::collections::HashMap<String, NativeContract>,
}

impl ContractClient {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            contracts: Default::default(),
        }
    }

    pub fn register(&mut self, contract: NativeContract) {
        self.contracts.insert(contract.id.clone(), contract);
    }

    pub fn get(&self, id: &str) -> SdkResult<&NativeContract> {
        self.contracts
            .get(id)
            .ok_or_else(|| SdkError::ContractNotFound(id.into()))
    }

    /// Call a method on a registered contract.
    pub fn call(
        &self,
        contract_id: &str,
        method: &str,
        args: serde_json::Value,
    ) -> SdkResult<CallResult> {
        let contract = self.get(contract_id)?;
        crate::contracts::dispatch(contract, method, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut client = ContractClient::new("did:v:agent:1");
        client.register(NativeContract {
            id: "ase-pool-1".into(),
            class: ContractClass::Financial,
            owner: "did:v:agent:1".into(),
            metadata: serde_json::json!({}),
        });
        let c = client.get("ase-pool-1").unwrap();
        assert_eq!(c.class, ContractClass::Financial);
    }

    #[test]
    fn call_unknown_contract_errors() {
        let client = ContractClient::new("did:v:agent:1");
        let err = client.call("nonexistent", "transfer", serde_json::json!({}));
        assert!(matches!(err, Err(SdkError::ContractNotFound(_))));
    }
}
