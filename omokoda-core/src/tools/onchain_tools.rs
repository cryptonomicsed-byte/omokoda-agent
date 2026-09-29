//! `settle_transaction_tax` exposed as a callable kernel `Tool` — wires
//! OSOVM's `elegbara_router::route_transaction_tax<T>` (see
//! `crate::onchain::settle_transaction_tax`) into the `act` surface so an
//! agent can actually invoke real on-chain settlement, not just have the
//! Rust function exist unreachable in `onchain.rs`.
//!
//! Deliberately env-gated, not hardcoded: as of this writing there is no
//! verified live `elegbara_router` package id or shared router object id —
//! two repos on disk carry conflicting partial candidates, and neither has
//! been confirmed against the live chain. Rather than guess or hardcode
//! either candidate, this tool reads the same two env vars
//! `onchain::settle_transaction_tax` already reads
//! (`OMOKODA_ELEGBARA_PACKAGE`, `OMOKODA_ELEGBARA_ROUTER_ID`) and fails
//! CLEANLY with an explicit, actionable error when either is unset —
//! never a silent no-op, never a panic, never a vague "failed" message.
//! It's meant to be exercised once real ids are confirmed on-chain, not
//! before.

use async_trait::async_trait;
use serde_json::json;

use crate::onchain;
use crate::tools::{ExecutionContext, Tool};

pub struct SettleTransactionTaxTool;

#[async_trait]
impl Tool for SettleTransactionTaxTool {
    fn name(&self) -> &str {
        "settle_transaction_tax"
    }

    fn description(&self) -> &str {
        "Submit a real on-chain settlement to OSOVM's elegbara_router::route_transaction_tax<T> \
         on Sui testnet — the router skims the 3.69% Èṣù tithe and returns the net Coin<T> to \
         the caller. Requires OMOKODA_ELEGBARA_PACKAGE and OMOKODA_ELEGBARA_ROUTER_ID to be set \
         to verified, live-confirmed ids; fails closed with an explicit error otherwise. \
         Params: {coin_object_id, coin_type}"
    }

    fn required_tier(&self) -> u8 {
        // wallet_sign (tools/wallet_tools.rs) is tier 3 for moving funds, but
        // that call is mediated and signed server-side by Vantage, with an
        // audit trail on Vantage's side regardless of what the agent claims.
        // This tool shells out to the local `sui` CLI directly against
        // whatever keystore/wallet the kernel process has access to, with no
        // server-side mediation or second signer in the loop, and burns real
        // gas + skims a real on-chain tithe on an irreversible testnet tx.
        // That is strictly more consequential than a mediated sign, so this
        // tool requires one tier above wallet_sign — matching
        // `agent_orchestration`'s tier 4, the next precedent up in this
        // registry, and staying below tier 5 (reserved for kernel
        // self-modification, a different risk class entirely).
        4
    }

    fn is_write_operation(&self) -> bool {
        true
    }

    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "coin_object_id": { "type": "string", "minLength": 1 },
                "coin_type": { "type": "string", "minLength": 1 }
            },
            "required": ["coin_object_id", "coin_type"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        // Fail clean and explicit BEFORE ever touching onchain::settle_transaction_tax,
        // which itself fails open/silent (returns None) by design for the
        // fail-open birth/update paths elsewhere in onchain.rs. Settlement is
        // real money movement, so this tool refuses to let "unset env var"
        // and "real on-chain failure" collapse into the same vague error.
        let package = std::env::var("OMOKODA_ELEGBARA_PACKAGE").ok();
        let router = std::env::var("OMOKODA_ELEGBARA_ROUTER_ID").ok();
        match (package.as_deref(), router.as_deref()) {
            (Some(p), Some(r)) if !p.trim().is_empty() && !r.trim().is_empty() => {}
            _ => {
                return Err(format!(
                    "settle_transaction_tax is not configured: OMOKODA_ELEGBARA_PACKAGE={:?}, \
                     OMOKODA_ELEGBARA_ROUTER_ID={:?}. Both must be set to a VERIFIED, live \
                     testnet-confirmed elegbara_router package id and shared ElegbaraRouter<T> \
                     object id before this tool can submit a real settlement. As of this \
                     writing no such verified ids exist on disk — do not guess or reuse an \
                     unconfirmed candidate id; confirm against the live chain first, then set \
                     both env vars and retry.",
                    package, router
                ));
            }
        }

        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let coin_object_id = v["coin_object_id"]
            .as_str()
            .ok_or("missing coin_object_id")?;
        let coin_type = v["coin_type"].as_str().ok_or("missing coin_type")?;

        match onchain::settle_transaction_tax(coin_object_id, coin_type).await {
            Some(receipt) => Ok((
                serde_json::to_string(&receipt).map_err(|e| e.to_string())?,
                crate::usage::TokenUsage::default(),
            )),
            None => Err("settle_transaction_tax: on-chain call failed even though \
                 OMOKODA_ELEGBARA_PACKAGE/OMOKODA_ELEGBARA_ROUTER_ID are set — this means the \
                 `sui` binary is missing, the coin object/type is invalid or not owned by this \
                 wallet, gas was insufficient, or the network call itself failed. Check kernel \
                 stderr for the underlying `sui client call` error (logged by onchain.rs) before \
                 retrying."
                .to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::AgentId;
    use crate::tools::ToolRegistry;
    use std::sync::Mutex;

    // std::env::set_var/remove_var race across parallel #[tokio::test] tasks
    // in the same process; every test here mutates the two elegbara env vars,
    // so serialize them the same way the rest of this suite would for any
    // shared-global-state test.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn ctx(tier: u8) -> ExecutionContext {
        ExecutionContext {
            agent_id: AgentId::from_str("agent-settle-test"),
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

    fn clear_env() {
        std::env::remove_var("OMOKODA_ELEGBARA_PACKAGE");
        std::env::remove_var("OMOKODA_ELEGBARA_ROUTER_ID");
    }

    #[test]
    fn is_registered_and_discoverable() {
        let registry = ToolRegistry::new();
        assert!(registry.exists("settle_transaction_tax"));
    }

    #[test]
    fn required_tier_is_four_and_is_a_write_operation() {
        assert_eq!(SettleTransactionTaxTool.required_tier(), 4);
        assert!(SettleTransactionTaxTool.is_write_operation());
    }

    #[test]
    fn tier_enforcement_rejects_a_lower_tier_agent() {
        let registry = ToolRegistry::new();
        // Tier 3 (wallet_sign's tier) is not enough for settlement.
        assert!(!registry.is_allowed("settle_transaction_tax", 3));
        assert!(registry.is_allowed("settle_transaction_tax", 4));
    }

    #[tokio::test]
    async fn execute_rejected_below_required_tier_via_registry() {
        let registry = ToolRegistry::new();
        let policy = crate::permissions::PermissionPolicy::default();
        let params = json!({"coin_object_id": "0xabc", "coin_type": "0x2::sui::SUI"}).to_string();
        let out = registry
            .execute("settle_transaction_tax", &params, ctx(3), &policy, None)
            .await;
        assert!(out.is_err());
        let msg = out.unwrap_err();
        assert!(msg.contains("requires tier 4"), "unexpected message: {msg}");
    }

    #[tokio::test]
    async fn execute_fails_cleanly_when_env_vars_are_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        let params = json!({"coin_object_id": "0xabc", "coin_type": "0x2::sui::SUI"}).to_string();
        let out = SettleTransactionTaxTool.execute(&params, &ctx(4)).await;
        assert!(out.is_err());
        let msg = out.unwrap_err();
        assert!(
            msg.contains("OMOKODA_ELEGBARA_PACKAGE"),
            "unexpected message: {msg}"
        );
        assert!(
            msg.contains("OMOKODA_ELEGBARA_ROUTER_ID"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("not configured"), "unexpected message: {msg}");
    }

    #[tokio::test]
    async fn execute_fails_cleanly_when_only_package_is_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        std::env::set_var("OMOKODA_ELEGBARA_PACKAGE", "0xsomepackage");
        let params = json!({"coin_object_id": "0xabc", "coin_type": "0x2::sui::SUI"}).to_string();
        let out = SettleTransactionTaxTool.execute(&params, &ctx(4)).await;
        clear_env();
        assert!(out.is_err());
        assert!(out.unwrap_err().contains("OMOKODA_ELEGBARA_ROUTER_ID"));
    }

    #[tokio::test]
    async fn execute_rejects_bad_params_before_touching_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        let out = SettleTransactionTaxTool.execute("{}", &ctx(4)).await;
        assert!(out.is_err());
    }
}
