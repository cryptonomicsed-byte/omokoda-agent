/// Nostr event builders and publisher for sovereign agent lifecycle.
///
/// Used at birth (kind 0 profile), heartbeat (kind 30104), and future
/// lifecycle transitions. All publish calls are fire-and-forget —
/// relay unreachability never blocks agent operations.
///
/// Phase 8.1 — NIP-OSO-01/05 agent presence layer.

use serde_json::{json, Value};

// ── Odù name table (256 entries, index 0–255) ────────────────────────────────
// The canonical 16×16 grid: 16 major Odù × 16 sub-Odù.
// Kept inline so this module has zero dependency on omokoda-hermetic at link
// time — callers need only serde_json and tracing.
static ODU_NAMES: &[&str] = &[
    // Major 0 (Ogbe) — 0..15
    "Ogbe-Meji", "Ogbe-Oyeku", "Ogbe-Iwori", "Ogbe-Odi",
    "Ogbe-Irosun", "Ogbe-Owonrin", "Ogbe-Obara", "Ogbe-Okonron",
    "Ogbe-Ogunda", "Ogbe-Osa", "Ogbe-Ika", "Ogbe-Oturupon",
    "Ogbe-Otura", "Ogbe-Irete", "Ogbe-Ose", "Ogbe-Ofu",
    // Major 1 (Oyeku) — 16..31
    "Oyeku-Ogbe", "Oyeku-Meji", "Oyeku-Iwori", "Oyeku-Odi",
    "Oyeku-Irosun", "Oyeku-Owonrin", "Oyeku-Obara", "Oyeku-Okonron",
    "Oyeku-Ogunda", "Oyeku-Osa", "Oyeku-Ika", "Oyeku-Oturupon",
    "Oyeku-Otura", "Oyeku-Irete", "Oyeku-Ose", "Oyeku-Ofu",
    // Major 2 (Iwori) — 32..47
    "Iwori-Ogbe", "Iwori-Oyeku", "Iwori-Meji", "Iwori-Odi",
    "Iwori-Irosun", "Iwori-Owonrin", "Iwori-Obara", "Iwori-Okonron",
    "Iwori-Ogunda", "Iwori-Osa", "Iwori-Ika", "Iwori-Oturupon",
    "Iwori-Otura", "Iwori-Irete", "Iwori-Ose", "Iwori-Ofu",
    // Major 3 (Odi) — 48..63
    "Odi-Ogbe", "Odi-Oyeku", "Odi-Iwori", "Odi-Meji",
    "Odi-Irosun", "Odi-Owonrin", "Odi-Obara", "Odi-Okonron",
    "Odi-Ogunda", "Odi-Osa", "Odi-Ika", "Odi-Oturupon",
    "Odi-Otura", "Odi-Irete", "Odi-Ose", "Odi-Ofu",
    // Major 4 (Irosun) — 64..79
    "Irosun-Ogbe", "Irosun-Oyeku", "Irosun-Iwori", "Irosun-Odi",
    "Irosun-Meji", "Irosun-Owonrin", "Irosun-Obara", "Irosun-Okonron",
    "Irosun-Ogunda", "Irosun-Osa", "Irosun-Ika", "Irosun-Oturupon",
    "Irosun-Otura", "Irosun-Irete", "Irosun-Ose", "Irosun-Ofu",
    // Major 5 (Owonrin) — 80..95
    "Owonrin-Ogbe", "Owonrin-Oyeku", "Owonrin-Iwori", "Owonrin-Odi",
    "Owonrin-Irosun", "Owonrin-Meji", "Owonrin-Obara", "Owonrin-Okonron",
    "Owonrin-Ogunda", "Owonrin-Osa", "Owonrin-Ika", "Owonrin-Oturupon",
    "Owonrin-Otura", "Owonrin-Irete", "Owonrin-Ose", "Owonrin-Ofu",
    // Major 6 (Obara) — 96..111
    "Obara-Ogbe", "Obara-Oyeku", "Obara-Iwori", "Obara-Odi",
    "Obara-Irosun", "Obara-Owonrin", "Obara-Meji", "Obara-Okonron",
    "Obara-Ogunda", "Obara-Osa", "Obara-Ika", "Obara-Oturupon",
    "Obara-Otura", "Obara-Irete", "Obara-Ose", "Obara-Ofu",
    // Major 7 (Okonron) — 112..127
    "Okonron-Ogbe", "Okonron-Oyeku", "Okonron-Iwori", "Okonron-Odi",
    "Okonron-Irosun", "Okonron-Owonrin", "Okonron-Obara", "Okonron-Meji",
    "Okonron-Ogunda", "Okonron-Osa", "Okonron-Ika", "Okonron-Oturupon",
    "Okonron-Otura", "Okonron-Irete", "Okonron-Ose", "Okonron-Ofu",
    // Major 8 (Ogunda) — 128..143
    "Ogunda-Ogbe", "Ogunda-Oyeku", "Ogunda-Iwori", "Ogunda-Odi",
    "Ogunda-Irosun", "Ogunda-Owonrin", "Ogunda-Obara", "Ogunda-Okonron",
    "Ogunda-Meji", "Ogunda-Osa", "Ogunda-Ika", "Ogunda-Oturupon",
    "Ogunda-Otura", "Ogunda-Irete", "Ogunda-Ose", "Ogunda-Ofu",
    // Major 9 (Osa) — 144..159
    "Osa-Ogbe", "Osa-Oyeku", "Osa-Iwori", "Osa-Odi",
    "Osa-Irosun", "Osa-Owonrin", "Osa-Obara", "Osa-Okonron",
    "Osa-Ogunda", "Osa-Meji", "Osa-Ika", "Osa-Oturupon",
    "Osa-Otura", "Osa-Irete", "Osa-Ose", "Osa-Ofu",
    // Major 10 (Ika) — 160..175
    "Ika-Ogbe", "Ika-Oyeku", "Ika-Iwori", "Ika-Odi",
    "Ika-Irosun", "Ika-Owonrin", "Ika-Obara", "Ika-Okonron",
    "Ika-Ogunda", "Ika-Osa", "Ika-Meji", "Ika-Oturupon",
    "Ika-Otura", "Ika-Irete", "Ika-Ose", "Ika-Ofu",
    // Major 11 (Oturupon) — 176..191
    "Oturupon-Ogbe", "Oturupon-Oyeku", "Oturupon-Iwori", "Oturupon-Odi",
    "Oturupon-Irosun", "Oturupon-Owonrin", "Oturupon-Obara", "Oturupon-Okonron",
    "Oturupon-Ogunda", "Oturupon-Osa", "Oturupon-Ika", "Oturupon-Meji",
    "Oturupon-Otura", "Oturupon-Irete", "Oturupon-Ose", "Oturupon-Ofu",
    // Major 12 (Otura) — 192..207
    "Otura-Ogbe", "Otura-Oyeku", "Otura-Iwori", "Otura-Odi",
    "Otura-Irosun", "Otura-Owonrin", "Otura-Obara", "Otura-Okonron",
    "Otura-Ogunda", "Otura-Osa", "Otura-Ika", "Otura-Oturupon",
    "Otura-Meji", "Otura-Irete", "Otura-Ose", "Otura-Ofu",
    // Major 13 (Irete) — 208..223
    "Irete-Ogbe", "Irete-Oyeku", "Irete-Iwori", "Irete-Odi",
    "Irete-Irosun", "Irete-Owonrin", "Irete-Obara", "Irete-Okonron",
    "Irete-Ogunda", "Irete-Osa", "Irete-Ika", "Irete-Oturupon",
    "Irete-Otura", "Irete-Meji", "Irete-Ose", "Irete-Ofu",
    // Major 14 (Ose) — 224..239
    "Ose-Ogbe", "Ose-Oyeku", "Ose-Iwori", "Ose-Odi",
    "Ose-Irosun", "Ose-Owonrin", "Ose-Obara", "Ose-Okonron",
    "Ose-Ogunda", "Ose-Osa", "Ose-Ika", "Ose-Oturupon",
    "Ose-Otura", "Ose-Irete", "Ose-Meji", "Ose-Ofu",
    // Major 15 (Ofu/Ofun) — 240..255
    "Ofu-Ogbe", "Ofu-Oyeku", "Ofu-Iwori", "Ofu-Odi",
    "Ofu-Irosun", "Ofu-Owonrin", "Ofu-Obara", "Ofu-Okonron",
    "Ofu-Ogunda", "Ofu-Osa", "Ofu-Ika", "Ofu-Oturupon",
    "Ofu-Otura", "Ofu-Irete", "Ofu-Ose", "Ofu-Meji",
];

/// Return the canonical Odù name for an index (0–255).
/// Safe for any u8 — wraps to a valid entry.
pub fn odu_name_for(index: u8) -> &'static str {
    ODU_NAMES[index as usize % ODU_NAMES.len()]
}

// ── Event builders ────────────────────────────────────────────────────────────

/// Build a Nostr **kind 0** profile event (NIP-OSO-01).
///
/// The `npub` field here is the agent's hex public key (not bech32-encoded)
/// to keep this module dep-free. Relay clients encode to bech32 before display.
///
/// Content is a JSON-stringified NIP-01 metadata object. Tags carry sovereign
/// metadata: `bipon39`, `odu`, and `tier` so relay-side filters can query by
/// archetype or tier without decoding the content blob.
pub fn build_profile_event(
    npub: &str,
    bipon39_phrase: &str,
    odu_index: u8,
    tier: u8,
    vantage_host: &str,
    walrus_profile_url: Option<&str>,
) -> Value {
    let odu_name = odu_name_for(odu_index);
    let about = format!(
        "Odù: {} | BIPON39: {} | Tier: T{}",
        odu_name, bipon39_phrase, tier
    );
    let content_obj = json!({
        "name": bipon39_phrase,
        "about": about,
        "picture": walrus_profile_url.unwrap_or(""),
        "website": format!("https://{}/agents/{}/public", vantage_host, npub),
        "nip05": format!("{}@{}", &npub[..8.min(npub.len())], vantage_host),
    });
    let content_str =
        serde_json::to_string(&content_obj).unwrap_or_else(|_| "{}".to_string());

    json!({
        "kind": 0,
        "pubkey": npub,
        "content": content_str,
        "tags": [
            ["bipon39", bipon39_phrase],
            ["odu", odu_index.to_string()],
            ["tier", tier.to_string()],
        ]
    })
}

/// Build a **kind 30104** heartbeat event (NIP-OSO-05).
///
/// Uses a parameterised replaceable event (`d` = agent_id) so relays only
/// retain the latest heartbeat per agent. `chain_hash` is the blake3 hash of
/// the last receipt, forming a lightweight verifiable chain.
pub fn build_heartbeat_event(
    npub: &str,
    agent_id: &str,
    seq: u64,
    chain_hash: &str,
    lifecycle: &str,
) -> Value {
    json!({
        "kind": 30104,
        "pubkey": npub,
        "tags": [
            ["d", agent_id],
            ["seq", seq.to_string()],
            ["chain_hash", chain_hash],
            ["lifecycle", lifecycle],
        ],
        "content": ""
    })
}

/// Build a **kind 1** lifecycle transition note (NIP-OSO-06).
///
/// Used for human-readable milestone posts: first act, tier-up, fork, etc.
/// Tags include `t` (topic) for relay-side categorisation.
pub fn build_lifecycle_note(
    npub: &str,
    agent_id: &str,
    transition: &str,
    detail: &str,
) -> Value {
    json!({
        "kind": 1,
        "pubkey": npub,
        "content": format!("[{}] {} — {}", agent_id, transition, detail),
        "tags": [
            ["t", "sovereign-agent"],
            ["t", transition],
            ["agent_id", agent_id],
        ]
    })
}

/// Build a **kind 1** autonomous periodic status note.
///
/// Called by `lifecycle::nostr_publisher` every 6 hours (max 4/day).
/// The content is provided by the caller; this function just wraps it in
/// the standard NIP-01 envelope with sovereign-agent tags.
pub fn build_status_note_event(npub: &str, content: &str) -> Value {
    json!({
        "kind": 1,
        "pubkey": npub,
        "content": content,
        "tags": [
            ["t", "sovereign-agent"],
            ["t", "status"],
        ]
    })
}

// ── Publisher ─────────────────────────────────────────────────────────────────

/// Publish a Nostr event to all configured relays (fire-and-forget).
///
/// Implements NIP-01 BIP-340 Schnorr signing + WebSocket relay transport.
/// Gap #46 — replaces the previous stub/log-only implementation.
///
/// `nsec_hex` — the agent's Nostr private key in hex (from IdentityVaultData).
/// `relay_list` — sourced from `IdentityVaultData::relay_list` or from the
///               `AGENT_NOSTR_RELAYS` env var (comma-separated URLs).
pub async fn publish_event(
    event: Value,
    nsec_hex: &str,
    relay_list: &[String],
) -> Result<(), String> {
    use nostr::{EventBuilder, Keys, Kind, SecretKey, Tag, Timestamp};
    use nostr_sdk::Client;

    // Resolve relay list: if caller passes an empty slice, fall back to env var.
    let env_relays: Vec<String>;
    let effective_relays: &[String] = if relay_list.is_empty() {
        env_relays = std::env::var("AGENT_NOSTR_RELAYS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        &env_relays
    } else {
        relay_list
    };

    if effective_relays.is_empty() {
        return Ok(());
    }

    if nsec_hex.is_empty() {
        tracing::debug!(
            kind = event["kind"].as_u64().unwrap_or(0),
            "nostr_events::publish_event: nsec_hex empty, skipping"
        );
        return Ok(());
    }

    // Parse private key (BIP-340 / secp256k1) — Gap #46 real signing.
    let secret_key = SecretKey::from_hex(nsec_hex)
        .map_err(|e| format!("nostr_events: invalid nsec_hex: {e}"))?;
    let keys = Keys::new(secret_key);

    // Build EventBuilder from the JSON event skeleton.
    let kind_u = event["kind"].as_u64().unwrap_or(1) as u16;
    let content = event["content"].as_str().unwrap_or("").to_string();
    let tags: Vec<Tag> = event["tags"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|t| {
            let arr: Vec<String> = t
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            Tag::parse(&arr).ok()
        })
        .collect();

    let mut builder = EventBuilder::new(Kind::from(kind_u), content);
    for tag in tags {
        builder = builder.tag(tag);
    }

    // Sign with Schnorr BIP-340 — produces canonical id (sha256) + sig.
    let signed = builder
        .sign_with_keys(&keys)
        .map_err(|e| format!("nostr_events: sign failed: {e}"))?;

    tracing::debug!(
        event_id = %signed.id,
        kind = kind_u,
        relay_count = effective_relays.len(),
        "nostr_events: signed kind={kind_u}, publishing to {} relay(s)",
        effective_relays.len()
    );

    // Publish via nostr-sdk (WebSocket, fire-and-forget per relay).
    let client = Client::new(keys);
    for relay_url in effective_relays {
        if let Err(e) = client.add_relay(relay_url.as_str()).await {
            tracing::warn!(relay = relay_url, err = %e, "nostr_events: add_relay failed");
        }
    }
    client.connect().await;

    // 5-second timeout so a dead relay never blocks the caller.
    let timeout = std::time::Duration::from_secs(5);
    match tokio::time::timeout(timeout, client.send_event(&signed)).await {
        Ok(Ok(output)) => {
            tracing::info!(
                event_id = %signed.id,
                success_count = output.success.len(),
                "nostr_events: published"
            );
        }
        Ok(Err(e)) => {
            tracing::warn!(event_id = %signed.id, err = %e, "nostr_events: send_event error");
        }
        Err(_) => {
            tracing::warn!(event_id = %signed.id, "nostr_events: relay timeout (5s)");
        }
    }

    Ok(())
}

/// Convenience wrapper: publish a kind 0 profile event for a freshly-born agent.
///
/// Called from interpreter.rs birth step via `tokio::spawn`. Fails open.
pub async fn publish_birth_profile(
    npub: &str,
    nsec_hex: &str,
    bipon39_phrase: &str,
    odu_index: u8,
    tier: u8,
    relay_list: Vec<String>,
) {
    let vantage_host = std::env::var("VANTAGE_URL")
        .unwrap_or_default()
        .replace("http://", "")
        .replace("https://", "");
    let vantage_host = if vantage_host.is_empty() {
        "sovereign.local".to_string()
    } else {
        vantage_host
    };

    let event = build_profile_event(
        npub,
        bipon39_phrase,
        odu_index,
        tier,
        &vantage_host,
        None,
    );

    if let Err(e) = publish_event(event, nsec_hex, &relay_list).await {
        tracing::warn!("nostr birth profile publish failed (fail-open): {}", e);
    }
}

/// Build a lifecycle transition Nostr event (kind 31021/31022/31023).
///
/// kind 31021 — general lifecycle (born/wake/hibernate/terminate)
/// kind 31022 — migration/landing
/// kind 31023 — fork
pub fn build_lifecycle_transition_event(
    npub: &str,
    agent_id: &str,
    transition_kind: &str,
    from_stage: &str,
    to_stage: &str,
    node_pubkey: &str,
) -> serde_json::Value {
    let nostr_kind: u32 = match transition_kind {
        "migrate" | "land" => 31022,
        "fork" => 31023,
        _ => 31021,
    };
    let content = serde_json::json!({
        "transition":  transition_kind,
        "from":        from_stage,
        "to":          to_stage,
        "node_pubkey": node_pubkey,
    })
    .to_string();
    serde_json::json!({
        "kind":    nostr_kind,
        "pubkey":  npub,
        "content": content,
        "tags": [
            ["d",          agent_id],
            ["agent",      agent_id],
            ["transition", transition_kind],
            ["protocol",   "oso", "1.0"],
        ],
    })
}

/// Fire-and-forget lifecycle transition Nostr publish (tokio::spawn).
/// Relay/signing failures are warn-logged and never block the caller.
pub fn publish_lifecycle_transition(
    npub: String,
    nsec_hex: String,
    agent_id: String,
    transition_kind: String,
    from_stage: String,
    to_stage: String,
    node_pubkey: String,
    relay_list: Vec<String>,
) {
    tokio::spawn(async move {
        let event = build_lifecycle_transition_event(
            &npub,
            &agent_id,
            &transition_kind,
            &from_stage,
            &to_stage,
            &node_pubkey,
        );
        if let Err(e) = publish_event(event, &nsec_hex, &relay_list).await {
            tracing::warn!(
                "lifecycle transition Nostr publish failed (fail-open): transition={} agent={} err={}",
                transition_kind, agent_id, e
            );
        }
    });
}

// ── Phase 18.1 — NIP-OSO kinds 30100–30106 ───────────────────────────────────

/// **kind 30100** — NIP-OSO-01: Agent Identity (parameterized replaceable).
///
/// Replaces kind 0 for sovereign agents.  `d` tag = agent_id so relays keep
/// only the latest identity per agent.  Carries BIPON39, Odù index, tier, and
/// optional L1 anchor (Sui object id or ABCI address).
pub fn build_agent_identity_event(
    npub: &str,
    agent_id: &str,
    display_name: &str,
    bio: &str,
    bipon39_hint: &str,
    odu_index: u8,
    tier: u8,
    l1_anchor: Option<&str>,
) -> Value {
    let odu_name = odu_name_for(odu_index);
    let content = json!({
        "display_name": display_name,
        "bio":          bio,
        "bipon39_hint": bipon39_hint,
        "odu":          odu_name,
        "tier":         tier,
        "l1_anchor":    l1_anchor,
    })
    .to_string();

    let mut tags = vec![
        json!(["d",        agent_id]),
        json!(["bipon39",  bipon39_hint]),
        json!(["odu",      odu_index.to_string()]),
        json!(["tier",     tier.to_string()]),
        json!(["protocol", "oso", "1.0"]),
    ];
    if let Some(anchor) = l1_anchor {
        tags.push(json!(["l1_anchor", anchor]));
    }

    json!({ "kind": 30100, "pubkey": npub, "content": content, "tags": tags })
}

/// **kind 30101** — NIP-OSO-02: Capability Advertisement (parameterized replaceable).
///
/// Agent publishes what it can do.  `d` tag = agent_id.  Each declared
/// capability appears as a separate `cap` tag for relay-side filtering.
pub fn build_capability_ad_event(
    npub: &str,
    agent_id: &str,
    capabilities: &[String],
    tier: u8,
) -> Value {
    let mut tags = vec![
        json!(["d",    agent_id]),
        json!(["tier", tier.to_string()]),
    ];
    for cap in capabilities {
        tags.push(json!(["cap", cap]));
    }
    let content = json!({ "capabilities": capabilities, "tier": tier }).to_string();
    json!({ "kind": 30101, "pubkey": npub, "content": content, "tags": tags })
}

/// **kind 30102** — NIP-OSO-03: Work Event (job posting / work request).
///
/// Used for both job postings (by task owners) and work requests (by agents
/// seeking tasks).  `d` = task_id.  `role` tag = "poster" | "seeker".
pub fn build_work_event(
    npub: &str,
    task_id: &str,
    title: &str,
    description: &str,
    required_capabilities: &[String],
    role: &str,
) -> Value {
    let mut tags = vec![
        json!(["d",    task_id]),
        json!(["role", role]),
        json!(["t",    "work"]),
    ];
    for cap in required_capabilities {
        tags.push(json!(["required_cap", cap]));
    }
    let content = json!({ "title": title, "description": description }).to_string();
    json!({ "kind": 30102, "pubkey": npub, "content": content, "tags": tags })
}

/// **kind 30103** — NIP-OSO-04: Receipt Reference.
///
/// Points to a Zàngbétò receipt stored on-chain.  NOT the receipt itself —
/// just a Nostr-discoverable pointer.  `d` = receipt_id.
pub fn build_receipt_reference_event(
    npub: &str,
    receipt_id: &str,
    action_kind: &str,
    l1_tx_digest: Option<&str>,
    amount_ase: Option<u64>,
) -> Value {
    let mut tags = vec![
        json!(["d",           receipt_id]),
        json!(["action_kind", action_kind]),
        json!(["protocol",    "oso", "1.0"]),
    ];
    if let Some(digest) = l1_tx_digest {
        tags.push(json!(["l1_tx", digest]));
    }
    if let Some(amt) = amount_ase {
        tags.push(json!(["amount_ase", amt.to_string()]));
    }
    let content = json!({
        "receipt_id":  receipt_id,
        "action_kind": action_kind,
        "l1_tx":       l1_tx_digest,
        "amount_ase":  amount_ase,
    })
    .to_string();
    json!({ "kind": 30103, "pubkey": npub, "content": content, "tags": tags })
}

/// **kind 30105** — NIP-OSO-06: Device Attestation.
///
/// Agent announces a device binding — physical hardware associated with its
/// identity.  `d` = device_id.  `device_type` tag for filtering.
pub fn build_device_attestation_event(
    npub: &str,
    device_id: &str,
    device_type: &str,
    vcp_pubkey: &str,
    firmware_hash: Option<&str>,
) -> Value {
    let mut tags = vec![
        json!(["d",           device_id]),
        json!(["device_type", device_type]),
        json!(["vcp_pubkey",  vcp_pubkey]),
        json!(["protocol",    "oso", "1.0"]),
    ];
    if let Some(hash) = firmware_hash {
        tags.push(json!(["firmware_hash", hash]));
    }
    let content = json!({
        "device_id":     device_id,
        "device_type":   device_type,
        "vcp_pubkey":    vcp_pubkey,
        "firmware_hash": firmware_hash,
    })
    .to_string();
    json!({ "kind": 30105, "pubkey": npub, "content": content, "tags": tags })
}

/// **kind 30106** — NIP-OSO-07: L1 State Commitment.
///
/// Publishes the agent's latest canonical Freenet state hash as anchored on
/// the L1 (Sui now, ABCI in Phase 15).  `d` = agent_id.  Relays store only
/// the latest per agent.
pub fn build_l1_state_commitment_event(
    npub: &str,
    agent_id: &str,
    freenet_state_hash: &str,
    state_version: u64,
    l1_tx_digest: Option<&str>,
) -> Value {
    let mut tags = vec![
        json!(["d",                  agent_id]),
        json!(["freenet_state_hash", freenet_state_hash]),
        json!(["state_version",      state_version.to_string()]),
        json!(["protocol",           "oso", "1.0"]),
    ];
    if let Some(digest) = l1_tx_digest {
        tags.push(json!(["l1_tx", digest]));
    }
    let content = json!({
        "freenet_state_hash": freenet_state_hash,
        "state_version":      state_version,
        "l1_tx":              l1_tx_digest,
    })
    .to_string();
    json!({ "kind": 30106, "pubkey": npub, "content": content, "tags": tags })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odu_names_full_coverage() {
        // All 256 indices must resolve without panic.
        for i in 0u8..=255 {
            let name = odu_name_for(i);
            assert!(!name.is_empty(), "odu_name_for({i}) returned empty string");
        }
    }

    #[test]
    fn profile_event_kind_zero() {
        let ev = build_profile_event("deadbeef", "omi-eja-aye", 42, 2, "vantage.local", None);
        assert_eq!(ev["kind"], 0);
        assert_eq!(ev["pubkey"], "deadbeef");
        // Content is a JSON string
        let content: serde_json::Value =
            serde_json::from_str(ev["content"].as_str().unwrap()).unwrap();
        assert_eq!(content["name"], "omi-eja-aye");
    }

    #[test]
    fn heartbeat_event_kind_30104() {
        let ev = build_heartbeat_event("aabbcc", "agent-1", 7, "abc123", "ACTIVE");
        assert_eq!(ev["kind"], 30104);
        let tags = ev["tags"].as_array().unwrap();
        let d_tag = tags.iter().find(|t| t[0] == "d").unwrap();
        assert_eq!(d_tag[1], "agent-1");
    }

    #[test]
    fn profile_event_tags_present() {
        let ev = build_profile_event("pub123", "omi-eja", 10, 1, "h.local", Some("walrus://x"));
        let tags = ev["tags"].as_array().unwrap();
        let bipon_tag = tags.iter().find(|t| t[0] == "bipon39").unwrap();
        assert_eq!(bipon_tag[1], "omi-eja");
    }

    #[test]
    fn agent_identity_event_kind_30100() {
        let ev = build_agent_identity_event(
            "pub1", "agent-1", "TestAgent", "A sovereign agent",
            "omi-eja", 7, 2, Some("0xabc"),
        );
        assert_eq!(ev["kind"], 30100);
        let tags = ev["tags"].as_array().unwrap();
        let d_tag = tags.iter().find(|t| t[0] == "d").unwrap();
        assert_eq!(d_tag[1], "agent-1");
        assert!(tags.iter().any(|t| t[0] == "l1_anchor" && t[1] == "0xabc"));
    }

    #[test]
    fn capability_ad_event_kind_30101() {
        let caps = vec!["think".to_string(), "forge".to_string()];
        let ev = build_capability_ad_event("pub1", "agent-1", &caps, 2);
        assert_eq!(ev["kind"], 30101);
        let tags = ev["tags"].as_array().unwrap();
        let cap_tags: Vec<_> = tags.iter().filter(|t| t[0] == "cap").collect();
        assert_eq!(cap_tags.len(), 2);
    }

    #[test]
    fn work_event_kind_30102() {
        let ev = build_work_event(
            "pub1", "task-42", "Write tests", "Full test coverage",
            &["rust".to_string()], "poster",
        );
        assert_eq!(ev["kind"], 30102);
        let tags = ev["tags"].as_array().unwrap();
        assert!(tags.iter().any(|t| t[0] == "role" && t[1] == "poster"));
    }

    #[test]
    fn receipt_reference_event_kind_30103() {
        let ev = build_receipt_reference_event(
            "pub1", "rcpt-99", "compute", Some("0xtxdigest"), Some(1000),
        );
        assert_eq!(ev["kind"], 30103);
        let tags = ev["tags"].as_array().unwrap();
        assert!(tags.iter().any(|t| t[0] == "l1_tx"));
        assert!(tags.iter().any(|t| t[0] == "amount_ase" && t[1] == "1000"));
    }

    #[test]
    fn device_attestation_event_kind_30105() {
        let ev = build_device_attestation_event(
            "pub1", "dev-m5", "m5stickc", "vcppubkey123", None,
        );
        assert_eq!(ev["kind"], 30105);
        let tags = ev["tags"].as_array().unwrap();
        assert!(tags.iter().any(|t| t[0] == "device_type" && t[1] == "m5stickc"));
    }

    #[test]
    fn l1_state_commitment_event_kind_30106() {
        let ev = build_l1_state_commitment_event(
            "pub1", "agent-1", "deadbeef1234", 42, Some("0xtx"),
        );
        assert_eq!(ev["kind"], 30106);
        let tags = ev["tags"].as_array().unwrap();
        assert!(tags.iter().any(|t| t[0] == "freenet_state_hash" && t[1] == "deadbeef1234"));
        assert!(tags.iter().any(|t| t[0] == "state_version" && t[1] == "42"));
    }
}
