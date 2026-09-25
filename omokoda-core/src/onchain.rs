//! Real, optional on-chain birth minting via a deployed Sui Move contract
//! (`omokoda::garden::register_agent`, package
//! `0x380e0599702b7ebd9005b02f36dd611cff209c94ca678f051233346cf7dbf22e`,
//! testnet). Shells out to the `sui` CLI -- the same "no SDK, real
//! binary, subprocess call" pattern already used for Hermes, since a full
//! Sui Rust SDK is a much heavier dependency than this kernel needs for
//! one entry-function call.
//!
//! Fail-open by design, matching Vantage registration's own pattern: if
//! `sui` isn't installed, gas is exhausted, or the network call fails,
//! birth proceeds without an on-chain record rather than blocking --
//! being born is never allowed to depend on blockchain availability. Same
//! fail-open discipline applies to the update calls below: a failed
//! on-chain update never blocks or fails a real think/act.
//!
//! `register_agent` mints the real `AgentInfo` object at birth.
//! `update_agent_stats` and `update_glyph_signal` (package v2, upgraded
//! live -- see GARDEN_PACKAGE_V2) make it genuinely dynamic: reputation/
//! tier are mutated in place on the existing object, and the glyph-index
//! divination signal (see divination.rs) is attached via Sui dynamic
//! fields -- traits that did not exist at mint time and evolve as the
//! agent actually thinks. Both are owner-gated in Move (only the minting
//! wallet's address may call them on a given AgentInfo).
//!
//! Configured via env vars, nothing hardcoded beyond the constants above
//! which name the actual deployed package this kernel talks to:
//!   OMOKODA_SUI_REGISTRY    - shared AgentRegistry object id (required;
//!                             unset = minting silently skipped)
//!   OMOKODA_SUI_GAS_BUDGET  - optional, default 20_000_000 MIST (~0.02 SUI)

const GARDEN_PACKAGE: &str = "0x380e0599702b7ebd9005b02f36dd611cff209c94ca678f051233346cf7dbf22e";
/// `update_agent_stats` and `update_glyph_signal` only exist from package
/// version 2 onward (a Sui upgrade doesn't retrofit new functions onto the
/// original package id -- the bytecode at GARDEN_PACKAGE is immutable).
/// register_agent works at either address; the update functions require
/// this one.
const GARDEN_PACKAGE_V2: &str =
    "0xb2108b39f975bf9e20972a8752df0c7b0f014d9f91031696affecd760632b630";
const DEFAULT_GAS_BUDGET: &str = "20000000";

/// Mint a real on-chain `AgentInfo` object for a newborn agent. Returns
/// the minted object's id on success, `None` on any failure (missing
/// config, missing `sui` binary, insufficient gas, network error) --
/// callers must treat `None` as "no on-chain record yet", never as a
/// reason to fail the birth itself.
pub async fn mint_onchain_agent(name: &str) -> Option<String> {
    let registry = std::env::var("OMOKODA_SUI_REGISTRY").ok()?;
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    // Move `vector<u8>` argument as a JSON array of byte values --
    // `sui client call`'s CLI arg-parsing accepts this literally.
    let name_arg = format!(
        "[{}]",
        name.bytes()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );

    let output = tokio::process::Command::new("sui")
        .args([
            "client",
            "call",
            "--package",
            GARDEN_PACKAGE,
            "--module",
            "garden",
            "--function",
            "register_agent",
            "--args",
            &registry,
            &name_arg,
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[onchain] register_agent failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    extract_minted_agent_info_id(&json)
}

/// Mutate an existing AgentInfo's reputation/tier in place. Returns true
/// on a real, confirmed on-chain success. Fail-open: any failure is
/// logged and returns false, never propagated as an error the caller
/// must handle -- an on-chain stat refresh is a nice-to-have, not a
/// dependency for real/act to function.
pub async fn update_onchain_stats(nft_id: &str, reputation: u64, tier: u8) -> bool {
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());
    let reputation_arg = reputation.to_string();
    let tier_arg = tier.to_string();

    let Ok(output) = tokio::process::Command::new("sui")
        .args([
            "client",
            "call",
            "--package",
            GARDEN_PACKAGE_V2,
            "--module",
            "garden",
            "--function",
            "update_agent_stats",
            "--args",
            nft_id,
            &reputation_arg,
            &tier_arg,
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await
    else {
        return false;
    };

    if !output.status.success() {
        eprintln!(
            "[onchain] update_agent_stats failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }
    true
}

/// Attach/refresh the agent's glyph-index divination signal on-chain
/// (see divination.rs::recurrence_signal) -- the genuinely dynamic part
/// of the NFT: these fields did not exist at mint and evolve as the
/// agent actually thinks. Same fail-open contract as update_onchain_stats.
pub async fn update_onchain_glyph_signal(
    nft_id: &str,
    dominant_glyph: u8,
    recurrence_count: u64,
    timestamp: u64,
) -> bool {
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());
    let glyph_arg = dominant_glyph.to_string();
    let recurrence_arg = recurrence_count.to_string();
    let timestamp_arg = timestamp.to_string();

    let Ok(output) = tokio::process::Command::new("sui")
        .args([
            "client",
            "call",
            "--package",
            GARDEN_PACKAGE_V2,
            "--module",
            "garden",
            "--function",
            "update_glyph_signal",
            "--args",
            nft_id,
            &glyph_arg,
            &recurrence_arg,
            &timestamp_arg,
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await
    else {
        return false;
    };

    if !output.status.success() {
        eprintln!(
            "[onchain] update_glyph_signal failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return false;
    }
    true
}

fn extract_minted_agent_info_id(json: &serde_json::Value) -> Option<String> {
    let changes = json.get("objectChanges")?.as_array()?;
    changes.iter().find_map(|o| {
        let obj_type = o.get("objectType")?.as_str()?;
        let change_type = o.get("type")?.as_str()?;
        if change_type == "created" && obj_type.ends_with("::garden::AgentInfo") {
            o.get("objectId")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// Ṣàngó — SkillForge Audit stage: on-chain audit receipts
// ---------------------------------------------------------------------------

/// Standalone package, published because the `garden` package's UpgradeCap
/// is owned by an address not present in this deployment's keystore --
/// same "separate named package constant" pattern as `GARDEN_PACKAGE` /
/// `GARDEN_PACKAGE_V2` above. Module `audit`, function `record`.
const SKILLFORGE_AUDIT_PACKAGE: &str =
    "0x8f15cdd07cd9eedd403d461aa6ea4ae6b6a2e0c69ac0c2a3c1ea440475a57425";

/// Anchor one SkillForge audit decision on-chain: a durable, content-
/// addressed (hash-only, never the raw name/URL) proof that this repo was
/// reviewed and what the verdict was. Fail-open, matching every other
/// on-chain call in this module: `None`/`OMOKODA_SUI_REGISTRY` unset never
/// blocks or fails the forge -- the receipt is a nice-to-have audit trail,
/// not a dependency for SkillForge to function. Reuses `OMOKODA_SUI_REGISTRY`
/// only as the "is on-chain configured at all" signal (this call takes no
/// registry object argument), so a runtime with on-chain birth minting
/// enabled gets audit anchoring for free.
pub async fn record_skillforge_audit(
    skill_name: &str,
    source_url: &str,
    risk_score: u32,
    requires_review: bool,
    approved: bool,
) -> Option<String> {
    std::env::var("OMOKODA_SUI_REGISTRY").ok()?;
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    let name_hash = blake3::hash(skill_name.as_bytes());
    let url_hash = blake3::hash(source_url.as_bytes());
    let to_vec_arg = |h: &blake3::Hash| {
        format!(
            "[{}]",
            h.as_bytes()
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let output = tokio::process::Command::new("sui")
        .args([
            "client",
            "call",
            "--package",
            SKILLFORGE_AUDIT_PACKAGE,
            "--module",
            "audit",
            "--function",
            "record",
            "--args",
            &to_vec_arg(&name_hash),
            &to_vec_arg(&url_hash),
            &risk_score.to_string(),
            &requires_review.to_string(),
            &approved.to_string(),
            &timestamp.to_string(),
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[onchain] skillforge_audit record failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let changes = json.get("objectChanges")?.as_array()?;
    changes.iter().find_map(|o| {
        let obj_type = o.get("objectType")?.as_str()?;
        let change_type = o.get("type")?.as_str()?;
        if change_type == "created" && obj_type.ends_with("::audit::AuditReceipt") {
            o.get("objectId")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    })
}

/// Transfer ownership of an already-minted on-chain object (agent NFT,
/// skill-audit receipt, etc.) to a new Sui address via the generic
/// `sui client transfer` CLI verb -- there is no bespoke Move entry
/// function for this in `garden.move`, only the initial
/// `transfer::public_transfer` at mint time, so the generic CLI transfer
/// is the real mechanism. Returns `true` on a successful on-chain
/// transfer, `false` on any failure (bad address, `sui` missing,
/// insufficient gas, object not owned by this wallet).
pub async fn transfer_object(object_id: &str, to_address: &str) -> bool {
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    let output = tokio::process::Command::new("sui")
        .args([
            "client",
            "transfer",
            "--object-id",
            object_id,
            "--to",
            to_address,
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            eprintln!(
                "[onchain] transfer_object failed: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            false
        }
        Err(e) => {
            eprintln!("[onchain] transfer_object: sui binary unavailable: {e}");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// GlyphIndex — memory-graph merkle anchoring
// ---------------------------------------------------------------------------

/// Standalone package for `glyph_anchor::anchor::record(merkle_root: vector<u8>,
/// node_count: u64, owner_hash: vector<u8>, timestamp: u64)`. Not yet
/// published (blocked on testnet gas at the time this was written -- see
/// `OMOKODA_GLYPH_ANCHOR_PACKAGE` below); until it is, this fails open on the
/// missing env var exactly like every other on-chain call in this module,
/// and the caller still gets the real, locally-computed merkle root back --
/// only the durable on-chain receipt is skipped, never the computation.
const GLYPH_ANCHOR_PACKAGE_ENV: &str = "OMOKODA_GLYPH_ANCHOR_PACKAGE";

/// Anchor a GlyphIndex memory-graph merkle root on-chain: a durable,
/// content-addressed (hash-only, no plaintext, no individual memory
/// contents) proof that "this agent held exactly this set of memories at
/// this time." Fail-open: `None` if `OMOKODA_GLYPH_ANCHOR_PACKAGE` is unset,
/// `sui` is unavailable, or the call fails -- anchoring is a durability
/// nice-to-have, never a dependency for the graph itself to work (the
/// caller already has the root from `larql_glyph::merkle_root` regardless).
pub async fn record_glyph_anchor(
    merkle_root_hex: &str,
    node_count: u64,
    owner: &str,
) -> Option<String> {
    let package = std::env::var(GLYPH_ANCHOR_PACKAGE_ENV).ok()?;
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    let root_bytes = hex::decode(merkle_root_hex).ok()?;
    let owner_hash = blake3::hash(owner.as_bytes());
    let to_vec_arg = |bytes: &[u8]| {
        format!(
            "[{}]",
            bytes.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
        )
    };
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let output = tokio::process::Command::new("sui")
        .args([
            "client",
            "call",
            "--package",
            &package,
            "--module",
            "anchor",
            "--function",
            "record",
            "--args",
            &to_vec_arg(&root_bytes),
            &node_count.to_string(),
            &to_vec_arg(owner_hash.as_bytes()),
            &timestamp.to_string(),
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[onchain] glyph anchor record failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let changes = json.get("objectChanges")?.as_array()?;
    changes.iter().find_map(|o| {
        let obj_type = o.get("objectType")?.as_str()?;
        let change_type = o.get("type")?.as_str()?;
        if change_type == "created" && obj_type.ends_with("::anchor::AnchorReceipt") {
            o.get("objectId")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_agent_info_object_from_a_real_response_shape() {
        // Shape verified live against a real testnet transaction
        // (objectId 0xcb792f0c...), trimmed to the fields this parser
        // actually reads.
        let json = serde_json::json!({
            "objectChanges": [
                {
                    "type": "mutated",
                    "objectType": "0x2::coin::Coin<0x2::sui::SUI>",
                    "objectId": "0xsomecoin"
                },
                {
                    "type": "created",
                    "objectType": "0x380e0599702b7ebd9005b02f36dd611cff209c94ca678f051233346cf7dbf22e::garden::AgentInfo",
                    "objectId": "0xcb792f0c6c4b8cb55e7fd0fafb7896ba0b98f6d6f33bd010d8692cff1935c034"
                }
            ]
        });
        assert_eq!(
            extract_minted_agent_info_id(&json),
            Some("0xcb792f0c6c4b8cb55e7fd0fafb7896ba0b98f6d6f33bd010d8692cff1935c034".to_string())
        );
    }

    #[test]
    fn no_agent_info_object_returns_none() {
        let json = serde_json::json!({
            "objectChanges": [
                {"type": "mutated", "objectType": "0x2::coin::Coin<0x2::sui::SUI>", "objectId": "0xsomecoin"}
            ]
        });
        assert_eq!(extract_minted_agent_info_id(&json), None);
    }

    #[test]
    fn malformed_response_returns_none_not_a_panic() {
        let json = serde_json::json!({"unexpected": "shape"});
        assert_eq!(extract_minted_agent_info_id(&json), None);
    }

    #[test]
    fn parses_a_settlement_receipt_from_a_realistic_response_shape() {
        // Shape modeled on route_transaction_tax's actual event struct
        // (elegbara_router.move: EsuTaxCollected { gross, tax, net }) plus
        // the digest field every sui client call --json response carries.
        let json = serde_json::json!({
            "digest": "7xK3z9mP2vQ8nR4tY6wL1cB5dF0gH3jK9mN2pQ4rS6tU",
            "events": [
                {
                    "type": "0xabc123::elegbara_router::EsuTaxCollected",
                    "parsedJson": {
                        "gross": "1000000",
                        "tax": "36900",
                        "net": "963100"
                    }
                }
            ]
        });
        assert_eq!(
            parse_settlement_receipt(&json),
            Some(SettlementReceipt {
                tx_digest: "7xK3z9mP2vQ8nR4tY6wL1cB5dF0gH3jK9mN2pQ4rS6tU".to_string(),
                gross: 1_000_000,
                tax: 36_900,
                net: 963_100,
            })
        );
    }

    #[test]
    fn settlement_receipt_accepts_numeric_or_string_u64_fields() {
        // Defensive: don't assume the RPC always stringifies u64s.
        let json = serde_json::json!({
            "digest": "abc",
            "events": [
                {
                    "type": "0xabc::elegbara_router::EsuTaxCollected",
                    "parsedJson": {"gross": 100, "tax": 4, "net": 96}
                }
            ]
        });
        assert_eq!(
            parse_settlement_receipt(&json),
            Some(SettlementReceipt {
                tx_digest: "abc".to_string(),
                gross: 100,
                tax: 4,
                net: 96,
            })
        );
    }

    #[test]
    fn settlement_receipt_ignores_unrelated_events() {
        let json = serde_json::json!({
            "digest": "abc",
            "events": [
                {"type": "0xabc::garden::AgentBorn", "parsedJson": {"agent": "x"}}
            ]
        });
        assert_eq!(parse_settlement_receipt(&json), None);
    }

    #[test]
    fn settlement_receipt_missing_digest_returns_none() {
        let json = serde_json::json!({
            "events": [
                {
                    "type": "0xabc::elegbara_router::EsuTaxCollected",
                    "parsedJson": {"gross": "1", "tax": "1", "net": "0"}
                }
            ]
        });
        assert_eq!(parse_settlement_receipt(&json), None);
    }

    #[test]
    fn settlement_receipt_malformed_response_returns_none_not_a_panic() {
        let json = serde_json::json!({"unexpected": "shape"});
        assert_eq!(parse_settlement_receipt(&json), None);
    }
}

// ---------------------------------------------------------------------------
// Èṣù — elegbara_router settlement (Track B: the real cross-pillar gap)
// ---------------------------------------------------------------------------
//
// As of 2026-08-12, no function in this kernel ever calls OSOVM's
// `elegbara_router.move` — the live economic loop (Track A) settles
// entirely off-chain through Vantage's bookkeeping (see
// `tools/wallet_tools.rs`). This is the first real wiring of a settlement
// call from the kernel to OSOVM's Move layer. It does not make Track A
// itself go on-chain yet — it exists so the call path is real, tested, and
// ready, rather than a second layer of doc-only integration.
//
// `route_transaction_tax<T>` in the active (`sources/`, not `deferred/`)
// elegbara_router is a `public fun`, not `entry fun` — it takes a
// `Coin<T>` by value and *returns* the net `Coin<T>` to the caller. A single
// `sui client call` does NOT auto-transfer that unconsumed owned return
// value back to the sender (confirmed live, 2026-08-22: it aborts with
// `UnusedValueWithoutDrop` — a `Coin<T>` never has `drop`). The real fix is
// a `sui client ptb` with an explicit transfer: resolve the PTB sender via
// `sui::tx_context::sender`, call the router, then `--transfer-objects` the
// returned net coin to that sender. Live-EXECUTED (not just dry-run)
// 2026-08-22 against the real elegbara_router testnet deployment (package
// 0xb3b6...8050af, router 0xe3de...41dc): tx digest
// AwYGMdgDspQg5MnoqBaBddTRjQrpgSTBmmAPLVmW3xV7, `"status": "success"`,
// real `EsuTaxCollected` event emitted (44312120 gross -> 1635117 tax
// (3.69%) -> 42677003 net) and the net coin genuinely landed back in the
// sender's wallet.
//
// Prerequisites this function does NOT create: the caller must already
// hold a `Coin<T>` object of the settlement's gross amount (ordinary Sui
// coin selection/merge-split, out of scope here), and the router's
// package + shared-object id must be configured.
//
//   OMOKODA_ELEGBARA_PACKAGE    - published elegbara_router package id
//                                 (required; unset = settlement skipped)
//   OMOKODA_ELEGBARA_ROUTER_ID  - shared ElegbaraRouter<T> object id
//                                 (required; unset = settlement skipped)

/// A parsed, verifiable record of one `route_transaction_tax` call: the
/// gross amount submitted, the 3.69% Èṣù tithe skimmed, the net amount
/// returned to the caller, and the transaction digest that anchors all
/// three numbers on-chain. This is the "receipt" the settlement gap
/// analysis referred to — previously nothing produced one at all.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SettlementReceipt {
    pub tx_digest: String,
    pub gross: u64,
    pub tax: u64,
    pub net: u64,
}

/// Submit a real settlement transaction to `elegbara_router::route_transaction_tax<T>`
/// and parse the resulting receipt back out of the transaction's emitted
/// `EsuTaxCollected` event. Fail-open like every other call in this module:
/// `None` on missing config, missing `sui` binary, insufficient gas, or any
/// network/parse failure — settlement anchoring is not allowed to block the
/// off-chain economic loop it is meant to eventually replace.
///
/// `coin_object_id` must be an object the calling wallet owns, of the exact
/// type named by `coin_type` (a full Move type tag, e.g.
/// `"0x2::sui::SUI"`), holding the full gross amount to be settled — the
/// router skims 3.69% and this PTB explicitly transfers the net remainder
/// back to the signer (see the PTB-vs-plain-call note above the type).
pub async fn settle_transaction_tax(coin_object_id: &str, coin_type: &str) -> Option<SettlementReceipt> {
    let package = std::env::var("OMOKODA_ELEGBARA_PACKAGE").ok()?;
    let router = std::env::var("OMOKODA_ELEGBARA_ROUTER_ID").ok()?;
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    let move_call_target = format!("{package}::elegbara_router::route_transaction_tax");
    let type_arg = format!("<{coin_type}>");
    let router_arg = format!("@{router}");
    let coin_arg = format!("@{coin_object_id}");

    let output = tokio::process::Command::new("sui")
        .args([
            "client",
            "ptb",
            "--move-call",
            "sui::tx_context::sender",
            "--assign",
            "sender",
            "--move-call",
            &move_call_target,
            &type_arg,
            &router_arg,
            &coin_arg,
            "--assign",
            "net",
            "--transfer-objects",
            "[net]",
            "sender",
            "--gas-budget",
            &gas_budget,
            "--json",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[onchain] route_transaction_tax settlement failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    parse_settlement_receipt(&json)
}

/// Pure parser: extract a `SettlementReceipt` from a `sui client call --json`
/// response by finding the `EsuTaxCollected` event and reading `digest` from
/// the top level. Split out from `settle_transaction_tax` so it's testable
/// against realistic response-shape fixtures without a live `sui` binary or
/// network call — same pattern as `extract_minted_agent_info_id` below.
fn parse_settlement_receipt(json: &serde_json::Value) -> Option<SettlementReceipt> {
    let digest = json.get("digest")?.as_str()?.to_string();
    let events = json.get("events")?.as_array()?;

    let parsed = events.iter().find_map(|e| {
        let event_type = e.get("type")?.as_str()?;
        if !event_type.ends_with("::elegbara_router::EsuTaxCollected") {
            return None;
        }
        e.get("parsedJson")
    })?;

    // Move u64 fields are serialized as JSON strings by the Sui RPC to
    // avoid f64 precision loss; accept either a string or a bare number
    // so this doesn't silently break if that changes upstream.
    let read_u64 = |field: &str| -> Option<u64> {
        let v = parsed.get(field)?;
        v.as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| v.as_u64())
    };

    Some(SettlementReceipt {
        tx_digest: digest,
        gross: read_u64("gross")?,
        tax: read_u64("tax")?,
        net: read_u64("net")?,
    })
}

// ── Phase 7.3 — soul::forge() integration ────────────────────────────────────


/// Call `omokoda::soul::forge()` at birth to create the immutable on-chain
/// soul record. Returns the minted SoulRecord object ID on success, `None`
/// on any failure (fail-open — birth never blocked by chain availability).
///
/// Required env vars:
///   OMOKODA_SOUL_PACKAGE   — deployed soul.move package id
///   OMOKODA_SUI_CLOCK      — Sui clock object id (0x6 on mainnet/testnet)
pub async fn forge_soul_onchain(
    agent_id:     &str,
    odu_index:    u8,
    dna_fingerprint: &[u8],  // 86 bytes
    hermetic_seed_hash: &[u8],
    mnemonic_checksum: &[u8],
    nostr_pubkey: &[u8],     // 32-byte BIP-340 pubkey
    bipon39_words: &str,     // UTF-8 mnemonic phrase
    walrus_blob_id: &str,    // optional Walrus blob pointer
) -> Option<String> {
    let package = std::env::var("OMOKODA_SOUL_PACKAGE").ok()?;
    if package.is_empty() { return None; }
    let clock = std::env::var("OMOKODA_SUI_CLOCK")
        .unwrap_or_else(|_| "0x6".to_string());
    let gas_budget = std::env::var("OMOKODA_SUI_GAS_BUDGET")
        .unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    let to_bytes_arg = |b: &[u8]| -> String {
        format!("[{}]", b.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))
    };
    let to_str_bytes_arg = |s: &str| -> String { to_bytes_arg(s.as_bytes()) };

    let agent_id_arg       = to_str_bytes_arg(agent_id);
    let dna_arg            = to_bytes_arg(dna_fingerprint);
    let hermetic_arg       = to_bytes_arg(hermetic_seed_hash);
    let checksum_arg       = to_bytes_arg(mnemonic_checksum);
    let npub_arg           = to_bytes_arg(nostr_pubkey);
    let words_arg          = to_str_bytes_arg(bipon39_words);
    let walrus_arg         = to_str_bytes_arg(walrus_blob_id);
    let odu_arg            = odu_index.to_string();

    let output = tokio::process::Command::new("sui")
        .args([
            "client", "call",
            "--package", &package,
            "--module",  "soul",
            "--function","forge",
            "--args",
            &agent_id_arg, &odu_arg, &dna_arg, &hermetic_arg, &checksum_arg,
            &npub_arg, &words_arg, &walrus_arg,
            &clock,
            "--gas-budget", &gas_budget,
            "--json",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[onchain] soul::forge failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    // Extract the SoulRecord object ID from the created objects list.
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("objectChanges")?.as_array()?.iter().find_map(|c| {
        if c.get("type")?.as_str()? == "created" {
            c.get("objectId")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    })
}

// ── Phase 17.2 — Freenet → L1 state commitment ───────────────────────────────

/// Anchor a Freenet agent-state commitment on-chain (Sui path, Phase 17.2).
///
/// Submits the BLAKE3 hex of the new `AgentPublicState` as a dynamic-field
/// update on the agent's `AgentInfo` NFT via `garden::update_state_commitment`.
/// Fails open — returns `None` when `OMOKODA_SUI_REGISTRY` is unset, `sui` is
/// absent, or the call fails.  State commitment anchoring never blocks the
/// agent runtime.
///
/// When Phase 15 (Ọ̀ṢỌ́ ABCI L1) is complete, callers should prefer
/// `integrations::state_commitment::submit_to_abci()` and fall back here
/// only during the Sui migration window.
pub async fn anchor_freenet_state_commitment(
    agent_nft_id: &str,
    state_hash_hex: &str,
    version: u64,
    committed_at: u64,
) -> Option<String> {
    std::env::var("OMOKODA_SUI_REGISTRY").ok()?;
    let gas_budget =
        std::env::var("OMOKODA_SUI_GAS_BUDGET").unwrap_or_else(|_| DEFAULT_GAS_BUDGET.to_string());

    let hash_bytes = hex::decode(state_hash_hex).ok()?;
    let hash_arg = format!(
        "[{}]",
        hash_bytes.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
    );
    let version_arg = version.to_string();
    let committed_at_arg = committed_at.to_string();

    let output = tokio::process::Command::new("sui")
        .args([
            "client", "call",
            "--package", GARDEN_PACKAGE_V2,
            "--module",  "garden",
            "--function","update_state_commitment",
            "--args",
            agent_nft_id, &hash_arg, &version_arg, &committed_at_arg,
            "--gas-budget", &gas_budget,
            "--json",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[onchain] update_state_commitment failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("digest")?.as_str().map(|s| s.to_string())
}
