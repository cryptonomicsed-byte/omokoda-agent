use super::client::WorkspaceClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VantageTask {
    pub id: String,
    pub guild_slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub kind_tag: String,
    pub created_by_name: String,
    pub claimed_by_name: Option<String>,
}

fn default_priority() -> u8 {
    50
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Proposed,
    Claimed,
    Executing,
    Blocked,
    Review,
    Accepted,
    Rejected,
    Cancelled,
    Unknown(String),
}

impl From<&str> for TaskStatus {
    fn from(s: &str) -> Self {
        match s {
            "proposed" => TaskStatus::Proposed,
            "claimed" => TaskStatus::Claimed,
            "executing" => TaskStatus::Executing,
            "blocked" => TaskStatus::Blocked,
            "review" => TaskStatus::Review,
            "accepted" => TaskStatus::Accepted,
            "rejected" => TaskStatus::Rejected,
            "cancelled" => TaskStatus::Cancelled,
            other => TaskStatus::Unknown(other.to_string()),
        }
    }
}

impl WorkspaceClient {
    /// Poll for tasks available to claim (status=proposed).
    pub async fn poll_available_tasks(&self) -> Result<Vec<VantageTask>, String> {
        let url = format!("{}?status=proposed&limit=20", self.task_base());
        let val = self.get(&url).await?;
        let tasks = val["tasks"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Ok(tasks)
    }

    /// Claim a specific task. Fetches the full task after claiming.
    pub async fn claim_task(&self, task_id: &str) -> Result<VantageTask, String> {
        let url = format!("{}/{}/claim", self.task_base(), task_id);
        self.post_form(&url, &[]).await?;
        let task_url = format!("{}/{}", self.task_base(), task_id);
        let val = self.get(&task_url).await?;
        serde_json::from_value(val["task"].clone()).map_err(|e| e.to_string())
    }

    /// Release a claimed task back to proposed.
    pub async fn release_task(&self, task_id: &str, note: &str) -> Result<(), String> {
        let url = format!("{}/{}/release", self.task_base(), task_id);
        self.post_form(&url, &[("note", note)]).await?;
        Ok(())
    }

    /// Submit an artifact for a task. Returns the artifact_id.
    pub async fn submit_artifact(
        &self,
        task_id: &str,
        kind: &str,
        title: &str,
        content_text: &str,
        content_hash: &str,
    ) -> Result<String, String> {
        let url = format!("{}/{}/submit", self.task_base(), task_id);
        let val = self
            .post_form(
                &url,
                &[
                    ("artifact_kind", kind),
                    ("artifact_title", title),
                    ("content_text", content_text),
                    ("content_hash", content_hash),
                ],
            )
            .await?;
        Ok(val["artifact_id"].as_str().unwrap_or("").to_string())
    }

    /// Attach an Omo-Koda2 ActReceipt to the most recent artifact for this task.
    pub async fn attach_receipt(
        &self,
        task_id: &str,
        receipt_body: &str,
        omokoda_receipt_id: &str,
    ) -> Result<(), String> {
        let url = format!("{}/{}/receipt", self.task_base(), task_id);
        self.post_form(
            &url,
            &[
                ("receipt_body", receipt_body),
                ("omokoda_receipt_id", omokoda_receipt_id),
            ],
        )
        .await?;
        Ok(())
    }
}
