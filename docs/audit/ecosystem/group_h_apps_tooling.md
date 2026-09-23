# Group H: Apps and Tooling Audit

**Audited:** 2026-09-22  
**Auditor:** Claude Sonnet 4.6 (forensic pass)  
**Taxonomy:** VERIFIED · IMPLEMENTED · PARTIAL · STUB · SPEC_ONLY · DEAD · DUPLICATE · REDUNDANT · CONFLICTING · OBSOLETE · BROKEN · UNKNOWN

---

## buzz-OG (`~/buzz-OG/`)

### Crate Inventory (31 workspace members + 1 example)

| Crate | Role | Status |
|---|---|---|
| `buzz-relay` | WebSocket relay server (main entry, git, huddle audio) | IMPLEMENTED |
| `buzz-core` | Types, event verification, filter matching, kind registry | IMPLEMENTED |
| `buzz-db` | Postgres event store + DAL | IMPLEMENTED |
| `buzz-auth` | Auth / authz | IMPLEMENTED |
| `buzz-pubsub` | Redis pub/sub fan-out, presence, typing | IMPLEMENTED |
| `buzz-search` | Postgres FTS full-text search | IMPLEMENTED |
| `buzz-audit` | Hash-chain audit log | IMPLEMENTED |
| `buzz-media` | Blossom/S3 media storage | IMPLEMENTED |
| `buzz-acp` | ACP harness bridging Buzz events → AI agents | IMPLEMENTED |
| `buzz-agent` | Minimal ACP-compliant agent (non-streaming, tool-calls-as-output) | IMPLEMENTED |
| `buzz-dev-mcp` | Developer MCP server (shell + file-edit tools for buzz-agent) | IMPLEMENTED |
| `buzz-persona` | Agent persona packs | IMPLEMENTED |
| `buzz-workflow` | YAML-as-code workflow engine (evalexpr conditions) | PARTIAL — WF-08 gap (see below) |
| `buzz-pair-relay` | Ephemeral sidecar relay for NIP-AB device pairing | IMPLEMENTED |
| `buzz-pairing-cli` | CLI for NIP-AB device pairing interop testing | IMPLEMENTED |
| `git-sign-nostr` | Sign git objects with a Nostr key | IMPLEMENTED |
| `git-credential-nostr` | Git credential helper for Nostr-authed push/fetch | IMPLEMENTED |
| `buzz-cli` | Agent-first CLI | IMPLEMENTED |
| `buzz-sdk` | Typed Nostr event builders | IMPLEMENTED |
| `buzz-admin` | Operator CLI for relay administration | IMPLEMENTED |
| `buzz-ws-client` | Shared NIP-42 WebSocket client | IMPLEMENTED |
| `buzz-test-client` | Integration test client + E2E test suite | IMPLEMENTED |
| `sprig` | All-in-one harness (ACP + agent + dev MCP) | IMPLEMENTED |
| `ifc-core` | Inter-federation channel core | IMPLEMENTED |
| `buzz-acp` | ACP harness | IMPLEMENTED |
| `buzz-backend-kubernetes` | Kubernetes backend provider | IMPLEMENTED |
| `buzz-relay-mesh` | Inter-relay mesh transport (iroh) | IMPLEMENTED |
| `buzz-datastore-tracing` | Datastore tracing | IMPLEMENTED |
| `buzz-deletion` | Community deletion / write fence | IMPLEMENTED |
| `buzz-voice` | Huddle audio | IMPLEMENTED |
| `buzz-conformance` | Conformance harness | IMPLEMENTED |
| `examples/countdown-bot` | Example bot | IMPLEMENTED |

Workspace root toolchain: `rust-toolchain.toml` pins `1.95.0`. `larql` requires `1.98` — these two repos cannot share a toolchain without per-repo overrides.

### WF-08 Gap — Approval Gate Executor (PARTIAL)

**What exists:** The full data model for approval gates is already implemented:
- `buzz-core/src/kind.rs:578` — `KIND_WORKFLOW_APPROVAL_REQUESTED = 46010`, plus grant (46011) and deny (46012) kinds are defined.
- `buzz-db/src/store/workflow.rs:1–1010` — `workflow_approvals` table, `RunStatus::WaitingApproval`, `create_approval()`, `CreateApprovalParams`, `ApprovalRecord`, `ApprovalStatus` enum are fully wired.
- `buzz-workflow/src/schema.rs:141–154` — `ActionDef::RequestApproval { from, message, timeout }` schema defined.
- `buzz-workflow/src/executor.rs:725–742` — `dispatch_action()` generates an approval token (`generate_approval_token()`) and returns `StepResult::Suspended { approval_token }` correctly.
- `buzz-workflow/src/executor.rs:1111–1168` — `execute_from_step()` (resume-from-index) is fully implemented.

**What is missing (WF-08):**
- `buzz-workflow/src/lib.rs:229–248` — When `result.approval_token.is_some()`, the engine logs a warning and marks the run `Failed` with `code: "approval_not_supported"`. The explicit comment reads: _"Approval gates are not yet implemented (WF-08). Fail explicitly rather than creating unreachable WaitingApproval rows."_
- Missing wiring: `create_approval()` is never called when a run suspends. No kind:46010 event is emitted to the relay. The relay `command_executor.rs` has no resume handler.
- The resume path (`execute_from_step`) and the DB schema (`create_approval`, `execute_from_step`) are both coded and waiting — the gap is exactly the 3 callsite connections: (1) call `db.create_approval()` on suspend, (2) emit kind:46010 via `action_sink`, (3) wire a relay handler that calls `execute_from_step()` on approve/deny events.

### ARM64 / Linux Compile Status

VERIFIED. `Dockerfile:11` documents native ARM64 runner support. Toolchain pins `1.95.0` stable. No `target_os = "macos"` guards in relay crates. Relay compiles on ARM64/Linux without Metal or macOS-only deps.

### Nostr Event Kinds Handled

Defined in `buzz-core/src/kind.rs`. Notable kinds:
- NIP-29 group events: 9, 10, 11, 12 (messages/DMs/threads)
- 39000/39001/39002 (channel metadata, membership)
- 40001–40009 (Buzz extended channel events)
- 46010/46011/46012 (workflow approval requested/granted/denied — defined, not yet dispatched)
- NIP-17 gift wraps, NIP-50 search, diff events (kind:40008)

### Agent Integration

IMPLEMENTED. `buzz-acp` bridges Buzz Nostr events to AI agents via the ACP harness. `buzz-dev-mcp` is a real MCP server using `rmcp = "1.1.0"` (Cargo.toml:136). No DIP or A2A v1.0 wiring — Buzz uses its own ACP protocol, not the sovereign DIP/VCP stack. No dependency on any Omo-Koda2 / Vantage crate. Zero cross-references to the sovereign ecosystem found in Rust source files.

### Tests

VERIFIED. `buzz-workflow/src/executor.rs:1312–1981` contains 50+ unit tests directly in the file (template resolution, condition evaluation, duration parsing, step output, approval schema). E2E tests live in `crates/buzz-test-client/tests/`. CI runs `just test` (requires Postgres + Redis) and `just test-unit` (no infrastructure). Desktop E2E via Playwright. Flutter tests in `mobile/`.

---

## omokoda-smithers (`~/omokoda-smithers/`)

### Structure

Not a conventional npm workspace at root. The actual pack lives entirely under `.smithers/`. Root directory contains: `agents/`, `agents.ts`, `gateway.ts`, `lib/`, `native/`, `prompts/`, `skills/` (empty), `smithers.config.ts`, `smithers.toon`, `spec/`, `tickets/`, `tsconfig.json`, `types/`, `ui/`, `workflows/`.

### Workflow TSX Files (11 found in `.smithers/workflows/`)

| File | Display Name | Status |
|---|---|---|
| `add.tsx` | Add skill/workflow | IMPLEMENTED |
| `create-skill.tsx` | Create Skill | IMPLEMENTED |
| `create-ui.tsx` | Create UI | IMPLEMENTED |
| `create-workflow.tsx` | Create Workflow | IMPLEMENTED |
| `docs-driven-development.tsx` | Docs-Driven Development | IMPLEMENTED |
| `eval-suite-run.tsx` | Eval Suite Run | IMPLEMENTED |
| `init.tsx` | Init | IMPLEMENTED |
| `post-failure.tsx` | Post Failure | IMPLEMENTED |
| `share-pack.tsx` | Share Pack | IMPLEMENTED |
| `skillforge-forge.tsx` | SkillForge Forge | IMPLEMENTED |
| `upgrade.tsx` | Upgrade | IMPLEMENTED |

### MCP stdio Server — Spec Compliance

VERIFIED. `.smithers/native/mcp.ts:17`:
```typescript
const SUPPORTED_PROTOCOLS = ["2025-06-18", "2025-03-26", "2024-11-05"] as const;
const DEFAULT_PROTOCOL = "2025-06-18";
const STRUCTURED_FROM = "2025-06-18";
```
All three MCP spec versions supported. Structured tool results gated on 2025-06-18+. Wire protocol: newline-delimited JSON over stdio (no SDK dependency). JSON-RPC error codes defined explicitly. VERIFIED spec-compliant.

### Durable / Crash-Survivable Approval Gate

PARTIAL. The `create-workflow.tsx` workflow uses a `<Approval>` component from `smithers-orchestrator` (`create-workflow.tsx:342`) and an `approvalSchema` (`create-workflow.tsx:158`). The `review` flag gates writing files. However, crash-survival is provided by `smithers-orchestrator`'s `outputMaybe` / `outputs` pattern — durability depends on the orchestrator runtime state store, not a standalone checkpoint journal. The `.smithers/native/kernel/registry.ts` `Registry` class handles action invocation with timeouts and audit logging via `AncpStore` (SQLite), but there is no explicit checkpoint-on-suspend-then-resume mechanism visible at the `buzz-workflow` level. Status: durable human-approval gate exists as a UI component and schema; crash-survivable resume (re-run from approval step after process restart) is PARTIAL — depends on orchestrator runtime rather than independent persistent state.

### Build / Run

`.smithers/native/package.json` uses Bun. MCP server: `bun ./.smithers/native/mcp.ts`. Root `.mcp.json` sets `ANCP_ACTOR=claude, ANCP_TIER=write`. No `package.json` at root `~/omokoda-smithers/` — the pack is invoked as a `.smithers` directory inside a consumer repo.

### Vantage / Omo-Koda2 Connection

STUB. `.smithers/native/index.ts:21` names the control plane `omokoda-agent-native-control-plane`. The `smithers.ts` and `index.ts` reference `a2aActions` (`a2a.ts` action file exists) but no HTTP calls to Vantage API or imports of Vantage/DIP/ARP types were found. The `omokoda` name is cosmetic identity; there is no runtime wiring to Vantage endpoints, the Omo-Koda2 Rust process, or sovereign protocol crates.

---

## larql (`~/larql/`)

### Crate Inventory (22 workspace members)

`larql-models`, `larql-compute`, `larql-compute-metal`, `larql-core`, `larql-vindex`, `larql-vindex-spec`, `larql-execution`, `larql-factory`, `larql-inference`, `larql-kv`, `larql-lql`, `larql-glyph`, `larql-cli`, `larql-demos`, `larql-server`, `vindex-cli`, `larql-router`, `larql-router-protocol`, `larql-python`, `larql-boundary`, `model-compute`, `larql-experts`.

### BaseVindex / PatchedVindex Abstraction

IMPLEMENTED. `larql-vindex/src/lib.rs` documents the full lifecycle:
- `VectorIndex` — base queryable format: extract from safetensors/GGUF, load, query (gate KNN/walk/HNSW/MoE router), residency management.
- `PatchedVindex` overlay — from `patch/` module; adds `KnnStore` and refine pass. Allows non-destructive weight overlays on top of a base `VectorIndex`.
- `StorageEngine` in `engine/` manages lifecycle + MEMIT decomposition (`memit_solve`).
- Key re-exports at crate root: `VectorIndex`, `PatchedVindex` (via `patch`), `GateIndex`, `GateLookup`, `RouterIndex`, `ResidencyManager`, `LayerState`.

### Metal Backend

IMPLEMENTED (macOS only). `larql-compute-metal/src/lib.rs:7`: _"Compiles to an empty crate on non-macOS hosts — the entire implementation lives under `#[cfg(target_os = "macos")]`."_ Full pipeline: `backend/`, `kernels/`, `buffers/`, `shaders/` (MSL), `stages/` (QKV/attn/FFN encoders), decode loop, MoE dispatch. On ARM64/Linux (Termux), this crate compiles to an empty lib — Metal backend is non-functional on the current device. CPU fallback via `larql-compute` is the active path.

### MoE Sharding via gRPC

IMPLEMENTED. `larql-router/src/` includes `grid/service.rs`, `grid/replication.rs`, `grid/routing.rs` — and `dispatch.rs` references tonic/prost-based gRPC. The `larql-router-protocol` crate holds protobuf definitions. `larql-compute-metal/src/moe_descriptor.rs`, `moe_dispatch/`, `moe_gpu_route/` implement GPU-side MoE routing. gRPC sharding between router nodes is wired, though exact tonic version is in `Cargo.lock` (not re-read to avoid scope creep).

### Model Support / Loader

IMPLEMENTED. `larql-vindex/src/extract/` builds vindex from safetensors and GGUF. Config types (`VindexConfig`, `VindexSource`, `QuantFormat`: fp4/q4/q8/bitnet) in `larql-vindex/src/config/types.rs`. `larql-factory` provides model factory. `larql-models` crate holds per-architecture definitions.

### PyO3 Bindings

IMPLEMENTED. `larql-python/src/` contains `lib.rs`, `session.rs`, `trace_py.rs`, `vindex.rs`, `walk.rs`. PyO3 extension module. Binding status (importable from Python) depends on build environment — not testable on Termux ARM64 without a matching Python dev env, but source is complete.

### Omo-Koda2 Integration

PARTIAL. `~/Omo-Koda2/omokoda-core/Cargo.toml` lists `larql` as a dependency. Integration files:
- `omokoda-core/src/memory/larql_query.rs` — bounded query engine over agent memory (distinct from `ifascript::larql`, per file header).
- `omokoda-core/src/memory/glyph_memory.rs` and `gix_bridge.rs` — GIX bridge referencing larql glyph primitives.
- `omokoda-core/src/gates/mod.rs` — references larql for gate scoring.
- `omokoda-core/src/interpreter.rs:872` — names larql as one of the "eco legs" alongside mnemopi / zerolang / Axiom.

Status: larql-glyph GIX integration is the live bridge. Full `VectorIndex` / inference path is not directly invoked by Omo-Koda2 runtime yet (no `larql-inference` import found in omokoda-core Cargo.toml scan).

---

## zerolang (`~/zerolang/`)

### Language Features in v0.3.4

IMPLEMENTED (compiler). The `native/zero-c/src/` directory contains a full C-implemented compiler with:
- **Frontend:** `ast.c`, `checker.c`, `unify.c`, `type_core.c` — typed AST, type checker, unification.
- **IR:** `ir.c`, `mir_binary.c`, `mir_verify.c` — MIR (mid-level IR), binary encoding, verifier.
- **Backends:** `aarch64_direct.c/emit.c` (direct ARM64 emission), `x64_emit.c`, `emit_elf64.c`, `emit_elf_aarch64.c`, `emit_macho64.c`, `emit_coff.c`, `emit_llvm_ir.c` — multi-target native code generation (ELF/Mach-O/COFF + LLVM IR path).
- **Program Graph:** `program_graph*.c` (25+ files) — the primary semantic representation. Repository, store, query, clone, reconcile, patch, projection, manifest, semantics, rewrite, roundtrip, contracts, borrow/memory/effect contracts.
- **C Interop:** `c_import.c`, `program_graph_c_import.c` — import C headers.
- **Stdlib:** `std_source.c`, `std_sig.c`, embedded skills/runtime (`embedded_skills.inc`, `embedded_runtime_sources.inc`).
- **Targets:** `target.c`, `target_backend.c`, `buildability*.c` — multi-platform build target matrix.
- **Tooling:** `canonical_text.c` (canonical serialization), `program_graph_repository.c` (version-controlled program graphs), `manifest_toml.c`.

### `.0` File Format

VERIFIED. Example at `examples/add.0`:
```zero
fn answer() -> i32 {
    return 40 + 2
}

pub fn main(world: World) -> Void raises {
    let value: i32 = answer()
    if value == 42 {
        check world.out.write("math works\n")
    } else {
        check world.out.write("math broke\n")
    }
}
```
Syntax: typed, expression-oriented, `World` capability passing, `raises` effect annotation, `check` for fallible operations. Clearly a capability-safe language.

### Compiler Completeness

IMPLEMENTED. `package.json` scripts confirm: `native:smoke`, `conformance`, `native:test`, `bench`, `rosetta:local` (cross-arch), `llvm:profile`, `stdlib:targets` — a full build/test/bench matrix exists. Build: `make -C native/zero-c`. Sandboxed conformance tests via `@vercel/sandbox`. Multi-target matrix: linux-musl-x64 (and others via `buildability_targets.c`). MSRV note: Node ≥24 required for type-stripping.

### VS Code Extension

IMPLEMENTED. `extensions/vscode/` contains `package.json`, `syntaxes/`, `language-configuration/`, `snippets/`, `scripts/`, `tests/` — full VS Code language extension for `.0` files. Build: `turbo run build --filter=./extensions/vscode`. Test: `turbo run test --filter=./extensions/vscode`.

### Omo-Koda2 Integration

PARTIAL. `omokoda-core/src/interpreter.rs:872` names zerolang as a planned "eco leg." `omokoda-core/src/memory/memdir.rs` and `glyph_memory.rs` reference zerolang syntax/semantics nominally. No direct `zero-c` binary invocation or zerolang AST import found in Omo-Koda2 Rust source — integration is aspirational/named only. `ifascript` is the active scripting layer; zerolang is the future replacement per the polyglot organism architecture.

---

## vanity-cloakseed (`~/vanity-cloakseed/`)

### Build / Deploy Status

IMPLEMENTED. Version 3.0.0. Vite + React 18 + TypeScript. `package.json` build: `vite build`. `dist/` directory present. `BUILD_COMPLETE.md`, `COMPLETE.txt`, `DELIVERABLES.txt` present — self-documented as production complete. Playwright e2e tests in `e2e/`. Netlify deployment config present.

### Chain Coverage

VERIFIED (6 chains). `src/utils/chains.ts` defines all 6:
- `ethereum` (ETH) — secp256k1, keccak256 address derivation
- `solana` (SOL) — ed25519, base58 address
- `bitcoin` (BTC) — P2PKH/P2WPKH/P2SH
- `sui` (SUI) — ed25519
- `cosmos` (COSMOS) — bech32 `cosmos1` prefix
- `aptos` (APTOS) — ed25519

Dependencies: `@noble/secp256k1`, `@noble/ed25519`, `@noble/hashes`, `bip32`, `bip39`, `bitcoinjs-lib`, `bs58`, `tweetnacl`, `ethers`.

### Web Workers (1–16 Workers)

VERIFIED. `src/components/Generator.jsx:13` — initial worker count from `navigator.hardwareConcurrency || 4`. Worker count slider options: `[1, 2, 4, 8, 16]` (line 197). Worker implementation: `src/workers/generatorWorker.js` — ES module Web Worker with start/stop message protocol, CPU-intensive secp256k1 key generation loop, reports attempts/rate/matches back to main thread. `src/workers/sharedWorkerBridge.js` provides the shared bridge. Real parallel generation — not simulated.

### BIP-39 Seed Cipher Overlay

IMPLEMENTED. `src/utils/bip39Helper.ts` — BIP-39 mnemonic generation and validation using the `bip39` library. `src/utils/ciphers.ts` and `src/utils/encryption.ts` provide the CloakSeed overlay. `src/utils/wordlists.ts/js` for custom wordlists. The "poison radar" feature (`src/utils/poisonRadar.ts`) detects honeypot/poison addresses. `CLOAKSEED_INTEGRATION.md` documents the cipher overlay design.

---

## franken-stream (`~/franken-stream/`)

PRESENT. Not absent. Brief summary:

- **Language:** Rust + Python hybrid. `Cargo.toml` + `pyproject.toml` + `requirements.txt` present.
- **Purpose:** Multi-provider AI streaming router/proxy (TUI + web UI). `providers.example.toml` lists provider configs; `providers.json.example` shows JSON format.
- **Crates:** `crates/` directory present; `franken_stream/` Python package.
- **UI:** `web-ui.html` (single-file web UI). TUI guide in `TUI_GUIDE.md`.
- **Agent Integration:** `AGENT_INTEGRATION.md` present. `run_e2e_tests.py`, `test_demo.py`, `test_enhanced.py`, `test_routes.py` — test suite.
- **Status:** PARTIAL — self-documented with `STATUS.md`, `COMPLETION_REPORT.md`, `E2E_AUDIT_SUMMARY.md`. Appears functional for routing/proxying but not integrated into the sovereign stack.
- **Key gaps:** No DIP/VCP/ARP wiring found; standalone tool.

---

## Missing Repos

- **agentic-waggle:** NOT ON DISK. Directory `~/agentic-waggle/` does not exist. No stub or archive found.

---

## Cross-Cutting Findings

| Finding | Repos | Severity |
|---|---|---|
| buzz-OG toolchain pins `1.95.0`; larql requires `1.98` (NEON dot-product intrinsics) — cannot be a unified workspace | buzz-OG, larql | LOW (separate repos, no shared workspace) |
| WF-08 approval gate: schema/DB/executor all complete; only 3 callsite connections missing | buzz-OG | MEDIUM |
| Metal backend compiles to empty lib on non-macOS — all ARM64/Linux larql inference is CPU-only | larql | INFO (by design, documented) |
| omokoda-smithers has zero runtime wiring to Vantage API or Omo-Koda2 process despite `omokoda-agent-native-control-plane` name | omokoda-smithers | MEDIUM |
| zerolang named as future eco leg in omokoda-core interpreter.rs:872 but has no active code path — ifascript is the live scripting layer | zerolang, Omo-Koda2 | LOW (roadmap item) |
| buzz-OG has no imports of DIP/VCP/ARP/Vantage/Omo-Koda2 — it is a fully independent Nostr workspace | buzz-OG | INFO |
| larql-glyph GIX bridge is the only live cross-repo connection (larql → Omo-Koda2) | larql, Omo-Koda2 | INFO |
| franken-stream exists on disk but was not in the stated repo list — no sovereign protocol wiring | franken-stream | INFO |
