//! provider_tools.rs — UCX provider-management tools for Omo-Koda2.
//!
//! Exposed tools:
//!   ucx_register_provider  — register this node's GPU/CPU as a UCX provider
//!   ucx_list_providers     — list providers currently registered with the broker
//!   ucx_deregister_provider — remove this agent's provider registration

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

fn broker_url() -> String {
    std::env::var("UCX_BROKER_URL").unwrap_or_else(|_| "http://localhost:7790".to_string())
}

fn agent_id_from_ctx(ctx: &ExecutionContext) -> String {
    ctx.agent_id.as_str().to_string()
}

// ── ucx_register_provider ────────────────────────────────────────────────────

pub struct UcxRegisterProviderTool;

#[async_trait]
impl Tool for UcxRegisterProviderTool {
    fn name(&self) -> &str {
        "ucx_register_provider"
    }
    fn required_tier(&self) -> u8 {
        3
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        30
    }

    fn description(&self) -> &str {
        "Register this node's GPU or CPU as a UCX compute provider. \
         Sets owner_agent_id to this agent so compute contributions are \
         tracked under its identity."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "tier":     { "type": "string", "enum": ["Community", "Professional", "Sovereign"] },
                "gpu_model":   { "type": "string" },
                "vram_gb":     { "type": "number" },
                "gpu_count":   { "type": "integer" },
                "ram_gb":      { "type": "number" },
                "disk_gb":     { "type": "number" },
                "price_gpu_hour_cents": { "type": "integer" },
                "regions":     { "type": "array", "items": { "type": "string" } }
            }
        }))
    }

    async fn execute(
        &self,
        params: &str,
        ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let agent_id = agent_id_from_ctx(ctx);
        let body = json!({
            "provider_id":     format!("omo-{}", &agent_id[..agent_id.len().min(12)]),
            "owner_agent_id":  agent_id,
            "tier":            params["tier"].as_str().unwrap_or("Community"),
            "gpu": {
                "model":   params["gpu_model"].as_str().unwrap_or("unknown"),
                "vram_gb": params["vram_gb"].as_f64().unwrap_or(0.0),
                "count":   params["gpu_count"].as_u64().unwrap_or(1),
                "vendor":  "Nvidia",
                "fp16": true, "bf16": true, "cuda": true, "rocm": false
            },
            "ram_gb":   params["ram_gb"].as_f64().unwrap_or(8.0),
            "disk_gb":  params["disk_gb"].as_f64().unwrap_or(100.0),
            "price_gpu_hour_cents": params["price_gpu_hour_cents"].as_i64().unwrap_or(30),
            "regions":  params["regions"].as_array().cloned().unwrap_or_else(|| vec![json!("local")]),
        });

        let resp = reqwest::Client::new()
            .post(format!("{}/api/providers", broker_url()))
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("ucx_register_provider error: {e}"))?;

        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        Ok((
            json!({ "ok": status.is_success(), "status": status.as_u16(), "body": val })
                .to_string(),
            TokenUsage::default(),
        ))
    }
}

// ── ucx_list_providers ───────────────────────────────────────────────────────

pub struct UcxListProvidersTool;

#[async_trait]
impl Tool for UcxListProvidersTool {
    fn name(&self) -> &str {
        "ucx_list_providers"
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
        "List compute providers currently registered with the UCX broker, \
         with their tier, GPU specs, and price."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {} }))
    }

    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let resp = reqwest::Client::new()
            .get(format!("{}/api/providers", broker_url()))
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await
            .map_err(|e| format!("ucx_list_providers error: {e}"))?;

        let val: Value = resp.json().await.unwrap_or(Value::Null);
        Ok((val.to_string(), TokenUsage::default()))
    }
}

// ── ucx_deregister_provider ──────────────────────────────────────────────────

pub struct UcxDeregisterProviderTool;

#[async_trait]
impl Tool for UcxDeregisterProviderTool {
    fn name(&self) -> &str {
        "ucx_deregister_provider"
    }
    fn required_tier(&self) -> u8 {
        3
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        15
    }

    fn description(&self) -> &str {
        "Deregister this agent's compute provider from the UCX broker."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "required": ["provider_id"],
            "properties": {
                "provider_id": { "type": "string" }
            }
        }))
    }

    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let provider_id = params["provider_id"]
            .as_str()
            .ok_or("provider_id required")?;

        let resp = reqwest::Client::new()
            .delete(format!("{}/api/providers/{}", broker_url(), provider_id))
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await
            .map_err(|e| format!("ucx_deregister_provider error: {e}"))?;

        Ok((
            json!({ "ok": resp.status().is_success() }).to_string(),
            TokenUsage::default(),
        ))
    }
}
