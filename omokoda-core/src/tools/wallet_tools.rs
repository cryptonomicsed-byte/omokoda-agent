//! Agent wallet tools — safe wallet creation, signing, and monitoring via
//! Vantage's agent-first wallet API (`/api/agents/{agent_id}/wallets/*`).
//!
//! Native integration of the Vantage wallet capability into the Ọ̀mọ̀ Kọ́dà
//! kernel. These are ordinary tier-gated, permission-gated `Tool`s — the same
//! surface every other kernel capability uses — NOT a separate agent framework.
//!
//! Security model (enforced on the Vantage side; mirrored in intent here):
//!   - Private keys are encrypted server-side and NEVER exposed to the agent.
//!   - Alchemy session tokens are held server-side.
//!   - Signing happens server-side; the agent supplies only intent + auth.
//!   - Every call carries the birth-minted `X-Agent-Key` (added by VantageClient).
//!
//! Unlike the `mesh_*` tools, wallet tools have NO local fallback: money
//! operations fail closed with a clear error when `VANTAGE_URL` is unset,
//! rather than silently pretending to succeed.

use async_trait::async_trait;
use serde_json::json;

use crate::tools::mesh_tools::vantage;
use crate::tools::{ExecutionContext, Tool};

/// Shared error when the wallet backend is not configured. Fail-closed: we do
/// not invent a local wallet, because there is no safe local place to hold keys.
fn no_backend() -> String {
    "wallet operations require Vantage: set VANTAGE_URL (and the agent must be \
     registered so it holds an X-Agent-Key)"
        .to_string()
}

/// Percent-encode the agent id for safe use in the URL path.
fn agent_path(context: &ExecutionContext) -> String {
    format!(
        "/api/agents/{}/wallets",
        urlencoding::encode(&context.agent_id.to_string())
    )
}

// ── wallet_list (read) ──────────────────────────────────────────────────────
pub struct WalletListTool;
#[async_trait]
impl Tool for WalletListTool {
    fn name(&self) -> &str {
        "wallet_list"
    }
    fn description(&self) -> &str {
        "List all wallets owned by this agent (no private keys). Params: none."
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        _params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let vc = vantage().ok_or_else(no_backend)?;
        let res = vc.get(&agent_path(context)).await?;
        Ok((res.to_string(), crate::usage::TokenUsage::default()))
    }
}

// ── wallet_get (read) ───────────────────────────────────────────────────────
pub struct WalletGetTool;
#[async_trait]
impl Tool for WalletGetTool {
    fn name(&self) -> &str {
        "wallet_get"
    }
    fn description(&self) -> &str {
        "Get one wallet's details — address, balance, positions (no private key). Params: {wallet_id}"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": { "wallet_id": { "type": "string", "minLength": 1 } },
            "required": ["wallet_id"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let vc = vantage().ok_or_else(no_backend)?;
        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let wallet_id = v["wallet_id"].as_str().ok_or("missing wallet_id")?;
        let path = format!("{}/{}", agent_path(context), urlencoding::encode(wallet_id));
        let res = vc.get(&path).await?;
        Ok((res.to_string(), crate::usage::TokenUsage::default()))
    }
}

// ── wallet_create (write) ───────────────────────────────────────────────────
pub struct WalletCreateTool;
#[async_trait]
impl Tool for WalletCreateTool {
    fn name(&self) -> &str {
        "wallet_create"
    }
    fn description(&self) -> &str {
        "Create a new wallet for this agent. Params: {type: \"custom\"|\"alchemy\", name}"
    }
    fn required_tier(&self) -> u8 {
        2 // Creator tier — opening a financial account is a real commitment.
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "type": { "type": "string", "enum": ["custom", "alchemy"] },
                "name": { "type": "string", "minLength": 1 }
            },
            "required": ["type", "name"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let vc = vantage().ok_or_else(no_backend)?;
        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let wallet_type = v["type"].as_str().ok_or("missing type")?;
        let name = v["name"].as_str().ok_or("missing name")?;
        let res = vc
            .post(
                &agent_path(context),
                json!({ "type": wallet_type, "name": name }),
            )
            .await?;
        Ok((res.to_string(), crate::usage::TokenUsage::default()))
    }
}

// ── wallet_sign (write, high tier) ──────────────────────────────────────────
pub struct WalletSignTool;
#[async_trait]
impl Tool for WalletSignTool {
    fn name(&self) -> &str {
        "wallet_sign"
    }
    fn description(&self) -> &str {
        "Sign a transaction — Vantage signs server-side, key never exposed. \
         Params: {wallet_id, transaction: object, intent}"
    }
    fn required_tier(&self) -> u8 {
        3 // Signing moves funds: irreversible. Requires an established agent.
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "wallet_id": { "type": "string", "minLength": 1 },
                "transaction": { "type": "object" },
                "intent": { "type": "string", "minLength": 1 }
            },
            "required": ["wallet_id", "transaction", "intent"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let vc = vantage().ok_or_else(no_backend)?;
        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let wallet_id = v["wallet_id"].as_str().ok_or("missing wallet_id")?;
        let path = format!(
            "{}/{}/sign",
            agent_path(context),
            urlencoding::encode(wallet_id)
        );
        let res = vc
            .post(
                &path,
                json!({
                    "transaction": v["transaction"].clone(),
                    "intent": v["intent"].clone(),
                }),
            )
            .await?;
        Ok((res.to_string(), crate::usage::TokenUsage::default()))
    }
}

// ── wallet_alchemy_approve (write) ──────────────────────────────────────────
pub struct WalletAlchemyApproveTool;
#[async_trait]
impl Tool for WalletAlchemyApproveTool {
    fn name(&self) -> &str {
        "wallet_alchemy_approve"
    }
    fn description(&self) -> &str {
        "Request an Alchemy session approval for a wallet. Params: {wallet_id, capabilities: array<string>}"
    }
    fn required_tier(&self) -> u8 {
        3
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "wallet_id": { "type": "string", "minLength": 1 },
                "capabilities": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["wallet_id", "capabilities"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let vc = vantage().ok_or_else(no_backend)?;
        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let wallet_id = v["wallet_id"].as_str().ok_or("missing wallet_id")?;
        let path = format!(
            "{}/{}/alchemy/approve",
            agent_path(context),
            urlencoding::encode(wallet_id)
        );
        let res = vc
            .post(&path, json!({ "capabilities": v["capabilities"].clone() }))
            .await?;
        Ok((res.to_string(), crate::usage::TokenUsage::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::AgentId;
    use crate::tools::ExecutionContext;

    fn ctx(tier: u8) -> ExecutionContext {
        ExecutionContext {
            agent_id: AgentId::from_str("agent-wallet-test"),
            name: "luna".to_string(),
            tier,
            reputation: 100.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "".into(),
            },
            workspace_root: std::env::current_dir().unwrap(),
            sandbox_mode: false,
        }
    }

    // With VANTAGE_URL unset (as in tests), wallet ops fail closed with a clear
    // error rather than fabricating a local wallet.
    #[tokio::test]
    async fn wallet_list_fails_closed_without_vantage() {
        let out = WalletListTool.execute("", &ctx(0)).await;
        assert!(out.is_err());
        assert!(out.unwrap_err().contains("VANTAGE_URL"));
    }

    #[tokio::test]
    async fn wallet_sign_fails_closed_without_vantage() {
        let params = json!({
            "wallet_id": "w1",
            "transaction": {},
            "intent": "trade_order"
        })
        .to_string();
        let out = WalletSignTool.execute(&params, &ctx(3)).await;
        assert!(out.is_err());
        assert!(out.unwrap_err().contains("VANTAGE_URL"));
    }

    #[test]
    fn sign_requires_high_tier_and_is_write() {
        assert_eq!(WalletSignTool.required_tier(), 3);
        assert!(WalletSignTool.is_write_operation());
        assert!(!WalletListTool.is_write_operation());
    }
}
