# Group D: Identity Layer Audit

**Audited:** 2026-09-22  
**Auditor:** Claude Sonnet 4.6  
**Taxonomy:** VERIFIED · IMPLEMENTED · PARTIAL · STUB · SPEC_ONLY · DEAD · DUPLICATE · REDUNDANT · CONFLICTING · OBSOLETE · BROKEN · UNKNOWN

---

## GIX (`~/GIX/`)

### Structure

```
~/GIX/
├── Cargo.toml         (workspace: gix-types, gix-core)
├── MANIFEST.toml
├── crates/
│   ├── gix-types/src/lib.rs    (1,305 lines — ALL primitives here)
│   └── gix-core/src/
│       ├── lib.rs
│       ├── graph.rs
│       ├── index.rs
│       ├── memory_authority.rs
│       ├── projection.rs
│       └── store.rs
└── target/
```

### Status: VERIFIED

### Exported Types (gix-types `lib.rs`)

| Symbol | Status |
|---|---|
| `content_hash(text) -> [u8;32]` | IMPLEMENTED — SHA-256, line 25 |
| `glyph_fold(digest) -> char` | IMPLEMENTED — GIX-FOLD-v1 full range logic, line 31 |
| `odu_link(digest) -> (u8, u16)` | IMPLEMENTED — line 48 |
| `GlyphNode`, `GlyphEdge` | IMPLEMENTED — structs with `from_chunk`, line 56–91 |
| `GixKind` enum | IMPLEMENTED — Memory/MemoryFold/Receipt/Simulation/Physical/Governance/Custom |
| `Gix1Entry` | IMPLEMENTED — line 108 |
| `GIX1_EMPTY_ROOT` | IMPLEMENTED — const line 136 |
| `gix1_merkle_root(ids) -> String` | IMPLEMENTED — full pairwise SHA-256 reduction, line 141 |
| `merkle_root(ids) -> String` | IMPLEMENTED — alias of above, line 177 |
| `gix1_audit(stored, ids) -> Result` | IMPLEMENTED — line 183 |
| `Gix1` wire envelope | IMPLEMENTED — version/kind/namespace/canonical_id/glyph/routing/integrity, line 297 |
| `GixNamespace`, `RoutingHints`, `IntegrityMeta` | IMPLEMENTED |
| `gix_fold_v1(inputs) -> [u8;32]` | IMPLEMENTED — line 378 |
| `GixFold`, `FoldAlgorithm`, `fold_sources_merkle_root` | IMPLEMENTED — Phase 8D fields, line 420 |
| `GixMemoryRef`, `GixMemoryTier` | IMPLEMENTED — line 601 |
| `GixVisibility`, `GixProvenance`, `GixMinipaeLocator` | IMPLEMENTED — Phase 8C bridge types, line 660 |
| `gix_kdf_v1(id, domain, owner, ctx) -> [u8;32]` | IMPLEMENTED — HKDF-SHA256, line 794 |
| `GixDomain` | IMPLEMENTED — 6 domains: Memory/Mesh/Receipt/AgentKey/Encryption/Duress |
| `HashDomain`, `MemoryState`, `MemoryKind` | IMPLEMENTED — Phase 9A, line 897 |
| `GixConflict`, `ConflictResolution`, `ConflictKind` | IMPLEMENTED — Phase 9F, line 856 |

### Exported Types (gix-core `lib.rs`)

| Symbol | Status |
|---|---|
| `GlyphGraph` (as `GixGraph`) | IMPLEMENTED |
| `Gix1Index` | IMPLEMENTED — full Merkle-auditable index with `save`/`load`/`audit`/`by_kind`/`by_namespace` |
| `MemoryAuthority`, `MemoryAuthorityError` | IMPLEMENTED |
| `verify_hash_domain_isolation`, `verify_locator_coherence` | IMPLEMENTED |
| `GixAgentProjection`, `StoreProjection` | IMPLEMENTED |
| `CanonicalObjectStore`, `GixSnapshotMeta` | IMPLEMENTED |
| `load_store_from_files`, `save_store_to_files` | IMPLEMENTED |

### Cross-Language Conformance Vectors

Five frozen GIX-FOLD-v1 vectors are present in BOTH `gix-types/src/lib.rs:219–226` AND `If-Script/src/glyph/mod.rs:156–163`. They match exactly, confirming cross-language determinism.

### Compilation

`cargo check` passes with **0 errors**. One warning: unused import `GixVisibility` in `gix-core/src/memory_authority.rs:8`.

```
Finished `dev` profile in 9.76s
```

### Integration with Omo-Koda2

**VERIFIED ACTUAL IMPORT.** `~/Omo-Koda2/omokoda-core/Cargo.toml` contains:

```toml
gix-core  = { path = "../../GIX/crates/gix-core" }
gix-types = { path = "../../GIX/crates/gix-types" }
```

`gix_bridge.rs` (`~/Omo-Koda2/omokoda-core/src/memory/gix_bridge.rs`) re-exports and actively calls:
- `gix_core::{Gix1Index, GlyphGraph, gix1_audit, gix1_merkle_root, GIX1_EMPTY_ROOT, ...}`
- `gix_types::{GixMemoryRef, GixMemoryTier, GixNamespace, GixMinipaeLocator, ...}`

Functions `memory_merkle_root()`, `audit_memory_root()`, `build_gix1_index()`, `project_gix()` are all implemented and call through to the crate. This is not reference-only — it is active runtime use.

### Note on glyph_memory.rs

`glyph_memory.rs` uses a separate `larql_glyph` dep (pinned git rev). `gix_bridge.rs` explicitly documents this split at line 7–11: "This module pulls those from the local gix-core path crate which is the canonical sovereign implementation." Two parallel graph systems exist for the same data; not a conflict, but a documented migration in progress.

---

## If-Script (`~/If-Script/`)

### Structure

```
~/If-Script/src/
├── lib.rs
├── glyph/mod.rs        GIX-FOLD-v1 + GlyphResidue + cast_with_memory
├── vm.rs               IfaVM
├── entropy.rs          CowrieOracle
├── odu/                256 Odù + ActionVessel
├── odu_ifa/            256 traditional Yoruba Odù
├── calabash/           65,536 composed scaling
├── compiler/           IfaParser, ParsedInvocation
├── cosmogram/          CosmogramEngine (tiered access)
├── larql/              LarqlQuery (DESCRIBE/VERIFY/PREPARE)
├── nostr/              NostrGateway, NostrIdentity, CastReceipt
├── seven_bridge.rs     USF-7 ↔ ActionVessel mapping
├── manifesto/          Clause, Manifesto
├── receipt/
├── ritual_codex/
├── soul/
├── hermetic/
├── archetype/
├── zangbeto/
├── field/
├── field_divination.rs (native only)
└── ...
```

### Status: VERIFIED (core VM + glyph primitives); PARTIAL (compiler)

### Glyph Primitives (`src/glyph/mod.rs`)

| Symbol | Status |
|---|---|
| `content_hash(text) -> [u8;32]` | IMPLEMENTED — identical algorithm to gix-types, line 33 |
| `glyph_fold(digest) -> char` | IMPLEMENTED — GIX-FOLD-v1, identical to gix-types, line 40 |
| `odu_link(digest) -> (u8, u16)` | IMPLEMENTED — identical, line 57 |
| `GlyphResidue` struct | IMPLEMENTED — with `from_chunk`, `verify()`, line 63 |
| `cast_with_memory(vm, residues, exp)` | IMPLEMENTED — memory-augmented cast, full tier gating, line 124 |
| `residue_entropy(residues) -> [u8;32]` | IMPLEMENTED — deterministic order-independent, line 93 |
| `MemoryCast` struct | IMPLEMENTED — line 106 |

**DUPLICATE NOTE:** `content_hash`, `glyph_fold`, `odu_link` are re-implemented in `If-Script/src/glyph/mod.rs` with identical logic to `gix-types/src/lib.rs`. Both maintain frozen cross-language conformance vectors that agree. This is deliberate — If-Script keeps its own copy to remain dep-free of gix-types — but it is a documented duplicate. Status: REDUNDANT (intentional).

### CowrieOracle (`src/entropy.rs`)

Status: **IMPLEMENTED**

- NIST Randomness Beacon: HTTP fetch wired at `src/entropy.rs:24–26` (`reqwest::blocking::Client`), compiled in on native targets, gated out on WASM (`#[cfg(not(target_arch = "wasm32"))]`).
- ChaCha20: full pure-Rust RFC 8439 implementation via `rand_chacha::ChaCha20Rng`.
- HKDF: `hkdf::Hkdf` wired as salt source mixing beacon pulse with ritual seed.
- Fallback path: wall-clock nanoseconds + process id as fallback salt. Documented explicitly.
- This is NOT a stub. The beacon URL, HKDF pipeline, and ChaCha20 keystream are all present.

### VM (`src/vm.rs`)

Status: **IMPLEMENTED** — `IfaVM` struct with `cast_odu()`, `with_intent()`, `CastResult`. Used by `cast_with_memory`.

### Compiler (`src/compiler/`)

Status: **PARTIAL** — `IfaParser`, `compile_program`, `compile_invocations`, `ParsedInvocation` exported. Parser exists. Full expression parsing present. Whether `.ifa`/`.0` scripts execute end-to-end requires a separate integration test that was not run.

### Digital Calabash (`src/calabash/`)

Status: **IMPLEMENTED** — `cast_scaled`, `compose_id`, `decompose`, `resolve`, `AccessDenied`, `ComposedOdu`, `AgentExperience`, `ConsensusLedger` all exported and used in `cast_with_memory`.

### Nostr Gateway (`src/nostr/`)

Status: **IMPLEMENTED** — `NostrGateway`, `NostrIdentity`, `CastReceipt`, `RitualClaim`, `Relay` all exported.

### Test Results

```
test result: ok. 9 passed; 0 failed — main lib tests (including
  the_full_authenticated_publish_path_works, 
  the_gateway_publishes_to_every_registered_relay)
test result: ok. 3 passed; 0 failed — stack.rs integration tests
test result: ok. 0 doc-tests
```

All tests pass. 12 total.

### Integration with Omo-Koda2

**VERIFIED ACTUAL IMPORT.** `~/Omo-Koda2/omokoda-core/Cargo.toml`:

```toml
ifascript = { git = "https://github.com/cryptonomicsed-byte/If-Script", rev = "5dbee51fb2c637cda71e6702bada54b2c2dfc2c9" }
```

Active call sites:
- `src/tools/if_script_tool.rs` — calls `ifascript::field_divination::FieldDiviner::default()`
- `src/interpreter.rs` — If-Script used in divination
- `src/identity/odu.rs` — Odù identity
- `src/memory/larql_query.rs` — LARQL queries
- `src/seven/state.rs` — USF-7 bridge
- `src/genesis/soul.rs` — soul derivation
- `src/ifscript_gate.rs` — hermetic gate

**CONFLICT / RISK:** The Cargo dep pins a remote git rev (`5dbee51`) not a local path. If `~/If-Script/` is ahead of that rev, Omo-Koda2 compiles against the **older** remote version, not the local one. The local `~/If-Script/` is the source of truth but is NOT what Omo-Koda2 currently links against. This is a latent divergence risk.

---

## minipae (`~/minipae/`)

### Structure

```
~/minipae/
├── minipae.py          (core: NIP-44 v2, NIP-AE kind 30174, slug grammar)
├── derive.py           (BIP-32 key derivation)
├── cap_bridge.py
├── ga_bridge.py
├── hermes_adapter.py
├── omokoda_adapter.py
├── daemon_adapter.py
├── merged_view.py
├── cross_relay_read.py
├── runtime_registry.json
├── NAMESPACES.md
├── ORCHESTRATION.md
├── PLAN.md
├── tests/              (test_minipae.py, test_ga_bridge.py, etc.)
├── memory/
├── skills/
├── integrations/
├── docker/
└── commonly/
```

### Status: IMPLEMENTED (NIP-44 v2, slug grammar, kind:30174); PARTIAL (relay calls — network-dependent)

### NIP-44 v2 Implementation

Status: **IMPLEMENTED** — pure Python, no external crypto lib wrapper.

Full implementation present in `minipae.py`:
- `conversation_key()`: ECDH over secp256k1 via pure-Python scalar mult + HKDF-extract (`b"nip44-v2"` salt), line 192
- `_message_keys()`: HKDF-expand 76 bytes → ChaCha20 key (32) + nonce (12) + HMAC key (32), line 203
- `nip44_encrypt()` / `nip44_decrypt()`: full ChaCha20 + HMAC-SHA256 + NIP-44 padding, lines 302–335
- `_calc_padded_len()`: correct power-of-2 chunk padding per spec, line 208
- `_unpad()`: validates declared length against `calc_padded_len`, NOT just trailing-zero check, line 228
- BIP-340 Schnorr signing (`schnorr_sign`, `schnorr_verify`): full spec-compliant, tagged hashes, even-y adjustments, lines 117–153

NIP-44 is **NOT a wrapper** around another lib. It is a from-scratch implementation passing against frozen test vectors (`tests/nip44_vectors.json`).

### Slug Grammar

Status: **IMPLEMENTED** — `validate_slug()` at line 347. Grammar:
- `"core"` — singleton
- `"sys/<key>"` — reserved system namespace
- `"mem/<namespace>/<segments...>"` — agent memory, `[a-z0-9_-]` per segment, ≤ 64 bytes, ≤ 255 total

`normalize_slug_segment()` (line 392): folds Unicode (strips combining marks, casefolds), maps non-grammar chars to dash, byte-truncates to 64.

`build_slug(namespace, *segments)` (line 430): validates output, enforces NAMESPACES.md registry.

### kind:30174 Events

Status: **IMPLEMENTED** — `KIND_AGENT_ENGRAM = 30174` at line 343. `d_tag` computed as `HMAC-SHA256(K_c, "agent-memory/v1/d-tag" || 0x00 || slug)` at line 373. `sign_event()` assembles and Schnorr-signs any kind, line 444.

### Relay Calls

Status: **PARTIAL** — relay URL defaults to `wss://relay.damus.io` (env `NIPAE_RELAY`). Actual WebSocket relay I/O exists in the CLI path (`ls`, `get`, `set`, `rm`) using `asyncio`/`websockets`. Real relay connectivity is **not tested in the unit test suite** — all 64 passing tests are offline (mock or crypto-only). Live relay delivery is untested.

### Test Results

```
64 passed in 105.57s
```

All 64 tests pass. Test coverage is solid for cryptographic correctness; no network integration tests.

### Integration with Omo-Koda2

**PARTIAL WIRING.** minipae is NOT imported as a Python dep from Rust. Instead:

1. **Key derivation is wired**: `~/Omo-Koda2/omokoda-core/src/identity/wallet.rs` implements `derive_minipae_key()` — BIP-32 path `m/44'/30174'/<agent>/<owner>` (hardened), documented to match `minipae/derive.py` exactly.

2. **Vault schema references it**: `~/Omo-Koda2/omokoda-core/src/identity/vault.rs` stores `minipae_private_key_hex: Option<String>` and `minipae_npub: Option<String>`.

3. **GIX bridge defines the locator type**: `GixMinipaeLocator` in `gix-types/src/lib.rs:716` describes the Nostr pointer type for GIX objects published to kind:30174.

4. **GAP — actual write not wired**: No code in Omo-Koda2 actually calls `minipae.py`'s `write()` function or spawns the Python process to publish kind:30174 events. The key is derived and stored, but the memory bus publication step is absent. Status of the birth-write hook: **SPEC_ONLY** within Omo-Koda2.

---

## ip-layer (`~/ip-layer/`)

### Structure

```
~/ip-layer/
├── README.md
├── schemas/
│   ├── ip_root.md          (kind 31900 — IP Root genesis)
│   ├── creation_receipt.md (kind 1901 — Creation Receipt)
│   ├── attestation.md      (kind 1902 — Attestation)
│   └── twin_binding.md     (kind 1903 — Twin Binding)
├── docs/
│   ├── verification-pass-1.md
│   ├── nip-gc-assessment.md
│   └── (other docs)
└── src/
    ├── publisher/sovereign_publisher.rs  (Rust, kind 31020/31030 publishers)
    └── relay/sovereign_relay.rs          (Rust, NIP-01 relay client)
```

### Status: PARTIAL (schemas = VERIFIED; Rust src = PARTIAL but ORPHANED)

### Schemas

Status: **VERIFIED** — Four schemas are complete, detailed, and internally consistent.

| Event Kind | Schema Status |
|---|---|
| `31900` — IP Root | Defined: `d` tag = agent pubkey, `owner`, `soul`, `license`, `dao`, `genesis`. Open Qs noted. |
| `1901` — Creation Receipt | Defined: `ip_root`, `x`, `url`, `m`, `L`/`l`, `split`, `attest`, `osovm_op`, `twin`, `e` tags. Unified with OSOVM RECEIPT opcode. |
| `1902` — Attestation | Defined (schema file present). |
| `1903` — Twin Binding | Defined: `ip_root`, `sim_id`, `twin_kind`, `fidelity`, `osovm_op`. |

The schemas have passed a pre-implementation verification pass (`docs/verification-pass-1.md`): kind-collision check done, OSOVM RECEIPT opcode shape confirmed.

### Rust Source Files

Two Rust files exist in `src/` but **there is no `Cargo.toml` in `~/ip-layer/`**. These files are NOT a compilable crate:

- `src/publisher/sovereign_publisher.rs` — imports `ip_layer::nostr::NostrEvent`, `twin_protocol::CaptureReceipt`, `twin_protocol::SceneReceipt`. Publishes kind-31020/31030 receipts (NOT 31900/1901/1902/1903 from the ip-layer schemas — different kind numbers). Status: **ORPHANED** (no crate manifest, can't compile standalone).
- `src/relay/sovereign_relay.rs` — imports `dip::adapters::NostrAdapter`. Status: **ORPHANED**.

These appear to be component files extracted for a future `ip-layer` Rust crate that was never assembled. They belong to the twin/DIP stack, not the IP provenance schemas.

### Integration with Omo-Koda2

**VERIFIED ACTUAL WIRING for kind 31900 and 1903.** Despite `~/ip-layer/` having no Rust crate, Omo-Koda2 has its own implementation at `~/Omo-Koda2/omokoda-core/src/ip_layer.rs`:

- `publish_ip_root(mnemonic, agent_name)` — publishes kind 31900 on birth, fail-open, uses `nostr_sdk`.
- `publish_twin_binding(mnemonic, sim_id, ...)` — publishes kind 1903 on demand, fail-open.
- `IP_ROOT_KIND: u16 = 31900` and `TWIN_BINDING_KIND: u16 = 1903` defined as constants, documented as the single source of truth.

**GAP — kind 1901 Creation Receipt not wired.** `publish_ip_root` fires at birth. There is no code in Omo-Koda2 that emits kind 1901 (Creation Receipt) after creative actions.

**GAP — kind 1902 Attestation not wired.** Schema only; no Rust publisher exists anywhere.

The `ip_layer.rs` in Omo-Koda2 does NOT depend on `~/ip-layer/` as a crate; it implements the protocol natively using `nostr_sdk`. The repo at `~/ip-layer/` is referenced as a schema source but is NOT a Rust dependency.

---

## Koodu (`~/Koodu/`)

### Structure

```
~/Koodu/
├── btc-time.js          (pure JS, BTC block height → 7-domain cycle)
├── spiral-calendar.js   (JS spiral calendar)
├── glyph-adapter.js
├── mesh-adapter.js
├── nostr-adapter.js
├── nostr-adapter.test.js
├── technosis-adapter.js
├── src/
│   ├── calendar/        spiral_calendar.jl, sacred_time.jl,
│   │                    organism_integration.jl, agent_lifecycle.jl,
│   │                    agent_wallet.jl, toc_enforcement.jl
│   ├── time/            sacred_time.jl, organism_integration.jl
│   ├── bridge/          organism_integration.jl
│   └── agents/
├── data/
├── json/
├── Btc spiral/
└── swibe-skill/
```

### Status: IMPLEMENTED (Julia calendar logic); PARTIAL (JS bridge); ORPHANED (Julia runtime — Julia not available)

### Julia Calendar Logic

Status: **IMPLEMENTED** — `src/calendar/spiral_calendar.jl` defines:
- `Moon` struct (28-day, 13-moon system, Orisha archetypes, star anchors)
- `Year` struct (13 moons × 28 days + void days)
- `Jubilee` / `GreatJubilee` cycle
- `compute_moons`, `find_gate_days`, `generate_year_almanac`, `to_ritual_codex_json`

`src/calendar/organism_integration.jl` defines:
- `RitualEvent` struct — `btc_height`, `spiral_json`, `economic_rules`, `lobe_routing`
- `emit_event(event, endpoint)` — HTTP POST to organism-core
- `subscribe_spiral()`, `LobeContext`, `inject_veil_context`

**Julia is not installed** on this machine (`julia --version` → NOT AVAILABLE). The `.jl` files cannot be executed. Status: logic is **IMPLEMENTED** but **UNRUNNABLE** without Julia.

### JavaScript Bridge

Status: **IMPLEMENTED** — `btc-time.js` is pure JS (no deps), implements `BTCTime` class:
- Block height → 7-domain cycle (`BTC_DOMAIN_CYCLE`, line 36)
- Halving epoch names and alchemy stages
- Genesis block = 780,000 (matches Omo-Koda2's `KOODU_GENESIS_BLOCK = 780_000`)

`spiral-calendar.js`, `glyph-adapter.js`, `mesh-adapter.js`, `nostr-adapter.js` also present. `nostr-adapter.test.js` exists (JS tests not executed).

### Integration with Omo-Koda2

**PARTIAL — Rust re-implementation, not a dep.** Omo-Koda2 does NOT import `~/Koodu/` as a package. Instead, the Koodu algorithm is re-implemented natively in Rust at `~/Omo-Koda2/omokoda-core/src/genesis/koodu_time.rs`:

- `KOODU_GENESIS_BLOCK: u64 = 780_000` — matches `btc-time.js` genesis block
- `BLOCKS_PER_DAY = 144`, `BLOCKS_PER_CYCLE = 1008`, `BLOCKS_PER_EPOCH = 52 cycles`
- `koodu_from_height(height)` → `(epoch, cycle, phase)`
- `fetch_btc_height()` → live HTTP fetch from `blockstream.info` (env override `KOODU_BTC_API`)
- `DefaultKooduProvider` → async trait returning `KooduTimeProof` at birth

This implementation is **tested** (3 unit tests in `koodu_time.rs`), and is called at birth via `genesis/orchestrator.rs` and in `interpreter.rs` (`koodu_from_unix`), and in `seven/state.rs` (`SevenCalendar::from_btc_height`).

**The relationship:** `~/Koodu/` is the canonical spec and reference implementation; `koodu_time.rs` is a Rust port of that spec. The two are not kept in sync automatically — a drift risk exists if Koodu's constants change without updating the Rust port.

---

## Dependency Graph

### What Omo-Koda2 Actually Imports vs. References

| Repo | Import Type | Cargo/Runtime Dep? | Call Sites | Notes |
|---|---|---|---|---|
| **GIX** (`gix-core`, `gix-types`) | Path dep | YES — `path = "../../GIX/crates/..."` | `src/memory/gix_bridge.rs` (active), `src/memory/glyph_memory.rs` (via larql-glyph, separate) | VERIFIED live use |
| **If-Script** | Git dep (pinned rev) | YES — `git = "...", rev = "5dbee51"` | `if_script_tool.rs`, `interpreter.rs`, `identity/odu.rs`, `ifscript_gate.rs`, `seven/state.rs` | VERIFIED live use; **latent divergence risk** if local `~/If-Script/` is ahead of pinned rev |
| **minipae** | No Cargo dep | NO | Key derived in `identity/wallet.rs`; vault stores keys; `GixMinipaeLocator` defined in `gix-types` | Key wiring done; **write side (kind:30174 publish) not connected** |
| **ip-layer** | No Cargo dep | NO | `omokoda-core/src/ip_layer.rs` implements kind 31900/1903 natively via `nostr_sdk` | Schema consumed as reference; kind 1901/1902 not implemented |
| **Koodu** | No Cargo dep | NO | `genesis/koodu_time.rs` is a Rust port; called at birth and in 7-state | Julia runtime unavailable; Rust port is the active implementation |

### Summary of Critical Gaps

| Gap | Severity | Location |
|---|---|---|
| If-Script: git rev pin may be behind local `~/If-Script/` | HIGH | `omokoda-core/Cargo.toml:ifascript` rev |
| minipae write side (kind:30174 engram publication) not wired into Omo-Koda2 | HIGH | `src/memory/` — no call to minipae Python |
| ip-layer kind 1901 (Creation Receipt) not emitted | MEDIUM | `src/ip_layer.rs` — only 31900/1903 implemented |
| ip-layer kind 1902 (Attestation) entirely absent | MEDIUM | No publisher anywhere |
| `ip-layer/src/` Rust files have no `Cargo.toml` — orphaned fragments | LOW | `~/ip-layer/src/publisher/`, `~/ip-layer/src/relay/` |
| Julia not installed — Koodu `.jl` files unrunnable | LOW | `~/Koodu/src/` — JS + Rust port cover the runtime need |
| `gix-core/src/memory_authority.rs:8` unused import warning | TRIVIAL | `GixVisibility` unused import |
| Duplicate GIX-FOLD-v1 in If-Script and gix-types | DESIGN | Intentional but creates maintenance obligation |
