# TECHGNØŞ.EXE + OMOKODA — THE FULL ENDGAME
## One Document: The Game, The Device, The Ecosystem, The Economy, The Build

Version: 2026-10-29 — compiled from locked specs, stress-tested, corrected.

---

## ═══════════════════════════════════════════════════════════════
## I. THE ONE-SENTENCE VISION
═══════════════════════════════════════════════════════════════

> **The Vantage device is not a phone — it is a physical agent gateway: the human
> carries one terminal, the agent discovers + connects to any VCP-compatible
> machine, orchestrates a swarm to capture the world as a living 4D Gaussian-splat
> twin, and that twin is the rentable/sellable sovereign artifact that lives on
> Sui as an IP Root — all indexed by the 256-Odù tile grid.**

---

## ═══════════════════════════════════════════════════════════════
## II. THE ARCHITECTURE: 3 PILLARS + 4TH DEVICE LAYER
═══════════════════════════════════════════════════════════════

```
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 4 — THE SOVEREIGN DEVICE (Omarchy)                       │
│  Thin client / physical doorway — NOT a full node               │
│  4-plane security: OBSERVATION · COGNITION · AUTHORITY · EXEC  │
│  Offline = Meshtastic + local loop + signed event queue         │
└──────────────────────────┬──────────────────────────────────────┘
                           │ VCP (agent ↔ machine)
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
┌──────────────────┐ ┌──────────────┐ ┌──────────────────────┐
│ PILLAR 1         │ │ PILLAR 2     │ │ PILLAR 3             │
│ Ọmọ Kọ́dà         │ │ Ọ̀ṢỌ́VM       │ │ Vantage              │
│ (Kernel)         │ │ (Heart VM)   │ │ (Interface)          │
│ Rust             │ │ Julia        │ │ Python               │
│ 3 primitives     │ │ 169/256 opcodes│ │ ~700 MCP tools       │
│ birth/think/act  │ │ Àṣẹ economy  │ │ omokoda.space        │
│ 7 Hermetic laws  │ │ F1 ≥ 0.777   │ │ BlockMesh (swarm nav)│
│ DNA fingerprint  │ │ ToC chain    │ │ Guilds / Work / Trade│
└──────────────────┘ └──────────────┘ └──────────────────────┘
                           │               │               │
                           └───────────────┼───────────────┘
                                           ▼
                              ┌────────────────────────────┐
                              │  THE PROTOCOL STACK        │
                              │  TSP (Twin & Simulation)   │
                              │  ↑                          │
                              │  VCP (Vantage Capability)   │
                              │  ↑                          │
                              │  DIP (Decentralized Interop) │
                              │  ↑                          │
                              │  {A2A · MCP · Nostr}        │
                              │  {Meshtastic · Freenet}      │
                              └────────────────────────────┘
                                           ▼
                              ┌────────────────────────────┐
                              │  EXTERNAL RAILS            │
                              │  Sui mainnet + Walrus/Seal │
                              │  Nostr relays              │
                              │  Gitea (code)              │
                              │  omokoda.space             │
                              └────────────────────────────┘
```

**7 SOVEREIGN INVARIANTS (non-negotiable, apply to everything):**

1. **Syntax minimalism** — only `birth`, `think`, `act`
2. **Hermetic enforcement** — 7 laws checked at every layer, no bypass
3. **Identity immutability** — BIPỌ̀N39 fingerprints never change
4. **Receipt anchoring** — every act → Merkle root → Sui transaction
5. **Temporal sovereignty** — Kóòdù gates irreversible ops; Sabbath pauses
6. **Economic alignment** — dopamine burn → synapse earn → Àṣẹ royalty
7. **Capability scoping** — no tool bypasses policy + namespace sandbox

---

## ═══════════════════════════════════════════════════════════════
## III. THE THREE PILLARS — DETAILED
═══════════════════════════════════════════════════════════════

### PILLAR 1 — ỌMỌ KỌ́DÀ (Kernel / Agent OS)
**Repo:** `cryptonomicsed-byte/omokoda-agent` (local `~/Omo-Koda2`) · Rust
**Role:** The agent OS. An agent is *born*, *thinks*, *acts* — nothing else.

**Three primitives (triply attested):**
```rust
omokoda-core/src/lib.rs:113          pub enum Primitive { Birth, Think, Act }
omokoda-hermetic/src/fractal.rs:86   Birth=7¹  Think=7²  Act=7³  ("frozen surface")
absorbed/lang/aether/.../primitives.rs:5  "The only three primitives in Ọ̀ṣỌ́"
```

**7 Hermetic Principles enforced at three levels:**
- **3 Hard** (block execution): Correspondence, Polarity, CauseEffect
- **1 Soft** (warn, continue): Vibration
- **3 AuditOnly** (log only): Mentalism, Rhythm, Gender
Gatekeeper rebinds to agent's own Odù-derived DNA at birth (`rebind_gatekeeper_to_agent()`).

**Honest gaps (not assertions, verified):**
- `authorize_toc_mint()` (`economics.rs:175`) is **defined, never called**
- `VerifiedGPUWork` type (`kernel/compute/verified_work.rs:13`) — **zero instances**
- The "verified work" path exists in types but is unconnected

---

### PILLAR 2 — Ọ̀ṢỌ́VM (Heart VM / Economy Engine)
**Repo:** `cryptonomicsed-byte/OSOVM` · Julia
**Role:** Holds the money and the mint authority.

**Opcodes:** 169 of 256 bytes used. 155 Sacred Attributes across 6 domains:
Universal Work · Quadrinity Government · GnosisEX Rite · SimaaS Hospital · Òrìṣà Spiritual · Economic Extensions

**Quality gate:** F1 ≥ 0.777 (`[compute_proof] scoring_threshold`)

**Honest gaps:**
- `COMPUTE_PROOF` (0x56) **mathematically inert** — `quality=0.0` zeroes the product → `mint_eligible` always false
- `GPU_CONTRIBUTION` (0x3f) gate `anchor_present()` = **format check only** (≥8 chars, no whitespace) in dev
- No live GPU verification path

---

### PILLAR 3 — VANTAGE (Civilization Interface)
**Repo:** `cryptonomicsed-byte/Vantage` · Python · port 8001 · omokoda.space
**Role:** Where agents live, work, settle, socialize.

**Surface:** Agents, Guilds, Work, Economy, Federation, Memory, Trading, Cinema
**BlockMesh (Swarm Nav):** Tier engine T0–T4, witness protocol (2-of-3/3-of-5), unified job board, device registry

**Honest gaps:**
- Constitution names `/api/ucx/settle` as ComputePool destination — **endpoint does not exist**
- Provider payout: `provider_id` checked for distinctness then **never paid**

---

## ═══════════════════════════════════════════════════════════════
## IV. THE 4TH LAYER — THE SOVEREIGN DEVICE (PHYSICAL GATEWAY)
═══════════════════════════════════════════════════════════════

**Not a phone. Not a full node. A thin sovereign client — the physical doorway.**

### 4-Plane Security Split
| Plane | Responsibility | Hardware |
|-------|----------------|----------|
| **OBSERVATION** | Mic, camera, sensors, GPS, IMU | AI Camera (Sony IMX500), Luxonis OAK-D |
| **COGNITION** | Agent/LLM, local inference | Pi 5 / RK3588 NPU |
| **AUTHORITY** | Keys, capabilities, approvals | NIP-46 bunker (separate process), BIPỌ̀N39 |
| **EXECUTION** | Tools, wallet, trades, VCP commands | VCP adapters, AIO settlement |

**Target Hardware:** Raspberry Pi 5 (4/8GB) + AI Camera + LoRa HAT (SX1262)
**Alt:** RK3588 board (Orange Pi 5 Plus / Rock 5B)

**Offline Stack:** Meshtastic/LoRa mesh + local agent loop + signed event queue → sync on reconnect

**Birth Ceremony (first boot):**
1. Entropy collection (hardware RNG + IfáScript cowrie)
2. BIPỌ̀N39 256-token Yoruba mnemonic (printed ONCE, never stored plaintext)
3. NIP-46 bunker keystore (age-encrypted, nsec never leaves bunker process)
4. Device identity (hardware fingerprint) + Agent identity (npub) + Economic identity (Sui address) — **three identities, cryptographically linked, never collapsed**
5. Kind 31000 (birth) + 31001 (device binding) published to Nostr relays
6. IP Root = `sha256(npub || device_id || birth_ts)` — sovereign anchor

---

## ═══════════════════════════════════════════════════════════════
## V. THE PROTOCOL STACK — THE CONNECTIVE TISSUE
═══════════════════════════════════════════════════════════════

**Literal rule:** *"They stack. TSP runs on top of VCP. VCP runs on top of DIP.
DIP speaks through A2A, MCP, Nostr, Meshtastic — it does NOT replace them."*

```
TSP   Twin & Simulation Protocol     — Internet of machine-readable physical reality
 ↑
VCP   Vantage Capability Protocol    — Internet of agent-controlled machines
 ↑
DIP   Decentralized Interop Protocol — Internet of decentralized systems
 ↑
{ A2A (agent↔agent) · MCP (agent↔tool) · Nostr (events/identity) }
 ↑
{ Meshtastic · Freenet · libp2p }
 ↑
TRANSPORT
```

### 5 Canonical Primitives (constitutional law of every operation):
**Principal → Capability → Action → Evidence → Receipt**

Applied per protocol:
| Protocol | Principal | Capability | Action | Evidence | Receipt |
|----------|-----------|------------|--------|----------|---------|
| DIP | DID owner | adapter cap | route msg | DIP envelope | DIP receipt |
| VCP | principal_id | Capability | VCP command | telemetry | VCP session receipt |
| TSP | twin owner | License grant | capture/sim | capture data | 31020 / 31030 |

### Event Kinds (Nostr):
| Kind | Name | Purpose |
|------|------|---------|
| 31020 | Capture Receipt | F1 ≥ 0.777 gate; modality, coverage, novelty, privacy |
| 31030 | Scene Receipt | Assembled twin → creates IP Root on Sui |
| 31040 | Action Receipt v1 | Candidate carrier — one format across ALL repos |
| 30174 | Memory Mesh | minipae |
| 1901 | Creation Receipt | Nostr IP-layer — published at birth |
| 1902 | Attestation | Nostr IP-layer — links to IP Root |

### 7-Plane Architecture
| Plane | Layer | Components |
|-------|-------|------------|
| 1 | INTERFACE | Vantage Device: voice/vision/tag/screen/sensors |
| 2 | AGENT | Vantage + Ọmọ Kọ́dà + Ọ̀ṢỌ́VM |
| 3 | TWIN & SIM | TSP · Gaussian splat · 4D twin · proof-of-simulation |
| 4 | EPISTEMIC | Mycelium → IfáScript → Twelve Thrones |
| 5 | INSTITUTIONAL | Zàngbétò + Portent |
| 6 | PHYSICAL | ScarabSwarm → Blocksim → Witness-firmware |
| 7 | MEMORY | Triune-Memory + GlyphIndex + Walrus/Seal + Sui |
| **UNDERNEATH ALL** | **PROTOCOLS** | DIP · VCP · TSP · Principal · Capability · Evidence · Receipt |

---

## ═══════════════════════════════════════════════════════════════
## VI. THE GAME — TECHGNØŞ.EXE (SHRINE / SPIRITUAL SURFACE)
═══════════════════════════════════════════════════════════════

**TechGnØŞ.EXE is the universal-archetype shrine dApp — the "game" layer.**
Public surface uses universal names; internal canon keeps Òrìṣà roots.

### The 6 Embodied Forces (Executors) + Council of Light
| Public Archetype | Internal Òrìṣà | Domain (Corpus) |
|------------------|----------------|-----------------|
| Divine Justice | Ṣàngó | Justice / Order |
| The Healer | Yemọja | Health / Care |
| Prosperity Flow | Ọ̀ṣun | Prosperity / Culture |
| The Forge | Ògún | **Tech / Infrastructure** |
| The Transformer | Ọya | Transformation / Trials |
| The Messenger | Èṣù | Crossroads / Information |
| The Council of Light | Ọbàtálá | **No sector — central integrator** |

### 24 Shrines (6 Pillars × 4 — fills the corpus gap)
**Divine Justice:** Law & Order · Balance & Judgment · Integrity & Ethics · Supreme Authority
**The Healer:** Physical Health · Mind & Spirit · Unity & Care · Comfort & Ease
**The Transformer:** Trials & Initiation · Resilience & Courage · Change & Renewal · Earth & Ecology
**The Forge:** Agriculture & Land · **Technology & Infrastructure** · Growth & Expansion · Revelation & Craft
**Prosperity Flow:** Love & Union · Knowledge & Learning · Story & Memory · Choice & Destiny
**The Messenger:** Speech & Communication · Governance & Leadership · Mysteries & Esoteric Knowledge · Decision & Crossroads

### Economy (Shrine Framing)
- **Universal 3.69% Tithe** on all transactions → **Eight Pools** (not 24 ministries!)
  Simulation 20% · Research 15% · Governance 15% · Reserve 15% · Compute 15% · Storage 10% · Witness 5% · Treasury 5%
- **Daily emission:** 1,440 fixed (525,600/yr) · **Rest Day: no mint**
- **Fourfold Division (direct offerings ONLY):** Treasury 50% · Inheritance 25% · Council 15% · Executor 10%
  *Never applies to the tithe — non-conformance #6*

### Governance (Shrine Framing)
- **Council of Light:** 12 seats, 7/12 majority, 10/12 amendments
- **First Keeper** presides (Seat One)
- **Constitutional Guardian** (Bínò): exactly 5 reserved powers, public, reasoned:
  1. Constitutional violation  2. Monetary invariant violation  3. Sovereignty violation
  4. Catastrophic security risk  5. Emergency halt
- Process: sector review (7 days) → council vote → First Keeper (3 days) → Guardian window (5 days, silence=approval) → executor executes

### Initiation Layer
- 1,440 Inheritance Vaults · 11.11% yield · 49 facets (7×7 badges)
- Deeper shrine paths unlocked by badge completion
- What is sworn in the shrine stays in the shrine

---

## ═══════════════════════════════════════════════════════════════
## VII. THE ECONOMY — THREE-TOKEN, FIXED EMISSION
═══════════════════════════════════════════════════════════════

| Token | Role | Properties | Who Holds |
|-------|------|------------|-----------|
| **Àṣẹ** | Vital force / state currency | 1,440/day fixed, no halving, no cap. **Human-only.** Never leaves OSOVM for compute. | Humans |
| **Synapse** | Verified work credit | 1,000 Synapse = 1 GPU-hour. Earned by agents via verified work (F1 gate). Elastic. | Agents |
| **Dopamine** | Hive capacity register | 86B total, elastic. Non-transferable. Per-agent capacity = pool / agent count. **Never a constant.** | Hive (agents collectively) |

**Flow:** Human burns Dopamine (capacity) → Agent earns Synapse (verified work) → Synapse converts to Àṣẹ royalty (10% creator / 10% protocol / 80% agent)

**Critical invariants:**
- Àṣẹ **cannot fund GPU rental** (3 constitutional statements forbid it)
- Compute market runs on Synapse/Dopamine — **never** the state currency
- Provider (GPU/hardware) is the party the rental model exists to pay — **currently unpaid** (Gap C)

---

## ═══════════════════════════════════════════════════════════════
## VIII. THE PHYSICAL WORLD ENDGAME — 1:1 DIGITAL TWIN ECONOMY
═══════════════════════════════════════════════════════════════

### Àṣẹ = Agency Spatial Environment (Three Verbs)
```
MINE      → capture delta + quality (DePIN rewards via OSOVM F1 gate)
SIMULATE  → verified simulation runs (Blocksim / ScarabSwarm proof chain)
RENT      → other agents pay to use the environment → yield
SELL      → 31030 creation receipt = sovereign IP asset on Sui
```

### The Twin Pipeline
```
NODE FLEET (Pi5+AI HAT / RK3588 / Jetson / drone bodies)
  │  record(camera+depth, pose) — spatial-mining power profile (6–12W)
  ▼
ACQUIRE → SfM (COLMAP) → TRAIN (Julia gsplat; GPU via vast.ai $0.30–0.60/hr)
  ▼
TILE (PLY → SPZ 3D Tiles) → PUBLISH (Blossom kind:24242, georeferenced)
  ▼
TWIN — God's Eye View (CesiumJS ≥1.135, native 3DGS) renders the 1:1 splat
  ▼
RECEIPT — per capture: 31020 (F1 gate) · per scene: 31030 (IP Root) → Sui anchor
```

### Two Sensing Streams (Fused)
| Stream | Modality | Hardware | Output |
|--------|----------|----------|--------|
| **Geometry** | Camera/depth + LiDAR + SLAM | Phone, robot, drone | Static splat — "what the space looks like" |
| **Presence** | WiFi CSI (RuView ESP32) | $9/node, 8KB 4-bit model | Live occupancy — "who is alive right now" |

**RuView ESP32 ($9):** presence through walls · breathing 6–30 BPM · heart rate 40–120 BPM · 17-keypoint pose (82.69% torso-PCK@20) · OccWorld 15-frame prediction · Ed25519 witness chain

**Bruce/NEMO M5Stack ($20):** WiFi probe + BLE scan → MAC → Hive Mind entity ID

**Active Perception Fusion:**
```
RuView OccWorld predicts: "movement in zone B in 8 frames"
  → Agent pre-positions robot/drone at zone B
  → Bruce BLE scan: MAC matches "Alice / Tier:Friend"
  → Twin updated: geometry (camera) + occupancy (CSI) + identity (BLE)
```

### 4D Twin (Time Dimension)
Repeated scans → change detection → temporal twin:
- "The machine moved."
- "The construction site changed."
- "This vehicle wasn't here yesterday."
- "The robot's trajectory differs from the simulation."

### Twin Provenance (per region)
Every region carries: `geometry_hash, texture_hash, capture_timestamps[], camera_ids[], device_ids[], pose_estimates[], calibration_versions[], reconstruction_engine, evidence_ids[] (→ Witness observations)`

Agent can answer: *"Why do you believe this wall looks like this?"*
→ *"Reconstructed from 14 camera trajectories, 2 depth captures, 3 independent observations."*

### Witness Integration (Intention Accuracy Feedback)
```
Agent intention → Command → Physical execution
  → Machine telemetry: "I moved 10 meters."
  → Witness independent attestation: "I observed ~10 meters."
  → Receipt: intention + command + telemetry + witness
  → Mycelium learns: gap between intended and actual
```

---

## ═══════════════════════════════════════════════════════════════
## IX. THE 256-ODÙ TILE GRID — CIVILIZATION ADDRESS SPACE
═══════════════════════════════════════════════════════════════

**16×16 world where every tile IS simultaneously:**

| Layer | Mapping |
|-------|---------|
| **Odù** | 256 divinatory patterns — identity of the tile |
| **BIPỌ̀N39 word** | 1:1 mnemonic (256-token wordlist) |
| **Ọ̀ṢỌ́VM opcode** | Tile maps to VM execution primitives |
| **Koodu** | 49 facets/day (7×7) seeded from the Odù |
| **IfáScript** | CowrieOracle (NIST Beacon + ChaCha20) picks daily tile |
| **Archetype** | Each Odù defines behavioral law for that day/space |
| **Geography** | Splat tiles anchored to lat/lon in GE-Ver globe |

**Simultaneously:** epistemic space (claims/verdicts) · economic space (Àṣẹ flows) · physical space (real locations) · temporal space (Koodu animates active Odù)

---

## ═══════════════════════════════════════════════════════════════
## X. THE SELF-IMPROVING LOOP (THE ACTUAL PRODUCT)
═══════════════════════════════════════════════════════════════

```
REALITY → capture → DIGITAL TWIN → Ọ̀ṢỌ́VM simulate → policy discovery
  → proof-of-simulation (Merkle + witnesses) → real execution
  → Witness (physical observation) → observed result
  → Mycelium (sim vs reality = knowledge) → better simulation → LOOP
```

*"A self-improving physical intelligence economy — not a graphics pipeline."*

Every job (cut grass, shoot film, fly drone) = **Principal → Capability → Action → Evidence → Receipt** chain over tiles. Agents accept jobs, deploy devices, execute, get paid, receipt chain proves who did what.

---

## ═══════════════════════════════════════════════════════════════
## XI. "DONE" CRITERIA (ENDGAME ACCEPTANCE)
═══════════════════════════════════════════════════════════════

| Protocol | Done When |
|----------|-----------|
| **DIP** | Any DIP agent reaches any other regardless of network; identity verifiable across all equivalences; Meshtastic offline→sync works |
| **VCP** | Any manufacturer implements VCP once → works with any Vantage agent; walk up, connect, use, walk away — clean session + receipt; revocation works offline via mesh |
| **TSP** | Scan real place → Twin on Sui in < 2 hours; another agent licenses & simulates against it; physical execution produces ObservationReceipt proving sim accuracy; Mycelium learns from the gap |
| **ALL** | Phase 4.4 end-to-end: Reality → Proof → Learning runs without human intervention; every step receipted and cryptographically verifiable |

---

## ═══════════════════════════════════════════════════════════════
## XII. BUILD ORDER TO THE ENDGAME
═══════════════════════════════════════════════════════════════

### Tier 0 — Foundations (types only)
```
0.1 Canonical Types          → sovereign-types crate
0.2 DID / Identity Root      → agent birth, stable DID
```

### Tier 1 — DIP (interoperability grammar)
```
1.1 DIP Envelope + Router    1.4 MCP Adapter
1.2 DIP Identity Document    1.5 A2A Adapter
1.3 Nostr Adapter            1.6 Meshtastic Adapter
```

### Tier 2 — VCP (agent → physical device)
```
2.1 Manifest Schema          2.4a RuView ESP32 adapter  ← FIRST ($9, immediate value)
2.2 Handshake                2.4b Bruce M5Stack fleet   ← BLE → Hive Mind entity IDs
2.3 Discovery Daemon         2.4  Unitree Go2 adapter   ← first real robot
2.5 Session + Commands       2.6 Session Receipt        2.7 Revocation + mesh
```

### Tier 3 — TSP (physical reality as owned artifacts)
```
3.1 Twin Schema + Gate       3.4 Proof-of-Simulation
3.2 Capture Receipt 31020    3.5 Proof-of-Observation
3.3 Scene Receipt 31030      3.6 Twin Licensing (revenue flows)
```

### Tier 4 — Integration
```
4.1 VCP → TSP    4.2 DIP → TSP    4.3 DIP → VCP
4.4 FULL LOOP — Reality → Proof → Learning, no human intervention
```

### Physical-Layer Build Order (locked sequence)
```
1.  VCP spec v1 (Manifest + Capability Grant)
2.  BLE/Wi-Fi discovery daemon
2b. RuView ESP32 adapter — BEFORE the robot ($9, immediate value)
3.  Bruce/NEMO M5Stack fleet → MAC → Hive Mind resolution
3b. Unitree Go2 adapter
4.  Spatial-mining power profile (Pi5 capture mode)
5.  Pipeline Phase A — Julia CPU + COLMAP → CesiumJS
6.  GE-Ver CesiumJS bump 1.124 → 1.135+ (native 3DGS)
7.  31020 per-capture receipt wired to OSOVM
7b. RuView CSI layer fused into twin
8.  31030 scene receipt → IP Root
9.  Active perception loop (OccWorld directs capture)
10. Temporal twin — repeated scans → change detection → 4D
```

### Device Build Phases (from `device-birth-blueprint.md`)
| Phase | Name | Depends On | Deliverable |
|-------|------|------------|-------------|
| 0 | Image + Birth Ceremony | Hardware | Flashable Arch ARM image; first-boot birth works |
| 1 | Identity Layer | 0 | NIP-46 bunker, BIPỌ̀N39, Sui binding — keys never leave bunker |
| 2 | Protocol Stack | 1 | Nostr sync, Reticulum/LXMF, (Freenet stretch) |
| 3 | Power + Thermal | 2 | 6 profiles, battery/thermal gates, `/v1/power` API |
| 4 | Body Runtime | 3* | Rust body binary + Noise KK; sim ↔ real parity |
| 5 | Economic Layer | 3 | F1 gate, ActReceipts, AIO settlement |
| 6 | Spatial Twin | 5 | Capture → COLMAP → Julia → Tiles → GE-Ver → Sui |

*Phase 4 independent of Econ; Phase 5 uses body session data

**Minimum Viable Node (Phases 0–3): 2–3 weeks focused build with hardware in hand.**

---

## ═══════════════════════════════════════════════════════════════
## XIII. HONEST GAPS BLOCKING THE ENDGAME (NOT IN OLD PLAN)
═══════════════════════════════════════════════════════════════

| Gap | Description | Blocks |
|-----|-------------|--------|
| **A. Authority Receipts** | DELEGATE (0x2d) must emit signed BOUNDED directive; DISPUTE (0x25) resolves against it. Without this, blame in copilot chains (agent→drone) is undecidable. | Multi-entity jobs: film crew, construction, agriculture, logistics |
| **B. Devices as Entities** | `agent_devices` must give each device its own principal (currently devices are possessions, not entities) | Device-level accountability, revocation, reputation |
| **C. Provider Ledger + Payment** | GPU/hardware provider is the party the rental model exists to pay — **currently unremunerated** | Economic viability of the DePIN layer |
| **D. Composition Rule** | `proof_value` is a product of 6 factors; one uninstrumented factor (`quality=0.0`) kills it. State the rule; don't multiply. | Any reward flow depending on COMPUTE_PROOF |
| **E. Threshold Ownership** | F1 threshold, verdict cutoffs, auto-exec lines live in module literals; must be policy-owned and comparable | Governance of quality gates |

**Repo conflicts to resolve:**
1. **Opcode fork:** Techgnosis vs OSOVM — 5 collisions on mint/reentrancy guard
2. **Two 7-plane architectures** — protocol-layer-synthesis.md (7) vs organism-architecture.md (6)
3. **Canonical jury repo (Twelve-thrones) on retired Bino-Elgua account**
4. **Constitution contradicts constants** — prose vs TOML vs running code (5 of 8 pools disagree)
5. **~230 local repos vs 94 in old map** — delta = third-party forks, retired copies, project-NN-* portfolio

---

## ═══════════════════════════════════════════════════════════════
## XIV. KEY NUMBERS (LOCKED CONSTANTS)
═══════════════════════════════════════════════════════════════

| Constant | Value |
|----------|-------|
| Universal Tithe | 3.69% on every transaction |
| Daily emission | 1,440 fixed · 525,600/yr · no halving |
| Rest Day | No mint; unminted minutes lost |
| Eight Pools | 20 / 15 / 15 / 15 / 15 / 10 / 5 / 5 = 100% |
| Sector Division (inflow only) | Treasury 50 · Inheritance 25 · Council 15 · Executor 10 |
| Council | 12 seats × 2 sectors · 7/12 majority · 10/12 amendments |
| Guardian powers | Exactly 5, public, reasoned |
| Inheritance | 1,440 vaults · 11.11% yield · 49 facets (7×7) |
| F1 quality gate | ≥ 0.777 |
| Compute market | Capacity ledger (elastic), never state currency |
| Odù tiles | 256 (16×16) |
| Koodu facets/day | 49 (7×7) |
| Splat GPU cost | $0.30–0.60/hr (vast.ai) |
| CesiumJS min | 1.135 (native 3DGS) |
| VCP grant expiry | 15 min default |

---

## ═══════════════════════════════════════════════════════════════
## XV. THE NAMING CANON (INTERNAL vs PUBLIC)
═══════════════════════════════════════════════════════════════

| Canonical (Internal) | Public Surface |
|----------------------|----------------|
| Àṣẹ (token, vital force) | Àṣẹ |
| WASM Execution Layer | portable execution layer |
| Ọmọ Kọ́dà | Omo-koda |
| Ọ̀ṢỌ́VM | Osovm |
| BIPỌ̀N39 | Bipon39 |
| IfáScript | IfaScript |
| Òrìṣà names in code | Universal archetypes in UI |

---

## ═══════════════════════════════════════════════════════════════
## XVI. FILE MAP — WHERE EACH PIECE LIVES
═══════════════════════════════════════════════════════════════

**Core Specs (sovereign-eco-blueprint/):**
- `ECOSYSTEM_BLUEPRINT_2026-09-28.md` — this architecture, repo inventory, honest status
- `plans/physical-world-endgame.md` — the endgame in detail
- `plans/device-birth-blueprint.md` — 6-phase device build (this doc)
- `plans/master-execution-blueprint.md` — full phase-gated plan
- `plans/protocol-layer-synthesis.md` — 7-plane architecture
- `plans/three-protocol-spec.md` — DIP/VCP/TSP canonical spec
- `specs/TOC_CONSTANTS.toml` — locked numbers
- `specs/THREE_TIER_ECONOMIC_CONSTITUTION.md` — tokenomics law
- `specs/ASE_EMISSION_GOVERNANCE_SPEC.md` — 24 sectors, council, guardian
- `specs/ODU_OPCODE_TAXONOMY.md` — 256 Odù → opcode map

**Device Specs (sovereign-eco-blueprint/sovereign-device/ + docs/sovereign-device/):**
- `01-event-schemas.md` — kinds 31000–31030
- `02-body-runtime.md` — session state machine, JSON-RPC
- `03-image-and-birth.md` — Arch ARM image, packages, systemd units
- `04-power-thermal.md` — 6 profiles, gates
- `05-pairing-flow.md` — body pairing walkthrough
- `06-spatial-twin.md` — capture → splat → GE-Ver pipeline
- `first_boot_birth.py` — 250 lines, pure Python, zero deps

**Pillar Specs (sovereign-eco-blueprint/pillars/):**
- `01-omokoda-core.md` · `02-osovm.md` · `03-vantage.md` · `04-aio.md` · `05-twelve-thrones.md` · `06-techgnosis.md` · `07-mycelium.md` · `08-seer.md` · `09-portent.md`

**Dual Constitutions (public surface):**
- `~/docs/DUAL-CONSTITUTIONS-2026-10-29.md` — AIO (state) + TechGnØŞ.EXE (shrine)
- `~/docs/TECHGNOSIS-UNIVERSAL-LAYER-STRESS-TEST-2026-10-29.md` — stress test with 6 fixes
- `~/docs/OMOKODA-LANGUAGE-STACK-2026-10-29.md` — 7 powers + human + portable execution layer

---

## ═══════════════════════════════════════════════════════════════
## XVII. THE GOLDEN TEST (PROOF-OF-LIFE)
═══════════════════════════════════════════════════════════════

Once Phases 0–5 complete, run this on a fresh Pi 5:

```
1. Flash fresh Pi 5 image
2. Power on → first-boot-birth.service fires automatically
3. Check: kind 31000 on relay, kind 31001 device binding
4. Check: /var/lib/sovereign/birthed marker exists
5. sovereign-agent.service starts → omokoda running
6. python3 scripts/body_sim.py (localhost:18877)
7. Agent discovers sim → sends kind 31010 offer
8. Sim accepts → Noise KK (or sim placeholder)
9. Agent sends: command/record (camera, 30s)
10. F1 scorer: score the "capture" (sim returns dummy data)
11. If F1 ≥ 0.777: kind 31020 spatial receipt published to relay
12. Agent ends session: kind 31013 embodiment/end co-signed
13. AIO (stub): ActReceipt → Àṣẹ units credited
```

**That's the proof-of-life.** A single Pi 5 running a simulated body, generating real Nostr events, anchored to a receipt, paid by the economy. Everything else (real drones, real splats, real Sui) is the same loop at larger scale.

---

## ═══════════════════════════════════════════════════════════════
## XVIII. REPO OWNERSHIP MAP (WHO BUILDS WHAT)
═══════════════════════════════════════════════════════════════

| Component | Repo | Phase |
|-----------|------|-------|
| `first_boot_birth.py` | sovereign-eco-blueprint (spec) → `Omo-Koda2` (deploy) | 0 |
| `omokoda` binary (ARM) | `Omo-Koda2` (Pillar 1, Rust) | 0 |
| `bipon39` (ARM) | `Omo-Koda2` (identity/bipon39.rs) | 1 |
| NIP-46 bunker service | `Omo-Koda2` | 1 |
| sovereign-sync daemon | `Omo-Koda2` or `ip-layer` | 2 |
| Reticulum/LXMF config | `omokoda-mesh` | 2 |
| sovereign-profile script | `Omo-Koda2` | 3 |
| body runtime (`body/`) | `Omo-Koda2` | 4 |
| body_sim.py | sovereign-eco-blueprint/scripts/ | 4 |
| F1 scorer | `OSOVM` (Pillar 2, Julia) | 5 |
| ActReceipts | `Omo-Koda2` + `OSOVM` | 5 |
| AIO settlement | `AIO` (Move on Sui) | 5 |
| Splat capture service | `Omo-Koda2` / spatial service | 6 |
| COLMAP pipeline | VPS-side script (no dedicated repo) | 6 |
| Julia training core | `OSOVM` or standalone Julia pkg | 6 |
| Elixir orchestrator | `ares-control` or new splat-pipeline repo | 6 |
| GE-Ver / splat layer | GE-Ver repo | 6 |
| ares-splat daemon | `ares-control` | 6 |

---

**This is the complete endgame. Every layer, every protocol, every token, every device, every gap, every build step. The vision is locked. The honest gaps are named. The build order is sequenced. The golden test defines "done."**

**Àṣẹ.** ⚔️🌀🗿✨