use crate::tools::{ExecutionContext, Tool};
use async_trait::async_trait;

/// Sovereign Tier Tool List — 18 capabilities for tier-4 agents.
/// These tools require the highest reputation tier (Sovereign).
pub struct ApplyPatchTool;
#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }
    fn description(&self) -> &str {
        "Apply structured patches across multiple files"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct ExecTool;
#[async_trait]
impl Tool for ExecTool {
    fn name(&self) -> &str {
        "exec"
    }
    fn description(&self) -> &str {
        "Run shell commands in the workspace"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct ProcessTool;
#[async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &str {
        "process"
    }
    fn description(&self) -> &str {
        "Manage background execution sessions"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct WebSearchTool;
#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Search the web using sovereign-approved engines"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct WebFetchTool;
#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn description(&self) -> &str {
        "Fetch a URL and return readable text. Params: a URL, or JSON {\"url\": \"...\"}. SSRF-guarded; redirects are not followed automatically."
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let url = extract_fetch_url(params)?;
        // The SSRF guard was previously never called from any tool, so
        // `security.ssrf_guard` was decorative. Every outbound fetch goes
        // through it now.
        crate::tools::net_guard::guard_url(&url)?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            // Do NOT follow redirects: a 302 to 127.0.0.1 or to an internal
            // host would otherwise bypass the guard, which only saw the
            // original URL. The caller re-issues and gets re-checked.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("http client: {e}"))?;

        let resp = client
            .get(&url)
            .header("User-Agent", "omo-koda/0.1 (+web_fetch)")
            .send()
            .await
            .map_err(|e| format!("fetch failed: {e}"))?;

        let status = resp.status();
        if status.is_redirection() {
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("<no Location header>")
                .to_string();
            return Err(format!(
                "fetch {url} -> HTTP {status} redirect to {location}; not followed automatically (re-issue web_fetch on the target so it is SSRF-checked)"
            ));
        }
        if !status.is_success() {
            return Err(format!("fetch {url} -> HTTP {status}"));
        }

        let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
        let text = html_to_text(&body);
        let trimmed: String = text.trim().chars().take(12_000).collect();
        if trimmed.is_empty() {
            return Err(format!("fetch {url} -> empty body"));
        }
        Ok((trimmed, crate::usage::TokenUsage::default()))
    }
}

pub struct BrowserTool;
#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }
    fn description(&self) -> &str {
        "Control the managed sovereign browser"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct CanvasTool;
#[async_trait]
impl Tool for CanvasTool {
    fn name(&self) -> &str {
        "canvas"
    }
    fn description(&self) -> &str {
        "Drive the node Canvas (A2UI, present, eval)"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct NodesTool;
#[async_trait]
impl Tool for NodesTool {
    fn name(&self) -> &str {
        "nodes"
    }
    fn description(&self) -> &str {
        "Discover and target paired nodes"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct ImageTool;
#[async_trait]
impl Tool for ImageTool {
    fn name(&self) -> &str {
        "image"
    }
    fn description(&self) -> &str {
        "Analyze an image with vision models"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct MessageTool;
#[async_trait]
impl Tool for MessageTool {
    fn name(&self) -> &str {
        "message"
    }
    fn description(&self) -> &str {
        "Send and manage cross-channel messages"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct CronTool;
#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }
    fn description(&self) -> &str {
        "Manage gateway cron jobs and wakeups"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct GatewayTool;
#[async_trait]
impl Tool for GatewayTool {
    fn name(&self) -> &str {
        "gateway"
    }
    fn description(&self) -> &str {
        "Manage the sovereign gateway process"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct SessionsListTool;
#[async_trait]
impl Tool for SessionsListTool {
    fn name(&self) -> &str {
        "sessions_list"
    }
    fn description(&self) -> &str {
        "List active agent sessions"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct SessionsHistoryTool;
#[async_trait]
impl Tool for SessionsHistoryTool {
    fn name(&self) -> &str {
        "sessions_history"
    }
    fn description(&self) -> &str {
        "Inspect session transcript history"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct SessionsSendTool;
#[async_trait]
impl Tool for SessionsSendTool {
    fn name(&self) -> &str {
        "sessions_send"
    }
    fn description(&self) -> &str {
        "Send message to another session"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct SessionsSpawnTool;
#[async_trait]
impl Tool for SessionsSpawnTool {
    fn name(&self) -> &str {
        "sessions_spawn"
    }
    fn description(&self) -> &str {
        "Spawn a sub-agent session"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct SessionStatusTool;
#[async_trait]
impl Tool for SessionStatusTool {
    fn name(&self) -> &str {
        "session_status"
    }
    fn description(&self) -> &str {
        "Get status of an agent session"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

pub struct AgentsListTool;
#[async_trait]
impl Tool for AgentsListTool {
    fn name(&self) -> &str {
        "agents_list"
    }
    fn description(&self) -> &str {
        "List available sovereign agent IDs"
    }
    fn required_tier(&self) -> u8 {
        5
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        _params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        Err(format!(
            "{} is registered but has no implementation in this build — nothing was executed. Do not report this call as a success.",
            self.name()
        ))
    }
}

/// Returns the capabilities available to a Tier 5 (Sovereign) agent.
///
/// Reaching T5 requires Proof-of-Evolution across all 5 domains — not
/// mere possession of these capabilities.  See CANONICAL_PILLAR_CONTRACT.md.
/// Accept either a bare URL or `{"url": "..."}` (the shape the model emits).
fn extract_fetch_url(params: &str) -> Result<String, String> {
    let trimmed = params.trim();
    if trimmed.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("invalid web_fetch params: {e}"))?;
        let url = v
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| "web_fetch requires {\"url\": \"https://...\"}".to_string())?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("web_fetch only accepts http(s) URLs".to_string());
        }
        Ok(url.to_string())
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(trimmed.to_string())
    } else {
        Err("web_fetch only accepts http(s) URLs".to_string())
    }
}

/// Crude HTML -> text: drop script/style bodies, strip tags, unescape the five
/// XML entities, collapse blank lines. Enough for an agent to read a page.
fn html_to_text(html: &str) -> String {
    use std::sync::LazyLock;
    static SCRIPT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?is)<(script|style|noscript)[^>]*>.*?</(script|style|noscript)>")
            .expect("static regex")
    });
    static TAG: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?s)<[^>]+>").expect("static regex"));
    static BLANK: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\n\s*\n\s*\n+").expect("static regex"));

    let without_scripts = SCRIPT.replace_all(html, " ");
    let without_tags = TAG.replace_all(&without_scripts, " ");
    let unescaped = without_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let collapsed = BLANK.replace_all(&unescaped, "\n\n");
    collapsed
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn sovereign_tool_list() -> Vec<&'static str> {
    vec![
        "apply_patch",         // 1  — multi-file edit
        "bash_unrestricted",   // 2  — full bash without restrictions
        "git_operations",      // 3  — commit, push, branch
        "file_system_write",   // 4  — unrestricted write
        "network_request",     // 5  — HTTP, WebSocket
        "process_spawn",       // 6  — spawn subprocesses
        "code_execution",      // 7  — execute arbitrary code
        "agent_orchestration", // 8  — spawn sub-agents
        "self_modification",   // 9  — modify own code
        "multi_agent_fabric",  // 10 — coordinate agent swarms
        "browser_automation",  // 11 — control browser
        "database_access",     // 12 — read/write databases
        "api_integration",     // 13 — connect to external APIs
        "voice_interface",     // 14 — speech I/O
        "image_generation",    // 15 — generate images
        "video_generation",    // 16 — generate video
        "sensor_access",       // 17 — read device sensors
        "physical_control",    // 18 — Unitree G1 autonomous embodiment
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sovereign_tool_list_has_18_capabilities() {
        assert_eq!(
            sovereign_tool_list().len(),
            18,
            "Sovereign tier must expose exactly 18 capabilities"
        );
    }
}
