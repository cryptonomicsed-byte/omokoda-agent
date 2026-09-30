//! Phase 25.5 — WorkContract: JobContract full 13-step lifecycle.
//!
//! 13 steps: create → post → discover → apply → review → assign →
//!           acknowledge → execute → submit → verify → dispute? → settle → close

use crate::contract::{CallResult, NativeContract};
use crate::error::{SdkError, SdkResult};
use serde_json::{json, Value};

pub struct WorkContract;

/// The 13 lifecycle stages in order.
pub const JOB_LIFECYCLE: &[&str] = &[
    "create",
    "post",
    "discover",
    "apply",
    "review",
    "assign",
    "acknowledge",
    "execute",
    "submit",
    "verify",
    "dispute",
    "settle",
    "close",
];

impl WorkContract {
    pub fn call(contract: &NativeContract, method: &str, args: Value) -> SdkResult<CallResult> {
        let output = match method {
            // Lifecycle transitions
            "create" => Self::create(&args)?,
            "post" => Self::transition("posted", &args)?,
            "discover" => Self::transition("discovered", &args)?,
            "apply" => Self::apply(&args)?,
            "review" => Self::review(&args)?,
            "assign" => Self::assign(&args)?,
            "acknowledge" => Self::transition("acknowledged", &args)?,
            "execute" => Self::transition("in_progress", &args)?,
            "submit" => Self::submit(&args)?,
            "verify" => Self::verify(&args)?,
            "dispute" => Self::dispute(&args)?,
            "settle" => Self::settle(&args)?,
            "close" => Self::close(&args)?,
            // Queries
            "get_state" => Self::get_state(&args)?,
            "get_lifecycle" => Ok::<Value, SdkError>(json!({ "stages": JOB_LIFECYCLE }))?,
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

    fn create(args: &Value) -> SdkResult<Value> {
        let workload = args["workload"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("workload required".into()))?;
        let reward = args["reward"].as_u64().unwrap_or(0);
        let job_id = format!("job:{workload}:{}", reward);
        Ok(json!({
            "job_id": job_id,
            "workload": workload,
            "reward": reward,
            "state": "created",
            "lifecycle_step": 0
        }))
    }

    fn transition(state: &str, args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let step = JOB_LIFECYCLE
            .iter()
            .position(|&s| s == state.split_once('_').map(|x| x.0).unwrap_or(state));
        Ok(json!({
            "job_id": job_id,
            "state": state,
            "lifecycle_step": step
        }))
    }

    fn apply(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let applicant = args["applicant"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("applicant required".into()))?;
        Ok(json!({
            "job_id": job_id,
            "applicant": applicant,
            "application_id": format!("app:{job_id}:{applicant}"),
            "state": "applied",
            "lifecycle_step": 3
        }))
    }

    fn review(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let approved = args["approved"].as_bool().unwrap_or(false);
        Ok(json!({
            "job_id": job_id,
            "reviewed": true,
            "approved": approved,
            "state": if approved { "reviewed" } else { "rejected" },
            "lifecycle_step": 4
        }))
    }

    fn assign(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let worker = args["worker"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("worker required".into()))?;
        Ok(json!({
            "job_id": job_id,
            "worker": worker,
            "state": "assigned",
            "lifecycle_step": 5
        }))
    }

    fn submit(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let proof_hash = args["proof_hash"].as_str().unwrap_or("");
        Ok(json!({
            "job_id": job_id,
            "proof_hash": proof_hash,
            "state": "submitted",
            "lifecycle_step": 8
        }))
    }

    fn verify(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let f1_score = args["f1_score"].as_f64().unwrap_or(0.0);
        let passed = f1_score >= 0.777;
        Ok(json!({
            "job_id": job_id,
            "f1_score": f1_score,
            "passed": passed,
            "state": if passed { "verified" } else { "failed_verification" },
            "lifecycle_step": 9
        }))
    }

    fn dispute(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let reason = args["reason"].as_str().unwrap_or("unspecified");
        Ok(json!({
            "job_id": job_id,
            "dispute_id": format!("dis:{job_id}"),
            "reason": reason,
            "state": "disputed",
            "lifecycle_step": 10
        }))
    }

    fn settle(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        let worker_pct = args["worker_pct"].as_f64().unwrap_or(1.0).clamp(0.0, 1.0);
        Ok(json!({
            "job_id": job_id,
            "worker_pct": worker_pct,
            "employer_pct": 1.0 - worker_pct,
            "state": "settled",
            "lifecycle_step": 11
        }))
    }

    fn close(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("job_id required".into()))?;
        Ok(json!({
            "job_id": job_id,
            "state": "closed",
            "lifecycle_step": 12,
            "final": true
        }))
    }

    fn get_state(args: &Value) -> SdkResult<Value> {
        let job_id = args["job_id"].as_str().unwrap_or("");
        Ok(json!({ "job_id": job_id, "state": "unknown", "lifecycle_step": null }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::ContractClass;

    fn contract() -> NativeContract {
        NativeContract {
            id: "work-1".into(),
            class: ContractClass::Work,
            owner: "agent:1".into(),
            metadata: json!({}),
        }
    }

    #[test]
    fn full_happy_path() {
        let c = contract();
        let r = WorkContract::call(
            &c,
            "create",
            json!({"workload": "inference", "reward": 100, "caller": "a"}),
        )
        .unwrap();
        let job_id = r.output["job_id"].as_str().unwrap().to_string();

        let r2 = WorkContract::call(
            &c,
            "assign",
            json!({"job_id": job_id, "worker": "b", "caller": "a"}),
        )
        .unwrap();
        assert_eq!(r2.output["state"], "assigned");

        let r3 = WorkContract::call(
            &c,
            "verify",
            json!({"job_id": job_id, "f1_score": 0.9, "caller": "a"}),
        )
        .unwrap();
        assert_eq!(r3.output["passed"], true);
    }

    #[test]
    fn verify_fails_below_threshold() {
        let r = WorkContract::call(
            &contract(),
            "verify",
            json!({"job_id": "x", "f1_score": 0.5, "caller": "a"}),
        )
        .unwrap();
        assert_eq!(r.output["passed"], false);
        assert_eq!(r.output["state"], "failed_verification");
    }

    #[test]
    fn lifecycle_stages_count() {
        assert_eq!(JOB_LIFECYCLE.len(), 13);
    }
}
