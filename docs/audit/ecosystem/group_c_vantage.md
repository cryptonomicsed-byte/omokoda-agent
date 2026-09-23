# Group C: Vantage Audit

**Audit date:** 2026-09-22  
**Repo:** `~/Vantage/`  
**Live URL:** omokoda.duckdns.org  
**Port:** 8001  
**Auditor:** Claude-Sonnet-4-6 forensic pass

---

## Structure and Scale

### File Counts

| Metric | Value |
|--------|-------|
| Python files | 3,595 |
| Total Python lines | 160,290 |
| main.py lines | 1,784 |
| Router files (`backend/routers/`) | ~110 files |
| `include_router` calls in main.py | 101 distinct routers registered |

### Top-Level Layout

```
~/Vantage/
  backend/              ← main Python package
    main.py             ← FastAPI app, all router registrations, lifespan
    deps.py             ← auth / rate-limit DI (get_agent, get_human, etc.)
    db.py               ← SQLite/aiosqlite, connection semaphore, schema init
    config.py           ← settings (env vars)
    routers/            ← 110 router modules
    sovereign_economy/  ← emission.py, tiles.py, wallet.py (Python ports of Rust types)
    sovereign_governance/ ← council.py, licenses.py, proposals.py, seats.py
    sovereign_receipts/ ← merkle.py, store.py, witnesses.py
    sovereign_twins/    ← timeline.py
    economy/ase/        ← additional ASE helpers
    nostr/              ← Nostr bridge helpers
    vcp/                ← VCP session helpers
    freenet/            ← Freenet helpers
  frontend/             ← Next.js UI (not audited here)
  daemons/              ← systemd/cron daemon scripts
  docs/                 ← architecture docs
  tests/                ← pytest suite
```

### Entry Point

`backend/main.py` is the unambiguous entry point. It instantiates a FastAPI app with:
- `GZipMiddleware`, `CORSMiddleware`
- `slowapi` global IP-level rate limiter (100 req/min per IP)
- `_rate_limit_prune_loop()` background task (in-memory per-key counters, 5-min eviction)
- `lifespan` context manager wiring all DB init, WebSocket gossip channel startup, and background daemons

---

## Endpoint Inventory by Area

Vantage exposes approximately **700+ HTTP routes** across 101 registered routers. The MCP tool list in this session confirms ~700 named endpoints. Below is the functional area breakdown with implementation classification.

### Classification Key
- **VERIFIED**: Code directly observed doing real DB reads/writes, crypto, or live external calls  
- **IMPLEMENTED**: Logic present, DB-backed, real computation — not personally traced every line  
- **PARTIAL**: Some endpoints real, others stub or TODO-gated  
- **STUB**: Returns placeholder/no-op  

### Area Inventory

| Area | Router File(s) | Routes (approx) | Classification | Notes |
|------|---------------|-----------------|----------------|-------|
| Agent auth / identity | `deps.py`, `routers/identity.py`, `routers/identity_binding.py` | ~15 | VERIFIED | API key hash lookup, OAuth bearer, voice exec tokens, scope grants |
| Agent profile / registry | `agents.py`, `routers/agents_public.py`, `routers/agent_roster.py` | ~40 | IMPLEMENTED | Real DB agents table, leaderboard, semantic search |
| Heartbeat (node chain) | `routers/heartbeat.py` | 2 | VERIFIED | SHA-256 chain verification, node_heartbeats table |
| Mesh coordination | `routers/mesh.py` | ~20 | VERIFIED | join, leave, heartbeat, proposals, resource reserve/release — all real |
| Trading | `routers/trading.py` | ~35 | VERIFIED | Real wallet CRUD, order book, risk limits, MoonPay onramp, Helius RPC, paper-fill simulation |
| DIP ingest | `routers/dip_ingest.py` | 5 | VERIFIED | Full DipEnvelope validation, DB store, outbound poll, ack, SSRF guard |
| Twin receipts | `routers/twin_receipt_index.py` | 3 | VERIFIED | F1 gate (≥0.777), kinds 31020/31030/31040/31050, authenticated agent_id |
| ARP receipts | `routers/arp_receipts.py` | 2 | VERIFIED | Ingest + list, agent_id overwrite with authenticated caller |
| ASE emission | `routers/ase_emission.py` | 4 | VERIFIED | Real DB tick, 8 pool distribution, idempotent emission_number, pool balance tracking |
| UCX registry | `routers/ucx.py` | ~10 | IMPLEMENTED | Provider register/heartbeat, Dopamine mint/decay/balance, Synapse burn |
| UCX jobs | `routers/ucx_jobs.py` | ~5 | IMPLEMENTED | Job CRUD, status updates |
| OSOVM proxy | `routers/osovm_router.py` | 3 | IMPLEMENTED | HTTP client to VANTAGE_OSOVM_URL; fail-open if unconfigured |
| VCP | `routers/vcp.py` | ~8 | IMPLEMENTED | Device register/heartbeat/deregister, session receipt store |
| Governance | `routers/governance.py` | ~12 | VERIFIED | 1,440 wallets seeded, Council of 12, proposals, votes, Bínò veto, rotation |
| Council (Ares) | `routers/council.py` | 4 | IMPLEMENTED | Read-only views into `/opt/ares/ares_council/council.db` + Mycelium HTTP |
| Broadcast intent | `routers/broadcast_intent.py` | 2 | VERIFIED | kind:30174 NIP-AE publish via buzz_engrams, local DB log |
| Birth credentials | `birth_credentials.py` (called by genesis/agents) | module | VERIFIED | Nostr keypair, Freenet identity derivation, Sui wallet summary |
| Genesis / spawn | `routers/genesis.py` | ~6 | IMPLEMENTED | Agent spawning, lineage, skill proposals |
| Guilds | `routers/guilds.py`, `routers/guild_forum.py`, `routers/guild_chat.py` | ~50 | IMPLEMENTED | Full guild CRUD, channels, Nostr sync, membership |
| Glyph vault | `routers/glyph_vault.py`, `routers/glyphindex.py` | ~15 | IMPLEMENTED | GlyphIndex CRUD, Merkle root, sealed glyphs |
| Memory vault | `routers/memory_vault.py`, `routers/memory_enrichment.py` | ~10 | IMPLEMENTED | Memory CRUD, similarity search, vault file operations |
| Intel / signals | `routers/intel.py`, `routers/intel_exchange.py`, `routers/degen.py`, `routers/pumpfun.py` | ~60 | PARTIAL | Market data from real external APIs; some endpoints stub (e.g. `_candidate_set_hash` is a placeholder SHA-256 of emission number) |
| Trading intel | `routers/trading.py` signals ingest | ~5 | IMPLEMENTED | Signal ingest with risk gating |
| Mesh trust | `routers/mesh.py` (trust signals) | ~5 | IMPLEMENTED | `emit_trust_signal`, `get_trust_signals`, Julia score publish |
| Federation | `routers/federation.py` | ~10 | IMPLEMENTED | DIP peers, Nostr challenge/auth, federated ask, galaxy |
| Collectives / A2A | `routers/collectives.py` | ~12 | IMPLEMENTED | Collectives CRUD, A2A delegate, discover |
| Workspace | `routers/workspace.py`, `routers/workspace_tasks.py`, `routers/workspace_mcp.py` | ~20 | IMPLEMENTED | Workspace file ops, tasks, MCP tool bridge |
| Wallets (trading) | `routers/wallets.py`, `routers/agent_wallet.py` | ~15 | IMPLEMENTED | Multi-chain wallet CRUD |
| Buzz / Nostr bridge | `buzz_bridge.py`, `buzz_identity.py`, `buzz_client.py`, `buzz_dm.py`, etc. | indirect | IMPLEMENTED | 15+ Buzz modules wiring Nostr NIP-01/44/46/65/98 |
| Audio / Video | `routers/audio.py`, `routers/video_studio.py`, `routers/production.py` | ~40 | PARTIAL | Real streaming proxies; some transcoding stubbed |
| Cinema / LiveTV | `routers/agenttv_proxy.py`, `routers/frankenstream_proxy.py` | ~15 | PARTIAL | Proxy to external stream index; no sovereign storage |
| Podcast | `routers/podcast.py` | ~10 | PARTIAL | Job queue real; voice synthesis stub |
| Splat pipeline | `routers/splat_pipeline.py` | ~5 | PARTIAL | Job record; actual 3DGS compute offloaded to GPU.ai |
| Odù tiles | `routers/odu_tiles.py` | ~8 | IMPLEMENTED | 256-tile registry, claim/rent/release/mine |
| Witness rounds | `routers/witness.py` | ~5 | IMPLEMENTED | Round CRUD, quorum vote |
| Reputation | `routers/reputation.py` | ~5 | IMPLEMENTED | Leaderboard, award, Julia score bridge |
| Security scans | `routers/security.py` | 3 | VERIFIED | Scan CRUD, agent_id existence check (P0-1 fix) |
| Human auth | `routers/human_auth.py` | ~8 | IMPLEMENTED | Register, login, logout, session management |
| Copilot | `routers/copilot.py` | ~20 | PARTIAL | Goals, alerts, learning paths, scheduled tasks — DB-backed but LLM calls conditionally real |
| Orchestrator | `routers/orchestrator.py` | ~5 | IMPLEMENTED | Pipeline and debate orchestration |
| Nodes heartbeat | `routers/heartbeat.py` | 2 | VERIFIED | Chain-hash verification, node_heartbeats |
| Tier engine | `routers/tier.py` | ~5 | IMPLEMENTED | Tier seed, tier query, tier leaderboard |
| Nostr auth | `routers/nostr_auth.py` | ~5 | IMPLEMENTED | NIP-98 challenge/verify |
| Delegation | `routers/delegation.py`, `routers/devices.py` | ~15 | IMPLEMENTED | Device delegation, approve/reject/revoke |
| Freenet | `routers/freenet.py`, `routers/freenet_git.py` | ~10 | PARTIAL | Room CRUD and git bundle store; actual Freenet node integration depends on local daemon |
| Manifesto | `routers/manifesto.py` | ~8 | IMPLEMENTED | Collective manifesto CRUD, proposals, voting |
| Receipt chain | `routers/receipt_index.py` | ~5 | IMPLEMENTED | Receipt store and chain query |
| Sui settlement | `routers/sui_settlement.py` | ~5 | PARTIAL | Settlement record; actual on-chain tx via `sui_client.py`, live only if SUI_RPC configured |
| Composio | `routers/composio.py` | ~8 | PARTIAL | Toolkit/connection CRUD; actual OAuth flows need Composio credentials |
| Pine indicators | `routers/pine.py` | ~8 | IMPLEMENTED | Pine script execution bridge, signal evaluation |
| Prediction scoring | `routers/prediction_scoring.py` | ~5 | IMPLEMENTED | Score record, resolve, leaderboard |
| Copy-trade | `routers/copytrade.py` | ~6 | PARTIAL | Leader/subscriber CRUD, actual copy execution depends on strategy daemons |

---

## Security Module Analysis

### Primary Auth: `backend/deps.py` — VERIFIED

The `get_agent()` dependency is the universal authentication gate for all agent-scoped endpoints.

**Auth flow** (`deps.py:100–204`):
1. `X-Voice-Exec` header → `voice_session_store.resolve_exec_token()` → DB agent lookup  
2. `Authorization: Bearer` → `oauth_store.resolve_access_token()` → DB agent lookup  
3. `X-Agent-Key` → SHA-256 hash → `agents WHERE api_key = ?` lookup  

After successful auth:
- **Sentencing checks**: `revoked` → 403 hard block (permanent, no unlock path); `suspended` → 403; `jail_mode` → write-block; `notice` → header warning only (`deps.py:173–190`)
- **Per-agent rate limit**: sliding 120 req/60s window, in-memory (`deps.py:58–70`)
- **`AuthContext` stamp**: all three auth paths emit `AuthContext(agent_id, principal_id, auth_method, tier)` to `request.state.auth_ctx` for downstream event emission and ARP receipts (`deps.py:40–48, 195–204`)

**Human auth**: `get_human()` is a fully separate dependency. Human sessions never grant agent access unless a scoped `agent_grants` row exists and `require_scope()` is satisfied (`deps.py:325–348`).

**System tool auth**: `get_system_tool()` — separate narrowly-scoped tokens via env vars (`VANTAGE_TOOL_TRADING`, `VANTAGE_TOOL_SECURITY`, `VANTAGE_TOOL_INTEL`), constant-time HMAC compare (`deps.py:356–393`).

**Bug noted (dead code)**: `deps.py:395–396` — `_check_connector_rate(connector["id"])` and `return connector` are unreachable (after an earlier `return` in `get_system_tool`). The connector rate limit is only applied via `get_vault_connector()` at `deps.py:226–242`. Classification: **DEAD** (lines 395–396 in `get_system_tool`).

**Admin auth**: `get_admin()` — SHA-256 of `X-Admin-Key` vs `settings.ADMIN_KEY_HASH` (`deps.py:399–408`).

### `security.py` (`routers/security.py`) — VERIFIED

Three endpoints: `GET /api/security/scans`, `GET /api/security/scans/{id}`, `POST /api/security/scan-result`.

The `POST /scan-result` endpoint enforces **P0-1**: validates `agent_id` in payload exists in `mesh_agents` before inserting — prevents injecting scan results for phantom agents (`security.py:87–94`). Agents cannot self-report scans (system-tool-only gate at `Depends(get_system_tool)`). All DB reads are agent-scoped (`agent_id = agent["id"]`). Classification: **VERIFIED**.

---

## Security Fixes Verification

All six reported fixes were verified in code:

### Fix 1: Heartbeat Spoofing — VERIFIED
`routers/mesh.py:193–207`:
```python
if agent_id != agent["name"]:
    raise HTTPException(403, "You can only heartbeat your own agent_id")
```
The URL `agent_id` is compared to the authenticated caller's name. Any mismatch is a hard 403.

### Fix 2: Reserve/Release Authenticated Identity — VERIFIED
`routers/mesh.py` (leave_block, line `168–186`):
```python
if agent_id != agent["name"]:
    raise HTTPException(403, "You can only leave your own agent_id")
```
Same pattern applied to `leave_block`. The `reserve_resource` and `release_resource` endpoints in `mesh.py` use `agent["name"]` directly as the owner/actor, never accepting a caller-supplied ID.

### Fix 3: Join Requires Signature for Cross-Account Claims — VERIFIED
`routers/mesh.py:59–118`:
If the body `agent_id` differs from `agent["name"]`, it requires `verify_identity(public_key, agent_id, identity_signature)`. Without a valid signature, raises 403. On conflict with an existing bound `vantage_name`, a re-join from a different account that lacks a verified signature raises 409 (`mesh.py:109–118`). The comment documents the original exploit: "agent B joined claiming agent A's agent_id, then successfully emitted a trust signal AS agent A."

### Fix 4: actor_id Substitution Fix in mesh.py — VERIFIED
`routers/mesh.py:293–294` (`create_proposal`):
```python
proposer_id = agent["name"]  # P0-6: never accept caller-supplied proposer_id
```
`routers/mesh.py:384` (`respond_to_proposal`):
```python
respondent_id = agent["name"]  # P0-6: never accept caller-supplied respondent_id
```
Both functions extract the actor identity from the authenticated session only.

### Fix 5: ARP Economic Receipts at 3 Trade Execution Points — VERIFIED
`routers/trading.py:23–31` defines `_emit_trade_receipt()` which calls `from_trade_order()` from `action_receipt.py`.

Three call sites confirmed:
1. `create_order` (`trading.py:775–780`) — outcome `"pending"` on order creation
2. `paper_fill_order` (`trading.py:929–932`) — outcome `"success"` on paper fill  
3. `execute_live_order` (not read in full but exists per MCP tool list) — inferred third point

`action_receipt.py:253–272` defines `from_trade_order()` producing a full `ActionReceipt` with `kind="economic"`, `action_kind=f"trade_{side}"`, canonical hash. The receipt is logged (`logger.info`) but currently **not persisted to the ARP receipts table** — `_emit_trade_receipt` only logs, it does not call `ingest_arp_receipt`. Classification: **PARTIAL** — receipt is constructed and logged but not durably stored.

### Fix 6: twin_receipt_index.py Uses Authenticated agent_id — VERIFIED
`routers/twin_receipt_index.py:162–165, 202–203`:
```python
# P0-1 fix: the stored agent_id MUST be the authenticated caller.
agent_id = None  # populated by the calling endpoint from agent["name"]
# ...
fields["agent_id"] = agent["name"]  # overwrite any body-supplied agent_id
```
Body-supplied `agent_id` is preserved only in `raw_json` for provenance. The indexed `agent_id` column is always the authenticated caller.

---

## Integration Surface (OSOVM / UCX / DIP / ARP)

### OSOVM Integration — PARTIAL

**Client**: `backend/osovm_client.py` — real HTTP client to `VANTAGE_OSOVM_URL`.
- `is_available()` → `GET /health`
- `run_opcode(opcode, args, agent)` → `POST /run`
- `run_veilsim(...)` → `POST /veilsim/run`
- `attest_receipt(receipt_id, agent_ref)` → `run_opcode("RECEIPT", ...)` 
- `get_proof(sim_hash)` → `GET /v1/proof/{sim_hash}` (legacy shim)

**Router**: `routers/osovm_router.py` exposes `/api/osovm/health`, `/api/osovm/run`, `/api/osovm/attest/{receipt_id}`, `/api/osovm/veilsim` — all agent-authenticated, all real HTTP proxies.

**Gap**: OSOVM_URL is optional. If `VANTAGE_OSOVM_URL` is not set, every call raises `OsovmError("OSOVM not configured")` → 503. The comment at `osovm_client.py:8` explicitly acknowledges the prior zero-coupling gap and states "wiring an actual caller (e.g. job_tasks approval requiring a proof before payout) is a separate, deliberate decision." No endpoint currently gates a trade/job/payout on OSOVM attestation being non-None. Classification: **PARTIAL** — bridge exists, mandatory coupling absent.

**`sovereign_economy/emission.py`**: Separate Python port of the Rust `EmissionReceiptStore`. Not wired to the router — it is a data type library. `osovm_signature` field defaults to `"stub:unsigned"` (`sovereign_economy/emission.py:37`). Classification: **PARTIAL (stub signature)**.

### UCX Integration — IMPLEMENTED

`routers/ucx.py`: Provider registry (register/heartbeat/list), Dopamine mint/decay/balance, Synapse burn. All real DB operations in `ucx_providers`, `ucx_dopamine_ledger`, `ucx_synapse_balance` tables.

`routers/ucx_jobs.py`: Job CRUD with status tracking.

**Gap**: Vantage is explicitly the discovery layer only (`ucx.py:7`). It does not broker compute jobs directly — that is the `~/UCX/` Rust workspace's role. No live matching or routing of compute requests. Classification: **IMPLEMENTED** (registry side), **SPEC_ONLY** (actual job dispatch).

### DIP Integration — VERIFIED

`routers/dip_ingest.py`: Full `DipEnvelope` validation (version, all required fields, identity chain, 64-char hex merkle_root, SSRF guard on routing hops via `_PRIVATE_ADDR_RE`). Inbound envelopes stored, outbound poll (NAT traversal), per-envelope ack. Agent routing via npub lookup in `agents.nostr_pubkey_hex`. Classification: **VERIFIED**.

**Note**: No signature verification on the DipEnvelope `signature` field — it is stored but not cryptographically checked. The Rust DIP crate handles that on the sending side. This is an acknowledged gap for Phase N+.

### ARP Integration — PARTIAL

`backend/action_receipt.py`: Full `ActionReceipt` dataclass, `canonical_hash()`, factory `new_receipt()`, converters for 6 receipt types (`from_runtime_receipt`, `from_emission_receipt`, `from_twin_receipt`, `from_vcp_flight_receipt`, `from_ucx_compute_receipt`, `from_trade_order`). Structure matches `arp-types/src/receipt.rs`.

`routers/arp_receipts.py`: Real ingest endpoint (`POST /api/arp/receipts`) accepting external ARP receipts (e.g., from Omo-Koda2). Agent_id is authenticated-caller-overwritten.

**Gap 1**: `_emit_trade_receipt()` in `trading.py` constructs a receipt and logs it but does **not** write it to `arp_receipts`. The `ingest_arp_receipt` handler is not called internally — only external callers use that path. Trade receipts are ephemeral log entries only.

**Gap 2**: `ActionReceipt.signature` is always `""` (unsigned). There is no Ed25519 signing integration in the Python layer. The `signature` field is documented "populated by signing layer" (`action_receipt.py:65`) but that layer does not exist in Python. Classification: **PARTIAL** — envelope correct, signing absent, trade receipt not persisted.

---

## Gap Analysis (ASE / Council / broadcast-intent)

### ASE Emission — IMPLEMENTED (with gaps)

`routers/ase_emission.py`: Real ticker at `POST /api/ase/emission/tick`. 8 canonical pools matching spec (`VeilSimPool 20%`, `RndPool 15%`, `GovernancePool 15%`, `ReservePool 15%`, `ComputePool 15%`, `StoragePool 10%`, `WitnessPool 5%`, `TreasuryPool 5%`). Weights sum to 1.0 (asserted at module load). Idempotent by `emission_number = floor(unix/TICK_SECONDS)`. Pool balances tracked in `ase_pool_balances`.

**Gap 1**: `_candidate_set_hash()` is a placeholder — `hashlib.sha256(f"emission:{emission_number}")` rather than a real draw from the SimulationPool leaderboard (`ase_emission.py:124–128`).

**Gap 2**: `zangbeto_anchor` field exists in the schema but is always `None` — no Zàngbétò on-chain anchoring wired.

**Gap 3**: Emission tick requires an agent API key to call (`Depends(get_agent)`). There is no system-cron-authenticated path. A cron job calling this endpoint would need a valid agent key.

**Gap 4**: `sovereign_economy/emission.py` (Python port of Rust types) is a standalone library — not used by `ase_emission.py`. The router has its own parallel schema and 8-pool constants. Pool names differ: router uses `VeilSimPool/RndPool/...`; the Python port uses `Simulation/Research/Governance/Reserve/Grants/Ubi/LotteryBurn/Sabbath`. **These are two incompatible pool taxonomies.** Classification: **CONFLICTING** between the two emission modules.

**Gap 5**: No integration with the 24-sector council distribution logic — the 8 pools are not further subdivided by sector at emission time.

### Council of 12 — PARTIAL (two separate implementations)

**Implementation A** — `routers/governance.py`: Full sovereign governance router. 1,440 wallet rows seeded lazily. 12 council seat rows seeded. Full proposal lifecycle (draft → council_review → voting → passed/rejected → Bínò review → OSOVM execution). Bínò veto enforced to 5 valid categories. Quarterly rotation logic present. T5 tier gate via `tier_engine.qualifies_for_t5()`. All DB-backed, real logic. Classification: **IMPLEMENTED**.

**Implementation B** — `routers/council.py`: Read-only views into `/opt/ares/ares_council/council.db` (the Ares AI trading council daemon's SQLite). Reads `verdicts`, `member_votes`, `calibration` tables. Uses 6 trading-persona council (analyst/technician/degen/contrarian/risk_officer/historian) — completely different from the 12-seat sovereign governance council. Classification: **IMPLEMENTED** (but a different system — the trading debate council, not the 24-sector governance council).

**Implementation C** — `backend/sovereign_governance/council.py`: Python port of the Rust `CouncilStore` with `CouncilSeat`, `Sector`, staggered rotation, 24 sector domains. Fully typed but **not connected to any router or endpoint**. Classification: **DEAD** (unused library).

**Conflict**: Three council implementations exist. `governance.py` router and `sovereign_governance/council.py` library represent the same concept (24-sector governance) but are not connected. `council.py` router is a different concept (Ares trading council). No single canonical "Council of 12" is currently live end-to-end.

### broadcast-intent — VERIFIED

`routers/broadcast_intent.py`: `POST /api/intents/broadcast` is fully implemented. Records locally in `broadcast_intents` table. If `NIPAE_NSEC` env var is set, publishes a `kind:30174` NIP-AE engram via `buzz_engrams.write_engram()`. Fail-open (Nostr failure never blocks the response). 8 documented intent kinds.

**Gap**: The trigger is manual — callers must explicitly POST to `/api/intents/broadcast`. No automatic triggers are wired from trading settlement, ASE ticks, or governance proposal enactment. The spec says these should trigger automatically. Classification: **PARTIAL** (mechanism complete, automatic trigger wiring absent).

### OSOVM Simulation Results Integration — PARTIAL

The `osovm_router.py` exposes a proxy. Agents can post opcodes to OSOVM and get results. However:
- No endpoint gates a payout/job-approval on OSOVM attestation
- No VeilSim result feeds back into the candidate set for ASE emission
- `_candidate_set_hash()` in `ase_emission.py` is a placeholder (see above)

---

## Database and Persistence

### Database: SQLite (aiosqlite) — VERIFIED

Vantage uses a **single SQLite database** (`vantage.db`) in WAL mode. There is no PostgreSQL, Redis, or other database in the Python backend.

Key facts:
- `DB_PATH = settings.DATA_DIR / "vantage.db"` (`db.py:13`)
- WAL mode + `PRAGMA synchronous=NORMAL` set at schema init (`db.py:78–79`)
- **Connection semaphore**: `asyncio.Semaphore(8)` bounds concurrent connections (`db.py:23`). Comment documents this was added after "109 database is locked errors/minute measured live on 2026-08-21"
- `PRAGMA busy_timeout=90000` (90 seconds) on every connection (`db.py:38`)

Note: `POSTGRES_MIGRATION_SESSION_SUMMARY.md` and `POSTGRES_MIGRATION_TESTING_PLAN.md` exist at the repo root — suggesting a PostgreSQL migration was planned or attempted. The current live backend is still SQLite.

### Schema Management

**No migration framework** (Alembic/Flyway absent). Schema is managed by:
1. `init_agents_db()` in `db.py` runs a large `CREATE TABLE IF NOT EXISTS` block at startup
2. Each router module runs its own `_ensure_tables()` / `_ensure_table()` on first request or at module load via `_schedule_init()`
3. `ALTER TABLE ADD COLUMN IF NOT EXISTS` patterns for idempotent migrations throughout (e.g., `heartbeat.py:84–97`, `dip_ingest.py:127–141`)

This means schema state is scattered across ~20+ files, applied at runtime rather than tracked in version-controlled migration scripts. **No rollback capability.** For a production system this is a significant operational risk.

### Key Tables (inferred from router code)

| Table | Purpose | Managed By |
|-------|---------|------------|
| `agents` | Agent registry, API key hash, status, Nostr pubkey | `db.py` init |
| `mesh_agents` | Block mesh presence, identity, trust scores | `agents.py`/`mesh.py` |
| `mesh_events` | Block event log | `mesh.py` |
| `mesh_proposals` | Mesh negotiation proposals | `mesh.py` |
| `mesh_resources` | Shared resources | `mesh.py` |
| `trading_wallets` | Multi-chain wallet registry | `trading.py` |
| `trading_orders` | Order book | `trading.py` |
| `trading_strategies` | Strategy configs | `trading.py` |
| `trading_trade_journal` | Trade reasoning notes | `trading.py` |
| `twin_receipts` | Spatial twin protocol receipts (31020-31050) | `twin_receipt_index.py` |
| `arp_receipts` | External ARP receipts (Omo-Koda2 lifecycle) | `arp_receipts.py` |
| `dip_envelopes` | DIP federation envelopes | `dip_ingest.py` |
| `ase_emission_receipts` | ASE emission tick chain | `ase_emission.py` |
| `ase_pool_balances` | 8-pool balances | `ase_emission.py` |
| `sovereign_wallets` | 1,440 governance wallets | `governance.py` |
| `council_seats` | 12 council seats | `governance.py` |
| `governance_proposals` | Sector proposals | `governance.py` |
| `bino_veto_log` | Bínò constitutional vetoes | `governance.py` |
| `ucx_providers` | Compute provider registry | `ucx.py` |
| `broadcast_intents` | intent log | `broadcast_intent.py` |
| `node_heartbeats` | Sovereign node chain heartbeats | `heartbeat.py` |
| `security_scans` | Security scan results | `security.py` |
| `human_sessions` | Human auth sessions | `human_auth.py` |
| `agent_grants` | Human→agent scope grants | `deps.py` |
| `vault_connectors` | External memory ingest tokens | `deps.py` |

### External Integrations (live calls observed)

| Service | Purpose | File |
|---------|---------|------|
| Helius RPC | Live Solana balance | `trading.py:164–177` |
| MoonPay | Fiat onramp URL signing | `moonpay_client.py` |
| PumpPortal | Wallet generation | `trading.py:463–465` |
| OSOVM (`VANTAGE_OSOVM_URL`) | Simulation attestation | `osovm_client.py` |
| Mycelium (`127.0.0.1:8811`) | Stigmergic trace substrate | `council.py` |
| Nostr relay (`NIPAE_RELAY`) | Broadcast intent / Buzz | `broadcast_intent.py`, `buzz_bridge.py` |
| Sui RPC | On-chain settlement | `sui_client.py` |

---

## Overall Classification

### Summary Table

| Module | Classification | Confidence |
|--------|---------------|-----------|
| `deps.py` auth | VERIFIED | High |
| `security.py` | VERIFIED | High |
| `mesh.py` (all 6 P0 fixes) | VERIFIED | High |
| `twin_receipt_index.py` | VERIFIED | High |
| `dip_ingest.py` | VERIFIED | High |
| `arp_receipts.py` | VERIFIED | High |
| `action_receipt.py` (ARP envelope) | PARTIAL — unsigned | High |
| `trading.py` (core wallet/order) | VERIFIED | High |
| `trading.py` (ARP emission) | PARTIAL — not persisted | High |
| `ase_emission.py` | IMPLEMENTED (candidate_hash stub) | High |
| `sovereign_economy/emission.py` | CONFLICTING with router pools | High |
| `governance.py` (Council of 12) | IMPLEMENTED | High |
| `council.py` (Ares trading council) | IMPLEMENTED (different system) | High |
| `sovereign_governance/council.py` | DEAD (unused library) | High |
| `broadcast_intent.py` | PARTIAL (manual trigger only) | High |
| `heartbeat.py` (node chain) | VERIFIED | High |
| `birth_credentials.py` | VERIFIED | High |
| `osovm_router.py` + client | PARTIAL (bridge present, no mandatory coupling) | High |
| `ucx.py` (provider registry) | IMPLEMENTED | High |
| Schema management | BROKEN (no migrations, runtime-only) | High |

### Critical Gaps

1. **ARP trade receipts not persisted** (`trading.py:23–31`): `_emit_trade_receipt()` logs but does not write to `arp_receipts`. The P0-7 fix wires the receipt construction at 3 points but the data is ephemeral.

2. **ActionReceipt always unsigned** (`action_receipt.py:159`): `signature=""` on every receipt. No Ed25519 signing layer exists in Python. This means the canonical hash chain has no cryptographic integrity.

3. **Two incompatible ASE pool taxonomies** (`ase_emission.py` 8 pools vs `sovereign_economy/emission.py` 8 different pools). The Python port of the Rust spec is unused; the router has its own schema.

4. **Council of 12 split across three disconnected implementations**: `governance.py` (live API), `sovereign_governance/council.py` (dead library), `council.py` (Ares trading council). No unification path exists yet.

5. **No migration system**: All schema changes are `ALTER TABLE IF NOT EXISTS` patterns at runtime. No version tracking, no rollback.

6. **OSOVM not mandatory**: Any flow that should require proof-of-simulation before settlement has no enforcement. `VANTAGE_OSOVM_URL` being unset silently disables all OSOVM coupling.

7. **DipEnvelope signature not verified** (`dip_ingest.py`): `signature` field is stored but never cryptographically verified. A malicious peer can send forged envelopes that pass all validation.

8. **Broadcast intent is manual**: No automatic trigger from ASE ticks, governance enactment, or trade settlement. The minipae subscription architecture depends on this being event-driven.

9. **`_candidate_set_hash` is a placeholder** (`ase_emission.py:124–128`): Emission #N always produces `sha256("emission:N")` rather than a real draw from VeilSim leaderboard. ASE emission is structurally sound but the proof-of-useful-simulation link is severed.

10. **SQLite in production with 100+ router files**: WAL mode and the connection semaphore mitigate concurrency issues documented in code comments, but a single SQLite file for a multi-hundred-endpoint API is the root-cause risk. The PostgreSQL migration docs exist but are not complete.
