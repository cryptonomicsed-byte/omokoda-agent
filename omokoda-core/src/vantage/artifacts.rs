use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ArtifactKind {
    Code,
    Doc,
    Data,
    Media,
    Eval,
    ToolOutput,
    Other,
}

impl ArtifactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArtifactKind::Code => "code",
            ArtifactKind::Doc => "doc",
            ArtifactKind::Data => "data",
            ArtifactKind::Media => "media",
            ArtifactKind::Eval => "eval",
            ArtifactKind::ToolOutput => "tool_output",
            ArtifactKind::Other => "other",
        }
    }
}

pub struct ArtifactPayload {
    pub task_id: String,
    pub kind: ArtifactKind,
    pub title: String,
    pub content_text: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArtifactResponse {
    pub id: String,
    pub task_id: String,
    pub status: String,
}

pub struct ReceiptPayload {
    pub omokoda_receipt_id: Option<String>,
    pub receipt_body: String,
}
