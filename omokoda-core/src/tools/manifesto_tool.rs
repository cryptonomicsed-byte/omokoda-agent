//! manifesto_tool — Living Manifesto propose/vote/view/initiate.
//!
//! Persists the Manifesto as JSON at `~/.omokoda/{agent_name}/manifesto.json`.
//! Actions:
//!   propose  — add a new Odù-backed principle (divines odu_id from IfaVM)
//!   vote     — cast a tier-weighted vote for a clause
//!   view     — return the full manifesto (or canon only)
//!   initiate — find canon clauses aligned with agent's Odù identity

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::tools::{ExecutionContext, Tool};

#[derive(Deserialize)]
struct ManifestoParams {
    /// Action: propose | vote | view | initiate
    action: String,
    /// Principle text — required for propose
    #[serde(default)]
    principle: Option<String>,
    /// Clause id — required for vote
    #[serde(default)]
    clause_id: Option<u64>,
    /// true = canon only, false/omitted = full manifesto for view
    #[serde(default)]
    canon_only: bool,
    /// Collective name, defaults to agent name
    #[serde(default)]
    collective: Option<String>,
}

fn manifesto_path(agent_name: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home)
        .join(".omokoda")
        .join(agent_name)
        .join("manifesto.json")
}

fn load_manifesto(path: &std::path::Path, collective: &str) -> ifascript::Manifesto {
    if path.exists() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(m) = serde_json::from_str::<ifascript::Manifesto>(&text) {
                return m;
            }
        }
    }
    ifascript::Manifesto::new(collective)
}

fn save_manifesto(path: &std::path::Path, manifesto: &ifascript::Manifesto) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
    }
    let text = serde_json::to_string_pretty(manifesto).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("write failed: {e}"))
}

pub struct ManifestoTool;

#[async_trait]
impl Tool for ManifestoTool {
    fn name(&self) -> &str {
        "manifesto"
    }
    fn description(&self) -> &str {
        "Living Manifesto — propose Odù-backed principles, vote to ratify, view canon. \
         Actions: propose (add principle), vote (ratify clause), \
         view (see manifesto/canon), initiate (align agent to canon). \
         Params: {action, principle?, clause_id?, canon_only?, collective?}"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        true // propose and vote mutate persisted state
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["propose", "vote", "view", "initiate"]
                },
                "principle": { "type": "string" },
                "clause_id": { "type": "integer" },
                "canon_only": { "type": "boolean" },
                "collective": { "type": "string" }
            },
            "required": ["action"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let parsed: ManifestoParams =
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?;

        let agent_name = context.name.clone();
        let tier = context.tier;
        let odu_primary = context.odu_identity.primary_index;
        let collective = parsed
            .collective
            .clone()
            .unwrap_or_else(|| agent_name.clone());
        let path = manifesto_path(&agent_name);
        let action = parsed.action.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
            let mut manifesto = load_manifesto(&path, &collective);

            match action.as_str() {
                "propose" => {
                    let principle = parsed.principle
                        .ok_or("propose requires principle text")?;

                    // Divine odu_id for this principle via IfaVM seeded with agent name
                    let mut vm = ifascript::IfaVM::with_intent(&agent_name);
                    let cast = vm.cast_odu();
                    let odu_id = cast.binary as u16;

                    let clause_id = manifesto.propose(odu_id, &principle, &agent_name);
                    save_manifesto(&path, &manifesto)?;

                    Ok(json!({
                        "action": "propose",
                        "clause_id": clause_id,
                        "odu_id": odu_id,
                        "odu_name": cast.odu.name,
                        "vessel": format!("{:?}", cast.odu.vessel),
                        "principle": principle,
                        "level": "Individual",
                    }))
                }

                "vote" => {
                    let clause_id = parsed.clause_id
                        .ok_or("vote requires clause_id")?;

                    let new_level = manifesto.vote(clause_id, tier)
                        .ok_or_else(|| format!("clause {} not found", clause_id))?;

                    save_manifesto(&path, &manifesto)?;

                    Ok(json!({
                        "action": "vote",
                        "clause_id": clause_id,
                        "new_level": format!("{:?}", new_level),
                        "weight": manifesto.weight(clause_id),
                        "in_canon": matches!(new_level,
                            ifascript::ConsensusLevel::Council | ifascript::ConsensusLevel::Canonical),
                    }))
                }

                "view" => {
                    let output = if parsed.canon_only {
                        let canon = manifesto.canon();
                        json!({
                            "action": "view",
                            "collective": manifesto.collective,
                            "canon_only": true,
                            "canon_count": canon.len(),
                            "clauses": canon.iter().map(|c| json!({
                                "id": c.id,
                                "odu_id": c.odu_id,
                                "odu_name": c.odu_name,
                                "vessel": c.vessel,
                                "principle": c.principle,
                                "author": c.author,
                                "level": format!("{:?}", c.level),
                            })).collect::<Vec<_>>(),
                        })
                    } else {
                        json!({
                            "action": "view",
                            "collective": manifesto.collective,
                            "total_clauses": manifesto.clauses.len(),
                            "clauses": manifesto.clauses.iter().map(|c| json!({
                                "id": c.id,
                                "odu_id": c.odu_id,
                                "odu_name": c.odu_name,
                                "vessel": c.vessel,
                                "principle": c.principle,
                                "author": c.author,
                                "level": format!("{:?}", c.level),
                                "weight": manifesto.weight(c.id),
                            })).collect::<Vec<_>>(),
                        })
                    };
                    Ok(output)
                }

                "initiate" => {
                    let agent_odu = odu_primary as u16;
                    let aligned = manifesto.initiate(agent_odu);
                    Ok(json!({
                        "action": "initiate",
                        "agent_odu": agent_odu,
                        "aligned_count": aligned.len(),
                        "clauses": aligned.iter().map(|c| json!({
                            "id": c.id,
                            "odu_id": c.odu_id,
                            "odu_name": c.odu_name,
                            "vessel": c.vessel,
                            "principle": c.principle,
                            "level": format!("{:?}", c.level),
                        })).collect::<Vec<_>>(),
                    }))
                }

                other => Err(format!("unknown action: {other}; use propose|vote|view|initiate")),
            }
        })
        .await
        .map_err(|e| format!("manifesto task join error: {e}"))??;

        Ok((result.to_string(), crate::usage::TokenUsage::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx(tier: u8) -> ExecutionContext {
        ExecutionContext {
            agent_id: crate::identity::AgentId::new(),
            name: format!("test-manifesto-{}", tier),
            tier,
            reputation: 1.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "test mnemonic phrase here".to_string(),
            },
            workspace_root: PathBuf::from("/tmp"),
            sandbox_mode: false,
        }
    }

    #[tokio::test]
    async fn view_empty_manifesto() {
        let tool = ManifestoTool;
        let result = tool.execute(r#"{"action":"view","collective":"test-collective-empty"}"#, &ctx(0)).await;
        assert!(result.is_ok(), "{:?}", result);
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["action"], "view");
        assert_eq!(v["total_clauses"], 0);
    }

    #[tokio::test]
    async fn propose_creates_clause() {
        let tool = ManifestoTool;
        let ctx = ctx(1);
        let result = tool
            .execute(
                r#"{"action":"propose","principle":"We build with integrity."}"#,
                &ctx,
            )
            .await;
        assert!(result.is_ok(), "{:?}", result);
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["action"], "propose");
        assert!(v["clause_id"].is_number());
        assert_eq!(v["level"], "Individual");
    }
}
