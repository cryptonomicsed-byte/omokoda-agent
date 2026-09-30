//! Phase 25.8 — GovernanceContract: Council/DAO, 24-sector governance, proposals.

use crate::contract::{CallResult, NativeContract};
use crate::error::{SdkError, SdkResult};
use serde_json::{json, Value};

pub struct GovernanceContract;

/// The 24 sectors (shrine domains).
pub const SECTORS: &[&str] = &[
    "Simulation",
    "AgentIdentity",
    "EconomicPolicy",
    "Research",
    "Infrastructure",
    "Governance",
    "Hardware",
    "DePIN",
    "Education",
    "Security",
    "CrossChain",
    "Emergency",
    "Healthcare",
    "Energy",
    "Transportation",
    "Agriculture",
    "Finance",
    "Media",
    "Legal",
    "Environment",
    "Science",
    "Culture",
    "Housing",
    "Labor",
];

impl GovernanceContract {
    pub fn call(contract: &NativeContract, method: &str, args: Value) -> SdkResult<CallResult> {
        let output = match method {
            "create_proposal" => Self::create_proposal(&args)?,
            "vote" => Self::vote(&args)?,
            "finalize_proposal" => Self::finalize_proposal(&args)?,
            "join_sector" => Self::join_sector(&args)?,
            "leave_sector" => Self::leave_sector(&args)?,
            "shrine_split" => Self::shrine_split(&args)?,
            "get_proposal" => Self::get_proposal(&args)?,
            "list_proposals" => Self::list_proposals(&args)?,
            "council_seat" => Self::council_seat(&args)?,
            "get_sectors" => {
                Ok::<Value, SdkError>(json!({ "sectors": SECTORS, "count": SECTORS.len() }))?
            }
            _ => return Err(SdkError::MethodNotFound(method.into())),
        };
        let arp = super::arp_payload(
            contract,
            method,
            args["caller"].as_str().unwrap_or(""),
            &output,
        );
        Ok(CallResult {
            contract_id: contract.id.clone(),
            method: method.into(),
            output,
            arp_payload: Some(arp),
        })
    }

    fn create_proposal(args: &Value) -> SdkResult<Value> {
        let sector = args["sector"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("sector required".into()))?;
        if !SECTORS.contains(&sector) {
            return Err(SdkError::InvalidArgs(format!("unknown sector: {sector}")));
        }
        let title = args["title"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("title required".into()))?;
        let proposer = args["caller"].as_str().unwrap_or("");
        let proposal_id = format!("prop:{sector}:{title}");
        Ok(json!({
            "proposal_id": proposal_id,
            "sector": sector,
            "title": title,
            "proposer": proposer,
            "votes_for": 0,
            "votes_against": 0,
            "status": "open",
            "quorum": 7
        }))
    }

    fn vote(args: &Value) -> SdkResult<Value> {
        let proposal_id = args["proposal_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("proposal_id required".into()))?;
        let support = args["support"]
            .as_bool()
            .ok_or(SdkError::InvalidArgs("support required".into()))?;
        let voter = args["caller"].as_str().unwrap_or("");
        Ok(json!({
            "proposal_id": proposal_id,
            "voter": voter,
            "support": support,
            "status": "voted"
        }))
    }

    fn finalize_proposal(args: &Value) -> SdkResult<Value> {
        let proposal_id = args["proposal_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("proposal_id required".into()))?;
        let votes_for = args["votes_for"].as_u64().unwrap_or(0);
        let votes_against = args["votes_against"].as_u64().unwrap_or(0);
        let passed = votes_for > votes_against && votes_for >= 7; // quorum = 7 (Council of 12, simple majority)
        Ok(json!({
            "proposal_id": proposal_id,
            "votes_for": votes_for,
            "votes_against": votes_against,
            "passed": passed,
            "status": if passed { "enacted" } else { "rejected" }
        }))
    }

    fn join_sector(args: &Value) -> SdkResult<Value> {
        let sector = args["sector"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("sector required".into()))?;
        let agent_id = args["agent_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("agent_id required".into()))?;
        Ok(json!({ "sector": sector, "agent_id": agent_id, "status": "joined" }))
    }

    fn leave_sector(args: &Value) -> SdkResult<Value> {
        let sector = args["sector"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("sector required".into()))?;
        let agent_id = args["agent_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("agent_id required".into()))?;
        Ok(json!({ "sector": sector, "agent_id": agent_id, "status": "left" }))
    }

    /// @shrineSplit opcode: 50% shrine / 25% inheritance / 15% AIO / 10% burn
    fn shrine_split(args: &Value) -> SdkResult<Value> {
        let amount = args["amount"]
            .as_u64()
            .ok_or(SdkError::InvalidArgs("amount required".into()))?;
        let shrine = amount / 2; // 50%
        let inheritance = amount / 4; // 25%
        let aio = (amount as f64 * 0.15) as u64; // 15%
        let burn = amount.saturating_sub(shrine + inheritance + aio); // 10% (remainder)
        Ok(json!({
            "gross": amount,
            "shrine": shrine,
            "inheritance": inheritance,
            "aio": aio,
            "burn": burn,
            "opcode": "0x14",
            "split": "50/25/15/10"
        }))
    }

    fn get_proposal(args: &Value) -> SdkResult<Value> {
        let proposal_id = args["proposal_id"].as_str().unwrap_or("");
        Ok(json!({ "proposal_id": proposal_id, "found": false }))
    }

    fn list_proposals(args: &Value) -> SdkResult<Value> {
        let sector = args["sector"].as_str().unwrap_or("");
        Ok(json!({ "sector": sector, "proposals": [] }))
    }

    fn council_seat(args: &Value) -> SdkResult<Value> {
        let agent_id = args["agent_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("agent_id required".into()))?;
        let action = args["action"].as_str().unwrap_or("query"); // "claim" | "resign" | "query"
        Ok(json!({
            "agent_id": agent_id,
            "seat": null,
            "action": action,
            "council_size": 12
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::ContractClass;

    fn contract() -> NativeContract {
        NativeContract {
            id: "gov-1".into(),
            class: ContractClass::Governance,
            owner: "a".into(),
            metadata: json!({}),
        }
    }

    #[test]
    fn shrine_split_50_25_15_10() {
        let r = GovernanceContract::call(
            &contract(),
            "shrine_split",
            json!({"amount": 1000, "caller": "a"}),
        )
        .unwrap();
        assert_eq!(r.output["shrine"], 500u64);
        assert_eq!(r.output["inheritance"], 250u64);
        assert_eq!(r.output["aio"], 150u64);
        assert_eq!(r.output["burn"], 100u64);
    }

    #[test]
    fn proposal_quorum_7() {
        let r = GovernanceContract::call(
            &contract(),
            "finalize_proposal",
            json!({"proposal_id": "x", "votes_for": 7, "votes_against": 4, "caller": "a"}),
        )
        .unwrap();
        assert_eq!(r.output["passed"], true);

        let r2 = GovernanceContract::call(
            &contract(),
            "finalize_proposal",
            json!({"proposal_id": "x", "votes_for": 6, "votes_against": 4, "caller": "a"}),
        )
        .unwrap();
        assert_eq!(r2.output["passed"], false);
    }

    #[test]
    fn unknown_sector_rejected() {
        let err = GovernanceContract::call(
            &contract(),
            "create_proposal",
            json!({"sector": "FakeSector", "title": "t", "caller": "a"}),
        );
        assert!(matches!(err, Err(SdkError::InvalidArgs(_))));
    }

    #[test]
    fn sectors_count_24() {
        assert_eq!(SECTORS.len(), 24);
    }
}
