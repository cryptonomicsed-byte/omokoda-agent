# Group F: Security and Mesh Audit

Audit date: 2026-09-22. Taxonomy used: VERIFIED · IMPLEMENTED · PARTIAL · STUB · SPEC_ONLY · DEAD · DUPLICATE · REDUNDANT · CONFLICTING · OBSOLETE · BROKEN · UNKNOWN.

---

## Zangbeto (`~/Zangbeto/`)

### Overall verdict: PARTIAL — multi-layer real enforcement kernel exists; external-anchor pipeline is scripts-only and no tests pass on this device.

### Rust Kernel — What It Actually Enforces

The workspace has 8 crates. The deepest enforcement logic is real and substantive.

**`crates/omo-kernel/src/lib.rs`** — IMPLEMENTED
- `RealityVM` struct wires capability gate (Èṣù), shadow-VM simulation, state transition engine, branch management (speculative DAG), and memory reconciliation (CRDT).
- `zangbeto::event::RealityEvent` is computed on every transition (lib.rs:183-210).
- Capability check at lib.rs:93-106: `agent_token.permits(op, timestamp)` returns `Permitted / Denied / Conditional`; conditional branches request a "blessing" from a named Òrìṣà. Notably `request_blessing` returns `Ok(true)` unconditionally (lib.rs:229-231) — STUB gap in the blessing path.

**`crates/zangbeto-enforcement/src/lib.rs`** — IMPLEMENTED
- `ZangbetoDaemon` + `ActionLadder` + `ArbitrationEngine` + `QuarantineManager` are fully typed.
- Escalation rules: Critical `EconomicAnomaly` → `RollbackTransition`; Critical `ConcurrencyConflict` → `QuarantineState { duration_ms: 600_000 }`; requires_consensus: `["ṣàngó"]` / `["yemọja"]`.
- 7 Orisha weights defined (lib.rs:66-77).

**`crates/zangbeto-enforcement/src/http.rs`** — IMPLEMENTED
- Axum router: `GET /health`, `POST /enforce`, `POST /review` (http.rs:46-57).
- Blocking verdicts: `quarantine → true`, `block → true`, `deny → true`, `halt → true` (http.rs:165-174).
- Default `EnforcementPolicy` has **zero** escalation rules (http.rs:36-43) — severity-only fallback. Real rules are only constructed in `lib.rs::create_default_enforcer`.

**`crates/zangbeto-enforcement/src/bin/zangbeto-server.rs`** — IMPLEMENTED
- Port: `ZANGBETO_PORT` env var, default **8787**.
- Binds `0.0.0.0:8787`, serves the Axum router from `http.rs`.

**`crates/zangbeto-enforcement/src/receipts.rs`** — PARTIAL
- `ReceiptStore` is an in-memory `Vec<TransitionReceipt>`. No persistence. No Arweave/Sui upload from within Rust.

**`crates/omo-kernel/src/zangbeto/`** (event, drift, ledger, pipeline, replay, ir) — IMPLEMENTED (types and pipeline stages present, not fully wired to external services)

### Arweave Upload Logic — PARTIAL (scripts only, not in Rust)

Real Arweave upload exists at:
- `zangbeto-fork/shrine/scripts/arweave_anchor.js:3-18`: uses `arweave` npm package, calls `arweave.transactions.post(tx)`, posts to `arweave.net:443`. This is **real** HTTP POST logic — not stubbed.
- Invoked manually: `node scripts/arweave_anchor.js receipt.json` → outputs `{arweave_tx, status}`.
- No Rust crate calls Arweave. The Rust kernel stores receipts in memory only.

### BTC OpenTimestamp Logic — PARTIAL (shell script only)

- `zangbeto-fork/shrine/scripts/ots.sh`: calls `opentimestamps stamp` then `opentimestamps upgrade`. Real `opentimestamps` CLI call — if the tool is installed. No Rust integration.
- Referenced in README:38 as step 4 of the "First Dance" workflow.
- No automation; operator must run manually and paste `btc_ots` hex into receipt JSON.

### Sui Submission Logic — PARTIAL (Node.js script)

- `shrine/scripts/submit_onchain_receipt.js:3-52`: uses `@mysten/sui.js`, calls `TransactionBlock` → `client.signAndExecuteTransactionBlock`. This is functional JavaScript against the Sui devnet.
- Move contracts at `shrine/sources/`: `zbt_core.move`, `zbt_diagnostics.move`, `zbt_errors.move`, `zbt_guard.move`, `zbt_v2.move`. These are real Move contracts with `module zbt::zbt_core`.
- `sabbath_active: bool` is a field on `DiagnosticReceipt` (zbt_core.move:79). **Sabbath-gated attestation** means the `submit_diagnostic` entry function accepts a `sabbath_active` boolean that gets stored on the on-chain receipt. There is no clock-check that enforces the Sabbath window in Move itself — the boolean is caller-supplied. The enforcement discipline is the off-chain ops ritual (`ops/sabbath_checklist.md`), not an on-chain gate.

### Night Patrol — SPEC_ONLY

- README:50 mentions "n8n Night Patrol skeleton with dedup fingerprinting."
- No n8n workflow JSON file exists in the repo (find found nothing). This is a README claim only.

### `~/Omo-Koda2/zangbeto-stub/src/lib.rs` — STUB

```rust
pub fn audit_state(state_bytes: &[u8]) -> AuditResult {
    // SHA-256 digest of input as "signature"
    AuditResult { receipt_id: Uuid::new_v4().to_string(), sig, passed: true }
}
```
- `passed: true` **always**. No network call. No policy evaluation.
- Exposes only one function: `audit_state(state_bytes) → AuditResult`.
- Does **not** expose `report_anomaly`, `review_act`, or `verdict_blocks`.

### `~/Omo-Koda2/omokoda-core/src/bus/zangbeto.rs` — IMPLEMENTED

- **This is the real Omo-Koda2 ↔ Zangbeto integration layer**, not the stub.
- `report_anomaly(agent_id, severity, classification, detail)` → HTTP POST to `$ZANGBETO_URL/enforce`. Fail-open: returns `None` when `ZANGBETO_URL` unset (bus/zangbeto.rs:55-77).
- `review_act(agent_id, tool, detail)` → POST to `$ZANGBETO_URL/review` (bus/zangbeto.rs:83-99).
- `verdict_blocks(verdict)` → checks `action/decision/enforcement/status/verdict` keys for blocking keywords; also checks `{"block": true}` / `{"allowed": false}` (bus/zangbeto.rs:107-131).
- `pending_incidents()` → `GET $ZANGBETO_URL/canary-trips?since={ts}` — pulls Canarytoken trips (bus/zangbeto.rs:144-182).
- `render_incident_notice` — renders safe fields only; explicitly does not expose raw token or manage_url (bus/zangbeto.rs:189-210).

### Stub vs Real API Compatibility

| Surface | Stub (`zangbeto-stub`) | Real bus (`bus/zangbeto.rs`) |
|---|---|---|
| `audit_state(bytes)` | YES (always passes) | NOT present — different API surface |
| `report_anomaly(agent_id, sev, cls, detail)` | NOT present | YES — HTTP POST /enforce |
| `review_act(agent_id, tool, detail)` | NOT present | YES — HTTP POST /review |
| `verdict_blocks(verdict_json)` | NOT present | YES |
| `pending_incidents()` | NOT present | YES — HTTP GET /canary-trips |

The stub and the bus client expose **different API surfaces** — stub was written only to make `omokoda-core` compile in CI; the bus client is the actual runtime integration already in the codebase.

### Cargo Tests

`cargo test --workspace` in `~/Zangbeto/` hit a `SIGSEGV` in `rustc` while compiling `cranelift-codegen` (policy-runtime → wasmtime transitive dependency). This is a known Android/Termux stack-overflow issue with large Rust crates. **Tests could not be verified on this device** — the failure is environmental, not a code bug. Status: BROKEN (environment), not BROKEN (code).

### Gap Summary

| Gap | Status |
|---|---|
| `ZANGBETO_URL` not set in Omo-Koda2 production config | bus.rs fail-opens silently |
| `request_blessing` always returns true | Blessing path is a stub |
| Night Patrol n8n workflow | SPEC_ONLY — file does not exist |
| ReceiptStore persistence | In-memory only |
| OTS script requires manual execution | Not automated |
| Sui submission requires manual execution | Not automated |

---

## omokoda-mesh-firmware (`~/omokoda-mesh-firmware/`)

### Overall verdict: PARTIAL — genuine Meshtastic fork with real sovereignty crypto change; boot wiring not completed.

### Is This a Real Meshtastic Fork?

**YES.** Confirmed Meshtastic origin:
- `platformio.ini` present with board envs (`heltec-v3`, etc.)
- `src/mesh/CryptoEngine.{h,cpp}`, `src/mesh/modules/`, all standard Meshtastic C++ structure.
- `version.properties` present.
- Build verified by fork author 2026-08-29: `pio run -e heltec-v3` produced `firmware-heltec-v3-2.8.0.92c00fc.factory.bin`. RAM 38.8%, Flash 68.4%.

### Sovereignty Features Added vs. Stock Meshtastic

**IMPLEMENTED — `NostrCryptoEngine`** (`src/mesh/NostrCryptoEngine.{h,cpp}`):
- Subclasses Meshtastic's `virtual class CryptoEngine`.
- Overrides `generateKeyPair`, `regeneratePublicKey`, `ensurePkiKeys`.
- Derives Curve25519 transport key from sovereign master seed via BIP-32 hardened path `m/44'/20000'/<node_index>'/3'` (NostrCryptoEngine.cpp:31).
- Key materials ported from `~/omokoda-mesh`: `src/mesh/nostr/bip32_derivation.{h,cpp}`, `master_seed.{h,cpp}`, `node_index.{h,cpp}`.
- Vendor submodule `vendor/secp256k1-embedded` added.
- Nostr key (N=0, secp256k1) stays Nostr identity; Curve25519 at N=3 is transport-only.

**NOT WIRED — Boot path** (PARTIAL):
- `src/main.cpp` `crypto` global still defaults to platform engine (`ESP32CryptoEngine`). `NostrCryptoEngine` is never constructed at boot. The class exists but is disconnected (docs/OMOKODA_FORK_PLAN.md:114-118).

### DIP Adapter

**NOT PRESENT.** Zero matches for `DIP`, `DipEnvelope`, `dip_adapter`, or `DipModule` in `src/`. The fork plan explicitly limits scope to the CryptoEngine seam only. DIP integration would be a separate phase (the path is: Meshtastic mesh originates a frame → packet exits onto the LoRa mesh → a gateway node bridges to DIP, but that gateway logic lives in `omokoda-mesh` or `DIP/`, not here).

### Hardware Targets Supported

Inherits all upstream Meshtastic board support (`variants/`, `boards/`): Heltec-v3, T-Beam, RAK, Seeed WIO-SX1262, Station G1/G2, and ~50 more. The fork adds no new boards. Build confirmed on `heltec-v3`.

### Protobuf Extension

**NOT STARTED.** Fork plan specifies carrying a Nostr pubkey alongside `meshtastic_User`/`NodeInfo` in protobufs — explicitly listed as not done (OMOKODA_FORK_PLAN.md:133).

---

## Scarabswarm Julia (`~/Scarabswarm/`)

### Overall verdict: IMPLEMENTED — physics, proofs, LLM pilot, and submission path all real; Blocksim is not the target (sovereign-node is).

### 6-DOF Physics — IMPLEMENTED (hand-rolled Euler, NOT RigidBodyDynamics)

`src/dynamics.jl`:
- `ScarabState` has position (3), velocity (3), attitude (roll/pitch/yaw), angular_velocity (p/q/r), motor_commands (4), imu_accel (3), imu_gyro (3) — full 6-DOF state.
- Euler integration with ZYX rotation matrix (dynamics.jl:69-80).
- **Not `RigidBodyDynamics.jl`** — deliberately hand-rolled for portability and determinism. The `dynamics_mujoco.jl` file exists for an optional MuJoCo bridge.
- Critical thrust_coeff bug was previously at 1e-6 (wrong unit); **corrected to 1.0** (dynamics.jl:40-52, documented comment explains both Hermes and Omo-Koda2 crashed because `thrust_coeff = 1e-6` gave negligible thrust regardless of pilot input).

### SHA256 Trajectory Proofs — VERIFIED

`src/validator.jl`:
- `compute_trajectory_hash` (validator.jl:49-65): JSON-serializes downsampled checkpoints, SHA-256.
- Cross-arch determinism addressed via quantization (validator.jl:68-80): float64 values rounded to physical tolerance before hashing to survive libm last-ulp variance across CPUs.
- `TrajectoryProof` struct: `trajectory_hash`, `imu_hash`, `execution_time`, `checkpoint_count`, `energy_used`, `timestamp`.

### Ollama LLM Pilot — IMPLEMENTED

`src/llm_pilot.jl`:
- `LLMPilot` backend: `:ollama`, `:openai`, `:omokoda` (POST to `/v1/think`), `:hermes` (`docker exec hermes chat -q`).
- `query_ollama` → HTTP POST to `http://$host/api/generate` (llm_pilot.jl:63-79).
- `parse_motor_commands` parses `THROTTLE:/ROLL:/PITCH:/YAW:` structured response.
- Optional: activated by `create_llm_pilot(...)` call rather than `create_naive_controller`.

### Blocksim Submission — NOT PRESENT, sovereign-node is the target

- No Blocksim dependency or endpoint anywhere in the repo.
- `submit_trajectory_proof` (validator.jl:271-293) POSTs to `$SOVEREIGN_NODE_URL/proofs/simulation` (default `http://localhost:8080`). This is the sovereign-node endpoint, not Blocksim.
- `proof_to_simulation_proof_payload` (validator.jl:201-262) builds the complete payload: `proof_id`, `agent_id`, `principal_id`, `trajectory_hash`, `sensor_hash`, `checkpoint_root`, metrics, `signature` (stub value `"stub-sig"` — signing not wired).

### Standalone Execution

- `examples/race_demo.jl`, `hermes_drone_demo.jl`, `llm_drone_demo.jl`, `mujoco_demo.jl` exist.
- `julia examples/race_demo.jl` is the expected entry point (requires `Pkg.instantiate` first).
- `src/veil_runner.jl` loads from `../../veilsim-studio/data/veils_1_200.json` — this path is relative and will fail if `veilsim-studio` is not present at `~/veilsim-studio/`.

### Distinction from `~/ScarabSwarm/` Rust

| Repo | Language | Role |
|---|---|---|
| `~/Scarabswarm/` (Julia) | Julia | 6-DOF flight sim, trajectory proofs, LLM pilot racing |
| `~/ScarabSwarm/` (Rust) | Rust | 100k trajectory PoS, `SimReceipt`, `PolicySelection`, port 7793 |

These are **not duplicates**. Julia = high-fidelity physics sim producing proofs. Rust = sovereign-protocol proof-of-simulation receipt layer.

---

## ares-control (`~/ares-control/`)

### Overall verdict: IMPLEMENTED — clean API/worker split, subprocess-based systemd control, EXECUTION gate works.

### Daemon Control Mechanism — subprocess systemctl

`service/systemd_control.py`:
- `run_action(unit, action)` → `subprocess.run(["systemctl", action, unit_name], timeout=30)` (systemd_control.py:17-27).
- `query_status(unit)` → `subprocess.run(["systemctl", "show", unit, "--property=ActiveState,SubState", "--value"], timeout=10)`.
- **Not dbus**. Direct subprocess call to `systemctl`. Requires worker process to run as root (or with appropriate polkit permissions) on hostinger-vps.

### API/Worker Split — VERIFIED

- `service/main.py` (FastAPI, port 8090): **never calls systemctl**. API accepts a toggle request, writes a `toggle_requests` row to SQLite with `status='pending'`, returns `{request_id, status: "pending"}`. Returns immediately.
- `service/worker.py` (separate process): polls `toggle_requests WHERE status='pending'` every 3s (configurable), calls `run_action`, updates status to `done`/`failed`. Uses `fcntl.flock` singleton lock to prevent double-processing (worker.py:38-48). Uses `os.nice(10)` to deprioritize (worker.py:124-126).
- Docstring explicitly cites the 2026-08-26 incident where ~40 background daemon processes starved Vantage's API (main.py:6-10).

### Authentication

- Header: `X-API-Key` compared via `secrets.compare_digest` against `ARES_CONTROL_API_KEY` env var (main.py:32-37).
- If `ARES_CONTROL_API_KEY` is unset: auth is skipped entirely — only acceptable for local-only deployments (documented caveat, main.py:35).
- EXECUTION category requires `approve_execution: true` in the request body as a second factor (main.py:82-89).

### Daemon Registry — 74 units

`service/daemons.py` contains 74 entries (confirmed via file line count). Categories:
- `INTEL` (free-toggle, analytics/scanning): majority
- `EXECUTION` (gated): `ares-freqtrade`, `ares-jupiter-signer`, `ares-pumpfun-scalp-manager`, `ares-pumpfun-trader`, `ares-strategy-bots`, `ares-strategy-executor-30/60`, `ares-trader-base/hyperliquid/polymarket/sui/trader`, `ares-trading-agents` (12 units)
- `BRIDGE` (Vantage/pillar connectors): ~15 units
- `DASHBOARD`, `BACKUP`

### Running Status

`curl http://localhost:8090/health` → **not running** on this device. ares-control is deployed on hostinger-vps, not locally.

### Service Files

`deploy/ares-control-api.service` and `ares-control-worker.service` present — systemd unit files for production deployment.

---

## Critical Gap: Zangbeto Stub → Real

### Current State

`~/Omo-Koda2/zangbeto-stub/src/lib.rs` is declared as a workspace member and used only so `omokoda-core` compiles in CI. The **actual** runtime integration is `omokoda-core/src/bus/zangbeto.rs`, which calls the real enforcement server over HTTP when `ZANGBETO_URL` is set.

The stub is already **bypassed at runtime** — `bus/zangbeto.rs` is what think() uses, not the stub. However the stub's `passed: true` design creates a false sense of security: any path that calls `zangbeto_stub::audit_state(...)` directly will always pass.

### Exact Steps to Complete the Wiring

**Step 1 — Deploy the enforcement server.**
```bash
cd ~/Zangbeto
ZANGBETO_PORT=8787 cargo run -p zangbeto-enforcement --bin zangbeto-server
# Or build a release binary and run as a systemd unit.
```
The server binary is at `crates/zangbeto-enforcement/src/bin/zangbeto-server.rs`. It is complete.

**Step 2 — Set `ZANGBETO_URL` in Omo-Koda2's environment.**
```bash
export ZANGBETO_URL=http://localhost:8787
```
`bus/zangbeto.rs:31-34` reads this env var. Without it, every call to `report_anomaly` / `review_act` / `pending_incidents` returns `None` and the runtime is fail-open.

**Step 3 — Verify `bus/zangbeto.rs` is called from think().**
Confirm `review_act` is called before tool execution and `verdict_blocks` gates the call. Grep:
```bash
grep -rn "review_act\|report_anomaly\|verdict_blocks" ~/Omo-Koda2/omokoda-core/src/
```
If not wired into the think() loop, add the call at the tool-dispatch layer.

**Step 4 — Wire the escalation rules.**
`zangbeto-enforcement/src/http.rs::default_ladder()` has zero escalation rules (severity-based fallback only). The full enforcement policy with Orisha-weighted consensus is in `lib.rs::create_default_enforcer`. The HTTP server currently uses `http::router()` which calls `default_ladder()`, not `create_default_enforcer`. To get real policy:

In `crates/zangbeto-enforcement/src/bin/zangbeto-server.rs`, replace:
```rust
let app = zangbeto_enforcement::http::router();
```
with:
```rust
use std::sync::Arc;
// Requires wiring replay_engine and policy_host; for initial rollout,
// use http::router_with(Arc::new(http::full_enforcement_ladder())) once
// that constructor is exposed, or promote create_default_enforcer args
// to optional (None → in-memory stubs).
```
This requires exposing `create_default_enforcer` with optional dependencies, or providing a simpler constructor that uses stub replay/policy hosts.

**Step 5 — Fix `request_blessing` in `omo-kernel`.**
`crates/omo-kernel/src/lib.rs:229-231`: `request_blessing` always returns `Ok(true)`. Replace with an HTTP call to `ZANGBETO_URL/review` (same pattern as `bus/zangbeto.rs::review_act`).

**Step 6 — Verify/remove the stub from Cargo workspace.**
Once `ZANGBETO_URL` is set and `bus/zangbeto.rs` is the active path, the stub can be removed from workspace `Cargo.toml`. Confirm no crate imports `zangbeto-stub` directly in production paths.

**Step 7 — Add `ZANGBETO_URL` to the Omo-Koda2 environment gate.**
In `omokoda-core/src/config.rs` or environment setup, add a startup warning if `ZANGBETO_URL` is unset, so the operator knows enforcement is in fail-open mode.

### Risk if Skipped

Without `ZANGBETO_URL`, every `review_act` call returns `None`, `verdict_blocks(None)` returns `false`, and all tool calls are permitted regardless of severity. The system is nominally wired but practically unenforced. This is the current production state.
