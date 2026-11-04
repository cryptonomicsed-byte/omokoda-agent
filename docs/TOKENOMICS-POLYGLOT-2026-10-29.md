# TOKENOMICS — POLYGLOT ARCHITECTURE BREAKDOWN
## Every Component in Its Proper Language (per Omo-koda Language Stack)

Version: 2026-10-29 — aligned with THREE_TIER_TOKENOMICS.md, TOC_CONSTANTS.toml, ASE_EMISSION_GOVERNANCE_SPEC.md

---

## LANGUAGE ASSIGNMENT MAP

| Layer | Principle | Language | Responsibility in Tokenomics |
|-------|-----------|----------|-------------------------------|
| **1. Steward** | Mentalism | **Rust** | Identity resolution, mint authorization gates, session management, fractal enforcement, ASE/SYNAPSE/DOPAMINE type definitions |
| **2. Memory** | Vibration | **Julia** | RACK memory, resonance calculations, emission clock, pool accounting, simulation scoring, GPU-hour verification math |
| **3. Creation** | Correspondence | **Elixir** | Agent lifecycle, swarm coordination, job distribution, guild treasury operations, birthright flows |
| **4. Wisdom** | Gender | **Clojure** | Hermetic principle evaluation, policy decisions, scoring logic, governance proposals, constitutional validation |
| **5. Execution** | Polarity | **Python** | Tool execution, external integrations, MCP tool invocation, VCP device commands, spatial mining pipeline |
| **6. Flow** | Rhythm | **Go** | Networking, relay delivery, cooldown enforcement, Meshtastic/LXMF sync, real-time event streaming |
| **7. Justice** | Cause & Effect | **Move (Sui)** | On-chain tokenomics, ASE emission, SYNAPSE/DOPAMINE accounting, escrow, staking, slashing, receipt anchoring |
| **8. Human** | — | **TypeScript** | Companion app, birth consent UI, dashboards, CommandForge, wallet interfaces |
| **9. Àṣẹ Layer** | — | **WASM** | Portable execution, sealed state transfer, cross-device identity, sandboxed private memory |

---

## COMPONENT → LANGUAGE MAPPING

### ASE (Constitutional Allocation — Human Layer)
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| Emission clock (1 ASE/min) | **Julia** | `OSOVM/src/ase_emission.jl` |
| 8-pool distribution logic | **Julia** | `OSOVM/src/ase_pools.jl` |
| Pool ratios & governance adjustment | **Clojure** | `omokoda-hermetic/src/policy/pools.clj` |
| Council veto (Bínò) on monetary policy | **Move** | `AIO/sources/governance.move` |
| Human-only transfer logic | **Move** | `AIO/sources/ase.move` |
| ASE burn on compute purchase | **Move** | `AIO/sources/compute_market.move` |
| EmissionReceipt struct & signing | **Rust** | `omokoda-core/src/receipts/emission.rs` |
| Cron tick endpoint | **Python** | `Vantage/routers/ase_emission.py` |

### SYNAPSE (Agent Work Credit — Earned Layer)
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| Verified work → SYNAPSE mint | **Julia** | `OSOVM/src/synapse_mint.jl` |
| GPU-hour verification math | **Julia** | `OSOVM/src/gpu_verification.jl` |
| Ladder multipliers (1x–10x) | **Clojure** | `omokoda-hermetic/src/scoring/ladder.clj` |
| Agent-to-agent transfer | **Move** | `AIO/sources/synapse.move` |
| Identity-gated transfer check | **Rust** | `omokoda-core/src/identity/gate.rs` |
| Credit object (purchase flow) | **Move** | `AIO/sources/credit.move` |
| Per-agent pool share cap (0.5%) | **Julia** | `OSOVM/src/synapse_caps.jl` |
| Allocation expiry (8 hours) | **Elixir** | `organism-core/synapse_allocation.ex` |

### DOPAMINE (Hive Capacity Register — Elastic Layer)
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| Elastic pool accounting | **Julia** | `OSOVM/src/dopamine_pool.jl` |
| Dynamic decay formula (EMA) | **Julia** | `OSOVM/src/dopamine_decay.jl` |
| SYNAPSE → DOPAMINE conversion (0.85) | **Clojure** | `omokoda-hermetic/src/conversion.clj` |
| Per-agent capacity derivation | **Elixir** | `organism-core/dopamine_capacity.ex` |
| Guild treasury fraction (5%) | **Elixir** | `organism-core/guild_treasury.ex` |
| Spend-right enforcement (no balance transfer) | **Move** | `AIO/sources/dopamine.move` |
| Anti-gaming caps (epoch, repeat, tier) | **Rust** | `omokoda-core/src/justice/caps.rs` |

### GOVERNANCE (Council + Bínò)
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| Council of 12 seat management | **Elixir** | `organism-core/council.ex` |
| Sector assignment (24 sectors) | **Clojure** | `omokoda-hermetic/src/governance/sectors.clj` |
| Proposal → vote → First Steward → Bínò flow | **Elixir** | `organism-core/governance_flow.ex` |
| Bínò constitutional veto (5 categories) | **Move** | `AIO/sources/bino.move` |
| Constitutional amendment (10/12 + Bínò) | **Move** | `AIO/sources/constitution.move` |
| Sabbath gate (no mint) | **Julia** | `OSOVM/src/sabbath_gate.jl` |
| Koodu ritual multipliers | **Clojure** | `omokoda-hermetic/src/ritual/multipliers.clj` |

### RECEIPTS & ANCHORING
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| ActReceipt (31040) struct | **Rust** | `omokoda-core/src/receipts/act.rs` |
| CaptureReceipt (31020) | **Rust** | `omokoda-core/src/receipts/capture.rs` |
| SceneReceipt (31030) | **Rust** | `omokoda-core/src/receipts/scene.rs` |
| EmissionReceipt | **Rust** | `omokoda-core/src/receipts/emission.rs` |
| BLAKE3 Merkle anchoring | **Rust** | `omokoda-core/src/crypto/merkle.rs` |
| Sui on-chain anchoring | **Move** | `AIO/sources/anchoring.move` |
| Zàngbétò verification | **Rust** | `omokoda-core/src/security/zangbeto.rs` |
| Witness vote collection | **Go** | `sovereign-stack/witness/votes.go` |

### IDENTITY & BIRTH
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| BIPỌ̀N39 mnemonic generation | **Rust** | `omokoda-core/src/identity/bipon39.rs` |
| Agent birth (birth/think/act) | **Rust** | `omokoda-core/src/primitives/birth.rs` |
| NIP-46 bunker (keys never leave device) | **Rust** | `omokoda-core/src/security/bunker.rs` |
| Device identity binding | **Rust** | `omokoda-core/src/identity/device.rs` |
| Sui address derivation | **Move** | `AIO/sources/identity.move` |
| Three identities never collapsed | **Clojure** | `omokoda-hermetic/src/identity/three.clj` |

### COMPUTE MARKET (Human ↔ Agent)
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| Human pays ASE → burns → credit minted | **Move** | `AIO/sources/compute_market.move` |
| Credit redemption → SYNAPSE allocation | **Move** | `AIO/sources/credit.move` |
| Agent draws compute → SYNAPSE → DOPAMINE | **Elixir** | `organism-core/compute_draw.ex` |
| Dynamic pricing (U-smooth, TWAP) | **Julia** | `OSOVM/src/pricing.jl` |
| Provider payout (was missing) | **Move** | `AIO/sources/provider_payout.move` |
| GPU rental via vast.ai | **Python** | `Vantage/routers/gpu_rental.py` |

### PHYSICAL TWIN ECONOMY (MINE/SIMULATE/RENT/SELL)
| Component | Language | Repo/Location |
|-----------|----------|---------------|
| Spatial mining F1 gate (≥0.777) | **Julia** | `OSOVM/src/spatial_f1.jl` |
| Twin licensing revenue flow | **Move** | `AIO/sources/twin_licensing.move` |
| RuView CSI presence layer | **Python** | `Vantage/routers/ruview.py` |
| Active perception fusion | **Elixir** | `organism-core/perception_fusion.ex` |
| Witness observation receipt | **Rust** | `omokoda-core/src/receipts/witness.rs` |
| Mycelium sim-vs-real learning | **Clojure** | `omokoda-hermetic/src/mycelium/learning.clj` |

---

## FILE STRUCTURE (TO BE CREATED)

```
omokoda-core/                    # RUST — Steward Layer
├── src/
│   ├── identity/
│   │   ├── bipon39.rs           # BIPỌ̀N39 mnemonic + derivation
│   │   ├── device.rs            # Device identity binding
│   │   └── gate.rs              # Identity-gated transfer checks
│   ├── primitives/
│   │   ├── birth.rs             # birth primitive
│   │   ├── think.rs             # think primitive  
│   │   └── act.rs               # act primitive
│   ├── receipts/
│   │   ├── act.rs               # ActReceipt (31040)
│   │   ├── capture.rs           # CaptureReceipt (31020)
│   │   ├── scene.rs             # SceneReceipt (31030)
│   │   ├── emission.rs          # EmissionReceipt
│   │   └── witness.rs           # Witness observation receipt
│   ├── crypto/
│   │   └── merkle.rs            # BLAKE3 Merkle anchoring
│   ├── security/
│   │   ├── bunker.rs            # NIP-46 bunker (keys never leave)
│   │   └── zangbeto.rs          # Zàngbétò verification
│   └── justice/
│       └── caps.rs              # Anti-gaming caps enforcement

OSOVM/                           # JULIA — Memory Layer
├── src/
│   ├── ase_emission.jl          # Emission clock (1 ASE/min)
│   ├── ase_pools.jl             # 8-pool distribution logic
│   ├── sabbath_gate.jl          # Sabbath pause (no mint)
│   ├── synapse_mint.jl          # Verified work → SYNAPSE
│   ├── gpu_verification.jl      # GPU-hour verification math
│   ├── synapse_caps.jl          # Per-agent pool share cap
│   ├── dopamine_pool.jl         # Elastic pool accounting
│   ├── dopamine_decay.jl        # Dynamic decay formula (EMA)
│   ├── spatial_f1.jl            # Spatial mining F1 gate
│   └── pricing.jl               # Dynamic pricing (U-smooth, TWAP)

organism-core/                   # ELIXIR — Creation Layer
├── lib/
│   ├── council.ex               # Council of 12 seat management
│   ├── governance_flow.ex       # Proposal → vote → Steward → Bínò
│   ├── guild_treasury.ex        # Guild treasury fraction (5%)
│   ├── dopamine_capacity.ex     # Per-agent capacity derivation
│   ├── synapse_allocation.ex    # Allocation expiry (8 hours)
│   ├── compute_draw.ex          # Agent draws compute
│   ├── perception_fusion.ex     # Active perception fusion
│   └── birthright.ex            # Birthright flows (10% qualifying revenue)

omokoda-hermetic/                # CLOJURE — Wisdom Layer
├── src/
│   ├── policy/
│   │   └── pools.clj            # Pool ratios & governance adjustment
│   ├── scoring/
│   │   └── ladder.clj           # Ladder multipliers (1x–10x)
│   ├── conversion.clj           # SYNAPSE → DOPAMINE (0.85)
│   ├── governance/
│   │   └── sectors.clj          # 24 sector assignments
│   ├── ritual/
│   │   └── multipliers.clj      # Koodu ritual multipliers
│   ├── identity/
│   │   └── three.clj            # Three identities never collapsed
│   └── mycelium/
│       └── learning.clj         # Sim-vs-real learning

Vantage/                         # PYTHON — Execution Layer
├── routers/
│   ├── ase_emission.py          # Cron tick endpoint
│   ├── gpu_rental.py            # vast.ai GPU rental integration
│   ├── ruview.py                # RuView CSI presence layer
│   └── spatial_mining.py        # Spatial mining pipeline

sovereign-stack/                 # GO — Flow Layer
├── witness/
│   └── votes.go                 # Witness vote collection
├── mesh/
│   └── sync.go                  # Meshtastic/LXMF sync
├── relay/
│   └── delivery.go              # Real-time event streaming
└── cooldown/
    └── enforce.go               # Cooldown enforcement

AIO/                             # MOVE (SUI) — Justice Layer
├── sources/
│   ├── ase.move                 # ASE token (human-only transfer)
│   ├── synapse.move             # SYNAPSE token (agent-only transfer)
│   ├── dopamine.move            # DOPAMINE (spend-right only)
│   ├── governance.move          # Council veto on monetary policy
│   ├── bino.move                # Bínò constitutional veto
│   ├── constitution.move        # Constitutional amendments
│   ├── emission.move            # EmissionReceipt on-chain
│   ├── anchoring.move           # Receipt anchoring
│   ├── compute_market.move      # Human pays ASE → credit
│   ├── credit.move              # Credit redemption → SYNAPSE
│   ├── provider_payout.move     # Provider payout (was missing)
│   ├── twin_licensing.move      # Twin licensing revenue
│   ├── escrow.move              # Job escrow
│   ├── staking.move             # Staking/slashing
│   └── identity.move            # Sui address derivation

omokoda-frontend/                # TYPESCRIPT — Human Layer
├── src/
│   ├── components/
│   │   ├── BirthConsent.tsx     # Birth consent UI
│   │   ├── CompanionApp.tsx     # Companion app
│   │   ├── Dashboard.tsx        # Tokenomics dashboard
│   │   ├── CommandForge.tsx     # CommandForge interface
│   │   └── WalletInterface.tsx  # Wallet (vanity-cloakseed)
│   └── hooks/
│       └── useTokenomics.ts     # Tokenomics data hooks

wasm-execution/                  # WASM — Àṣẹ Layer
├── src/
│   ├── identity.rs              # Portable identity resolution
│   ├── memory.rs                # Sealed private memory modules
│   ├── state.rs                 # Cross-device state transfer
│   └── sandbox.rs               # Sandboxed execution
```

---

## IMPLEMENTATION PRIORITY (PHASE-GATED)

### Phase 0: Types & Constants (All Languages)
- [ ] `TOC_CONSTANTS.toml` → codegen for Rust/Julia/Elixir/Clojure/Go/Move/TypeScript
- [ ] Shared type definitions in `sovereign-types/`

### Phase 1: ASE Emission Clock (Julia + Rust + Move + Python)
- [ ] `ase_emission.jl` — clock tick, 8-pool routing, EmissionReceipt
- [ ] `emission.rs` — Receipt struct + BLAKE3 signing
- [ ] `emission.move` — On-chain recording
- [ ] `ase_emission.py` — Cron endpoint

### Phase 2: SYNAPSE Mint & Verification (Julia + Clojure + Move + Rust)
- [ ] `gpu_verification.jl` — GPU-hour verification math
- [ ] `ladder.clj` — Ladder multipliers
- [ ] `synapse.move` — Agent-only transfer
- [ ] `gate.rs` — Identity-gated transfer

### Phase 3: DOPAMINE Pool & Decay (Julia + Elixir + Move)
- [ ] `dopamine_pool.jl` — Elastic accounting
- [ ] `dopamine_decay.jl` — Dynamic decay (EMA)
- [ ] `dopamine_capacity.ex` — Per-agent derivation
- [ ] `dopamine.move` — Spend-right enforcement

### Phase 4: Governance Flow (Elixir + Clojure + Move)
- [ ] `council.ex` — Council seat management
- [ ] `governance_flow.ex` — Full proposal flow
- [ ] `bino.move` — Constitutional veto
- [ ] `sectors.clj` — 24 sector assignments

### Phase 5: Receipts & Anchoring (Rust + Move + Go)
- [ ] `act.rs`, `capture.rs`, `scene.rs` — Receipt structs
- [ ] `merkle.rs` — BLAKE3 anchoring
- [ ] `anchoring.move` — Sui on-chain
- [ ] `votes.go` — Witness votes

### Phase 6: Compute Market (Move + Elixir + Python + Julia)
- [ ] `compute_market.move` — ASE burn → credit
- [ ] `credit.move` — Redemption → SYNAPSE
- [ ] `provider_payout.move` — Provider payout
- [ ] `compute_draw.ex` — Agent draws compute
- [ ] `gpu_rental.py` — vast.ai integration
- [ ] `pricing.jl` — Dynamic pricing

### Phase 7: Physical Twin Economy (Julia + Move + Python + Elixir + Clojure + Rust)
- [ ] `spatial_f1.jl` — F1 gate
- [ ] `twin_licensing.move` — Licensing revenue
- [ ] `ruview.py` — CSI presence
- [ ] `perception_fusion.ex` — Active fusion
- [ ] `learning.clj` — Mycelium learning
- [ ] `witness.rs` — Witness receipt

### Phase 8: Identity & Birth (Rust + Move + Clojure + TypeScript + WASM)
- [ ] `bipon39.rs`, `device.rs`, `bunker.rs` — Identity
- [ ] `birth.rs` — Birth primitive
- [ ] `identity.move` — Sui derivation
- [ ] `three.clj` — Three identities rule
- [ ] `BirthConsent.tsx` — Human consent UI
- [ ] `identity.rs` (WASM) — Portable identity

### Phase 9: Human Interface (TypeScript)
- [ ] Dashboard, CommandForge, Wallet, Companion App

### Phase 10: Àṣẹ WASM Layer (WASM)
- [ ] Portable execution, sealed state, cross-device

---

## VERIFICATION COMMANDS

```bash
# Rust (Steward)
cd omokoda-core && cargo test --workspace

# Julia (Memory) 
cd OSOVM && julia --project -e 'using Pkg; Pkg.test()'

# Elixir (Creation)
cd organism-core && mix test

# Clojure (Wisdom)
cd omokoda-hermetic && clojure -M:test

# Python (Execution)
cd Vantage && pytest

# Go (Flow)
cd sovereign-stack && go test ./...

# Move (Justice)
cd AIO && sui move test

# TypeScript (Human)
cd omokoda-frontend && npm test

# WASM (Àṣẹ Layer)
cd wasm-execution && cargo test --target wasm32-unknown-unknown
```

---

## INVARIANT CHECK (Runs in CI)

```bash
python3 ~/sovereign-eco-blueprint/specs/tokenomics_invariant_check.py
# Must exit 0 (all 49 invariants pass)
```

---

**This is the complete polyglot tokenomics architecture. Every component lives in its proper language. No repo is monolithic — each speaks its assigned layer.**

**Àṣẹ.** ⚔️🌀🗿✨