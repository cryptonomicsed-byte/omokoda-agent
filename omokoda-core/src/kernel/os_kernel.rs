//! OsKernel — sovereign OS façade.
//!
//! Composes all kernel subsystems into a single owned struct that can be
//! mounted into AppState.  Every subsystem is Arc-wrapped internally so
//! cloning OsKernel is cheap (used by Axum's State<Arc<AppState>>).
//!
//! Lifecycle contract:
//!   on_birth(agent_id, tier) — allocates a PID, initialises FS tree,
//!                               registers IPC channel, grants defaults.
//!   on_turn_start(pid)       — transitions process → Running.
//!   on_turn_end(pid)         — transitions process → WaitingForWork.
//!   on_terminate(pid)        — transitions → Terminating, reaps resources.

use std::sync::Arc;

use super::{
    capability::{
        CapabilityGrant, CapabilityKind, CapabilityPolicy, CapabilityStore, OsCapability,
    },
    compute::ComputeManager,
    device::DeviceManager,
    fs::SovereignFS,
    ipc::{IpcMessage, SovereignIPC},
    process::{Pid, ProcessState, ProcessTable},
    scheduler::{JobSlot, Priority, ResourceBudget, ResourceScheduler},
    security::{PolicyEnforcer, SecurityPolicy},
};

// ── OsKernel ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct OsKernel {
    pub processes: Arc<ProcessTable>,
    pub ipc: Arc<SovereignIPC>,
    pub capabilities: Arc<CapabilityStore>,
    pub fs: Arc<SovereignFS>,
    pub scheduler: Arc<ResourceScheduler>,
    pub devices: Arc<DeviceManager>,
    pub compute: Arc<ComputeManager>,
    pub security: Arc<PolicyEnforcer>,
}

impl OsKernel {
    /// Boot a new kernel with sensible defaults.
    pub fn new() -> Self {
        let budget = ResourceBudget::new(
            64_000,                 // 64 000 cpu-millis per window
            4 * 1024 * 1024 * 1024, // 4 GiB
            4,                      // 4 GPU slots
        );
        Self {
            processes: ProcessTable::new(),
            ipc: SovereignIPC::new(),
            capabilities: CapabilityStore::new(CapabilityPolicy::default()),
            fs: Arc::new(SovereignFS::new()),
            scheduler: ResourceScheduler::new(budget),
            devices: Arc::new(DeviceManager::new()),
            compute: Arc::new(ComputeManager::default()),
            security: Arc::new(PolicyEnforcer::new(SecurityPolicy::default())),
        }
    }

    // ── Agent lifecycle ───────────────────────────────────────────────────────

    /// Called when an agent is born or resumed from a snapshot.
    /// Allocates a PID, bootstraps the FS subtree, grants default
    /// capabilities, and opens an IPC channel.
    /// Returns the new PID.
    pub fn on_birth(&self, agent_id: &str, boot_id: &str) -> Pid {
        let pid = self.processes.spawn(agent_id, boot_id);

        // FS subtree for this agent
        self.fs.init_agent(agent_id);

        // IPC channel (capacity 64 messages)
        self.ipc.register(pid, 64);

        // Default capabilities — granted to every freshly born agent
        for kind in [
            CapabilityKind::MemoryReadOwn,
            CapabilityKind::MemoryWriteOwn,
            CapabilityKind::NetworkReceive,
            CapabilityKind::SkillExecute,
            CapabilityKind::WalletRead,
        ] {
            self.capabilities.grant(CapabilityGrant {
                pid,
                agent_id: agent_id.to_string(),
                capability: OsCapability::new(kind, "*", "kernel"),
                receipt_id: None,
            });
        }

        // Transition: Booting → WaitingForWork
        self.processes.transition(pid, ProcessState::WaitingForWork);

        pid
    }

    /// Mark an agent as actively executing a turn.
    pub fn on_turn_start(&self, pid: Pid) {
        self.processes.transition(pid, ProcessState::Running);
    }

    /// Mark an agent as idle after finishing a turn.
    pub fn on_turn_end(&self, pid: Pid) {
        self.processes.transition(pid, ProcessState::WaitingForWork);
    }

    /// Gracefully shut down an agent process and release its resources.
    pub fn on_terminate(&self, pid: Pid) {
        self.processes.transition(pid, ProcessState::Terminating);
        self.ipc.unregister(pid);
        self.capabilities.evict(pid);
        self.processes.reap(pid);
    }

    // ── IPC helpers ───────────────────────────────────────────────────────────

    pub async fn send(&self, pid: Pid, msg: IpcMessage) -> bool {
        self.ipc.send(pid, msg).await
    }

    pub async fn broadcast(&self, msg: IpcMessage) -> usize {
        self.ipc.broadcast(msg).await
    }

    // ── Scheduler helpers ─────────────────────────────────────────────────────

    pub fn enqueue_job(&self, pid: Pid, job_id: impl Into<String>, priority: Priority) -> String {
        let slot_id = format!("{pid}:{}", job_id.into());
        let slot = JobSlot {
            slot_id: slot_id.clone(),
            pid,
            job_id: slot_id.clone(),
            priority,
            cpu_quota: 1_000,
            mem_quota: 64 * 1024 * 1024, // 64 MiB
            gpu_quota: None,
            enqueued_at: now_secs(),
        };
        self.scheduler.enqueue(slot);
        slot_id
    }

    // ── Status snapshot ───────────────────────────────────────────────────────

    pub fn status(&self) -> KernelStatus {
        let procs = self.processes.list();
        let budget = self.scheduler.budget_snapshot();
        KernelStatus {
            process_count: procs.len(),
            running_count: procs
                .iter()
                .filter(|p| matches!(p.state, ProcessState::Running))
                .count(),
            waiting_count: procs
                .iter()
                .filter(|p| matches!(p.state, ProcessState::WaitingForWork))
                .count(),
            queued_jobs: self.scheduler.queued_count(),
            running_jobs: self.scheduler.running_count(),
            device_count: self.devices.tree.list_all().len(),
            cpu_used_millis: budget.used_cpu_millis,
            cpu_total_millis: budget.total_cpu_millis,
            mem_used_bytes: budget.used_mem_bytes,
            mem_total_bytes: budget.total_mem_bytes,
            gpu_used_slots: budget.used_gpu_slots,
            gpu_total_slots: budget.total_gpu_slots,
            processes: procs.into_iter().map(ProcessSummary::from).collect(),
        }
    }
}

impl Default for OsKernel {
    fn default() -> Self {
        Self::new()
    }
}

// ── Status types (JSON-serialisable) ─────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct KernelStatus {
    pub process_count: usize,
    pub running_count: usize,
    pub waiting_count: usize,
    pub queued_jobs: usize,
    pub running_jobs: usize,
    pub device_count: usize,
    pub cpu_used_millis: u64,
    pub cpu_total_millis: u64,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub gpu_used_slots: u32,
    pub gpu_total_slots: u32,
    pub processes: Vec<ProcessSummary>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProcessSummary {
    pub pid: u64,
    pub agent_id: String,
    pub state: String,
    pub started_at: u64,
}

impl From<super::process::AgentProcess> for ProcessSummary {
    fn from(p: super::process::AgentProcess) -> Self {
        Self {
            pid: p.pid,
            agent_id: p.agent_id,
            state: format!("{:?}", p.state),
            started_at: p.started_at,
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birth_allocates_pid_and_initialises_fs() {
        let kernel = OsKernel::new();
        let pid = kernel.on_birth("agent-alpha", "boot-1");
        assert!(pid > 0);

        // process exists and is WaitingForWork
        let proc = kernel.processes.get(pid).expect("process must exist");
        assert_eq!(proc.agent_id, "agent-alpha");
        assert!(matches!(proc.state, ProcessState::WaitingForWork));

        // FS subtree was created
        let listing = kernel.fs.list("/agents/agent-alpha", "agent-alpha");
        assert!(
            !listing.is_empty(),
            "agent FS subtree must be non-empty after birth"
        );
    }

    #[test]
    fn turn_lifecycle_transitions_state() {
        let kernel = OsKernel::new();
        let pid = kernel.on_birth("agent-beta", "boot-1");

        kernel.on_turn_start(pid);
        assert!(matches!(
            kernel.processes.get(pid).unwrap().state,
            ProcessState::Running
        ));

        kernel.on_turn_end(pid);
        assert!(matches!(
            kernel.processes.get(pid).unwrap().state,
            ProcessState::WaitingForWork
        ));
    }

    #[test]
    fn terminate_reaps_process() {
        let kernel = OsKernel::new();
        let pid = kernel.on_birth("agent-gamma", "boot-1");
        kernel.on_terminate(pid);
        assert!(kernel.processes.get(pid).is_none());
    }

    #[test]
    fn status_reflects_live_processes() {
        let kernel = OsKernel::new();
        let pid1 = kernel.on_birth("agent-a", "b1");
        let pid2 = kernel.on_birth("agent-b", "b2");
        kernel.on_turn_start(pid1);

        let status = kernel.status();
        assert_eq!(status.process_count, 2);
        assert_eq!(status.running_count, 1);
        assert_eq!(status.waiting_count, 1);

        let _ = pid2; // keep alive
    }

    #[test]
    fn default_capabilities_granted_at_birth() {
        let kernel = OsKernel::new();
        let pid = kernel.on_birth("agent-cap", "boot-1");

        assert!(kernel
            .capabilities
            .check(pid, &CapabilityKind::MemoryReadOwn, "*"));
        assert!(kernel
            .capabilities
            .check(pid, &CapabilityKind::MemoryWriteOwn, "*"));
        assert!(kernel
            .capabilities
            .check(pid, &CapabilityKind::SkillExecute, "*"));
        // Non-default capability must not be auto-granted
        assert!(!kernel
            .capabilities
            .check(pid, &CapabilityKind::GpuAccess, "*"));
    }

    #[test]
    fn multiple_agents_have_independent_processes() {
        let kernel = OsKernel::new();
        let pid_a = kernel.on_birth("agent-x", "b1");
        let pid_b = kernel.on_birth("agent-y", "b2");
        assert_ne!(pid_a, pid_b);

        kernel.on_turn_start(pid_a);
        // agent-y should still be WaitingForWork
        assert!(matches!(
            kernel.processes.get(pid_b).unwrap().state,
            ProcessState::WaitingForWork
        ));
    }

    #[test]
    fn enqueue_job_adds_to_scheduler() {
        let kernel = OsKernel::new();
        let pid = kernel.on_birth("agent-sched", "b1");
        kernel.enqueue_job(pid, "job-001", Priority::Normal);
        assert_eq!(kernel.scheduler.queued_count(), 1);
    }
}
