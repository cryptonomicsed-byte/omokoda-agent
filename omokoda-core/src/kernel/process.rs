use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Pid = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    Booting,
    Running,
    Suspended,
    WaitingForWork,
    Executing { job_id: String },
    Terminating,
    Terminated,
}

#[derive(Debug, Clone)]
pub struct AgentProcess {
    pub pid: Pid,
    pub agent_id: String,
    pub boot_id: String,
    pub state: ProcessState,
    pub started_at: u64,
    pub updated_at: u64,
    /// Current resource consumption snapshot
    pub cpu_millis: u64,
    pub mem_bytes: u64,
}

impl AgentProcess {
    pub fn new(pid: Pid, agent_id: impl Into<String>, boot_id: impl Into<String>) -> Self {
        let now = now_secs();
        Self {
            pid,
            agent_id: agent_id.into(),
            boot_id: boot_id.into(),
            state: ProcessState::Booting,
            started_at: now,
            updated_at: now,
            cpu_millis: 0,
            mem_bytes: 0,
        }
    }

    pub fn transition(&mut self, next: ProcessState) {
        self.state = next;
        self.updated_at = now_secs();
    }
}

#[derive(Debug, Default)]
pub struct ProcessTable {
    inner: RwLock<HashMap<Pid, AgentProcess>>,
    next_pid: std::sync::atomic::AtomicU64,
}

impl ProcessTable {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(HashMap::new()),
            next_pid: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub fn alloc_pid(&self) -> Pid {
        self.next_pid
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn spawn(&self, agent_id: impl Into<String>, boot_id: impl Into<String>) -> Pid {
        let pid = self.alloc_pid();
        let proc = AgentProcess::new(pid, agent_id, boot_id);
        self.inner.write().unwrap().insert(pid, proc);
        pid
    }

    pub fn get(&self, pid: Pid) -> Option<AgentProcess> {
        self.inner.read().unwrap().get(&pid).cloned()
    }

    pub fn transition(&self, pid: Pid, next: ProcessState) -> bool {
        let mut table = self.inner.write().unwrap();
        if let Some(p) = table.get_mut(&pid) {
            p.transition(next);
            true
        } else {
            false
        }
    }

    pub fn reap(&self, pid: Pid) {
        self.inner.write().unwrap().remove(&pid);
    }

    pub fn list(&self) -> Vec<AgentProcess> {
        self.inner.read().unwrap().values().cloned().collect()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
