# SID — Sovereign ID / Super Intelligence Document — `sid/0.1`

**Status:** draft standard · 2026-11-11
**Derives from:** `card-protocol` (`business-card`, MIT). The `business-card` format remains
authoritative there; this document defines a different object.

## 1. The product

A SID is an **agent birth certificate**: the genesis record of an agent identity. Issuing v1
is the birth — before it, no agent exists. Every later version is a mutation of the same
identity, chained by hash, so the certificate's history *is* the agent's biography.

A `business-card` describes a business to a reader. A SID constitutes an agent. That
difference drives every rule below.

## 2. Read rules

1. Read the frontmatter first. `status: cast` = minted, not yet anchored → untrusted for
   irreversible acts.
2. Quote only what the certificate says. An absent or `null` field is a refusal; never invent.
3. Cite `provenance` when asked to justify identity.
4. The agent does not edit its own certificate. Growth is a new version, minted by the issuer.
5. A `time.birth_block` without a verifiable proof is a claim, not a proof.

## 3. File shape

`SID.md` at the agent root. Frontmatter is flat, scalar, and canonical:

```yaml
---
format: sid/0.1
id: sid_<slug>
version: 1
previous_version: null          # explicit null on v1; integer on v>1
previous_hash: none             # "none" on v1; 64-hex on v>1
hash: <64-hex>
acting_hash: <64-hex>
status: cast                    # cast | live | suspended | retired
born_at: <RFC3339 UTC>
updated: <RFC3339 UTC>
---
```

`format` is also required **inside** the certificate JSON. (card-protocol keeps `format` in
frontmatter only because legacy consumers load its JSON without one; SID has no legacy
consumer, and a certificate that cannot name its own format is not self-describing.)

## 4. Sections

Sections appear in this order. Every section is emitted with a machine-readable comment —
`<!-- sid:<key>@1 {json} -->` — so Markdown round-trips to JSON losslessly.

### 4.1 Identity
| field | type | notes |
|---|---|---|
| `name` | string | the agent's name |
| `type` | `"agent"` | distinguishes a SID from a business card |
| `odu_link` | integer 0–65535 | GIX address: `odu_primary << 8 \| odu_secondary` |
| `odu_primary` | integer 0–255 | the birth divination, primary Odu |
| `odu_secondary` | integer 0–255 | the birth divination, secondary Odu |
| `npub` | string `npub1…` | Nostr binding (public) |
| `principal` | string \| null | gatekeeper identity (Èṣù/GIX key id) — **not** the npub |
| `parent` | `sid_…` \| null | lineage; null for a root agent |
| `issuer` | string \| null | who minted this version |
| `description` | string | one sentence, optional |

Invariant: `odu_link == (odu_primary << 8) | odu_secondary`. A certificate whose Odu fields
disagree with its link is malformed — the address is derived, never independently authored.

### 4.2 Genesis
`entropy_source` (`ifascript/cowrie-v1`), `entropy_commitment` (64-hex), `seed_hash` (64-hex),
`keygen` (`gix_kdf_v1+bipon39`). **Hashes only.** The mnemonic, seed and keys never appear.

### 4.3 Time
`birth_block` (integer), `birth_block_hash` (string), `ots_proof` (string ref \| null),
`anchored` (boolean). `anchored: true` requires `ots_proof` — the verifier enforces it.

### 4.4 Capabilities
`skills` (array of manifest names), `permissions` (array of capability strings),
`namespaces` (array). What the agent may do — declared, not implied.

### 4.5 Memory
`locator` (string \| null — e.g. a `GixMinipaeLocator` or NIP-AE `d`-tag), `namespace_root`
(64-hex \| null). A reference and a root; never the memory itself.

### 4.6 Provenance
`trace_chain_head` (64-hex \| null — e.g. Mycelium chain head), `profile_id` (string \| null —
the runtime profile this certificate minted).

### 4.7 Lifecycle
`state` (`cast|live|suspended|retired`) and `history` (array of `{state, at}`), append-only.
`lifecycle.state` must equal the top-level `status`.

### 4.8 Changelog
Array of human-readable strings, oldest first. One line per version.

### 4.9 Appendix
Free-form object for humans and planners. **Not** quotable, **not** in `acting_hash`.

## 5. The hashing contract

Byte-identical with card-protocol's canonicalization (and with Python's
`json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True)`):

1. Prune empty arrays and empty objects when they are object *values* (bottom-up).
2. Sort keys.
3. Compact separators (`,` and `:`).
4. Escape non-ASCII as `\uXXXX` (surrogate pairs above the BMP).

```
hash        = sha256(canonicalJson(cert minus "hash" and every "_"-prefixed key))
acting_hash = sha256(canonicalJson({ identity, capabilities, memory, lifecycle, status }))
```

Acting keys are exactly `identity`, `capabilities`, `memory`, `lifecycle`, `status`. Genesis,
time, provenance, changelog and appendix are attestation and history — never acting state.
A consumer that reloads on `acting_hash` therefore ignores a re-anchor or a changelog edit
and *does* reload when anything the agent acts on changes.

## 6. Chain rules

- **v1:** `previous_version: null`, `previous_hash: none`. Anything else on v1 is malformed.
- **v>1:** `previous_version` is `version - 1`; `previous_hash` is the previous version's
  `hash`, 64-hex. A v2 whose `previous_hash` is `none` is not a chain.
- A mutation mints a new version. Editing a live hash in place is forbidden: the whole point
  of the chain is that history is append-only and provable.
- Retirement is a terminal version: `status: retired`, `lifecycle.state: retired`, no
  further versions.

## 7. Status

| status | meaning | trust |
|---|---|---|
| `cast` | minted, not yet anchored | untrusted for irreversible acts |
| `live` | anchored and in service | usable |
| `suspended` | temporarily barred | must not act |
| `retired` | terminal | read-only |

## 8. Anchoring

The certificate's `hash` is anchored to a trustless clock (OpenTimestamps → Bitcoin). The
anchor is a *receipt*, not a field the issuer may set by will: `anchored: true` demands an
`ots_proof` reference that verifies independently. Anchoring is what turns "born at
2026-11-11T01:30:00Z, says the issuer" into "existed at or before block N, checkable by
anyone."

## 9. The birth event (Nostr)

### 9.1 Kind

**Kind `30401`** — parameterized replaceable (30000–39999), keyed by `d = <sid id>`.

Registry — kinds in use in the ecosystem at the time of writing:

| kind | owner | use |
|---|---|---|
| `30174` | NIP-AE / minipae / mycelium | agent engrams (memory) |
| `30401` | **SID (this document)** | agent birth certificate |
| `31020` | contested (four owners) | do not publish here |
| `47001`–`47010` | Crucible / mycelium | claims, challenges, verdicts |
| `24134` | minipae | (integration utility) |

A parameterized replaceable event means relays keep only the latest version per `d` tag. That
is intentional: the relay holds the *current* identity document, while history remains
provable through the certificate's own `previous_hash` chain and each version's anchor.

### 9.2 Content

`content` is compact JSON (the announcement, not the whole certificate):

```json
{"format":"sid/0.1","id":"sid_oracle-7","version":1,"hash":"<64-hex>",
 "acting_hash":"<64-hex>","status":"cast","born_at":"…","odu_link":4711,
 "npub":"npub1…","principal":"gix:…","birth_block":975325,"birth_block_hash":"…"}
```

Tags: `["d", id]`, `["format","sid/0.1"]`, `["v", version]`, `["hash", hash]`,
`["status", status]`, `["alt", "SID birth certificate: <name> v<version>"]`.

### 9.3 What must never be in the event

No seed, no mnemonic, no private key. `memory.locator` is omitted by default (opt-in) — a
public announcement should not advertise where an agent's encrypted memory lives.

## 10. Conformance

A file claiming `sid/0.1` is conformant only when **all** hold:

1. `schema/sid.schema.json` validates it;
2. `hash` recomputes from the canonical body;
3. `acting_hash` recomputes from the acting keys;
4. `odu_link == (odu_primary << 8) | odu_secondary`;
5. `lifecycle.state == status`;
6. the chain rules of §6 hold;
7. `time.anchored == true` implies a present `ots_proof`.

Schema validity alone proves nothing about the hash or the chain — a verifier must run all
seven.

## 11. Format versioning

`sid/0.1` is the only format this document defines. A parser MUST reject an unknown `format`
rather than guess. Future versions bump the minor number and must state what changed in this
document's changelog; a major change (an incompatible field or hash rule) takes a new format
name, never a silent reinterpretation of `sid/0.1`.

## 12. Security notes

- **Secrets:** the certificate is designed to be publishable. Anything that must stay secret
  is a hash or a reference.
- **Identity ≠ key:** `npub` and `principal` are separate fields on purpose. A compromised
  Nostr key must not silently become a compromised agent identity; revocation is a new
  version with a new binding, recorded in the chain.
- **Trust:** `cast` is a claim. Only an anchored, `live` certificate with a verifying chain is
  evidence of identity.
- **Privacy:** the appendix and the memory locator are the two places personal or operational
  detail may accumulate. Both are excluded from the announcement by default.
