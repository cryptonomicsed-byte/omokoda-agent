//! omokoda-acp -- a stdio JSON-RPC 2.0 shim implementing the subset of the
//! Agent Client Protocol (agentclientprotocol.com) needed for an ACP client
//! (e.g. buzz-acp, bridging @mentions in a Buzz room) to address a live
//! Omo-Koda2 agent: initialize, session/new, session/prompt, session/cancel.
//!
//! This is a thin translator, not a reimplementation -- one ACP prompt
//! becomes one real call to the kernel's existing `/v1/cognition` webhook
//! (the same endpoint Vantage Copilot calls), since the kernel already owns
//! the full think+tool loop. No MCP server spawning is needed: the kernel
//! is the agent.
//!
//! Framing: newline-delimited JSON-RPC 2.0 over stdin/stdout (each message
//! is exactly one JSON value on one line), per ACP's stdio transport.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

struct Session {
    #[allow(dead_code)]
    cwd: Option<String>,
}

struct AcpState {
    sessions: Mutex<HashMap<String, Session>>,
    kernel_url: String,
    cognition_token: Option<String>,
    // Set via OMOKODA_AGENT_ID/OMOKODA_AGENT_KEY when this instance is
    // dedicated to one specific (non-owner) Omo-Koda2 agent, e.g. spawned
    // per /buzz-register. Absent => falls back to the original single
    // sovereign-owner routing.
    agent_id: Option<String>,
    agent_key: Option<String>,
    http: reqwest::Client,
}

fn kernel_url() -> String {
    std::env::var("OMOKODA_KERNEL_URL").unwrap_or_else(|_| "http://localhost:7777".to_string())
}

fn cognition_token() -> Option<String> {
    std::env::var("OMOKODA_COGNITION_TOKEN").ok()
}

#[tokio::main]
async fn main() {
    let state = AcpState {
        sessions: Mutex::new(HashMap::new()),
        kernel_url: kernel_url(),
        cognition_token: cognition_token(),
        agent_id: std::env::var("OMOKODA_AGENT_ID").ok(),
        agent_key: std::env::var("OMOKODA_AGENT_KEY").ok(),
        http: reqwest::Client::new(),
    };

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let stdout = std::io::stdout();

    while let Ok(Some(line)) = reader.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("omokoda-acp: malformed JSON-RPC line, skipping: {e}");
                continue;
            }
        };
        handle_message(&state, msg, &stdout).await;
    }
}

fn write_response(stdout: &std::io::Stdout, id: Value, result: Value) {
    let resp = json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", resp);
    let _ = lock.flush();
}

fn write_error(stdout: &std::io::Stdout, id: Value, code: i64, message: &str) {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    });
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", resp);
    let _ = lock.flush();
}

fn write_notification(stdout: &std::io::Stdout, method: &str, params: Value) {
    let note = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", note);
    let _ = lock.flush();
}

async fn handle_message(state: &AcpState, msg: Value, stdout: &std::io::Stdout) {
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    match method {
        "initialize" => {
            let result = json!({
                "protocolVersion": 1,
                "agentCapabilities": {
                    "loadSession": false,
                    "promptCapabilities": { "text": true }
                }
            });
            if let Some(id) = id {
                write_response(stdout, id, result);
            }
        }
        "session/new" => {
            let cwd = params
                .get("cwd")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            let session_id = uuid::Uuid::new_v4().to_string();
            state
                .sessions
                .lock()
                .await
                .insert(session_id.clone(), Session { cwd });
            if let Some(id) = id {
                write_response(stdout, id, json!({ "sessionId": session_id }));
            }
        }
        "session/prompt" => {
            let Some(id) = id else { return };
            let session_id = params
                .get("sessionId")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            if !state.sessions.lock().await.contains_key(&session_id) {
                write_error(stdout, id, -32602, "unknown sessionId");
                return;
            }

            let text = extract_prompt_text(&params);
            if text.trim().is_empty() {
                write_error(stdout, id, -32602, "prompt had no text content");
                return;
            }

            write_notification(
                stdout,
                "session/update",
                json!({
                    "sessionId": session_id,
                    "update": { "sessionUpdate": "agent_thought_chunk", "content": { "type": "text", "text": "thinking..." } }
                }),
            );

            match call_cognition(state, &text).await {
                Ok(reply) => {
                    write_notification(
                        stdout,
                        "session/update",
                        json!({
                            "sessionId": session_id,
                            "update": { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": reply.clone() } }
                        }),
                    );
                    // agent_message_chunk alone is NOT published to Buzz by
                    // the harness -- buzz-acp only logs it. Real agents
                    // (goose/codex/claude-code) publish replies themselves
                    // via `buzz messages send`, using their own shell tool.
                    // We have no tool loop, so we shell out to the same CLI
                    // directly here. Found live 2026-07-25: without this,
                    // agent_returned=ok fires but nothing ever reaches the
                    // relay -- a silent gap, not an error.
                    if let Err(e) = publish_to_buzz(&reply).await {
                        eprintln!("omokoda-acp: buzz-cli publish failed (reply still returned via ACP): {e}");
                    }
                    write_response(stdout, id, json!({ "stopReason": "end_turn" }));
                }
                Err(e) => {
                    write_error(stdout, id, -32000, &format!("kernel dispatch failed: {e}"));
                }
            }
        }
        "session/cancel" => {
            // Notification, no id, no response. Our dispatch is a single
            // blocking HTTP call per prompt (no cancellable sub-steps to
            // interrupt yet) -- best-effort acknowledgment only.
            if let Some(sid) = params.get("sessionId").and_then(|s| s.as_str()) {
                eprintln!("omokoda-acp: cancel requested for session {sid} (no-op, call already in flight or done)");
            }
        }
        other => {
            if let Some(id) = id {
                write_error(stdout, id, -32601, &format!("method not found: {other}"));
            }
        }
    }
}

/// ACP prompt params carry a `prompt` array of content blocks
/// (`{"type":"text","text":"..."}` among others). We only speak text.
fn extract_prompt_text(params: &Value) -> String {
    params
        .get("prompt")
        .and_then(|p| p.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

async fn call_cognition(state: &AcpState, text: &str) -> Result<String, String> {
    let mut body = json!({ "agent_name": "buzz-acp", "text": text });
    if let (Some(id), Some(key)) = (&state.agent_id, &state.agent_key) {
        body["agent_id"] = json!(id);
        body["agent_key"] = json!(key);
    }
    let mut req = state
        .http
        .post(format!("{}/v1/cognition", state.kernel_url))
        .json(&body);
    if let Some(token) = &state.cognition_token {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("{status}: {body}"));
    }
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    body.get("reply")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "no 'reply' field in kernel response".to_string())
}

/// Actually publish `content` to Buzz via `buzz messages send`. Reuses
/// BUZZ_RELAY_URL / BUZZ_PRIVATE_KEY from our own environment (inherited
/// from buzz-acp, which set them when it spawned us -- same identity that
/// joined the channel). Targets the first channel in BUZZ_ACP_CHANNELS
/// (single-channel v1 scope, matching /v1/cognition's single-agent scope).
async fn publish_to_buzz(content: &str) -> Result<(), String> {
    let channel = std::env::var("BUZZ_ACP_CHANNELS")
        .ok()
        .and_then(|s| s.split(',').next().map(|c| c.trim().to_string()))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "BUZZ_ACP_CHANNELS not set, don't know which channel to reply in".to_string()
        })?;
    let cli = std::env::var("BUZZ_CLI_PATH").unwrap_or_else(|_| "buzz".to_string());

    let output = tokio::process::Command::new(&cli)
        .args([
            "messages",
            "send",
            "--channel",
            &channel,
            "--content",
            content,
        ])
        .output()
        .await
        .map_err(|e| format!("failed to spawn '{cli}': {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "buzz messages send exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
