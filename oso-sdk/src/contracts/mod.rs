//! Phase 25.3-25.8 — Native contract class implementations.
//! Each module implements the concrete method dispatch for one contract class.

pub mod agent;
pub mod device;
pub mod evidence;
pub mod financial;
pub mod governance;
pub mod work;

pub use agent::AgentContract;
pub use device::DeviceContract;
pub use evidence::EvidenceContract;
pub use financial::FinancialContract;
pub use governance::GovernanceContract;
pub use work::WorkContract;

use crate::contract::{CallResult, ContractClass, NativeContract};
use crate::error::SdkResult;

/// Dispatch a method call to the correct contract class implementation.
pub fn dispatch(
    contract: &NativeContract,
    method: &str,
    args: serde_json::Value,
) -> SdkResult<CallResult> {
    match contract.class {
        ContractClass::Financial => financial::FinancialContract::call(contract, method, args),
        ContractClass::Agent => agent::AgentContract::call(contract, method, args),
        ContractClass::Work => work::WorkContract::call(contract, method, args),
        ContractClass::Device => device::DeviceContract::call(contract, method, args),
        ContractClass::Evidence => evidence::EvidenceContract::call(contract, method, args),
        ContractClass::Governance => governance::GovernanceContract::call(contract, method, args),
    }
}

/// Build an ARP-shaped payload for a contract call.
pub fn arp_payload(
    contract: &NativeContract,
    method: &str,
    caller: &str,
    output: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "principal": caller,
        "capability": format!("{:?}:{}", contract.class, method),
        "action": {
            "contract_id": contract.id,
            "method": method,
            "class": format!("{:?}", contract.class)
        },
        "evidence": output,
        "receipt": {
            "contract_id": contract.id,
            "method": method,
            "ts": 0
        }
    })
}
