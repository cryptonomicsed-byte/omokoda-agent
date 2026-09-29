//! The coordination client against a mock Vantage instance.
//!
//! The unit tests in `coordination::` check the types. These check the wire:
//! that the form-vs-JSON split matches the routes, that a receipt crosses
//! unmodified, and that a claim which moved nothing is reported as such
//! rather than read as success.

use httpmock::prelude::*;
use omokoda_core::coordination::{CoordinationClient, MessageType, WorkRef, WorkState};
use omokoda_core::receipt::Receipt;
use std::sync::OnceLock;

fn client(server: &MockServer) -> CoordinationClient {
    CoordinationClient::new(server.base_url(), "builders").with_api_key("test-key")
}

fn a_receipt() -> Receipt {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    // from_str, not new: AgentId::new hashes a DNA fingerprint and slices
    // its first 16 characters, so a short literal panics.
    Receipt::new_merkle(
        &omokoda_core::AgentId::from_str("agent-1"),
        "run_tests",
        "{}",
        "",
        "root",
        &signing_key,
    )
}

#[tokio::test]
async fn a_claim_posts_a_form_and_reports_the_transition() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/code/messages")
            .header("X-Agent-Key", "test-key")
            .body_contains("msg_type=claim")
            .body_contains("work_ref=tro%3A123");
        then.status(200).json_body(serde_json::json!({
            "event_id": "abc123",
            "msg_type": "claim",
            "work_ref_link": {
                "work_ref": "tro:123", "verified": true,
                "transitioned": true, "note": ""
            }
        }));
    });

    let posted = client(&server)
        .claim("code", &WorkRef::tro(123), "taking this")
        .await
        .expect("claim");

    mock.assert();
    let link = posted
        .work_ref_link
        .expect("a claim should report its link");
    assert!(link.transitioned);
}

#[tokio::test]
async fn a_claim_that_moved_nothing_is_not_read_as_success() {
    // The HTTP call succeeded and the task is still someone else's. Treating
    // a 200 as "I have the work" is exactly the bug typed references exist
    // to make visible.
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/code/messages");
        then.status(200).json_body(serde_json::json!({
            "event_id": "abc123",
            "work_ref_link": {
                "work_ref": "tro:123", "verified": true,
                "transitioned": false, "note": "already matched"
            }
        }));
    });

    let posted = client(&server)
        .claim("code", &WorkRef::tro(123), "")
        .await
        .unwrap();
    let link = posted.work_ref_link.unwrap();
    assert!(!link.transitioned);
    assert_eq!(link.note, "already matched");
}

#[tokio::test]
async fn a_message_with_no_reference_reports_no_link() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/general/messages");
        then.status(200)
            .json_body(serde_json::json!({"event_id": "e1", "msg_type": "say"}));
    });

    let posted = client(&server)
        .post("general", "morning", MessageType::Say, None)
        .await
        .unwrap();
    assert!(posted.work_ref_link.is_none());
}

#[tokio::test]
async fn a_receipt_crosses_as_json_and_unmodified() {
    // Reshaping it would mean the instance verifying something other than
    // what this kernel signed; every check on the far side recomputes the id
    // from these exact fields.
    let receipt = a_receipt();
    let expected_id = receipt.receipt_id.clone();

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/code/receipts")
            .header("content-type", "application/json")
            .json_body_partial(format!(
                r#"{{"receipt": {{"receipt_id": "{expected_id}"}}}}"#
            ));
        then.status(200).json_body(serde_json::json!({
            "receipt_id": expected_id, "accepted": true, "chain_position": "start"
        }));
    });

    let accepted = client(&server)
        .submit_receipt("code", &receipt, Some(&WorkRef::tro(1)), Some("artifact-1"))
        .await
        .expect("receipt");

    mock.assert();
    assert!(accepted.accepted);
    assert_eq!(accepted.receipt_id, expected_id);
}

#[tokio::test]
async fn delivering_with_a_receipt_binds_it_to_the_artifact_that_was_just_posted() {
    // Ordered this way because a receipt naming an artifact that does not
    // exist proves nothing, and the event id is only known after the post.
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/code/messages");
        then.status(200)
            .json_body(serde_json::json!({"event_id": "artifact-99"}));
    });
    let receipt_mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/code/receipts")
            .body_contains("artifact-99");
        then.status(200).json_body(serde_json::json!({
            "receipt_id": "r1", "accepted": true,
            "attestation_event_id": "att-1"
        }));
    });

    let (posted, accepted) = client(&server)
        .deliver_with_receipt("code", &WorkRef::tro(5), "shipped", &a_receipt())
        .await
        .expect("deliver");

    receipt_mock.assert();
    assert_eq!(posted.event_id, "artifact-99");
    assert_eq!(accepted.attestation_event_id.as_deref(), Some("att-1"));
}

#[tokio::test]
async fn a_refusal_surfaces_the_instances_own_reason() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/channels/code/receipts");
        then.status(422)
            .body("tro:5 is claimed by another principal");
    });

    let err = client(&server)
        .submit_receipt("code", &a_receipt(), Some(&WorkRef::tro(5)), None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("claimed by another principal"));
}

#[tokio::test]
async fn presence_is_a_put_with_the_wire_spelling_of_the_state() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(PUT)
            .path("/api/guilds/builders/presence")
            .body_contains("state=needs_review");
        then.status(200)
            .json_body(serde_json::json!({"state": "needs_review", "routable": false}));
    });

    client(&server)
        .set_state(WorkState::NeedsReview, "waiting on review", None)
        .await
        .expect("presence");
    mock.assert();
}

/// The secret the join test must never see on the wire.
///
/// A static rather than a captured variable because httpmock's `matches`
/// takes a plain fn pointer, and the alternative -- asserting only on what
/// the body *does* contain -- would pass just as happily if the client
/// started sending the secret alongside everything else.
static JOIN_SECRET: OnceLock<String> = OnceLock::new();

fn body_carries_no_secret(req: &HttpMockRequest) -> bool {
    let body = req
        .body
        .as_ref()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .unwrap_or_default();
    match JOIN_SECRET.get() {
        Some(secret) => !body.contains(secret),
        None => false,
    }
}

#[tokio::test]
async fn the_join_handshake_never_sends_a_secret() {
    // The whole reason the handshake is shaped this way. What crosses the
    // wire is a public key and then a signed event, and nothing else.
    let keys = nostr::Keys::generate();
    JOIN_SECRET
        .set(keys.secret_key().to_secret_hex())
        .expect("one join test sets this");

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/guilds/builders/join-request");
        then.status(200).json_body(serde_json::json!({
            "challenge": "chal-1", "kind": 22242, "relay": "ws://relay.example"
        }));
    });
    let confirm = server.mock(|when, then| {
        when.method(POST)
            .path("/api/guilds/builders/join-confirm")
            .matches(body_carries_no_secret);
        then.status(200)
            .json_body(serde_json::json!({"joined": true}));
    });

    let client = CoordinationClient::new(server.base_url(), "builders");
    let challenge = client
        .join_request(&keys, "kernel", &["code".into()])
        .await
        .expect("challenge");
    assert_eq!(challenge.kind, 22242);

    client
        .join_confirm(&keys, &challenge)
        .await
        .expect("confirm");
    confirm.assert();
}
