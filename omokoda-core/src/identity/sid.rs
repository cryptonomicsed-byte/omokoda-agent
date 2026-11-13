//! SID — Sovereign ID / Super Intelligence Document (`sid/0.1`).
//!
//! An agent's birth certificate. The kernel already performs the birth ceremony
//! (`genesis::BirthOrchestrator` → `AgentGenesisReceipt`); this module renders
//! that receipt as a portable, canonical, hash-chained document and verifies it.
//! It does not replace the ceremony — it projects it.
//!
//! Three properties make the document trustworthy:
//!   * `hash` covers the canonical JSON of everything but `hash` and `_`-prefixed
//!     keys, byte-identically with the TypeScript and Python implementations;
//!   * `acting_hash` covers only what an agent acts on, so re-anchoring rewrites
//!     `hash` without forcing every consumer to reload;
//!   * `previous_hash` chains versions, so growth is append-only.
//!
//! Secrets never enter the document: the receipt carries commitments
//! (`birth_entropy_commitment`, `genesis_hash`, `derivation_root_id`), and this
//! module refuses to emit a certificate that carries a seed, mnemonic or key.

use serde_json::{json, Map, Value};

use crate::genesis::receipt::AgentGenesisReceipt;

pub const FORMAT: &str = "sid/0.1";
pub const STATUSES: [&str; 4] = ["cast", "live", "suspended", "retired"];

/// Fields an agent acts on. Attestation (genesis/time/provenance) and history
/// (changelog/appendix) are excluded so a re-anchor never forces a reload.
pub const ACTING_KEYS: [&str; 5] = ["identity", "capabilities", "memory", "lifecycle", "status"];

// ── canonical JSON ─────────────────────────────────────────────────────────

/// SHA-256 over the canonical body, excluding `hash` and `_`-prefixed keys.
pub fn sid_hash(cert: &Value) -> Result<String, String> {
    Ok(sha256_hex(&canonical_json(&without_hash(cert))?))
}

/// SHA-256 over the acting keys only.
pub fn acting_hash(cert: &Value) -> Result<String, String> {
    let mut m = Map::new();
    if let Value::Object(src) = cert {
        for k in ACTING_KEYS {
            if let Some(v) = src.get(k) {
                m.insert(k.to_string(), v.clone());
            }
        }
    }
    Ok(sha256_hex(&canonical_json(&Value::Object(m))?))
}

fn without_hash(cert: &Value) -> Value {
    match cert {
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, v) in m {
                if k == "hash" || k.starts_with('_') {
                    continue;
                }
                out.insert(k.clone(), v.clone());
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Python-identical canonical JSON: sorted keys, compact separators, empty
/// object values pruned bottom-up, and every control char / DEL / code point
/// >= 0x80 escaped (`ensure_ascii`). A single differing byte makes two
/// conformant implementations disagree about the same certificate.
pub fn canonical_json(v: &Value) -> Result<String, String> {
    let mut out = String::new();
    write_value(&prune(&normalize_numbers(v)?), &mut out)?;
    Ok(out)
}

fn normalize_numbers(v: &Value) -> Result<Value, String> {
    Ok(match v {
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, x) in m {
                out.insert(k.clone(), normalize_numbers(x)?);
            }
            Value::Object(out)
        }
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for x in a {
                out.push(normalize_numbers(x)?);
            }
            Value::Array(out)
        }
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                v.clone()
            } else {
                let f = n.as_f64().ok_or_else(|| format!("unrepresentable number {n}"))?;
                if f.is_finite() && f.fract() == 0.0 {
                    Value::from(f as i64)
                } else {
                    // A non-integral number would make the hash depend on a form
                    // the spec does not define.
                    return Err(format!("non-integral number {f} in certificate"));
                }
            }
        }
        other => other.clone(),
    })
}

fn prune(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, x) in m {
                let p = prune(x);
                match &p {
                    Value::Array(a) if a.is_empty() => continue,
                    Value::Object(o) if o.is_empty() => continue,
                    _ => {}
                }
                out.insert(k.clone(), p);
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(prune).collect()),
        other => other.clone(),
    }
}

fn write_value(v: &Value, out: &mut String) -> Result<(), String> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_str(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(x, out)?;
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_str(k, out);
                out.push(':');
                write_value(&m[k.as_str()], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        let cp = ch as u32;
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if cp < 0x20 || cp == 0x7f => out.push_str(&format!("\\u{:04x}", cp)),
            _ if cp >= 0x80 => {
                if cp < 0x10000 {
                    out.push_str(&format!("\\u{:04x}", cp));
                } else {
                    let w = cp - 0x10000;
                    out.push_str(&format!(
                        "\\u{:04x}\\u{:04x}",
                        0xD800 + (w >> 10),
                        0xDC00 + (w & 0x3FF)
                    ));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(s.as_bytes()))
}

// ── projection: receipt → certificate ──────────────────────────────────────

/// What the agent's other systems contribute to its identity document.
///
/// SID is the hub, and each subsystem keeps its own authority:
///   identity      -> GIX      (odu_link, kdf, merkle primitives)
///   memory        -> minipae  (NIP-AE engram, memory root, glyphs)
///   coordination  -> waggle   (stigmergic field keyed by agent id)
///   trace         -> mycelium (external chain head)
///   clock         -> OKF      (verified height + OTS)
///   calendar      -> Koodu    (epoch/cycle/phase)
///
/// Anything not supplied is recorded as absent. A missing subsystem is a fact
/// about the binding, never something to invent.
#[derive(Default, Clone)]
pub struct SidContext<'a> {
    pub manifest: Option<&'a crate::genesis::manifest::AgentManifest>,
    pub trace_chain_head: Option<&'a str>,
    pub profile_id: Option<&'a str>,
    /// The waggle field this agent deposits scent into. The kernel keys it by
    /// agent id (`WaggleField::new(agent_id)`), so it is deterministic.
    pub waggle_field: Option<&'a str>,
}

/// Render an `AgentGenesisReceipt` as a `sid/0.1` certificate (v1, finalized).
///
/// This is the tie-in point: the ceremony stays where it is, and the receipt
/// becomes a portable identity document. `status` is derived from the truth of
/// the receipt — `live` only when a Bitcoin height was actually obtained,
/// otherwise `cast` (the certificate says so rather than looking proven).
pub fn from_genesis_receipt(r: &AgentGenesisReceipt, name: Option<&str>) -> Result<Value, String> {
    from_genesis_receipt_with(r, name, &SidContext::default())
}

/// Project a receipt with the rest of the agent's systems bound in.
pub fn from_genesis_receipt_with(
    r: &AgentGenesisReceipt,
    name: Option<&str>,
    ctx: &SidContext,
) -> Result<Value, String> {
    // A birth block recorded by the kernel's clock is a CLAIM, not a proof: the
    // proof is an OpenTimestamps attestation, which the pipeline adds later.
    // So the certificate is cast at birth even when a height is known, and
    // `anchored_v2` is what promotes it to live. Say so in the document rather
    // than letting a recorded height look like an independent proof.
    let status = "cast";
    // Two authorities, never one: Koodu is the CALENDAR (epoch/cycle/phase,
    // derived from a height); OKF is the CLOCK (verified height + OTS proof).
    // At birth the height Koodu read is unattested, so the clock is recorded as
    // such and `anchored_v2` is what hands it to OKF.
    let clock_source = "unattested";
    let calendar = json!({
        "epoch": r.koodu_epoch,
        "cycle": r.koodu_cycle,
        "phase": r.koodu_phase,
        "basis": if r.gregorian_fallback { "gregorian" } else { "height" },
    });
    // A certificate whose calendar contradicts its own birth block must never
    // be emitted: Koodu and the clock would then disagree inside one document.
    if let Some(bh) = r.bitcoin_height {
        let (e, c, p) = crate::genesis::koodu_time::koodu_from_height(bh);
        if (r.koodu_epoch, r.koodu_cycle, r.koodu_phase) != (e, c, p) {
            return Err(format!(
                "Koodu calendar ({}, {}, {}) disagrees with birth block {bh}, which implies ({e}, {c}, {p})",
                r.koodu_epoch, r.koodu_cycle, r.koodu_phase
            ));
        }
    }
    let born_at = rfc3339_from_millis(r.born_at);
    let symbolic = name.unwrap_or(r.symbolic_address.as_str());
    let odu_secondary = (r.composed_odu & 0xff) as u8;

    // The certificate's identity is the receipt's identity. odu_link must equal
    // (primary << 8) | secondary — verify() enforces it, and composed_odu is
    // exactly that composition, so a receipt with inconsistent Odu fields is
    // surfaced rather than papered over.
    let mut identity = Map::new();
    identity.insert("name".into(), json!(symbolic));
    identity.insert("type".into(), json!("agent"));
    identity.insert("odu_link".into(), json!(r.composed_odu));
    identity.insert("odu_primary".into(), json!(r.primary_odu));
    identity.insert("odu_secondary".into(), json!(odu_secondary));
    identity.insert("npub".into(), Value::Null);
    identity.insert("principal".into(), json!(r.derivation_root_id));
    identity.insert("parent".into(), Value::Null);
    identity.insert("issuer".into(), Value::Null);
    identity.insert(
        "description".into(),
        json!(format!("{} · {}", r.temperament, r.orisha_alignment)),
    );

    let history = vec![json!({ "state": "cast", "at": born_at })];

    // The appendix holds the rest of the receipt: hashed, but never acted on.
    // Floats are stringified because the canonical form refuses non-integral
    // numbers (a hash must not depend on how a GPU-second rounded).
    let appendix = json!({
        "agent_id": r.agent_id,
        "genesis_hash": r.genesis_hash,
        "harmonic_signature": r.harmonic_signature,
        "sigil_hash": r.sigil_hash,
        "cloak_commitment": r.cloak_commitment,
        "koodu_gregorian_fallback": r.gregorian_fallback,
        "soul": {
            "temperament": r.temperament,
            "orisha_alignment": r.orisha_alignment,
            "destiny_threads": r.destiny_threads,
        },
        "minipae_pubkey": r.minipae_pubkey,
        "birth_memory_glyph": r.birth_memory_glyph,
        "hermetic_fingerprint": r.hermetic_fingerprint,
        "blockmesh_identity": r.blockmesh_identity,
        "vantage_identity": r.vantage_identity,
        "cold_archive_anchor": r.cold_archive_anchor,
        "contributed_gpu_seconds": r.contributed_gpu_seconds.map(|f| f.to_string()),
        "first_lease_id": r.first_lease_id,
        "first_work_id": r.first_work_id,
        "witness_receipt": r.witness_receipt,
        "memory_write_status": r.memory_write_status,
        "genesis_signature": r.genesis_signature,
        // An unsigned receipt is a real state (bootstrap births, gregorian
        // fallback), not something to paper over: the certificate says so.
        "genesis_signed": !r.genesis_signature.trim().is_empty(),
        "receipt_version": r.receipt_version,
    });

    let mut cert = json!({
        "format": FORMAT,
        "id": format!("sid_{}", slug(&r.agent_id)),
        "version": 1,
        "previous_version": Value::Null,
        "previous_hash": "none",
        "status": status,
        "born_at": born_at,
        "updated": born_at,
        "identity": Value::Object(identity),
        "genesis": {
            "entropy_source": "ifascript/cowrie-v1",
            "entropy_commitment": r.birth_entropy_commitment,
            "seed_hash": r.derivation_root_id,
            "keygen": "bipon39+derivation_root",
        },
        "time": {
            // clock — OKF's authority
            "birth_block": r.bitcoin_height,
            "birth_block_hash": r.bitcoin_anchor,
            "clock_source": clock_source,
            "ots_proof": Value::Null,
            "ots_status": Value::Null,
            "anchored": false,
            // calendar — Koodu's authority
            "calendar": calendar,
        },
        "memory": {
            // minipae: the NIP-AE address is the locator, the merkle root is
            // the namespace. Falls back to the receipt's IP-layer root when no
            // manifest was supplied.
            "locator": ctx
                .manifest
                .map(|m| m.memory.minipae_pubkey.clone())
                .filter(|v| !v.is_empty())
                .or_else(|| r.ip_root_event.clone()),
            "namespace_root": ctx
                .manifest
                .map(|m| m.memory.memory_root.clone())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| r.memory_root.clone()),
            "glyph_root": ctx.manifest.map(|m| m.memory.glyph_root.clone()),
            "birth_glyph": ctx
                .manifest
                .map(|m| m.memory.birth_glyph.clone())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| r.birth_memory_glyph.clone()),
        },
        "lifecycle": { "state": status, "history": history },
        "provenance": {
            // mycelium owns traces; a null head means "not supplied", not "none".
            "trace_chain_head": ctx.trace_chain_head,
            "profile_id": ctx.profile_id,
            "manifest_hash": ctx
                .manifest
                .and_then(|m| serde_json::to_value(m).ok())
                .and_then(|v| canonical_json(&v).ok())
                .map(|c| sha256_hex(&c)),
        },
        "changelog": [format!(
            "v1 — born from genesis receipt {} at {}",
            &r.genesis_hash[..r.genesis_hash.len().min(12)],
            born_at
        )],
        "appendix": appendix,
    });

    // capabilities — waggle/enforcement is the gate; the root is the manifest's
    // commitment to everything the agent may do.
    if let Some(root) = ctx
        .manifest
        .map(|m| m.identity.capability_root.clone())
        .filter(|v| !v.is_empty())
    {
        cert["capabilities"] = json!({ "root": root });
    }

    // appendix: which subsystem owns which function, so the document is a map
    // of the agent's real stack rather than a list of fields.
    cert["appendix"]["subsystems"] = json!({
        "identity": "gix",
        "memory": "minipae",
        "coordination": "waggle",
        "trace": "mycelium",
        "clock": "okf",
        "calendar": "koodu",
    });
    cert["appendix"]["coordination"] = json!({
        "field": ctx.waggle_field.unwrap_or(r.agent_id.as_str()),
        "kind": "waggle/stigmergic-field",
    });
    if let Some(m) = ctx.manifest {
        if let Ok(net) = serde_json::to_value(&m.network) {
            cert["appendix"]["network"] = net;
        }
    }

    finalize(&mut cert);
    let problems = verify(&cert);
    if !problems.is_empty() {
        return Err(format!(
            "the certificate projected from this receipt does not verify: {}",
            problems.join("; ")
        ));
    }
    Ok(cert)
}

/// Promote a `cast` certificate to `live` once the OKF pipeline has a real
/// OpenTimestamps proof. This is a MUTATION, so it mints v2 chained to v1 —
/// never an in-place edit.
///
/// Ownership: OKF supplies the CLOCK (proof, verified height, block hash);
/// Koodu supplies the CALENDAR. The promotion therefore checks that the
/// certificate's calendar is the one its anchored height actually implies —
/// if Koodu and the clock disagree, the certificate is refused rather than
/// carrying two contradictory facts.
///
/// `acting_hash` DOES change here, and should: `status` and `lifecycle` are
/// acting keys, so a trust promotion from cast to live must reach consumers.
pub fn anchored_v2(
    cert: &Value,
    ots_proof: &str,
    ots_status: &str,
    anchor_block: u64,
    anchor_block_hash: Option<&str>,
    at: &str,
) -> Result<Value, String> {
    let problems = verify(cert);
    if !problems.is_empty() {
        return Err(format!(
            "cannot anchor an invalid certificate: {}",
            problems.join("; ")
        ));
    }
    if cert.get("status").and_then(Value::as_str) != Some("cast") {
        return Err(format!(
            "only a cast certificate can be anchored (status is {})",
            cert.get("status").and_then(Value::as_str).unwrap_or("?")
        ));
    }
    if ots_proof.trim().is_empty() {
        return Err("anchoring requires an OTS proof path".into());
    }
    if ots_status != "complete" {
        return Err(format!(
            "the OTS proof is {ots_status}, not complete: it commits to a calendar \
             server but no Bitcoin block has attested it yet. The okf-ots-upgrade \
             timer completes it — re-run this once the attestation lands."
        ));
    }

    // The calendar is a BIRTH fact: it must be the coordinates the BIRTH height
    // implies. The anchor lands at a later block and does not rewrite birth.
    let birth_height = cert
        .get("time")
        .and_then(|t| t.get("birth_block"))
        .and_then(Value::as_u64);
    let calendar = cert
        .get("time")
        .and_then(|t| t.get("calendar"))
        .cloned()
        .unwrap_or(Value::Null);
    if let (Some(bh), Some(cal)) = (birth_height, calendar.as_object()) {
        let (e, c, p) = crate::genesis::koodu_time::koodu_from_height(bh);
        let same = cal.get("epoch").and_then(Value::as_u64) == Some(e)
            && cal.get("cycle").and_then(Value::as_u64) == Some(c)
            && cal.get("phase").and_then(Value::as_u64) == Some(p as u64);
        if !same {
            return Err(format!(
                "Koodu calendar disagrees with the birth clock: certificate says ({}, {}, {}), birth block {bh} implies ({e}, {c}, {p})",
                cal.get("epoch").and_then(Value::as_u64).unwrap_or(u64::MAX),
                cal.get("cycle").and_then(Value::as_u64).unwrap_or(u64::MAX),
                cal.get("phase").and_then(Value::as_u64).unwrap_or(u64::MAX),
            ));
        }
    }

    let v1_hash = cert.get("hash").and_then(Value::as_str).unwrap_or("").to_string();
    let v1_version = cert.get("version").and_then(Value::as_i64).unwrap_or(1);

    let mut out = cert.clone();
    let obj = out.as_object_mut().ok_or("certificate is not an object")?;
    obj.insert("version".into(), json!(v1_version + 1));
    obj.insert("previous_version".into(), json!(v1_version));
    obj.insert("previous_hash".into(), json!(v1_hash));
    obj.insert("status".into(), json!("live"));
    obj.insert("updated".into(), json!(at));
    obj.insert(
        "time".into(),
        json!({
            // clock — OKF
            // clock — OKF: the proof lands at anchor_block, which is NOT the
            // birth. Birth facts are preserved; proof facts are added.
            "birth_block": birth_height,
            "birth_block_hash": cert
                .get("time")
                .and_then(|t| t.get("birth_block_hash"))
                .cloned()
                .unwrap_or(Value::Null),
            "clock_source": "okf",
            "ots_proof": ots_proof,
            "ots_status": ots_status,
            "anchor_block": anchor_block,
            "anchor_block_hash": anchor_block_hash,
            "anchored": true,
            // calendar — Koodu, unchanged from birth
            "calendar": calendar,
        }),
    );

    let mut history = obj
        .get("lifecycle")
        .and_then(|l| l.get("history"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    history.push(json!({ "state": "live", "at": at }));
    obj.insert(
        "lifecycle".into(),
        json!({ "state": "live", "history": history }),
    );

    let mut changelog = obj
        .get("changelog")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    changelog.push(json!(format!(
        "v{} — anchored by the OKF clock: OTS proof over the v{v1_version} hash, proving existence at block {anchor_block} (Koodu calendar from birth block {})",
        v1_version + 1,
        birth_height.map(|h| h.to_string()).unwrap_or_else(|| "unknown".into())
    )));
    obj.insert("changelog".into(), Value::Array(changelog));

    finalize(&mut out);
    let problems = verify(&out);
    if !problems.is_empty() {
        return Err(format!(
            "anchored certificate does not verify: {}",
            problems.join("; ")
        ));
    }
    Ok(out)
}

/// Fill only ABSENT v1 defaults, then acting_hash, then hash (which covers it).
pub fn finalize(cert: &mut Value) {
    if cert.get("version").and_then(Value::as_i64) == Some(1) {
        if let Some(obj) = cert.as_object_mut() {
            if !obj.contains_key("previous_version") {
                obj.insert("previous_version".into(), Value::Null);
            }
            match obj.get("previous_hash") {
                Some(Value::Null) | None => {
                    obj.insert("previous_hash".into(), json!("none"));
                }
                _ => {}
            }
        }
    }
    if let Ok(ah) = acting_hash(cert) {
        cert["acting_hash"] = json!(ah);
    }
    if let Ok(h) = sid_hash(cert) {
        cert["hash"] = json!(h);
    }
}

pub fn verify(cert: &Value) -> Vec<String> {
    let mut e: Vec<String> = Vec::new();
    let get = |k: &str| cert.get(k);

    if get("format").and_then(Value::as_str) != Some(FORMAT) {
        e.push(format!("format must be {FORMAT}"));
    }
    match get("id").and_then(Value::as_str) {
        Some(id) if id.starts_with("sid_") && id.len() > 4 => {}
        _ => e.push("id must match sid_<slug>".into()),
    }
    let version = get("version").and_then(Value::as_i64);
    if !matches!(version, Some(v) if v >= 1) {
        e.push("version must be an integer >= 1".into());
    }
    let status = get("status").and_then(Value::as_str).unwrap_or("");
    if !STATUSES.contains(&status) {
        e.push("status must be cast|live|suspended|retired".into());
    }
    if get("born_at").and_then(Value::as_str).unwrap_or("").is_empty() {
        e.push("born_at is required".into());
    }

    if let Some(i) = get("identity") {
        if i.get("type").and_then(Value::as_str) != Some("agent") {
            e.push("identity.type must be \"agent\"".into());
        }
        let p = i.get("odu_primary").and_then(Value::as_u64);
        let s = i.get("odu_secondary").and_then(Value::as_u64);
        let l = i.get("odu_link").and_then(Value::as_u64);
        match (p, s, l) {
            (Some(p), Some(s), Some(l)) => {
                if p > 255 || s > 255 {
                    e.push("odu_primary/odu_secondary must be 0-255".into());
                }
                let expected = (p << 8) | s;
                if l != expected {
                    e.push(format!(
                        "identity.odu_link {l} != (odu_primary<<8|odu_secondary) {expected}"
                    ));
                }
            }
            _ => e.push("identity.odu_primary/odu_secondary/odu_link must be integers".into()),
        }
    } else {
        e.push("identity is required".into());
    }

    match get("hash").and_then(Value::as_str) {
        Some(h) if is_hex64(h) => match sid_hash(cert) {
            Ok(calc) if calc == h => {}
            Ok(calc) => e.push(format!("hash does not match the canonical body ({calc})")),
            Err(err) => e.push(format!("hash could not be computed: {err}")),
        },
        _ => e.push("hash must be 64-hex".into()),
    }
    match get("acting_hash").and_then(Value::as_str) {
        Some(h) if is_hex64(h) => match acting_hash(cert) {
            Ok(calc) if calc == h => {}
            Ok(calc) => e.push(format!("acting_hash does not match the acting keys ({calc})")),
            Err(err) => e.push(format!("acting_hash could not be computed: {err}")),
        },
        _ => e.push("acting_hash must be 64-hex".into()),
    }

    if version == Some(1) {
        if !matches!(get("previous_version"), Some(Value::Null)) {
            e.push("v1 must have previous_version = null".into());
        }
        match get("previous_hash") {
            Some(Value::String(s)) if s == "none" => {}
            Some(Value::Null) => {}
            _ => e.push("v1 must have previous_hash = \"none\"".into()),
        }
    } else if let Some(v) = version {
        if get("previous_version").and_then(Value::as_i64) != Some(v - 1) {
            e.push("previous_version must be version - 1".into());
        }
        if !get("previous_hash")
            .and_then(Value::as_str)
            .map(is_hex64)
            .unwrap_or(false)
        {
            e.push("previous_hash must be 64-hex for v>1".into());
        }
    }

    if let Some(state) = get("lifecycle").and_then(|l| l.get("state")).and_then(Value::as_str) {
        if state != status {
            e.push("lifecycle.state must equal status".into());
        }
    }
    let t = get("time");
    let anchored = t.and_then(|x| x.get("anchored")).and_then(Value::as_bool) == Some(true);
    if anchored
        && t.and_then(|x| x.get("ots_proof"))
            .map(Value::is_null)
            .unwrap_or(true)
    {
        e.push("time.anchored = true requires time.ots_proof".into());
    }
    if anchored
        && t.and_then(|x| x.get("ots_status"))
            .and_then(Value::as_str)
            != Some("complete")
    {
        // A fresh OTS stamp is a commitment to a calendar server, not a Bitcoin
        // attestation. Claiming `anchored` on it is the exact overclaim this
        // format exists to prevent.
        e.push(
            "time.anchored = true requires time.ots_status = \"complete\" (a pending proof is not an attestation)"
                .into(),
        );
    }
    if anchored && t.and_then(|x| x.get("clock_source")).and_then(Value::as_str) != Some("okf") {
        // The clock has exactly one owner. An anchored certificate whose clock
        // did not come from the pipeline is claiming time it cannot prove.
        e.push("time.anchored = true requires time.clock_source = \"okf\"".into());
    }
    if let Some(cal) = t.and_then(|x| x.get("calendar")) {
        for k in ["epoch", "cycle", "phase"] {
            if cal.get(k).and_then(Value::as_u64).is_none() {
                e.push(format!("time.calendar.{k} must be a non-negative integer"));
            }
        }
    }

    for forbidden in ["seed", "mnemonic", "private_key", "nsec", "secret"] {
        for section in ["genesis", "identity"] {
            if get(section).and_then(|s| s.get(forbidden)).is_some() {
                e.push(format!("{section} must not carry {forbidden}"));
            }
        }
    }
    e
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

// ── Markdown ───────────────────────────────────────────────────────────────

/// Emit `SID.md`: flat frontmatter plus per-section machine comments so the
/// document round-trips to JSON losslessly.
pub fn emit_md(cert: &Value, changelog: &[String]) -> Result<String, String> {
    let h = sid_hash(cert)?;
    let ah = acting_hash(cert)?;
    let s = |k: &str| cert.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let version = cert.get("version").and_then(Value::as_i64).unwrap_or(0);

    let mut fm = vec![
        "---".to_string(),
        format!("format: {FORMAT}"),
        format!("id: {}", s("id")),
        format!("version: {version}"),
    ];
    if version == 1 {
        fm.push("previous_version: null".into());
        fm.push("previous_hash: none".into());
    } else {
        fm.push(format!(
            "previous_version: {}",
            cert.get("previous_version").and_then(Value::as_i64).unwrap_or(0)
        ));
        fm.push(format!("previous_hash: {}", s("previous_hash")));
    }
    fm.push(format!("hash: {h}"));
    fm.push(format!("acting_hash: {ah}"));
    fm.push(format!("status: {}", s("status")));
    fm.push(format!("born_at: {}", s("born_at")));
    if !s("updated").is_empty() {
        fm.push(format!("updated: {}", s("updated")));
    }
    fm.push("---".into());

    let name = cert
        .get("identity")
        .and_then(|i| i.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| cert.get("id").and_then(Value::as_str).unwrap_or("agent"));

    let mut out = vec![
        fm.join("\n"),
        String::new(),
        format!("# SID — {name}"),
        String::new(),
    ];
    for (key, title) in [
        ("identity", "Identity"),
        ("genesis", "Genesis"),
        ("time", "Time"),
        ("capabilities", "Capabilities"),
        ("memory", "Memory"),
        ("provenance", "Provenance"),
        ("lifecycle", "Lifecycle"),
    ] {
        if let Some(v) = cert.get(key) {
            out.push(format!("## {title}"));
            out.push(String::new());
            out.push(format!(
                "<!-- sid:{key}@1 {} -->",
                serde_json::to_string(v).map_err(|e| e.to_string())?
            ));
            out.push(String::new());
        }
    }
    if !changelog.is_empty() {
        out.push("## Changelog".into());
        out.push(String::new());
        for line in changelog {
            out.push(format!("- {line}"));
        }
        out.push(String::new());
    }
    if let Some(a) = cert.get("appendix") {
        out.push("## Appendix".into());
        out.push(String::new());
        out.push(format!(
            "<!-- sid:appendix@1 {} -->",
            serde_json::to_string(a).map_err(|e| e.to_string())?
        ));
    }
    Ok(out.join("\n"))
}

/// Parse `SID.md` back to the certificate JSON.
pub fn parse_md(md: &str) -> Result<Value, String> {
    let mut lines = md.lines();
    if lines.next() != Some("---") {
        return Err("SID.md: missing frontmatter".into());
    }
    let mut cert = Map::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        let (k, v) = line.split_once(':').ok_or_else(|| format!("bad frontmatter line: {line}"))?;
        let k = k.trim().to_string();
        let v = v.trim();
        let val = match v {
            "null" => Value::Null,
            _ if v.parse::<i64>().is_ok() => json!(v.parse::<i64>().unwrap()),
            _ => json!(v),
        };
        cert.insert(k, val);
    }
    if !closed {
        return Err("SID.md: frontmatter not terminated".into());
    }
    let mut in_changelog = false;
    let mut changelog: Vec<Value> = Vec::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix("<!-- sid:") {
            if let Some((key, payload)) = rest.split_once("@1 ") {
                let payload = payload.trim_end_matches(" -->").trim();
                let v: Value =
                    serde_json::from_str(payload).map_err(|e| format!("bad {key} block: {e}"))?;
                cert.insert(key.to_string(), v);
            }
            continue;
        }
        if line.trim() == "## Changelog" {
            in_changelog = true;
            continue;
        }
        if in_changelog {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entry) = line.strip_prefix("- ") {
                changelog.push(json!(entry));
            } else {
                in_changelog = false; // a new section started
            }
        }
    }
    if !changelog.is_empty() {
        cert.insert("changelog".into(), Value::Array(changelog));
    }
    Ok(Value::Object(cert))
}

// ── helpers ────────────────────────────────────────────────────────────────

pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let t = out.trim_matches('-').to_string();
    if t.is_empty() {
        "agent".into()
    } else {
        t
    }
}

/// Unix milliseconds -> `YYYY-MM-DDTHH:MM:SSZ` (civil-from-days, no chrono).
pub fn rfc3339_from_millis(millis: u64) -> String {
    let secs = millis / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> AgentGenesisReceipt {
        AgentGenesisReceipt {
            agent_id: "agent-0123456789abcdef".into(),
            genesis_hash: "a".repeat(64),
            birth_entropy_commitment: "b".repeat(64),
            harmonic_signature: "c".repeat(64),
            derivation_root_id: "d".repeat(64),
            symbolic_address: "oracle-7".into(),
            sigil_hash: "e".repeat(64),
            cloak_commitment: "f".repeat(64),
            born_at: 1_762_822_620_000,
            koodu_epoch: 3,   // koodu_from_height(975218)
            koodu_cycle: 37,
            koodu_phase: 4,
            bitcoin_height: Some(975_218),
            bitcoin_anchor: Some("0".repeat(64)),
            gregorian_fallback: false,
            primary_odu: 18,
            composed_odu: 4711,
            temperament: "steady".into(),
            orisha_alignment: "Ògún".into(),
            destiny_threads: vec!["forge".into()],
            minipae_pubkey: "1".repeat(64),
            memory_root: "2".repeat(64),
            birth_memory_glyph: "3".repeat(64),
            ip_root_event: None,
            device_binding: None,
            hermetic_fingerprint: "4".repeat(64),
            blockmesh_identity: None,
            vantage_identity: None,
            cold_archive_anchor: None,
            contributed_gpu_seconds: Some(12.5),
            first_lease_id: None,
            first_work_id: None,
            witness_receipt: None,
            memory_write_status: Default::default(),
            genesis_signature: "5".repeat(64),
            receipt_version: 2,
        }
    }

    #[test]
    fn projects_a_verifying_certificate() {
        let cert = from_genesis_receipt(&receipt(), None).expect("projects");
        assert!(verify(&cert).is_empty(), "{:?}", verify(&cert));
        assert_eq!(cert["format"], FORMAT);
        assert_eq!(cert["id"], "sid_agent-0123456789abcdef");
        // A recorded height is a clock reading, not an OTS proof: still cast.
        assert_eq!(cert["status"], "cast");
        assert_eq!(cert["time"]["anchored"], false);
        assert_eq!(cert["time"]["clock_source"], "unattested"); // OKF has not spoken
        assert_eq!(cert["time"]["calendar"]["epoch"], 3);       // Koodu
        assert_eq!(cert["time"]["calendar"]["cycle"], 37);
        assert_eq!(cert["time"]["calendar"]["phase"], 4);
        assert_eq!(cert["identity"]["odu_link"], 4711);
        assert_eq!(cert["identity"]["odu_secondary"], 103); // 4711 & 0xff
        assert_eq!(cert["time"]["birth_block"], 975_218);
        // a non-integral float in the receipt must not break the hash
        assert_eq!(cert["appendix"]["contributed_gpu_seconds"], "12.5");
    }

    #[test]
    fn anchoring_mints_v2_and_leaves_acting_hash_untouched() {
        let v1 = from_genesis_receipt(&receipt(), None).unwrap();
        assert_eq!(v1["status"], "cast");
        let v2 = anchored_v2(&v1, "receipts/x.ots", "complete", 975_400, Some(&"9".repeat(64)), "2026-11-11T02:00:00Z")
            .expect("anchors");
        assert_eq!(v2["status"], "live");
        assert_eq!(v2["version"], 2);
        assert_eq!(v2["previous_hash"], v1["hash"]);
        assert_eq!(v2["time"]["anchored"], true);
        // the anchor is a LATER block; birth is untouched
        assert_eq!(v2["time"]["anchor_block"], 975_400);
        assert_eq!(v2["time"]["birth_block"], 975_218);
        assert_eq!(v2["time"]["calendar"]["phase"], 4);
        assert_eq!(v2["time"]["clock_source"], "okf");
        assert!(verify(&v2).is_empty(), "{:?}", verify(&v2));
        // status and lifecycle are acting keys, so the trust promotion MUST
        // reach consumers: acting_hash moves with it.
        assert_ne!(acting_hash(&v1).unwrap(), acting_hash(&v2).unwrap());
        assert_ne!(v1["hash"], v2["hash"]);
        // ...while the Odu identity the agent acts AS does not change.
        assert_eq!(v1["identity"], v2["identity"]);
        // history is appended, not rewritten
        assert_eq!(v2["lifecycle"]["history"].as_array().unwrap().len(), 2);
        // and a live certificate cannot be anchored twice
        assert!(anchored_v2(&v2, "again", "complete", 1, None, "2026-11-11T03:00:00Z").is_err());
    }

    #[test]
    fn calendar_must_match_the_birth_height() {
        let mut r = receipt();
        r.koodu_phase = 9; // not what block 975218 implies
        let err = from_genesis_receipt(&r, None).unwrap_err();
        // projection verifies, so an inconsistent calendar is refused outright
        assert!(err.contains("calendar") || err.contains("verif"), "{err}");
    }

    #[test]
    fn binds_every_agent_subsystem() {
        use crate::genesis::manifest::AgentManifest;
        let mut m = AgentManifest::default();
        m.identity.capability_root = "ab".repeat(32);
        m.memory.minipae_pubkey = "cd".repeat(32);
        m.memory.memory_root = "ef".repeat(32);
        m.memory.glyph_root = "12".repeat(32);
        m.memory.birth_glyph = "34".repeat(32);
        m.network.ip_root_event = Some("evt-1".into());

        let ctx = SidContext {
            manifest: Some(&m),
            trace_chain_head: Some(&"9a".repeat(32)),
            profile_id: Some("profile-okf"),
            waggle_field: None,
        };
        let cert = from_genesis_receipt_with(&receipt(), None, &ctx).expect("projects");
        assert!(verify(&cert).is_empty(), "{:?}", verify(&cert));

        // capabilities <- manifest
        assert_eq!(cert["capabilities"]["root"], "ab".repeat(32));
        // memory <- minipae
        assert_eq!(cert["memory"]["locator"], "cd".repeat(32));
        assert_eq!(cert["memory"]["namespace_root"], "ef".repeat(32));
        assert_eq!(cert["memory"]["glyph_root"], "12".repeat(32));
        assert_eq!(cert["memory"]["birth_glyph"], "34".repeat(32));
        // provenance <- mycelium + runtime profile + manifest commitment
        assert_eq!(cert["provenance"]["trace_chain_head"], "9a".repeat(32));
        assert_eq!(cert["provenance"]["profile_id"], "profile-okf");
        assert!(cert["provenance"]["manifest_hash"].as_str().unwrap().len() == 64);
        // the binding map names the owner of each function
        assert_eq!(cert["appendix"]["subsystems"]["memory"], "minipae");
        assert_eq!(cert["appendix"]["subsystems"]["coordination"], "waggle");
        assert_eq!(cert["appendix"]["subsystems"]["trace"], "mycelium");
        assert_eq!(cert["appendix"]["subsystems"]["clock"], "okf");
        assert_eq!(cert["appendix"]["subsystems"]["calendar"], "koodu");
        // waggle field is keyed by agent id, so it is deterministic
        assert_eq!(cert["appendix"]["coordination"]["field"], "agent-0123456789abcdef");
        // network bindings ride along
        assert_eq!(cert["appendix"]["network"]["ip_root_event"], "evt-1");
    }

    #[test]
    fn absent_subsystems_are_absent_not_invented() {
        let cert = from_genesis_receipt(&receipt(), None).unwrap();
        assert!(cert.get("capabilities").is_none(), "no manifest -> no capabilities");
        assert!(cert["provenance"].get("trace_chain_head").is_none()
            || cert["provenance"]["trace_chain_head"].is_null());
        assert_eq!(cert["memory"]["locator"], Value::Null); // no ip_root_event, no manifest
        assert!(verify(&cert).is_empty());
    }

    #[test]
    fn unanchored_receipt_is_cast_not_live() {
        let mut r = receipt();
        r.bitcoin_height = None;
        r.gregorian_fallback = true;
        let cert = from_genesis_receipt(&r, None).expect("projects");
        assert_eq!(cert["status"], "cast");
        assert_eq!(cert["time"]["anchored"], false);
        assert!(verify(&cert).is_empty());
    }

    #[test]
    fn markdown_round_trips() {
        let cert = from_genesis_receipt(&receipt(), None).unwrap();
        let changelog: Vec<String> = cert["changelog"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let md = emit_md(&cert, &changelog).unwrap();
        let back = parse_md(&md).unwrap();
        assert_eq!(sid_hash(&back).unwrap(), sid_hash(&cert).unwrap());
        assert_eq!(back["identity"], cert["identity"]);
    }

    #[test]
    fn tampering_is_detected() {
        let mut cert = from_genesis_receipt(&receipt(), None).unwrap();
        cert["identity"]["name"] = json!("impostor");
        let problems = verify(&cert);
        assert!(problems.iter().any(|p| p.contains("hash does not match")), "{problems:?}");
    }

    #[test]
    fn inconsistent_odu_link_is_refused() {
        let mut r = receipt();
        r.composed_odu = 1; // no longer (18 << 8) | x
        let err = from_genesis_receipt(&r, None).unwrap_err();
        assert!(err.contains("odu_link"), "{err}");
    }

    #[test]
    fn canonical_json_matches_python_shape() {
        let v = json!({"b":[1,2,{"z":1,"a":2}],"a":"x","empty_arr":[],"nested":{"gone":[],"keep":true}});
        assert_eq!(
            canonical_json(&v).unwrap(),
            r#"{"a":"x","b":[1,2,{"a":2,"z":1}],"nested":{"keep":true}}"#
        );
    }

    #[test]
    fn non_ascii_matches_python() {
        // Vectors computed by the Python canonicalizer (see sid repo conformance).
        let v = json!({
            "format":"sid/0.1","id":"sid_ogun-test-1","version":1,"previous_version":Value::Null,
            "previous_hash":"none","status":"cast","born_at":"2026-11-11T00:00:00Z",
            "identity":{"name":"Ògún-forge","type":"agent","odu_link":4711,"odu_primary":18,"odu_secondary":103},
            "lifecycle":{"state":"cast","history":[{"state":"cast","at":"2026-11-11T00:00:00Z"}]}
        });
        assert_eq!(
            sid_hash(&v).unwrap(),
            "87b9e2745b284c43a041d20f90fdf7284c3598fa2da187bb07888b36528c4290"
        );
        assert_eq!(
            acting_hash(&v).unwrap(),
            "dd9eb5da15159b51cbdb54e9ad573137f719dd37cd67533e8ab30fe3bdece7ea"
        );
    }
}
