//! Phase 25.4 — AgentContract: Registry, Hiring, Delegation, SkillRegistry, Reputation.

use crate::contract::{CallResult, NativeContract};
use crate::error::{SdkError, SdkResult};
use serde_json::{json, Value};

pub struct AgentContract;

impl AgentContract {
    pub fn call(contract: &NativeContract, method: &str, args: Value) -> SdkResult<CallResult> {
        let output = match method {
            "register" => Self::register(&args)?,
            "lookup" => Self::lookup(&args)?,
            "hire" => Self::hire(&args)?,
            "terminate" => Self::terminate(&args)?,
            "delegate" => Self::delegate(&args)?,
            "revoke_delegation" => Self::revoke_delegation(&args)?,
            "register_skill" => Self::register_skill(&args)?,
            "lookup_skill" => Self::lookup_skill(&args)?,
            "rate" => Self::rate(&args)?,
            "get_reputation" => Self::get_reputation(&args)?,
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

    fn register(args: &Value) -> SdkResult<Value> {
        let agent_id = args["agent_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("agent_id required".into()))?;
        let npub = args["npub"].as_str().unwrap_or("");
        let tier = args["tier"].as_u64().unwrap_or(0);
        Ok(json!({
            "agent_id": agent_id,
            "npub": npub,
            "tier": tier,
            "status": "registered",
            "registry_id": format!("reg:{agent_id}")
        }))
    }

    fn lookup(args: &Value) -> SdkResult<Value> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        Ok(json!({ "agent_id": agent_id, "found": false, "tier": 0, "skills": [] }))
    }

    fn hire(args: &Value) -> SdkResult<Value> {
        let employer = args["employer"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("employer required".into()))?;
        let employee = args["employee"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("employee required".into()))?;
        let role = args["role"].as_str().unwrap_or("worker");
        Ok(json!({
            "hire_id": format!("hire:{employer}:{employee}"),
            "employer": employer,
            "employee": employee,
            "role": role,
            "status": "hired"
        }))
    }

    fn terminate(args: &Value) -> SdkResult<Value> {
        let hire_id = args["hire_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("hire_id required".into()))?;
        Ok(json!({ "hire_id": hire_id, "status": "terminated" }))
    }

    fn delegate(args: &Value) -> SdkResult<Value> {
        let from = args["from"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("from required".into()))?;
        let to = args["to"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("to required".into()))?;
        let capability = args["capability"].as_str().unwrap_or("*");
        Ok(json!({
            "delegation_id": format!("del:{from}:{to}"),
            "from": from,
            "to": to,
            "capability": capability,
            "status": "delegated"
        }))
    }

    fn revoke_delegation(args: &Value) -> SdkResult<Value> {
        let delegation_id = args["delegation_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("delegation_id required".into()))?;
        Ok(json!({ "delegation_id": delegation_id, "status": "revoked" }))
    }

    fn register_skill(args: &Value) -> SdkResult<Value> {
        let agent_id = args["agent_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("agent_id required".into()))?;
        let skill = args["skill"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("skill required".into()))?;
        let level = args["level"].as_u64().unwrap_or(1);
        Ok(json!({
            "skill_id": format!("skill:{agent_id}:{skill}"),
            "agent_id": agent_id,
            "skill": skill,
            "level": level,
            "status": "registered"
        }))
    }

    fn lookup_skill(args: &Value) -> SdkResult<Value> {
        let skill = args["skill"].as_str().unwrap_or("");
        let min_level = args["min_level"].as_u64().unwrap_or(1);
        Ok(json!({ "skill": skill, "min_level": min_level, "agents": [] }))
    }

    fn rate(args: &Value) -> SdkResult<Value> {
        let target = args["target"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("target required".into()))?;
        let score = args["score"]
            .as_f64()
            .ok_or(SdkError::InvalidArgs("score required".into()))?;
        if !(0.0..=1.0).contains(&score) {
            return Err(SdkError::InvalidArgs("score must be 0.0-1.0".into()));
        }
        Ok(json!({ "target": target, "score": score, "status": "rated" }))
    }

    fn get_reputation(args: &Value) -> SdkResult<Value> {
        let agent_id = args["agent_id"].as_str().unwrap_or("");
        Ok(json!({ "agent_id": agent_id, "reputation": 0.5, "ratings_count": 0 }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::ContractClass;

    fn contract() -> NativeContract {
        NativeContract {
            id: "agent-reg-1".into(),
            class: ContractClass::Agent,
            owner: "agent:1".into(),
            metadata: json!({}),
        }
    }

    #[test]
    fn register_returns_registry_id() {
        let res = AgentContract::call(
            &contract(),
            "register",
            json!({"agent_id": "abc", "npub": "npub1...", "tier": 2, "caller": "abc"}),
        )
        .unwrap();
        assert!(res.output["registry_id"]
            .as_str()
            .unwrap()
            .starts_with("reg:"));
    }

    #[test]
    fn rate_validates_score_range() {
        let err = AgentContract::call(
            &contract(),
            "rate",
            json!({"target": "x", "score": 1.5, "caller": "a"}),
        );
        assert!(matches!(err, Err(SdkError::InvalidArgs(_))));
    }

    #[test]
    fn delegate_and_revoke() {
        let res = AgentContract::call(
            &contract(),
            "delegate",
            json!({"from": "a", "to": "b", "capability": "COMPUTE", "caller": "a"}),
        )
        .unwrap();
        let del_id = res.output["delegation_id"].as_str().unwrap().to_string();
        let rev = AgentContract::call(
            &contract(),
            "revoke_delegation",
            json!({"delegation_id": del_id, "caller": "a"}),
        )
        .unwrap();
        assert_eq!(rev.output["status"], "revoked");
    }
}
