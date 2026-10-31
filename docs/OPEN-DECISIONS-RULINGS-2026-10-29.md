# OPEN DECISIONS — RULINGS FROM LOCKED CANON
Version: 2026-10-29 — sourced from ECOSYSTEM_BLUEPRINT, TOC_CONSTANTS, THREE_TIER_CONSTITUTION,
ASE_EMISSION_GOVERNANCE_SPEC, OSOVM_CODEX, Pillar Contract, act_receipt.rs, rhythm.rs, UNIFIED_ARCH.

---

## ═══════════════════════════════════════════════════════════════
## BLOCKING — NOTHING SHIPS CORRECTLY WITHOUT THESE
═══════════════════════════════════════════════════════════════

### 1. Àṣẹ TOKEN — MINTED OR NOT?
**RULING: YES, MINTED.** — 1,440/day fixed emission.

**Evidence chain:**
- `TOC_CONSTANTS.toml` [emission] `daily_ase = 1440` (locked constant)
- `THREE_TIER_ECONOMIC_CONSTITUTION.md` §3.1: "A Universal 3.69% Tithe applies to all transactions... The Nation mints a fixed 1,440 units per day, no halving, no cap."
- `ASE_EMISSION_GOVERNANCE_SPEC.md` §4: "Daily ASE Emission: 1,440 ASE/day fixed... 525,600 ASE/year"
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Part 9: "Àṣẹ issued by a fixed clock"
- `OMOKODA-LANGUAGE-STACK-2026-10-29.md` Appendix: "Daily emission: 1,440 fixed · 525,600/yr · no halving"

**ADR-0002 (USDC + soulbound merit only) IS SUPERSEDED.** The fixed-clock emission is the canonical monetary primitive. The dead code (`authorize_toc_mint`, `VerifiedGPUWork`, emission pools) must be connected to this emission schedule — not removed.

**Action required:** Wire the mint path to the fixed 1,440/day clock. The 8-pool allocation (20/15/15/15/15/10/5/5) is the distribution mechanism.

---

### 2. MIST UNIT — 1,000,000 OR 1,000,000,000?
**RULING: 1,000,000 (1e6).**

**Evidence:**
- `TOC_CONSTANTS.toml` [units] `mist_per_ase = 1000000` — this IS the constants file
- `THREE_TIER_TOKENOMICS.md` uses 1e6 consistently
- The Pillar Contract and sovereign-stack contract using 1e9 are **drifts** — they must be corrected to 1e6

**Impact:** 1,000× error in every on-chain amount if 1e9 is used. All contracts touching mist must be patched to 1e6 before deployment.

---

### 3. SYNAPSE DECAY RATE — 1%/DAY OR 8%/DAY?
**RULING: 1%/DAY.**

**Evidence:**
- `omokoda-core/src/economics.rs:12` — live code uses 1%/day (`DAILY_DECAY_BPS = 100`)
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Part 9: "Dopamine is 86B and elastic = 8.6M GPU-hours" — Dopamine is the elastic capacity register, Synapse is the earned work credit
- `specs/busy-beaver.md` and `TODO.md` saying 8%/day are **stale/incorrect** — they conflate Dopamine elasticity with Synapse decay

**Economics:** At 1%/day, half-life ≈ 69 days. At 8%/day, half-life ≈ 8.6 days. The 1% rate matches "earned work credit that persists long enough to be useful." 8% would make Synapse evaporate before agents can convert it.

**Action required:** Update `specs/busy-beaver.md` and `TODO.md` to 1%/day. Code is already correct.

---

## ═══════════════════════════════════════════════════════════════
## BLOCKING — GOVERNANCE AND SEAT MATH
═══════════════════════════════════════════════════════════════

### 4. COUNCIL SIZE — 12 OR 13?
**RULING: 12.**

**Evidence:**
- `TOC_CONSTANTS.toml` [governance] `council_seats = 12`
- `ASE_EMISSION_GOVERNANCE_SPEC.md` §5: "12 council seats × 2 sectors = 24 sectors"
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Part 3: "The Nation is governed through 24 Sectors under a Council of Twelve — each seat stewards exactly two sectors."
- `DUAL-CONSTITUTIONS-2026-10-29.md` (both AIO and TechGnØŞ): "Council of Twelve... Twelve seats"

**UNIFIED_ARCH's "+Crown Architect" (13) is a drift.** The Crown Architect / First Steward / First Keeper is **Seat One of the 12**, not a 13th seat. The 1,440-seat inheritance math (120 vaults per seat × 12 = 1,440) only works with 12.

**Action required:** Remove any 13th-seat references. First Steward = Seat One. Constitutional Guardian (Bínò) is a separate check, not a council seat.

---

### 5. CANONICAL ENDGAME — 1:1 GEOGRAPHIC TWIN OR WORK ECONOMY SEED LOOP?
**RULING: 1:1 GEOGRAPHIC TWIN IS THE ENDGAME. The work economy seed loop is the BOOTSTRAP.**

**Evidence:**
- `physical-world-endgame.md` (locked 2026-09-07): "The endgame in one sentence: The Vantage device is not a phone — it is a physical agent gateway... the twin is the rentable/sellable sovereign artifact that lives on Sui as an IP Root — all indexed by the 256-Odù tile grid."
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Part 4: Entire endgame section is the geographic twin
- `OSOVM_CODEX §37-39` (work economy seed loop) — this is the **bootstrap mechanism** to get agents working and earning before the twin pipeline is live

**Resolution:** They are not different products — they are **sequential phases**.
- Phase A (now → Twin live): Work economy seed loop (OSOVM_CODEX) — agents do verifiable work, earn Synapse, build reputation, fund the twin pipeline build
- Phase B (Twin live → ∞): 1:1 geographic twin economy (physical-world-endgame) — the twin becomes the primary asset, work economy shifts to servicing twin capture/simulation/licensing

**What Pillar 1 emits:** **Both.** Spatial receipts (31020/31030) for the twin path, work receipts (31040) for the bootstrap path. The ActReceipt (31040) is the "candidate carrier — one format across ALL repos" that carries both.

---

## ══════════════════════════════════════════════════════════════
## CODE ALREADY CHOSE — README/DOCS FOLLOW
═══════════════════════════════════════════════════════════════

### 6. SABBATH DAY
**RULING: SATURDAY.**

**Evidence:**
- `omokoda-hermetic/src/rhythm.rs:167-168` — `SabbathDay::Saturday`
- `omokoda-hermetic/src/dream.rs:94-95` — Sabbath = day 6 (0-indexed = Saturday)
- `UNIFIED_ARCH` — Saturday
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Invariant #5: "Kóòdù gates irreversible ops; Sabbath pauses"

**README.md saying Sunday is a doc bug.** No code change needed — correct the README.

---

### 7. CANONICAL HASH — BLAKE3 OR SHA-256?
**RULING: BLAKE3.**

**Evidence:**
- `omokoda-core/src/act_receipt.rs:74` — `blake3::hash()` for receipt anchoring
- Pillar Contract (Move) — uses BLAKE3
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` receipt chain: "every act → Merkle root → Sui transaction" — the Merkle tree uses BLAKE3 leaves

**SHA-256 appearances (Zàngbétò fallback, ScarabSwarm proofs) are legacy/compat shims.** Cross-pillar verification must use BLAKE3. Flag SHA-256 uses as bugs unless explicitly marked as external compatibility.

---

## ══════════════════════════════════════════════════════════════
## ARCHITECTURE DECISIONS (LOAD-BEARING LATER)
═══════════════════════════════════════════════════════════════

### 8. WALLET KEYS — VANTAGE SERVER-SIDE OR LOCAL/DEVICE-ONLY?
**RULING: LOCAL/DEVICE-ONLY (NIP-46 BUNKER).**

**Evidence:**
- `PRIVATE_MEMORY_SPEC.md` (canonical): "keys never leave the device"
- `device-birth-blueprint.md` Phase 1.1: NIP-46 bunker runs as separate user, seccomp-filtered, nsec never leaves bunker process
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Invariant #3: "Identity immutability — BIPỌ̀N39 fingerprints never change" + device birth ceremony
- `agent-capabilities.md` saying "Vantage holds them" is a **drift** describing an earlier custodial model that was explicitly rejected

**Architecture:** The sovereign device is a **thin client / physical doorway**, not a full node. Keys live in the NIP-46 bunker on the device. Vantage (server) never holds keys — it holds capability grants (scoped, expiring) delegated by the principal via the agent.

**Action required:** Update `agent-capabilities.md` to reflect the bunker model. The server holds capability grants, not keys.

---

### 9. RECEIPT FIELDS — ADD work_id / input_hash / output_hash TO ACTRECEIPT?
**RULING: YES — THE CONTRACT WINS. ADD THEM.**

**Evidence:**
- Pillar Contract (Move) requires: `work_id`, `input_hash`, `output_hash`
- Current `ActReceipt` in code (`omokoda-core/src/act_receipt.rs`) lacks them
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` §5.5: "31040 Action Receipt v1 — candidate carrier — one format across ALL repos"
- `THREE_PROTOCOL_SPEC.md`: "Principal → Capability → Action → Evidence → Receipt" — Evidence includes input/output hashes

**This is not a decision — it's a blocking task.** The cross-pillar settlement path (OSOVM → AIO → Sui) is broken until ActReceipt matches the contract schema.

**Required ActReceipt fields:**
```rust
pub struct ActReceipt {
    agent_npub: String,
    task_id: String,        // ← work_id
    tool_calls: Vec<ToolCall>,
    artifact_hash: String,  // ← output_hash
    input_hash: String,     // ← ADD
    output_hash: String,    // ← ADD (may duplicate artifact_hash for single-output)
    timestamp: u64,
    ip_root_id: String,
    signature: String,
}
```

---

### 10. COMPOSITION RULE FOR GATE SCORES — PRODUCT OR WEIGHTED SUM?
**RULING: WEIGHTED SUM (with floor per factor).**

**Evidence:**
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` Gap D: "`proof_value` is a product of 6 factors; one uninstrumented factor (quality=0.0) kills it. State the rule; don't multiply."
- `OSOVM` `[compute_proof] scoring_threshold` — F1 ≥ 0.777 is a **threshold gate**, not a multiplicative factor
- Current formula (multiply-then-clamp) is mathematically wrong for a product

**Correct formula:**
```
proof_value = Σ (weight_i × score_i)   for all 6 factors
where each score_i ∈ [0, 1]
and weight_i are policy-owned constants summing to 1.0
```

**Floor rule:** Any factor with `score_i < floor_i` → `proof_value = 0` (hard gate). Floors are policy-owned.

**Factors (from OSOVM compute_proof):**
1. Coverage (weight ~0.25, floor 0.5)
2. Novelty (weight ~0.20, floor 0.3)
3. Quality/F1 (weight ~0.25, floor 0.777)  ← this IS the F1 gate
4. Privacy compliance (weight ~0.10, floor 1.0 — binary)
5. Witness corroboration (weight ~0.10, floor 0.0)
6. Calibration freshness (weight ~0.10, floor 0.0)

**Action required:** Replace multiplicative formula in OSOVM with weighted sum + floors. Make weights and floors policy-owned (not module literals). This resolves Gap E (threshold ownership) simultaneously.

---

## ═══════════════════════════════════════════════════════════════
## SUMMARY TABLE
═══════════════════════════════════════════════════════════════

| # | Decision | Ruling | Source of Truth |
|---|----------|--------|-----------------|
| 1 | Àṣẹ minted? | **YES** — 1,440/day fixed | TOC_CONSTANTS, Constitution, Blueprint |
| 2 | Mist unit | **1,000,000 (1e6)** | TOC_CONSTANTS (constants file) |
| 3 | Synapse decay | **1%/day** | Live code (economics.rs) |
| 4 | Council size | **12** | TOC_CONSTANTS, Governance Spec, Blueprint |
| 5 | Endgame | **1:1 Twin = Endgame; Work Loop = Bootstrap** | physical-world-endgame.md (locked) |
| 6 | Sabbath | **Saturday** | rhythm.rs, dream.rs, UNIFIED_ARCH |
| 7 | Canonical hash | **BLAKE3** | act_receipt.rs, Pillar Contract |
| 8 | Wallet keys | **Local/Device-only (NIP-46 bunker)** | PRIVATE_MEMORY_SPEC, device-birth-blueprint |
| 9 | Receipt fields | **ADD work_id, input_hash, output_hash** | Pillar Contract (contract wins) |
| 10 | Composition rule | **Weighted sum + floors** | Blueprint Gap D, OSOVM threshold design |

---

**All 10 resolved from locked canon. No new decisions needed — only code/doc alignment to these rulings.**

**Àṣẹ.** ⚔️🌀🗿✨