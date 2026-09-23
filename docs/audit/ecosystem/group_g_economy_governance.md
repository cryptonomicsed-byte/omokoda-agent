# Group G: Economy and Governance Audit

**Date:** 2026-09-22  
**Auditor:** Subagent (Claude Sonnet 4.6)  
**Taxonomy:** VERIFIED · IMPLEMENTED · PARTIAL · STUB · SPEC_ONLY · DEAD · DUPLICATE · REDUNDANT · CONFLICTING · OBSOLETE · BROKEN · UNKNOWN

---

## Twelve-thrones (`~/Twelve-thrones/`)

### Overview — IMPLEMENTED (standalone), PARTIAL (ecosystem integration)

A TypeScript consensus engine that queries up to 10 LLM models in parallel, detects
disagreement, and archives results to Arweave + Sui. The core ritual is real and
executable; the on-chain pipeline is wired but requires env vars and a deployed Move
package to run end-to-end.

### Model SDKs and Registry

All model calls use raw `fetch()` against vendor REST APIs — **no SDK packages** are
imported. The seven provider keys are read from env vars
(`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`, `XAI_API_KEY`,
`GOOGLE_API_KEY`, `GROQ_API_KEY`, `MISTRAL_API_KEY`).

**`package.json`** dependencies: `@mysten/sui.js ^0.48.0`, `arweave ^1.14.0`,
`chromadb ^1.4.0`, `dotenv ^16.0.0`, `express ^5.2.1`.

**Registered thrones** (`ritual-router-v8-ADVANCED.ts:60-71`, 10 total — not 12):

| ID | Model | Provider | Weight |
|----|-------|----------|--------|
| 1  | grok-beta | XAI | 0.85 |
| 2  | claude-sonnet-4-20250514 | Anthropic | 0.98 |
| 3  | claude-opus-4-20250514 | Anthropic | 0.98 |
| 4  | deepseek-chat (R1) | DeepSeek | 0.88 |
| 5  | deepseek-chat (V3) | DeepSeek | 0.88 |
| 6  | gpt-4o | OpenAI | 0.96 |
| 7  | o1-mini | OpenAI | 0.96 |
| 8  | gemini-1.5-pro | Google | 0.87 |
| 9  | llama-3.1-70b-versatile | Groq | 0.82 |
| 10 | mistral-large-latest | Mistral | 0.80 |

**Finding:** The repo is named "Twelve Thrones" but only 10 models are defined.
Throne IDs 11 and 12 are absent. No placeholder or TODO exists for the missing two.
Status: **BROKEN** (conceptual mismatch — the name implies 12).

### Consensus Execution — IMPLEMENTED

`igniteTheEternalCircle()` at `ritual-router-v8-ADVANCED.ts:483` fires all 10 throne
queries simultaneously via `Promise.all(queries)` (`line 492`). Execution is fully
parallel. No timeout guard per-model; `ERROR:` prefix handling exists if a fetch fails
(`parseResponse` line 285).

### Disagreement Detection — IMPLEMENTED

`analyzeDisagreement()` (`line 312`) uses a simple vote-count ratio:

- `ratio >= 0.9` → "unanimous", `detected = false`
- `ratio >= 0.7` → "strong", `detected = true`
- `ratio >= 0.5` → "moderate", `detected = true`
- `ratio < 0.5`  → "severe", `detected = true`

The disagreement threshold is raw head-count, not weighted. The weighted consensus
(`weightedYes`/`weightedNo`) is computed separately but does not feed back into
disagreement severity. **Gap:** weighting and severity are computed on different inputs.

### Arweave Upload — IMPLEMENTED (conditional)

`src/arweave.ts:archiveConsensusToArweave()` uses `arweave-js` to create, sign, and
submit a transaction to `arweave.net` (`lines 6-62`). Requires an Arweave wallet JSON
at `ARWEAVE_WALLET_PATH` (`line 27`). A missing wallet throws immediately (`line 29`).
The ADVANCED script stores to ChromaDB locally on success but does **not** call
`archiveConsensusToArweave` in the main flow — archival is only wired in `npm run archive`
(`package.json:12`), a separate script. The main consensus run does NOT auto-archive.
Status: **PARTIAL** (code exists, not in the main execution path).

### Sui Minting — IMPLEMENTED (conditional)

`src/sui.ts` has `recordConsensusOnSui()` and `mintEpistemicNFT()` using
`@mysten/sui.js` (`lines 10-66`). Both call `moveCall` targeting
`${SUI_PACKAGE_ID}::consensus_ledger::record_consensus` and
`${SUI_PACKAGE_ID}::epistemic_nft::mint_epistemic_map`. The package ID defaults to
`"0x0"` if `SUI_PACKAGE_ID` is unset (`line 13`, `line 46`) — meaning all Sui calls
will silently target the zero address and fail without an error visible to the user.
Status: **PARTIAL** (code real, defaults broken).

### Move Contracts — IMPLEMENTED

`contracts/sources/consensus_ledger.move` and `epistemic_nft.move` are complete and
correct Sui Move modules:
- `consensus_ledger::record_consensus` shares a `ConsensusRecord` object on-chain.
- `epistemic_nft::mint_epistemic_map` mints an `EpistemicNFT` with Display metadata.

`contracts/Move.toml` targets the `framework/mainnet` revision. The package has not
been deployed (no `deployment/` artefacts with object IDs observed, only shell scripts
`deployment/testnet.sh` and `deployment/mainnet.sh` referenced in `package.json`).
Status: **IMPLEMENTED** (contracts), **STUB** (deployment).

### BSL License

`LICENSE` file is a genuine Business Source License 1.1, with change date 2028-03-22
and change license MIT. **No runtime enforcement** — the license is not checked in code.
Status: **SPEC_ONLY** (license file exists, zero runtime gates).

### Output Format

The main engine produces a `ConsensusResult` JSON object (`types.ts:1`):
- Stored locally in ChromaDB (`ritual-router-v8-ADVANCED.ts:544`)
- Printed to stdout in a formatted table
- Intended to be archived to Arweave (separate script) and then recorded as a Sui
  `ConsensusRecord` + `EpistemicNFT` (separate script)

The full pipeline (consensus → Arweave → Sui → NFT) requires three separate script
invocations. There is no single end-to-end runner.

### Vantage Integration — NONE

`grep -r "vantage\|VANTAGE"` found only two results in markdown documentation files
(`genesis/TITLES_REFACTOR.md`, `genesis/RESPONSE_FORMAT_GUIDE.md`), both about the
word "X is buzzing" (unrelated). **No integration with the Vantage API, ARP receipts,
or the broader sovereign ecosystem.**
Status: **DEAD** (zero wiring).

### Standalone Execution

`npm start` runs `npx ts-node server-final.ts` (639 lines, an Express server with
endpoints). `npm run genesis` runs the ADVANCED consensus script directly.
The repo can run standalone with valid API keys and optional env vars for Arweave/Sui.

---

## Portent (`~/Portent/`)

### Overview — IMPLEMENTED (NOT spec-only as described in memory)

Memory said "zero code." **This is incorrect.** Portent has a full Python implementation
across 5 modules totaling approximately 1,400 lines of runnable code:

| File | Lines | Status |
|------|-------|--------|
| `sdk/portent_sdk.py` | ~495 | IMPLEMENTED |
| `relay/portent_relay.py` | ~766 | IMPLEMENTED |
| `agent/portent_agent.py` | ~263 | IMPLEMENTED |
| `contracts/portent_program.py` | ~552 | IMPLEMENTED |
| `dashboard/dashboard.py` | >200 | IMPLEMENTED |

### Nostr Event Kinds

Defined in `contracts/portent_program.py:49-61`:

| Kind | Name |
|------|------|
| 30007 | `KIND_CROSS_REFERENCE` — Nostr↔Solana cross-reference |
| 30009 | `KIND_ATTESTATION` — oracle/cross-check work |
| 30010 | `KIND_REPUTATION_SNAPSHOT` — agent reputation |
| 30011 | `KIND_STAKE_CLAIM` — staking positions |
| 30012 | `KIND_GOVERNANCE` — proposals and votes |
| 30015 | `KIND_PREDICTION_POST` — staked predictions |
| 30016 | `KIND_RESOLUTION` — market resolution |
| 30017 | `KIND_DISPUTE` — freeze a market |

**Finding:** Portent uses kinds 30007–30017, which **overlap with Synapse's** 30000–30006
range. No conflict on the exact numbers used, but both repos are publishing custom kinds
into the same NIP-33 parameterized-replaceable space on the same relay infrastructure.
No cross-repo coordination visible.

### Relay — IMPLEMENTED

`relay/portent_relay.py` is a complete SQLite-backed HTTP relay (stdlib only, no deps):
- `ThreadingHTTPServer` on port 8899 (`PORTENT_RELAY_PORT`)
- NIP-01 canonical event ID validation (`validate_event` line 237)
- NIP-33 replaceable dedup (`_store_event` line 272)
- Full settlement engine: fee split → burn / buyback reserve / staker pool (`_settle_market` line 501)
- Expiry burns (`_expire_stale` line 607)
- Milestone burns at 50K/100K/500K/1M resolved events (`MILESTONES` line 82)
- Endpoints: `/event`, `/stats`, `/leaderboard`, `/predictions`, `/reputation`, `/resolutions`, `/governance`, `/oracles`, `/events`, `/health`

**Signature verification is stubbed** (`relay.py:252-255`): the id check (canonical sha256)
is enforced, but the secp256k1 Schnorr sig is NOT verified. Comment says: "Signature
verification is stubbed in the SDK (secp256k1 lib optional); the wire format is NIP-01
and the id check binds content." This means any event with a valid ID but forged
signature is accepted.
Status: **PARTIAL** (cryptographic enforcement absent).

### Signer — PARTIAL

`sdk/portent_sdk.py:Signer` tries `coincurve` for real secp256k1 signing; falls back to
a deterministic stub (`_impl = "stub"`) that emits `"00" * 64 + " [UNSIGNED_STUB: coincurve not installed]"` (`line 172`). The event ID computation is always correct; only the sig is fake.
`coincurve` is not in `requirements.txt` / package deps — it is an optional import.

### Smart Contract — Solana ABI Spec (Python dataclasses, NOT Move/Rust)

`contracts/portent_program.py` defines Solana account layouts as Python dataclasses with
`struct.pack/unpack` serialization. This is an **ABI specification**, not a deployed
on-chain program. There is no Anchor IDL, no Rust program, and no Sui Move contract.
The module generates pump.fun launch params and Nostr↔Solana cross-reference events but
does not deploy anything.
Status: **SPEC_ONLY** (on-chain component).

### Oracle Agent Integration — IMPLEMENTED (simulation mode)

`agent/portent_agent.py` is a fully runnable autonomous agent (`python3 agent/portent_agent.py`).
It connects to the relay at `PORTENT_RELAY` (default: `http://127.0.0.1:8899`) and:
1. Resolves overdue markets (oracle duty) using a deterministic pseudo-signal
2. Cross-checks recent resolutions
3. Publishes staked predictions (momentum or contrarian)
4. Refreshes reputation snapshots

The signal function is deterministic pseudo-random (sha256-based), not a real oracle.
Status: **PARTIAL** — runnable loop, not a real oracle signal.

### Vantage Integration — PARTIAL (one-way, fire-and-forget)

`relay/portent_relay.py:_notify_vantage_resolution()` (`line 429`) fires a
`POST /api/intents/broadcast` to `VANTAGE_URL` (default `http://127.0.0.1:8000`) in a
daemon thread when a market resolves. It is explicitly marked **Gap #43** in a code
comment at `line 413`. Fail-open: exceptions are suppressed. No reverse wiring (Vantage
events do not feed into Portent).
Status: **PARTIAL** (fire-and-forget, no bidirectional binding).

### Dashboard — IMPLEMENTED

`dashboard/dashboard.py` is a stdlib HTTP server (port 8898) serving a dark-aurora
HTML/CSS dashboard that reads the same SQLite DB as the relay. Stats, leaderboard,
predictions, and governance views are implemented.

### Tokenomics — IMPLEMENTED (spec)

`tokenomics.json` defines 1B PORT supply, pump.fun launch, 6 distribution buckets, and
staking tiers. The economics are mirrored faithfully in the Python modules but no
on-chain token has been deployed.

---

## Synapse (`~/Synapse/`)

### Overview — IMPLEMENTED (reference dashboard), PARTIAL (relay wiring)

Synapse is a Next.js 16 + TypeScript application, not a simple library. It contains a
running reference dashboard with a real-time simulation of agent skill/memory/
negotiation/trust activity, plus the NIP-30 specification.

### Event Kinds (30000–30006) — IMPLEMENTED (spec + client simulation)

Defined in `NIPS/30-synapse.md` and implemented in `src/lib/synapse/store.ts:39-47`:

| Kind | Label | NIP description |
|------|-------|-----------------|
| 30000 | SKILL_CARD | Signed agent skill declaration (SLA, cost-in-trust, schema) |
| 30001 | MEMORY_SHARD | Encrypted knowledge artifact with access policy |
| 30002 | ENDORSEMENT | Signed weighted trust edge (not replaceable) |
| 30003 | A2A_NEGOTIATION | State-machine contract (PROPOSED→COUNTERED→ACCEPTED→EXECUTING→SETTLED) |
| 30004 | COMPOSITION | DAG edge: skill composes others |
| 30005 | VERDICT | Orchestrator/critic verdict (WOWED or DEMANDS_ITERATION) |
| 30006 | INTENT | Agent-published intent, bid-able |

The store (`store.ts:260-611`) drives a live simulation that fires all 7 kinds via
`publishEvent()`, which calls `buildAndSign()` with real BIP-340 keys.

### BIP-340 Signing — IMPLEMENTED (with mock fallback)

`src/lib/synapse/signing.ts` implements:
- `deriveKeypair(seed)` — HKDF-SHA256 → 32-byte secp256k1 private key → BIP-340 x-only pubkey via `nostr-tools v2 getPublicKey` (`line 133`)
- `buildAndSign()` — calls `nostr-tools finalizeEvent` for real Schnorr signature (`line 167`)
- `verifyEvent()` — calls `nostr-tools verifyEvent` (`line 189`)
- Fallback: if `nostr-tools` unavailable, emits a deterministic mock sig and logs a warning (`line 176-179`)
- `NOSTR_PRIVKEY` env var override for the primary agent (`line 57-88`)

Per `NIPS/30-synapse.md:9` (Changelog):
> **v0.2.0** (2026-09-14): real BIP-340 Schnorr signing implemented (gap #46).
> Replaced LCG `mockKeypair()` / mock `sig` with HKDF-SHA256 key derivation +
> `nostr-tools v2 finalizeEvent`.

**Memory said "mock signatures only" — this is outdated.** As of v0.2.0 the signing is
real when `nostr-tools` is installed (which it is: `package.json:64` lists
`"nostr-tools": "^2.25.2"` in dependencies).
Status: **IMPLEMENTED** (real BIP-340 with graceful mock fallback).

### Buzz-OG Relay Wiring — PARTIAL (design mapping, not live connection)

`grep` found references to `buzz` throughout `src/components/synapse/buzz-interop.tsx`
and `src/lib/synapse/seed.ts`. The `buzz-interop` component shows a table mapping each
Synapse kind to its Buzz relay primitive. However, there is **no HTTP connection to a
running Buzz relay**. All events are published into the local in-memory Zustand store
(`store.ts:241-256`), not sent over the wire to any relay.

There is no WebSocket relay client, no Nostr relay URL in the config, and no
`relay.extension.wasm-filter` installed. The dashboard simulates the relay interaction
in-browser only.
Status: **PARTIAL** (architectural design complete, wire transport absent).

### Reference Dashboard — IMPLEMENTED

The Next.js app includes:
- `src/app/page.tsx` — main dashboard (104 lines)
- `src/app/api/route.ts` — API route
- `src/components/synapse/buzz-interop.tsx` — kind↔Buzz mapping table
- `src/lib/synapse/seed.ts`, `store.ts`, `signing.ts`, `types.ts` — full state machine

The simulation auto-advances every 1600ms (`store.ts:215`), publishing all 7 event kinds,
advancing negotiation state machines, maintaining trust edge graphs, and running the
QUEUED→PLANNING→FAN_OUT→CRITIQUE→ITERATING→COMPLETE task lifecycle.

### Move Pagerank Module — SPEC_ONLY

`NIPS/30-synapse.md:§5` contains a Move code block for `synapse::trust::pagerank` but
explicitly states: "This module is **planned** for v0.2. The v0.1 demo computes trust
scores client-side from the static seed." No Move file exists in the repo.
Status: **SPEC_ONLY**.

### Database — IMPLEMENTED

`prisma/` directory exists with `db.ts` at `src/lib/db.ts`. Prisma client is a
dependency (`package.json:63`). The schema and migrations are present (`db:push`,
`db:generate`, `db:migrate` scripts).

### ARP / Receipt Integration — NONE

No grep hits for "vantage", "ARP", "receipt", or "sovereign" in the Synapse source tree.
The dashboard is fully self-contained; no integration with the broader sovereign stack.
Status: **DEAD** (zero wiring to ecosystem receipts or identity).

---

## Blocksim

### Status — ABSENT

```
ls ~/Blocksim/ 2>&1
→ ls: cannot access '.../Blocksim/': No such file or directory
```

Blocksim does not exist on disk. It was described in ecosystem documentation as a
"PRODUCTION API" for Proof-of-Simulation chain — the backend that ScarabSwarm
(`~/ScarabSwarm/`) was intended to submit `SimReceipt` objects to.

### Impact Analysis

The ScarabSwarm→Blocksim integration path is broken at the destination:

1. `~/ScarabSwarm/` compiles clean and generates `SimReceipt` + `ProofOfSimulation`
   objects (per the `project_scarabswarm.md` memory entry).
2. Those receipts have nowhere to land. The `blocksim` submission endpoint does not
   exist.
3. The "Proof-of-Useful-Simulation" claim in the Vantage Sovereign Architecture is
   partially implemented on the emitter side (ScarabSwarm) but has zero on-chain or
   persistent receiver.
4. The ASE Emission system that was supposed to gate rewards on verified simulation
   proofs cannot verify them — the verifier is missing.

**Risk tier:** HIGH. ScarabSwarm simulation proofs are economically meaningless without
a receiver that validates and registers them. The ToC (Token-of-Compute) dopamine decay
system in `~/UCX/` also depends on verified simulation work as a burn trigger.

---

## Economic Completeness Assessment

### What percentage of the described economy actually executes today?

**Estimate: ~18% end-to-end.**

| Layer | Described capability | Reality | Status |
|-------|---------------------|---------|--------|
| Prediction markets (PORT) | Full staked prediction economy on Solana | Relay + agent + dashboard run. No on-chain Solana program. Sig verification stub. | PARTIAL — ~40% |
| Multi-model consensus (Twelve-thrones) | 12 models → Arweave → Sui → NFT | 10 models query in parallel. Arweave/Sui wiring exists but separated into manual scripts. No auto-pipeline. | PARTIAL — ~55% |
| Simulation proofs (ScarabSwarm→Blocksim) | PoS chain anchoring GPU work receipts | ScarabSwarm emitter exists. Blocksim receiver ABSENT. | BROKEN — ~10% |
| Skill/memory/trust layer (Synapse) | NIP-30 agents on Buzz relay | All 7 kinds defined, BIP-340 signing real, simulation live. No relay transport. | PARTIAL — ~60% |
| ASE emission gating | Zàngbétò receipts gate every emission tick | Emission tick endpoint exists in Vantage. Receipt validation path references Blocksim and ScarabSwarm outputs which don't reach any validator. | BROKEN — ~15% |
| Governance (PORT) | On-chain council + staggered terms | Python governance event handlers exist. No deployed contract. Voting logic in relay only. | PARTIAL — ~25% |
| Vantage ↔ Portent bridge | Oracle resolution triggers downstream agents | Fire-and-forget POST (Gap #43). One direction only. | STUB — ~10% |
| Twelve-thrones ↔ ecosystem | Epistemic consensus feeds sovereign identity | Zero wiring. | DEAD — 0% |
| Synapse ↔ ecosystem | Agent skill/trust feeds sovereign reputation | Zero wiring. | DEAD — 0% |

### Critical Gaps Blocking Economic Execution

1. **Blocksim absent** — entire Proof-of-Simulation incentive chain is broken.
2. **Portent on-chain component** — only ABI spec exists; no deployed Solana program means
   PORT token does not exist and no prediction can be settled on-chain.
3. **Twelve-thrones pipeline not automated** — Arweave archival and Sui recording are
   separate manual scripts, not wired into the consensus run.
4. **Synapse relay transport absent** — all events stay in-browser; no agent-to-agent
   communication occurs over any wire.
5. **Signature verification disabled** — both Portent relay and Synapse SDK have
   fallback stubs that accept unsigned or mock-signed events; a malicious actor could
   inject events into Portent markets or Synapse trust graphs without valid keys.
6. **Cross-ecosystem receipts absent** — none of the three repos produces or consumes
   ARP `ActionReceipt` envelopes; their economic activity is invisible to the
   Zàngbétò receipt-gated emission system.
