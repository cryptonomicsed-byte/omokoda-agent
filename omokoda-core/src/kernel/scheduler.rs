use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Mutex};

use super::process::Pid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Background = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
    Kernel = 4,
}

#[derive(Debug, Clone)]
pub struct JobSlot {
    pub slot_id: String,
    pub pid: Pid,
    pub job_id: String,
    pub priority: Priority,
    pub cpu_quota: u64,         // millis per scheduling window
    pub mem_quota: u64,         // bytes
    pub gpu_quota: Option<f32>, // fraction of a GPU device
    pub enqueued_at: u64,
}

impl PartialEq for JobSlot {
    fn eq(&self, other: &Self) -> bool {
        self.slot_id == other.slot_id
    }
}
impl Eq for JobSlot {}

impl PartialOrd for JobSlot {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JobSlot {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then(other.enqueued_at.cmp(&self.enqueued_at)) // earlier = higher priority at same level
    }
}

#[derive(Debug, Clone)]
pub struct ResourceBudget {
    pub total_cpu_millis: u64,
    pub total_mem_bytes: u64,
    pub total_gpu_slots: u32,
    pub used_cpu_millis: u64,
    pub used_mem_bytes: u64,
    pub used_gpu_slots: u32,
}

impl ResourceBudget {
    pub fn new(cpu: u64, mem: u64, gpu_slots: u32) -> Self {
        Self {
            total_cpu_millis: cpu,
            total_mem_bytes: mem,
            total_gpu_slots: gpu_slots,
            used_cpu_millis: 0,
            used_mem_bytes: 0,
            used_gpu_slots: 0,
        }
    }

    pub fn can_fit(&self, slot: &JobSlot) -> bool {
        (self.used_cpu_millis + slot.cpu_quota <= self.total_cpu_millis)
            && (self.used_mem_bytes + slot.mem_quota <= self.total_mem_bytes)
            && (slot.gpu_quota.is_none() || self.used_gpu_slots < self.total_gpu_slots)
    }

    pub fn allocate(&mut self, slot: &JobSlot) {
        self.used_cpu_millis += slot.cpu_quota;
        self.used_mem_bytes += slot.mem_quota;
        if slot.gpu_quota.is_some() {
            self.used_gpu_slots += 1;
        }
    }

    pub fn release(&mut self, slot: &JobSlot) {
        self.used_cpu_millis = self.used_cpu_millis.saturating_sub(slot.cpu_quota);
        self.used_mem_bytes = self.used_mem_bytes.saturating_sub(slot.mem_quota);
        if slot.gpu_quota.is_some() {
            self.used_gpu_slots = self.used_gpu_slots.saturating_sub(1);
        }
    }
}

pub struct ResourceScheduler {
    queue: Mutex<BinaryHeap<JobSlot>>,
    running: Mutex<HashMap<String, JobSlot>>, // slot_id → slot
    budget: Mutex<ResourceBudget>,
}

impl ResourceScheduler {
    pub fn new(budget: ResourceBudget) -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(BinaryHeap::new()),
            running: Mutex::new(HashMap::new()),
            budget: Mutex::new(budget),
        })
    }

    pub fn enqueue(&self, slot: JobSlot) {
        self.queue.lock().unwrap().push(slot);
    }

    /// Try to dispatch the highest-priority slot that fits current budget.
    pub fn tick(&self) -> Option<JobSlot> {
        let mut queue = self.queue.lock().unwrap();
        let mut budget = self.budget.lock().unwrap();

        let mut candidates: Vec<JobSlot> = queue.drain().collect();
        candidates.sort_unstable_by(|a, b| b.cmp(a)); // highest priority first

        let mut dispatched: Option<JobSlot> = None;
        let mut remainder = Vec::new();

        for slot in candidates {
            if dispatched.is_none() && budget.can_fit(&slot) {
                budget.allocate(&slot);
                dispatched = Some(slot);
            } else {
                remainder.push(slot);
            }
        }

        for s in remainder {
            queue.push(s);
        }

        if let Some(ref s) = dispatched {
            self.running
                .lock()
                .unwrap()
                .insert(s.slot_id.clone(), s.clone());
        }

        dispatched
    }

    pub fn complete(&self, slot_id: &str) {
        let mut running = self.running.lock().unwrap();
        if let Some(slot) = running.remove(slot_id) {
            self.budget.lock().unwrap().release(&slot);
        }
    }

    pub fn running_count(&self) -> usize {
        self.running.lock().unwrap().len()
    }

    pub fn queued_count(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    pub fn budget_snapshot(&self) -> ResourceBudget {
        self.budget.lock().unwrap().clone()
    }
}
