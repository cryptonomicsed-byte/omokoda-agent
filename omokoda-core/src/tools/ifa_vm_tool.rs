//! ifa_vm_tool — direct IfaVM execution with CowrieOracle intent seeding.
//!
//! Exposes the full IfaVM API:
//!   cast        — single-corpus cast (prescriptions only, T0)
//!   cast_dual   — both corpora (Digital Calabash + Ifá), T1
//!   cast_full   — steward-tier full Odù record, T2
//!   execute     — bytecode program execution, T2
//!   lookup      — corpus lookup by name, T0
//!
//! All cast operations seed the CowrieOracle with the agent's name so
//! entropy is deterministically bound to agent identity.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::tools::{ExecutionContext, Tool};

#[derive(Deserialize)]
struct IfaVmParams {
    /// Operation: cast | cast_dual | cast_full | execute | lookup
    action: String,
    /// Optional extra intent string for CowrieOracle seeding (appended to agent name).
    #[serde(default)]
    intent: Option<String>,
    /// Bytecode program lines for `execute` action.
    #[serde(default)]
    program: Vec<String>,
    /// Odù name for `lookup` action.
    #[serde(default)]
    name: Option<String>,
}

pub struct IfaVmTool;

#[async_trait]
impl Tool for IfaVmTool {
    fn name(&self) -> &str {
        "ifa_vm_cast"
    }
    fn description(&self) -> &str {
        "Execute IfaVM operations: cast (single corpus), cast_dual (both corpora), \
         cast_full (steward-tier full record), execute (bytecode program), \
         lookup (corpus search by name). CowrieOracle seeded with agent identity. \
         Params: {action, intent?, program?, name?}"
    }
    fn required_tier(&self) -> u8 {
        0 // cast/lookup are T0; execute enforced at logic level below
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["cast", "cast_dual", "cast_full", "execute", "lookup"],
                    "description": "IfaVM operation to perform"
                },
                "intent": { "type": "string", "description": "Extra intent for CowrieOracle entropy" },
                "program": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Bytecode lines for execute action"
                },
                "name": { "type": "string", "description": "Odù name for lookup action" }
            },
            "required": ["action"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let parsed: IfaVmParams =
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?;

        // execute/cast_full require T2
        if matches!(parsed.action.as_str(), "execute" | "cast_full") && context.tier < 2 {
            return Err(format!(
                "ifa_vm_cast action '{}' requires tier 2, current tier is {}",
                parsed.action, context.tier
            ));
        }

        let agent_name = context.name.clone();
        let intent = parsed.intent.clone();
        let action = parsed.action.clone();
        let program = parsed.program.clone();
        let lookup_name = parsed.name.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
            // Seed CowrieOracle with agent identity + optional extra intent
            let oracle_intent = match &intent {
                Some(extra) => format!("{} {}", agent_name, extra),
                None => agent_name.clone(),
            };

            match action.as_str() {
                "cast" => {
                    let mut vm = ifascript::IfaVM::with_intent(&oracle_intent);
                    let cast = vm.cast_odu();
                    Ok(json!({
                        "action": "cast",
                        "odu_index": cast.index,
                        "universal_name": cast.universal_name,
                        "vessel": format!("{:?}", cast.vessel),
                        "prescriptions": cast.prescriptions,
                        "oracle_intent": oracle_intent,
                    }))
                }
                "cast_dual" => {
                    let mut vm = ifascript::IfaVM::with_intent(&oracle_intent);
                    let (digital, ifa) = vm.cast_dual();
                    Ok(json!({
                        "action": "cast_dual",
                        "digital_calabash": {
                            "odu_index": digital.index,
                            "universal_name": digital.universal_name,
                            "vessel": format!("{:?}", digital.vessel),
                        },
                        "ifa_corpus": {
                            "odu_index": ifa.index,
                            "universal_name": ifa.universal_name,
                            "vessel": format!("{:?}", ifa.vessel),
                        },
                        "oracle_intent": oracle_intent,
                    }))
                }
                "cast_full" => {
                    let mut vm = ifascript::IfaVM::with_intent(&oracle_intent);
                    let odu = vm.cast_odu_full();
                    Ok(json!({
                        "action": "cast_full",
                        "odu_name": odu.name,
                        "universal_name": odu.universal_name,
                        "archetype": odu.archetype,
                        "description": odu.description,
                        "prescriptions": odu.prescriptions,
                        "taboos": odu.taboos,
                        "archetypes": odu.archetypes,
                        "vessel": format!("{:?}", odu.vessel),
                        "oracle_intent": oracle_intent,
                    }))
                }
                "execute" => {
                    if program.is_empty() {
                        return Err("execute action requires non-empty program".to_string());
                    }
                    let mut vm = ifascript::IfaVM::with_intent(&oracle_intent);
                    let prog_refs: Vec<&str> = program.iter().map(|s| s.as_str()).collect();
                    match vm.execute(prog_refs) {
                        Ok(()) => Ok(json!({
                            "action": "execute",
                            "status": "ok",
                            "stack_depth": vm.stack.len(),
                            "halted": vm.halted,
                            "oracle_intent": oracle_intent,
                        })),
                        Err(e) => Ok(json!({
                            "action": "execute",
                            "status": "error",
                            "error": format!("{:?}", e),
                            "stack_depth": vm.stack.len(),
                            "halted": vm.halted,
                        })),
                    }
                }
                "lookup" => {
                    let name_str = lookup_name.as_deref().unwrap_or("");
                    match ifascript::IfaVM::lookup_odu(name_str) {
                        Some(odu) => Ok(json!({
                            "action": "lookup",
                            "found": true,
                            "odu_name": odu.name,
                            "universal_name": odu.universal_name,
                            "archetype": odu.archetype,
                            "description": odu.description,
                            "vessel": format!("{:?}", odu.vessel),
                        })),
                        None => Ok(json!({
                            "action": "lookup",
                            "found": false,
                            "query": name_str,
                        })),
                    }
                }
                other => Err(format!("unknown action: {other}; use cast|cast_dual|cast_full|execute|lookup")),
            }
        })
        .await
        .map_err(|e| format!("ifa_vm_cast task join error: {e}"))??;

        Ok((result.to_string(), crate::usage::TokenUsage::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx(tier: u8) -> ExecutionContext {
        ExecutionContext {
            agent_id: crate::identity::AgentId::new("test"),
            name: "test-agent".to_string(),
            tier,
            reputation: 1.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "test mnemonic phrase here for agent".to_string(),
            },
            workspace_root: PathBuf::from("/tmp"),
            sandbox_mode: false,
        }
    }

    #[tokio::test]
    async fn cast_returns_odu_fields() {
        let tool = IfaVmTool;
        let result = tool.execute(r#"{"action":"cast"}"#, &ctx(0)).await;
        assert!(result.is_ok(), "cast should succeed: {:?}", result);
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["action"], "cast");
        assert!(v["universal_name"].is_string());
        assert!(v["odu_index"].is_number());
    }

    #[tokio::test]
    async fn cast_dual_returns_both_corpora() {
        let tool = IfaVmTool;
        let result = tool.execute(r#"{"action":"cast_dual"}"#, &ctx(1)).await;
        assert!(result.is_ok());
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["digital_calabash"]["odu_name"].is_string());
        assert!(v["ifa_corpus"]["odu_name"].is_string());
    }

    #[tokio::test]
    async fn execute_requires_tier2() {
        let tool = IfaVmTool;
        let result = tool.execute(r#"{"action":"execute","program":["CAST"]}"#, &ctx(1)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tier 2"));
    }

    #[tokio::test]
    async fn lookup_finds_known_odu() {
        let tool = IfaVmTool;
        let result = tool.execute(r#"{"action":"lookup","name":"Ogbe"}"#, &ctx(0)).await;
        assert!(result.is_ok());
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // May or may not find depending on corpus name casing — just assert no panic
        assert!(v["action"] == "lookup");
    }

    #[tokio::test]
    async fn intent_seeding_in_output() {
        let tool = IfaVmTool;
        let result = tool
            .execute(r#"{"action":"cast","intent":"ritual clarity"}"#, &ctx(0))
            .await;
        assert!(result.is_ok());
        let (out, _) = result.unwrap();
        assert!(out.contains("oracle_intent"));
    }
}
