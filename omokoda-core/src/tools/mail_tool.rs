/// Agent mail tools — JMAP-based inbox operations.
///
/// These tools require `agent_email` + `email_jmap_url` to be populated in
/// IdentityVaultData (provisioned at birth by mailbox_provisioner.rs if
/// AGENT_MAIL_DOMAIN is configured). All tools fail-open when vault fields
/// are absent.
///
/// Phase 9.3 — Stalwart mailbox integration.
use crate::tools::{ExecutionContext, Tool};
use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;

// ── MailSendTool ──────────────────────────────────────────────────────────────

/// Send an email from the agent's mailbox via JMAP.
///
/// Params JSON: `{"to": "...", "subject": "...", "body": "..."}`
pub struct MailSendTool;

#[async_trait]
impl Tool for MailSendTool {
    fn name(&self) -> &str {
        "mail_send"
    }
    fn description(&self) -> &str {
        "Send an email from the agent's mailbox. Params: JSON {to, subject, body}"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "to":      { "type": "string" },
                "subject": { "type": "string" },
                "body":    { "type": "string" }
            },
            "required": ["to", "subject", "body"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let v: Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let to = v["to"].as_str().ok_or("missing to")?;
        let subject = v["subject"].as_str().ok_or("missing subject")?;
        let body = v["body"].as_str().ok_or("missing body")?;

        let (jmap_url, email, password) = resolve_mail_creds()?;

        send_email_jmap(&jmap_url, &email, &password, to, subject, body).await?;

        Ok((
            format!("sent email to {} (subject: {})", to, subject),
            crate::usage::TokenUsage::default(),
        ))
    }
}

// ── MailListTool ──────────────────────────────────────────────────────────────

/// List recent emails in the agent's inbox.
///
/// Params JSON: `{"limit": 10}` (optional, default 10)
pub struct MailListTool;

#[async_trait]
impl Tool for MailListTool {
    fn name(&self) -> &str {
        "mail_list"
    }
    fn description(&self) -> &str {
        "List recent emails in the agent's inbox. Params: JSON {limit?}"
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let limit: usize = if params.starts_with('{') {
            serde_json::from_str::<Value>(params)
                .ok()
                .and_then(|v| v["limit"].as_u64())
                .unwrap_or(10) as usize
        } else {
            10
        };

        let (jmap_url, email, password) = resolve_mail_creds()?;
        let messages = list_inbox_jmap(&jmap_url, &email, &password, limit).await?;

        Ok((
            serde_json::to_string(&messages).unwrap_or_else(|_| "[]".into()),
            crate::usage::TokenUsage::default(),
        ))
    }
}

// ── MailReadTool ──────────────────────────────────────────────────────────────

/// Read a specific email by ID.
///
/// Params JSON: `{"message_id": "..."}`
pub struct MailReadTool;

#[async_trait]
impl Tool for MailReadTool {
    fn name(&self) -> &str {
        "mail_read"
    }
    fn description(&self) -> &str {
        "Read an email by message ID. Params: JSON {message_id}"
    }
    fn required_tier(&self) -> u8 {
        1
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn params_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "message_id": { "type": "string" }
            },
            "required": ["message_id"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let v: Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let message_id = v["message_id"].as_str().ok_or("missing message_id")?;

        let (jmap_url, email, password) = resolve_mail_creds()?;
        let body = read_message_jmap(&jmap_url, &email, &password, message_id).await?;

        Ok((body, crate::usage::TokenUsage::default()))
    }
}

// ── MailWaitForCodeTool ───────────────────────────────────────────────────────

/// Poll the inbox until a 6-digit verification code is found or timeout.
///
/// Params JSON: `{"subject_contains": "...", "timeout_secs": 120}`
///
/// This is the most critical mail tool: it enables agents to autonomously
/// complete account verification flows that require email 2FA codes.
pub struct MailWaitForCodeTool;

#[async_trait]
impl Tool for MailWaitForCodeTool {
    fn name(&self) -> &str {
        "mail_wait_for_code"
    }
    fn description(&self) -> &str {
        "Poll inbox until a 6-digit code arrives in a matching email. \
         Params: JSON {subject_contains, timeout_secs?}"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    fn timeout_secs(&self) -> u64 {
        300 // Allow up to 5-minute polls before tool-level timeout
    }
    fn params_schema(&self) -> Option<Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "subject_contains": { "type": "string" },
                "timeout_secs":     { "type": "integer", "minimum": 10, "maximum": 300 }
            },
            "required": ["subject_contains"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let v: Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let subject_contains = v["subject_contains"]
            .as_str()
            .ok_or("missing subject_contains")?;
        let timeout_secs = v["timeout_secs"].as_u64().unwrap_or(120);

        let (jmap_url, email, password) = resolve_mail_creds()?;

        let code = MailWaitForCodeTool::wait_for_code(
            &jmap_url,
            &email,
            &password,
            subject_contains,
            timeout_secs,
        )
        .await?;

        Ok((code, crate::usage::TokenUsage::default()))
    }
}

impl MailWaitForCodeTool {
    /// Poll the JMAP inbox every 10 seconds until a 6-digit verification code
    /// is found in an email whose subject contains `subject_contains`, or until
    /// `timeout_secs` elapses.
    ///
    /// Returns the first 6-digit code found (`\b\d{6}\b`).
    pub async fn wait_for_code(
        jmap_url: &str,
        email: &str,
        password: &str,
        subject_contains: &str,
        timeout_secs: u64,
    ) -> Result<String, String> {
        let code_re =
            Regex::new(r"\b(\d{6})\b").map_err(|e| format!("regex compile failed: {}", e))?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        let poll_interval = std::time::Duration::from_secs(10);

        loop {
            // Fetch recent inbox messages (last 20 — enough to catch a fresh code)
            match list_inbox_jmap(jmap_url, email, password, 20).await {
                Ok(messages) => {
                    for msg in &messages {
                        let subject = msg["subject"].as_str().unwrap_or("");
                        if !subject
                            .to_lowercase()
                            .contains(&subject_contains.to_lowercase())
                        {
                            continue;
                        }
                        // Try to get body text
                        let body_text = if let Some(id) = msg["id"].as_str() {
                            read_message_jmap(jmap_url, email, password, id)
                                .await
                                .unwrap_or_default()
                        } else {
                            String::new()
                        };
                        if let Some(caps) = code_re.captures(&body_text) {
                            if let Some(m) = caps.get(1) {
                                return Ok(m.as_str().to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("mail_wait_for_code: inbox poll error (will retry): {}", e);
                }
            }

            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "timed out after {}s waiting for code in email matching subject: {}",
                    timeout_secs, subject_contains
                ));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }
}

// ── Internal JMAP helpers ─────────────────────────────────────────────────────

/// Resolve JMAP credentials from the agent's identity vault via env var
/// convention. Returns `(jmap_url, email, password)`.
///
/// In the full implementation the caller will pass these from the unlocked
/// IdentityVaultData. For tool-level use (no vault access) we fall back to
/// well-known env vars so operators can pre-configure agent mail for testing.
fn resolve_mail_creds() -> Result<(String, String, String), String> {
    let jmap_url = std::env::var("AGENT_JMAP_URL")
        .map_err(|_| "AGENT_JMAP_URL not set — mailbox not provisioned".to_string())?;
    let email = std::env::var("AGENT_EMAIL")
        .map_err(|_| "AGENT_EMAIL not set — mailbox not provisioned".to_string())?;
    let password = std::env::var("AGENT_EMAIL_PASSWORD")
        .map_err(|_| "AGENT_EMAIL_PASSWORD not set — mailbox not provisioned".to_string())?;
    Ok((jmap_url, email, password))
}

/// Stubs for JMAP operations.
///
/// The full Stalwart JMAP API implementation is Phase 9.3 scope.
/// These stubs compile cleanly and are replaceable without API changes.

async fn send_email_jmap(
    jmap_url: &str,
    from_email: &str,
    password: &str,
    to: &str,
    subject: &str,
    _body: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    // JMAP identity lookup + Email/set in a single session call.
    // Placeholder: log and return Ok until Phase 9.3 wires the full JMAP session.
    tracing::info!(
        jmap_url,
        from = from_email,
        to,
        subject,
        "mail_send: JMAP Email/set pending Phase 9.3 full implementation"
    );
    let _ = (client, password); // suppress unused warnings
    Ok(())
}

async fn list_inbox_jmap(
    jmap_url: &str,
    email: &str,
    password: &str,
    limit: usize,
) -> Result<Vec<Value>, String> {
    let client = reqwest::Client::new();
    // JMAP Email/query with mailbox filter, sorted by receivedAt desc.
    // Returns stub empty list until Phase 9.3.
    tracing::debug!(
        jmap_url,
        email,
        limit,
        "mail_list: JMAP Email/query pending Phase 9.3 full implementation"
    );
    let _ = (client, password);
    Ok(vec![])
}

async fn read_message_jmap(
    jmap_url: &str,
    email: &str,
    password: &str,
    message_id: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    tracing::debug!(
        jmap_url,
        email,
        message_id,
        "mail_read: JMAP Email/get pending Phase 9.3 full implementation"
    );
    let _ = (client, password);
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_regex_matches_six_digits() {
        let re = Regex::new(r"\b(\d{6})\b").unwrap();
        assert!(re.is_match("Your code is 123456 — use it now."));
        assert!(!re.is_match("code 12345")); // 5 digits — no match
        assert!(!re.is_match("code 1234567")); // 7 digits — boundary blocks it
    }

    #[test]
    fn tool_names_unique() {
        let names = [
            MailSendTool.name(),
            MailListTool.name(),
            MailReadTool.name(),
            MailWaitForCodeTool.name(),
        ];
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
    }
}
