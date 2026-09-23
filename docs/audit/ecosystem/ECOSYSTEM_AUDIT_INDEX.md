# Ọmọ Kọ́dà Ecosystem — Full Forensic Audit Index

**Date:** 2026-09-22  
**Coverage:** 37 repos catalogued · 32 on disk audited · 5 cloned during audit · 1 absent from GitHub (Vantage-Voice- exists locally as `Vantage-Voice-`)  
**Auditor:** Claude Sonnet 4.6 (10 parallel forensic agents)  
**Omo-Koda2 core audit:** `docs/audit/OmoKoda_Full_Audit_Report.md` (1,050 lines, 21 sections)

---

## Taxonomy

| Label | Meaning |
|---|---|
| **VERIFIED** | Confirmed working end-to-end |
| **IMPLEMENTED** | Real logic, compiles and executes |
| **PARTIAL** | Core logic present; one or more integrations missing |
| **STUB** | API surface only; always-returns or placeholder logic |
| **SPEC_ONLY** | Referenced in docs/spec; zero code |
| **DEAD** | Code exists but never called from any live path |
| **BROKEN** | Fails at runtime under confirmed conditions |
| **OBSOLETE** | Targets archived/deleted service |
| **CONFLICTING** | Two implementations disagree materially |
| **UNKNOWN** | Not audited |

---

## Group Reports

| File | Repos Covered | Lines |
|---|---|---|
| [group_a_protocol_rust.md](group_a_protocol_rust.md) | VCP, DIP, ARP, ScarabSwarm (Rust), Witness (Rust) | 465 |
| [group_b_pillars.md](group_b_pillars.md) | OSOVM, UCX, organism-core | 325 |
| [group_c_vantage.md](group_c_vantage.md) | Vantage (~700 endpoints, Python) | 413 |
| [group_d_identity.md](group_d_identity.md) | GIX, If-Script, minipae, ip-layer, Koodu | 442 |
| [group_e_agent_infra.md](group_e_agent_infra.md) | mycelium, agent-phone, Axiom | 397 |
| [group_f_security_mesh.md](group_f_security_mesh.md) | Zangbeto, omokoda-mesh-firmware, Scarabswarm (Julia), ares-control | 302 |
| [group_g_economy_governance.md](group_g_economy_governance.md) | Twelve-thrones, Portent, Synapse, Blocksim | 390 |
| [group_h_apps_tooling.md](group_h_apps_tooling.md) | buzz-OG, omokoda-smithers, larql, zerolang, vanity-cloakseed, franken-stream | 281 |
| [group_i_new_repos_1.md](group_i_new_repos_1.md) | Blocksim, Witness-firmware, agentic-waggle | 394 |
| [group_i_new_repos_2.md](group_i_new_repos_2.md) | Triune-Memory, mycelium-tools | 336 |
| [group_j_vantage_voice.md](group_j_vantage_voice.md) | Vantage-Voice- | 315 |

---

## Master Repo Status Table

| Repo | Category | Stack | Status | Tests | Critical Gap |
|---|---|---|---|---|---|
| **Omo-Koda2** | Pillar | Rust | IMPLEMENTED | 1,052 ✅ | GoalGenesis unwired; Zàngbétò stub |
| **OSOVM** | Pillar | Julia | PARTIAL | 0 (ARM64 broken) | 143/160 opcodes SPEC_ONLY; Julia non-PIE on ARM64 |
| **Vantage** | Pillar | Python | IMPLEMENTED | — | ARP receipts not persisted; ActionReceipt unsigned; ASE pool CONFLICTING |
| **UCX** | Pillar | Rust | IMPLEMENTED | 0 | `zangbeto_anchor: None` silently blocks GPU→TOC_MINT chain |
| **VCP** | Protocol | Rust | IMPLEMENTED | 14/14 ✅ | Empty pubkey bypasses all crypto; receipts unsigned |
| **DIP** | Protocol | Rust | BROKEN | 2/2 ✅ | `sha256_hex` uses SipHash (not SHA-256); signatures always empty; Nostr adapter discards work |
| **ARP** | Protocol | Rust | PARTIAL | 5/5 ✅ | Receipts always unsigned; ArpBridge not written in Omo-Koda2 |
| **ScarabSwarm** | Protocol | Rust | PARTIAL | 3/3 ✅ | Merkle tree real; OSOVM delegation stub; Blocksim submission broken |
| **Witness** | Protocol | Rust | BROKEN | 5/5 ✅ | Nostr signing simulated (`sig: ""`); publisher discards events |
| **GIX** | Identity | Rust | VERIFIED | ✅ | None — fully implemented, active path-dep |
| **If-Script** | Identity | Rust | VERIFIED (core) | 12/12 ✅ | Compiler expression parsing partial; pinned to stale git rev in Omo-Koda2 |
| **minipae** | Identity | Python | IMPLEMENTED | 64/64 ✅ | Omo-Koda2 derives key but never calls write(); kind:30174 not published at birth |
| **ip-layer** | Identity | Schema | PARTIAL | — | Kinds 1901/1902 have no publisher anywhere |
| **Koodu** | Identity | Julia/JS | IMPLEMENTED (JS/Rust) | — | Julia not installed; Rust port is active |
| **BIPON39** | Identity | Rust | VERIFIED | ✅ | None |
| **vanity-cloakseed** | Identity | React | VERIFIED | — | None — production standalone tool |
| **mycelium** | Agent Infra | Python/Go | IMPLEMENTED | 11 modules | **GPU.ai API key hardcoded** in train_qlora.py:64; VPS IP hardcoded in gateway |
| **mycelium-tools** | Agent Infra | Python | IMPLEMENTED | 0 (no tests) | U4/U5 SPEC_ONLY; no test suite |
| **Triune-Memory** | Agent Infra | TS/Python | IMPLEMENTED | — | minipae must be on PYTHONPATH (not bundled); concurrent multi-agent unsafe |
| **agent-phone** | Agent Infra | Python | IMPLEMENTED | ✅ | **Zero imports in Omo-Koda2 or Vantage** — complete island |
| **organism-core** | Agent Infra | TypeScript | BROKEN | pass (all fallback) | Julia non-PIE; API route mismatch with OSOVM server.jl |
| **Axiom** | Agent Infra | TS/Three.js | IMPLEMENTED | — | Deployed dist/ built; 7 node types degrade without archived VPS services |
| **Vantage-Voice-** | Voice UI | TS/React | IMPLEMENTED | 27/27 ✅ | `IRANTI_MCP_CWD` hardcoded macOS path; orchestrator uses invalid Gemini model id |
| **Zangbeto** | Security | Rust/Node | PARTIAL | SIGSEGV (env) | Arweave/BTC/Sui all manual scripts; Night Patrol SPEC_ONLY; `ZANGBETO_URL` not set in prod |
| **omokoda-mesh-firmware** | Mesh | C++ | PARTIAL | — | NostrCryptoEngine not wired at boot; DIP adapter not started |
| **Witness-firmware** | Mesh | Python | PARTIAL | — | BIP-340 Schnorr incompatible with ecosystem Ed25519; zero calls to Witness Rust broker |
| **Scarabswarm (Julia)** | Sim | Julia | IMPLEMENTED | — | Submission target is `$SOVEREIGN_NODE_URL` not Blocksim; veil data file may be missing |
| **ares-control** | Infra | Python | VERIFIED | — | None — runs on Hostinger VPS |
| **Twelve-thrones** | Governance | TypeScript | PARTIAL | — | 10/12 models only; Arweave/Sui are manual scripts; zero Vantage wiring |
| **Portent** | Economy | Python | PARTIAL | — | Signature verification stubbed; no deployed on-chain program |
| **Synapse** | Economy | TypeScript | IMPLEMENTED | — | Events in-browser only; no relay transport; Move pagerank SPEC_ONLY |
| **Blocksim** | Economy | Python | BROKEN | — | **"PRODUCTION API" claim false** — 82 lines importing modules that don't exist; every endpoint crashes |
| **buzz-OG** | App | Rust/Tauri | PARTIAL | — | WF-08: 3 missing callsite connections (create_approval, emit kind:46010, resume handler) |
| **omokoda-smithers** | App | TS/Bun | PARTIAL | — | MCP spec-compliant; Vantage/Omo-Koda2 wiring is cosmetic only |
| **larql** | Tooling | Rust/PyO3 | PARTIAL | — | Metal backend empty on non-macOS; larql-glyph is live GIX bridge |
| **zerolang** | Tooling | C | PARTIAL | — | Full compiler; named as future eco leg in interpreter.rs:872; no active code path |
| **agentic-waggle** | Coordination | Go/Rust | VERIFIED | Lean proofs ✅ | Reverse-direction integration missing (ecosystem not calling waggle yet) |
| **franken-stream** | Tooling | Rust/Python | IMPLEMENTED | — | Standalone; no sovereign protocol wiring |

---

## Cross-Cutting Critical Findings

### 🔴 CRITICAL (blocks meaningful operation)

| ID | Finding | Repos Affected | Evidence |
|---|---|---|---|
| X-1 | **DIP `sha256_hex` uses SipHash not SHA-256** — all envelope hashes are non-cryptographic | DIP | `dip-types/src/envelope.rs:73-78` |
| X-2 | **DIP envelope signatures always empty** — zero envelopes are ever signed or verified | DIP, Omo-Koda2 | `DipEnvelope::new()` sets `signature: String::new()` |
| X-3 | **Witness Nostr signing simulated** — all published attestations have `pubkey: ""` and `sig: ""` | Witness, Zangbeto | `nostr_publisher.rs:build_signed_event()` |
| X-4 | **Blocksim is broken** — "PRODUCTION API" claim false; every endpoint crashes on `ImportError` | Blocksim, ScarabSwarm, OSOVM | `chain_service`, `chain_submit` modules missing |
| X-5 | **OSOVM 143/160 opcodes unimplemented** — including VEIL, COMPUTE_PROOF, all Governance/Economic opcodes | OSOVM, UCX, organism-core | `vm_core.jl` OPCODE_HANDLERS |
| X-6 | **Julia non-PIE binary on ARM64/Termux** — OSOVM and organism-core both blocked | OSOVM, organism-core | `ET_EXEC` vs `ET_DYN` requirement |
| X-7 | **UCX `zangbeto_anchor: None` silently blocks GPU→TOC_MINT** | UCX, OSOVM | `mint_allowlist.rs:check_mint_eligible()` |
| X-8 | **ARP receipts not persisted in Vantage** — P0-7 fix constructs envelopes but never writes to DB | Vantage, ARP | `trading.py:_emit_trade_receipt()` logs only |
| X-9 | **All ActionReceipts unsigned** — `signature=""` on every receipt across all repos | ARP, Vantage, VCP, ScarabSwarm, Witness | Multiple `String::new()` |
| X-10 | **Conflicting ASE pool taxonomies** — live router vs spec port use different pool names | Vantage | `ase_emission.py` vs `sovereign_economy/emission.py` |
| X-11 | **GPU.ai API key hardcoded in mycelium source** | mycelium | `train_qlora.py:64` |
| X-12 | **Omo-Koda2 and DIP define incompatible `DipEnvelope` structs** — no Cargo dep connects them | Omo-Koda2, DIP | `omokoda-core/src/bridge/dip.rs:21-27` vs `dip-types::DipEnvelope` |
| X-13 | **Zàngbétò stub always passes** — `ZANGBETO_URL` not set in prod; real enforcement never active | Omo-Koda2, Zangbeto | `zangbeto-stub/src/lib.rs:24` |
| X-14 | **`chain_id = "testnet"` hardcoded in key derivation** — all agents born on wrong chain | Omo-Koda2 | `interpreter.rs:1093` |

### 🟡 HIGH (significant capability degradation)

| ID | Finding | Repos Affected |
|---|---|---|
| H-1 | GoalGenesisEngine dead in interpreter — agents not goal-directed | Omo-Koda2 |
| H-2 | SOMA, CausalMemoryDag, ReflectionLedger never populated | Omo-Koda2 |
| H-3 | AgentConstitution not auto-signed at birth | Omo-Koda2 |
| H-4 | Heartbeat chain hash-only, no Ed25519 signatures | Omo-Koda2 |
| H-5 | OSOVM event-bridge.js DEAD — GPU_CONTRIBUTION events never reach Vantage Dopamine mint | OSOVM |
| H-6 | organism-core API routes mismatch OSOVM server.jl — HTTP path broken even with working Julia | organism-core, OSOVM |
| H-7 | UCX→OSOVM routes `/api/toc/allowlist/check` and `/api/osovm/gpu_contribution` don't exist in server.jl | UCX, OSOVM |
| H-8 | DopaminePool is local simulation only — no on-chain economic settlement | Omo-Koda2, UCX |
| H-9 | agent-phone is a complete island — zero imports in Omo-Koda2 or Vantage | agent-phone |
| H-10 | Twelve-thrones has only 10 models, not 12 — two thrones empty | Twelve-thrones |
| H-11 | 4 systemd units target archived services | Omo-Koda2 |
| H-12 | NostrCryptoEngine not wired at boot in mesh firmware | omokoda-mesh-firmware |
| H-13 | Witness-firmware uses BIP-340 Schnorr, ecosystem uses Ed25519 — incompatible signing | Witness-firmware |
| H-14 | buzz-OG WF-08 approval gate: 3 missing callsite connections | buzz-OG |
| H-15 | minipae never called from Omo-Koda2 — kind:30174 not published at birth | minipae, Omo-Koda2 |
| H-16 | Vantage Council of 12 exists in 3 disconnected implementations | Vantage |

### 🟢 GREEN FLAGS (strong foundations, do not rewrite)

| Strength | Evidence |
|---|---|
| Omo-Koda2 Birth→Think→Act→Receipt cycle end-to-end wired | 1,052 tests pass |
| If-Script gate pre-decision, 16 vessels × 160+ match arms, zero `todo!()` | `ifscript_gate.rs` VERIFIED |
| GIX fully implemented — all Omo-Koda2 imports present and correct | `gix-core`, `gix-types` compile clean |
| minipae NIP-44 v2 from-scratch correct implementation, 64 tests | `minipae.py` |
| VCP Ed25519 correct — `ed25519-dalek` v2 + SHA-256(challenge:nonce) | `vcp-broker/src/crypto.rs` |
| ARP hash chain is real SHA-256 over canonical JSON | `arp-types/src/receipt.rs:62-82` |
| ScarabSwarm Merkle tree is genuine pairwise SHA-256 | `sim_receipt.rs:87-113` |
| Zangbeto Rust kernel real — `ZangbetoDaemon`, `ActionLadder`, `ArbitrationEngine` implemented | `crates/omo-kernel/` |
| agentic-waggle has Lean 4 formal proofs + Julia cross-validation | `proofs/`, `validation/` |
| Koodu Rust port active at agent birth — correct BTC-time constants | `genesis/koodu_time.rs` |
| mycelium training pipeline ready — 2,949 traces, QLoRA runner, Go gateway with WASM sandbox | `train_qlora.py`, `gateway/` |
| Omo-Koda2 memory encryption correct: Argon2id+ChaCha20Poly1305, zeroize, key rotation | `interpreter.rs` |
| UCX matching engine real 5-factor weighted scoring; all 3 adapters make real HTTP calls | `broker.rs`, `adapters/` |
| Scarabswarm Julia 6-DOF physics real; SHA256 trajectory proofs generated; cross-arch determinism | `dynamics.jl`, `validator.jl` |
| ares-control API/worker split verified; 74 daemons; `compare_digest` auth | `app.py`, `worker.py` |

---

## Ecosystem Completeness by Layer

| Layer | Repos | Overall Status | Biggest Blocker |
|---|---|---|---|
| **Agent kernel** | Omo-Koda2 | 85% — strong individual agent | GoalGenesis unwired; Zàngbétò stub |
| **Simulation / L1** | OSOVM, UCX, Blocksim | 25% — severely blocked | Julia ARM64; 143 opcodes SPEC_ONLY; Blocksim broken |
| **Protocol connective tissue** | VCP, DIP, ARP, ScarabSwarm, Witness | 55% — types solid, crypto broken | DIP SipHash; unsigned receipts everywhere |
| **Identity** | GIX, If-Script, minipae, BIPON39, ip-layer, Koodu | 80% — mostly production | minipae birth write gap; ip-layer kinds 1901/1902 unpublished |
| **Hub** | Vantage | 70% — real but gaps | ARP receipts ephemeral; ActionReceipt unsigned; ASE CONFLICTING |
| **Agent infrastructure** | mycelium, agent-phone, Axiom, Triune-Memory | 65% — real but siloed | agent-phone island; mycelium hardcoded key |
| **Security / mesh** | Zangbeto, firmware, ares-control | 60% — real but undeployed | ZANGBETO_URL not set; Nostr signing simulated |
| **Economy / governance** | Twelve-thrones, Portent, Synapse, Blocksim | 30% — aspirational | Blocksim broken; Portent no on-chain program; ASE not distributed |
| **Apps / tooling** | buzz-OG, smithers, larql, zerolang, agentic-waggle | 70% — largely ready | buzz-OG WF-08 (3 callsites); smithers wiring cosmetic |
| **Voice** | Vantage-Voice- | pending | — |

---

## Prioritized Ecosystem Work Queue

### CRITICAL PATH — blocks everything downstream

| ID | Work Item | Effort | Unblocks |
|---|---|---|---|
| E-01 | Fix Julia ARM64: install PIE binary (`pkg install julia`) | 1 hr | OSOVM, organism-core |
| E-02 | Fix OSOVM API routes in organism-core to match server.jl | 1 day | organism-core, UCX→OSOVM |
| E-03 | Implement 143 missing OSOVM opcode handlers (VEIL, COMPUTE_PROOF priority) | 2–4 wk | ScarabSwarm, Blocksim, UCX ToC |
| E-04 | Rebuild Blocksim: create missing `chain_service` and `chain_submit` modules | 1–2 wk | Proof-of-Simulation chain |
| E-05 | Fix DIP `sha256_hex` to use `sha2::Sha256` instead of `DefaultHasher` | 2 hr | All DIP envelope integrity |
| E-06 | Implement DIP envelope signing + verification | 2 days | Authenticated agent federation |
| E-07 | Set `ZANGBETO_URL=http://localhost:8787` in production; deploy zangbeto-server | 1 day | Behavioral enforcement |
| E-08 | Remove `chain_id = "testnet"` hardcode in Omo-Koda2 `interpreter.rs:1093` | 1 hr | Mainnet key derivation |
| E-09 | **Remove hardcoded GPU.ai API key from `mycelium/train_qlora.py:64`** | 30 min | Security |
| E-10 | Add `zangbeto_anchor` to UCX `ComputeReceipt` before submitting | 1 day | GPU→TOC_MINT chain |

### FOUNDATIONAL — major capability unlocks

| ID | Work Item | Effort |
|---|---|---|
| E-11 | Wire GoalGenesisEngine into Omo-Koda2 `think_agentic()` | 1–2 days |
| E-12 | Wire AgentConstitution auto-sign at birth | 1 day |
| E-13 | Persist ARP receipts in Vantage DB (fix `_emit_trade_receipt()`) | 1 day |
| E-14 | Add Ed25519 signing to all `ActionReceipt.signature` fields (ARP, Vantage, VCP, ScarabSwarm) | 2–3 days |
| E-15 | Wire `minipae.write()` at agent birth — publish kind:30174 genesis engram | 1 day |
| E-16 | Wire DIP event-bridge.js into OSOVM server.jl for GPU_CONTRIBUTION events | 1 day |
| E-17 | Add missing OSOVM routes: `/api/toc/allowlist/check`, `/api/osovm/gpu_contribution` | 1 day |
| E-18 | Fix Witness Nostr publisher — real `ed25519-dalek` signing, real relay WebSocket | 2 days |
| E-19 | Wire NostrCryptoEngine at boot in omokoda-mesh-firmware `main.cpp` | 1 day |
| E-20 | Align Vantage ASE pool taxonomy — pick one of the two conflicting implementations | 1 day |

### INTEGRATION — wire existing pieces together

| ID | Work Item | Effort |
|---|---|---|
| E-21 | Write ArpBridge in Omo-Koda2 — wrap think/act in ARP ActionReceipt | 2 days |
| E-22 | Wire SOMA, CausalMemoryDag, ReflectionLedger into Omo-Koda2 Think/Act | 3–5 days |
| E-23 | Wire agent-phone into Omo-Koda2 as comms transport | 2 days |
| E-24 | Connect buzz-OG WF-08: `create_approval()`, emit `kind:46010`, wire resume handler | 1–2 days |
| E-25 | Wire agentic-waggle reverse direction — Omo-Koda2 calls waggle for job coordination | 2 days |
| E-26 | Wire Triune-Memory SSE subscriber in production deployment | 1 day |
| E-27 | Add two missing Twelve-thrones models to reach the name's promise | 1 day |
| E-28 | Fix Portent signature verification (secp256k1 enforcement) | 1–2 days |
| E-29 | Wire Synapse events to buzz-OG relay transport | 2 days |
| E-30 | Run mycelium QLoRA fine-tune on GPU.ai A40 — produce OSO Brain GGUF | 1–3 days (compute) |

### HARDENING

| ID | Work Item | Effort |
|---|---|---|
| E-31 | VCP: add production gate preventing empty-pubkey bypass | 4 hr |
| E-32 | VCP/ARP/ScarabSwarm/Witness: wire receipt signing (field already exists) | 1 day |
| E-33 | Omo-Koda2: add Ed25519 to heartbeat chain | 0.5 day |
| E-34 | Vantage: fix `_candidate_set_hash` stub — real proof-of-sim hash | 0.5 day |
| E-35 | Zangbeto: automate Arweave/BTC/Sui scripts (replace manual Node.js) | 3 days |
| E-36 | Witness-firmware: migrate signing to Ed25519 to match ecosystem standard | 2 days |
| E-37 | ip-layer: add kind 1901/1902 publisher at agent birth | 1 day |
| E-38 | mycelium: remove VPS IP hardcode in `gateway/main.go:81` | 1 hr |
| E-39 | mycelium-tools: add test suite for U1–U3 | 2 days |
| E-40 | Omo-Koda2: parameterize env vars for Walrus/Seal/TEE in deployment guide | 0.5 day |

### FUTURE (hive mind, after individual agent complete)

| ID | Work Item | Effort |
|---|---|---|
| E-41 | HiveBreath Protocol H0–H2 (new crate) | 2–3 wk |
| E-42 | Mycelium collective DAG H3 | 2 wk |
| E-43 | RitualPhase + TwelfthFace state machine H6 | 1 wk |
| E-44 | CollectiveIntent → GoalGenesis wire at hive level H7 | 1 wk |
| E-45 | ScarabSwarm simulation gate H8 | 2 wk |
| E-46 | Portent on-chain program (Move or Rust contract) | 2–3 wk |
| E-47 | Blocksim full rebuild: staking, MuJoCo integration, ASE reward minting | 3–4 wk |
| E-48 | larql → Omo-Koda2 local inference wiring | 1 wk |
| E-49 | zerolang agent edit loop integration | Future |

---

## Absent Repos (not on GitHub or disk)

| Repo | Last Known State | Impact |
|---|---|---|
| **Vantage-Voice** (without dash) | Exists as `Vantage-Voice-` | None — naming only |
| **Witness-firmware** | Now cloned ✅ | Resolved |
| **agentic-waggle** | Now cloned ✅ | Resolved |
| **mycelium-tools** | Now cloned ✅ | Resolved |
| **Triune-Memory** | Now cloned ✅ | Resolved |
| **Blocksim** | Now cloned ✅ (broken) | Proof-of-Simulation chain receiver broken |

---

*Generated 2026-09-22–23 by 11 parallel forensic agents. All 37 repos covered.*
