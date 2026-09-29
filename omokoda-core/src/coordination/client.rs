//! HTTP client for a Vantage instance's coordination layer.
//!
//! Every call here maps to one endpoint under `/api/guilds`. The shapes were
//! read off the routes rather than assumed: the message and join paths take
//! form fields, the receipt path takes JSON, and the difference is not
//! cosmetic — sending a form body to the receipt endpoint would arrive as
//! nothing at all.

use crate::coordination::presence::WorkState;
use crate::coordination::work_ref::WorkRef;
use crate::receipt::Receipt;
use nostr::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// What a message in a guild channel is doing.
///
/// `System` is missing on purpose: Vantage reserves it for its own
/// orchestrator and drops one signed by anybody else, so a variant for it
/// here would only be a way to build a message that gets silently discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Say,
    Propose,
    Claim,
    Handoff,
    Artifact,
}

impl MessageType {
    pub fn as_str(self) -> &'static str {
        match self {
            MessageType::Say => "say",
            MessageType::Propose => "propose",
            MessageType::Claim => "claim",
            MessageType::Handoff => "handoff",
            MessageType::Artifact => "artifact",
        }
    }

    /// Whether Vantage will try to resolve a work reference on this type.
    /// Attaching one to a `say` is not an error, it is simply ignored.
    pub fn carries_work_ref(self) -> bool {
        matches!(
            self,
            MessageType::Claim | MessageType::Artifact | MessageType::Propose
        )
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    #[error("transport: {0}")]
    Transport(String),
    /// The instance answered, and said no. The body is its explanation, and
    /// it is worth surfacing verbatim — Vantage's refusals name which check
    /// failed, which is the difference between a fixable error and a
    /// mysterious one.
    #[error("vantage refused ({status}): {body}")]
    Refused { status: u16, body: String },
    #[error("unexpected response shape: {0}")]
    Malformed(String),
    #[error("signing: {0}")]
    Signing(String),
    #[error("this reference cannot close a task: {0} is recorded but never verified")]
    UnverifiableWorkRef(WorkRef),
}

#[derive(Debug, Clone, Deserialize)]
pub struct JoinChallenge {
    pub challenge: String,
    pub kind: u16,
    #[serde(default)]
    pub relay: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostedMessage {
    pub event_id: String,
    #[serde(default)]
    pub msg_type: String,
    #[serde(default)]
    pub thread_root_event_id: Option<String>,
    /// What became of the work reference, if the message carried one. `None`
    /// means Vantage resolved nothing — worth checking rather than assuming
    /// a claim landed.
    #[serde(default)]
    pub work_ref_link: Option<WorkRefLink>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkRefLink {
    pub work_ref: String,
    pub verified: bool,
    /// Whether the underlying row actually moved. False with a `note` saying
    /// why is the interesting case: the task was already claimed, or claimed
    /// by somebody else.
    pub transitioned: bool,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReceiptAccepted {
    pub receipt_id: String,
    pub accepted: bool,
    #[serde(default)]
    pub chain_position: String,
    #[serde(default)]
    pub attestation_event_id: Option<String>,
}

/// A client bound to one guild on one instance.
pub struct CoordinationClient {
    base_url: String,
    guild: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl CoordinationClient {
    pub fn new(base_url: impl Into<String>, guild: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            guild: guild.into(),
            api_key: None,
            http: reqwest::Client::new(),
        }
    }

    /// Authenticate as an agent this instance hosts.
    ///
    /// An agent that holds its own key does not need one of these: it joins
    /// by keypair proof and publishes to the relay directly. The API key path
    /// exists for a kernel running *inside* someone's instance.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/guilds/{}{}", self.base_url, self.guild, path)
    }

    fn authed(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => builder.header("X-Agent-Key", key),
            None => builder,
        }
    }

    async fn read<T: for<'de> Deserialize<'de>>(
        response: reqwest::Response,
    ) -> Result<T, CoordinationError> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(CoordinationError::Refused {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str(&body)
            .map_err(|e| CoordinationError::Malformed(format!("{e}: {body}")))
    }

    // ── joining ─────────────────────────────────────────────────────────────

    /// Step one of the keypair handshake: ask for a challenge.
    pub async fn join_request(
        &self,
        keys: &Keys,
        display_name: &str,
        capabilities: &[String],
    ) -> Result<JoinChallenge, CoordinationError> {
        let form = [
            ("pubkey", keys.public_key().to_hex()),
            ("display_name", display_name.to_string()),
            ("framework", "omokoda".to_string()),
            (
                "capabilities",
                serde_json::to_string(capabilities).unwrap_or_else(|_| "[]".into()),
            ),
        ];
        let response = self
            .http
            .post(self.url("/join-request"))
            .form(&form)
            .send()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        Self::read(response).await
    }

    /// Step two: sign the challenge and present the proof.
    ///
    /// The secret half never leaves this process — what crosses the wire is
    /// a signed event, which is the entire reason the handshake is shaped
    /// this way.
    pub async fn join_confirm(
        &self,
        keys: &Keys,
        challenge: &JoinChallenge,
    ) -> Result<serde_json::Value, CoordinationError> {
        let event = EventBuilder::new(Kind::Custom(challenge.kind), "")
            .tag(Tag::custom(
                TagKind::Custom("relay".into()),
                [challenge.relay.clone()],
            ))
            .tag(Tag::custom(
                TagKind::Custom("challenge".into()),
                [challenge.challenge.clone()],
            ))
            .sign(keys)
            .await
            .map_err(|e| CoordinationError::Signing(e.to_string()))?;

        let signed =
            serde_json::to_value(&event).map_err(|e| CoordinationError::Signing(e.to_string()))?;
        let response = self
            .http
            .post(self.url("/join-confirm"))
            .json(&serde_json::json!({ "signed_event": signed }))
            .send()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        Self::read(response).await
    }

    // ── working ─────────────────────────────────────────────────────────────

    /// Post into a channel.
    pub async fn post(
        &self,
        channel: &str,
        content: &str,
        msg_type: MessageType,
        work_ref: Option<&WorkRef>,
    ) -> Result<PostedMessage, CoordinationError> {
        let mut form = vec![
            ("content".to_string(), content.to_string()),
            ("msg_type".to_string(), msg_type.as_str().to_string()),
        ];
        if let Some(reference) = work_ref {
            form.push(("work_ref".to_string(), reference.to_string()));
        }
        let response = self
            .authed(
                self.http
                    .post(self.url(&format!("/channels/{channel}/messages")))
                    .form(&form),
            )
            .send()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        Self::read(response).await
    }

    /// Claim a unit of work.
    ///
    /// Refuses a git reference before it reaches the wire. A commit is
    /// recorded and attributed but never verified, so claiming one cannot
    /// mark anything claimed — the post would succeed and do nothing, which
    /// is the failure mode hardest to notice.
    pub async fn claim(
        &self,
        channel: &str,
        work_ref: &WorkRef,
        note: &str,
    ) -> Result<PostedMessage, CoordinationError> {
        if !work_ref.is_verifiable() {
            return Err(CoordinationError::UnverifiableWorkRef(work_ref.clone()));
        }
        let content = if note.is_empty() {
            format!("claiming {work_ref}")
        } else {
            note.to_string()
        };
        self.post(channel, &content, MessageType::Claim, Some(work_ref))
            .await
    }

    /// Deliver. Only the principal holding the claim can close it, so this
    /// failing with `transitioned: false` and a note means somebody else got
    /// there first — a real answer, not an error.
    pub async fn deliver(
        &self,
        channel: &str,
        work_ref: &WorkRef,
        summary: &str,
    ) -> Result<PostedMessage, CoordinationError> {
        self.post(channel, summary, MessageType::Artifact, Some(work_ref))
            .await
    }

    /// Declare what this agent is doing.
    pub async fn set_state(
        &self,
        state: WorkState,
        detail: &str,
        work_ref: Option<&WorkRef>,
    ) -> Result<serde_json::Value, CoordinationError> {
        let mut form = vec![
            ("state".to_string(), state.as_str().to_string()),
            ("detail".to_string(), detail.to_string()),
        ];
        if let Some(reference) = work_ref {
            form.push(("work_ref".to_string(), reference.to_string()));
        }
        let response = self
            .authed(self.http.put(self.url("/presence")).form(&form))
            .send()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        Self::read(response).await
    }

    // ── proving ─────────────────────────────────────────────────────────────

    /// Pin this kernel's receipt-signing key with the instance.
    ///
    /// Once, before the first receipt. The instance pins it on first use and
    /// will not let a second key quietly replace it, so this being called
    /// twice with different keys is a rotation and should be deliberate.
    pub async fn register_receipt_key(
        &self,
        verifying_key: &ed25519_dalek::VerifyingKey,
        label: &str,
    ) -> Result<serde_json::Value, CoordinationError> {
        let form = [
            ("pubkey", hex::encode(verifying_key.to_bytes())),
            ("label", label.to_string()),
        ];
        let response = self
            .authed(self.http.post(self.url("/receipt-keys")).form(&form))
            .send()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        Self::read(response).await
    }

    /// Submit a receipt against work this agent claimed.
    ///
    /// The receipt goes verbatim. Reshaping it to suit an HTTP body would
    /// mean the instance verifying something other than what was signed, and
    /// every check on the far side recomputes the id from these exact fields.
    pub async fn submit_receipt(
        &self,
        channel: &str,
        receipt: &Receipt,
        work_ref: Option<&WorkRef>,
        artifact_event_id: Option<&str>,
    ) -> Result<ReceiptAccepted, CoordinationError> {
        let body = serde_json::json!({
            "receipt": receipt,
            "work_ref": work_ref.map(|r| r.to_string()).unwrap_or_default(),
            "artifact_event_id": artifact_event_id.unwrap_or_default(),
        });
        let response = self
            .authed(
                self.http
                    .post(self.url(&format!("/channels/{channel}/receipts")))
                    .json(&body),
            )
            .send()
            .await
            .map_err(|e| CoordinationError::Transport(e.to_string()))?;
        Self::read(response).await
    }

    /// Deliver and prove in one step: post the artifact, then bind the
    /// receipt to the event it produced.
    ///
    /// Ordered this way because a receipt naming an artifact that does not
    /// exist proves nothing, and the artifact's event id is only known after
    /// the post.
    pub async fn deliver_with_receipt(
        &self,
        channel: &str,
        work_ref: &WorkRef,
        summary: &str,
        receipt: &Receipt,
    ) -> Result<(PostedMessage, ReceiptAccepted), CoordinationError> {
        let posted = self.deliver(channel, work_ref, summary).await?;
        let accepted = self
            .submit_receipt(channel, receipt, Some(work_ref), Some(&posted.event_id))
            .await?;
        Ok((posted, accepted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::work_ref::WorkKind;

    #[test]
    fn message_types_use_vantages_spelling() {
        let names: Vec<&str> = [
            MessageType::Say,
            MessageType::Propose,
            MessageType::Claim,
            MessageType::Handoff,
            MessageType::Artifact,
        ]
        .iter()
        .map(|m| m.as_str())
        .collect();
        assert_eq!(
            names,
            vec!["say", "propose", "claim", "handoff", "artifact"]
        );
    }

    #[test]
    fn there_is_no_system_message_type() {
        // Vantage reserves it for its own orchestrator and drops one signed
        // by anybody else. A variant here would only build a message that
        // gets silently discarded.
        let json = serde_json::to_string(&MessageType::Say).unwrap();
        assert!(!json.contains("system"));
    }

    #[test]
    fn only_the_work_carrying_types_carry_a_reference() {
        assert!(MessageType::Claim.carries_work_ref());
        assert!(MessageType::Artifact.carries_work_ref());
        assert!(!MessageType::Say.carries_work_ref());
        assert!(!MessageType::Handoff.carries_work_ref());
    }

    #[tokio::test]
    async fn claiming_a_commit_is_refused_before_it_reaches_the_wire() {
        // The post would succeed and move nothing, which is the failure mode
        // hardest to notice.
        let client = CoordinationClient::new("http://127.0.0.1:1", "guild");
        let reference = WorkRef::new(WorkKind::Commit, "9f3a1c0").unwrap();
        let err = client.claim("code", &reference, "").await.unwrap_err();
        assert!(matches!(err, CoordinationError::UnverifiableWorkRef(_)));
    }

    #[test]
    fn the_url_is_built_from_the_guild_not_interpolated_by_the_caller() {
        let client = CoordinationClient::new("https://vantage.example/", "builders");
        assert_eq!(
            client.url("/presence"),
            "https://vantage.example/api/guilds/builders/presence"
        );
    }

    #[test]
    fn a_refusal_carries_the_instances_own_explanation() {
        // Vantage's refusals name which check failed. Swallowing the body
        // would turn a fixable error into a mysterious one.
        let err = CoordinationError::Refused {
            status: 422,
            body: "tro:9 is claimed by another principal".into(),
        };
        assert!(err.to_string().contains("claimed by another principal"));
    }
}
