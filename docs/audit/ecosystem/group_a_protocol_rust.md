# Group A: Protocol Rust Crates Audit

**Date:** 2026-09-22
**Auditor:** Claude-Sonnet-4-6
**Scope:** 5 connective-tissue Rust crates — VCP, DIP, ARP, ScarabSwarm, Witness

---

## VCP (`~/VCP/`)

### Structure
Two crates: `vcp-types` (pure wire types) and `vcp-broker` (HTTP + handshake engine).

```
vcp-types/src/   device.rs  handshake.rs  receipt.rs  session.rs  error.rs  lib.rs  (1 device submod: zima.rs)
vcp-broker/src/  crypto.rs  handshake_engine.rs  session_store.rs  registry.rs  storage.rs  lib.rs  bin/main.rs
```

Total source: 1,601 lines (excluding target/).

### Compilation
`cargo check` passes with **1 warning** — `unused import: GixVisibility` in the upstream `gix-core` crate (`gix-core/src/memory_authority.rs:8`). VCP itself is warning-free.

### Tests
**14 tests, all pass.**

```
handshake_engine::gix_tests::canonical_ids_agree_after_session     ok
handshake_engine::gix_tests::save_and_load_graph_roundtrip         ok
handshake_engine::gix_tests::load_graph_from_missing_file_is_noop  ok
handshake_engine::gix_tests::session_composite_gix1_returns_hex    ok
handshake_engine::gix_tests::session_composite_returns_none_for_missing_session  ok
handshake_engine::gix_tests::session_composite_gix1_inserts_store_edge  ok
handshake_engine::gix_tests::vcp_session_survives_restart_with_full_store  ok
session_store::gix_tests::complete_auto_stamps_gix1                ok
session_store::gix_tests::complete_preserves_existing_gix1         ok
storage::tests::json_backend_roundtrip_store                       ok
storage::tests::null_backend_is_always_ok                          ok
storage::tests::json_backend_snapshot_id_is_stable_across_reload  ok
storage::tests::stale_tmp_file_does_not_corrupt_load               ok
storage::tests::with_storage_restores_index_and_graph              ok
```

### Key Exported Types

| Type | Status | Location |
|------|--------|----------|
| `DeviceManifest` | IMPLEMENTED | `vcp-types/src/device.rs` |
| `ChallengeRequest` | IMPLEMENTED | `vcp-types/src/handshake.rs` |
| `AuthResponse` | IMPLEMENTED | `vcp-types/src/handshake.rs` |
| `CapabilityNegotiation` | IMPLEMENTED | `vcp-types/src/handshake.rs` |
| `CapabilityGrant` | IMPLEMENTED | `vcp-types/src/handshake.rs` |
| `VcpReceipt` | IMPLEMENTED | `vcp-types/src/receipt.rs` |
| `SessionMessage` | IMPLEMENTED | `vcp-types/src/session.rs` |
| `RevocationRecord` | IMPLEMENTED | `vcp-types/src/handshake.rs` |
| `HandshakeEngine` | IMPLEMENTED | `vcp-broker/src/handshake_engine.rs` |
| `SessionStore` | IMPLEMENTED | `vcp-broker/src/session_store.rs` |
| `DeviceRegistry` | IMPLEMENTED | `vcp-broker/src/registry.rs` |
| `JsonFileBackend` | IMPLEMENTED | `vcp-broker/src/storage.rs` |

### Ed25519 Crypto — VERIFIED

`vcp-broker/src/crypto.rs:12-46` — `verify_auth_signature()` uses `ed25519-dalek` v2.2.0. Full implementation:
- Builds auth message as `SHA-256(challenge_id:nonce)` at line 19.
- Decodes 32-byte public key from hex at lines 22-30.
- Decodes 64-byte signature from hex at lines 33-38.
- Calls `verifying_key.verify(&message, &signature)` at line 44-45.
- Returns `VcpError::SignatureInvalid` on failure (not a panic).

Status: **VERIFIED — real crypto, not a stub.**

### 7-Step Handshake — VERIFIED

`handshake_engine.rs` implements all 7 steps:
1. `register_device()` — line 94: validates non-empty device_id, registers in `DeviceRegistry`, stamps `GixNamespace::MeshDevice` envelope.
2. `issue_challenge()` — line 139: generates UUID nonce, stores `ChallengeRequest` via `sessions.insert_challenged()`.
3/4. `handle_auth()` — line 179: retrieves challenge from `SessionStore`, calls `verify_auth_signature()` if `public_key` is non-empty, returns `CapabilityNegotiation`.
5. `create_grant()` — line 217: produces `CapabilityGrant`, calls `sessions.activate()`.
6. SESSION — `SessionMessage` types exist; streaming loop is NOT in the Rust broker — delegated to HTTP clients via the running server.
7. `session_store::complete()` — line 51: stores `VcpReceipt`, auto-stamps `gix1_canonical_id`.

### SessionStore — VERIFIED

`session_store.rs:14-101` — `SessionState` enum has four variants: `Challenged`, `Active`, `Completed`, `Revoked`.

`get_challenge(session_id)` at line 28 reads the stored `ChallengeRequest` and returns its nonce. This nonce is then passed to `verify_auth_signature()` at `handshake_engine.rs:192-196`. The nonce IS correctly stored and retrieved.

### Empty public_key behavior

`handshake_engine.rs:186`: `if !manifest.public_key.is_empty()` — if `public_key` is empty string, signature verification is **skipped entirely**. This is documented as dev/test mode. In production this is a security hole — any device with empty `public_key` can authenticate without signing.

Status: **PARTIAL — dev bypass exists with no production gate.**

### HTTP Server
Port 7791. Routes fully wired in `bin/main.rs`: register, list, session, auth, grant, revoke, receipt. `JsonFileBackend` provides durable GIX state across restarts.

### Wiring to Other Repos
- `~/Omo-Koda2/omokoda-core/src/bridge/dip.rs` and `~/Vantage/backend/routers/vcp.py` exist.
- **No Cargo dependency on `vcp-types` or `vcp-broker` in Omo-Koda2.** The Omo-Koda2 bridge talks to VCP over HTTP only.
- Vantage has `/api/vcp/devices` router (`vcp.py`).

### Gaps
1. **BROKEN** — Empty `public_key` bypasses all crypto (`handshake_engine.rs:186`). No env-gate or config flag to enforce crypto in production.
2. **STUB** — `agent_signature` field on `CapabilityGrant` is always `String::new()` (`handshake_engine.rs:243`). Agent never actually signs the grant.
3. **STUB** — `device_signature` on `VcpReceipt` is always `String::new()`. No device signs its own receipt.
4. **MISSING** — Session step 6 (streaming `SessionMessage` exchange) has types but no broker-side stream handler. HTTP clients must manage the session loop themselves.
5. **MISSING** — No BLE or mDNS discovery logic. MANIFEST.toml claims `transports = ["http", "ble", "mdns", "nostr"]` but only HTTP is implemented.
6. **MISSING** — No integration test calling the full 7-step handshake end-to-end through the HTTP server (unit tests exercise the engine directly).

---

## DIP (`~/DIP/`)

### Structure
Two crates: `dip-types` (envelope + adapter types) and `dip-bridge` (router + 8 adapter modules).

```
dip-types/src/    envelope.rs  adapter.rs  identity.rs  error.rs  lib.rs
dip-bridge/src/   registry.rs  router.rs  lib.rs  bin/main.rs
dip-bridge/src/adapters/   nostr.rs  a2a.rs  mcp.rs  meshtastic.rs  libp2p.rs  freenet.rs  zima.rs  world_query.rs  mod.rs
```

Total adapter source: 892 lines. Total crate ~1,400 lines.

### Compilation
`cargo check` passes **zero warnings** in DIP crates. (One upstream `gix-core` warning, same as VCP.)

### Tests
**2 tests, all pass** (both in `dip-types`):
- `envelope::gix_tests::new_envelope_has_gix1_canonical_id`
- `envelope::gix_tests::two_envelopes_have_distinct_gix1_ids`

`dip-bridge` has **0 tests**. The router and registry have no unit tests.

### Key Exported Types

| Type | Status | Location |
|------|--------|----------|
| `DipEnvelope` | IMPLEMENTED | `dip-types/src/envelope.rs` |
| `DipMessage` (enum, 9 variants) | IMPLEMENTED | `dip-types/src/envelope.rs` |
| `DipMessageKind` | IMPLEMENTED | `dip-types/src/envelope.rs` |
| `AdapterRegistry` | IMPLEMENTED | `dip-bridge/src/registry.rs` |
| `AdapterRouter` | IMPLEMENTED | `dip-bridge/src/router.rs` |
| `NetworkBinding` | IMPLEMENTED | `dip-types/src/envelope.rs` |

### Adapters — Real vs Stub

| Adapter | File | Status | Notes |
|---------|------|--------|-------|
| Nostr | `adapters/nostr.rs:6-29` | PARTIAL | Constructs kind-4 event JSON. **Comment line 27: "Full WebSocket relay publish deferred to Phase 9"**. `let _ = (relay_url, event)` — result discarded. `receive()` works. |
| A2A | `adapters/a2a.rs:6-36` | IMPLEMENTED | Makes real `reqwest::Client::post()` to `{agent_url}/tasks/send`. Timeout 10s. `receive()` parses response. Real HTTP call. |
| MCP | `adapters/mcp.rs:6-38` | IMPLEMENTED | JSON-RPC `tools/call` format. Real `reqwest::Client::post()`. `receive()` parses params. Real HTTP call. |
| Meshtastic | `adapters/meshtastic.rs:8-17` | PARTIAL | HTTP path (`MESHTASTIC_HTTP_URL`) is real (`reqwest::post /api/v1/sendtext`). Serial path: `tracing::info!("queue msg")` — **deferred**. 240-byte truncation logic present. |
| Libp2p | `adapters/libp2p.rs:9-36` | PARTIAL | HTTP gateway path real (`reqwest::post /api/libp2p/send`). Direct libp2p: comment "Phase 2: Direct libp2p via rust-libp2p (requires separate binary)". |
| Freenet | `adapters/freenet.rs:9-37` | PARTIAL | HTTP gateway path real (`reqwest::put /v1/contract/{key}/state`). No direct Freenet node integration. |
| Zima | `adapters/zima.rs` | PARTIAL | Exists (203 lines), registered in main.rs only when `ZIMA_API_URL` set. Not in spec count of 6. |
| WorldQuery | `adapters/world_query.rs` | PARTIAL | 388 lines. Bonus adapter, not in original 6. |

### DipEnvelope Signature — BROKEN

`dip-types/src/envelope.rs:24` — field `signature: String` is documented "Ed25519 signature over canonical_hash()".

`DipEnvelope::new()` at line 56 sets `signature: String::new()`. The `canonical_hash()` function at line 61-70 only hashes three fields (`envelope_id`, `from`, `to`).

**Critical finding:** `sha256_hex()` at line 73-78 uses `std::collections::hash_map::DefaultHasher` — this is a **non-cryptographic hash** (SipHash-based, output format-faked to look like SHA-256). The function name is misleading. This is **not SHA-256**. Nobody computes or verifies signatures on DipEnvelopes. The `signature` field is always empty.

Status: **BROKEN — signature field always empty; `sha256_hex` is fake (DefaultHasher, not SHA-256).**

### AdapterRegistry — IMPLEMENTED
`registry.rs:5-38` — real `HashMap<String, AdapterManifest>` behind `RwLock`. `register()`, `get()`, `list()`, `find_by_kind()` all work.

### AdapterRouter — IMPLEMENTED
`router.rs:24-96` — `route()` parses `to` prefix, dispatches to built-in adapter modules. Falls back to registered external adapters. `parse_network_kind()` handles all 6 official prefixes plus `zima:`, `habitat:`, `ha:`.

### HTTP Server
Port 7792. Routes: `/health`, `/api/adapters` (GET+POST), `/api/dip/outbound` (POST), `/api/dip/inbound` (POST). Fully wired.

### Wiring to Other Repos
- `~/Omo-Koda2/omokoda-core/src/bridge/dip.rs` — custom `DipBridge` struct (NOT using `dip-types` as a Cargo dep). Talks to DIP over HTTP at `http://127.0.0.1:7792`. **Type mismatch: Omo-Koda2's `DipEnvelope` struct is a local re-definition (`sender_id`, `network`, `address`) — incompatible with `dip-types::DipEnvelope` (`envelope_id`, `kind`, `from`, `to`, `payload`).**
- `~/Vantage/backend/routers/` has `dip_inbound` and `dip_outbound` endpoints referenced in Vantage MCP tools.

### Gaps
1. **BROKEN** — `DipEnvelope.signature` always empty; `canonical_hash()` uses `DefaultHasher` not SHA-256 (`dip-types/src/envelope.rs:73-78`).
2. **BROKEN** — Nostr adapter: event is constructed then immediately discarded (`adapters/nostr.rs:27: let _ = (relay_url, event)`). No actual publish.
3. **CONFLICTING** — `Omo-Koda2/omokoda-core/src/bridge/dip.rs` defines its own `DipEnvelope` struct with different fields. Not using `dip-types` as a dependency. Two incompatible envelope schemas exist simultaneously.
4. **MISSING** — `dip-bridge` has 0 unit tests.
5. **PARTIAL** — Serial-path Meshtastic and direct libp2p are deferred.
6. **MISSING** — Signature verification on inbound envelopes (router.rs never checks `envelope.signature`).

---

## ARP (`~/ARP/`)

### Structure
Two crates: `arp-types` (receipt chain + bridge) and `arp-broker` (HTTP server + store).

```
arp-types/src/   receipt.rs  principal.rs  bridge.rs  zangbeto.rs  error.rs  lib.rs
arp-broker/src/  store.rs  lib.rs  bin/main.rs
```

### Compilation
`cargo check` passes **zero warnings**.

### Tests
**5 tests, all pass:**
- `arp-broker`: `store::gix_tests::submit_stamps_gix1_on_receipt`, `store::gix_tests::two_receipts_have_distinct_gix1_ids`
- `arp-types bridge`: `tests::ingest_and_root`, `tests::find_by_receipt_id`, `tests::kind_override`

### Key Exported Types

| Type | Status | Location |
|------|--------|----------|
| `ActionReceipt` | IMPLEMENTED | `arp-types/src/receipt.rs` |
| `ReceiptKind` (12 variants) | IMPLEMENTED | `arp-types/src/receipt.rs:88-101` |
| `ActionSpec` | IMPLEMENTED | `arp-types/src/receipt.rs:107-116` |
| `Principal` | IMPLEMENTED | `arp-types/src/principal.rs` |
| `PrincipalKind` (5 variants) | IMPLEMENTED | `arp-types/src/principal.rs:23-35` |
| `CapabilityRef` | IMPLEMENTED | `arp-types/src/principal.rs:38-46` |
| `EvidenceRef` | IMPLEMENTED | `arp-types/src/receipt.rs:121-128` |
| `WitnessAttestation` | IMPLEMENTED | `arp-types/src/receipt.rs:130-138` |
| `ThroneEvaluation` | IMPLEMENTED | `arp-types/src/receipt.rs:141-150` |
| `ConsensusReceipt` | IMPLEMENTED | `arp-types/src/receipt.rs:154-162` |
| `PhysicalAttestation` | IMPLEMENTED | `arp-types/src/receipt.rs:165-173` |
| `ArpBridge` | IMPLEMENTED | `arp-types/src/bridge.rs` |
| `ReceiptStore` | IMPLEMENTED | `arp-broker/src/store.rs` |

### 5-Primitive Chain — VERIFIED

`receipt.rs:62-82`:
- `ActionReceipt.hash()` computes real `SHA-256` (`sha2::Sha256`) over canonical JSON including `receipt_id`, `kind`, `principal_id`, `agent_id`, `action`, `timestamp`, `previous_hash`.
- `chain_valid(prev)` at line 79: `self.previous_hash.as_deref() == Some(&prev.hash())` — direct comparison. **This is a real cryptographic hash chain.**

`ReceiptStore.submit()` at `store.rs:39-84`: verifies chain on ingest, routes to `Genesis` / `Valid` / `Broken` / `PrevNotFound`. Stamps `gix1_canonical_id` on every receipt.

Status: **VERIFIED — real SHA-256 chain, not concatenation.**

### Python Compatibility — VERIFIED

`~/Vantage/backend/action_receipt.py` exists (273 lines). The Python `canonical_hash()` at line 68-79 hashes the **same 8 fields** as Rust `receipt.rs:64-71`: `receipt_id`, `kind`, `principal_id`, `agent_id`, `action_kind`, `action_target`, `timestamp`, `previous_hash` — using `json.dumps(..., sort_keys=True)` + `hashlib.sha256`. **Field names match the Rust struct serialization.** Python and Rust produce compatible canonical hashes.

`from_vcp_flight_receipt()`, `from_ucx_compute_receipt()`, `from_trade_order()`, `from_emission_receipt()`, `from_twin_receipt()` — 5 of the 6 converters are implemented. The 6th (native runtime receipts) is `from_runtime_receipt()`.

### ArpBridge in Omo-Koda2

`~/Omo-Koda2/omokoda-core/src/bridge/arp.rs` — this is NOT importing `arp-types` as a Cargo dependency. It re-implements receipt construction as `serde_json::Value` and POSTs to `/api/arp/receipts`. The field structure matches the ARP schema. **It does not use `ArpBridge` or `ActionReceipt` Rust types directly.**

`~/Vantage/backend/routers/arp_receipts.py` — ingest endpoint exists, stores to SQLite `arp_receipts` table. Chain verification noted as "deferred to Phase 5."

### HTTP Server (arp-broker)
Port 7795. Routes: `POST /api/receipts`, `GET /api/receipts`, `GET /api/receipts/:id`, `POST /api/receipts/:id/verify`, `GET /api/receipts/:id/gix`, `GET /api/receipts/audit`.

### Zàngbétò Wiring
`arp-types/src/zangbeto.rs` — `anchor_receipt()` and `verify_anchor()` are real `reqwest` HTTP calls to `{ZANGBETO_URL}/api/zangbeto/records`. Fail-open (returns `None` on error). **These are async functions; the HTTP server does not call them automatically on submit.** Caller must invoke explicitly.

### Gaps
1. **PARTIAL** — `ActionReceipt.signature` is always empty string. No Ed25519 signing of receipts by the principal. ARP MANIFEST says `security = "high"` but receipts are unsigned.
2. **MISSING** — Vantage `arp_receipts.py` defers chain verification ("deferred to Phase 5"). The Vantage ingest endpoint does NOT validate `previous_hash`.
3. **MISSING** — `arp-broker` does not auto-anchor to Zàngbétò on submit. `zangbeto.rs` functions exist but are not called from `store.rs`.
4. **CONFLICTING** — Omo-Koda2 sends `serde_json::Value` receipts to Vantage `/api/arp/receipts`, not to `arp-broker` on port 7795. Two receipt sinks exist with no synchronization.
5. **MISSING** — `ThroneEvaluation` type exists but no code populates it. Twelve Thrones integration is purely type-level.
6. **MISSING** — `ConsensusReceipt` type exists but no quorum logic.

---

## ScarabSwarm Rust (`~/ScarabSwarm/`)

### Structure
Two crates: `swarm-types` (trajectory/policy/receipt types) and `swarm-broker` (HTTP server + OSOVM delegation).

```
swarm-types/src/   trajectory.rs  policy.rs  sim_receipt.rs  error.rs  lib.rs
swarm-broker/src/  osovm_delegation.rs  vantage_store.rs  lib.rs  bin/main.rs
```

### Compilation
`cargo check` passes **zero warnings**.

### Tests
**3 tests, all pass** (all in `swarm-types::sim_receipt`):
- `gix_tests::stamp_gix1_sets_canonical_id`
- `gix_tests::stamp_gix1_is_idempotent`
- `gix_tests::two_receipts_have_distinct_gix1_ids`

No tests for `osovm_delegation`, `vantage_store`, or the HTTP handlers.

### Key Exported Types

| Type | Status | Location |
|------|--------|----------|
| `Trajectory` | IMPLEMENTED | `swarm-types/src/trajectory.rs` |
| `TrajectoryPoint` | IMPLEMENTED | `swarm-types/src/trajectory.rs` |
| `TrajectoryCandidate` | IMPLEMENTED | `swarm-types/src/trajectory.rs` |
| `Policy` | IMPLEMENTED | `swarm-types/src/policy.rs` |
| `PolicySelection` | IMPLEMENTED | `swarm-types/src/policy.rs` |
| `SimReceipt` | IMPLEMENTED | `swarm-types/src/sim_receipt.rs` |
| `ProofOfSimulation` | IMPLEMENTED | `swarm-types/src/sim_receipt.rs:76-81` |
| `SimOutcome` (4 variants) | IMPLEMENTED | `swarm-types/src/sim_receipt.rs:65-72` |

### ProofOfSimulation — PARTIAL (types real, trajectory computation not in this crate)

`sim_receipt.rs:76-81`:
```rust
pub struct ProofOfSimulation {
    pub proof_id:    String,
    pub sim_receipt: SimReceipt,
    pub vcp_receipt: Option<serde_json::Value>,
    pub arp_receipt: Option<serde_json::Value>,
}
```
The wrapper struct exists. `SimReceipt.merkle_root` and `proof_of_sim` fields are computed by real SHA-256 Merkle tree logic.

### Merkle Tree — VERIFIED REAL

`sim_receipt.rs:87-113` — `merkle_root()` function: sorts leaf hashes, pairwise SHA-256 reduction via `sha2::Sha256` (real crypto). `merkle_proof()` at line 117-150 generates inclusion proofs. **This is a real Merkle implementation, not a placeholder.**

`proof_of_sim` computation: `SHA-256(merkle_root || policy_hash || timestamp)` — real, at `osovm_delegation.rs:77-84`.

### Actual Trajectory Computation

The `Trajectory` and `TrajectoryPoint` types exist as data containers. **No trajectory physics, dynamics, or simulation engine lives in this crate.** The actual 100k-trajectory sweep is delegated to OSOVM via `osovm_delegation::run_via_osovm()` — an HTTP call to `{OSOVM_URL}/api/sim/run`. When OSOVM is unreachable, `synthetic_result()` generates fake hashes (`osovm_delegation.rs:88-100`). The synthetic result has `winner_score: 0.0`, `n_feasible: 0`.

`~/OSOVM/witness_bridge/src/lib.rs` references ScarabSwarm as "Scarabswarm (PoSim)" and confirms the actual trajectory computation lives in OSOVM, not here.

### HTTP Server
Port 7793. Routes: `/health`, `POST /api/sim/run`, `GET /api/sim/receipts`, `GET /api/sim/receipts/:id`. Full pipeline wired: runs sim, stamps GIX1, POSTs to Vantage `record_sim_receipt` and opens witness round — all in tokio::spawn (fail-open).

### SimReceipt GIX1 Stamping — VERIFIED
`sim_receipt.rs:41-53` — `stamp_gix1()` uses `Gix1::new(GixKind::Simulation, GixNamespace::OsovmExecution, ...)`. Idempotent. Called in `bin/main.rs` before storing.

### Wiring to Other Repos
- `~/Omo-Koda2/omokoda-core/src/agent_catalog/osovm.rs` references `SimReceipt`/`Witness` conceptually.
- `~/OSOVM/witness_bridge/src/lib.rs` explicitly names ScarabSwarm.
- No Cargo dependency on `swarm-types` in Omo-Koda2 (HTTP-only coupling).

### Gaps
1. **STUB** — No actual trajectory physics in this crate. All real simulation is in OSOVM. When OSOVM is down, the receipt is fabricated with 0 feasible trajectories.
2. **MISSING** — `SimReceipt.signature` is always `String::new()`. No signing of proof-of-simulation receipts.
3. **MISSING** — `ProofOfSimulation.vcp_receipt` and `arp_receipt` are always `None`. No wiring to VCP or ARP on completion.
4. **MISSING** — No tests for `osovm_delegation`, `vantage_store`, or HTTP handlers.
5. **MISSING** — Synthetic-result UUID generator (`osovm_delegation.rs:102-106`) uses `SystemTime::now().subsec_nanos()` — not cryptographically random. Synthetic hashes are deterministic/predictable.

---

## Witness Rust (`~/Witness/`)

### Structure
Two crates: `witness-types` (attestation + observation types) and `witness-broker` (HTTP server + Nostr publisher + Zàngbétò anchor).

```
witness-types/src/   attestation.rs  observation.rs  firmware.rs  error.rs  lib.rs
witness-broker/src/  nostr_publisher.rs  vantage_store.rs  zangbeto_anchor.rs  lib.rs  bin/main.rs
```

### Compilation
`cargo check` passes with **2 warnings** in `witness-broker` binary:
- `unused import: AttestationStatus` (`bin/main.rs`)
- `field 'kind' is never read` in `AttestationRequest` struct (`bin/main.rs:86`)

These are dead-code warnings, not errors.

### Tests
**5 tests, all pass** (in `witness-types`):
- `attestation::gix_tests::stamp_gix1_sets_canonical_id`
- `attestation::gix_tests::stamp_gix1_is_idempotent`
- `attestation::gix_tests::two_attestations_have_distinct_gix1_ids`
- `observation::gix_tests::stamp_gix1_sets_canonical_id`
- `observation::gix_tests::stamp_gix1_is_idempotent`

No tests for `nostr_publisher`, `vantage_store`, or `zangbeto_anchor`.

### Key Exported Types

| Type | Status | Location |
|------|--------|----------|
| `WitnessAttestation` | IMPLEMENTED | `witness-types/src/attestation.rs` |
| `AttestationKind` (4 variants) | IMPLEMENTED | `witness-types/src/attestation.rs:92-102` |
| `AttestationStatus` (4 variants) | IMPLEMENTED | `witness-types/src/attestation.rs:105-112` |
| `ObservationBundle` | IMPLEMENTED | `witness-types/src/observation.rs` |
| `SensorReading` | IMPLEMENTED | `witness-types/src/observation.rs` |
| `FirmwareManifest` | IMPLEMENTED | `witness-types/src/firmware.rs` |

### WitnessAttestation Nostr kind 31020 — PARTIAL

`attestation.rs:68-82` — `to_nostr_tags()` produces correct Nostr tag array: `[["d", attest_id], ["t", "witness"], ["session", ...], ["device", ...], ["observation_hash", ...], ["outcome", ...]]`. The `NOSTR_KIND_WITNESS = 31020` constant is defined in `witness-types`.

`nostr_publisher.rs:19-70` — `publish_attestation()` builds event JSON and calls `build_signed_event()`. The signed event JSON structure is correct (`id`, `pubkey`, `created_at`, `kind: 31020`, `tags`, `content`, `sig`).

**Critical finding:** `build_signed_event()` at line 79-91 hardcodes `"pubkey": ""` and `"sig": ""`. The `_nsec` parameter is ignored. **No actual Nostr signing occurs.** If `WITNESS_NOSTR_GATEWAY` is not set, line 65 returns `published = true` anyway (logs intent as success). If `WITNESS_NOSTR_GATEWAY` is set, it posts an unsigned event with empty pubkey.

`WitnessAttestation.canonical_hash()` at line 58-66 uses `DefaultHasher` (same bug as DIP) — **not SHA-256 despite the field name**. The actual `sha256_hex` in `nostr_publisher.rs:93-98` correctly uses `sha2::Sha256`, but this is only used for the event ID computation, not for the attestation hash stored on the struct.

Status: **PARTIAL — Nostr event JSON structure correct; signature/pubkey fields always empty; canonical_hash is DefaultHasher not SHA-256.**

### Nostr Publishing Pipeline
`publish_attestation()` reads `WITNESS_NOSTR_NSEC` (returns None if unset, aborting publish) and `WITNESS_NOSTR_RELAYS`. HTTP gateway path via `WITNESS_NOSTR_GATEWAY` env var. Direct WebSocket relay posting is not implemented ("would require a WebSocket client" — comment line 53).

### Zàngbétò Wiring
`zangbeto_anchor.rs` — real `reqwest` HTTP call. Same fail-open pattern as ARP.

### HTTP Server
Port 7794. Routes: `/health`, `POST /api/observations`, `GET /api/observations/:id`, `GET/POST /api/attestations`, `GET /api/attestations/:id`, `POST /api/attestations/:id/sign`. Full pipeline wired: observation → attestation → sign → nostr publish → zangbeto anchor.

### GIX1 Stamping — VERIFIED
`attestation.rs:44-55` — `stamp_gix1()` uses `Gix1::new(GixKind::Receipt, GixNamespace::MeshDevice, ...)`. Idempotent.

### Wiring to Other Repos
- No Cargo dependency on `witness-types` in Omo-Koda2 or Vantage.
- `~/OSOVM/witness_bridge/src/lib.rs` mentions "Witness-firmware (LoRa DePIN)" as a producer but uses a different signing path (direct Ed25519 for on-chain Sui calls).
- Vantage has `/api/witness/rounds` endpoint consumed by `vantage_store.rs`.

### Gaps
1. **BROKEN** — `build_signed_event()` ignores `nsec` parameter; outputs `"pubkey": ""` and `"sig": ""` (`nostr_publisher.rs:84,89`). Published Nostr events are invalid and unverifiable.
2. **BROKEN** — `WitnessAttestation.canonical_hash()` uses `DefaultHasher` not SHA-256 (`attestation.rs:84-89`).
3. **MISSING** — `WitnessAttestation.signature` is always `String::new()`. No Ed25519 signing by the Witness node.
4. **MISSING** — Direct WebSocket Nostr publish not implemented. Only HTTP gateway path (requires `WITNESS_NOSTR_GATEWAY`).
5. **MISSING** — No tests for publisher, store, or anchor modules.
6. **MISSING** — `FirmwareManifest` type exists but nothing verifies firmware authenticity.
7. **MISSING** — The `kind` field in `AttestationRequest` is never read (compiler warning), so attestation kind is always `PolicyExecution` regardless of caller intent.

---

## Cross-Cutting Integration Matrix

| From | To | Via | Status |
|------|----|-----|--------|
| Omo-Koda2 | ARP | HTTP POST `/api/arp/receipts` (Vantage, port 8000) | PARTIAL — sends `serde_json::Value`, not `ActionReceipt` type |
| Omo-Koda2 | ARP | HTTP POST `arp-broker` port 7795 | NOT WIRED — no call site found |
| Omo-Koda2 | DIP | HTTP POST `dip-bridge` port 7792 | PARTIAL — local `DipEnvelope` type, schema mismatch |
| Omo-Koda2 | VCP | No call site | NOT WIRED |
| Omo-Koda2 | ScarabSwarm | Reference in `osovm.rs` comments | SPEC_ONLY |
| Omo-Koda2 | Witness | No call site | NOT WIRED |
| VCP | Witness | `MANIFEST.toml` optional dep | SPEC_ONLY |
| VCP | ARP | `MANIFEST.toml` `arp = true` | SPEC_ONLY — VcpReceipt never wrapped in ActionReceipt |
| ScarabSwarm | OSOVM | HTTP POST `/api/sim/run` | IMPLEMENTED (fail-open) |
| ScarabSwarm | Vantage | HTTP POST `/api/witness/rounds` + `/api/ucx/sim_receipt` | IMPLEMENTED (fail-open) |
| Witness | Nostr | HTTP gateway POST `/api/events` | BROKEN — empty pubkey/sig |
| Witness | Zàngbétò | HTTP POST `/api/zangbeto/records` | IMPLEMENTED (fail-open) |
| ARP | Zàngbétò | HTTP POST `/api/zangbeto/records` | IMPLEMENTED but not auto-called |
| Vantage Python | ARP Rust types | No import — parallel re-implementation | CONFLICTING |
| DIP adapter | Nostr relay | `let _ = (relay_url, event)` — discarded | BROKEN |
| DIP adapter | A2A agent | `reqwest::post` real call | IMPLEMENTED |
| DIP adapter | MCP server | `reqwest::post` real call | IMPLEMENTED |

---

## Summary: Production-Readiness Assessment

| Crate | Compiles | Tests | Core Logic | Crypto | Wiring | Verdict |
|-------|----------|-------|------------|--------|--------|---------|
| VCP | CLEAN | 14/14 | Real (7-step handshake) | Real (ed25519-dalek) | HTTP-only, no Cargo dep | PARTIAL |
| DIP | CLEAN | 2/2 | Real (router+registry) | BROKEN (DefaultHasher, empty sig) | HTTP-only, schema conflict | PARTIAL |
| ARP | CLEAN | 5/5 | Real (SHA-256 chain) | Missing (receipts unsigned) | Two parallel sinks | PARTIAL |
| ScarabSwarm | CLEAN | 3/3 | Real (Merkle, delegation) | Missing (receipts unsigned) | HTTP to OSOVM/Vantage | PARTIAL |
| Witness | 2 warnings | 5/5 | Real (observation pipeline) | BROKEN (empty pubkey/sig) | HTTP to Vantage | PARTIAL |

**No crate is production-ready.** All five compile and their type systems are sound. The primary blockers are:

1. **Crypto gap across all 5 crates** — Signed types (`DipEnvelope.signature`, `VcpReceipt.device_signature`, `SimReceipt.signature`, `WitnessAttestation.signature`, `ActionReceipt.signature`) are all always empty strings. The infrastructure to sign and verify exists for VCP (Ed25519) and ARP (SHA-256 chain) but is not enforced end-to-end.

2. **DIP and Witness `canonical_hash` use `DefaultHasher`** — This is a critical semantic bug. Any audit, comparison, or verification using those hashes is silently wrong. Both `dip-types/src/envelope.rs:74` and `witness-types/src/attestation.rs:85` must be replaced with `sha2::Sha256`.

3. **No cross-repo Cargo dependencies** — All inter-repo communication is HTTP-only with locally re-defined types. Type safety across crate boundaries is not enforced. The Omo-Koda2 `DipEnvelope` (5 fields: `sender_id/network/address/payload/timestamp`) is incompatible with `dip-types::DipEnvelope` (9 fields: `envelope_id/kind/from/to/payload/trace_id/reply_to/ttl_secs/created_at/signature`).

4. **Empty `public_key` bypass in VCP** — `handshake_engine.rs:186` allows any device to authenticate without cryptographic proof. Must be gated behind an explicit `VCP_REQUIRE_CRYPTO=true` env var in production or removed entirely.
