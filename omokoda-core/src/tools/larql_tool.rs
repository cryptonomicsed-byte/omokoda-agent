//! larql_tool — LARQL corpus query engine for the Digital Calabash.
//!
//! LARQL (Living Ancestral Relational Query Language) queries the 256-Odù
//! corpus using three query forms:
//!   DESCRIBE <index|"name"> [AT SCALE micro|meso|macro]
//!   VERIFY <Vessel> WHERE <field> CONTAINS <value>
//!   PREPARE <action> CHECK: <Vessel>
//!
//! Returns a summary list and an optional passed boolean (for VERIFY/PREPARE).

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::tools::{ExecutionContext, Tool};

#[derive(Deserialize)]
struct LarqlParams {
    query: String,
}

pub struct LarqlTool;

#[async_trait]
impl Tool for LarqlTool {
    fn name(&self) -> &str {
        "larql_query"
    }
    fn description(&self) -> &str {
        "Query the Digital Calabash corpus using LARQL. Supports: \
         DESCRIBE <index|\"name\"> [AT SCALE micro|meso|macro], \
         VERIFY <Vessel> WHERE <field> CONTAINS <value>, \
         PREPARE <action> CHECK: <Vessel>. \
         Params: {\"query\": \"DESCRIBE 0\"}"
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
            "properties": {
                "query": {
                    "type": "string",
                    "description": "LARQL query string"
                }
            },
            "required": ["query"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let parsed: LarqlParams =
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?;

        let query_str = parsed.query.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, String> {
            let query = ifascript::parse_larql(&query_str)
                .map_err(|e| format!("LARQL parse error: {:?}", e))?;

            let answer = ifascript::larql::execute(&query)
                .map_err(|e| format!("LARQL execution error: {:?}", e))?;

            Ok(json!({
                "query": query_str,
                "summary": answer.summary,
                "passed": answer.passed,
            }))
        })
        .await
        .map_err(|e| format!("larql_query task join error: {e}"))??;

        Ok((result.to_string(), crate::usage::TokenUsage::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            agent_id: crate::identity::AgentId::from_str("test-agent"),
            name: "test-agent".to_string(),
            tier: 0,
            reputation: 1.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: "test mnemonic".to_string(),
            },
            workspace_root: PathBuf::from("/tmp"),
            sandbox_mode: false,
        }
    }

    #[tokio::test]
    async fn describe_index_returns_summary() {
        let tool = LarqlTool;
        let result = tool.execute(r#"{"query":"DESCRIBE 0"}"#, &ctx()).await;
        assert!(result.is_ok(), "DESCRIBE 0 should succeed: {:?}", result);
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["summary"].is_array());
        assert!(!v["summary"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_query_returns_error() {
        let tool = LarqlTool;
        let result = tool.execute(r#"{"query":"INVALID SYNTAX HERE"}"#, &ctx()).await;
        assert!(result.is_err() || {
            let (out, _) = result.unwrap();
            out.contains("error") || out.contains("parse")
        });
    }

    #[tokio::test]
    async fn verify_query_returns_passed_field() {
        let tool = LarqlTool;
        let result = tool
            .execute(r#"{"query":"VERIFY Genesis WHERE archetypes CONTAINS \"Steward\""}"#, &ctx())
            .await;
        assert!(result.is_ok(), "VERIFY query should not panic: {:?}", result);
        let (out, _) = result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // passed may be true/false/null — just assert it's present
        assert!(v.get("passed").is_some());
    }
}
