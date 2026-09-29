use gix_types::{Gix1, GixKind, GixNamespace, RoutingHints};
use hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a prefix-coded task ID.
/// Think→t, Act→a, Dream→d, Delegate→g, Background→b, Agent→ag, Monitor→m
pub fn prefixed_id(prefix: &str) -> String {
    use uuid::Uuid;
    let raw = Uuid::new_v4().to_string().replace('-', "");
    format!("{}{}", prefix, &raw[..8])
}

/// Lifecycle state of a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Queued, not yet started
    Pending,
    /// Currently executing
    Running,
    /// Finished with output
    Completed(String),
    /// Finished with error
    Failed(String),
    /// Cancelled before completion
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed(_) | Self::Failed(_) | Self::Cancelled)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed(_) => "completed",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// The kind of work a task performs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskKind {
    /// LLM think with optional privacy
    Think { prompt: String, private: bool },
    /// Tool act, optionally sandboxed
    Act {
        tool: String,
        params: String,
        sandbox: bool,
    },
    /// Background memory consolidation via DreamEngine
    Dream {
        /// Optional hint about what memory region to consolidate
        consolidation_target: Option<String>,
    },
    /// Delegate to a swarm agent (local process or remote node)
    Delegate {
        agent_id: String,
        task_description: String,
        /// Erlang/OTP node name for cross-node delegation; None = local
        swarm_node: Option<String>,
    },
    /// Background work described in prose (queued for later execution)
    Background { description: String },
    /// Delegate to another agent (legacy alias — prefer Delegate)
    Agent {
        agent_id: String,
        prompt: String,
        max_turns: u32,
    },
    /// Watch an MCP server or process and react to state changes
    Monitor { target: String },
    /// Explicit delegation to a remote agent node
    RemoteAgent {
        node_url: String,
        agent_id: String,
        prompt: String,
    },
}

impl TaskKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Think { .. } => "think",
            Self::Act { .. } => "act",
            Self::Dream { .. } => "dream",
            Self::Delegate { .. } => "delegate",
            Self::Background { .. } => "background",
            Self::Agent { .. } => "agent",
            Self::Monitor { .. } => "monitor",
            Self::RemoteAgent { .. } => "remote_agent",
        }
    }

    pub fn id_prefix(&self) -> &'static str {
        match self {
            Self::Think { .. } => "t",
            Self::Act { .. } => "a",
            Self::Dream { .. } => "d",
            Self::Delegate { .. } => "g",
            Self::Background { .. } => "b",
            Self::Agent { .. } => "ag",
            Self::Monitor { .. } => "m",
            Self::RemoteAgent { .. } => "ag",
        }
    }

    pub fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Act { .. }
                | Self::Agent { .. }
                | Self::Delegate { .. }
                | Self::RemoteAgent { .. }
        )
    }

    pub fn is_async(&self) -> bool {
        matches!(
            self,
            Self::Dream { .. }
                | Self::Background { .. }
                | Self::Delegate { .. }
                | Self::Monitor { .. }
                | Self::RemoteAgent { .. }
        )
    }
}

/// A single unit of work with full lifecycle tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    /// Optional parent task that spawned this one
    pub parent_id: Option<String>,
    /// Priority — higher runs first (default: 0)
    pub priority: i32,
    /// Whether completion was surfaced to the user
    pub notified: bool,
    /// GIX1 canonical_id (hex) stamped when this task completes.
    /// `GixNamespace::OsovmExecution` — links the task into the GIX receipt graph.
    #[serde(default)]
    pub gix1_canonical_id: Option<String>,
}

impl Task {
    pub fn new(kind: TaskKind) -> Self {
        let id = prefixed_id(kind.id_prefix());
        Self {
            id,
            kind,
            status: TaskStatus::Pending,
            created_at: now_secs(),
            started_at: None,
            finished_at: None,
            parent_id: None,
            priority: 0,
            notified: false,
            gix1_canonical_id: None,
        }
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn duration_ms(&self) -> Option<u64> {
        match (self.started_at, self.finished_at) {
            (Some(s), Some(f)) => Some((f.saturating_sub(s)) * 1000),
            _ => None,
        }
    }
}

/// Task registry — holds all tasks and manages state transitions
#[derive(Debug, Default)]
pub struct TaskManager {
    tasks: HashMap<String, Task>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Submit a new task; returns its ID
    pub fn submit(&mut self, kind: TaskKind) -> String {
        let task = Task::new(kind);
        let id = task.id.clone();
        self.tasks.insert(id.clone(), task);
        id
    }

    /// Submit a child task that belongs to a parent
    pub fn submit_child(&mut self, kind: TaskKind, parent_id: &str) -> String {
        let task = Task::new(kind).with_parent(parent_id.to_string());
        let id = task.id.clone();
        self.tasks.insert(id.clone(), task);
        id
    }

    pub fn start(&mut self, id: &str) {
        if let Some(task) = self.tasks.get_mut(id) {
            if task.status == TaskStatus::Pending {
                task.status = TaskStatus::Running;
                task.started_at = Some(now_secs());
            }
        }
    }

    pub fn complete(&mut self, id: &str, output: String) {
        if let Some(task) = self.tasks.get_mut(id) {
            if task.status == TaskStatus::Running {
                task.status = TaskStatus::Completed(output);
                task.finished_at = Some(now_secs());
                // GIX Phase 4: stamp a GIX1 envelope on job/task completion.
                let created_at_ms = task.finished_at.unwrap_or(0).saturating_mul(1000);
                let env = Gix1::new(
                    GixKind::Receipt,
                    GixNamespace::OsovmExecution,
                    task.id.as_bytes(),
                    None,
                    created_at_ms,
                    RoutingHints::default(),
                );
                task.gix1_canonical_id = Some(hex::encode(env.canonical_id));
            }
        }
    }

    pub fn fail(&mut self, id: &str, error: String) {
        if let Some(task) = self.tasks.get_mut(id) {
            if !task.status.is_terminal() {
                task.status = TaskStatus::Failed(error);
                task.finished_at = Some(now_secs());
            }
        }
    }

    pub fn cancel(&mut self, id: &str) {
        if let Some(task) = self.tasks.get_mut(id) {
            if !task.status.is_terminal() {
                task.status = TaskStatus::Cancelled;
                task.finished_at = Some(now_secs());
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&Task> {
        self.tasks.get(id)
    }

    pub fn status(&self, id: &str) -> Option<&TaskStatus> {
        self.tasks.get(id).map(|t| &t.status)
    }

    pub fn list_by_status(&self, status: &TaskStatus) -> Vec<&Task> {
        let label = status.label();
        let mut tasks: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| t.status.label() == label)
            .collect();
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        tasks
    }

    pub fn children_of(&self, parent_id: &str) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| t.parent_id.as_deref() == Some(parent_id))
            .collect();
        tasks.sort_by_key(|t| t.created_at);
        tasks
    }

    pub fn all(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        tasks
    }

    pub fn pending_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .count()
    }

    pub fn running_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Running)
            .count()
    }

    /// Count of non-terminal tasks (pending + running).
    pub fn active_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| !t.status.is_terminal())
            .count()
    }

    /// All tasks of a given label (e.g. "think", "act").
    pub fn by_kind(&self, kind_label: &str) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self
            .tasks
            .values()
            .filter(|t| t.kind.label() == kind_label)
            .collect();
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        tasks
    }

    pub fn summary(&self) -> String {
        let total = self.tasks.len();
        let pending = self.pending_count();
        let running = self.running_count();
        let completed = self
            .tasks
            .values()
            .filter(|t| matches!(t.status, TaskStatus::Completed(_)))
            .count();
        let failed = self
            .tasks
            .values()
            .filter(|t| matches!(t.status, TaskStatus::Failed(_)))
            .count();
        format!(
            "Tasks: {} total | {} pending | {} running | {} completed | {} failed",
            total, pending, running, completed, failed
        )
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod gix_tests {
    use super::*;

    fn run_task(kind: TaskKind) -> Task {
        let mut mgr = TaskManager::new();
        let id = mgr.submit(kind);
        mgr.start(&id);
        mgr.complete(&id, "done".into());
        mgr.get(&id).unwrap().clone()
    }

    #[test]
    fn complete_stamps_gix1_canonical_id() {
        let task = run_task(TaskKind::Think {
            prompt: "test".into(),
            private: false,
        });
        assert!(task.gix1_canonical_id.is_some());
        let hex = task.gix1_canonical_id.unwrap();
        assert_eq!(hex.len(), 64, "canonical_id should be 32-byte hex");
    }

    #[test]
    fn pending_task_has_no_gix1() {
        let mut mgr = TaskManager::new();
        let id = mgr.submit(TaskKind::Think {
            prompt: "x".into(),
            private: false,
        });
        assert!(mgr.get(&id).unwrap().gix1_canonical_id.is_none());
    }

    #[test]
    fn two_tasks_have_distinct_gix1_ids() {
        let t1 = run_task(TaskKind::Think {
            prompt: "a".into(),
            private: false,
        });
        let t2 = run_task(TaskKind::Think {
            prompt: "b".into(),
            private: false,
        });
        assert_ne!(t1.gix1_canonical_id, t2.gix1_canonical_id);
    }

    #[test]
    fn failed_task_has_no_gix1() {
        let mut mgr = TaskManager::new();
        let id = mgr.submit(TaskKind::Think {
            prompt: "x".into(),
            private: false,
        });
        mgr.start(&id);
        mgr.fail(&id, "oops".into());
        assert!(mgr.get(&id).unwrap().gix1_canonical_id.is_none());
    }
}
