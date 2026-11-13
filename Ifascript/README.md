# ⚠️ STALE — this is NOT the If-Script API

The two files here (`src/lib.rs`, `src/odu.rs`) are a **tiny stale stub**:
2 files against the live crate's 73. They predate the Digital Calabash and do
not contain it.

Read those files and you will conclude If-Script has no `calabash` module, no
`ActionVessel`, and an `Odu` of `{name, archetype, binary, index}`. All three
conclusions are wrong.

## What the build actually uses

`omokoda-core` depends on the git crate, pinned:

```toml
ifascript = { git = "https://github.com/cryptonomicsed-byte/If-Script", rev = "5dbee51…" }
```

The resolved source lives at
`~/.cargo/git/checkouts/if-script-c3aa0a5f84bab064/5dbee51/src/` — 73 files,
including:

- `calabash/mod.rs` — `resolve(odu_id: u16) -> ComposedOdu`, `compose_id`,
  `decompose`, `vessel_for` (the 65,536 space)
- `calabash/scaling.rs` — `tier_for_xp`, `AgentExperience`, `ConsensusLedger`
- `odu/mod.rs` — `ActionVessel` (the 16), `get_odu`, `ODU_SET`
- `odu_ifa.rs` — the traditional Yorùbá corpus
- `vm.rs`, `cosmogram.rs`, `manifesto/`, `nostr/`

To read the real thing:

```sh
D=$(ls -d ~/.cargo/git/checkouts/if-script-*/* | head -1)
grep -rn "pub enum ActionVessel" -A 20 "$D/src/odu/mod.rs"
grep -n "pub fn resolve" -A 20 "$D/src/calabash/mod.rs"
```

## Why this directory still exists

`docs/Finish this.md` describes a CI step that checks out a stub into this path.
Nothing in the workspace references it as a path dependency, so it is not
compiled — it is only a placeholder for that checkout. It is not documentation,
and it must not be treated as the API.
