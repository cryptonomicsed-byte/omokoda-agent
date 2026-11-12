# SID — Sovereign ID / Super Intelligence Document

**The agent birth certificate.** A versioned, hash-chained, agent-readable document whose
issuance *constitutes* an agent — identity, genesis, time, capabilities, memory, provenance,
lifecycle — with every version anchorable to a trustless clock.

- **Format:** `sid/0.1`
- **File:** `SID.md` at the agent root
- **Status:** draft standard

Derived from [`card-protocol`](https://github.com/cryptonomicsed-byte/card-protocol) — the
`business-card` format stays there, for HelixMain's Business DNA cards. SID keeps the
machinery that made that protocol trustworthy (canonical JSON, the hash contract, the
version chain, the conformance suite) and changes the *object*: a card **describes** a
business to whoever reads it; a SID **constitutes** an agent.

> Before the certificate exists, there is no agent. Minting v1 is the birth.

## The one idea

A birth certificate is not a description. It is the genesis record of an identity: it says
*who*, *from what entropy*, *when — verifiably*, *with what capabilities*, *remembering what*,
*descended from whom*. Every later version is the agent's biography: the card-protocol's
`previous_hash` chain becomes a life.

## Read rules (agents)

1. Read the frontmatter first. `status: cast` means minted but not yet anchored to Bitcoin:
   untrusted for irreversible acts.
2. Quote only what the certificate says. An absent or `null` field is a refusal to act, not
   an invitation to invent. A `null` parent stays `null`.
3. Cite `provenance` when asked why you are who you are.
4. You do not edit your own certificate. Growth is a new version, minted by the gatekeeper.
5. A `birth_block` with no verifiable proof is a *claim*, not a proof.

## File shape

```yaml
---
format: sid/0.1
id: sid_<slug>
version: 2
previous_version: 1            # explicit null on v1
previous_hash: <64-hex>        # "none" on v1
hash: <64-hex>                 # over the full canonical JSON; never "pending"
acting_hash: <64-hex>          # over the acting keys only
status: live                   # cast | live | suspended | retired
born_at: 2026-11-11T01:30:00Z
updated: 2026-11-11T01:30:00Z
---
```

Sections, in order: **1. Identity · 2. Genesis · 3. Time · 4. Capabilities · 5. Memory ·
6. Provenance · 7. Lifecycle · 8. Changelog · 9. Appendix**.

## The hashing contract (inherited, unchanged)

- `canonicalJson(v)` — sorted keys, compact separators, Python `ensure_ascii` string escaping,
  empty arrays/objects pruned bottom-up. Byte-identical with
  `json.dumps(obj, sort_keys=True, separators=(",", ":"))`.
- `hash = sha256(canonicalJson(cert minus "hash" and any "_"-prefixed key))`
- `acting_hash = sha256(canonicalJson(cert restricted to the acting keys))`
- Acting keys: `identity`, `capabilities`, `memory`, `lifecycle`, `status`. Genesis, time,
  provenance, changelog and appendix are attestation and history — never acting state.

## Secrets never enter the certificate

`genesis.seed_hash` and `genesis.entropy_commitment` are hashes. The mnemonic, the seed and
every private key stay out. The certificate proves the seed existed; it does not carry it.
`identity.npub` is a public binding; `identity.principal` is the gatekeeper's identity — and
per the ecosystem doctrine a Nostr key is never the whole agent identity, so the two are
recorded separately and are not interchangeable.

## The birth event (Nostr)

No birth event existed in the ecosystem (kinds in use: `30174` NIP-AE memory, `47001`–`47010`
Crucible, `24134`; `31020` is contested). SID registers **kind `30401`** — a parameterized
replaceable event keyed by `d = <sid id>` — so relays always hold the current certificate
version while history stays provable through the version chain and its anchors.

```json
{
  "kind": 30401,
  "tags": [["d", "sid_oracle-7"], ["format", "sid/0.1"], ["v", "1"], ["hash", "<64-hex>"]],
  "content": "{\"format\":\"sid/0.1\",\"id\":\"sid_oracle-7\",\"version\":1,\"hash\":\"…\",\"odu_link\":4711,\"npub\":\"npub1…\",\"status\":\"cast\",\"born_at\":\"…\"}"
}
```

The announcement carries only public identity material — never a seed, a mnemonic, a private
key, or (by default) the memory locator. See `spec/sid-0.1.md` §9.

## Contents

| Path | What |
|---|---|
| `spec/sid-0.1.md` | The standard: sections, hashing contract, chain rules, birth event, conformance |
| `schema/sid.schema.json` | JSON Schema 2020-12 for the certificate JSON |
| `implementations/typescript/` | Canonicalization, hash chain, SID.md emitter / parser / verifier, birth event, tests (`@sid-protocol/core`) |
| `conformance/fixtures/` | Minimal and full certificate fixtures |

## Code

```sh
cd implementations/typescript
npm install && npm test
```

```js
const { emitSidMd, parseSidMd, verifySid, birthEvent } = require("./dist/sid");
```

## Conformance

A SID is conformant when the schema validates **and** `verifySid()` returns `ok: true`.
Schema validity alone proves nothing about the hash or the chain.

## License

MIT — see `LICENSE`. SID derives from `card-protocol` (MIT); the `business-card` format
remains authoritative there.
