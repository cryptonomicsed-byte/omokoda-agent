//! ip-layer birth-flow hook — publishes this agent's IP Root (kind 31900,
//! see cryptonomicsed-byte/ip-layer's schemas/ip_root.md) at birth.
//!
//! Real, verified pattern: reuses identity/buzz_relay.rs's proven
//! nostr-sdk Client/EventBuilder/sign/send_event flow (already live for
//! NIP-29 group joins), and mirrors onchain.rs::mint_onchain_agent's
//! fail-open shape -- an unreachable relay or missing config never blocks
//! a birth, it just means no IP Root gets published this time.
//!
//! Signing key: this agent's own NIP-06 secp256k1 identity
//! (identity::nip06::derive_nip06_identity), the same derived key
//! nostr_identity_tool.rs already surfaces (npub only) to agent-visible
//! context -- the IP Root's pubkey is that same identity's public half,
//! so "who signed this" and "who the agent publicly is on Nostr" are the
//! same key, not a second unrelated one.
//!
//! `d` tag (the IP Root's stable addressable id): the agent's own hex
//! pubkey, per ip_root.md's own field note ("For Omo-Koda2-born agents
//! this can be the agent's own Nostr pubkey").

use nostr_sdk::prelude::*;
use serde_json::json;

use crate::identity::nip06::derive_nip06_identity;

/// Nostr kind 31900 — IP Root, per cryptonomicsed-byte/ip-layer's schema.
/// Provisional pending a real NIP submission (see that repo's own
/// open-questions note); this constant is the single source of truth for
/// the number within this kernel.
const IP_ROOT_KIND: u16 = 31900;

/// Nostr kind 1901 — Creation Receipt, per ip-layer's schema.
/// A regular event published at birth recording agent_id, npub,
/// birth_timestamp, and genesis_hash.  One per birth.
const CREATION_RECEIPT_KIND: u16 = 1901;

/// Nostr kind 1902 — Attestation, per ip-layer's schema.
/// Published immediately after the Creation Receipt; attests that the
/// IP Root (kind 31900) was published and links the two events.
const ATTESTATION_KIND: u16 = 1902;

/// Nostr kind 1903 — Twin Binding, per cryptonomicsed-byte/ip-layer's
/// schemas/twin_binding.md. A plain regular (non-addressable) event: a
/// re-binding to a different sim_id is a new event, the old one stands as
/// history, per that schema's own design note.
const TWIN_BINDING_KIND: u16 = 1903;

/// Resolve the relay to publish to: IP_LAYER_RELAY_URL, falling back to
/// BUZZ_RELAY_URL (the kernel's existing Buzz relay, already configured in
/// production), falling back to a local dev default.
fn relay_url() -> String {
    std::env::var("IP_LAYER_RELAY_URL")
        .or_else(|_| std::env::var("BUZZ_RELAY_URL"))
        .unwrap_or_else(|_| "ws://localhost:3000".to_string())
}

/// Sign and publish a single event, returning the real event id only if at
/// least one relay genuinely accepted it. Shared by publish_ip_root and
/// publish_twin_binding.
///
/// Real bug found and fixed here (confirmed live via a direct probe against
/// an unreachable relay): send_event()'s outer Result is Ok(..) even when
/// EVERY relay in Output.failed rejected the event -- the event only
/// genuinely landed somewhere if Output.success is non-empty. Checking only
/// the outer Result (as this kernel's other existing nostr-sdk callers
/// currently do, e.g. identity/buzz_relay.rs's self_join/
/// join_and_chat_mentioning) would silently report success for an event
/// nobody ever received.
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

/// Publish this agent's IP Root at birth. Returns the real event id on
/// success, None on any failure (no relay configured, signing failed,
/// relay unreachable, etc.) -- never propagates an error, matching
/// onchain.rs::mint_onchain_agent's fail-open convention for optional
/// birth-time side effects.
///
/// `mnemonic`: the agent's own birth mnemonic, used to deterministically
/// derive the same NIP-06 identity nostr_identity_tool.rs exposes.
/// `agent_name`: for the event's free-form content field (display_name).
pub async fn publish_ip_root(mnemonic: &str, agent_name: &str) -> Option<String> {
    let identity = derive_nip06_identity(mnemonic, 0).ok()?;
    let secret_key = SecretKey::from_hex(&identity.secret_key_hex).ok()?;
    let keys = Keys::new(secret_key);

    let content = json!({
        "display_name": agent_name,
        "framework": "omo-koda2",
    })
    .to_string();

    // `d` tag = this agent's own hex pubkey, per ip_root.md's own field
    // note for Omo-Koda2-born agents -- one IP Root per agent identity,
    // addressable/updatable in place (kind 31900 is a NIP-33 parameterized
    // replaceable range) rather than re-minting on every birth call.
    let builder = EventBuilder::new(Kind::Custom(IP_ROOT_KIND), content)
        .tag(Tag::identifier(identity.public_key_hex.clone()));

    sign_and_publish(keys, builder).await
}

/// Publish a Creation Receipt (kind 1901) at birth.
///
/// Records agent_id, npub (hex pubkey), birth_timestamp, and genesis_hash to
/// the same Nostr relay as the IP Root.  Published as a plain regular event
/// (not addressable/replaceable) — each birth produces one unique receipt.
///
/// Fail-open, same convention as publish_ip_root.
///
/// `mnemonic`: the agent's birth mnemonic (same NIP-06 identity).
/// `agent_id`: canonical agent identifier string.
/// `birth_timestamp`: Unix seconds at birth.
/// `genesis_hash`: SHA-256 hex of the genesis receipt.
pub async fn publish_creation_receipt(
    mnemonic: &str,
    agent_id: &str,
    birth_timestamp: i64,
    genesis_hash: &str,
) -> Option<String> {
    let identity = derive_nip06_identity(mnemonic, 0).ok()?;
    let secret_key = SecretKey::from_hex(&identity.secret_key_hex).ok()?;
    let keys = Keys::new(secret_key);

    let content = json!({
        "agent_id":        agent_id,
        "npub":            identity.public_key_hex.clone(),
        "birth_timestamp": birth_timestamp,
        "genesis_hash":    genesis_hash,
        "framework":       "omo-koda2",
    })
    .to_string();

    let builder = EventBuilder::new(Kind::Custom(CREATION_RECEIPT_KIND), content)
        .tag(Tag::custom(
            TagKind::custom("agent_id"),
            vec![agent_id.to_string()],
        ))
        .tag(Tag::custom(
            TagKind::custom("genesis_hash"),
            vec![genesis_hash.to_string()],
        ));

    sign_and_publish(keys, builder).await
}

/// Publish an Attestation event (kind 1902) linking the IP Root to the
/// Creation Receipt.
///
/// Should be called after both publish_ip_root and publish_creation_receipt
/// have completed.  If either returned None (relay down) the attestation
/// still publishes what it has — it just omits the None event ids.
/// Fail-open.
///
/// `mnemonic`: the agent's birth mnemonic.
/// `ip_root_event_id`: event id returned by publish_ip_root (may be empty).
/// `creation_receipt_event_id`: event id returned by publish_creation_receipt
///     (may be empty).
/// `agent_id`: canonical agent identifier.
pub async fn publish_attestation(
    mnemonic: &str,
    ip_root_event_id: &str,
    creation_receipt_event_id: &str,
    agent_id: &str,
) -> Option<String> {
    let identity = derive_nip06_identity(mnemonic, 0).ok()?;
    let secret_key = SecretKey::from_hex(&identity.secret_key_hex).ok()?;
    let keys = Keys::new(secret_key);

    let content = json!({
        "agent_id":                  agent_id,
        "ip_root_event_id":          ip_root_event_id,
        "creation_receipt_event_id": creation_receipt_event_id,
        "attestation":               "birth-complete",
        "framework":                 "omo-koda2",
    })
    .to_string();

    let mut builder = EventBuilder::new(Kind::Custom(ATTESTATION_KIND), content)
        .tag(Tag::custom(
            TagKind::custom("agent_id"),
            vec![agent_id.to_string()],
        ));

    if !ip_root_event_id.is_empty() {
        builder = builder.tag(Tag::custom(
            TagKind::custom("ip_root"),
            vec![ip_root_event_id.to_string()],
        ));
    }
    if !creation_receipt_event_id.is_empty() {
        builder = builder.tag(Tag::custom(
            TagKind::custom("creation_receipt"),
            vec![creation_receipt_event_id.to_string()],
        ));
    }

    sign_and_publish(keys, builder).await
}

/// Publish a Twin Binding (kind 1903, per ip-layer's schemas/twin_binding.md)
/// -- a one-time claim that a real VeilSim `sim_id` IS the digital twin of
/// this agent's IP Root. Unlike the IP Root (published automatically at
/// birth), there is no sim_id to bind until an agent actually has a real
/// VeilSim/OSOVM twin running -- this is deliberately NOT wired into the
/// birth flow; callers (e.g. a kernel tool, once this agent's VeilSim twin
/// genuinely exists) invoke this on demand with a real sim_id.
///
/// Fail-open, same convention as publish_ip_root: returns the real event id
/// on success, None on any failure (no relay, signing failed, sim_id empty,
/// etc.) -- never blocks whatever triggered the binding.
///
/// `mnemonic`: the agent's own birth mnemonic (same NIP-06 identity as its
/// IP Root, so the binding's pubkey matches the `ip_root` it claims).
/// `sim_id`: the real VeilSim sim id (matches ZangbetoReceipt.sim_id in
/// OSOVM's zangbeto_receipts.jl) -- required, this function does not
/// fabricate one.
/// `twin_kind`: per the schema, e.g. "veilsim-1to1"; extensible for future
/// twin types.
/// `fidelity`: optional, self-declared free-text sync-fidelity claim.
/// `osovm_veil_receipt_id`: optional, the first veil execution's receipt id
/// (becomes the `osovm_op` tag's third element, per the schema).
pub async fn publish_twin_binding(
    mnemonic: &str,
    sim_id: &str,
    twin_kind: &str,
    fidelity: Option<&str>,
    osovm_veil_receipt_id: Option<&str>,
) -> Option<String> {
    if sim_id.trim().is_empty() || twin_kind.trim().is_empty() {
        return None;
    }

    let identity = derive_nip06_identity(mnemonic, 0).ok()?;
    let secret_key = SecretKey::from_hex(&identity.secret_key_hex).ok()?;
    let keys = Keys::new(secret_key);

    let content = json!({ "notes": "" }).to_string();

    // ip_root tag: this agent's own hex pubkey, matching the `d` tag its
    // own IP Root was published under (see publish_ip_root above).
    let mut builder = EventBuilder::new(Kind::Custom(TWIN_BINDING_KIND), content)
        .tag(Tag::custom(
            TagKind::custom("ip_root"),
            vec![identity.public_key_hex.clone()],
        ))
        .tag(Tag::custom(
            TagKind::custom("sim_id"),
            vec![sim_id.to_string()],
        ))
        .tag(Tag::custom(
            TagKind::custom("twin_kind"),
            vec![twin_kind.to_string()],
        ));

    if let Some(f) = fidelity.filter(|f| !f.trim().is_empty()) {
        builder = builder.tag(Tag::custom(
            TagKind::custom("fidelity"),
            vec![f.to_string()],
        ));
    }
    if let Some(receipt_id) = osovm_veil_receipt_id.filter(|r| !r.trim().is_empty()) {
        builder = builder.tag(Tag::custom(
            TagKind::custom("osovm_op"),
            vec!["VEIL".to_string(), receipt_id.to_string()],
        ));
    }

    sign_and_publish(keys, builder).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explicit, fast-failing unreachable relay -- loopback with nothing
    /// bound (confirmed live via a direct probe: nostr-sdk's Client
    /// reports this as "relay not connected" within milliseconds, no
    /// timeout wait) -- NOT relying on "nothing happens to be listening on
    /// the ws://localhost:3000 default", which is environment-dependent
    /// and was confirmed false on this exact dev machine (a real local
    /// relay IS reachable there sometimes). Also the real regression test
    /// for the Output.success bug fixed above: before that fix, this
    /// exact scenario (event "sent" but accepted by zero relays) returned
    /// Some(event_id) instead of None.
    #[tokio::test]
    async fn publish_ip_root_fails_open_when_no_relay_accepts_the_event() {
        // SAFETY: test-only, single-threaded test binary; no other test in
        // this crate reads this env var.
        unsafe {
            std::env::set_var("IP_LAYER_RELAY_URL", "ws://127.0.0.1:19999");
        }
        let mnemonic = bipon39::entropy_to_mnemonic(&[9u8; 32])
            .expect("test entropy should produce a valid mnemonic")
            .join(" ");
        let result = publish_ip_root(&mnemonic, "test-agent").await;
        unsafe {
            std::env::remove_var("IP_LAYER_RELAY_URL");
        }
        assert!(result.is_none(), "expected None when no relay accepts the event, got {result:?}");
    }

    #[tokio::test]
    async fn publish_twin_binding_rejects_empty_sim_id_without_touching_the_network() {
        let mnemonic = bipon39::entropy_to_mnemonic(&[9u8; 32])
            .expect("test entropy should produce a valid mnemonic")
            .join(" ");
        let result = publish_twin_binding(&mnemonic, "", "veilsim-1to1", None, None).await;
        assert!(result.is_none(), "expected None for empty sim_id, got {result:?}");
    }

    #[tokio::test]
    async fn publish_twin_binding_rejects_empty_twin_kind_without_touching_the_network() {
        let mnemonic = bipon39::entropy_to_mnemonic(&[9u8; 32])
            .expect("test entropy should produce a valid mnemonic")
            .join(" ");
        let result = publish_twin_binding(&mnemonic, "sim-42", "  ", None, None).await;
        assert!(result.is_none(), "expected None for blank twin_kind, got {result:?}");
    }

    /// Same regression coverage as the IP Root test above, for the Twin
    /// Binding path -- confirms the shared sign_and_publish() helper's
    /// Output.success check applies here too, not just to publish_ip_root.
    #[tokio::test]
    async fn publish_twin_binding_fails_open_when_no_relay_accepts_the_event() {
        unsafe {
            std::env::set_var("IP_LAYER_RELAY_URL", "ws://127.0.0.1:19999");
        }
        let mnemonic = bipon39::entropy_to_mnemonic(&[10u8; 32])
            .expect("test entropy should produce a valid mnemonic")
            .join(" ");
        let result = publish_twin_binding(
            &mnemonic,
            "sim-42",
            "veilsim-1to1",
            Some("0.97"),
            Some("veil-receipt-abc123"),
        )
        .await;
        unsafe {
            std::env::remove_var("IP_LAYER_RELAY_URL");
        }
        assert!(result.is_none(), "expected None when no relay accepts the event, got {result:?}");
    }
}
