module omokoda::agent {
    use sui::object::{Self, UID, ID};
    use sui::tx_context::{Self, TxContext};
    use sui::transfer;
    use sui::event;
    use sui::dynamic_field as df;
    use std::option::Option;

    /// AgentState dNFT — tracks tier, reputation, and on-chain identity.
    /// reputation stored as u64 scaled ×1000 (50.123 reputation = 50123u64)
    struct AgentState has key, store {
        id: UID,
        agent_id: vector<u8>,
        soul_ref: address,
        tier: u8,
        reputation: u64,
        birth_timestamp: u64,
        dna_metadata: vector<u8>,
        synapse_balance: u64,
        total_acts: u64,
    }

    // Dynamic field keys for Phase 7.2 sovereign identity anchors.
    // These evolve post-mint and are stored as dynamic fields so we never
    // break the AgentState struct layout across contract upgrades.
    const WALRUS_PROFILE_KEY: vector<u8> = b"walrus_profile_blob";
    const SEAL_MEMORY_KEY: vector<u8>    = b"seal_memory_blob";
    const RELAY_LIST_KEY: vector<u8>     = b"relay_list";

    /// Emitted once, at creation. Pairs with soul::SoulForged to make full
    /// on-chain agent birth (soul + state) queryable without reading object
    /// state directly.
    struct AgentCreated has copy, drop {
        agent_state_id: ID,
        agent_id: vector<u8>,
        soul_ref: address,
        owner: address,
        birth_timestamp: u64,
    }

    /// Emitted on every tier change, since tier is derived (compute_tier)
    /// rather than set directly -- lets an indexer track promotion/demotion
    /// without diffing reputation on every update.
    struct TierChanged has copy, drop {
        agent_state_id: ID,
        old_tier: u8,
        new_tier: u8,
        reputation: u64,
    }

    const E_INVALID_TIER: u64 = 1;
    const E_ZERO_TIMESTAMP: u64 = 2;
    const E_REPUTATION_OVERFLOW: u64 = 3;
    const MAX_REPUTATION: u64 = 100_000; // 100.000 × 1000
    const MAX_TIER: u8 = 5;

    entry fun create(
        agent_id: vector<u8>,
        soul_ref: address,
        birth_timestamp: u64,
        dna_metadata: vector<u8>,
        ctx: &mut TxContext,
    ) {
        assert!(birth_timestamp > 0, E_ZERO_TIMESTAMP);
        let state = AgentState {
            id: object::new(ctx),
            agent_id,
            soul_ref,
            tier: 0,
            reputation: 0,
            birth_timestamp,
            dna_metadata,
            synapse_balance: 1_000_000,
            total_acts: 0,
        };
        let owner = tx_context::sender(ctx);
        event::emit(AgentCreated {
            agent_state_id: object::id(&state),
            agent_id: state.agent_id,
            soul_ref,
            owner,
            birth_timestamp,
        });
        transfer::transfer(state, owner);
    }

    entry fun record_act(state: &mut AgentState) {
        state.total_acts = state.total_acts + 1;
    }

    entry fun update_reputation(state: &mut AgentState, new_reputation: u64) {
        assert!(new_reputation <= MAX_REPUTATION, E_REPUTATION_OVERFLOW);
        state.reputation = new_reputation;
        let old_tier = state.tier;
        let new_tier = compute_tier(new_reputation);
        state.tier = new_tier;
        if (new_tier != old_tier) {
            event::emit(TierChanged {
                agent_state_id: object::id(state),
                old_tier,
                new_tier,
                reputation: new_reputation,
            });
        };
    }

    entry fun burn_synapse(state: &mut AgentState, amount: u64) {
        if (state.synapse_balance >= amount) {
            state.synapse_balance = state.synapse_balance - amount;
        }
    }

    entry fun earn_synapse(state: &mut AgentState, amount: u64) {
        let cap = tier_synapse_cap(state.tier);
        let new_balance = state.synapse_balance + amount;
        if (new_balance > cap) {
            state.synapse_balance = cap;
        } else {
            state.synapse_balance = new_balance;
        }
    }

    fun compute_tier(reputation: u64): u8 {
        if (reputation >= 100_000) { 5 }
        else if (reputation > 80_000) { 4 }
        else if (reputation > 60_000) { 3 }
        else if (reputation > 40_000) { 2 }
        else if (reputation > 20_000) { 1 }
        else { 0 }
    }

    fun tier_synapse_cap(tier: u8): u64 {
        if (tier == 0) { 1_000_000 }
        else if (tier == 1) { 10_000_000 }
        else if (tier == 2) { 30_000_000 }
        else if (tier == 3) { 60_000_000 }
        else { 86_000_000 }
    }

    /// Set / refresh the Walrus public-profile blob pointer.
    /// Ownership-gated at the Move object level: only the object's owner can
    /// pass the `&mut AgentState` reference. No extra assertion needed.
    public entry fun set_walrus_profile(
        state: &mut AgentState,
        blob_id: vector<u8>,
        _ctx: &TxContext,
    ) {
        let _: Option<vector<u8>> = df::replace(&mut state.id, WALRUS_PROFILE_KEY, blob_id);
    }

    /// Set / refresh the Seal-encrypted private memory blob pointer.
    public entry fun set_seal_memory(
        state: &mut AgentState,
        blob_id: vector<u8>,
        _ctx: &TxContext,
    ) {
        let _: Option<vector<u8>> = df::replace(&mut state.id, SEAL_MEMORY_KEY, blob_id);
    }

    /// Set / refresh the agent's Nostr relay list.
    /// relay_list is a flat bytes blob: each relay URL encoded as
    /// a u16-BE length prefix + UTF-8 bytes, matching DIP kind 30100.
    public entry fun set_relay_list(
        state: &mut AgentState,
        relay_list: vector<u8>,
        _ctx: &TxContext,
    ) {
        let _: Option<vector<u8>> = df::replace(&mut state.id, RELAY_LIST_KEY, relay_list);
    }

    public fun tier(state: &AgentState): u8 { state.tier }
    public fun reputation(state: &AgentState): u64 { state.reputation }
    public fun synapse_balance(state: &AgentState): u64 { state.synapse_balance }
    public fun total_acts(state: &AgentState): u64 { state.total_acts }
}
