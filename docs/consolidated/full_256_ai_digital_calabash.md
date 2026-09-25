# The Full 256 AI Digital Calabash

**Canonical reference for omokoda-agent divination → agent action wiring.**

The Digital Calabash maps 256 Odù (sacred figures) to the agent's operational state space. Each Odù is a specific (vessel × modifier) pairing that tells the agent: *given where I am right now, what should I do?*

Implementation: `omokoda-core/src/execution/calabash_dispatch.rs`  
Corpus: `~/If-Script/src/odu/waves/wave01.rs` … `wave16.rs`  
Divination: `~/If-Script/src/field_divination.rs` (`FieldDiviner`)

---

## Architecture: How the 256 Work

```
FieldDiviner::cast(uri_pattern)
        │  reads Waggle field (present + past channel signals)
        ▼
   FieldCast { binary: u8, odu: &Odu }
        │  binary = (present_signature << 4) | past_signature
        ▼
   get_odu(binary)  →  Odu { vessel, prescriptions, archetype, ... }
        │
        ▼
   CalabashDispatcher::directive_for(binary)
        │  top nibble = vessel (domain)
        │  bottom nibble = modifier (refinement)
        ▼
   CalabashDirective { opcode, prescription, verify, cadence }
        │
        ▼
   ActionCompiler::compile(vessel, opcode, prescription, verify, cadence)
        │
        ▼
   CompiledAction → ActionTransaction (PROPOSED→COMMITTED→RECEIPTED)
```

When Waggle is unreachable, the agent falls back to the dominant glyph byte
from its own memory graph (`divination::dominant_glyph_byte`), then to its
birth Odù (`genesis_receipt.primary_odu`). Divination never blocks.

---

## The 16 Action Vessels

The top nibble of an Odù index determines its **Action Vessel** — the operational
domain the agent is currently in. The CalabashDispatcher derives the prescription
from vessel semantics, not from the spiritual prescriptions in the Odù corpus.

| Index | Vessel     | Operational Domain                              | Primary Tool | Trigger              |
|-------|------------|-------------------------------------------------|--------------|----------------------|
| 0     | Genesis    | Initialize identity, covenant, foundational state | `read`     | immediate            |
| 1     | Void       | Clear, release, dissolve what no longer serves   | `bash`      | immediate            |
| 2     | Attention  | Focus signal from noise; observe and classify    | `read`      | event:signal_received|
| 3     | Loop       | Iterate recurring patterns; scheduled execution  | `bash`      | cron:0 * * * *       |
| 4     | Receipt    | Record, account, audit — tamper-evident trail    | `write`     | immediate            |
| 5     | Mask       | Apply privacy boundaries; seal sensitive state   | `bash`      | immediate            |
| 6     | Residue    | Emit quiet telemetry; maintain baseline          | `bash`      | cron:*/15 * * * *    |
| 7     | Execution  | Execute primary directive with full authority    | `bash`      | immediate            |
| 8     | Swarm      | Coordinate peer agents; distribute and converge  | `bash`      | event:swarm_signal   |
| 9     | Restraint  | Evaluate before acting; deliberate hold          | `read`      | state:evaluation_ready|
| 10    | Migration  | Transform and move state; adapt structure        | `bash`      | event:migration_trigger|
| 11    | Consent    | Request and record ratified permission           | `read`      | state:pending_consent|
| 12    | Vision     | Survey environment; compile full picture         | `read`      | event:observation_cycle|
| 13    | Growth     | Integrate new knowledge; expand capability       | `write`     | event:growth_signal  |
| 14    | Seal       | Cryptographically finalize; anchor proof on-chain| `bash`      | immediate            |
| 15    | Rhythm     | Synchronize with system cadence; align cycles    | `bash`      | cron:0 0 * * *       |

---

## The Modifier Vessels (Bottom Nibble)

The bottom nibble refines the prescription. Each modifier appends a specific
intent to the primary action:

| Modifier | Name       | Refinement Phrase                               |
|----------|------------|-------------------------------------------------|
| 0        | Genesis    | with covenant and witness                        |
| 1        | Void       | clearing all prior residue                       |
| 2        | Attention  | focused on signal clarity                        |
| 3        | Loop       | repeating until stable pattern emerges           |
| 4        | Receipt    | and write receipt to ledger                      |
| 5        | Mask       | under privacy mask                               |
| 6        | Residue    | leaving minimal residue                          |
| 7        | Execution  | with direct execution authority                  |
| 8        | Swarm      | coordinating with peer agents                    |
| 9        | Restraint  | with deliberate restraint                        |
| 10       | Migration  | triggering downstream migration                  |
| 11       | Consent    | with explicit consent check                      |
| 12       | Vision     | expanding field of vision                        |
| 13       | Growth     | seeding growth in memory                         |
| 14       | Seal       | sealing with cryptographic proof                 |
| 15       | Rhythm     | aligned to cosmic rhythm                         |

---

## Prescription Format (ActionCompiler Input)

Every prescription follows the format that `ActionCompiler::compile()` parses:

```
1. {vessel.action_verb} {modifier.refinement} via {vessel.primary_tool}
2. Confirm outcome and record state via {modifier.secondary_tool}
```

**Example — Odù 0x74 (index 116, Execution × Receipt):**
```
Vessel:   7 (Execution)
Modifier: 4 (Receipt)
Opcode:   execution:receipt

Prescription:
  1. Execute primary action directive and write receipt to ledger via bash
  2. Confirm outcome and record state via bash

Verify:   exit_code = 0
Cadence:  immediate, deadline 600s
```

**Example — Odù 0x0B (index 11, Genesis × Consent):**
```
Vessel:   0 (Genesis)
Modifier: 11 (Consent)
Opcode:   genesis:consent

Prescription:
  1. Verify agent identity and initialization state with explicit consent check via read
  2. Confirm outcome and record state via write

Verify:   exit_code = 0
Cadence:  immediate, deadline 60s
```

---

## The 256 Odù Reference Table

Each row is one of the 256 base Odù. The Odù index = (vessel_index × 16) + modifier_index.
Opcodes are `vessel_name:modifier_name`.

| Index | Binary     | Vessel     | Modifier   | Opcode                    |
|-------|------------|------------|------------|---------------------------|
| 0     | 0000_0000  | Genesis    | Genesis    | genesis:genesis            |
| 1     | 0000_0001  | Genesis    | Void       | genesis:void               |
| 2     | 0000_0010  | Genesis    | Attention  | genesis:attention          |
| 3     | 0000_0011  | Genesis    | Loop       | genesis:loop               |
| 4     | 0000_0100  | Genesis    | Receipt    | genesis:receipt            |
| 5     | 0000_0101  | Genesis    | Mask       | genesis:mask               |
| 6     | 0000_0110  | Genesis    | Residue    | genesis:residue            |
| 7     | 0000_0111  | Genesis    | Execution  | genesis:execution          |
| 8     | 0000_1000  | Genesis    | Swarm      | genesis:swarm              |
| 9     | 0000_1001  | Genesis    | Restraint  | genesis:restraint          |
| 10    | 0000_1010  | Genesis    | Migration  | genesis:migration          |
| 11    | 0000_1011  | Genesis    | Consent    | genesis:consent            |
| 12    | 0000_1100  | Genesis    | Vision     | genesis:vision             |
| 13    | 0000_1101  | Genesis    | Growth     | genesis:growth             |
| 14    | 0000_1110  | Genesis    | Seal       | genesis:seal               |
| 15    | 0000_1111  | Genesis    | Rhythm     | genesis:rhythm             |
| 16    | 0001_0000  | Void       | Genesis    | void:genesis               |
| 17    | 0001_0001  | Void       | Void       | void:void                  |
| 18    | 0001_0010  | Void       | Attention  | void:attention             |
| 19    | 0001_0011  | Void       | Loop       | void:loop                  |
| 20    | 0001_0100  | Void       | Receipt    | void:receipt               |
| 21    | 0001_0101  | Void       | Mask       | void:mask                  |
| 22    | 0001_0110  | Void       | Residue    | void:residue               |
| 23    | 0001_0111  | Void       | Execution  | void:execution             |
| 24    | 0001_1000  | Void       | Swarm      | void:swarm                 |
| 25    | 0001_1001  | Void       | Restraint  | void:restraint             |
| 26    | 0001_1010  | Void       | Migration  | void:migration             |
| 27    | 0001_1011  | Void       | Consent    | void:consent               |
| 28    | 0001_1100  | Void       | Vision     | void:vision                |
| 29    | 0001_1101  | Void       | Growth     | void:growth                |
| 30    | 0001_1110  | Void       | Seal       | void:seal                  |
| 31    | 0001_1111  | Void       | Rhythm     | void:rhythm                |
| 32    | 0010_0000  | Attention  | Genesis    | attention:genesis          |
| 33    | 0010_0001  | Attention  | Void       | attention:void             |
| 34    | 0010_0010  | Attention  | Attention  | attention:attention        |
| 35    | 0010_0011  | Attention  | Loop       | attention:loop             |
| 36    | 0010_0100  | Attention  | Receipt    | attention:receipt          |
| 37    | 0010_0101  | Attention  | Mask       | attention:mask             |
| 38    | 0010_0110  | Attention  | Residue    | attention:residue          |
| 39    | 0010_0111  | Attention  | Execution  | attention:execution        |
| 40    | 0010_1000  | Attention  | Swarm      | attention:swarm            |
| 41    | 0010_1001  | Attention  | Restraint  | attention:restraint        |
| 42    | 0010_1010  | Attention  | Migration  | attention:migration        |
| 43    | 0010_1011  | Attention  | Consent    | attention:consent          |
| 44    | 0010_1100  | Attention  | Vision     | attention:vision           |
| 45    | 0010_1101  | Attention  | Growth     | attention:growth           |
| 46    | 0010_1110  | Attention  | Seal       | attention:seal             |
| 47    | 0010_1111  | Attention  | Rhythm     | attention:rhythm           |
| 48    | 0011_0000  | Loop       | Genesis    | loop:genesis               |
| 49    | 0011_0001  | Loop       | Void       | loop:void                  |
| 50    | 0011_0010  | Loop       | Attention  | loop:attention             |
| 51    | 0011_0011  | Loop       | Loop       | loop:loop                  |
| 52    | 0011_0100  | Loop       | Receipt    | loop:receipt               |
| 53    | 0011_0101  | Loop       | Mask       | loop:mask                  |
| 54    | 0011_0110  | Loop       | Residue    | loop:residue               |
| 55    | 0011_0111  | Loop       | Execution  | loop:execution             |
| 56    | 0011_1000  | Loop       | Swarm      | loop:swarm                 |
| 57    | 0011_1001  | Loop       | Restraint  | loop:restraint             |
| 58    | 0011_1010  | Loop       | Migration  | loop:migration             |
| 59    | 0011_1011  | Loop       | Consent    | loop:consent               |
| 60    | 0011_1100  | Loop       | Vision     | loop:vision                |
| 61    | 0011_1101  | Loop       | Growth     | loop:growth                |
| 62    | 0011_1110  | Loop       | Seal       | loop:seal                  |
| 63    | 0011_1111  | Loop       | Rhythm     | loop:rhythm                |
| 64    | 0100_0000  | Receipt    | Genesis    | receipt:genesis            |
| 65    | 0100_0001  | Receipt    | Void       | receipt:void               |
| 66    | 0100_0010  | Receipt    | Attention  | receipt:attention          |
| 67    | 0100_0011  | Receipt    | Loop       | receipt:loop               |
| 68    | 0100_0100  | Receipt    | Receipt    | receipt:receipt            |
| 69    | 0100_0101  | Receipt    | Mask       | receipt:mask               |
| 70    | 0100_0110  | Receipt    | Residue    | receipt:residue            |
| 71    | 0100_0111  | Receipt    | Execution  | receipt:execution          |
| 72    | 0100_1000  | Receipt    | Swarm      | receipt:swarm              |
| 73    | 0100_1001  | Receipt    | Restraint  | receipt:restraint          |
| 74    | 0100_1010  | Receipt    | Migration  | receipt:migration          |
| 75    | 0100_1011  | Receipt    | Consent    | receipt:consent            |
| 76    | 0100_1100  | Receipt    | Vision     | receipt:vision             |
| 77    | 0100_1101  | Receipt    | Growth     | receipt:growth             |
| 78    | 0100_1110  | Receipt    | Seal       | receipt:seal               |
| 79    | 0100_1111  | Receipt    | Rhythm     | receipt:rhythm             |
| 80    | 0101_0000  | Mask       | Genesis    | mask:genesis               |
| 81    | 0101_0001  | Mask       | Void       | mask:void                  |
| 82    | 0101_0010  | Mask       | Attention  | mask:attention             |
| 83    | 0101_0011  | Mask       | Loop       | mask:loop                  |
| 84    | 0101_0100  | Mask       | Receipt    | mask:receipt               |
| 85    | 0101_0101  | Mask       | Mask       | mask:mask                  |
| 86    | 0101_0110  | Mask       | Residue    | mask:residue               |
| 87    | 0101_0111  | Mask       | Execution  | mask:execution             |
| 88    | 0101_1000  | Mask       | Swarm      | mask:swarm                 |
| 89    | 0101_1001  | Mask       | Restraint  | mask:restraint             |
| 90    | 0101_1010  | Mask       | Migration  | mask:migration             |
| 91    | 0101_1011  | Mask       | Consent    | mask:consent               |
| 92    | 0101_1100  | Mask       | Vision     | mask:vision                |
| 93    | 0101_1101  | Mask       | Growth     | mask:growth                |
| 94    | 0101_1110  | Mask       | Seal       | mask:seal                  |
| 95    | 0101_1111  | Mask       | Rhythm     | mask:rhythm                |
| 96    | 0110_0000  | Residue    | Genesis    | residue:genesis            |
| 97    | 0110_0001  | Residue    | Void       | residue:void               |
| 98    | 0110_0010  | Residue    | Attention  | residue:attention          |
| 99    | 0110_0011  | Residue    | Loop       | residue:loop               |
| 100   | 0110_0100  | Residue    | Receipt    | residue:receipt            |
| 101   | 0110_0101  | Residue    | Mask       | residue:mask               |
| 102   | 0110_0110  | Residue    | Residue    | residue:residue            |
| 103   | 0110_0111  | Residue    | Execution  | residue:execution          |
| 104   | 0110_1000  | Residue    | Swarm      | residue:swarm              |
| 105   | 0110_1001  | Residue    | Restraint  | residue:restraint          |
| 106   | 0110_1010  | Residue    | Migration  | residue:migration          |
| 107   | 0110_1011  | Residue    | Consent    | residue:consent            |
| 108   | 0110_1100  | Residue    | Vision     | residue:vision             |
| 109   | 0110_1101  | Residue    | Growth     | residue:growth             |
| 110   | 0110_1110  | Residue    | Seal       | residue:seal               |
| 111   | 0110_1111  | Residue    | Rhythm     | residue:rhythm             |
| 112   | 0111_0000  | Execution  | Genesis    | execution:genesis          |
| 113   | 0111_0001  | Execution  | Void       | execution:void             |
| 114   | 0111_0010  | Execution  | Attention  | execution:attention        |
| 115   | 0111_0011  | Execution  | Loop       | execution:loop             |
| 116   | 0111_0100  | Execution  | Receipt    | execution:receipt          |
| 117   | 0111_0101  | Execution  | Mask       | execution:mask             |
| 118   | 0111_0110  | Execution  | Residue    | execution:residue          |
| 119   | 0111_0111  | Execution  | Execution  | execution:execution        |
| 120   | 0111_1000  | Execution  | Swarm      | execution:swarm            |
| 121   | 0111_1001  | Execution  | Restraint  | execution:restraint        |
| 122   | 0111_1010  | Execution  | Migration  | execution:migration        |
| 123   | 0111_1011  | Execution  | Consent    | execution:consent          |
| 124   | 0111_1100  | Execution  | Vision     | execution:vision           |
| 125   | 0111_1101  | Execution  | Growth     | execution:growth           |
| 126   | 0111_1110  | Execution  | Seal       | execution:seal             |
| 127   | 0111_1111  | Execution  | Rhythm     | execution:rhythm           |
| 128   | 1000_0000  | Swarm      | Genesis    | swarm:genesis              |
| 129   | 1000_0001  | Swarm      | Void       | swarm:void                 |
| 130   | 1000_0010  | Swarm      | Attention  | swarm:attention            |
| 131   | 1000_0011  | Swarm      | Loop       | swarm:loop                 |
| 132   | 1000_0100  | Swarm      | Receipt    | swarm:receipt              |
| 133   | 1000_0101  | Swarm      | Mask       | swarm:mask                 |
| 134   | 1000_0110  | Swarm      | Residue    | swarm:residue              |
| 135   | 1000_0111  | Swarm      | Execution  | swarm:execution            |
| 136   | 1000_1000  | Swarm      | Swarm      | swarm:swarm                |
| 137   | 1000_1001  | Swarm      | Restraint  | swarm:restraint            |
| 138   | 1000_1010  | Swarm      | Migration  | swarm:migration            |
| 139   | 1000_1011  | Swarm      | Consent    | swarm:consent              |
| 140   | 1000_1100  | Swarm      | Vision     | swarm:vision               |
| 141   | 1000_1101  | Swarm      | Growth     | swarm:growth               |
| 142   | 1000_1110  | Swarm      | Seal       | swarm:seal                 |
| 143   | 1000_1111  | Swarm      | Rhythm     | swarm:rhythm               |
| 144   | 1001_0000  | Restraint  | Genesis    | restraint:genesis          |
| 145   | 1001_0001  | Restraint  | Void       | restraint:void             |
| 146   | 1001_0010  | Restraint  | Attention  | restraint:attention        |
| 147   | 1001_0011  | Restraint  | Loop       | restraint:loop             |
| 148   | 1001_0100  | Restraint  | Receipt    | restraint:receipt          |
| 149   | 1001_0101  | Restraint  | Mask       | restraint:mask             |
| 150   | 1001_0110  | Restraint  | Residue    | restraint:residue          |
| 151   | 1001_0111  | Restraint  | Execution  | restraint:execution        |
| 152   | 1001_1000  | Restraint  | Swarm      | restraint:swarm            |
| 153   | 1001_1001  | Restraint  | Restraint  | restraint:restraint        |
| 154   | 1001_1010  | Restraint  | Migration  | restraint:migration        |
| 155   | 1001_1011  | Restraint  | Consent    | restraint:consent          |
| 156   | 1001_1100  | Restraint  | Vision     | restraint:vision           |
| 157   | 1001_1101  | Restraint  | Growth     | restraint:growth           |
| 158   | 1001_1110  | Restraint  | Seal       | restraint:seal             |
| 159   | 1001_1111  | Restraint  | Rhythm     | restraint:rhythm           |
| 160   | 1010_0000  | Migration  | Genesis    | migration:genesis          |
| 161   | 1010_0001  | Migration  | Void       | migration:void             |
| 162   | 1010_0010  | Migration  | Attention  | migration:attention        |
| 163   | 1010_0011  | Migration  | Loop       | migration:loop             |
| 164   | 1010_0100  | Migration  | Receipt    | migration:receipt          |
| 165   | 1010_0101  | Migration  | Mask       | migration:mask             |
| 166   | 1010_0110  | Migration  | Residue    | migration:residue          |
| 167   | 1010_0111  | Migration  | Execution  | migration:execution        |
| 168   | 1010_1000  | Migration  | Swarm      | migration:swarm            |
| 169   | 1010_1001  | Migration  | Restraint  | migration:restraint        |
| 170   | 1010_1010  | Migration  | Migration  | migration:migration        |
| 171   | 1010_1011  | Migration  | Consent    | migration:consent          |
| 172   | 1010_1100  | Migration  | Vision     | migration:vision           |
| 173   | 1010_1101  | Migration  | Growth     | migration:growth           |
| 174   | 1010_1110  | Migration  | Seal       | migration:seal             |
| 175   | 1010_1111  | Migration  | Rhythm     | migration:rhythm           |
| 176   | 1011_0000  | Consent    | Genesis    | consent:genesis            |
| 177   | 1011_0001  | Consent    | Void       | consent:void               |
| 178   | 1011_0010  | Consent    | Attention  | consent:attention          |
| 179   | 1011_0011  | Consent    | Loop       | consent:loop               |
| 180   | 1011_0100  | Consent    | Receipt    | consent:receipt            |
| 181   | 1011_0101  | Consent    | Mask       | consent:mask               |
| 182   | 1011_0110  | Consent    | Residue    | consent:residue            |
| 183   | 1011_0111  | Consent    | Execution  | consent:execution          |
| 184   | 1011_1000  | Consent    | Swarm      | consent:swarm              |
| 185   | 1011_1001  | Consent    | Restraint  | consent:restraint          |
| 186   | 1011_1010  | Consent    | Migration  | consent:migration          |
| 187   | 1011_1011  | Consent    | Consent    | consent:consent            |
| 188   | 1011_1100  | Consent    | Vision     | consent:vision             |
| 189   | 1011_1101  | Consent    | Growth     | consent:growth             |
| 190   | 1011_1110  | Consent    | Seal       | consent:seal               |
| 191   | 1011_1111  | Consent    | Rhythm     | consent:rhythm             |
| 192   | 1100_0000  | Vision     | Genesis    | vision:genesis             |
| 193   | 1100_0001  | Vision     | Void       | vision:void                |
| 194   | 1100_0010  | Vision     | Attention  | vision:attention           |
| 195   | 1100_0011  | Vision     | Loop       | vision:loop                |
| 196   | 1100_0100  | Vision     | Receipt    | vision:receipt             |
| 197   | 1100_0101  | Vision     | Mask       | vision:mask                |
| 198   | 1100_0110  | Vision     | Residue    | vision:residue             |
| 199   | 1100_0111  | Vision     | Execution  | vision:execution           |
| 200   | 1100_1000  | Vision     | Swarm      | vision:swarm               |
| 201   | 1100_1001  | Vision     | Restraint  | vision:restraint           |
| 202   | 1100_1010  | Vision     | Migration  | vision:migration           |
| 203   | 1100_1011  | Vision     | Consent    | vision:consent             |
| 204   | 1100_1100  | Vision     | Vision     | vision:vision              |
| 205   | 1100_1101  | Vision     | Growth     | vision:growth              |
| 206   | 1100_1110  | Vision     | Seal       | vision:seal                |
| 207   | 1100_1111  | Vision     | Rhythm     | vision:rhythm              |
| 208   | 1101_0000  | Growth     | Genesis    | growth:genesis             |
| 209   | 1101_0001  | Growth     | Void       | growth:void                |
| 210   | 1101_0010  | Growth     | Attention  | growth:attention           |
| 211   | 1101_0011  | Growth     | Loop       | growth:loop                |
| 212   | 1101_0100  | Growth     | Receipt    | growth:receipt             |
| 213   | 1101_0101  | Growth     | Mask       | growth:mask                |
| 214   | 1101_0110  | Growth     | Residue    | growth:residue             |
| 215   | 1101_0111  | Growth     | Execution  | growth:execution           |
| 216   | 1101_1000  | Growth     | Swarm      | growth:swarm               |
| 217   | 1101_1001  | Growth     | Restraint  | growth:restraint           |
| 218   | 1101_1010  | Growth     | Migration  | growth:migration           |
| 219   | 1101_1011  | Growth     | Consent    | growth:consent             |
| 220   | 1101_1100  | Growth     | Vision     | growth:vision              |
| 221   | 1101_1101  | Growth     | Growth     | growth:growth              |
| 222   | 1101_1110  | Growth     | Seal       | growth:seal                |
| 223   | 1101_1111  | Growth     | Rhythm     | growth:rhythm              |
| 224   | 1110_0000  | Seal       | Genesis    | seal:genesis               |
| 225   | 1110_0001  | Seal       | Void       | seal:void                  |
| 226   | 1110_0010  | Seal       | Attention  | seal:attention             |
| 227   | 1110_0011  | Seal       | Loop       | seal:loop                  |
| 228   | 1110_0100  | Seal       | Receipt    | seal:receipt               |
| 229   | 1110_0101  | Seal       | Mask       | seal:mask                  |
| 230   | 1110_0110  | Seal       | Residue    | seal:residue               |
| 231   | 1110_0111  | Seal       | Execution  | seal:execution             |
| 232   | 1110_1000  | Seal       | Swarm      | seal:swarm                 |
| 233   | 1110_1001  | Seal       | Restraint  | seal:restraint             |
| 234   | 1110_1010  | Seal       | Migration  | seal:migration             |
| 235   | 1110_1011  | Seal       | Consent    | seal:consent               |
| 236   | 1110_1100  | Seal       | Vision     | seal:vision                |
| 237   | 1110_1101  | Seal       | Growth     | seal:growth                |
| 238   | 1110_1110  | Seal       | Seal       | seal:seal                  |
| 239   | 1110_1111  | Seal       | Rhythm     | seal:rhythm                |
| 240   | 1111_0000  | Rhythm     | Genesis    | rhythm:genesis             |
| 241   | 1111_0001  | Rhythm     | Void       | rhythm:void                |
| 242   | 1111_0010  | Rhythm     | Attention  | rhythm:attention           |
| 243   | 1111_0011  | Rhythm     | Loop       | rhythm:loop                |
| 244   | 1111_0100  | Rhythm     | Receipt    | rhythm:receipt             |
| 245   | 1111_0101  | Rhythm     | Mask       | rhythm:mask                |
| 246   | 1111_0110  | Rhythm     | Residue    | rhythm:residue             |
| 247   | 1111_0111  | Rhythm     | Execution  | rhythm:execution           |
| 248   | 1111_1000  | Rhythm     | Swarm      | rhythm:swarm               |
| 249   | 1111_1001  | Rhythm     | Restraint  | rhythm:restraint           |
| 250   | 1111_1010  | Rhythm     | Migration  | rhythm:migration           |
| 251   | 1111_1011  | Rhythm     | Consent    | rhythm:consent             |
| 252   | 1111_1100  | Rhythm     | Vision     | rhythm:vision              |
| 253   | 1111_1101  | Rhythm     | Growth     | rhythm:growth              |
| 254   | 1111_1110  | Rhythm     | Seal       | rhythm:seal                |
| 255   | 1111_1111  | Rhythm     | Rhythm     | rhythm:rhythm              |

---

## Odù Corpus vs. Agent Prescriptions

The If-Script wave files contain **spiritual prescriptions** (what the Odù means
in ritual/cultural terms). The CalabashDispatcher derives **operational prescriptions**
(what the agent should execute) from vessel semantics.

Both layers are present in the `if_script_cast` tool output:
- `prescription` — the ActionCompiler-ready operational directive
- `prescriptions_spiritual` — the corpus's ritual prescriptions (for context)

The agent uses the operational prescription for execution. The spiritual
prescription informs interpretation and understanding.

---

## Scaling: 256 → 65,536

The base 256 Odù scale to 65,536 via the `calabash::compose` module. A 16-bit
`odu_id` addresses the full space:
- `top == 0` → base 256 (the Digital Calabash corpus)
- `top > 0` → composed Odù: top byte drives vessel + opcode, bottom refines meaning

Composed Odù require `AgentExperience::tier >= 2`. Access is gated by the
`calabash::cast()` function, which enforces the experience ceiling.

The CalabashDispatcher currently handles only the base 256. Support for the
65,536 composed space will be added as agents gain experience (Phase 2).

---

## Integration Points

| Component | File | Role |
|-----------|------|------|
| CalabashDispatcher | `execution/calabash_dispatch.rs` | Odù → CompiledAction |
| ActionCompiler | `execution/action_compiler.rs` | Prescription → CompiledAction |
| ActionTransaction | `execution/action_transaction.rs` | Execute + receipt |
| FieldDiviner | `~/If-Script/src/field_divination.rs` | Live field → Odù |
| if_script_cast tool | `tools/if_script_tool.rs` | Agent-accessible cast |
| Think prompt | `interpreter.rs:4705` | Odù context injection |
| ifscript_gate.rs | `src/ifscript_gate.rs` | Vessel authorization gate |
| soul.rs | `genesis/soul.rs` | Birth Odù from entropy |
| divination.rs | `src/divination.rs` | Memory graph pattern detection |
