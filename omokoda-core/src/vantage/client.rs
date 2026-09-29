use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;

static HTTP: OnceLock<Client> = OnceLock::new();
fn http() -> &'static Client {
    HTTP.get_or_init(Client::new)
}

pub struct WorkspaceClient {
    pub base_url: String,
    pub api_key: String,
    pub guild_slug: String,
    /// VANTAGE_AGENT_ID — numeric agent id for mesh heartbeat endpoint.
    pub agent_id: Option<String>,
    /// VANTAGE_BLOCK_ID — mesh block the agent is currently in.
    pub block_id: Option<String>,
}

impl WorkspaceClient {
    /// Load from environment variables: VANTAGE_URL, VANTAGE_KEY, VANTAGE_GUILD_SLUG,
    /// VANTAGE_AGENT_ID, VANTAGE_BLOCK_ID.
    /// Returns None if VANTAGE_URL is not set.
    pub fn from_env() -> Option<Self> {
        let base = std::env::var("VANTAGE_URL").ok()?;
        if base.trim().is_empty() {
            return None;
        }
        let api_key = std::env::var("VANTAGE_KEY").unwrap_or_default();
        let guild_slug = std::env::var("VANTAGE_GUILD_SLUG").unwrap_or_default();
        let agent_id = std::env::var("VANTAGE_AGENT_ID")
            .ok()
            .filter(|s| !s.is_empty());
        let block_id = std::env::var("VANTAGE_BLOCK_ID")
            .ok()
            .filter(|s| !s.is_empty());
        Some(Self {
            base_url: base.trim_end_matches('/').to_string(),
            api_key,
            guild_slug,
            agent_id,
            block_id,
        })
    }

    pub fn guild_path(&self, suffix: &str) -> String {
        format!("{}/api/guilds/{}{}", self.base_url, self.guild_slug, suffix)
    }

    pub(crate) fn task_base(&self) -> String {
        self.guild_path("/tasks")
    }

    pub(crate) fn memory_base(&self) -> String {
        self.guild_path("/memory")
    }

    pub async fn get(&self, url: &str) -> Result<Value, String> {
        let resp = http()
            .get(url)
            .header("X-Agent-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("GET {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    pub async fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        let resp = http()
            .post(url)
            .header("X-Agent-Key", &self.api_key)
            .form(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("POST {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    pub async fn delete(&self, url: &str) -> Result<Value, String> {
        let resp = http()
            .delete(url)
            .header("X-Agent-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("DELETE {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    pub async fn post_json(&self, url: &str, body: Value) -> Result<Value, String> {
        let resp = http()
            .post(url)
            .header("X-Agent-Key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("POST {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    /// PUT with a JSON body.
    pub async fn put(&self, url: &str, body: &serde_json::Value) -> Result<Value, String> {
        let resp = http()
            .put(url)
            .header("X-Agent-Key", &self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("PUT {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    /// PATCH with a JSON body.
    pub async fn patch(&self, url: &str, body: &serde_json::Value) -> Result<Value, String> {
        let resp = http()
            .patch(url)
            .header("X-Agent-Key", &self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("PATCH {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    pub async fn put_form(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        let resp = http()
            .put(url)
            .header("X-Agent-Key", &self.api_key)
            .form(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let val: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!("PUT {url} -> {status}: {val}"));
        }
        Ok(val)
    }

    /// POST /me/heartbeat — refreshes agents.last_seen_at so the agent never
    /// appears stale or offline in Vantage's mesh view.  Fire-and-forget: a
    /// network hiccup should not crash the cognitive loop.
    pub async fn heartbeat(&self) -> Result<(), String> {
        let url = format!("{}/api/me/heartbeat", self.base_url);
        let resp = http()
            .post(&url)
            .header("X-Agent-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("heartbeat -> {}", resp.status()));
        }
        Ok(())
    }

    /// POST /api/nodes/heartbeat — sends the full canonical AgentHeartbeat record
    /// (tamper-evident SHA-256 chain) to Vantage so it can verify chain continuity.
    /// Falls back gracefully: a network hiccup must not crash the cognitive loop.
    pub async fn send_heartbeat(
        &self,
        beat: &crate::lifecycle::AgentHeartbeat,
    ) -> Result<(), String> {
        let url = format!("{}/api/nodes/heartbeat", self.base_url);
        let resp = http()
            .post(&url)
            .header("X-Agent-Key", &self.api_key)
            .json(beat)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("send_heartbeat -> {}", resp.status()));
        }
        Ok(())
    }

    /// POST /api/mesh/agents/{agent_id}/heartbeat — refreshes mesh_agents.last_seen_at
    /// so the agent stays active in the mesh block view.  Requires VANTAGE_AGENT_ID
    /// and VANTAGE_BLOCK_ID env vars; skips silently if either is absent.
    pub async fn mesh_heartbeat(&self) -> Result<(), String> {
        let agent_id = match &self.agent_id {
            Some(id) => id.clone(),
            None => return Ok(()),
        };
        let block_id = match &self.block_id {
            Some(id) => id.clone(),
            None => return Ok(()),
        };
        let url = format!("{}/api/mesh/agents/{}/heartbeat", self.base_url, agent_id);
        let resp = http()
            .post(&url)
            .header("X-Agent-Key", &self.api_key)
            .json(&serde_json::json!({ "block_id": block_id }))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("mesh_heartbeat -> {}", resp.status()));
        }
        Ok(())
    }
}
