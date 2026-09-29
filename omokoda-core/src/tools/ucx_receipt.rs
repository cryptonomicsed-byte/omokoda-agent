//! ucx_receipt.rs — UCX job status and receipt retrieval tools.
//!
//! Exposed tools:
//!   ucx_job_status  — poll a job's current status
//!   ucx_job_receipt — retrieve the final ComputeReceipt for a completed job

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

fn broker_url() -> String {
    std::env::var("UCX_BROKER_URL").unwrap_or_else(|_| "http://localhost:7790".to_string())
}

// ── ucx_job_status ────────────────────────────────────────────────────────────

pub struct UcxJobStatusTool;

#[async_trait]
impl Tool for UcxJobStatusTool {
    fn name(&self) -> &str {
        "ucx_job_status"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        15
    }

    fn description(&self) -> &str {
        "Poll the status of a UCX compute job. Returns: Pending | Running | Completed | Failed | Cancelled."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "required": ["job_id"],
            "properties": {
                "job_id": { "type": "string", "description": "UUID returned by ucx_request_compute" }
            }
        }))
    }

    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let job_id = params["job_id"].as_str().ok_or("job_id required")?;

        let resp = reqwest::Client::new()
            .get(format!("{}/api/jobs/{}/status", broker_url(), job_id))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("ucx_job_status error: {e}"))?;

        let val: Value = resp.json().await.unwrap_or(Value::Null);
        Ok((val.to_string(), TokenUsage::default()))
    }
}

// ── ucx_job_receipt ────────────────────────────────────────────────────────────

pub struct UcxJobReceiptTool;

#[async_trait]
impl Tool for UcxJobReceiptTool {
    fn name(&self) -> &str {
        "ucx_job_receipt"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        20
    }

    fn description(&self) -> &str {
        "Retrieve the final ComputeReceipt for a completed UCX job. \
         Includes GPU/CPU seconds, billing, and verification proof."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "required": ["job_id"],
            "properties": {
                "job_id": { "type": "string" }
            }
        }))
    }

    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let job_id = params["job_id"].as_str().ok_or("job_id required")?;

        let resp = reqwest::Client::new()
            .get(format!("{}/api/jobs/{}/receipt", broker_url(), job_id))
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("ucx_job_receipt error: {e}"))?;

        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);

        if !status.is_success() {
            return Err(format!("ucx_job_receipt {} — {}", status, val));
        }
        Ok((val.to_string(), TokenUsage::default()))
    }
}
