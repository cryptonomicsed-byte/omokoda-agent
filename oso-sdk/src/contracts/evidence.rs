//! Phase 25.7 — EvidenceContract: Zàngbétò proofs, bundles, witness attestations.

use crate::contract::{CallResult, NativeContract};
use crate::error::{SdkError, SdkResult};
use serde_json::{json, Value};

pub struct EvidenceContract;

impl EvidenceContract {
    pub fn call(contract: &NativeContract, method: &str, args: Value) -> SdkResult<CallResult> {
        let output = match method {
            "submit_proof" => Self::submit_proof(&args)?,
            "verify_proof" => Self::verify_proof(&args)?,
            "bundle" => Self::bundle(&args)?,
            "attest" => Self::attest(&args)?,
            "get_attestations" => Self::get_attestations(&args)?,
            "challenge" => Self::challenge(&args)?,
            "resolve_challenge" => Self::resolve_challenge(&args)?,
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

    fn submit_proof(args: &Value) -> SdkResult<Value> {
        let proof_hash = args["proof_hash"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("proof_hash required".into()))?;
        let kind = args["kind"].as_str().unwrap_or("generic");
        let subject = args["subject"].as_str().unwrap_or("");
        Ok(json!({
            "proof_id": format!("proof:{proof_hash}"),
            "proof_hash": proof_hash,
            "kind": kind,
            "subject": subject,
            "nostr_kind": 31020,
            "status": "submitted"
        }))
    }

    fn verify_proof(args: &Value) -> SdkResult<Value> {
        let proof_id = args["proof_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("proof_id required".into()))?;
        let valid = args["valid"].as_bool().unwrap_or(true);
        Ok(json!({
            "proof_id": proof_id,
            "valid": valid,
            "status": if valid { "verified" } else { "invalid" }
        }))
    }

    fn bundle(args: &Value) -> SdkResult<Value> {
        let proofs = args["proof_ids"].as_array().cloned().unwrap_or_default();
        let bundle_id = format!("bundle:{}", proofs.len());
        Ok(json!({
            "bundle_id": bundle_id,
            "proof_count": proofs.len(),
            "proofs": proofs,
            "nostr_kind": 31030,
            "status": "bundled"
        }))
    }

    fn attest(args: &Value) -> SdkResult<Value> {
        let target = args["target"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("target required".into()))?;
        let attester = args["attester"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("attester required".into()))?;
        let claim = args["claim"].as_str().unwrap_or("witnessed");
        Ok(json!({
            "attestation_id": format!("att:{attester}:{target}"),
            "attester": attester,
            "target": target,
            "claim": claim,
            "nostr_kind": 31020,
            "status": "attested"
        }))
    }

    fn get_attestations(args: &Value) -> SdkResult<Value> {
        let target = args["target"].as_str().unwrap_or("");
        Ok(json!({ "target": target, "attestations": [] }))
    }

    fn challenge(args: &Value) -> SdkResult<Value> {
        let proof_id = args["proof_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("proof_id required".into()))?;
        let reason = args["reason"].as_str().unwrap_or("disputed");
        Ok(json!({
            "challenge_id": format!("chal:{proof_id}"),
            "proof_id": proof_id,
            "reason": reason,
            "status": "challenged"
        }))
    }

    fn resolve_challenge(args: &Value) -> SdkResult<Value> {
        let challenge_id = args["challenge_id"]
            .as_str()
            .ok_or(SdkError::InvalidArgs("challenge_id required".into()))?;
        let upheld = args["upheld"].as_bool().unwrap_or(false);
        Ok(json!({
            "challenge_id": challenge_id,
            "upheld": upheld,
            "status": if upheld { "proof_invalidated" } else { "challenge_rejected" }
        }))
    }
}
