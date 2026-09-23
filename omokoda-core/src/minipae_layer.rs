//! minipae NIP-AE birth-write hook — publishes a NIP-AE kind 30078 event
//! under the agent's minipae identity at birth.
//!
//! Pattern mirrors ip_layer::publish_ip_root: fail-open, never blocks birth,
//! uses the same sign_and_publish convention.
//!
//! Relay: MINIPAE_RELAY_URL → BUZZ_RELAY_URL → localhost fallback.
//! HTTP:  MINIPAE_URL/write  — posts genesis engram JSON to the minipae
//!        Python service. Fail-open: service unreachable never blocks birth.
//! Key: agent's minipae secp256k1 key derived from the mnemonic
//! (m/44'/30174'/<agent_index>'/<owner_index>') via identity::wallet.

use nostr_sdk::prelude::*;
use serde_json::json;

use crate::identity::wallet::derive_minipae_key;

/// NIP-AE addressable application data kind.
const MINIPAE_BIRTH_KIND: u16 = 30078;

fn relay_url() -> String {
    std::env::var("MINIPAE_RELAY_URL")
        .or_else(|_| std::env::var("BUZZ_RELAY_URL"))
        .unwrap_or_else(|_| "ws://localhost:3000".to_string())
}

/// Base URL for the minipae Python HTTP service.
/// Defaults to localhost:30174 (the minipae kind number as port).
fn minipae_http_url() -> String {
    std::env::var("MINIPAE_URL")
        .unwrap_or_else(|_| "http://localhost:30174".to_string())
        .trim_end_matches('/')
        .to_string()
}

async fn sign_and_publish(keys: Keys, builder: EventBuilder) -> Option<String> {
    let event = builder.sign(&keys).await.ok()?;

    let client = Client::new(keys);
    client.add_relay(&relay_url()).await.ok()?;
    client.connect().await;

    let output = client.send_event(&event).await.ok();
    client.disconnect().await;

    match output {
        Some(o) if !o.success.is_empty() => Some(o.id().to_hex()),
        _ => None,
    }
}

/// POST the genesis engram to the minipae Python service at MINIPAE_URL/write.
///
/// Content: agent_id, npub, bipon39_mnemonic_hash (NOT the mnemonic itself),
/// birth_timestamp, odu_base.  Fail-open — service unreachable returns None.
pub async fn post_genesis_engram_http(
    agent_id: &str,
    npub: &str,
    mnemonic: &str,
    birth_timestamp_secs: i64,
    odu_base: u8,
) -> Option<String> {
    // Hash the mnemonic — never expose the raw mnemonic over HTTP.
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(mnemonic.as_bytes());
    let bipon39_mnemonic_hash = hex::encode(h.finalize());

    let body = json!({
        "agent_id":               agent_id,
        "npub":                   npub,
        "bipon39_mnemonic_hash":  bipon39_mnemonic_hash,
        "birth_timestamp":        birth_timestamp_secs,
        "odu_base":               odu_base,
        "kind":                   "genesis",
        "path":                   "mem/birth/genesis",
    });

    let url = format!("{}/write", minipae_http_url());
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(4))
        .send()
        .await
        .ok()?;

    if resp.status().is_success() {
        // Return the engram id if the service echoes one, otherwise "ok".
        let text = resp.text().await.unwrap_or_else(|_| "ok".to_string());
        Some(text)
    } else {
        None
    }
}

/// Publish the agent's minipae birth event (kind 30078) at birth.
/// Also fires a fire-and-forget HTTP POST to the minipae Python service.
/// Returns the real Nostr event id on success, None on any failure — fail-open.
///
/// `mnemonic`: the agent's own birth mnemonic.
/// `agent_name`: display name for the `d` tag / content payload.
/// `genesis_receipt_id`: genesis receipt id to embed as provenance.
/// `npub`: agent's Nostr bech32 public key (for the HTTP engram).
/// `birth_timestamp_secs`: Unix birth time (for the HTTP engram).
/// `odu_base`: primary Odù index 0–255 (for the HTTP engram).
pub async fn publish_minipae_birth(
    mnemonic: &str,
    agent_name: &str,
    genesis_receipt_id: &str,
) -> Option<String> {
    let minipae = derive_minipae_key(mnemonic, "", 0, 0).ok()?;
    let secret_key = SecretKey::from_hex(&minipae.private_key_hex).ok()?;
    let keys = Keys::new(secret_key);

    let content = json!({
        "kind": "birth",
        "path": "mem/birth/genesis",
        "agent_name": agent_name,
        "genesis_receipt_id": genesis_receipt_id,
        "framework": "omo-koda2",
    })
    .to_string();

    let builder = EventBuilder::new(Kind::Custom(MINIPAE_BIRTH_KIND), content)
        .tag(Tag::identifier(format!("birth:{agent_name}")))
        .tag(Tag::custom(TagKind::custom("genesis"), vec![genesis_receipt_id.to_string()]));

    sign_and_publish(keys, builder).await
}

/// Full birth write: Nostr relay publish (kind 30078) + HTTP POST to minipae
/// Python service at MINIPAE_URL/write.  Both are fail-open.
///
/// Returns the Nostr event id if the relay accepted it (None otherwise).
/// The HTTP POST result is fire-and-forget.
pub async fn publish_minipae_birth_full(
    mnemonic: &str,
    agent_id: &str,
    agent_name: &str,
    genesis_receipt_id: &str,
    npub: &str,
    birth_timestamp_secs: i64,
    odu_base: u8,
) -> Option<String> {
    // HTTP engram to minipae Python service — fire-and-forget.
    {
        let mnemonic_c  = mnemonic.to_string();
        let agent_id_c  = agent_id.to_string();
        let npub_c      = npub.to_string();
        let odu         = odu_base;
        let ts          = birth_timestamp_secs;
        tokio::spawn(async move {
            post_genesis_engram_http(&agent_id_c, &npub_c, &mnemonic_c, ts, odu).await;
        });
    }

    // Nostr relay publish — return the event id.
    publish_minipae_birth(mnemonic, agent_name, genesis_receipt_id).await
}
