# Group B: Pillar Repos Audit

**Audit Date:** 2026-09-22
**Auditor:** Claude-Sonnet-4-6
**Scope:** OSOVM (`~/OSOVM/`), UCX (`~/UCX/`), organism-core (`~/organism-core/`)

---

## OSOVM (`~/OSOVM/`)

### Status: PARTIAL — Logic compiles, cannot execute on this machine

### Structure

| Category | Count | Notes |
|---|---|---|
| Julia source files (own, non-stdlib) | 53 | in `src/`, `integrations/`, `ffi/`, `test/`, root |
| Julia test files | 30 | `test/*.jl` — 4,202 lines total |
| Julia source lines (own code) | ~17,233 | `src/` tree only |
| JS files | 2 | `event-bridge.js`, `dashboard/app.js` |
| Docs/spec files | 50+ | Root-level `.md` files — heavy spec accumulation |
| Bundled Julia stdlib | `julia-1.10.5/` | Source tree only, no compiled binary |

**Entry point:** `src/server_main.jl` (boots the server) → `src/server.jl` (HTTP router, `OsoVMServer.start/1`). Port: `OSOVM_PORT` env var, default `7780`.

**Startup command:** `julia src/server.jl [port]` or `julia src/server_main.jl`

### Opcode Inventory

**Total defined in `src/opcodes.jl`:** 160 unique opcode-to-hex mappings (39 core + 121 expansion aliases, some aliases map to the same byte — the CORE_OPCODES dict has 39 entries, EXPANSION_OPCODES has 130, with 9 alias duplicates). The canonical number matches the memory (160).

**Actually implemented with handlers in `src/vm_core.jl`:** **17 opcodes** in the `OPCODE_HANDLERS` dict (lines 657–676):

```
0x00 HALT, 0x01 NOOP, 0x11 IMPACT, 0x1f RECEIPT,
0x20 STAKE, 0x21 UNSTAKE, 0x22 TRANSFER, 0x23 BALANCE,
0x27 TITHE, 0x28 NONREENTRANT, 0x2b GENESIS_FLAW_TOKEN,
0x3c AGENT_CONVERT, 0x3d JOB_PAYMENT, 0x3e AGENT_BIRTH,
0x3f GPU_CONTRIBUTION, 0x54 TOC_MINT, 0x55 TOC_DECAY
```

**Status of those 17:** IMPLEMENTED — each handler contains real state-mutation logic. No TODO stubs. IMPACT has daily cap enforcement. AGENT_BIRTH checks balance and calls AseSupply. TOC_MINT has a gate (`toc_is_fully_verified()`). TOC_DECAY applies 1%/day rate from constants.

**Remaining 143 opcodes (e.g. VEIL 0x12, REQUIRE 0x29, EMIT 0x2a, COMPUTE_PROOF 0x56, all Governance/GnosisEX/SimaaS/Orisa/Economic/Extended ops):** SPEC_ONLY. Defined in `opcodes.jl` with symbol→byte mappings. No handler in `OPCODE_HANDLERS`. The `apply_instruction` function returns `{ status: "unknown_opcode" }` for any unregistered opcode — it does not panic.

**VEIL (0x12):** Called in `server.jl:handle_veilsim_run` which invokes `OsoVM.execute_instruction` with the VEIL opcode. But VEIL has no entry in `OPCODE_HANDLERS` — it would return `unknown_opcode`. The veilsim route will always return a fallback 0.88 F1 score (line 247: `get(veil_result, "f1", get(..., 0.88))`). Status: BROKEN for real execution, silently produces a fake score.

**COMPUTE_PROOF (0x56):** Defined in `opcodes.jl` line 53. No handler. Referenced from `toc-evolve-hook.ts` as a real call. Returns `unknown_opcode`. Status: STUB — call will fail silently.

### HTTP API

Routes defined in `src/server.jl` (lines 404–437):
- `GET /health` — real
- `GET /opcodes` — real (returns full opcode map)
- `POST /run` — real (executes any opcode via OPCODE_HANDLERS)
- `POST /veilsim/run` — BROKEN (VEIL has no handler, produces fake 0.88 F1)
- `POST /ucx/preflight` — real logic (Synapse balance check, soft-lock)
- `POST /ucx/settle` — real logic (releases lock, records GPU contribution)
- `GET /ucx/meter/:session_id` — real

Note: `organism-core/bridge/rlm-osovm.ts` calls `POST /v1/vm` and `POST /v1/vm/:id/execute` — those routes DO NOT EXIST in `server.jl`. The bridge will always fall through to CLI fallback, then simulation. Status: CONFLICTING — bridge and server speak different API shapes.

### Event Bridge (`event-bridge.js`)

**Status: IMPLEMENTED but never called in practice.** The bridge handles `GPU_CONTRIBUTION`, `TOC_MINT`, and `TOC_DECAY` events by POSTing to Vantage's `/api/ucx/dopamine/mint` and `/api/ucx/synapse/mint` endpoints. Logic is correct. However, `server.jl` never calls `event-bridge.js` — the Julia server emits no events to the JS bridge. The bridge is a standalone module with no caller wiring it to OSOVM's HTTP server. Status: DEAD (written but not integrated).

### ARM64 / Julia Execution Status

**Julia binary installed at:** `/data/data/com.termux/files/usr/local/julia/bin/julia`

**Binary type:** `ELF 64-bit LSB executable, ARM aarch64` — correct architecture.

**Error on execution:** `error: "...julia" has unexpected e_type: 2`

**Diagnosis:** `e_type: 2` is `ET_EXEC` (position-dependent executable). Android's Termux kernel enforces W^X and some versions reject non-PIE (`ET_EXEC`) ARM64 ELFs. Julia 1.9+ switched to PIE (`ET_DYN`, e_type=3) for Linux/ARM64, but the binary present appears to be a non-PIE build (either Julia 1.8 or an older ARM64 port). The bundled `julia-1.10.5/` in the OSOVM repo is only Julia source — no compiled binary.

**Workaround options:**
1. Install Julia via Termux's package manager: `pkg install julia` (if available for the current Termux version)
2. Download the official Julia 1.10.5 aarch64 musl or glibc binary from julialang.org and verify it's PIE
3. Run via Docker using `Dockerfile.julia` (requires `FROM julia:1.9-bullseye` which is x86_64 — requires `--platform linux/amd64` on ARM64, or use `julia:1.9-bullseye-arm64v8`)
4. Run OSOVM on a separate VPS/server and have organism-core call it over HTTP (the HTTP path is already wired)

**Tests:** 30 test files exist, 4,202 lines. Cannot be executed locally due to Julia binary failure. No CI run output available.

### External Dependencies

| Env Var | Purpose | Required |
|---|---|---|
| `OSOVM_PORT` | HTTP server port | No (default 7780) |
| `VANTAGE_URL` | Dopamine/Synapse event notifications | No (fail-open) |
| Julia packages: HTTP, JSON3, SHA, Dates, UUIDs | `server.jl` imports | Must be in Julia env |

### Integration Surface in Omo-Koda2

- `omokoda-core/src/tools/osovm_tool.rs` — `OsovmRunTool`, `OsovmVeilSimTool`, `OsovmHealthTool`. Registered via env gate: `if std::env::var("OSOVM_URL").is_ok()` (tools/mod.rs:~line with OSOVM_URL check). Tools call `POST /run` and `POST /veilsim/run` — both routes exist.
- `omokoda-core/src/economics.rs` — references "osovm_proof / witnesses / zangbeto required" as a string error message. Indicates semantic dependency but no direct HTTP call.
- `omokoda-core/src/bus/sango.rs` — comment references OSOVM/elegbara_router on Sui. No code call.

**Integration status: PARTIAL.** The tool bridge is wired correctly. OSOVM must be running at `OSOVM_URL` for tools to activate. The server's 17 implemented opcodes will work; VEIL/COMPUTE_PROOF will return `unknown_opcode` errors.

---

## UCX (`~/UCX/`)

### Status: IMPLEMENTED — Compiles clean, real HTTP calls, no test coverage

### Structure

| Crate | Key Files | Lines |
|---|---|---|
| `ucx-protocol` | `job.rs`, `capability.rs`, `receipt.rs`, `provider.rs`, `error.rs` | 457 |
| `ucx-broker` | `broker.rs`, `capability_token.rs`, `mint_allowlist.rs`, `vantage_discovery.rs`, `bin/main.rs` | ~800+ |
| `ucx-provider` | local provider impl | (not enumerated separately) |
| `adapters/gpu-ai` | `inference.rs`, `fine_tune.rs`, `marketplace.rs` | 762 |
| `adapters/akash` | `deployment.rs` | 326 |
| `adapters/vast` | `instance.rs` | 364 |
| `adapters/runpod` | `pod.rs`, `serverless.rs` | (present) |
| **Total audited** | | ~2,518 lines |

### HTTP Server

**Entry point:** `crates/ucx-broker/src/bin/main.rs` — Axum server.
**Port:** `UCX_PORT` env var, default `7790`.

**Routes:**
- `POST /api/jobs` — submit compute job
- `GET /api/jobs/:id` — poll status
- `GET /api/jobs/:id/receipt` — retrieve receipt
- `DELETE /api/jobs/:id` — cancel
- `GET /health` — liveness
- `POST /compute/token` — CapabilityToken mint (gated on `UCX_GPUAI_MASTER_KEY`)
- GPU.ai marketplace routes: `/ucx/contribute`, `/ucx/earnings`, `/ucx/fund`, `/ucx/balance`

### Matching Engine (`crates/ucx-broker/src/broker.rs`)

**Status: IMPLEMENTED — real scored matching logic.**

The `submit()` method (line 37) does:
1. Iterate native providers, call `can_accept()`, score via `score_provider()`
2. Pick best-fit provider (highest score wins)
3. Fall through to external pool if `job.constraints.allow_external`
4. Return `InsufficientCapacity` if unmatched

`score_provider()` (lines 199–267) is a real weighted scoring function: 40 pts VRAM fit, 20 pts GPU count, 15 pts tier, 15 pts price, 10 pts trust. This is real logic, not a stub.

`notify_osovm_on_complete()` (line 92) is wired to call `mint_allowlist::notify_gpu_contribution()` after job completion.

### Adapter Reality Check

**gpu-ai/inference.rs** — IMPLEMENTED. Makes real `POST https://api.gpu.ai/v1/chat/completions`. Auth via `GPUAI_KEY` env var. `from_env()` returns `None` if key absent. `is_available()` makes a real GET to `/v1/models`. Translate receipt calculates token cost at $0.001/1K. `zangbeto_anchor: None` — anchor not set (gap: receipt won't pass UCX→OSOVM mint gate).

**gpu-ai/fine_tune.rs** — IMPLEMENTED. Makes real `POST https://api.gpu.ai/v1/fine_tuning/jobs`. Job ID stored, status polled via `GET /v1/fine_tuning/jobs/:id`. Real HTTP.

**gpu-ai/marketplace.rs** — IMPLEMENTED. Real calls to GPU.ai for listing GPUs, checking earnings, initiating deposits. Auth via `UCX_GPUAI_MASTER_KEY`.

**adapters/akash/deployment.rs** — IMPLEMENTED. Makes real `POST https://api.akashnet.net/...` calls. Constructs SDL (deployment spec) for jobs. Auth via `AKASH_KEY` + `AKASH_API_URL`. Real deployment lifecycle.

**adapters/vast/instance.rs** — IMPLEMENTED. Makes real calls to `https://console.vast.ai/api/v0`. Queries for cheapest offer matching requirements. Auth via `VAST_KEY`.

**Critical gap — zangbeto_anchor:** Both gpu-ai and vast adapters set `zangbeto_anchor: None` in their `ComputeReceipt`. This means `mint_allowlist::check_mint_eligible()` (which checks `zangbeto_anchor.is_none()` and returns `false`) will block all TOC_MINT eligibility for real provider jobs. The Zangbeto settlement step is not wired into any adapter. Status: BROKEN for the full GPU→TOC_MINT chain.

### Tests

**Result: 0 tests, 0 failures, 0 passes.**

`cargo test` output shows all 7 test targets ran `running 0 tests`. The workspace compiles with 2 minor dead-code warnings:
- `ucx-provider/src/local_provider.rs:66` — `job` field never read
- `adapters/vast/src/instance.rs:25` — `offer_id` field never read

No unit tests exist for the matching engine, score function, or any adapter. The broker logic is entirely untested by automated means.

### Integration Surface in Omo-Koda2

- `omokoda-core/src/tools/ucx_tool.rs` — `UcxRequestComputeTool`, `UcxOfferComputeTool`, plus VCP tools. Registered via: `if std::env::var("UCX_BROKER_URL").is_ok() || std::env::var("VANTAGE_URL").is_ok()`.
- `omokoda-core/src/tools/ucx_receipt.rs` — `UcxJobStatusTool`, `UcxJobReceiptTool`.
- `omokoda-core/src/tools/ucx_policy.rs` — `UcxCheckPolicyTool`.
- `omokoda-core/src/session.rs` — UCX provisioning mentioned in comments ("Set at birth via UCX provisioning").
- `omokoda-core/src/tools/mod.rs` — full tool registration block for UCX.

**Integration status: PARTIAL.** Tool stubs are wired and will activate when `UCX_BROKER_URL` is set. The broker must be running. Adapters require their respective API keys. The GPU→TOC_MINT chain is broken because `zangbeto_anchor` is never set.

### External Dependencies

| Env Var | Purpose | Required |
|---|---|---|
| `UCX_PORT` | Broker listen port | No (default 7790) |
| `GPUAI_KEY` | GPU.ai adapter auth | Required for gpu-ai adapter |
| `AKASH_KEY` | Akash adapter auth | Required for akash adapter |
| `VAST_KEY` | Vast.ai adapter auth | Required for vast adapter |
| `UCX_GPUAI_MASTER_KEY` | CapabilityToken HMAC signing | Required for `/compute/token` route |
| `VANTAGE_URL` + `VANTAGE_KEY` | Provider discovery from Vantage | Optional |
| `OSOVM_URL` | GPU contribution → TOC_MINT notification | Optional (fail-open) |
| `UCX_BROKER_URL` | Self-reference URL (for discovered providers) | Optional |

---

## organism-core (`~/organism-core/`)

### Status: PARTIAL — TypeScript wiring layer, runs in simulation mode only

### Structure

| Category | Count |
|---|---|
| Bridge TypeScript files | 14 (`bridge/*.ts`) |
| Test files | 3 (`test/`, `test-e2e/`) |
| Root orchestration | `full-breath.ts` |
| Compiled output | `dist/bridge/nostr-wire.{js,d.ts}` only |
| Node modules | Installed (`tsx`, `typescript`, `esbuild`, `@types/node`) |

### Bridge File Inventory

| File | Status | What it does |
|---|---|---|
| `birth-ifa-swibe.ts` | PARTIAL | Calls IfáScript Rust binary → JS fallback. Binary missing → always uses fallback. |
| `rlm-osovm.ts` | BROKEN | Calls OSOVM HTTP at wrong URL (`/v1/vm`, `/v1/vm/:id/execute` — routes don't exist in `server.jl`). Falls back to `julia cli.jl` subprocess (fails on this machine). Falls back to simulation. |
| `toc-evolve-hook.ts` | PARTIAL | Calls OSOVM `/run` (correct route). Falls back gracefully on network failure. AIO anchor unimplemented. |
| `zangbeto-audit.ts` | SPEC_ONLY | Witness quorum is a hash-based simulation (deterministic, not network calls). No Sui node calls. |
| `event-bus.ts` | UNKNOWN (not read) | Event routing between bridges |
| `nostr-wire.ts` | PARTIAL (compiled) | Nostr protocol wire — only file with compiled output |
| `twin-state.ts` | UNKNOWN | TwinStateVector bridge to Vantage (referenced in Vantage twin_state.py) |
| `twelve-thrones-consensus.ts` | UNKNOWN | Consensus layer |
| `spiral-time-bridge.ts` | UNKNOWN | Time bridge |
| Other 6 bridges | UNKNOWN | `agenttv-thrones-validator.ts`, `aio-jubilee-treasury.ts`, `ifa-veil-router.ts`, `nex-graph-bridge.ts`, `paradigm-omokoda.ts`, `swibe-techgnosis-bridge.ts` |

### ARM64 / Julia Blocker — Exact Analysis

The README (line 28) states: "Real Julia execution is blocked by system-level binary architecture mismatch (`unexpected e_type: 2`)."

This is confirmed. The `rlm-osovm.ts` bridge:
1. Tries HTTP to `OSOVM_HTTP_URL` (default `http://localhost:7778`) — but OSOVM server uses port 7780 by default, and the API routes differ (`/v1/vm` vs `/run`)
2. Falls back to `julia cli.jl` subprocess at `../../OSOVM/src/cli.jl` — fails because Julia binary has `e_type: 2` error
3. Falls back to simulation

**Test run confirms this** (from `npm test` output):
```
[Organism] ⚠️ OSOVM HTTP attempt 1 failed (TypeError: fetch failed)
[Organism] ⚠️ Julia VM execution failed (System Error). Falling back to Simulation Mode.
[Organism] SIMULATION Success: Task Hash sim-hash-7b2
✅ Audit: VERIFIED
[toc-evolve-hook] OSOVM unreachable for agent-...
✅ Exhale: Minted undefined ToC
```

The test passes ("The organism has breathed. Lifecycle complete.") but every step that touches a real service runs in simulation or fallback. No real opcodes are executed. `undefined ToC` confirms toc-evolve-hook returned nothing usable.

**Two distinct blockers in `rlm-osovm.ts`:**

1. **Julia ARM64 binary error** (`e_type: 2`) — blocks the CLI fallback path. Fix: install a PIE-compiled Julia binary.
2. **API route mismatch** — `rlm-osovm.ts` calls `/v1/vm` and `/v1/vm/:id/execute` but `server.jl` exposes `/run`. Even if Julia runs, the HTTP path fails. Fix: either update `rlm-osovm.ts` to use `/run`, or add the `/v1/vm` session-management routes to `server.jl`.

### Vantage Integration

`organism-core/bridge/twin-state.ts` is referenced in `Vantage/backend/routers/twin_state.py` comments. No direct import — Vantage's twin_state router documents the TS type as the canonical shape for the body it receives, indicating loose coupling via HTTP POST from organism-core to Vantage.

### Omo-Koda2 Integration

Zero references to `organism` or `organism_core` found in `~/Omo-Koda2/` Rust source. organism-core is not imported, not called as a library, and has no build dependency on Omo-Koda2. It is a standalone TypeScript orchestration layer that communicates with all three pillars over HTTP only.

### Tests

3 test files:
- `test/one-breath.test.ts` — runs the full lifecycle. Passes, all simulated.
- `test/nostr-wire.test.ts` — tests Nostr wire (unknown coverage)
- `test-e2e/e2e-test.ts`, `e2e-fail-test.ts` — E2E (not run in default `npm test`)

---

## Critical Path Analysis

### What needs to happen for OSOVM to work on ARM64?

**Problem 1 — Julia binary (`e_type: 2`):**
The installed Julia binary at `/data/data/com.termux/files/usr/local/julia/bin/julia` is a non-PIE ARM64 ELF (`ET_EXEC`). Android/Termux requires PIE. Fix options:
- `pkg install julia` via Termux package manager (installs a Termux-patched build)
- Download Julia 1.10.x `aarch64-linux-musl` tarball from julialang.org and verify `readelf -h julia | grep Type` shows `DYN` not `EXEC`
- Run OSOVM in Docker with `julia:1.10-bullseye-arm64v8` base image (confirmed PIE)

**Problem 2 — organism-core API route mismatch:**
`rlm-osovm.ts` calls `/v1/vm` (session management API) but `server.jl` exposes `/run` (stateless per-request API). Fix requires either:
- Add session management routes to `server.jl` (`POST /v1/vm`, `POST /v1/vm/:id/execute`)
- Update `rlm-osovm.ts` to use `/run` directly (simpler — the stateless model is consistent)
- Update `OSOVM_HTTP_URL` in organism-core to `http://localhost:7780` (fixes port) then fix routes

**Problem 3 — VEIL opcode not handled:**
`/veilsim/run` calls the VEIL opcode (0x12) which has no entry in `OPCODE_HANDLERS`. Silently returns fake score. Fix: implement `op_veil()` in `vm_core.jl` and add `0x12 => op_veil` to the handler map.

**Problem 4 — event-bridge.js never called:**
The JS event bridge that routes GPU_CONTRIBUTION → Vantage Dopamine mint is written but server.jl never calls it. Fix: after executing an opcode in `handle_run()`, check if result contains a routable event name and call the event bridge via Node subprocess or embed a lightweight HTTP POST in the Julia server.

### What needs to happen for UCX to be wired to Omo-Koda2?

**The wiring already exists at the tool registration level.** The path:
1. Set `UCX_BROKER_URL=http://localhost:7790` in Omo-Koda2's environment
2. Build UCX broker with adapters: `cargo build --features gpu-ai,akash,vast`
3. Run UCX broker: `./target/release/ucx-broker`
4. Set API keys: `GPUAI_KEY`, `AKASH_KEY`, `VAST_KEY` as needed
5. Agents with Tier ≥ 3 can call `ucx_request_compute` tool

**Remaining gaps to close for the full GPU→TOC_MINT chain:**
1. `zangbeto_anchor` must be set on `ComputeReceipt` — requires Witness/Zangbeto settlement to run before the receipt is issued. Currently `None` in all adapters. Add a post-completion Zangbeto confirmation step in each adapter's `receipt()` method.
2. The `/api/toc/allowlist/check` route called by `mint_allowlist.rs` does not exist in `server.jl`. Add it or change to call `/run` with `GPU_CONTRIBUTION` opcode directly.
3. The `/api/osovm/gpu_contribution` route called by `notify_gpu_contribution()` does not exist in `server.jl`. Map it to `POST /run` with `opcode=GPU_CONTRIBUTION`.

**Summary of missing routes in server.jl needed by UCX:**
- `POST /api/toc/allowlist/check` — not implemented
- `POST /api/osovm/gpu_contribution` — not implemented (UCX calls this after job completion)

The `/ucx/preflight` and `/ucx/settle` routes in OSOVM ARE correctly implemented and handle the Synapse budget side of the contract.

---

## Cross-Repo Status Summary

| Claim | Reality |
|---|---|
| "160 opcodes" | 160 defined in opcodes.jl; **17 have real handlers**; 143 are SPEC_ONLY |
| "OSOVM HTTP server" | IMPLEMENTED — port 7780, 7 routes; BROKEN for VEIL; missing 3 routes UCX needs |
| "UCX compiles clean" | TRUE — 2 dead-code warnings only |
| "UCX 3 adapters with real HTTP calls" | TRUE — gpu-ai, akash, vast all make real API calls |
| "UCX scored matching" | IMPLEMENTED — 5-factor scoring in broker.rs |
| "UCX wired to Omo-Koda2" | PARTIAL — tool bridge exists, env gate works; zangbeto_anchor gap breaks TOC_MINT chain |
| "organism-core ARM64 blocker" | CONFIRMED — Julia binary non-PIE; PLUS route mismatch is a second independent blocker |
| "organism-core runs real VM" | FALSE — all paths fall through to simulation in test |
| "event-bridge.js routes events" | DEAD — written, not integrated into server.jl call chain |
