//! sovereign_node.rs — MCP tool bridge to the sovereign-node physical capture layer.
//!
//! Calls POST http://{SOVEREIGN_NODE_URL}/mcp with JSON-RPC 2.0 to invoke:
//!   vcp_nearby_devices, vcp_connect, vcp_capture,
//!   sovereign_status, sovereign_job_status, dip_send
//!
//! Config: SOVEREIGN_NODE_URL (default "http://localhost:8080")

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

fn node_url() -> String {
    std::env::var("SOVEREIGN_NODE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// Call the sovereign-node /mcp JSON-RPC 2.0 endpoint.
/// Returns the text content of the first result item.
async fn mcp_call(tool_name: &str, arguments: Value) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0",
        "id":      1,
        "method":  "tools/call",
        "params":  { "name": tool_name, "arguments": arguments }
    });
    let resp = client
        .post(format!("{}/mcp", node_url()))
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("sovereign-node unreachable: {e}"))?;

    let val: Value = resp
        .json()
        .await
        .map_err(|e| format!("mcp response parse error: {e}"))?;

    if let Some(err) = val.get("error") {
        return Err(format!("mcp error: {err}"));
    }
    val.pointer("/result/content/0/text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("unexpected mcp response shape: {val}"))
}

// ── vcp_nearby_devices ────────────────────────────────────────────────────────

pub struct VcpNearbyDevicesTool;

#[async_trait]
impl Tool for VcpNearbyDevicesTool {
    fn name(&self) -> &str {
        "vcp_nearby_devices"
    }
    fn description(&self) -> &str {
        "List all VCP devices currently visible to the sovereign node (robots, drones, IoT). \
         Call during PERCEIVE to know what physical machines are nearby."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {}, "required": [] }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let result = mcp_call("vcp_nearby_devices", json!({})).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── vcp_connect ───────────────────────────────────────────────────────────────

pub struct VcpConnectTool;

#[async_trait]
impl Tool for VcpConnectTool {
    fn name(&self) -> &str {
        "vcp_connect"
    }
    fn description(&self) -> &str {
        "Check whether a specific VCP device is reachable and ready for capability negotiation. \
         Requires device_id from vcp_nearby_devices."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "device_id": { "type": "string", "description": "VCP device ID from vcp_nearby_devices" }
            },
            "required": ["device_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("vcp_connect", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── vcp_capture ───────────────────────────────────────────────────────────────

pub struct VcpCaptureTool;

#[async_trait]
impl Tool for VcpCaptureTool {
    fn name(&self) -> &str {
        "vcp_capture"
    }
    fn description(&self) -> &str {
        "Trigger a full Gaussian-splat capture pipeline for a VCP device. \
         Returns a job_id — poll with sovereign_job_status."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "device_id": { "type": "string", "description": "VCP device ID to capture" }
            },
            "required": ["device_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("vcp_capture", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── sovereign_status ──────────────────────────────────────────────────────────

pub struct SovereignStatusTool;

#[async_trait]
impl Tool for SovereignStatusTool {
    fn name(&self) -> &str {
        "sovereign_status"
    }
    fn description(&self) -> &str {
        "Return sovereign node uptime, DID, active job count, and device count."
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {}, "required": [] }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let result = mcp_call("sovereign_status", json!({})).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── sovereign_job_status ──────────────────────────────────────────────────────

pub struct SovereignJobStatusTool;

#[async_trait]
impl Tool for SovereignJobStatusTool {
    fn name(&self) -> &str {
        "sovereign_job_status"
    }
    fn description(&self) -> &str {
        "Poll the status of a capture job by job_id. \
         Use after vcp_capture to check if the capture completed."
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "job_id": { "type": "string" }
            },
            "required": ["job_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("sovereign_job_status", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── dip_send ──────────────────────────────────────────────────────────────────

pub struct DipSendTool;

#[async_trait]
impl Tool for DipSendTool {
    fn name(&self) -> &str {
        "dip_send"
    }
    fn description(&self) -> &str {
        "Send a DIP Message envelope to a destination DID via Vantage, Nostr, or Meshtastic."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "destination_did": { "type": "string" },
                "network":         { "type": "string", "enum": ["vantage", "nostr", "meshtastic"] },
                "payload":         { "type": "object" }
            },
            "required": ["destination_did", "network", "payload"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("dip_send", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── vcp_body_session_open ─────────────────────────────────────────────────────

pub struct VcpBodySessionOpenTool;

#[async_trait]
impl Tool for VcpBodySessionOpenTool {
    fn name(&self) -> &str {
        "vcp_body_session_open"
    }
    fn description(&self) -> &str {
        "Open a fine-grained VCP body session for a device, specifying capabilities \
         (locomotion, sensor.camera, …). Returns session_id for subsequent commands. \
         Use for scripted robot control; use vcp_capture for fully automated splat pipeline."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "device_id":    { "type": "string" },
                "capabilities": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Capability names (e.g. [\"locomotion\", \"sensor.camera\"])"
                },
                "agent_tier":   { "type": "string", "description": "t1–t5, default t4" }
            },
            "required": ["device_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("vcp_body_session_open", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── vcp_body_session_command ──────────────────────────────────────────────────

pub struct VcpBodySessionCommandTool;

#[async_trait]
impl Tool for VcpBodySessionCommandTool {
    fn name(&self) -> &str {
        "vcp_body_session_command"
    }
    fn description(&self) -> &str {
        "Send a capability command within an open VCP body session (locomotion, sensor.camera, etc). \
         Returns the command receipt."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "capability": { "type": "string", "description": "e.g. locomotion, sensor.camera" },
                "action":     { "type": "string", "description": "e.g. walk, capture_frame" },
                "params":     { "type": "object" }
            },
            "required": ["session_id", "capability", "action"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("vcp_body_session_command", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── vcp_body_session_close ────────────────────────────────────────────────────

pub struct VcpBodySessionCloseTool;

#[async_trait]
impl Tool for VcpBodySessionCloseTool {
    fn name(&self) -> &str {
        "vcp_body_session_close"
    }
    fn description(&self) -> &str {
        "Close a VCP body session and return the session receipt. \
         If session had camera capability and mission_success=true, \
         a capture job is auto-queued — poll with sovereign_job_status."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "session_id":      { "type": "string" },
                "mission_success": { "type": "boolean" }
            },
            "required": ["session_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("vcp_body_session_close", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── sovereign_timeline ────────────────────────────────────────────────────────

pub struct SovereignTimelineTool;

#[async_trait]
impl Tool for SovereignTimelineTool {
    fn name(&self) -> &str {
        "sovereign_timeline"
    }
    fn description(&self) -> &str {
        "Return the 4D provenance timeline for a twin — ordered snapshots of every capture, \
         with quality scores and Odù tile locations."
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "twin_id": { "type": "string", "description": "Twin or device ID" }
            },
            "required": ["twin_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("sovereign_timeline", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── sovereign_timeline_diff ───────────────────────────────────────────────────

pub struct SovereignTimelineDiffTool;

#[async_trait]
impl Tool for SovereignTimelineDiffTool {
    fn name(&self) -> &str {
        "sovereign_timeline_diff"
    }
    fn description(&self) -> &str {
        "Compute 4D change detection between earliest and latest snapshot of a twin's timeline. \
         Returns quality delta, added modalities, and time span. Requires ≥2 snapshots."
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "twin_id": { "type": "string" }
            },
            "required": ["twin_id"]
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let args: Value = serde_json::from_str(params).unwrap_or(json!({}));
        let result = mcp_call("sovereign_timeline_diff", args).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── sovereign_ip_root ─────────────────────────────────────────────────────────

pub struct SovereignIpRootTool;

#[async_trait]
impl Tool for SovereignIpRootTool {
    fn name(&self) -> &str {
        "sovereign_ip_root"
    }
    fn description(&self) -> &str {
        "Return the node's cached IP Root event (Nostr kind 31900). \
         This establishes the agent's provenance identity on Nostr. \
         Returns status=not_configured if no Nostr identity is set."
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(json!({ "type": "object", "properties": {}, "required": [] }))
    }
    async fn execute(
        &self,
        _params: &str,
        _ctx: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let result = mcp_call("sovereign_ip_root", json!({})).await?;
        Ok((result, TokenUsage::default()))
    }
}

// ── registry helper ───────────────────────────────────────────────────────────

/// Returns all 12 sovereign-node MCP tools, registered when SOVEREIGN_NODE_URL is set.
pub fn sovereign_node_tools() -> Vec<Box<dyn Tool>> {
    vec![
        // Core capture pipeline
        Box::new(VcpNearbyDevicesTool),
        Box::new(VcpConnectTool),
        Box::new(VcpCaptureTool),
        // Fine-grained VCP body session control
        Box::new(VcpBodySessionOpenTool),
        Box::new(VcpBodySessionCommandTool),
        Box::new(VcpBodySessionCloseTool),
        // Job + node status
        Box::new(SovereignStatusTool),
        Box::new(SovereignJobStatusTool),
        // DIP messaging
        Box::new(DipSendTool),
        // 4D provenance
        Box::new(SovereignTimelineTool),
        Box::new(SovereignTimelineDiffTool),
        // IP identity
        Box::new(SovereignIpRootTool),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_count_is_twelve() {
        assert_eq!(sovereign_node_tools().len(), 12);
    }

    #[test]
    fn tool_names_are_correct() {
        let tools = sovereign_node_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        // Core 6
        assert!(names.contains(&"vcp_nearby_devices"));
        assert!(names.contains(&"vcp_connect"));
        assert!(names.contains(&"vcp_capture"));
        assert!(names.contains(&"sovereign_status"));
        assert!(names.contains(&"sovereign_job_status"));
        assert!(names.contains(&"dip_send"));
        // Phase 4 additions
        assert!(names.contains(&"vcp_body_session_open"));
        assert!(names.contains(&"vcp_body_session_command"));
        assert!(names.contains(&"vcp_body_session_close"));
        assert!(names.contains(&"sovereign_timeline"));
        assert!(names.contains(&"sovereign_timeline_diff"));
        assert!(names.contains(&"sovereign_ip_root"));
    }

    #[test]
    fn write_ops_are_correct() {
        assert!(VcpCaptureTool.is_write_operation());
        assert!(DipSendTool.is_write_operation());
        assert!(VcpBodySessionOpenTool.is_write_operation());
        assert!(VcpBodySessionCommandTool.is_write_operation());
        assert!(VcpBodySessionCloseTool.is_write_operation());
        assert!(!VcpNearbyDevicesTool.is_write_operation());
        assert!(!SovereignStatusTool.is_write_operation());
        assert!(!SovereignJobStatusTool.is_write_operation());
        assert!(!SovereignTimelineTool.is_write_operation());
        assert!(!SovereignTimelineDiffTool.is_write_operation());
        assert!(!SovereignIpRootTool.is_write_operation());
    }

    #[test]
    fn tiers_are_set() {
        assert_eq!(SovereignStatusTool.required_tier(), 1);
        assert_eq!(VcpCaptureTool.required_tier(), 2);
        assert_eq!(DipSendTool.required_tier(), 2);
        assert_eq!(VcpBodySessionOpenTool.required_tier(), 2);
        assert_eq!(SovereignTimelineTool.required_tier(), 1);
        assert_eq!(SovereignIpRootTool.required_tier(), 1);
    }
}
