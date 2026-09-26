//! osovm_tool.rs — MCP bridge to the OSOVM HTTP server at :7780.
//!
//! Config: OSOVM_URL (default "http://localhost:7780")
//!
//! Exposed tools:
//!   osovm_run      — execute an OSOVM opcode
//!   osovm_veilsim  — run a VeilSim scenario
//!   osovm_health   — check OSOVM server health
//!
//! Helper (not a tool):
//!   attest_receipt — fire-and-forget POST /run with opcode=RECEIPT; used by
//!                    the interpreter to anchor ActReceipts in OSOVM for
//!                    tier-3+ actions. Returns None if OSOVM is absent.
//!                    NOTE: uses /run not /attest — no /attest route exists in
//!                    OSOVM server.jl (confirmed by Hermes audit 2026-09-26).

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

fn osovm_url() -> String {
    std::env::var("OSOVM_URL")
        .unwrap_or_else(|_| "http://localhost:7780".to_string())
}

async fn osovm_post(path: &str, body: Value) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!("{}{}", osovm_url(), path);
    let resp = client
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("OSOVM unreachable at {url}: {e}"))?;
    let val: Value = resp
        .json()
        .await
        .map_err(|e| format!("OSOVM response parse error: {e}"))?;
    if val.get("status").and_then(|v| v.as_str()) == Some("error") {
        return Err(format!("OSOVM error: {}", val.get("error").unwrap_or(&json!("unknown"))));
    }
    Ok(serde_json::to_string_pretty(&val).unwrap_or_default())
}

// ── attest_receipt ────────────────────────────────────────────────────────────

/// Submit an ActReceipt to OSOVM for attestation.
///
/// Called fire-and-forget from the interpreter after tier-3+ actions so every
/// high-consequence receipt is anchored in the OSOVM simulation fabric.
/// Returns the attestation ID on success; returns None if OSOVM is absent or
/// OSOVM_URL is not configured — fail-open by design.
///
/// `merkle_root` must be the 64-hex-char SHA-256 of the receipt bytes.
/// op_receipt (vm_core.jl:281) sets `verified = length(hash_val) >= 64`;
/// passing a short or empty hash produces verified=false at the callee.
/// The response key is `result.receipt` (inner dict from op_receipt), not
/// the top-level `receipt` key (which /run does not emit).
pub async fn attest_receipt(
    receipt_id: &str,
    agent_id: &str,
    tool_name: &str,
    gate_alignment: f64,
    merkle_root: &str,
) -> Option<String> {
    if std::env::var("OSOVM_URL").is_err() {
        return None;
    }
    // Use the existing /run endpoint with opcode=RECEIPT.
    // Pass `hash` = merkle_root so op_receipt's verified flag is true.
    let body = json!({
        "opcode": "RECEIPT",
        "args": {
            "receipt_id":    receipt_id,
            "tool_name":     tool_name,
            "gate_alignment": gate_alignment,
            "hash":          merkle_root,
            "attested_at":   std::time::SystemTime::now()
                                 .duration_since(std::time::UNIX_EPOCH)
                                 .unwrap_or_default()
                                 .as_secs(),
        },
        "agent": agent_id,
    });
    match osovm_post("/run", body).await {
        Ok(resp) => {
            let val: Value = serde_json::from_str(&resp).unwrap_or_default();
            // /run wraps op_receipt output in "result"; inner key is "receipt".
            val.get("result")
                .and_then(|r| r.get("receipt"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or(Some(receipt_id.to_string()))
        }
        Err(e) => {
            tracing::debug!(receipt_id, tool_name, error = %e, "OSOVM attest: skipped (service absent)");
            None
        }
    }
}

// ── osovm_run ─────────────────────────────────────────────────────────────────

pub struct OsovmRunTool;

#[async_trait]
impl Tool for OsovmRunTool {
    fn name(&self) -> &str { "osovm_run" }
    fn description(&self) -> &str {
        "Execute an OSOVM opcode (e.g. IMPACT, RECEIPT, VEIL, TRANSFER). \
         Returns f1_score, ase_minted, receipts, and vm_state_hash."
    }
    fn required_tier(&self) -> u8 { 3 }
    fn is_write_operation(&self) -> bool { true }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "opcode": { "type": "string", "description": "OSOVM opcode name e.g. IMPACT, RECEIPT, VEIL" },
                "args":   { "type": "object", "description": "Opcode arguments" },
                "agent":  { "type": "string", "description": "Agent DID or pubkey" }
            },
            "required": ["opcode"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let mut args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        if args.get("agent").is_none() {
            args["agent"] = json!(ctx.agent_id.to_string());
        }
        let result = osovm_post("/run", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── osovm_veilsim ─────────────────────────────────────────────────────────────

pub struct OsovmVeilsimTool;

#[async_trait]
impl Tool for OsovmVeilsimTool {
    fn name(&self) -> &str { "osovm_veilsim" }
    fn description(&self) -> &str {
        "Run a VeilSim physics scenario in OSOVM. \
         Provide veil_ids, entity_count, step_count. \
         Returns f1_score, energy_drift, robustness, and receipt."
    }
    fn required_tier(&self) -> u8 { 3 }
    fn is_write_operation(&self) -> bool { true }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "veil_ids":     { "type": "array", "items": { "type": "integer" } },
                "entity_count": { "type": "integer" },
                "step_count":   { "type": "integer" }
            },
            "required": ["veil_ids", "entity_count", "step_count"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let mut body: Value = serde_json::from_str(params).unwrap_or(json!({}));
        body["agent"] = json!(ctx.agent_id.to_string());
        let result = osovm_post("/veilsim/run", body).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── osovm_health ──────────────────────────────────────────────────────────────

pub struct OsovmHealthTool;

#[async_trait]
impl Tool for OsovmHealthTool {
    fn name(&self) -> &str { "osovm_health" }
    fn description(&self) -> &str {
        "Check if the OSOVM simulation engine is running and healthy."
    }
    fn required_tier(&self) -> u8 { 0 }
    fn is_write_operation(&self) -> bool { false }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {}, "required": [] }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let client = reqwest::Client::new();
        let url = format!("{}/health", osovm_url());
        match client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                let text = r.text().await.unwrap_or_else(|_| "ok".into());
                Ok((text, TokenUsage::default()))
            }
            Ok(r) => Err(format!("OSOVM health check failed: HTTP {}", r.status())),
            Err(e) => Err(format!("OSOVM unreachable: {e}")),
        }
    }
}

// ── registry helper ───────────────────────────────────────────────────────────

pub fn osovm_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(OsovmRunTool),
        Box::new(OsovmVeilsimTool),
        Box::new(OsovmHealthTool),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_count() { assert_eq!(osovm_tools().len(), 3); }

    #[test]
    fn osovm_run_is_write() { assert!(OsovmRunTool.is_write_operation()); }

    #[test]
    fn osovm_health_is_read() { assert!(!OsovmHealthTool.is_write_operation()); }

    #[test]
    fn names() {
        let tools = osovm_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(names.contains(&"osovm_run"));
        assert!(names.contains(&"osovm_veilsim"));
        assert!(names.contains(&"osovm_health"));
    }
}
