# Conformance — `sid/0.1`

A certificate is conformant when **all** of these hold. Schema validity alone proves
nothing about the hash or the chain.

| # | Check | Enforced by |
|---|---|---|
| 1 | `schema/sid.schema.json` validates | Ajv (`npm test`) |
| 2 | `hash` recomputes from the canonical body | `verifySid()` / `verify_sid()` |
| 3 | `acting_hash` recomputes from the acting keys | `verifySid()` / `verify_sid()` |
| 4 | `identity.odu_link == (odu_primary << 8) \| odu_secondary` | both |
| 5 | `identity.calabash` agrees with the Odu fields, `vessel` non-empty | both |
| 6 | `previous_version`/`previous_hash` follow §6 chain rules | both, and `verifySid(cert, previous)` for the link |
| 7 | `lifecycle.state == status` | both |
| 8 | `time.anchored` requires `ots_proof` **and** `ots_status: "complete"` **and** `clock_source: "okf"` | both |
| 9 | `time.calendar.{epoch,cycle,phase}` are non-negative integers | both |
| 10 | no `seed`/`mnemonic`/`private_key`/`nsec`/`secret` in `genesis` or `identity` | both |

Every rule is **data-only** — no Odù corpus, no network, no clock — so any
implementation can enforce all ten against a certificate it received from
someone else.

## Verification parity — `conformance/vectors.json`

One vector file, two implementations, one expected verdict each. The TypeScript
suite runs it (`npm test`); the Python runner is:

```sh
python3 conformance/run_vectors.py
```

Both must produce the same `ok` and the same error for every vector. A
disagreement means one implementation is wrong, which is exactly what a
standard is supposed to make impossible to miss.

Vectors cover: valid minimal/full, a tampered acting field, a bad `odu_link`, a
Calabash reading that contradicts its address, an anchored certificate on a
pending proof, one with the wrong clock, a correctly anchored one, a v1 carrying
a chain, a secret in `genesis`, a non-integer calendar coordinate, and a
zero version.

## Schema nullability

A field that is legitimately absent must be nullable in the schema. `time.birth_block`
is `null` on a Gregorian-fallback birth, and `identity.npub`/`principal` are null
for an unbound agent — a schema that demands an integer there rejects a real
certificate while the verifier accepts it. Both must agree, or the standard is
wrong in a way that only shows up on live data.

## Fixtures

- `fixtures/minimal-sid.json` — v1, unanchored (`status: cast`), minimum required fields.
- `fixtures/full-sid.json` — v1, anchored (`status: live`), every section populated.

Fixtures are authored as **bodies** (no hashes) and stamped by the implementation:

```sh
cd implementations/typescript && npm install && npm run fixtures
```

A handwritten hash in a fixture is a landmine — it will look conformant while proving nothing.

## Cross-language rule

The hash must be byte-identical between implementations:

```sh
python3 - <<'PY'
import json, sys; sys.path.insert(0, "implementations/python")
import canonical
for n in ("minimal-sid", "full-sid"):
    c = json.load(open(f"conformance/fixtures/{n}.json"))
    assert canonical.canonical_hash(c) == c["hash"], n
    assert canonical.acting_hash(c) == c["acting_hash"], n
print("python == typescript")
PY
```

Both fixtures currently agree byte-for-byte. If a change to either canonicalizer breaks that,
the change is wrong — not the fixture.

## Birth event

`conformance/birth-event-check.py` proves the kind-30401 announcement is cryptographically
valid: it takes the event the TypeScript emitter produces, computes the NIP-01 id
(`sha256` of the compact `[0, pubkey, created_at, kind, tags, content]` serialization), signs
it with BIP-340, and verifies the signature with the ecosystem's pure-Python implementation
(`minipae`). It uses a throwaway key derived from a fixed string — a **test vector**, never a
real agent key — and publishes nothing.

```sh
node -e "process.stdout.write(JSON.stringify(require('./implementations/typescript/dist/sid.js').birthEvent(require('./conformance/fixtures/full-sid.json'))))" > /tmp/ev.json
python3 conformance/birth-event-check.py /tmp/ev.json
```

## What is deliberately not covered

- **Signature authority.** SID does not sign certificates. The gatekeeper (Èṣù / GIX, Rust)
  does. A certificate's authenticity is its anchor + its chain, not a signature this spec
  invents.
- **Anchor verification.** `time.ots_proof` names a proof; validating it is the anchor
  tool's job (OpenTimestamps → Bitcoin), not the format's.
- **Relay behaviour.** That kind 30401 replaces per `d` tag is NIP-01 semantics, not
  something a fixture can prove offline.
