//! ucx_policy.rs — UCX cost-cap and policy enforcement for Omo-Koda2 agents.
//!
//! Provides synchronous helpers the tool layer uses before submitting jobs,
//! and a `ucx_check_policy` tool the agent can call to self-inspect limits.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

/// Default per-job cost cap (cents) if VANTAGE_UCX_COST_CAP_CENTS is unset.
const DEFAULT_COST_CAP_CENTS: u64 = 500; // $5.00

/// Default max GPU hours per job if VANTAGE_UCX_MAX_GPU_HOURS is unset.
const DEFAULT_MAX_GPU_HOURS: f64 = 8.0;

pub struct UcxPolicyConfig {
    pub cost_cap_cents: u64,
    pub max_gpu_hours: f64,
    pub allow_external: bool,
    pub deny_providers: Vec<String>,
}

impl UcxPolicyConfig {
    pub fn from_env() -> Self {
        Self {
            cost_cap_cents: std::env::var("VANTAGE_UCX_COST_CAP_CENTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_COST_CAP_CENTS),
            max_gpu_hours: std::env::var("VANTAGE_UCX_MAX_GPU_HOURS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MAX_GPU_HOURS),
            allow_external: std::env::var("UCX_ALLOW_EXTERNAL")
                .map(|s| s != "false" && s != "0")
                .unwrap_or(true),
            deny_providers: std::env::var("UCX_DENY_PROVIDERS")
                .map(|s| s.split(',').map(str::trim).map(String::from).collect())
                .unwrap_or_default(),
        }
    }

    pub fn check_job(
        &self,
        provider_id: Option<&str>,
        estimated_cents: Option<u64>,
    ) -> Result<(), String> {
        if let Some(cents) = estimated_cents {
            if cents > self.cost_cap_cents {
                return Err(format!(
                    "job estimated cost {}¢ exceeds cap {}¢",
                    cents, self.cost_cap_cents
                ));
            }
        }
        if let Some(pid) = provider_id {
            if self.deny_providers.iter().any(|d| d == pid) {
                return Err(format!("provider '{}' is on the deny list", pid));
            }
        }
        Ok(())
    }
}

// ── ucx_check_policy tool ─────────────────────────────────────────────────────

pub struct UcxCheckPolicyTool;

#[async_trait]
impl Tool for UcxCheckPolicyTool {
    fn name(&self) -> &str {
        "ucx_check_policy"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        5
    }

    fn description(&self) -> &str {
        "Return this agent's UCX compute policy: cost cap, max GPU hours, \
         allowed/denied providers, and whether external providers are allowed."
    }

    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {} }))
    }

    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let cfg = UcxPolicyConfig::from_env();
        let out = json!({
            "cost_cap_cents":  cfg.cost_cap_cents,
            "max_gpu_hours":   cfg.max_gpu_hours,
            "allow_external":  cfg.allow_external,
            "deny_providers":  cfg.deny_providers,
        });
        Ok((out.to_string(), TokenUsage::default()))
    }
}
