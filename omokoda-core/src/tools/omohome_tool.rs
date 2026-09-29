use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

fn omohome_base() -> String {
    std::env::var("OMOHOME_URL").unwrap_or_else(|_| "http://127.0.0.1:7795".to_string())
}

async fn get(path: &str) -> Result<Value, String> {
    let url = format!("{}{}", omohome_base(), path);
    reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())
}

async fn post(path: &str, body: Value) -> Result<Value, String> {
    let url = format!("{}{}", omohome_base(), path);
    reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())
}

// ── omohome_list_devices ──────────────────────────────────────────────────────

pub struct OmoHomeListDevicesTool;

#[async_trait]
impl Tool for OmoHomeListDevicesTool {
    fn name(&self) -> &str {
        "omohome_list_devices"
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        10
    }
    fn description(&self) -> &str {
        "List all physical devices known to the OmoHome sovereign habitat node."
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {} }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let result = match get("/api/devices").await {
            Ok(v) => json!({ "ok": true, "devices": v }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        Ok((result.to_string(), TokenUsage::default()))
    }
}

// ── omohome_get_state ─────────────────────────────────────────────────────────

pub struct OmoHomeGetStateTool;

#[async_trait]
impl Tool for OmoHomeGetStateTool {
    fn name(&self) -> &str {
        "omohome_get_state"
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        10
    }
    fn description(&self) -> &str {
        "Get the current state of a physical device (e.g. 'light.living_room')."
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "string", "description": "HA entity_id, e.g. light.living_room" }
            },
            "required": ["entity_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let eid = params
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "entity_id required".to_string())?;
        let result = match get(&format!("/api/devices/{}/state", eid)).await {
            Ok(v) => json!({ "ok": true, "state": v }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        Ok((result.to_string(), TokenUsage::default()))
    }
}

// ── omohome_call_service ──────────────────────────────────────────────────────

pub struct OmoHomeCallServiceTool;

#[async_trait]
impl Tool for OmoHomeCallServiceTool {
    fn name(&self) -> &str {
        "omohome_call_service"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        15
    }
    fn description(&self) -> &str {
        "Call a service on a physical device. E.g. turn_on a light, start a vacuum."
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "string" },
                "service":   { "type": "string", "description": "turn_on | turn_off | start | set_temperature | ..." },
                "data":      { "type": "object" }
            },
            "required": ["entity_id", "service"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        let eid = params
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "entity_id required".to_string())?;
        let svc = params
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "service required".to_string())?;
        let data = params.get("data").cloned().unwrap_or(json!({}));
        let body = json!({ "service": svc, "data": data });
        let result = match post(&format!("/api/devices/{}/call_service", eid), body).await {
            Ok(v) => json!({ "ok": true, "result": v }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        Ok((result.to_string(), TokenUsage::default()))
    }
}

// ── omohome_list_areas ────────────────────────────────────────────────────────

pub struct OmoHomeListAreasTool;

#[async_trait]
impl Tool for OmoHomeListAreasTool {
    fn name(&self) -> &str {
        "omohome_list_areas"
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        10
    }
    fn description(&self) -> &str {
        "List all physical areas/rooms in the habitat."
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {} }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let result = match get("/api/areas").await {
            Ok(v) => json!({ "ok": true, "areas": v }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        Ok((result.to_string(), TokenUsage::default()))
    }
}

// ── omohome_health ────────────────────────────────────────────────────────────

pub struct OmoHomeHealthTool;

#[async_trait]
impl Tool for OmoHomeHealthTool {
    fn name(&self) -> &str {
        "omohome_health"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        5
    }
    fn description(&self) -> &str {
        "Check if the OmoHome habitat node is running and how many devices/areas are registered."
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {} }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let result = match get("/health").await {
            Ok(v) => json!({ "ok": true, "health": v }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        Ok((result.to_string(), TokenUsage::default()))
    }
}
