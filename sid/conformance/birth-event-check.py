#!/usr/bin/env python3
"""Prove a SID birth event is cryptographically valid (NIP-01 id + BIP-340 sig).

Input: the JSON an emitter produced ({kind, content, tags}), i.e.

    node -e "process.stdout.write(JSON.stringify(
      require('./implementations/typescript/dist/sid.js')
        .birthEvent(require('./conformance/fixtures/full-sid.json'))))" > /tmp/ev.json
    python3 conformance/birth-event-check.py /tmp/ev.json

The key here is a TEST VECTOR derived from a fixed public string. It must never
be used to sign a real agent's certificate, and this script publishes nothing.
"""
import hashlib
import json
import sys

KIND = 30401
CREATED_AT = 1762822620  # fixed so the vector is reproducible


def main(path: str) -> int:
    ev = json.load(open(path))
    problems = []

    if ev.get("kind") != KIND:
        problems.append(f"kind {ev.get('kind')} != {KIND}")
    tags = ev.get("tags") or []
    names = [t[0] for t in tags if t]
    for required in ("d", "format", "v", "hash", "status", "alt"):
        if required not in names:
            problems.append(f"missing tag {required!r}")
    content = json.loads(ev.get("content") or "{}")
    for forbidden in ("seed", "mnemonic", "private_key", "nsec", "secret", "memory_locator"):
        if forbidden in content:
            problems.append(f"content must not carry {forbidden}")

    try:
        sys.path.insert(0, "/opt/ares/minipae")
        import minipae
    except Exception as e:  # noqa: BLE001
        print(f"verifier_unavailable: minipae not importable ({e})")
        return 3

    seckey = int.from_bytes(hashlib.sha256(b"sid-birth-event-proof").digest(), "big")
    pub = minipae.pubkey_from_secret(seckey).hex()
    serial = json.dumps(
        [0, pub, CREATED_AT, ev["kind"], ev["tags"], ev["content"]],
        separators=(",", ":"), ensure_ascii=False)
    eid = hashlib.sha256(serial.encode()).hexdigest()
    sig = minipae.schnorr_sign(bytes.fromhex(eid), seckey, aux=b"\x00" * 32).hex()
    ok = minipae.schnorr_verify(bytes.fromhex(eid), bytes.fromhex(pub), bytes.fromhex(sig))
    if not ok:
        problems.append("BIP-340 signature did not verify")

    print(f"kind        {ev['kind']}")
    print(f"d tag       {dict((t[0], t[1]) for t in tags).get('d')}")
    print(f"event id    {eid}")
    print(f"signature   {sig}")
    print(f"verify      {'VALID' if ok else 'INVALID'}")
    if problems:
        print("problems    " + "; ".join(problems))
        return 1
    print("birth event conformant")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/birth_event.json"))
