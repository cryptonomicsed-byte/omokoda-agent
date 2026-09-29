//! ucx_tool.rs — UCX compute tools for Omo-Koda2 agents.
//!
//! Config: UCX_BROKER_URL (default "http://localhost:7790")
//!         VANTAGE_URL + VANTAGE_KEY (existing env vars, reused for provider registration)
//!
//! Exposed tools:
//!   ucx_request_compute — submit a compute job to the UCX broker
//!   ucx_offer_compute   — register this agent's machine as a UCX provider

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

fn broker_url() -> String {
    std::env::var("UCX_BROKER_URL").unwrap_or_else(|_| "http://localhost:7790".to_string())
}

fn vantage_url() -> Option<String> {
    std::env::var("VANTAGE_URL").ok().filter(|s| !s.is_empty())
}

fn vantage_key() -> String {
    std::env::var("VANTAGE_KEY").unwrap_or_default()
}

// ── ucx_request_compute ──────────────────────────────────────────────────────

pub struct UcxRequestComputeTool;

#[async_trait]
impl Tool for UcxRequestComputeTool {
    fn name(&self) -> &str {
        "ucx_request_compute"
    }
    fn required_tier(&self) -> u8 {
        3
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        120
    }

    fn description(&self) -> &str {
        "Submit a compute job to the UCX broker (Universal Compute Exchange). \
         The broker routes to the best available native or external provider. \
         Returns an Allocation with job_id and provider_id for polling."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "required": ["workload"],
            "properties": {
                "workload": {
                    "type": "string",
                    "enum": ["Inference", "Training", "Rendering", "Simulation", "CiCd", "Generic"],
                    "description": "Type of compute workload"
                },
                "requirements": {
                    "type": "object",
                    "description": "Hardware requirements: vram_gb, ram_gb, cpu_cores, gpu_count, cuda",
                    "properties": {
                        "vram_gb":   { "type": "number" },
                        "ram_gb":    { "type": "number" },
                        "cpu_cores": { "type": "integer" },
                        "cuda":      { "type": "boolean" }
                    }
                },
                "constraints": {
                    "type": "object",
                    "description": "Policy constraints: max_price_cents, allow_external",
                    "properties": {
                        "max_price_cents": { "type": "integer" },
                        "allow_external":  { "type": "boolean" }
                    }
                },
                "runtime_spec": {
                    "type": "object",
                    "description": "Workload-specific config: model, messages (inference); training_file, suffix (training)"
                }
            }
        }))
    }

    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let spec: Value = serde_json::from_str(params)
            .map_err(|e| format!("ucx_request_compute: invalid params: {e}"))?;

        let body = json!({
            "submitter_id": context.agent_id.as_str(),
            "workload":     spec.get("workload").cloned().unwrap_or(json!("Generic")),
            "requirements": spec.get("requirements").cloned().unwrap_or(json!({})),
            "constraints":  spec.get("constraints").cloned().unwrap_or(json!({"allow_external": true})),
            "runtime_spec": spec.get("runtime_spec").cloned().unwrap_or(json!({})),
        });

        let url = format!("{}/api/jobs", broker_url());
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| format!("UCX broker unreachable at {url}: {e}"))?;

        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("UCX broker error {status}: {val}"));
        }

        Ok((
            serde_json::to_string_pretty(&val).unwrap_or_default(),
            TokenUsage::default(),
        ))
    }
}

// ── ucx_offer_compute ────────────────────────────────────────────────────────

pub struct UcxOfferComputeTool;

#[async_trait]
impl Tool for UcxOfferComputeTool {
    fn name(&self) -> &str {
        "ucx_offer_compute"
    }
    fn required_tier(&self) -> u8 {
        4
    }
    fn is_write_operation(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Register this agent's machine as a UCX provider via the Vantage rendezvous. \
         Hardware is auto-discovered; the agent can override price and policy. \
         Call periodically (every <2 min) to keep the registration alive."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "price_gpu_hour_cents": {
                    "type": "integer",
                    "description": "Price per GPU-hour in USD cents, e.g. 35 = $0.35/hr"
                },
                "price_cpu_hour_cents": {
                    "type": "integer",
                    "description": "Price per CPU-hour in USD cents, e.g. 2 = $0.02/hr"
                },
                "policy_deny": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Workload types to refuse, e.g. [\"cryptomining\"]"
                },
                "tier": {
                    "type": "string",
                    "enum": ["Personal", "Community", "Professional"],
                    "description": "Marketplace tier (default Community)"
                }
            }
        }))
    }

    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let overrides: Value = serde_json::from_str(params).unwrap_or(json!({}));

        let vantage = vantage_url()
            .ok_or_else(|| "VANTAGE_URL not set — cannot register UCX provider".to_string())?;

        // Build a minimal capability advertisement.
        // In a full implementation this calls ucx-provider's discover_capability() via FFI.
        // Here we build from env/context and let the operator override via params.
        let capability = json!({
            "provider_id":            format!("omokoda-{}", context.agent_id.as_str()),
            "tier":                   overrides.get("tier").cloned().unwrap_or(json!("Community")),
            "trust":                  "Standard",
            "cpu": {
                "cores": num_cpus(),
                "arch":  current_arch()
            },
            "ram_gb":                 available_ram_gb(),
            "disk_gb":                0,
            "runtimes":               ["Oci"],
            "price_gpu_hour_cents":   overrides.get("price_gpu_hour_cents").cloned(),
            "price_cpu_hour_cents":   overrides.get("price_cpu_hour_cents").cloned().unwrap_or(json!(2)),
            "policy_deny":            overrides.get("policy_deny").cloned().unwrap_or(json!([])),
            "regions":                []
        });

        let url = format!("{}/api/ucx/providers/register", vantage);
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("X-Agent-Key", vantage_key())
            .json(&capability)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Vantage unreachable at {url}: {e}"))?;

        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("Vantage UCX register error {status}: {val}"));
        }

        Ok((
            serde_json::to_string_pretty(&val).unwrap_or_default(),
            TokenUsage::default(),
        ))
    }
}

// ── hardware helpers ─────────────────────────────────────────────────────────

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        return "X86_64";
    }
    if cfg!(target_arch = "aarch64") {
        return "Arm64";
    }
    "Other"
}

fn available_ram_gb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(line) = info.lines().find(|l| l.starts_with("MemAvailable:")) {
                if let Some(kb) = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<f64>().ok())
                {
                    return kb / (1024.0 * 1024.0);
                }
            }
        }
    }
    4.0
}

// ── vcp_request_session ───────────────────────────────────────────────────────

fn vcp_broker_url() -> String {
    std::env::var("VCP_BROKER_URL").unwrap_or_else(|_| "http://localhost:7791".to_string())
}

/// Request a VCP session — agent inhabits a physical device for a scoped task.
pub struct VcpRequestSessionTool;

#[async_trait]
impl Tool for VcpRequestSessionTool {
    fn name(&self) -> &str {
        "vcp_request_session"
    }

    fn description(&self) -> &str {
        "Request a VCP (Vantage Connection Protocol) session to inhabit a physical \
         device (robot, drone, sensor, camera). Submits a session request to the \
         VCP broker and returns a challenge that the device must answer. \
         Required params: device_id, required_caps (list of capability names). \
         Optional: duration_secs (default 3600), scope (Exclusive/Shared/SingleUse)."
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

    async fn execute(
        &self,
        params: &str,
        ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let spec: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let device_id = spec["device_id"].as_str().ok_or("device_id required")?;
        let required_caps = spec["required_caps"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let agent_did = ctx.agent_id.clone();
        let url = format!("{}/api/sessions", vcp_broker_url());

        let body = json!({
            "device_id":     device_id,
            "agent_did":     agent_did,
            "required_caps": required_caps,
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("VCP broker unreachable: {e}"))?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);

        if !status.is_success() {
            return Err(format!("VCP session request failed {status}: {val}"));
        }

        Ok((val.to_string(), TokenUsage::default()))
    }
}

// ── vcp_list_devices ──────────────────────────────────────────────────────────

pub struct VcpListDevicesTool;

#[async_trait]
impl Tool for VcpListDevicesTool {
    fn name(&self) -> &str {
        "vcp_list_devices"
    }

    fn description(&self) -> &str {
        "List physical devices currently available for VCP session inhabitation. \
         Returns device_id, label, class, safety_class for each fresh device."
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

    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let url = if let Some(vantage) = vantage_url() {
            format!("{vantage}/api/vcp/devices")
        } else {
            format!("{}/api/devices", vcp_broker_url())
        };

        let client = reqwest::Client::new();
        let mut req = client.get(&url);
        if let Some(_vantage) = vantage_url() {
            req = req.header("X-Agent-Key", vantage_key());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("VCP device list failed: {e}"))?;
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        Ok((val.to_string(), TokenUsage::default()))
    }
}

// ── vcp_revoke_session ────────────────────────────────────────────────────────

pub struct VcpRevokeSessionTool;

#[async_trait]
impl Tool for VcpRevokeSessionTool {
    fn name(&self) -> &str {
        "vcp_revoke_session"
    }

    fn description(&self) -> &str {
        "Revoke an active VCP session. Required params: session_id."
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

    async fn execute(
        &self,
        params: &str,
        ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let spec: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let session_id = spec["session_id"].as_str().ok_or("session_id required")?;
        let url = format!("{}/api/sessions/{session_id}", vcp_broker_url());

        let body = json!({ "revoker_did": ctx.agent_id });

        let client = reqwest::Client::new();
        let resp = client
            .delete(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("VCP revoke failed: {e}"))?;
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        Ok((val.to_string(), TokenUsage::default()))
    }
}

/// Returns registered tools for UCX compute.
pub fn ucx_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(UcxRequestComputeTool),
        Box::new(UcxOfferComputeTool),
    ]
}

/// Returns registered tools for VCP device inhabitation.
pub fn vcp_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(VcpRequestSessionTool),
        Box::new(VcpListDevicesTool),
        Box::new(VcpRevokeSessionTool),
    ]
}
