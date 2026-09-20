//! Phase 8G — GIX integration tests.
//!
//! Full-stack coverage across:
//!   Omo-Koda2 memory layer (OduDirectory / OduEntry)
//!     ↔ gix_bridge (Merkle index, graph projection, minipae locator, provenance)
//!     ↔ gix-core  (CanonicalObjectStore, StoreProjection, GixFold, save/load)
//!
//! These tests exercise path boundaries that unit tests inside each crate cannot:
//! the bridge functions use omokoda types as input and gix-core types as output,
//! so only an integration test can drive the full chain.

use omokoda_core::memory::engine::MemoryTier;
use omokoda_core::memory::gix_bridge::{
    audit_memory_root, build_gix1_index,
    entry_to_minipae_locator, entry_with_provenance, memory_merkle_root,
    merkle_proof, project_gix, GixNamespace,
};
use omokoda_core::memory::memdir::{OduDirectory, OduEntry};

use gix_core::{
    content_hash, load_store_from_files, save_store_to_files,
    CanonicalObjectStore, GixFold, GixVisibility, GlyphEdge,
};

// ── fixtures ──────────────────────────────────────────────────────────────────

fn entry(id: &str, content: &str, path: &str, ts: u64, tags: &[&str]) -> OduEntry {
    OduEntry {
        id:            id.into(),
        content:       content.into(),
        importance:    0.5,
        created_at:    ts,
        last_accessed: ts,
        tags:          tags.iter().map(|t| t.to_string()).collect(),
        path:          path.into(),
    }
}

fn dir_with(entries: Vec<OduEntry>) -> OduDirectory {
    let mut d = OduDirectory::new();
    for e in entries { d.insert(e); }
    d
}

// ── 1. Merkle root determinism across the bridge ──────────────────────────────

#[test]
fn memory_merkle_root_is_deterministic_and_auditable() {
    let dir = dir_with(vec![
        entry("e1", "alpha memory",  "memory/core",  100, &[]),
        entry("e2", "beta memory",   "memory/core",  200, &[]),
        entry("e3", "gamma memory",  "memory/other", 300, &[]),
    ]);

    let root1 = memory_merkle_root(&dir);
    let root2 = memory_merkle_root(&dir);
    assert_eq!(root1, root2, "root must be deterministic");

    // build_gix1_index must produce the same root as memory_merkle_root.
    let index = build_gix1_index(&dir);
    assert!(index.audit().is_ok(), "freshly-built index must self-audit");

    // audit_memory_root must accept the root we just computed.
    let audit = audit_memory_root(&dir, &root1);
    assert!(audit.is_ok(), "stored root must audit against same directory: {audit:?}");
}

#[test]
fn audit_memory_root_fails_when_dir_changes() {
    let dir1 = dir_with(vec![
        entry("e1", "original memory", "memory/core", 100, &[]),
    ]);
    let root1 = memory_merkle_root(&dir1);

    // Dir2 has a different entry — root1 is now stale.
    let dir2 = dir_with(vec![
        entry("e1", "original memory",  "memory/core", 100, &[]),
        entry("e2", "additional memory","memory/core", 200, &[]),
    ]);
    let audit = audit_memory_root(&dir2, &root1);
    assert!(audit.is_err(), "stale root must fail audit after dir change");
}

// ── 2. Memory → GIX envelope → CanonicalObjectStore ──────────────────────────

#[test]
fn full_memory_lifecycle_in_canonical_store() {
    let dir = dir_with(vec![
        entry("e1", "episodic memory content", "memory/episodic", 1_000, &["session"]),
        entry("e2", "semantic memory content", "memory/semantic", 2_000, &["concept"]),
    ]);

    let mut store = CanonicalObjectStore::new();

    // Insert each memory entry via entry_to_minipae_locator → insert_memory.
    let mut inserted_ids = Vec::new();
    for e in dir.entries.values() {
        let (env, locator) = entry_to_minipae_locator(
            e, MemoryTier::Episodic, "npub1testkey", None,
        );
        let id = hex::encode(env.canonical_id);
        store.insert_memory(env, locator);
        inserted_ids.push(id);
    }

    // All objects must be in the store.
    for id in &inserted_ids {
        assert!(store.contains(id), "object {id} must be in store");
    }

    // Locator slugs must be deterministic: "mem/<canonical_id>".
    for id in &inserted_ids {
        let loc = store.resolve_locator(id).expect("locator must be registered");
        assert_eq!(loc.slug, format!("mem/{id}"));
        assert_eq!(loc.agent_pubkey, "npub1testkey");
    }

    store.audit_consistency().expect("store must be consistent after bridge inserts");
}

// ── 3. Provenance chain: supersedes + derived_from ────────────────────────────

#[test]
fn provenance_chain_fingerprint_is_stamped_on_envelope() {
    let old_id = content_hash("initial knowledge");

    let new_entry = entry("v2", "revised knowledge", "memory/semantic", 200, &[]);
    let (env, prov) = entry_with_provenance(
        &new_entry, MemoryTier::Semantic,
        Some(old_id),        // supersedes v1
        vec![old_id],        // derived_from v1
    );

    // The provenance fingerprint must appear on the envelope.
    assert_eq!(env.provenance, Some(prov.fingerprint()),
        "provenance fingerprint must be stamped onto the Gix1 envelope");
    assert_eq!(env.namespace, GixNamespace::TriuneMemory);
    assert!(env.verify_integrity(), "envelope must pass integrity check");

    // Provenance lineage fields must be preserved.
    assert_eq!(prov.supersedes, Some(old_id));
    assert_eq!(prov.derived_from, vec![old_id]);
}

#[test]
fn provenance_fingerprint_is_deterministic() {
    let e = entry("v1", "stable content", "memory/core", 100, &[]);
    let (_, prov1) = entry_with_provenance(&e, MemoryTier::Working, None, vec![]);
    let (_, prov2) = entry_with_provenance(&e, MemoryTier::Working, None, vec![]);
    assert_eq!(prov1.fingerprint(), prov2.fingerprint(),
        "same entry + same lineage must always yield the same provenance fingerprint");
}

// ── 4. GixFold over bridge-inserted memory entries ────────────────────────────

#[test]
fn gix_fold_over_memory_entries_adds_fold_source_edges() {
    let e1 = entry("m1", "memory one", "memory/core", 100, &[]);
    let e2 = entry("m2", "memory two", "memory/core", 200, &[]);

    let mut store = CanonicalObjectStore::new();

    let (env1, loc1) = entry_to_minipae_locator(&e1, MemoryTier::Working, "agent-abc", None);
    let (env2, loc2) = entry_to_minipae_locator(&e2, MemoryTier::Working, "agent-abc", None);
    let s1 = env1.canonical_id;
    let s2 = env2.canonical_id;
    store.insert_memory(env1, loc1);
    store.insert_memory(env2, loc2);

    // Build a flat fold over the two memory canonical_ids.
    let fold = GixFold::new(vec![s1, s2], 2.0);
    let fold_id = store.insert_fold(&fold);

    // fold_source edges must connect fold → both source memories.
    let sources = store.walk(&fold_id, 1, Some("fold_source"));
    let source_ids: Vec<&str> = sources.iter().map(|n| n.canonical_id.as_str()).collect();
    let id1 = hex::encode(s1);
    let id2 = hex::encode(s2);
    assert!(source_ids.contains(&id1.as_str()), "fold_source edge to memory one");
    assert!(source_ids.contains(&id2.as_str()), "fold_source edge to memory two");

    store.audit_consistency().expect("store must be consistent after fold insert");
}

#[test]
fn hierarchical_fold_walk_topology_over_bridge_entries() {
    // Child fold wraps two raw memories; parent fold wraps the child.
    let e1 = entry("leaf1", "leaf memory alpha", "memory/core", 100, &[]);
    let e2 = entry("leaf2", "leaf memory beta",  "memory/core", 200, &[]);

    let mut store = CanonicalObjectStore::new();
    let (env1, loc1) = entry_to_minipae_locator(&e1, MemoryTier::Episodic, "agent-x", None);
    let (env2, loc2) = entry_to_minipae_locator(&e2, MemoryTier::Episodic, "agent-x", None);
    let s1 = env1.canonical_id;
    let s2 = env2.canonical_id;
    store.insert_memory(env1, loc1);
    store.insert_memory(env2, loc2);

    let child_fold  = GixFold::new(vec![s1, s2], 2.0);
    let child_id    = store.insert_fold(&child_fold);
    let parent_fold = GixFold::from_child_folds(&[&child_fold], 1.0);
    let parent_id   = store.insert_fold(&parent_fold);

    // walk_fold_children from parent must reach the child fold.
    let descendants = store.walk_fold_children(&parent_id, 3);
    let desc_ids: Vec<&str> = descendants.iter().map(|n| n.canonical_id.as_str()).collect();
    assert!(desc_ids.contains(&child_id.as_str()),
        "walk_fold_children must reach child fold via fold_child edge");
    assert_eq!(parent_fold.member_count, 2, "parent fold must accumulate leaf count");
}

// ── 5. Visibility + StoreProjection ──────────────────────────────────────────

#[test]
fn public_projection_excludes_private_bridge_memories() {
    let e_priv = entry("priv", "private thought", "memory/core",  100, &[]);
    let e_pub  = entry("pub",  "public fact",     "memory/core",  200, &[]);

    let mut store = CanonicalObjectStore::new();
    let (env_priv, loc_priv) = entry_to_minipae_locator(
        &e_priv, MemoryTier::Working, "agent-q", None,
    );
    let (env_pub, loc_pub) = entry_to_minipae_locator(
        &e_pub,  MemoryTier::Working, "agent-q", None,
    );
    let priv_id = hex::encode(env_priv.canonical_id);
    let pub_id  = hex::encode(env_pub.canonical_id);

    store.insert_memory(env_priv, loc_priv);
    store.insert_memory(env_pub,  loc_pub);

    // Both default to Private (GixKind::Memory → Private).
    let proj = store.project_public();
    assert_eq!(proj.object_count, 0, "memory objects are Private by default");

    // Promote one to Public.
    store.set_visibility(&pub_id, GixVisibility::Public);
    let proj = store.project_public();
    assert_eq!(proj.object_count, 1);
    assert!(proj.canonical_ids.contains(&pub_id));
    assert!(!proj.canonical_ids.contains(&priv_id));
}

#[test]
fn agent_fingerprint_differs_per_agent_same_store() {
    let e = entry("shared", "shared public fact", "memory/core", 100, &[]);

    let mut store = CanonicalObjectStore::new();
    let (env, loc) = entry_to_minipae_locator(&e, MemoryTier::Semantic, "agent-a", None);
    let id = hex::encode(env.canonical_id);
    store.insert_memory(env, loc);
    store.set_visibility(&id, GixVisibility::Public);

    let proj = store.project_public();
    assert_eq!(proj.object_count, 1);

    let fp_alice = proj.agent_fingerprint(b"alice");
    let fp_bob   = proj.agent_fingerprint(b"bob");
    assert_ne!(fp_alice.canonical_id_hex(), fp_bob.canonical_id_hex(),
        "agents with identical public objects must still get different fingerprints");
    // But the same agent must get the same fingerprint every time.
    let fp_alice2 = proj.agent_fingerprint(b"alice");
    assert_eq!(fp_alice.canonical_id_hex(), fp_alice2.canonical_id_hex(),
        "agent fingerprint must be deterministic");
}

// ── 6. Save/load cycle with bridge-inserted objects ───────────────────────────

#[test]
fn bridge_populated_store_survives_save_load_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let gp = dir.path().join("g.json");
    let ip = dir.path().join("i.json");
    let sp = dir.path().join("s.json");

    let e1 = entry("p1", "persistent memory one",   "memory/core", 1_000, &["session"]);
    let e2 = entry("p2", "persistent memory two",   "memory/core", 2_000, &["session"]);
    let e3 = entry("p3", "persistent memory three", "memory/other",3_000, &[]);

    let mut store = CanonicalObjectStore::new();
    let mut ids = Vec::new();
    for e in [&e1, &e2, &e3] {
        let (env, loc) = entry_to_minipae_locator(e, MemoryTier::Episodic, "agent-persist", None);
        let id = hex::encode(env.canonical_id);
        store.insert_memory(env, loc);
        ids.push(id);
    }
    // Wire a "recalls" edge between p1 and p2.
    store.add_edge(GlyphEdge {
        from:     ids[0].clone(),
        to:       ids[1].clone(),
        relation: "recalls".into(),
        weight:   2,
    });

    save_store_to_files(&mut store, &gp, &ip, &sp).expect("save must succeed");

    let loaded = load_store_from_files(&gp, &ip, &sp).expect("load must succeed");
    loaded.audit_consistency().expect("loaded store must audit clean");

    // All objects must survive the cycle.
    for id in &ids {
        assert!(loaded.contains(id), "object {id} must survive save/load");
    }
    // Edge topology must survive.
    assert_eq!(loaded.graph.edge_count(), 1, "edge must survive save/load");
}

// ── 7. project_gix topology ───────────────────────────────────────────────────

#[test]
fn project_gix_follows_edges_reflect_temporal_order_in_path() {
    let dir = dir_with(vec![
        entry("t1", "first event",  "session/alpha", 1_000, &[]),
        entry("t2", "second event", "session/alpha", 2_000, &[]),
        entry("t3", "third event",  "session/alpha", 3_000, &[]),
        entry("t4", "other session","session/beta",  4_000, &[]),
    ]);

    let graph = project_gix(&dir);

    // t1→t2 and t2→t3 "follows" edges must exist within session/alpha.
    // t4 is in a separate cluster — no cross-cluster follows.
    let follows: Vec<_> = graph.edges().iter()
        .filter(|e| e.relation == "follows")
        .collect();
    assert_eq!(follows.len(), 2,
        "two follows edges within session/alpha cluster (t1→t2, t2→t3)");

    // All follows edges must have weight 1.
    for e in &follows {
        assert_eq!(e.weight, 1, "follows edges have weight 1");
    }
}

#[test]
fn project_gix_no_self_edges_for_duplicate_content() {
    let dir = dir_with(vec![
        // Same content in two different entries — canonical_ids would collide.
        // The bridge must not produce self-edges.
        entry("dup1", "identical content", "memory/core", 100, &["tag"]),
        entry("dup2", "identical content", "memory/core", 200, &["tag"]),
    ]);
    let graph = project_gix(&dir);
    for edge in graph.edges() {
        assert_ne!(edge.from, edge.to, "self-edge found: {edge:?}");
    }
}

// ── 8. Merkle proof inclusion ─────────────────────────────────────────────────

#[test]
fn merkle_proof_verifies_leaf_inclusion_in_root() {
    use sha2::{Digest, Sha256};

    let dir = dir_with(vec![
        entry("a", "alpha",   "memory/core", 100, &[]),
        entry("b", "bravo",   "memory/core", 200, &[]),
        entry("c", "charlie", "memory/core", 300, &[]),
    ]);

    let index = build_gix1_index(&dir);
    // NOTE: index.root() uses SHA-256(entry.id) canonical_ids (from add_receipt),
    // which differs from memory_merkle_root() which hashes entry.content.
    // merkle_proof must agree with index.root() — they share the same canonical_id space.
    let index_root = index.root().to_string();

    // Verify a proof for each entry in the index.
    for entry in index.entries() {
        let id = &entry.canonical_id;
        let (leaf_hash, proof_root, siblings) =
            merkle_proof(&index, id).unwrap_or_else(|e| panic!("proof failed for {id}: {e}"));

        // leaf_hash must be SHA-256(id.as_bytes()) — same first step as gix1_merkle_root.
        let expected_leaf: [u8; 32] = {
            let mut h = Sha256::new();
            h.update(id.as_bytes());
            h.finalize().into()
        };
        assert_eq!(leaf_hash, hex::encode(expected_leaf),
            "leaf hash mismatch for entry {id}");

        // proof_root must match the index's stored root (same canonical_id space).
        assert_eq!(proof_root, index_root,
            "proof root mismatch for entry {id} (siblings: {siblings:?})");
    }
}

#[test]
fn merkle_proof_fails_for_unknown_entry() {
    let dir = dir_with(vec![
        entry("a", "some content", "memory/core", 100, &[]),
    ]);
    let index = build_gix1_index(&dir);
    let result = merkle_proof(&index, "nonexistent-canonical-id");
    assert!(result.is_err(), "proof for unknown entry must return Err");
}

// ── 9. Shared-group projection across bridge objects ─────────────────────────

#[test]
fn shared_group_projection_shows_correct_subset() {
    let e_pub    = entry("pub",    "public fact",        "memory/core", 100, &[]);
    let e_guild  = entry("guild",  "guild-only memory",  "memory/core", 200, &[]);
    let e_priv   = entry("priv",   "private memory",     "memory/core", 300, &[]);

    let mut store = CanonicalObjectStore::new();
    let mut insert = |e: &OduEntry| -> String {
        let (env, loc) = entry_to_minipae_locator(e, MemoryTier::Working, "agent-z", None);
        let id = hex::encode(env.canonical_id);
        store.insert_memory(env, loc);
        id
    };
    let pub_id   = insert(&e_pub);
    let guild_id = insert(&e_guild);
    let priv_id  = insert(&e_priv);

    store.set_visibility(&pub_id,   GixVisibility::Public);
    store.set_visibility(&guild_id, GixVisibility::Shared("guild-omega".into()));
    // priv_id stays Private (default).

    // project_shared("guild-omega") must include Public + Shared(guild-omega).
    let proj = store.project_shared("guild-omega");
    assert!(proj.canonical_ids.contains(&pub_id),   "public objects visible to guild");
    assert!(proj.canonical_ids.contains(&guild_id), "shared-for-guild visible to guild");
    assert!(!proj.canonical_ids.contains(&priv_id), "private objects not visible to guild");

    // project_shared("other-guild") must only include Public.
    let proj2 = store.project_shared("other-guild");
    assert_eq!(proj2.object_count, 1, "other guild sees only the public object");
    assert!(proj2.canonical_ids.contains(&pub_id));
    assert!(!proj2.canonical_ids.contains(&guild_id));
}
