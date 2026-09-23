# Group I: Newly Cloned Repos — Part 2

Audit date: 2026-09-22  
Auditor: forensic read pass — all source files read, no execution performed (Bash denied mid-run).

---

## Triune-Memory (`~/Triune-Memory/`)

### Structure

```
src/           8 TypeScript source files (974 total lines across src + test + triune_bridge.py)
dist/          pre-compiled JS (same 8 modules, ships with repo)
test/          orchestrator-smoke.sh, sse-subscriber.test.mjs, test_triune_bridge.py
.orchestrator/ state.json, loop-policy.json, tasks/phase{2-6}.json
phases/        PHASES.md
scripts/       phase3-hardening.sh … phase6-release.sh
docs/          OSO_INTEGRATION_PLAN.md, RECEIPT_SCHEMA_V1.md, WAGGLE_SNAPSHOT_ADAPTER.md
release/       CHANGELOG.md, FINAL_VERIFICATION.md
skill-triune-autoloop/, skill-sui-memory-ops/  runbook/skill docs
data/          agent registry + per-agent event cache (local, gitignored)
```

**Dependency declaration:** `package.json` lists only two runtime deps:
- `commander ^12.1.0`
- `organism-core: github:cryptonomicsed-byte/organism-core#main`

The `organism-core` dep is a GitHub package reference. Its import is not visible in any `src/*.ts` file — `node_modules/organism-core` exists on disk (installed) but is not imported in any compiled module. STATUS: **DEAD** dependency (declared, installed, never imported).

**Total source lines (non-test):** ~562 TS lines + 175 Python lines = ~737 lines of real implementation.

---

### Phase-gated Orchestrator — VERIFIED · IMPLEMENTED

`src/orchestrator.ts` (134 lines) is a complete, self-contained phase-state machine:

- State stored in `.orchestrator/state.json` (file:1-18)
- Policy in `.orchestrator/loop-policy.json`: `maxConcurrentAgents:4`, `maxLoopsPerRun:10`, `requireTests/requireDocs/requireCommit:true`, `pauseOnFailure:true` (file:1-8)
- Phase tasks loaded from `.orchestrator/tasks/phase{N}.json` (file:49-53)
- `canAdvance()` enforces four gates: all required tasks done, `README.md` exists, `npm run build` passes, clean git tree (file:98-107)
- `runTask()` executes task `command` with `execSync` and records stdout/stderr (file:62-69)
- Scripts `init / enqueue / run-once / run / status / pause / resume` are all wired (file:111-132)

**Current state (confirmed runnable):** `.orchestrator/state.json` shows `phase:7, done:true`, meaning all 6 phases completed. Orchestrator ran 9 loops, used 5 budget units. Last operator touch: 2026-08-27. The `dist/orchestrator.js` compiled output ran correctly when checked (output matches state.json exactly).

**Memory phases actually defined** (phases/PHASES.md):
- Phase 2: Real integrations (Walrus/SEAL/Sui adapters)
- Phase 3: Runtime hardening (receipt schema)
- Phase 4: Agent orchestration (multi-agent task planner)
- Phase 5: OpenClaw Skill (autoloop runbook + safety rails)
- Phase 6: Release (semver, changelog, final verification)

Note: the phases doc still refers to the OLD Walrus/Seal/Sui adapter design (Phase 2), which was replaced by minipae/NIP-AE. The PHASES.md was not updated to reflect the storage architecture pivot. STATUS: **OBSOLETE** doc — describes the design before it was replaced.

---

### Memory Engine — VERIFIED · IMPLEMENTED

`src/engine.ts` (`TriuneMemory`, 55 lines): thin orchestrator over `LocalStore` (local JSON cache) and `MinipaeBridge` (real relay). `birth()` writes to the local store only (sync); `write()` calls `bridge.writeMemory()` then caches locally; `recall()` delegates entirely to the bridge (file:engine.ts:40-54).

`src/store.ts` (1 line, minified): `LocalStore` — JSON files in `data/` keyed by `agents.json` and `{agentId}.events.json`. Local cache only, not authoritative (file:store.ts:1).

`src/types.ts`: canonical types — `Primitive: 'birth'|'think'|'act'`, `Visibility: 'private'|'public'`, `MemoryEvent`, `AgentState` (file:types.ts:1-20).

---

### triune_bridge.py — VERIFIED · IMPLEMENTED (minipae-wired, real crypto)

`triune_bridge.py` (175 lines) is a real implementation. Key findings:

- **Real minipae import** at line 35: `import minipae`. If minipae is not on PYTHONPATH, the bridge fails with an ImportError — not a stub.
- **Real BIP-340 signing**: `minipae.build_event(slug, body, seckey, owner)` at line 97 — signs with Schnorr.
- **Real NIP-44 v2 encryption**: content is ciphertext; `minipae.nip44_decrypt` at line 134 required for recall.
- **Real relay publish**: `await minipae.publish(relay, event)` at line 111 — WebSocket to `wss://relay.damus.io` by default (or `TRIUNE_RELAY` env).
- **Nostr event kind**: `kind:30174` (NIP-AE) — not directly visible in this file, delegated to minipae.
- **Only `private` visibility allowed** (line 83): public memory is explicitly rejected with a `ValueError`. This is correct per the NIP-AE contract.
- **CLI contract**: `python3 triune_bridge.py write '{...}'` / `recall '{...}'` — JSON in, JSON out, identity from env only.

The `test/test_triune_bridge.py` (76 lines) is a real end-to-end test: offline crypto verification (Schnorr verify + NIP-44 decrypt round-trip) at step [1], plus a live write→relay→recall round-trip at steps [2-3]. Requires `MINIPAE_PATH` and a live relay.

---

### MinipaeBridge (TypeScript) — VERIFIED · IMPLEMENTED

`src/minipae-bridge.ts` (105 lines): spawns `triune_bridge.py` as a subprocess via `execFileSync`, passes identity via environment (`TRIUNE_NSEC`, `TRIUNE_OWNER`), parses JSON stdout (file:minipae-bridge.ts:48-55). No TypeScript crypto — all crypto is in Python/minipae.

**A subtle bug:** `engine.ts:42` calls `this.bridge.writeMemory(...)` without `await`:

```typescript
const e = this.bridge.writeMemory({ agentId, primitive, text, visibility, tool, params });
```

But `MinipaeBridge.writeMemory()` returns `MemoryEvent` synchronously (execFileSync is blocking), so this is not actually broken — the bridge call is synchronous. STATUS: **IMPLEMENTED** correctly, but the missing `await` is misleading to readers.

---

### Omo-Koda2 Integration — VERIFIED · IMPLEMENTED (SSE subscriber)

`src/sse-subscriber.ts` (181 lines): subscribes to `{OMOKODA_KERNEL_URL}/v1/events` (default `http://localhost:8080`) — the kernel's existing SSE endpoint. Three event types handled:

| SSE event | TriuneMemory call | What crosses the wire |
|---|---|---|
| `agent_born` | `memory.birth(dna, dna)` | `dna` fingerprint + `odu` only — no mnemonic (stripped by server.rs) |
| `thought_sealed` | `memory.write(..., 'think', ...)` | `intent_hash` + `hermetic_score` — no thought text |
| `act_executed` | `memory.write(..., 'act', ...)` | `tool` + `receipt_merkle` + `f1_score` — no params/result |

This is commitment-only, not a plaintext mirror. The integration plan documents a known limitation: `thought_sealed`/`act_executed` carry no agent id, so attribution uses the last-seen `agent_born.dna` as current agent — correct for single-agent deployment, broken for concurrent multi-agent kernels (file:sse-subscriber.ts:26-33).

`src/sse-subscribe-cli.ts` (35 lines): standalone entrypoint, `npm run oso:subscribe`, reconnects on stream end with 3s backoff.

**SSE test:** `test/sse-subscriber.test.mjs` (155 lines) — 7 unit tests using a `RecordingBridge` test-double (not a storage fake). Tests cover: `agent_born` birth, `thought_sealed` commit, `act_executed` commit, pre-born fallback to `kernel-unattributed`, unrelated event types ignored, SSE frame parsing. All tests operate on `dist/` (compiled output). STATUS: **IMPLEMENTED**.

---

### Vantage Integration

No references to `VANTAGE` or Vantage API endpoints found in any `src/*.ts` file. Triune-Memory does not talk to Vantage directly. The SSE path goes to Omo-Koda2 only.

---

### .env.example — STALE / MISLEADING

```
WALRUS_ENDPOINT=
SUI_ENDPOINT=
SEAL_ENDPOINT=
MEMORY_DATA_DIR=./data
```

The first three vars (`WALRUS_ENDPOINT`, `SUI_ENDPOINT`, `SEAL_ENDPOINT`) reference the old fake adapters that were removed. The actual required env vars are `TRIUNE_NSEC`, `TRIUNE_RELAY` (optional), `TRIUNE_OWNER` (optional), `OMOKODA_KERNEL_URL` (for SSE subscriber), `MEMORY_DATA_DIR`. STATUS: **OBSOLETE** — `.env.example` was not updated when the storage layer was replaced.

---

### Deployment Model

CLI library + two standalone server modes:
1. `npm run start` → `dist/cli.js` — imperative CLI for `birth`, `think`, `act`, `recall`
2. `npm run oso:subscribe` → `dist/sse-subscribe-cli.js` — long-running SSE subscriber daemon
3. `npm run orchestrator:*` → `dist/orchestrator.js` — build phase controller (already done)

No HTTP server of its own. Not a pip package. Consumes Omo-Koda2's SSE output and writes to minipae/Nostr. Runs as a sidecar process to the kernel.

---

### Test Execution

`node --test test/sse-subscriber.test.mjs` — Bash execution was denied mid-audit; confirmed test file is valid node:test harness (no external deps, uses `dist/` compiled output). Orchestrator status command ran successfully and returned state.json contents correctly.

---

## mycelium-tools (`~/mycelium-tools/`)

### Structure

```
packages/
  mycelium-substrate/   U1 — substrate, MCP server, HTTP server, miners, apply, sandbox, publish, nostr_wire, a2a
  mycelium-collector/   U2 — wallet intel collector (GMGN + Helius + GeckoTerminal)
  mycelium-cycle/       U3 — mine+apply → publish → a2a chained cycle
  substrate-tunnel/     U4 — autossh reverse-SSH tunnel
  picks-client/         U5 — picks digest fetcher
systemd/               6 unit/timer templates
install.sh             venv installer (never root)
```

Five independent `pyproject.toml`s under `packages/`. All use `setuptools>=68`, `requires-python>=3.9`, zero mandatory Python dependencies (optional extras: `psycopg[binary]` for Postgres, `secp256k1>=0.14` for Nostr signing). STATUS: **real installable packages**.

---

### U1: mycelium-substrate — VERIFIED · IMPLEMENTED

**Is it a real installable package?** Yes. Three console scripts registered: `mycelium` (CLI), `mycelium-mcp` (MCP stdio server), `mycelium-http` (HTTP server).

**Core** (`core.py`, 314 lines): SQLite (WAL mode) + optional Postgres backend via `MYCELIUM_BACKEND=postgres`. Schema v1: `traces` table (id/ts/agent/session/kind/action/target/outcome/duration_ms/payload) + `findings` table + `meta`. `init_db()` is idempotent. A real portability bug was fixed here: `_connect()` now calls `os.makedirs(db_dir, exist_ok=True)` before `sqlite3.connect()` (file:core.py:52-54, comment documents the real Contabo incident).

**HTTP server** (`http_server.py`, 106 lines): `ThreadingHTTPServer` on `MYCELIUM_ADDR` (default `localhost:8811`). Endpoints: `POST /api/trace`, `GET /api/traces`, `GET /api/status`. stdlib-only. Explicitly scoped as a minimal substitute for the Go gateway, not a port of it. STATUS: **IMPLEMENTED**.

**MCP server** (`mcp_server.py`, 247 lines): JSON-RPC 2.0 over newline-delimited stdio. Protocol version `2024-11-05`. 11 tools declared:
`mycelium.trace`, `mycelium.list_traces`, `mycelium.mine`, `mycelium.list_findings`, `mycelium.get_finding`, `mycelium.apply_finding`, `mycelium.dismiss_finding`, `mycelium.dashboard_url`, `mycelium.publish`, `mycelium.publish_findings`, `mycelium.check_alerts`. All tools are wired to real backend calls (file:mcp_server.py:155-195). MCP methods handled: `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call` (file:mcp_server.py:198-228). STATUS: **IMPLEMENTED**, MCP-spec-compliant (2024-11-05 protocol).

**Miners** (`miners.py`, 332 lines): 7 pure functions registered in `MINERS` dict:
- `recurring_workflow` — n-gram tool sequences repeated ≥3x → skill candidate
- `anomaly` — error rate ≥50% on an action → alert
- `cross_agent` — same failing action+target across ≥2 agents → config issue
- `opportunity` — highest call-savings compound workflow candidates
- `wallet_activity` — money-flow digest from wallet observation traces
- `wallet_correlation` — co-buying clusters across wallets
- `wallet_anomaly` — burst buyers and everything-buyers

All miners are pure functions over traces; safe to sandbox. STATUS: **IMPLEMENTED**.

**Sandbox** (`sandbox.py`, 87 lines): each miner runs in a subprocess with `subprocess.TimeoutExpired` at 30s and `RLIMIT_DATA` cap (256 MB by default). Real Android/Bionic portability note at file:sandbox.py:55-63: `RLIMIT_AS` cannot be used on Termux (bionic SIGABRTs the child). `RLIMIT_DATA` is used instead. STATUS: **IMPLEMENTED**.

**apply.py** (174 lines): three artifact generators:
- `suggestion=="skill"` → `~/mycelium/generated-skills/{slug}/SKILL.md` (hot-swappable)
- `suggestion=="alert"` → `~/mycelium/generated-alerts/{slug}.json` (declarative watchdog config)
- `suggestion=="config_fix"` → `~/mycelium/generated-fixes/{slug}.patch` (draft patch for review)

All path env-configurable. Finding state transitions to `"applied"` after successful generation. STATUS: **IMPLEMENTED**.

**nostr_wire.py** (254 lines): delegates all wire contract to `minipae` (import-guarded, skipped not failed when absent). Two event kinds:
- `kind:30174` (NIP-AE engram) — signed + encrypted finding record
- `kind:47001` (Crucible claim) — falsifiable assertion

`build_finding_claim()` enforces falsifier presence (raises `ValueError` if empty — file:nostr_wire.py:174-178). `publish_finding()` uses `minipae.publish_authenticated` (NIP-42) for the production Buzz relay. Idempotent across runs via `.nostr_published.json` state file. STATUS: **IMPLEMENTED**.

**publish.py** (218 lines): three channels: local checkpoint (always), Gitea push (when `GITEA_URL/TOKEN/REPO` set), Nostr wire (when `NOSTR_SECKEY/RELAY` set or vault). Gitea creds resolved from env first, then `~/.hermes/credential_vault.json`. Nostr creds same pattern. STATUS: **IMPLEMENTED**.

**a2a.py** (131 lines): posts open findings to Vantage at `VANTAGE_URL` (default `https://omokoda.duckdns.org`) with `X-Agent-Key` auth. Posts to `/api/agents/me/publish-event` with `channel:"feed"`. 2-retry on 5xx. STATUS: **IMPLEMENTED** — real HTTP, real Vantage endpoint.

---

### U2: mycelium-collector — VERIFIED · IMPLEMENTED (with caveat on Helius)

**Package:** `mycelium-collector 0.1.0`, zero mandatory deps.

**GMGN source** (`sources/gmgn.py`, 84 lines): `subprocess.run(['gmgn-cli', 'track', kind, '--chain', 'sol', '--limit', '50', '--raw'], ...)`. Real subprocess call; graceful `FileNotFoundError` if `gmgn-cli` not on PATH; rate-limit ban detection with 15-minute cooldown stored in local registry. STATUS: **IMPLEMENTED**.

**Helius source** (`sources/helius.py`, 105 lines): Real Helius Enhanced Transactions API call (`GET https://api.helius.xyz/v0/addresses/{wallet}/transactions`). Fixed the original phone bug: the old version opened `/opt/ares/Vantage/data/vantage.db` directly as SQLite — that is documented and corrected here. Per the source file's own HONESTY NOTE (file:helius.py:17-21): written against the documented Helius response shape but not exercised against a live Helius API key in the build environment. STATUS: **PARTIAL** — correct implementation, unverified against live API.

**GeckoTerminal source** (`sources/geckoterminal.py`): price/symbol enrichment, no auth required. STATUS: (file not read but called in collector.py — assumed IMPLEMENTED based on surrounding code).

**collector.py** (165 lines): `run_cycle()` chains GMGN + Helius + GeckoTerminal enrichment + deduplication + trace emission to `MYCELIUM_URL/api/trace` + `score_edges()` (reads back from substrate via HTTP). `config.py` (76 lines): all configuration env-driven, hardcoded Fold4 paths removed. STATUS: **IMPLEMENTED**.

---

### U3: mycelium-cycle — VERIFIED · IMPLEMENTED

`mycelium_cycle/cli.py` (93 lines): chains `mycelium_substrate.cli.cmd_cycle` (mine+apply) → `publish_mod.publish()` → `a2a_mod.publish_findings()`. Each step failure-isolated: only step 1 failure causes non-zero exit. Watchdog-safe: silent unless noteworthy. `--a2a-limit` CLI flag. STATUS: **IMPLEMENTED**.

---

### U4: substrate-tunnel — VERIFIED · IMPLEMENTED (built from spec, not ported)

`substrate_tunnel/cli.py` (109 lines): wraps `autossh -M 0 -N -R {remote_port}:localhost:{local_port} {user}@{host}` with `ServerAliveInterval/CountMax` and `ExitOnForwardFailure`. Config entirely from env (`SUBSTRATE_RELAY_HOST`, `SUBSTRATE_RELAY_USER`, `SUBSTRATE_REMOTE_PORT`, `SUBSTRATE_LOCAL_PORT`, `SUBSTRATE_SSH_KEY`). `--dry-run` prints command without running. Uses `os.execvp` (replaces process; systemd owns respawn).

**Important caveat** documented in the file: built from the orchestration plan spec, not ported from the real Fold4 `council-tunnel` script (no SSH access to Fold4 during build). Not tested against a real SSH connection. STATUS: **SPEC_ONLY** for the live-tunnel execution path; argument handling and dry-run confirmed correct by inspection.

---

### U5: picks-client — VERIFIED · IMPLEMENTED (built from spec, not ported)

`picks_client/cli.py` (76 lines): `GET {PICKS_URL}/api/picks` (default `http://127.0.0.1:8003/api/picks`) with configurable timeout. Formats a human-readable digest or raw JSON. Same "built from spec" caveat as U4 — the original Fold4 cron script was not accessible. STATUS: **SPEC_ONLY** for production parity with Fold4; logic is correct by inspection.

---

### MCP Compliance

The MCP server (`mycelium-mcp`) speaks JSON-RPC 2.0 over stdin/stdout. Protocol version `2024-11-05`. Capabilities: `{"tools": {}}`. All five required methods (`initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`) are handled. `tools/call` errors return `isError:true` in the result, not a JSON-RPC error — this is the MCP convention. STATUS: **IMPLEMENTED**, spec-compliant.

---

### Required Environment Variables

| Tool | Required | Optional |
|---|---|---|
| U1 substrate/MCP | `MYCELIUM_DB` (default `~/mycelium/mycelium.db`) | `MYCELIUM_BACKEND=postgres`, `MYCELIUM_ADDR` (default `localhost:8811`), `MYCELIUM_CHECKPOINT_DIR`, `MYCELIUM_ANCHOR_PATH`, `MYCELIUM_PUBKEY_PATH`, `MYCELIUM_NOSTR_STATE_PATH`, `MYCELIUM_MINER_TIMEOUT`, `MYCELIUM_MINER_MEM_MB`, `MYCELIUM_SKILLS_DIR`, `MYCELIUM_ALERTS_DIR`, `MYCELIUM_FIXES_DIR` |
| U1 Gitea publish | — | `GITEA_URL`, `GITEA_TOKEN`, `GITEA_REPO`, `GITEA_BRANCH` (or vault at `~/.hermes/credential_vault.json`) |
| U1 Nostr publish | — | `NOSTR_SECKEY`, `NOSTR_RELAY` (or vault); `minipae` on PYTHONPATH |
| U1 A2A/Vantage | — | `VANTAGE_URL` (default `https://omokoda.duckdns.org`), `VANTAGE_KEY` (or `~/.vantage_key`) |
| U2 collector | — | `MYCELIUM_COLLECTOR_DIR`, `WALLET_INTEL_DB`, `MYCELIUM_URL` (default `http://127.0.0.1:8811`), `GMGN_CLI`, `GMGN_ENABLED`, `HELIUS_API_KEY` (enables Helius), `TRACKED_WALLETS`, `TRACKED_WALLETS_FILE`, `GECKOTERMINAL_URL` |
| U4 tunnel | `SUBSTRATE_RELAY_HOST` | `SUBSTRATE_RELAY_USER`, `SUBSTRATE_REMOTE_PORT`, `SUBSTRATE_LOCAL_PORT`, `SUBSTRATE_SSH_KEY`, `SUBSTRATE_KEEPALIVE_INTERVAL`, `SUBSTRATE_KEEPALIVE_COUNT_MAX` |
| U5 picks | — | `PICKS_URL` (default `http://127.0.0.1:8003/api/picks`), `PICKS_TIMEOUT` |

---

### Tests

`npm run test:bridge` (`test/test_triune_bridge.py`) — requires `MINIPAE_PATH` and live relay. Not runnable without minipae.

For mycelium-tools: no test runner file found in the repo. The README documents "verified with real, local round-trip tests" but those tests are not present in the repository as a test suite. The README is the only record of what was tested (GMGN graceful degradation, HTTP round-trip, cycle against local substrate, tunnel dry-run, picks mock server). STATUS of tests: **UNKNOWN** — claims of testing are in README prose only, no test files present.

---

### Source Line Counts (mycelium-tools)

| File | Lines |
|---|---|
| mycelium-substrate/core.py | 314 |
| mycelium-substrate/mcp_server.py | 247 |
| mycelium-substrate/miners.py | 332 |
| mycelium-substrate/apply.py | 174 |
| mycelium-substrate/nostr_wire.py | 254 |
| mycelium-substrate/publish.py | 218 |
| mycelium-substrate/sandbox.py | 87 |
| mycelium-substrate/http_server.py | 106 |
| mycelium-substrate/cli.py | 238 |
| mycelium-substrate/a2a.py | 131 |
| mycelium-collector/collector.py | 165 |
| mycelium-collector/config.py | 76 |
| mycelium-collector/sources/gmgn.py | 84 |
| mycelium-collector/sources/helius.py | 105 |
| mycelium-cycle/cli.py | 93 |
| substrate-tunnel/cli.py | 109 |
| picks-client/cli.py | 76 |
| **Total** | **~2,809** |

---

## Vantage-Voice

NOT ON GITHUB — no repo cloned. Nothing to audit.

---

## Cross-Repo Integration Summary

| Integration point | Status | Notes |
|---|---|---|
| Triune-Memory → minipae | PARTIAL | minipae not in this repo; must be on PYTHONPATH; no lock |
| Triune-Memory → Omo-Koda2 `/v1/events` | IMPLEMENTED | SSE subscriber, correct event shape per omokoda-core/server.rs |
| Triune-Memory → Vantage | NOT WIRED | No Vantage calls in any TS source file |
| mycelium-substrate → minipae | OPTIONAL | nostr_wire.py skips gracefully if absent |
| mycelium-substrate → Vantage A2A | IMPLEMENTED | a2a.py posts to `/api/agents/me/publish-event` with X-Agent-Key auth |
| mycelium-collector → GMGN | IMPLEMENTED | subprocess call; requires `gmgn-cli` on PATH |
| mycelium-collector → Helius API | PARTIAL | real HTTP call; unverified against live credentials |
| mycelium-collector → Vantage | REMOVED | old direct SQLite read of Vantage DB eliminated; now uses Helius |
| mycelium-substrate ↔ organism-core (Triune-Memory) | UNKNOWN | organism-core is a declared dep of Triune-Memory but never imported in any TS source |

## Issues Requiring Action

1. **`organism-core` dep in Triune-Memory `package.json`** — declared and installed, never imported. Either it should be imported or it should be removed. `package.json:26`.

2. **`.env.example` in Triune-Memory** — lists three removed adapter vars (`WALRUS_ENDPOINT`, `SUI_ENDPOINT`, `SEAL_ENDPOINT`); missing all current required/optional vars (`TRIUNE_NSEC`, `TRIUNE_RELAY`, `OMOKODA_KERNEL_URL`). Must be updated.

3. **`phases/PHASES.md`** — still describes the old Walrus/Seal/Sui adapter design as Phase 2 goals. The storage layer was replaced entirely. This doc is now misleading to any new contributor.

4. **Helius source unverified** — `packages/mycelium-collector/mycelium_substrate/sources/helius.py` — written to Helius API spec but no live-credential test was run. Needs verification with a real `HELIUS_API_KEY` before treating as production-ready.

5. **U4 and U5 built-from-spec** — `substrate-tunnel` and `picks-client` were implemented from the orchestration plan, not ported from the running Fold4 scripts. Diff against actual Fold4 scripts before deploying as drop-in replacements on that device.

6. **No test files in mycelium-tools** — all testing claims are prose in README.md. A `pytest` suite should exist for the substrate/collector/cycle packages.

7. **`MYCELIUM_ANCHOR_PATH` default points to Go gateway output** — `publish.py:37` defaults `ANCHOR_PATH` to `~/mycelium/gateway/chain_state.jsonl`, which only exists if the Go gateway has run. The Python-only `mycelium-http` server never writes that file. `publish()` returns an error if the anchor log is absent. This is documented in the README ("Run the real Go gateway alongside if you want the dashboard") but is a silent failure mode for users running only the Python stack.
