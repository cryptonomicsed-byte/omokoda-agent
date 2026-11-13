# Vendored copy — `sid/` in the kernel

These files are **byte-identical copies** of the `sid` repo's standard:

| file | source of truth |
|---|---|
| `spec/sid-0.1.md` | `sid/spec/sid-0.1.md` |
| `schema/sid.schema.json` | `sid/schema/sid.schema.json` |
| `conformance/README.md` | `sid/conformance/README.md` |
| `conformance/vectors.json` | `sid/conformance/vectors.json` |
| `conformance/fixtures/*.json` | `sid/conformance/fixtures/*.json` |

The **standard** lives in the `sid` repo. Edit it there, then re-copy — editing
these files directly forks the standard, and the two will drift silently (which
has already happened twice: the format grew here first, then the schema fix
landed there first).

What is deliberately **not** vendored, because it belongs to the standard repo:

- `implementations/typescript/` and `implementations/python/` — the reference
  implementations (`verifySid`, `verify_sid`)
- `conformance/run_vectors.py` — it runs those implementations

The kernel's own implementation is Rust: `omokoda-core/src/identity/sid.rs`
(`from_genesis_receipt_with`, `verify`, `emit_md`, `parse_md`, `anchored_v2`).
It is checked against the same rules but is not a copy of either reference
implementation.

Verify the copy has not drifted:

```sh
for f in spec/sid-0.1.md schema/sid.schema.json conformance/README.md \
         conformance/vectors.json conformance/fixtures/minimal-sid.json \
         conformance/fixtures/full-sid.json; do
  diff -q "sid/$f" "/path/to/sid/$f" || echo "DRIFTED: $f"
done
```
