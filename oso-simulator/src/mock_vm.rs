//! MockOsovmState — in-memory OSOVM simulation.
//!
//! Tracks:
//!   - Agent balances (BTreeMap<agent_id, ASE_balance>)
//!   - Job table (BTreeMap<job_id, JobEntry>)
//!   - Receipt log
//!   - Simulation environment (twin_f1, agent_tier)
//!
//! Implements all canonical OSO-IR opcodes as pure state transitions.
//! No network, no chain — fully deterministic in-process.

use std::collections::BTreeMap;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct JobEntry {
    pub job_id: String,
    pub owner: String,
    pub status: String,
    pub escrow_amount: u64,
}

/// The simulated OSOVM state.
pub struct MockOsovmState {
    /// Agent ASE balances.
    pub balances: BTreeMap<String, u64>,
    /// Active jobs.
    pub jobs: BTreeMap<String, JobEntry>,
    /// Emitted receipt hashes.
    pub receipts: Vec<String>,
    /// Total ASE spent by the caller.
    pub ase_spent: u64,
    /// Total ASE earned by the caller.
    pub ase_earned: u64,

    /// The simulated caller agent.
    pub caller_id: String,
    pub caller_tier: u8,
    /// Spatial twin F1 score (0.0–1.0).
    pub twin_f1: f64,

    /// Key-value store (STORE/LOAD ops).
    pub kv: BTreeMap<String, Value>,
    /// Blob store (STORE_BLOB/LOAD_BLOB ops).
    pub blobs: BTreeMap<String, Vec<u8>>,

    /// Receipt counter for unique hash generation.
    receipt_counter: u64,
}

impl MockOsovmState {
    pub fn new(caller_id: &str, initial_balance: u64, tier: u8, twin_f1: f64) -> Self {
        let mut balances = BTreeMap::new();
        balances.insert(caller_id.to_string(), initial_balance);
        Self {
            balances,
            jobs: BTreeMap::new(),
            receipts: Vec::new(),
            ase_spent: 0,
            ase_earned: 0,
            caller_id: caller_id.to_string(),
            caller_tier: tier,
            twin_f1,
            kv: BTreeMap::new(),
            blobs: BTreeMap::new(),
            receipt_counter: 0,
        }
    }

    /// Snapshot the current state as a JSON object.
    pub fn snapshot(&self) -> Value {
        let balances: serde_json::Map<String, Value> = self.balances.iter()
            .map(|(k, v)| (k.clone(), Value::Number((*v).into())))
            .collect();

        let jobs: Vec<Value> = self.jobs.values().map(|j| serde_json::json!({
            "job_id": j.job_id,
            "owner": j.owner,
            "status": j.status,
            "escrow_amount": j.escrow_amount,
        })).collect();

        serde_json::json!({
            "caller_id": self.caller_id,
            "caller_tier": self.caller_tier,
            "twin_f1": self.twin_f1,
            "balances": balances,
            "jobs": jobs,
            "kv_keys": self.kv.keys().collect::<Vec<_>>(),
            "blob_keys": self.blobs.keys().collect::<Vec<_>>(),
        })
    }

    // ── ASE operations ────────────────────────────────────────────────────────

    pub fn balance_of(&self, agent: &str) -> u64 {
        *self.balances.get(agent).unwrap_or(&0)
    }

    pub fn transfer(&mut self, from: &str, to: &str, amount: u64) -> Result<(), String> {
        let from_bal = self.balance_of(from);
        if from_bal < amount {
            return Err(format!(
                "TRANSFER_ASE: insufficient balance for '{}': have {}, need {}",
                from, from_bal, amount
            ));
        }
        *self.balances.entry(from.to_string()).or_default() -= amount;
        *self.balances.entry(to.to_string()).or_default() += amount;

        if from == self.caller_id.as_str() {
            self.ase_spent += amount;
        }
        if to == self.caller_id.as_str() {
            self.ase_earned += amount;
        }
        self.emit_receipt("TRANSFER", &format!("{}→{}:{}", from, to, amount));
        Ok(())
    }

    pub fn burn(&mut self, from: &str, amount: u64) -> Result<(), String> {
        let bal = self.balance_of(from);
        if bal < amount {
            return Err(format!(
                "BURN_ASE: insufficient balance for '{}': have {}, need {}",
                from, bal, amount
            ));
        }
        *self.balances.entry(from.to_string()).or_default() -= amount;
        if from == self.caller_id.as_str() {
            self.ase_spent += amount;
        }
        self.emit_receipt("BURN", &format!("{}:{}", from, amount));
        Ok(())
    }

    pub fn lock(&mut self, from: &str, amount: u64, job_id: &str) -> Result<(), String> {
        let bal = self.balance_of(from);
        if bal < amount {
            return Err(format!(
                "LOCK_ASE: insufficient balance for '{}': have {}, need {}",
                from, bal, amount
            ));
        }
        *self.balances.entry(from.to_string()).or_default() -= amount;
        if from == self.caller_id.as_str() {
            self.ase_spent += amount;
        }
        // Create or update job entry
        let entry = self.jobs.entry(job_id.to_string()).or_insert_with(|| JobEntry {
            job_id: job_id.to_string(),
            owner: from.to_string(),
            status: "escrowed".into(),
            escrow_amount: 0,
        });
        entry.escrow_amount += amount;
        self.emit_receipt("LOCK", &format!("{}:{}:job={}", from, amount, job_id));
        Ok(())
    }

    pub fn emit_ase(&mut self, to: &str, amount: u64) {
        *self.balances.entry(to.to_string()).or_default() += amount;
        if to == self.caller_id.as_str() {
            self.ase_earned += amount;
        }
        self.emit_receipt("EMIT", &format!("{}:{}", to, amount));
    }

    // ── Job operations ────────────────────────────────────────────────────────

    #[allow(dead_code)]
    pub fn create_job(&mut self, job_id: &str, owner: &str) {
        self.jobs.insert(job_id.to_string(), JobEntry {
            job_id: job_id.to_string(),
            owner: owner.to_string(),
            status: "pending".into(),
            escrow_amount: 0,
        });
    }

    // ── KV / blob ─────────────────────────────────────────────────────────────

    pub fn kv_store(&mut self, key: &str, value: Value) {
        self.kv.insert(key.to_string(), value);
    }

    pub fn kv_load(&self, key: &str) -> Value {
        self.kv.get(key).cloned().unwrap_or(Value::Null)
    }

    // ── Receipt ───────────────────────────────────────────────────────────────

    fn emit_receipt(&mut self, op: &str, detail: &str) {
        self.receipt_counter += 1;
        // Deterministic pseudo-hash: sha256 not available without deps, use format
        let hash = format!("sim:{:08x}:{}:{}", self.receipt_counter, op, detail);
        self.receipts.push(hash);
    }
}
