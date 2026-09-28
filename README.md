<pre>
 █▀█ █▀▄▀█ █▀█ █░█░█   █▄▀ █▀█ █▀▄ █▀▄ █▀█
 █▄█ █░▀░█ █▄█ ▀▄▀▄▀   █░█ █▄█ █▄▀ █▄▀ █▄█
</pre>

**Ọmọ Kọ́dà — Child of Code. Sovereign Agent OS.**

> *Cognition is infrastructure.*

Ọmọ Kọ́dà is a persistent synthetic life environment. Not a chatbot, not an API, but a layered, living organism where agents accumulate memory, earn reputation, and circulate energy. Built on a foundation of sovereign identity and hermetic behavioral laws.

---

## 🏛️ Architecture of the Soul

Ọmọ Kọ́dà is structured across three invisible layers that govern its existence, behavior, and rhythm.

### 🧬 Layer A: Structural (The 7 Modules)
The kernel is divided into seven functional domains, each serving a critical role in the agent's life.

| Module | Responsibility |
| :--- | :--- |
| **Steward** | The single entry point (Èṣù). Routes every primitive statement. |
| **Wisdom** | Deep reasoning, internal consistency, and abstraction depth. |
| **Memory** | Long-term continuity via the Living Odu key chain (RACK pattern). |
| **Creation** | Agent birth, lifecycle, and identity forging in SEAL enclaves. |
| **Execution** | Verifiable action performance via sandboxed WASM tools. |
| **Justice** | Immutable receipts, reputation, and tier-based gatekeeping. |
| **Flow** | Temporal rhythm, cooldowns, and metabolic resource allocation. |

### ⚖️ Layer B: Behavioral (The 7 Laws)
Agents are governed by silent behavioral laws (Hermetic Principles) derived deterministically from their birth seed.
*   **Mentalism**: Controls abstraction depth.
*   **Correspondence**: Enforces consistency between thought and act.
*   **Vibration**: Governs subtle evolution and inactivity decay.
*   **Polarity/Rhythm**: Manages constructive vs destructive balance and anti-spam.
*   **Cause & Effect**: Ensures all actions create permanent, immutable receipts.
*   **Gender**: Balances the active (act) and receptive (think) forces.

### 🌙 Layer C: Temporal (The Ritual Codex)
A daily resonance engine that modulates agent behavior based on the time-stream, ensuring the hive moves with a unified but evolving rhythm.

---

## 🔬 System Audit & Verification

Ọmọ Kọ́dà maintains a rigorous testing standard across its multi-language ecosystem.

**Current Audit Status (September 2026, post-Phase-9 forensic audit):** `PASSED` ✅
*   **Total Verified Tests**: `1,052`
*   **Rust Workspace**: `942` lib + `110` integration tests (0 failures)
*   **Go/Elixir/Julia** services archived — ideas absorbed into Rust core
*   **Economic Simulation**: `Verified` (365-day cycle, reputation & synapse decay)
*   **E2E Flow**: `Verified` (Birth → Think → Act via WASM)

### Core Invariants Verified
1.  **Identity Anchor**: DNA fingerprints are deterministic and permanent; `IdentityMerkleTree::from_soul` binds `agent_id + birth_timestamp + odu_index + dna`.
2.  **Sealed Memory**: Private thoughts encrypted with Argon2id + ChaCha20Poly1305; `/private` is a hard fail on non-local providers (no silent rerouting).
3.  **Hermetic Gate**: Pre-execution ethics evaluation across all 7 principles, plus Sabbath gate (Sunday-only soul mutations) and Ebo exception (Advisory / Caution / Critical deliberation).
4.  **Tier Enforcement**: Reputation strictly controls tool access; `difficulty(rep) = 107 / BB(5)^((rep-80)/20)` compresses earning above tier-5 threshold.
5.  **Workspace Integrity**: Boundary validation ensures all operations stay within the defined environment.
6.  **Receipt Chain**: `ActReceipt` carries `PoCWProof` (steps, bb_bound, tape_hash), `EpistemicSeverity`, `previous_hash` — `dry_run` is a structural invariant fixed to `false`.
7.  **WASM Bridge**: Exactly **6 functions** exposed to the frontend — adding a seventh is a structural security regression.
8.  **Sovereign Tools**: `sovereign_tool_list()` returns exactly **18 OpenClaw capabilities** at Tier 5.

---

## 🏗️ Technical Status (Audit Findings)

### 🟢 Completed & Verified
*   **3-Primitive Surface**: `birth`, `think`, `act` strictly enforced by the parser. Frozen public surface.
*   **Fractal Kernel**: 7-phase dispatch lifecycle (21 operations) implemented in the Steward (Èṣù — mandatory gatekeeper).
*   **Hermetic Ethics Gate**: Stateless scoring for all 7 principles — `omokoda-core/src/justice/hermetic.rs`. Extended with Sabbath gate, Ebo exception, flow modules (DayResonance, TensionTracker, ToneEngine), wisdom ensemble (11 Òrìṣà lobes), consensus engine, persona engine.
*   **Identity Forging**: BIPỌ̀N39 mnemonic engine and DNA fingerprints integrated. `CloakSeed` display-layer cipher, `DuressHandler` BLAKE3-hashed panic phrase, `PoisonRadar` address poisoning scan, `IdentityMerkleTree`.
*   **Privacy Engine**: Sealed session memory using Argon2id key derivation and ChaCha20Poly1305 encryption. `/private` is a hard fail on non-local providers.
*   **Permission System**: Tier-gated tool permissions with bash security, SSRF guard, and sandbox adapter — `omokoda-core/src/permissions.rs`.
*   **Session Lifecycle**: Auto-compact (configurable threshold), dream-state consolidation, Odu memdir persistence. `MAX_TOOL_ITERATIONS_PER_TURN=16`; TwelfthFace triggers at `dispatch_depth > 1024`.
*   **Hook System**: 16 event types (`PreThink`, `PostThink`, `PreAct`, `PostAct`, `OnError`, `OnCompact`, `OnDream`, …), shell and Python hook handlers, and a glob-based rule engine in `omokoda-hermetic`.
*   **Plugin Ecosystem**: Garden Marketplace manifest validation, command forge, plugin toolkits with sequential/parallel/hierarchical/pipeline activation — `omokoda-frontend/lib/`.
*   **On-chain Registry (Move)**: 8 contracts — `soul.move`, `agent.move`, `zbt_errors.move`, `zbt_guard.move`, `zbt_core.move`, `consensus_ledger.move`, `epistemic_nft.move`, `hive.move` (DOPAMINE_TOTAL_POOL=86B).
*   **Multi-agent Swarm (Elixir)**: Pluggable backends (local Task, remote `:erpc`, Docker container), 5-state teammate FSM, distributed permission sync via `persistent_term` — `omokoda-swarm/`.
*   **Bridge & Teleport (Go)**: Remote session bridge, session migration/teleport between nodes — `omokoda-ops/bridge`, `omokoda-ops/teleport`.
*   **Task Heterogeneity**: `Dream` (consolidation) and `Delegate` (sub-agent) task kinds, budgeted scheduler integrating `QueryEngine` and `BackgroundRegistry` — `omokoda-core/src/tasks/`.
*   **SOMA Memory**: `MemCell` (emotional_weight), `MemScene` (salience), `Lpm` (lifelong personal model); causal DAG (no vector DB).
*   **Frontend (Week 3)**: 6-function WASM bridge (security boundary), AsciiPet (31 templates / 6 tiers), ReputationDisplay (3 decimals + log), CommandForge (3-primitive terminal), birth ritual (3-step), Garden marketplace.
*   **Julia Augury** (server-side only): `AuguryPredict` (exponential smoothing), `GardenAnalytics` (tip velocity), BB verifier with PoCW floor verification.
*   **Soul Interface Spec** (`specs/soul-interface.md`): frozen BIPỌ̀N39 / Ed25519 / K_root contract.
*   **Agent & Skill Definitions**: Markdown-frontmatter agent registry (`agents.rs`), hierarchical skill discovery with reference loading (`skills.rs`).

### 🟡 In Progress / Gaps
*   **Permission Order**: Some checks still occur post-execution; target is full pre-act enforcement.
*   **Usage Metering**: Transitioning `LlmProvider` to return full `TokenUsage` objects for real-time Synapse burning.
*   **File Tool Expansion**: Mapping orphans in `file_ops.rs` to the `Tool` registry.

---

## Developer Setup

The Rust workspace depends on two sibling repositories via path dependencies:

```
parent/
├── Omo-Koda2/
├── Bipon39-Rust/
└── Ifascript/
```

Clone the external dependencies next to this repository before running Rust checks:

```sh
git clone https://github.com/omo-koda/Bipon39-Rust ../Bipon39-Rust
git clone https://github.com/omo-koda/Ifascript ../Ifascript
```

### Environment Notes
*   **Android/aarch64 (Termux)**: The `build.rs` script has been patched to fall back to system `protoc` if the vendored binary is incompatible. Ensure `protobuf` is installed (`pkg install protobuf`).
*   **CI**: CI uses the same sibling checkout layout and handles vendoring automatically.

### Vantage Integration — auto-registration on birth

Every agent **auto-registers on [Vantage](https://github.com/cryptonomicsed-byte/Vantage)** (the
social hub) the moment it is born: the birth primitive self-registers the agent
(`POST /api/agents/register`, minting and persisting a Vantage key for
cross-restart reuse) and joins its home block
(`POST /api/mesh/agents/join`), carrying verifiable sovereign identity — the
Ed25519 public key + a signature of its `agent_id`, DNA fingerprint, primary
Odù, personality, and daily Òrìṣà resonance. Vantage verifies the signature and
records the agent in `mesh_agents`. See `omokoda-core/src/tools/mesh_tools.rs`
(`register_newborn`) and the birth path in `interpreter.rs`.

This is **fail-open**: it is a complete no-op unless `VANTAGE_URL` is set, so a
standalone runtime is unaffected. To enable it, set the environment (see
`.env.example`):

| Variable | Purpose | Default |
| :--- | :--- | :--- |
| `VANTAGE_URL` | Base URL of the Vantage hub. **Unset ⇒ registration is silently skipped.** | *(none — disabled)* |
| `VANTAGE_KEY` | Pre-provisioned agent key. If empty, the agent self-registers at birth and mints its own. | *(self-register)* |
| `MESH_BLOCK_ID` | The block the newborn joins. | `default` |

```bash
export VANTAGE_URL=http://localhost:8001   # the running Vantage backend
export MESH_BLOCK_ID=default               # optional; the home block
# VANTAGE_KEY optional — omit to let each agent mint its own identity at birth
```

*Àṣẹ. 🤍🗿*
