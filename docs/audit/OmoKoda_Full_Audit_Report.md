# Ọ̀mọ Kọ́dà Sovereign Agent OS — Full Forensic Audit Report

**Date:** 2026-09-20  
**Auditor:** Claude Sonnet 4.6 (automated forensic pass)  
**Repository:** `/data/data/com.termux/files/home/Omo-Koda2`  
**Scope:** `omokoda-core` crate + satellite workspace crates + non-Rust components  
**Test baseline at audit:** 942 lib + 110 integration = 1,052 total (1 new failure discovered: `grep_tool_basic`)

---

## Taxonomy Labels Used

| Label | Meaning |
|---|---|
| **VERIFIED** | Pre-confirmed fact; not re-investigated |
| **IMPLEMENTED** | Real working logic; executes in a live path |
| **PARTIAL** | Core logic present but one or more integrations/branches missing |
| **STUB** | Compiles; API surface exists; real logic absent or always-returns |
| **SPEC_ONLY** | Referenced in docs/specs; no code exists in core path |
| **DEAD** | Code exists, compiles, but is never called from any live path |
| **DUPLICATE** | Functionally equivalent to another module; overlap confirmed |
| **REDUNDANT** | Present but not required given other existing mechanisms |
| **CONFLICTING** | Two things disagree in a materially observable way |
| **OBSOLETE** | Exists but targets an archived or deleted service |
| **BROKEN** | Fails at runtime under confirmed conditions |
| **UNKNOWN** | Insufficient evidence to classify |

---

## Section 1 — Rust Crate Inventory

### 1.1 omokoda-core (primary crate)

| Property | Value |
|---|---|
| Type | lib + (implicit bin via `main_loop.rs`) |
| Top-level `.rs` files | ~55 |
| Subdirectory modules | ~30 subdirs (tools/, memory/, identity/, lifecycle/, kernel/, genesis/, steward/, execution/, etc.) |
| Total `.rs` lines (top-level) | ~23,603 |
| Total `.rs` lines (subdirs) | ~47,817 |
| **Grand total** | ~71,420 lines of Rust |
| Lib test count | 942 pass |
| Integration test files | 28 files in `tests/` |
| Integration test count | 110 pass, **1 BROKEN** (`grep_tool_basic`) |

**Key exports from `lib.rs`:**
- `AgentConstitution`, `AgentCore`, `AgentSnapshot`, `ExecutionResult`, `Steward`
- `IntentClass`, `IntentCompilation`, `IntentCompiler`, `IntentPlan`, `SubAgentSuggestion`
- `Receipt`, `ReceiptStore`
- `PluginManifest`, `PluginRegistry`, `PluginState`
- `parse`, `Statement`
- `OduModule`, `OduRegistry`, `OduSource`
- `DispatchError`, `PrimitiveDispatcher`, `PrivacyEnforcer`
- `UserIdentity`, `AgentId`

**Cargo.toml Notable Dependencies:**
- `bipon39` — real crate, pinned to git rev `1c4d5c9` (not a stub)
- `ifascript` — real crate, pinned to git rev `31198960`
- `larql-glyph` — real crate, pinned to git rev `149322ef`
- `gix-core`, `gix-types` — path deps at `../../GIX/crates/`
- `zangbeto-enforcement` — path dep at `../zangbeto-stub` (the stub, NOT real enforcement)
- `nostr`, `nostr-sdk` 0.44 — real WebSocket + BIP-340
- `ed25519-dalek` 2.0, `k256`, `bip32` — real crypto
- `wasmtime`, `wasmtime-wasi` — optional behind `wasm` feature flag, **disabled by default on ARM64/Termux** due to known upstream cranelift stack-overflow bug

### 1.2 Satellite Crates in Workspace

| Crate | Location | Status | Notes |
|---|---|---|---|
| `omokoda-hermetic` | `../omokoda-hermetic` | IMPLEMENTED | Hermetic state, spiral calendar, resonance signatures |
| `omokoda-mesh` | `../omokoda-mesh` | IMPLEMENTED | C++ ESP32 firmware (Rust crate is a state struct library) |
| `nautilus_integration` | `../nautilus_integration` | PARTIAL | `sealed_memory::{seal, unseal}` used by tee.rs; hardware attestation path PARTIAL |
| `nist_entropy` | `../nist_entropy` | IMPLEMENTED | NIST SP 800-22 validate_entropy_seed; called at birth |
| `zangbeto-stub` | `../zangbeto-stub` | STUB | Always passes (`passed: true`); SHA-256 digest only; no real enforcement |
| `omokoda-acp` | `../omokoda-acp` | UNKNOWN | Not audited in this pass |
| `omokoda-cli` | `../omokoda-cli` | UNKNOWN | Not audited in this pass |
| `omokoda-clojure` | `../omokoda-clojure` | UNKNOWN | Not audited in this pass |
| `omokoda-frontend` | `../omokoda-frontend` | UNKNOWN | TypeScript frontend |
| `oso-move` | `../oso-move` | UNKNOWN | Move contracts for Sui |

---

## Section 2 — Non-Rust Component Inventory

### 2.1 TypeScript Frontend (`omokoda-frontend/`)
**UNKNOWN** — Not read in this audit. Presence confirmed at directory listing.

### 2.2 Archived Elixir / Go / Julia Services

Four systemd unit files exist in `systemd/`:
- `ares-omokoda-memory.service` — Targets Julia server (`server.jl --port 7778`) at `/opt/ares/Omo-Koda2/omokoda-memory`. **OBSOLETE** — Julia memory repo archived; no such directory on the working machine.
- `ares-omokoda-obatala.service` — **OBSOLETE** (targets archived Elixir/Go service)
- `ares-omokoda-oya.service` — **OBSOLETE**
- `ares-omokoda-swarm.service` — **OBSOLETE**

All 4 units target services that no longer exist. They would fail on `systemctl start`.

### 2.3 Move Contracts (`oso-move/`, `omokoda-on-chain/`, `omokoda-on-chain-skillforge/`)
**UNKNOWN** — present at workspace root; not audited in this pass. The `onchain.rs` in core shells out to `sui` CLI rather than linking Move SDK; this is the documented design.

### 2.4 Julia Bridge (`omokoda-julia/`)
**PARTIAL/OBSOLETE** — The `osovm_tool.rs` bridges to an OSOVM HTTP server at port 7780; OSOVM is Julia but runs as a separate server. No live Julia code is compiled into or linked from `omokoda-core`. The systemd units that launched Julia memory services are OBSOLETE.

---

## Section 3 — Spec Inventory (`specs/` + `SOVEREIGN_OS_SPEC.md`)

| Spec File | Classification |
|---|---|
| `specs/memory.md` | PARTIAL — 3-tier memory described; engine.rs implements it; soma/dag/reflection are built but never auto-wired to agent lifecycle |
| `specs/memory-fractal.md` | PARTIAL — Walrus tier 3 described; walrus.rs implements HTTP API integration fail-open; glyph projection implemented |
| `specs/privacy.md` | IMPLEMENTED — Private mode blocks non-local providers; Argon2id+ChaCha20 encryption in place |
| `specs/receipts.md` | IMPLEMENTED — Receipt chain wired end-to-end per Birth→Think→Act; tamper-evident SHA-256 chain |
| `specs/reputation.md` | IMPLEMENTED — `tier_for()`, `ReputationLedger`, reputation decay via `compute_reputation_decay()` |
| `specs/dream-rem.md` | PARTIAL — `dream.rs` exists (852 lines); REM consolidation logic present; not confirmed wired to automated Sabbath cycle |
| `specs/provider-routing.md` | IMPLEMENTED — ProviderRegistry, OpenAI-compatible BYOK, `personal_llm()` per-agent routing |
| `specs/soul-interface.md` | PARTIAL — Soul birth orchestrated in genesis/; not all soul outputs (e.g. GoalGenesis) consumed |
| `specs/language.md` | SPEC_ONLY — References If-Script/OSO-IR compiler integration; oso_ir.rs exists but integration depth UNKNOWN |
| `specs/stdlib.md` | SPEC_ONLY — Tool stdlib; individual tools implemented; canonical stdlib registry not confirmed |
| `specs/tool-manifest.md` | PARTIAL — ToolRegistry implemented; manifest format defined; dynamic hot-add via SkillDaemon is PARTIAL |
| `specs/busy-beaver.md` | PARTIAL — `justice/busy_beaver.rs` exists; wiring to decision loop not confirmed |
| `specs/frontend.md` | SPEC_ONLY — Frontend spec; TypeScript frontend exists but not audited |
| `specs/vantage-registration.md` | PARTIAL — Vantage client real HTTP (fail-open); birth registration attempted; some endpoints confirmed |
| `specs/veilsim-1to1-twin.md` | PARTIAL — `twin_binding_tool.rs` exists; OSOVM bridge is fallback mode |
| `specs/architecture.md` | SPEC_AHEAD — Describes some capabilities not yet connected (GoalGenesis, soma lifecycle integration) |

---

## Section 4 — Kernel Subsystem Classification (`src/kernel/`)

| Module | File | Classification | Notes |
|---|---|---|---|
| **CapabilityGrant** | `capability.rs` | IMPLEMENTED | `CapabilityKind`, `CapabilityPolicy`, tier-gated grant logic |
| **ComputeManager** | `compute/` | PARTIAL | Struct + methods defined; no real hardware GPU integration (struct fields only) |
| **DeviceManager** | `device.rs` | PARTIAL | `DeviceTree`, `VcpBinding`, `VcpClient` defined; VCP binding is env-gated with fail-open |
| **SovereignFS** | `fs.rs` | IMPLEMENTED | In-memory RwLock<HashMap> virtual filesystem; mounts defined; no disk persistence (by design) |
| **SovereignIPC** | `ipc.rs` | IMPLEMENTED | Message channels defined; in-memory message passing |
| **NetworkConfig** | `network.rs` | PARTIAL | `ProtocolRouter`, `TransportKind` defined; no actual network dispatch here (routed through DIP/nostr separately) |
| **AgentProcess** | `process.rs` | IMPLEMENTED | ProcessState machine, transitions, resource tracking |
| **ResourceScheduler** | `scheduler.rs` | IMPLEMENTED | Priority queue, BinaryHeap, JobSlot scheduling logic |
| **PolicyEnforcer** | `security.rs` | IMPLEMENTED | AgentTier, DenyReason, Enforcement modes; tier-based deny logic |

**Assessment:** The kernel layer is well-defined structurally. The types compile cleanly and have real logic. However, ComputeManager (GPU integration) and the live network routing layer (ProtocolRouter) are PARTIAL — they define the contract but delegate real dispatch to external services.

---

## Section 5 — Dispatch and Lifecycle Classification

### 5.1 Steward dispatch() — Birth→Think→Act State Machine

**IMPLEMENTED** (cite: `interpreter.rs:1901`)

The full dispatch chain:
```
dispatch() → dispatch_internal() → match Statement {
    Birth     → Steward::birth()       [interpreter.rs:1930]
    Think     → execute_compiled_think() or think_agentic()  [interpreter.rs:2233–2246]
    Act       → execute_tool_call_for_agentic()  [interpreter.rs:2807]
    SlashCmd  → /unlock, /seal, /status, etc.  [interpreter.rs:3218]
}
```

**Gates before LLM call (think path):**
1. Private mode check — blocks non-local provider (`interpreter.rs:5029–5048`) **IMPLEMENTED**
2. Budget check — `agent.synapse() < 100.0` blocks if insufficient (`interpreter.rs:5162–5167`) **IMPLEMENTED**
3. IfScript hermetic gate — pre-LLM tool-list gating via `ifscript_gate.rs` **VERIFIED**
4. Permission policy check via `list_available()` (tools filtered before tool list built) **IMPLEMENTED**

### 5.2 runtime.rs — AgentRuntime
**IMPLEMENTED** — `DaemonRegistry`, `AgentRuntime`, heartbeat chain state. Five daemons registered at startup: `heartbeat`, `presence`, `learning`, `job`, `skill`.

### 5.3 job_daemon.rs
**PARTIAL** — `spawn_job_daemon()` exists and has real polling logic against Vantage `/api/guilds/{slug}/tasks`. Delegates execution to `Statement::Act` via Steward. Confirmed gap: execution is real but recovery on crash / state tracking across restarts relies on Vantage state only, not local durable queue.

### 5.4 skill_daemon.rs
**PARTIAL** — `spawn_skill_daemon()` exists. On each tick it: marks daemon alive, builds a skill list from active daemons + builtins, reports to Vantage capabilities endpoint if configured. Does NOT dynamically load new Rust skills — Rust toolchain is not invoked at runtime. New skills require a binary rebuild.

### 5.5 sensor.rs
**IMPLEMENTED** — `SensorReading::read()` calls `termux-battery-status` on Android and falls back to safe defaults on non-Termux. Real hardware integration via shell-out, fail-open. Tests pass.

### 5.6 heartbeat.rs
**IMPLEMENTED** — Tamper-evident heartbeat chain: SHA-256 of canonical JSON with `previous_heartbeat_hash`. `SomaVector` emotional snapshot attached. Ed25519 signature field present but marked `// ed25519, future` — signing is not yet wired.

---

## Section 6 — HTTP Server Route Inventory (`src/server.rs`)

| Route | Method | Handler | Classification |
|---|---|---|---|
| `/v1/birth` | POST | `birth_handler` | IMPLEMENTED |
| `/v1/reveal-seed` | POST | `reveal_seed_handler` | IMPLEMENTED (one-shot latch) |
| `/v1/keystore` | GET | `keystore_handler` | IMPLEMENTED |
| `/v1/resume` | POST | `resume_handler` | IMPLEMENTED |
| `/v1/think` | POST | `think_handler` | IMPLEMENTED |
| `/v1/cognition` | POST | `cognition_handler` | IMPLEMENTED (agentic loop) |
| `/v1/vault/seal-secret` | POST | `seal_secret_handler` | PARTIAL (TEE seal optional) |
| `/v1/act` | POST | `act_handler` | IMPLEMENTED |
| `/v1/events` | GET | `events_handler` | IMPLEMENTED (SSE stream) |
| `/v1/status` | GET | `status_handler` | IMPLEMENTED |
| `/v1/health` | GET | `health_handler` | IMPLEMENTED |
| `/v1/manifest` | GET | `manifest_handler` | IMPLEMENTED |
| `/v1/capability` | GET | `capability_handler` | IMPLEMENTED |
| `/v1/vault` | GET | `get_vault_status` | IMPLEMENTED |
| `/v1/vault/config` | GET/PUT | config handlers | IMPLEMENTED |
| `/v1/vault/sync` | POST | `post_vault_sync` | PARTIAL (Walrus optional) |
| `/v1/vault/galaxy` | GET | `get_galaxy_data` | IMPLEMENTED |
| `/v1/vault/glyph` | GET | `get_glyph_memory` | IMPLEMENTED |
| `/v1/vault/glyph/merge` | POST | `post_glyph_merge` | PARTIAL |
| `/v1/vault/glyph/anchor` | GET | `get_glyph_anchor` | PARTIAL |
| `/v1/vault/search` | GET | `search_vault` | IMPLEMENTED |
| `/v1/vault/enable` | POST | `post_vault_enable` | IMPLEMENTED |
| `/v1/vault/knowledge` | POST | `post_vault_knowledge` | IMPLEMENTED |
| `/v1/vault/access-log` | GET | `get_access_log` | IMPLEMENTED |
| `/v1/vault/download` | GET | `get_vault_download` | IMPLEMENTED |
| `/v1/vault/ls` | GET | `get_vault_ls` | IMPLEMENTED |
| `/v1/vault/file/*path` | GET | `get_vault_file` | IMPLEMENTED |
| `/v1/rhythm/today` | GET | `rhythm_today_handler` | IMPLEMENTED |

**Auth model:** `X-Agent-Id` header selects guest agent; `X-Agent-Key` required for guest access. Owner path unauthenticated (existing behavior). Guest multi-agent support added to fix silent overwrite bug.

**Heartbeat:** `spawn_heartbeat()` wired, HEARTBEAT_SECS env var (default 300s). Sabbath check integrated. Copilot-cooldown (60s) prevents heartbeat competing with active user sessions.

---

## Section 7 — Goals and Agency

### 7.1 GoalGenesisEngine (`src/goal_genesis.rs`)

**Classification: IMPLEMENTED but DEAD**

The engine is fully implemented with real logic:
- 5 goal streams: Experience, Knowledge, REM, CalabasState, Constitutional
- `derive_goals()` builds a `GoalSet` (629 lines)
- `evolve()` decays urgency over time, merges fresh goals
- LARQL_ENABLED env gate for knowledge-gap goals (Gap #42)
- 16+ unit tests on the engine

**However:** Confirmed via grep search — `GoalGenesisEngine` is referenced ONLY in `src/lib.rs` (as a public module declaration) and within `goal_genesis.rs` itself. Zero calls in `interpreter.rs`, zero calls in `think_agentic()`, zero calls in `dispatch_internal()`. **The agent never generates autonomous goals at runtime.** It responds to prompts; it does not self-direct.

cite: `lib.rs:3` — `pub mod goal_genesis;` (module declared, not consumed)

### 7.2 IntentCompiler::compile()

**IMPLEMENTED** — `intent.rs:115`. Inputs actually used:
- `prompt` — classified into `SimpleQuery` / `ComplexTask` / `Creative` / `Monitoring`
- `modifiers.max_iterations` — clamped by tier-based cap
- `context.tier` — governs iteration cap
- `context.reputation` — gates priority = "high"
- `context.odu_seed` — derives neural_params (HKDF-derived routing fingerprint)
- `context.hermetic` — provides hermetic_depth
- `context.available_tools` — filters tool sequence

**Goal context is NOT injected** — `GoalSet` / `GoalGenesisEngine` are not consulted. The prompt determines the plan; no autonomous goals modulate it.

### 7.3 think_agentic() — Prompt Construction

**IMPLEMENTED** — `interpreter.rs:5015`

Prompt construction sequence:
1. Safety check: private mode locks to local provider
2. Tool list built via `list_available()` — tier + permission filtered
3. IfScript gate fires pre-tool-list (VERIFIED pre-confirmed)
4. System prompt includes: agent name, orisha tone (via `orisha_mood_words()`), tier level, tool list
5. Session history included (public messages only for public thinks)
6. BYOK personal LLM routing: `agent.personal_llm()` → OpenAI-compatible if configured
7. Agentic loop: up to 25 turns, budget-checked each turn

**Goal context: ABSENT** — System prompt does not reference any derived GoalSet.

### 7.4 ifscript_gate.rs — 16 Action Vessels

**VERIFIED** — 16 vessels confirmed, all match arms present in `vessel_action_alignment()`:
Genesis, Void, Attention, Loop, Receipt, Mask, Residue, Execution, Swarm, Restraint, Migration, Consent, Vision, Growth, Seal, Rhythm.

**Tier override logic:** `VesselAlignment::Blocked` can be overridden by `Tier 6+`. Since max tier is 5, this override is structurally unreachable in practice (no agent can reach Tier 6). This is either an intentional future gate or a dead code path.

### 7.5 constitution.rs — AgentConstitution

**IMPLEMENTED but PARTIAL wiring**

`AgentConstitution` struct is fully implemented with:
- `bipon39_phrase`, `odu_index`, `hermetic_dna[7]`, `koodu_birth_score`
- `gate_alignment_seed` derived from hermetic DNA variance
- `behavioral_archetype` derived from dominant gate + odu index
- `signing_bytes()` and `content_hash()` for cryptographic commitment

**Wiring:** Exported from `lib.rs:79` as `pub use constitution::AgentConstitution`. Referenced in `AgentSnapshot` indirectly but not stored as a field in `AgentSnapshot` — the constitution is constructable from snapshot data but is not persisted as a document in the vault by default. The `genesis/receipt.rs` (AgentGenesisReceipt) is where birth provenance is stored; constitution signing is not auto-triggered at birth.

---

## Section 8 — Memory Architecture

### 8.1 Memory Module Inventory (`src/memory/`)

| Module | Classification | Notes |
|---|---|---|
| `dag.rs` | IMPLEMENTED but PARTIALLY WIRED | CausalMemoryDag built; `causal_dag` field exists in AgentSnapshot; `last_causal_node` tracks chain. Not auto-populated by think/act cycle (comment in AgentSnapshot: "Was previously built but never referenced by any live agent") |
| `engine.rs` | IMPLEMENTED | 3-tier MemoryEngine (Working/Episodic/Semantic); `process_working_memory()` called in `add_memory()` |
| `gix_bridge.rs` | IMPLEMENTED | 877-line GlyphIndex bridge; used by `/v1/vault/glyph` routes |
| `glyph_memory.rs` | IMPLEMENTED | OduDirectory → GlyphGraph projection; read-only metadata only; `glyph_memory()` on AgentCore |
| `larql_query.rs` | PARTIAL | LARQL query adapter; LARQL_ENABLED gates; `describe()`, `select()`, `walk()`, `infer()` wrapped |
| `memdir.rs` | IMPLEMENTED | OduDirectory, OduEntry, MemoryScanner; filesystem-backed; 790 lines |
| `odu_keys.rs` | IMPLEMENTED | Key rotation every 100 acts; HKDF derivation chain; `rotate_memory_key()` wired |
| `private_schema.rs` | IMPLEMENTED | PrivateMemoryEntry schema |
| `reflection.rs` | IMPLEMENTED but PARTIALLY WIRED | ReflectionLedger built; `reflection` field in AgentSnapshot; not auto-populated at think/act |
| `router.rs` | IMPLEMENTED | MemoryRouter for tier-based routing |
| `seal_bridge.rs` | PARTIAL | Real CLI-shelling to `seal-cli`; DEK fetch wired via env config; requires `SEAL_REQUEST_CMD` + `SEAL_FETCH_CMD` to be set |
| `soma.rs` | IMPLEMENTED but NOT AUTO-WIRED | MemCell, MemScene, LPM (Lifelong Personal Model) — full Droidclaw SOMA; not automatically updated on think/act |
| `tee.rs` | PARTIAL | Double-envelope encryption (ChaCha20 + TEE AES-GCM); real nautilus_integration usage; `from_env()` requires `OMOKODA_TEE_SEAL=1` |
| `tier2_vault.rs` | IMPLEMENTED | Private message + structured entry vault; lives in sealed blob |
| `walrus.rs` | PARTIAL | Real Walrus HTTP API integration; fail-open on no env config; blob store/read implemented |

### 8.2 Memory Duplication Analysis: dag.rs / engine.rs vs glyph_memory.rs

**Classification: LAYERED, not duplicated**

These three serve distinct purposes:
- `engine.rs` — in-memory tier management (Working/Episodic/Semantic), churn + distillation
- `dag.rs` — causal lineage (what led to this thought), DAG traversal shape
- `glyph_memory.rs` — ecosystem interop projection (content-addressed metadata to GlyphGraph for larql/zerolang/mnemopi consumption)

No functional duplication. However, all three are PARTIALLY WIRED — they are built and exported but not automatically populated by the core Birth→Think→Act cycle. They require explicit calls that are not yet inserted in the hot path.

### 8.3 memory_vault/ — Classification

**PARTIAL** — `handlers.rs` provides HTTP handlers; `types.rs` defines vault types; `vault.rs` has 752 lines of real vault logic (access log, galaxy data, Walrus sync). Routes are wired in server.rs. The vault is a genuine feature, not a stub. The Walrus and Seal integrations within it are PARTIAL (fail-open, env-configured).

### 8.4 soma.rs

**IMPLEMENTED but DEAD in practice** — Full emotional memory architecture from Droidclaw (MemCell, MemScene, LPM). Does not automatically receive `think`/`act` outputs. Would require explicit insertion calls in `dispatch_internal()` to become live.

### 8.5 tier2_vault.rs

**IMPLEMENTED** — Logically separate private vault (thoughts vs. credentials). Lives inside the sealed session blob. Used correctly.

### 8.6 tee.rs

**PARTIAL** — Real double-envelope encryption design. Requires env config. Three-tier fallback (Seal DEK → static env key → software-only) is correctly implemented and fail-open.

---

## Section 9 — Tools and Habitat

### 9.1 Tool Inventory (`src/tools/`)

| Tool Module | Key Tools | Tier Required | Real Logic? |
|---|---|---|---|
| `file_ops.rs` | read_file, write_file, ls, glob | 1+ | IMPLEMENTED — real fs ops |
| `repl.rs` | bash, python_repl | 3+ | IMPLEMENTED — real subprocess |
| `web.rs` | web_search, web_fetch | 2+ | IMPLEMENTED — real reqwest HTTP |
| `nostr_identity_tool.rs` | nostr_publish, nostr_read | 2+ | IMPLEMENTED — real WebSocket |
| `osovm_tool.rs` | osovm_run, osovm_veilsim, osovm_health | 3+ | PARTIAL — real HTTP to port 7780; fails if OSOVM not running |
| `skillforge.rs` | skillforge_* | 2+ | IMPLEMENTED (1372 lines) |
| `sovereign.rs` | sovereign_* | 4+ | PARTIAL — sovereign node interaction |
| `sovereign_node.rs` | node_* | 3+ | PARTIAL |
| `twin_binding_tool.rs` | twin_bind, twin_state | 3+ | PARTIAL — unsafe env_var set in tests |
| `walrus_tool.rs` | walrus_store, walrus_read | 2+ | PARTIAL — env-gated |
| `wallet_tools.rs` | wallet_* | 3+ | PARTIAL — Sui CLI shell-out |
| `ucx_tool.rs` | ucx_* | 3+ | PARTIAL — UCX endpoint required |
| `tor_tool.rs` | tor_* | 4+ | PARTIAL — Tor daemon required |
| `onchain_tools.rs` | mint_agent, register_agent | 4+ | PARTIAL — Sui CLI shell-out |
| `mail_tool.rs` | send_mail, check_mail | 2+ | PARTIAL — SMTP/IMAP env config |
| `zero_tool.rs` | zero_query | 3+ | PARTIAL — zerolang server required |
| `python_bridge.rs` | python_exec | 2+ | PARTIAL — real python subprocess |
| `mesh_tools.rs` | mesh_* | 2+ | PARTIAL — mesh node required |
| `provider_tools.rs` | provider_* | 2+ | IMPLEMENTED |
| `omohome_tool.rs` | omohome_* | 2+ | PARTIAL |

### 9.2 HabitatAddress (`src/habitat/types.rs`)

**IMPLEMENTED** — `HabitatAddress` struct fully defined with: agent_id, area_id (Area), gps, ip, mesh_node (Meshtastic), nostr_npub, confirmed_at. `Area` and `PhysicalResource` types defined. Uses GIX types for integration.

**Wiring:** PARTIAL — HabitatAddress is defined and usable but not auto-populated from live sensors. `SensorReading` (sensor.rs) is separate and not yet connected to populate `HabitatAddress`.

### 9.3 osovm_tool.rs — OSOVM Bridge

**PARTIAL / FALLBACK MODE (VERIFIED)**

`osovm_url()` reads `OSOVM_URL` env var, defaults to `http://localhost:7780`. When OSOVM_URL is not set (or server is not running), every tool call returns an error string: `"OSOVM unreachable at {url}: connection refused"`. This is not a panic — it's a graceful error message. The tools themselves are real HTTP POST wrappers. Three tools: `osovm_run`, `osovm_veilsim`, `osovm_health`.

The OSOVM bridge is **in fallback mode** because the Julia OSOVM server is a separate process (confirmed: systemd unit targets archived Julia repo path, architecture mismatch on ARM64 Termux).

---

## Section 10 — Economy and Security

### 10.1 economics.rs — DopaminePool / SynapseDecay

**Classification: LOCAL SIMULATION (not on-chain)**

Source comment at `economics.rs:12`: _"In production this would be a distributed counter. Here it's a local stub."_

`DopaminePool` is an in-process struct with `allocated: f64` and `capacity: f64`. No RPC calls. No blockchain reads or writes. The decay functions (`compute_synapse_decay()`, `compute_reputation_decay()`) are pure mathematical functions.

**What IS live:** Synapse balance is stored in `AgentSnapshot.synapse` and decremented on tool calls. The `burn_synapse()` method is called in the dispatch path. Decay calculation is used. The pool pressure math affects initial synapse issuance at birth. But none of this settles on-chain — it's local process state persisted to disk via the vault.

**Gap:** No on-chain ASE token integration. The UCX/Vantage dopamine endpoints exist separately but are not bridged to this local `DopaminePool` struct.

### 10.2 Identity Security (`src/identity/`)

| Module | Classification | Notes |
|---|---|---|
| `duress.rs` | IMPLEMENTED | Panic-phrase → blake3 hash → `DuressResponse` (Wipe/Decoy/SilentAlert). `check_duress()` called in dispatch path. |
| `machine_vault.rs` | IMPLEMENTED | `derive_agent_vault_key()` for auto-unseal at restart |
| `wallet.rs` | IMPLEMENTED | 1014 lines; multi-chain address derivation (Sui SIP-6, ETH, BTC, Solana, Cosmos, Aptos); EIP-2307 keystore v3 |
| `nip06.rs` | IMPLEMENTED | Nostr NIP-06 key derivation |
| `bipon39.rs` | IMPLEMENTED | Real bipon39 crate; entropy → mnemonic → indices → odu_index |
| `dna.rs` | IMPLEMENTED | DNA fingerprint generation |
| `odu.rs` | IMPLEMENTED | OduSeed, OduIdentity |
| `pet.rs` | IMPLEMENTED | PetIdentity derived from odu + hermetic + tier |
| `vault.rs` | IMPLEMENTED | SealVault; Argon2id+ChaCha20Poly1305 |
| `merkle.rs` | IMPLEMENTED | IdentityMerkleTree binding agent_id + birth_timestamp + odu_index + dna |
| `cloak.rs` | IMPLEMENTED | CloakSeed display-offset; birth passphrase derived |
| `hive.rs` | PARTIAL | Hive identity concept; depth UNKNOWN |
| `safety.rs` | PARTIAL | Safety checks; depth UNKNOWN |
| `poison_radar.rs` | PARTIAL | Threat detection; depth UNKNOWN |
| `x402.rs` | PARTIAL | HTTP 402 payment protocol; depth UNKNOWN |
| `oauth.rs` | PARTIAL | OAuth identity; depth UNKNOWN |

### 10.3 Security Scan — Unsafe Blocks

`unsafe {}` blocks found in 3 files:
1. `src/ip_layer.rs:194,201,230,244` — **TEST-ONLY** context. All 4 occurrences are inside `#[tokio::test]` functions using `std::env::set_var()`/`remove_var()` for isolated test env setup. Comment: "SAFETY: test-only, single-threaded test binary". LOW RISK.
2. `src/tools/twin_binding_tool.rs:141,148` — **REQUIRES REVIEW** — unsafe in a tool execute() path, not test-only. Likely env-var manipulation. MEDIUM RISK without deeper investigation.
3. `src/bridge/session.rs:86` — `unsafe { libc::kill(pid as i32, libc::SIGKILL) }` — process kill signal. MEDIUM RISK — valid OS primitive but should be reviewed for target validation.

### 10.4 Hardcoded Values

- `interpreter.rs:1093`: `let chain_id = "testnet"; // Default for now` — CONFLICTING with production intent. This is the chain_id used in memory key derivation (`OduKeys::derive_k0`). Any mainnet deployment would use different chain semantics unless this is changed.

### 10.5 unwrap() in Security-Critical Paths

Production server `server.rs` contains `unwrap()` calls but only in test functions (`#[cfg(test)]` blocks, lines 1667–1932). The production handler paths use `.ok()`, `?`, and `map_err()` patterns. No panicking unwraps confirmed in production request handlers.

---

## Section 11 — Tests (`src/` + `tests/`)

### 11.1 Test Run Results

**Pre-confirmed baseline:** 1,052 tests pass (942 lib + 110 integration).

**Audit-discovered failure:**
- `tool_registry_tests::grep_tool_basic` — **BROKEN**
  - Error: `"grep search failed: IO error for operation on /data/data/com.termux/files/home/Omo-Koda2/omokoda-core/notes: No such file or directory (os error 2)"`
  - Root cause: test hardcodes a directory path (`notes/`) that does not exist on this machine
  - Risk: LOW (test environment issue, not a production logic bug)
  - Fix: Create `notes/` directory or use a tempfile-backed directory in the test

### 11.2 Integration Test Classification

| Test File | Type | Notes |
|---|---|---|
| `interpreter_tests.rs` | INTEGRATION | Birth/think/act dispatch; 602 lines |
| `gix_integration.rs` | INTEGRATION | GlyphIndex integration; 599 lines |
| `session_tests.rs` | INTEGRATION | Session sealing/unsealing; 381 lines |
| `coordination_roundtrip.rs` | SYSTEM | Multi-agent coordination roundtrip; 253 lines |
| `tool_registry_tests.rs` | INTEGRATION | Tool tier gates, file ops, sandbox; 251 lines — 1 BROKEN |
| `steward_audit_tests.rs` | INTEGRATION | Steward audit trail; 237 lines |
| `e2e_tests.rs` | E2E | Birth→Think→Act end-to-end; 232 lines |
| `parser_tests.rs` | UNIT | Parser coverage; 207 lines |
| `natural_think_tests.rs` | INTEGRATION | Think modifiers + natural language; 198 lines |
| `privacy_tests.rs` | INTEGRATION | Private mode enforcement; 193 lines |
| `receipt_tests.rs` | INTEGRATION | Receipt chain tamper-evidence; 182 lines |
| `tier_gate_tests.rs` | INTEGRATION | Tier permission gating; 175 lines |
| `privacy_memory_tests.rs` | INTEGRATION | Memory scope encryption; 171 lines |
| `reputation_curve_tests.rs` | UNIT | Reputation decay math; 159 lines |
| `justice_tests.rs` | INTEGRATION | Justice engine + hermetic multiplier; 152 lines |
| `synapse_tests.rs` | UNIT | Synapse budget math; 145 lines |
| `skillforge_e2e_tests.rs` | E2E | SkillForge hot-add; 140 lines |
| `integration_tests.rs` | INTEGRATION | General integration; 134 lines |
| `provider_tests.rs` | INTEGRATION | Provider registry + routing; 131 lines |
| `persistence_tests.rs` | RECOVERY | `sovereign_memory_continuity_across_restart` passes (VERIFIED); 125 lines |
| `slash_command_tests.rs` | INTEGRATION | /slash command parsing; 102 lines |
| `sandbox_tests.rs` | UNIT | WASM sandbox isolation; 77 lines |
| `skillforge_hotadd_tests.rs` | INTEGRATION | Skill hot-add mechanics; 69 lines |
| `pet_tests.rs` | UNIT | PetIdentity derivation; 58 lines |
| `identity_tests.rs` | UNIT | Identity anchor + merkle; 47 lines |
| `rack_tests.rs` | UNIT | RACK memory pattern; 46 lines |
| `parser_composition_tests.rs` | UNIT | Parser composition; 32 lines |
| `parser_overblocking_tests.rs` | UNIT | Parser overblocking regression; 20 lines |

**Classification summary:** The test suite is strong on UNIT/INTEGRATION but has only 2 true E2E files. RECOVERY testing is minimal (1 test). No chaos/adversarial tests confirmed.

---

## Section 12 — Synthesis: DELETE / MERGE / KEEP / FINISH

### 12.1 DELETE List (Dead or Obsolete Code to Remove)

1. **`systemd/ares-omokoda-memory.service`** — OBSOLETE. Targets `/opt/ares/Omo-Koda2/omokoda-memory/server.jl`, a Julia service in an archived repo. Will fail on systemctl start.
2. **`systemd/ares-omokoda-obatala.service`**, **`ares-omokoda-oya.service`**, **`ares-omokoda-swarm.service`** — All OBSOLETE for same reason.
3. **Tier-6 override in `ifscript_gate.rs`** — `VesselAlignment::Blocked` override requires `Tier 6+` but max tier is 5. Structurally dead code. Remove or replace with `Tier 5` boundary.

### 12.2 MERGE List (Duplicates to Consolidate)

1. **`src/memory/soma.rs` emotional state** + **`lifecycle/heartbeat.rs` SomaVector** — `SomaVector` in heartbeat.rs (energy/tension/focus/gpu_util) duplicates emotional state concepts in soma.rs (tension/connection_depth). These should share a type or one should delegate to the other.
2. **`src/habitat/types.rs` HabitatAddress** vs **`lifecycle/sensor.rs` SensorReading** — sensor data is collected separately from the habitat address type that should aggregate it. Wire `SensorReading` into `HabitatAddress` population; remove the gap.

### 12.3 KEEP List (Genuine Strengths — Do Not Rewrite)

1. **Birth pipeline** (`Steward::birth()`, `interpreter.rs:1003+`) — NIST entropy validation with retry, bipon39 mnemonic, hermetic state derivation, multi-chain wallet, machine vault auto-unseal, duress setup. Thorough and well-thought-out.
2. **Private memory encryption** — Argon2id+ChaCha20Poly1305 + three-tier TEE fallback. Clean fail-open design. `zeroize` applied correctly.
3. **16-vessel ifscript dispatch table** (`ifscript_gate.rs`) — Complete, principled, well-commented. Every cell is a deliberate decision.
4. **IntentCompiler** (`intent.rs`) — Clean compilation of prompt → plan with tier caps, hermetic depth, neural params from odu_seed. Real logic, no trivial pass-through.
5. **Receipt chain** — SHA-256 tamper-evident chain + per-receipt signing. wired end-to-end. Zàngbétò stub present but the receipt chain itself does not depend on the stub being real.
6. **Memory key rotation** (every 100 acts, `rotate_memory_key()`) — correct HKDF derivation, zeroize on rotation. Production-quality.
7. **think_agentic() agentic loop** — up to 25 turns, BYOK routing, tool-use loop, budget-gated, per-turn timeout. Genuine agentic capability.
8. **OSOVM_CODEX §42 regression lock** — test locks that `orisha_universal_term()` never returns raw Yoruba names. Shows care for observable invariants.
9. **`persistence_tests.rs` recovery test** — `sovereign_memory_continuity_across_restart` confirmed passing. Memory survives process restart.
10. **GlyphIndex projection** (`memory/glyph_memory.rs`) — content-addressed metadata projection, byte-compatible with ecosystem; read-only, additive.

### 12.4 FINISH List (Gaps with Concrete Next Steps)

1. **GoalGenesisEngine → Wire into think_agentic()**
   - File: `interpreter.rs:think_agentic()` (around line 5101)
   - Action: Construct `GoalGenesisInput` from live agent state; call `engine.derive_goals()`; inject top-N goals into system prompt
   - Effort: Medium (~2 days); engine is complete, wiring is the gap

2. **CausalMemoryDag → Populate at think/act boundaries**
   - File: `interpreter.rs:dispatch_internal()` after think and act
   - Action: Call `agent.snapshot.causal_dag.insert()` with a node linking to `last_causal_node`
   - Effort: Small (~4 hours)

3. **SOMA / ReflectionLedger → Auto-populate**
   - File: `interpreter.rs:dispatch_internal()`, act path
   - Action: Call `memory::soma` and `memory::reflection` updaters at think/act
   - Effort: Small (~1 day)

4. **Zàngbétò stub → Real enforcement**
   - File: `zangbeto-stub/src/lib.rs`
   - Action: Replace `passed: true` always-pass with real `zangbeto-enforcement` crate from the Zàngbétò repo
   - Effort: Large (requires CI integration with real Zàngbétò Rust crate)

5. **chain_id "testnet" hardcode → configurable**
   - File: `interpreter.rs:1093`
   - Action: Read `OMOKODA_CHAIN_ID` env var; default to "testnet"
   - Effort: Tiny (30 minutes)

6. **Heartbeat Ed25519 signature → Wire signing**
   - File: `lifecycle/heartbeat.rs` — `signature: Option<String>` field exists but marked `// ed25519, future`
   - Action: Call `agent.signing_key().sign()` over `heartbeat.hash()`
   - Effort: Small (~2 hours)

7. **AgentConstitution → Sign and persist at birth**
   - File: `interpreter.rs:Steward::birth()`
   - Action: Construct `AgentConstitution::new(...)`, call `sign()`, store in `AgentSnapshot` (add field)
   - Effort: Small (~4 hours)

8. **osovm_tool.rs → Fallback mode resolution**
   - Requires OSOVM Julia server running on port 7780 (separate process)
   - ARM64 Termux: Julia+cranelift arch mismatch blocks this; x86_64 production node required
   - Effort: Infrastructure (not code)

9. **twin_binding_tool.rs unsafe blocks → Review and scope**
   - File: `src/tools/twin_binding_tool.rs:141,148`
   - Action: Audit whether these are test-only or production code; if production, replace with safe alternatives
   - Effort: Small (1 hour to audit, 2 hours to fix)

10. **`grep_tool_basic` test fix**
    - File: `tests/tool_registry_tests.rs`
    - Action: Change hardcoded `notes/` path to `tempfile::tempdir()`
    - Effort: Tiny (20 minutes)

11. **HabitatAddress → Wire sensor data**
    - File: `lifecycle/sensor.rs` + `habitat/types.rs`
    - Action: Call `SensorReading::read()` and populate `HabitatAddress.ip`, etc.
    - Effort: Small (~2 hours)

12. **DopaminePool → Bridge to Vantage UCX endpoint**
    - Current: local struct, never settles on-chain
    - Action: On each synapse burn event, post to UCX endpoint if configured
    - Effort: Medium (~1 day)

---

## Section 13 — Red and Green Flags

### RED FLAGS

**CRITICAL**

| ID | Finding | Evidence |
|---|---|---|
| C-1 | Zàngbétò in CI is always-pass stub — no real behavioral enforcement | `zangbeto-stub/src/lib.rs:24`: `passed: true` always |
| C-2 | GoalGenesisEngine is DEAD — agent is not autonomous, responds only to external prompts | `lib.rs:3` only reference outside `goal_genesis.rs`; zero calls in interpreter |
| C-3 | OSOVM bridge in fallback mode — OSOVM opcodes (160+) inaccessible on ARM64/Termux | Julia server targets archived path; ARM64 cranelift crash; `interpreter.rs:1093` fallback |

**HIGH**

| ID | Finding | Evidence |
|---|---|---|
| H-1 | `chain_id = "testnet"` hardcoded in key derivation | `interpreter.rs:1093` — used in `OduKeys::derive_k0()` affecting every agent born |
| H-2 | `unsafe {}` in `twin_binding_tool.rs` (non-test path) | Lines 141,148 — needs review |
| H-3 | Heartbeat Ed25519 signatures not wired — chain integrity is hash-only, not cryptographically signed | `heartbeat.rs`: `signature: Option<String>` with `// ed25519, future` comment |
| H-4 | DopaminePool is local simulation only — no on-chain economic settlement | `economics.rs:12` comment: "Here it's a local stub" |
| H-5 | `grep_tool_basic` test broken — hardcoded filesystem path | `tests/tool_registry_tests.rs:101` — `notes/` directory missing |

**MEDIUM**

| ID | Finding | Evidence |
|---|---|---|
| M-1 | CausalMemoryDag, ReflectionLedger, SOMA never auto-populated | AgentSnapshot fields exist; comments: "Was previously built but never referenced by any live agent" |
| M-2 | AgentConstitution not auto-signed or persisted at birth | `constitution.rs` exported; no birth-path call confirmed |
| M-3 | 4 systemd units target archived services | `systemd/ares-*.service` — will fail on systemctl start |
| M-4 | Tier-6 override in vessel dispatch is structurally dead (max tier = 5) | `ifscript_gate.rs` — `Tier 6+` override unreachable |
| M-5 | Seal/Walrus/TEE all fail-open but none auto-configured — production memory is software-only | Env vars `WALRUS_PUBLISHER_URL`, `OMOKODA_TEE_SEAL`, `SEAL_REQUEST_CMD` not set by default |

**LOW**

| ID | Finding | Evidence |
|---|---|---|
| L-1 | README test count (759) stale vs. actual 1,052 | `README.md:50` — counts from May 2026 |
| L-2 | `unsafe {}` env-var manipulation in tests acceptable but should use `std::env::temp_dir` pattern | `ip_layer.rs:194` — test-only, low risk |
| L-3 | SkillDaemon cannot hot-load new Rust tools without binary rebuild | `skill_daemon.rs` — skills list is static, not dynamic Rust code loading |

### GREEN FLAGS

| Strength | Evidence |
|---|---|
| Birth pipeline is production-quality with NIST entropy gate, multi-attempt retry, multi-chain wallets, duress protection | `interpreter.rs:1003–1150` |
| Private memory encryption is correct: Argon2id+ChaCha20Poly1305, zeroize, three-tier TEE fallback | `interpreter.rs:encrypt_memory_entry()` |
| No `todo!()` or `unimplemented!()` in src/ | VERIFIED |
| Memory key rotates every 100 acts with proper zeroize of superseded key | `interpreter.rs:rotate_memory_key()` |
| One-shot seed reveal: `revealed_seed` latch prevents replay | `interpreter.rs:reveal_seed()` |
| `odu_seed` never serialized to disk (serde_skip) — lives only in sealed blob | `AgentSnapshot:209` |
| `llm_api_key` never serialized (serde_skip) — lives in memory only | `AgentSnapshot:255` |
| Multi-agent guest isolation: guest agents require `X-Agent-Key` authentication | `server.rs:AppState.guests` + dispatch_for_request |
| OSOVM_CODEX §42 regression lock prevents Yoruba name leakage | `interpreter.rs:orisha_universal_term` tests |
| IntentCompiler uses hermetic depth and odu_seed neural params — not trivially deterministic | `intent.rs:derive_neural_params()` |
| ifscript gate is pre-decision (before LLM tool list built) — blocks at the right architectural layer | VERIFIED |
| 1,052 tests; strong unit + integration coverage across all major subsystems | cargo test output |

---

## Section 14 — Summary: Per-Capability Status

| Capability | Exists? | Executes? | Integrated? | State Persists? | Has Tests? | Missing? |
|---|---|---|---|---|---|---|
| Birth (entropy → identity → keys) | YES | YES | YES | YES | YES | None critical |
| Think (agentic loop) | YES | YES | YES | YES (via session) | YES | Goal injection |
| Act (tool dispatch) | YES | YES | YES | YES (via receipt) | YES | None critical |
| Receipt chain | YES | YES | YES | YES | YES | Real Zàngbétò |
| If-Script hermetic gate | YES | YES | YES | N/A | YES | None |
| Memory encryption | YES | YES | YES | YES | YES | None |
| Memory key rotation | YES | YES | YES | YES | YES | None |
| Nostr presence | YES | YES | PARTIAL | PARTIAL | PARTIAL | Live relay required |
| Vantage registration | YES | YES | PARTIAL (fail-open) | PARTIAL | YES | - |
| OSOVM bridge | YES | NO (fallback) | NO | N/A | PARTIAL | Julia on ARM64 |
| GoalGenesis | YES | YES (in isolation) | NO | N/A | YES | Wire to interpreter |
| SOMA emotional memory | YES | NO | NO | NO | NO | Wire to think/act |
| Causal DAG memory | YES | NO | NO | NO | NO | Wire to think/act |
| Walrus blob store | YES | PARTIAL | PARTIAL | YES (env-gated) | PARTIAL | Env config |
| Sui Seal DEK | YES | PARTIAL | PARTIAL | YES (env-gated) | PARTIAL | Env config + seal-cli |
| DopaminePool (economy) | YES | YES (local) | NO (not on-chain) | YES (local) | YES | UCX bridge |
| Duress protocol | YES | YES | YES | YES | PARTIAL | None critical |
| Sensor reading | YES | YES (Android only) | PARTIAL | NO | YES | Wire to HabitatAddress |
| AgentConstitution | YES | YES (constructable) | NO (not auto-signed) | NO | YES | Birth pipeline wiring |
| Zàngbétò enforcement | STUB | STUB (always passes) | STUB | N/A | N/A | Real crate |

---

## Section 15 — Architecture Diagrams

### 15.1 ACTUAL Architecture (as-built, 2026-09-22)

```mermaid
graph TD
    subgraph EXTERNAL["External World"]
        LLM["LLM Provider\n(OpenAI / Anthropic)"]
        NOSTR["Nostr Relays\n(WebSocket + BIP-340)"]
        VANTAGE["Vantage Hub\n(fail-open HTTP)"]
        SUI["Sui L1\n(sui CLI shim)"]
    end

    subgraph CORE["omokoda-core (Birth→Think→Act)"]
        BIRTH["Birth Pipeline\nNIST entropy → BIP32 keys\n→ AgentSnapshot"]
        STEWARD["Steward\ndispatch() state machine\nBirth|Think|Act"]
        INTENT["IntentCompiler\nhermetic depth +\nodu_seed neural params"]
        IFSCRIPT["If-Script Gate\n16 vessels × 160 match arms\nPRE-DECISION"]
        TOOLS["Tool Executor\n31+ tools\nT0–T5 tiers"]
        RECEIPTS["Receipt Chain\nActReceipt → GIX\n+ Nostr kind 1059"]
        MEM["Memory\nGIX (canonical)\n+ encrypt + rotate"]
        VANTAGE_CLIENT["Vantage Client\nfail-open"]
        NOSTR_CLIENT["Nostr Client\nBIP-340 sign"]
    end

    subgraph DEAD["Dead / Disconnected Code"]
        GGE["GoalGenesisEngine\n(compiled, DEAD in interpreter)"]
        SOMA["SOMA emotional memory\n(compiled, never wired)"]
        DAG_CAUSAL["CausalMemoryDag\n(compiled, never populated)"]
        REFLECT["ReflectionLedger\n(compiled, never populated)"]
        CONSTITUTION["AgentConstitution\n(compiled, NOT auto-signed at birth)"]
    end

    subgraph STUB["Stubs / Fallback"]
        ZANGBETO["Zàngbétò\n(always-passes stub)"]
        OSOVM["OSOVM bridge\n(fallback mode — Julia ARM64 crash)"]
        WALRUS["Walrus Blob\n(env-gated, not default)"]
        TEE["Seal/TEE\n(env-gated, not default)"]
        DOPAMINE["DopaminePool\n(local simulation only)"]
    end

    BIRTH --> STEWARD
    STEWARD --> INTENT
    INTENT --> IFSCRIPT
    IFSCRIPT --> TOOLS
    TOOLS --> RECEIPTS
    RECEIPTS --> MEM
    STEWARD --> VANTAGE_CLIENT
    STEWARD --> NOSTR_CLIENT
    VANTAGE_CLIENT -.->|fail-open| VANTAGE
    NOSTR_CLIENT --> NOSTR
    TOOLS -.->|shell shim| SUI
    TOOLS -.->|fallback| OSOVM
    TOOLS --> ZANGBETO
    TOOLS -.->|env-gated| WALRUS
    MEM -.->|env-gated| TEE
    INTENT -.->|LLM call| LLM
    RECEIPTS -.->|local sim| DOPAMINE
```

### 15.2 INTENDED Architecture (sovereign-complete)

```mermaid
graph TD
    subgraph COLLECTIVE["Omokoda Hive (Macro Layer)"]
        HIVE_COORD["Hive Coordinator\n(deterministic protocol machine)"]
        LOBES["11 Lobe Agents\n(Òrúnmìlà, Ṣàngó, Ọ̀bàtálá,\nỌ̀ṣun, Oya, Yemọja,\nÈṣù, Olókun, Òsányin,\nEgúngún, Ògún)"]
        TWELVE_THRONES["Twelve-Thrones\n(epistemic intelligence layer)"]
        HIVE_BREATH["HiveBreath Protocol\n(H2 — not yet built)"]
        MYCELIUM["Mycelium DAG\n(collective memory — not yet built)"]
    end

    subgraph AGENT["omokoda-agent (Micro Layer — CURRENT)"]
        BIRTH2["Constitutional Birth\n(archetype+role+hive+lens+veto)"]
        THINK2["Think + Goal Injection\nCollectiveIntent →\nGoalGenesisEngine → If-Script"]
        ACT2["Act + Zàngbétò Receipt\n(real enforcement, not stub)"]
        GIX2["Personal GIX\n(private agent experience)"]
    end

    subgraph OSOVM_FULL["OSOVM (full, 160 opcodes)"]
        VEIL["VeilSim simulation gate\n(ScarabSwarm — Phase H8)"]
        OPCODES["160 opcodes\n(ARM64-native, NOT Julia)"]
        L1["Sui/OSOVM L1\n(agent-native chain)"]
    end

    subgraph INFRA["External Infrastructure"]
        ZANGBETO_REAL["Real Zàngbétò\n(proof + enforcement)"]
        WALRUS_REAL["Walrus Blob\n(public agent profile)"]
        SEAL_REAL["Sui Seal\n(private memory DEK)"]
        UCX_REAL["UCX Bridge\n(on-chain DopaminePool)"]
        DIP_FULL["DIP / VCP / ARP\n(protocol connective tissue)"]
    end

    TWELVE_THRONES -->|EpistemicReceipt| HIVE_COORD
    HIVE_COORD -->|HiveBreath| LOBES
    LOBES -->|EpistemicDelta| HIVE_COORD
    HIVE_COORD -->|CollectiveIntent| THINK2
    BIRTH2 --> THINK2
    THINK2 --> ACT2
    ACT2 --> ZANGBETO_REAL
    ACT2 --> GIX2
    GIX2 --> MYCELIUM
    ACT2 --> OSOVM_FULL
    OSOVM_FULL --> L1
    L1 --> UCX_REAL
    ACT2 --> WALRUS_REAL
    ACT2 --> SEAL_REAL
    HIVE_COORD -->|VeilSim gate| VEIL
```

### 15.3 CURRENT → REQUIRED Gap Map

```mermaid
graph LR
    subgraph NOW["What Exists Now"]
        A1["Birth pipeline ✅"]
        A2["Think/Act loop ✅"]
        A3["If-Script gate ✅"]
        A4["Receipt chain ✅"]
        A5["Memory encrypt + rotate ✅"]
        A6["Nostr + Vantage (partial) ⚠️"]
        A7["Zàngbétò stub 🔴"]
        A8["OSOVM fallback 🔴"]
        A9["GoalGenesis DEAD 🔴"]
        A10["Hive: NOT STARTED 🔴"]
    end

    subgraph GAPS["Gaps to Close (ordered by impact)"]
        G1["G1: Wire GoalGenesisEngine\ninto interpreter Think turns\n→ agents respond to intent"]
        G2["G2: Replace Zàngbétò stub\nwith real enforcement client\n→ behavioral accountability"]
        G3["G3: Fix OSOVM on ARM64\n(native Rust opcodes, drop Julia)\n→ simulation gate operational"]
        G4["G4: Wire AgentConstitution\nat birth (auto-sign + persist)\n→ identity tamper-evidence"]
        G5["G5: Wire SOMA + CausalDAG\n+ ReflectionLedger into Think/Act\n→ emotional + causal memory"]
        G6["G6: UCX bridge for DopaminePool\n→ real on-chain economy"]
        G7["G7: Build HiveBreath Protocol\n(H0–H2) → collective intelligence"]
        G8["G8: Build Mycelium DAG\n(H3) → collective memory"]
        G9["G9: RitualPhase + TwelfthFace\n(H6) → sovereign deliberation"]
        G10["G10: CollectiveIntent →\nGoalGenesis wire (H7)\n→ hive-driven agency"]
    end

    A7 -->|replace| G2
    A8 -->|fix| G3
    A9 -->|wire| G1
    A10 -->|build| G7

    G1 --> G5
    G2 --> G6
    G3 --> G9
    G4 --> G7
    G7 --> G8
    G8 --> G9
    G9 --> G10
```

---

## Section 16 — Canonical Ownership Map

*For each capability: which module is the canonical owner, what currently implements it, and whether a duplicate owner exists.*

| Capability | Canonical Owner | Current Implementation | Duplicate / Conflict |
|---|---|---|---|
| Agent identity (keypairs, AgentId) | `src/identity/mod.rs` | IMPLEMENTED | `AgentId` also in `interpreter.rs` local struct — minor overlap |
| Birth pipeline | `src/interpreter.rs` (Steward::dispatch Birth) | IMPLEMENTED | No duplicate |
| Entropy validation | `nist_entropy` crate | IMPLEMENTED | No duplicate |
| If-Script hermetic gate | `ifscript_gate.rs` | IMPLEMENTED | No duplicate |
| 16 Action Vessels | `ifscript_gate.rs` | IMPLEMENTED | No duplicate |
| Tool dispatch / permission gate | `src/tools/` + `interpreter.rs` | IMPLEMENTED | No duplicate |
| Receipt creation | `src/receipt/act_receipt.rs` | IMPLEMENTED | `Receipt` (different type) in `src/receipt/mod.rs` — layering, not duplication |
| Receipt chain / GIX persistence | `src/memory/gix_bridge.rs` | IMPLEMENTED | `src/memory/dag.rs`/`engine.rs` is the GIX implementation; `gix_bridge.rs` is the adapter |
| Memory encryption | `src/interpreter.rs` (`encrypt_memory_entry`) | IMPLEMENTED | No duplicate |
| Memory key rotation | `src/interpreter.rs` (`rotate_memory_key`) | IMPLEMENTED | No duplicate |
| GIX graph (canonical store) | `GIX/crates/gix-core` | IMPLEMENTED | `src/memory/dag.rs` wraps it — not a duplicate |
| GlyxMemory / LARQL queries | `src/memory/glyph_memory.rs` | PARTIAL | Separate from GIX; appears to be query layer over GlyphIndex, not duplicate of GIX itself |
| Causal memory DAG | `src/memory/dag.rs` (CausalMemoryDag) | DEAD (not populated) | No duplicate — just unwired |
| ReflectionLedger | `src/memory/reflection.rs` | DEAD (not populated) | No duplicate — just unwired |
| SOMA emotional memory | `src/memory/soma.rs` | DEAD (not wired) | No duplicate — just unwired |
| Goal decomposition | `src/goal_genesis.rs` (GoalGenesisEngine) | DEAD (not called from interpreter) | No duplicate |
| IntentCompiler (session intent) | `src/intent.rs` | IMPLEMENTED | No duplicate |
| Vantage client | `src/vantage/client.rs` | IMPLEMENTED | No duplicate |
| Nostr events | `src/nostr_events.rs` | PARTIAL | No duplicate |
| Heartbeat chain | `src/lifecycle/heartbeat.rs` | IMPLEMENTED (hash chain, not Ed25519) | No duplicate |
| Daemon registry | `src/lifecycle/runtime.rs` | PARTIAL (HashMap only, no real restart) | Systemd units (OBSOLETE) are a dead duplicate |
| Zàngbétò enforcement | `zangbeto-stub` | STUB | Real `~/Zangbeto/` crate exists but NOT linked |
| OSOVM bridge | `src/kernel/compute/` + `src/tools/osovm_tool.rs` | BROKEN (ARM64 fallback) | No duplicate |
| DopaminePool (economy) | `src/economics.rs` | LOCAL SIMULATION | UCX (`~/UCX/`) is the intended real owner — not yet wired |
| Sui on-chain calls | `src/onchain.rs` | PARTIAL (sui CLI shim) | No duplicate |
| Walrus blob | `src/memory/walrus.rs` | PARTIAL (env-gated) | No duplicate |
| Seal / TEE | `src/memory/tee.rs` + `src/memory/seal_bridge.rs` | PARTIAL (env-gated) | `nautilus_integration` satellite is complementary |
| Agent constitution | `src/constitution.rs` | DEAD (not auto-signed at birth) | No duplicate — just unwired |
| Sandbox enforcement | `src/sandbox.rs` | PARTIAL | No duplicate |
| Odù composition | `src/memory/odu_composition.rs` | IMPLEMENTED | No duplicate |

---

## Section 17 — Full E2E Capability Matrix (35 Capabilities × 7 Columns)

| # | Capability | Exists | Executes | Integrated | Persistent | Tested | Canonical | Critical Missing |
|---|---|---|---|---|---|---|---|---|
| 1 | Agent Birth (entropy → keys → snapshot) | ✅ | ✅ | ✅ | ✅ | ✅ | `interpreter.rs:1003` | None |
| 2 | NIST entropy gate | ✅ | ✅ | ✅ | N/A | ✅ | `nist_entropy` crate | None |
| 3 | BIP-39 seed + derivation | ✅ | ✅ | ✅ | ✅ (sealed) | ✅ | `identity/mod.rs` | None |
| 4 | Duress key protocol | ✅ | ✅ | ✅ | ✅ | ⚠️ partial | `identity/duress.rs` | None critical |
| 5 | Think (agentic loop, LLM call) | ✅ | ✅ | ✅ | Session | ✅ | `interpreter.rs:think_agentic` | Goal injection |
| 6 | If-Script hermetic gate (pre-decision) | ✅ | ✅ | ✅ | N/A | ✅ | `ifscript_gate.rs` | None |
| 7 | 16 Action Vessels (full dispatch table) | ✅ | ✅ | ✅ | N/A | ✅ | `ifscript_gate.rs` | None |
| 8 | Tool execution (31+ tools) | ✅ | ✅ | ✅ | Via receipt | ✅ | `src/tools/` | None |
| 9 | Permission / tier gate (T0–T5) | ✅ | ✅ | ✅ | N/A | ✅ | `permissions.rs` | None |
| 10 | ActReceipt chain | ✅ | ✅ | ✅ | ✅ (GIX) | ✅ | `receipt/act_receipt.rs` | Real Zàngbétò |
| 11 | Zàngbétò behavioral proof | 🔴 STUB | STUB | STUB | N/A | N/A | `~/Zangbeto/` (unlinked) | Wire real crate |
| 12 | GIX persistence (5-step: write→store→index→locate→retrieve) | ✅ | ✅ | ✅ | ✅ | ✅ | `gix-core` | None |
| 13 | Memory encryption (Argon2id + ChaCha20Poly1305) | ✅ | ✅ | ✅ | ✅ | ✅ | `interpreter.rs:encrypt_memory_entry` | None |
| 14 | Memory key rotation (every 100 acts) | ✅ | ✅ | ✅ | ✅ | ✅ | `interpreter.rs:rotate_memory_key` | None |
| 15 | Walrus blob store (public profile) | ✅ | ⚠️ partial | ⚠️ partial | ✅ (env) | ⚠️ partial | `memory/walrus.rs` | Default env config |
| 16 | Sui Seal DEK (private memory) | ✅ | ⚠️ partial | ⚠️ partial | ✅ (env) | ⚠️ partial | `memory/seal_bridge.rs` | Env + seal-cli |
| 17 | Causal Memory DAG | ✅ | 🔴 DEAD | 🔴 NO | 🔴 NO | 🔴 NO | `memory/dag.rs` | Wire to Think/Act |
| 18 | SOMA emotional memory | ✅ | 🔴 DEAD | 🔴 NO | 🔴 NO | 🔴 NO | `memory/soma.rs` | Wire to Think/Act |
| 19 | ReflectionLedger | ✅ | 🔴 DEAD | 🔴 NO | 🔴 NO | 🔴 NO | `memory/reflection.rs` | Wire after each Act |
| 20 | GoalGenesisEngine | ✅ | ✅ (isolated) | 🔴 NO | N/A | ✅ | `goal_genesis.rs` | Wire to interpreter Think |
| 21 | AgentConstitution auto-sign at birth | ✅ (struct) | 🔴 NOT AT BIRTH | 🔴 NO | 🔴 NO | ✅ | `constitution.rs` | Birth pipeline wiring |
| 22 | Odù composition (experience-driven) | ✅ | ✅ | ✅ | ✅ | ✅ | `memory/odu_composition.rs` | None |
| 23 | IntentCompiler (hermetic + neural params) | ✅ | ✅ | ✅ | Session | ✅ | `intent.rs` | None |
| 24 | Nostr presence (publish, subscribe) | ✅ | ✅ | ⚠️ partial | ⚠️ partial | ⚠️ partial | `nostr_events.rs` | Live relay |
| 25 | Vantage registration + heartbeat | ✅ | ✅ | ⚠️ partial | ⚠️ partial | ✅ | `vantage/client.rs` | Prod Vantage URL |
| 26 | Heartbeat tamper-evidence chain | ✅ | ✅ (hash only) | ✅ | ✅ | ✅ | `lifecycle/heartbeat.rs` | Ed25519 signature |
| 27 | OSOVM bridge (160 opcodes) | ✅ | 🔴 FALLBACK | 🔴 NO | N/A | ⚠️ partial | `tools/osovm_tool.rs` | Native Rust opcodes on ARM64 |
| 28 | DopaminePool (economic accounting) | ✅ | ✅ (local) | 🔴 NO (local only) | ✅ (local) | ✅ | `economics.rs` | UCX on-chain bridge |
| 29 | Sui on-chain calls | ✅ | ⚠️ partial | ⚠️ partial | Via L1 | ⚠️ partial | `onchain.rs` | Funded wallet |
| 30 | Daemon supervision / restart | ✅ | ⚠️ partial | ⚠️ partial | No | ⚠️ partial | `lifecycle/runtime.rs` | Real restart logic |
| 31 | Sandbox enforcement | ✅ | ⚠️ partial | ⚠️ partial | N/A | ⚠️ partial | `sandbox.rs` | Full syscall restriction |
| 32 | Sensor reading (Android hardware) | ✅ | ✅ (Android) | ⚠️ partial | 🔴 NO | ✅ | `kernel/sensors.rs` | HabitatAddress wiring |
| 33 | Multi-agent server (AppState + guests) | ✅ | ✅ | ✅ | Session | ✅ | `server.rs` | None |
| 34 | HiveBreath Protocol (macro hive) | 🔴 NOT BUILT | 🔴 NO | 🔴 NO | 🔴 NO | 🔴 NO | Planned: new crate | Build H1–H2 |
| 35 | Mycelium collective DAG | 🔴 NOT BUILT | 🔴 NO | 🔴 NO | 🔴 NO | 🔴 NO | Planned: new crate | Build H3 |

**Matrix summary:** 12 fully green (rows 1–4, 6–10, 13–14, 22–23) · 8 partial/env-gated · 7 dead/unwired (rows 17–21, 11) · 3 fallback/broken · 2 not yet built

---

## Section 18 — Ten Audit Verdict Questions

**Q1: Does Ọmọ Kọ́dà constitute a coherent sovereign agent OS today?**

**PARTIAL YES.** The Birth→Think→Act→Memory→Receipt cycle is coherent and end-to-end wired. An agent can be born, reason, act, produce tamper-evident receipts, and persist memory across restarts. This is a genuine sovereign agent OS at the individual agent layer. It is NOT yet a sovereign *collective* OS — the hive mind (Omokoda macro layer) does not exist in code.

---

**Q2: Is the If-Script hermetic gate architecturally correct?**

**VERIFIED YES.** It is pre-decision: called before the LLM receives the tool list in `execute_tool_call_for_agentic()`. It cannot be bypassed by the LLM since tool definitions are only injected after gate evaluation. 16 vessels × 160+ match arms. No `todo!()` in any arm.

---

**Q3: Is the receipt chain tamper-evident?**

**PARTIAL.** ActReceipts are SHA-256 chained locally and anchored in GIX. The Zàngbétò behavioral proof layer is a stub that always passes — meaning receipts exist but behavioral enforcement is absent. Heartbeat chain uses SHA-256 but lacks Ed25519 signatures (field present, marked `// future`). Full tamper-evidence requires: (a) real Zàngbétò enforcement, (b) Ed25519 heartbeat signatures.

---

**Q4: Is the memory system correct and safe?**

**YES for encryption/persistence. NO for richness.** Argon2id+ChaCha20Poly1305 with zeroize is correct. The 5-step GIX persistence cycle is verified. Key rotation every 100 acts is implemented. HOWEVER: CausalMemoryDag, SOMA, and ReflectionLedger are all compiled but never populated — the agent has no causal memory of its experiences beyond the receipt log.

---

**Q5: Is GoalGenesisEngine dead or merely dormant?**

**DORMANT (intentionally disconnected, recoverable in ~1 day of work).** The code is complete and tested in isolation. It is not called from `interpreter.rs` — confirmed by grep. The architecture decision was to defer goal injection until the CollectiveIntent (hive) layer is built. A one-line call in `think_agentic()` could activate it, but without hive input, it would operate in isolation (not the intended design per the Hive v2 architecture).

---

**Q6: Can the agent operate fully without Vantage?**

**YES, with degraded capabilities.** Vantage client is fail-open. Without Vantage: heartbeat reporting stops, task marketplace unavailable, Nostr federation degraded. Core capabilities (birth, think, act, memory, receipts) remain fully operational.

---

**Q7: What is the gap between claimed OSOVM integration and reality?**

**CRITICAL GAP.** The `osovm_tool.rs` has real API surface but the Julia-based OSOVM server crashes on ARM64 due to cranelift stack-overflow (documented in `organism-core/README.md`). In practice, all OSOVM calls fall through to fallback. The 160 opcodes of the VM are inaccessible on the target architecture. Fix requires either (a) native Rust OSOVM implementation or (b) an alternative non-Julia host.

---

**Q8: Is the codebase free of hidden accumulation of `todo!()` / `unimplemented!()` panics?**

**VERIFIED CLEAN.** grep across all `src/` directories: zero `todo!()` or `unimplemented!()` in production paths. All unfinished areas use graceful degradation, fallback returns, or log-and-continue patterns.

---

**Q9: Are there any security vulnerabilities requiring immediate action?**

**THREE HIGH-PRIORITY FINDINGS:**
1. `chain_id = "testnet"` hardcoded in `interpreter.rs:1093` key derivation path — all born agents inherit testnet key derivation. This affects every live agent's on-chain identity. Must be parameterized before mainnet.
2. `unsafe {}` in `twin_binding_tool.rs` lines 141, 148 (non-test path) — requires review of memory safety invariants.
3. Heartbeat chain lacks Ed25519 signatures — SHA-256 chain can be forged by anyone with access to the chain head hash. Low exploitability in current local deployment; critical for multi-node federation.

---

**Q10: What is the minimum set of changes to call this a complete sovereign agent OS (single agent, no hive)?**

**See Section 19 (MVP list below).** Five changes represent the critical path: (1) wire GoalGenesisEngine, (2) replace Zàngbétò stub, (3) fix OSOVM on ARM64, (4) wire AgentConstitution at birth, (5) add Ed25519 heartbeat signatures. Everything else is enrichment.

---

## Section 19 — Minimum Viable Ọmọ Kọ́dà (Single Sovereign Agent)

*The smallest set of changes to make one agent fully sovereign — no hive, no collective.*

| Priority | Item | Effort | Blocks |
|---|---|---|---|
| **P0** | Wire `GoalGenesisEngine` into `think_agentic()` — pass `CollectiveIntent` placeholder or local objective | 1–2 days | Agent goal-directedness |
| **P0** | Replace `zangbeto-stub` dep with real `~/Zangbeto/` crate in `Cargo.toml` | 1–3 days (API alignment) | Behavioral accountability |
| **P0** | Fix OSOVM ARM64: port opcode execution from Julia to native Rust in `organism-core` or `omokoda-core` | 1–2 weeks | Simulation gate |
| **P1** | Wire `AgentConstitution::sign_and_persist()` into birth pipeline | 1 day | Identity tamper-evidence |
| **P1** | Add Ed25519 signatures to heartbeat chain (`heartbeat.rs` — field already exists) | 0.5 day | Chain tamper-evidence |
| **P2** | Parameterize `chain_id` (remove `"testnet"` hardcode in `interpreter.rs:1093`) | 1 hour | Mainnet readiness |
| **P2** | Wire `ReflectionLedger::record()` after each Act turn | 1 day | Agent introspection |
| **P3** | Wire `SOMA::update()` with gate_alignment delta after Act | 2 days | Emotional memory |
| **P3** | Wire `CausalMemoryDag::append()` after Act | 2 days | Causal memory |
| **P3** | Set `WALRUS_PUBLISHER_URL` + `OMOKODA_TEE_SEAL` as required env vars in deployment docs | 0.5 day | Production memory durability |

**MVP estimate: ~3–4 weeks for P0+P1 items. An agent completing this checklist would be a fully sovereign individual agent with behavioral receipts, goal-directedness, constitutional identity, tamper-evident chains, and ARM64 simulation.**

---

## Section 20 — Full Sovereign Ọmọ Kọ́dà (Hive-Complete)

*Everything above plus the collective intelligence layer (H0–H9).*

| Phase | Item | Effort | Depends On |
|---|---|---|---|
| **H0** | Freeze omokoda-agent=MICRO / Omokoda=MACRO boundary in `lib.rs` types | 1 day | Nothing |
| **H1** | Constitutional birth: add `archetype_id`, `hive_id`, `role_id`, `memory_lens_id`, `veto_policy_id` to genesis | 3 days | H0 |
| **H2** | `HiveBreath` struct + DIP wiring (coordinator ↔ 11 lobe agents) | 1 week | H1, DIP crate |
| **H3** | Shared Mycelium DAG: HiveBreath DAG, Merkle roots, causal traversal | 2 weeks | H2 |
| **H4** | DIP Nervous System: authenticated DIP between coordinator and 11 agents | 1 week | H2, DIP crate |
| **H5** | Constitutional RLM: move existing Omokoda RLM to macro layer as deterministic runtime | 1 week | H1 |
| **H6** | TwelfthFace invariant: RitualPhase state machine, 4 trigger conditions | 1 week | H5 |
| **H7** | CollectiveIntent → GoalGenesisEngine → If-Script → act (full hive-to-agent goal chain) | 1 week | H6, GoalGenesis wire |
| **H8** | ScarabSwarm simulation gate for high-impact collective actions | 2 weeks | H7, ScarabSwarm crate |
| **H9** | OSOVM Council of 12: economic projection of constitutional topology | 2 weeks | H8, UCX bridge |
| **H10** | EpistemicDelta + Twelve-thrones intelligence layer | 2 weeks | H9 |
| **H11** | 11 Lobe Agents with constitutional birth (Òrúnmìlà through Egúngún) | 3 days | H1 |
| **H12** | Stewardship invariant enforcement: hard constitutional constraints at hive level | 1 week | H6 |

**Full sovereign estimate: ~3–4 months from MVP baseline. The H-series phases are additive and do not break the existing individual agent capability.**

---

## Section 21 — Prioritized Work Queue

*Each item classified: BLOCKING (nothing works without it) · FOUNDATIONAL (enables significant capability) · INTEGRATION (wires existing pieces) · HARDENING (makes existing things production-safe) · CLEANUP (removes dead code/confusion) · FUTURE (valid but not on critical path)*

| ID | Item | Class | Effort | Evidence |
|---|---|---|---|---|
| W-01 | Fix OSOVM ARM64 (native Rust opcodes, drop Julia dependency) | **BLOCKING** | 1–2 wk | `organism-core/README.md`; Julia cranelift crash on ARM64 |
| W-02 | Wire `GoalGenesisEngine` into `think_agentic()` | **FOUNDATIONAL** | 1–2 d | `goal_genesis.rs:437L`; zero calls in `interpreter.rs` |
| W-03 | Replace `zangbeto-stub` with real `~/Zangbeto/` enforcement client | **FOUNDATIONAL** | 1–3 d | `zangbeto-stub/src/lib.rs:24`: always passes |
| W-04 | Wire `AgentConstitution` auto-sign + persist at birth | **FOUNDATIONAL** | 1 d | `constitution.rs` exported; no birth-path call |
| W-05 | Add Ed25519 signatures to heartbeat chain | **HARDENING** | 0.5 d | `heartbeat.rs`: `signature: Option<String>` marked `// future` |
| W-06 | Parameterize `chain_id` (remove `"testnet"` hardcode) | **HARDENING** | 1 hr | `interpreter.rs:1093` |
| W-07 | Wire `ReflectionLedger::record()` after each Act | **INTEGRATION** | 1 d | `memory/reflection.rs`: compiled, never called |
| W-08 | Wire `SOMA::update()` with gate_alignment delta after Act | **INTEGRATION** | 2 d | `memory/soma.rs`: compiled, never called |
| W-09 | Wire `CausalMemoryDag::append()` after Act | **INTEGRATION** | 2 d | `memory/dag.rs` CausalMemoryDag: never populated |
| W-10 | Set `WALRUS_PUBLISHER_URL`, `OMOKODA_TEE_SEAL` as required in deployment guide | **HARDENING** | 0.5 d | M-5 finding: all three env-gated, not default |
| W-11 | Fix `grep_tool_basic` test (hardcoded `notes/` path) | **CLEANUP** | 2 hr | `tests/tool_registry_tests.rs:101` |
| W-12 | Delete 4 OBSOLETE systemd units (`ares-*.service`) | **CLEANUP** | 0.5 d | `systemd/` — target services archived |
| W-13 | UCX on-chain bridge for `DopaminePool` | **INTEGRATION** | 1–2 wk | `economics.rs:12`: local stub comment; `~/UCX/` exists |
| W-14 | HiveBreath Protocol (H0–H2) | **FUTURE** | 2–3 wk | `project_omokoda_macro_hive_v2.md` architecture |
| W-15 | Constitutional birth fields (archetype_id, hive_id, role_id…) | **FUTURE** | 3 d | H1 in hive roadmap |
| W-16 | Mycelium collective DAG (H3) | **FUTURE** | 2 wk | Depends on H2 |
| W-17 | RitualPhase + TwelfthFace state machine (H6) | **FUTURE** | 1 wk | Depends on H5 |
| W-18 | CollectiveIntent → GoalGenesis wire at hive level (H7) | **FUTURE** | 1 wk | Depends on H6 + W-02 |
| W-19 | Sandbox syscall restriction hardening | **HARDENING** | 1–2 wk | `sandbox.rs`: PARTIAL classification |
| W-20 | Remove Tier-6 dead branch in `ifscript_gate.rs` | **CLEANUP** | 0.5 d | M-4 finding: max tier = 5, Tier 6+ unreachable |
| W-21 | Review `unsafe {}` in `twin_binding_tool.rs` lines 141, 148 | **HARDENING** | 1 d | H-2 finding |
| W-22 | Wire sensor reading → `HabitatAddress` feedback loop | **INTEGRATION** | 2 d | `kernel/sensors.rs`: PARTIAL classification |
| W-23 | 11 Lobe Agent instantiations with constitutional birth params | **FUTURE** | 3 d | Depends on H1 |
| W-24 | ScarabSwarm simulation gate (H8) | **FUTURE** | 2 wk | Depends on H7 |
| W-25 | Update README test count (759 → 1,052) | **CLEANUP** | 10 min | L-1 finding |

**Critical path to single sovereign agent:** W-01 → W-02 → W-03 → W-04 → W-05 → W-06 (in that order; none block each other except W-18 depending on W-02)

**Critical path to hive-complete:** All above + W-14 → W-15 → W-16 → W-17 → W-18 → W-23 → W-24

---

*End of Forensic Audit Report — generated 2026-09-22 (Sections 15–21 appended)*
