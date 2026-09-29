use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::process::Pid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    // Compute
    GpuAccess,
    NpuAccess,
    CpuHighPriority,
    // Devices
    CameraCapture,
    MicrophoneCapture,
    NetworkTransmit,
    NetworkReceive,
    // Storage
    MemoryReadOwn,
    MemoryWriteOwn,
    MemoryReadShared,
    EvidenceWrite,
    // Execution
    SpawnProcess,
    KillProcess,
    SkillExecute,
    DaemonRegister,
    // Economic
    WalletRead,
    WalletSign,
    TradeExecute,
    // Physical
    DroneControl,
    RobotControl,
    SensorRead,
    // Admin (T5 only)
    CapabilityGrant,
    CapabilityRevoke,
    KernelInspect,
}

#[derive(Debug, Clone)]
pub struct OsCapability {
    pub kind: CapabilityKind,
    pub resource: String,   // specific resource path/id or "*" for all
    pub granted_by: String, // granting agent_id or "kernel"
    pub granted_at: u64,
    pub expires_at: Option<u64>,
}

impl OsCapability {
    pub fn new(
        kind: CapabilityKind,
        resource: impl Into<String>,
        granted_by: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            resource: resource.into(),
            granted_by: granted_by.into(),
            granted_at: now_secs(),
            expires_at: None,
        }
    }

    pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.expires_at = Some(self.granted_at + ttl_secs);
        self
    }

    pub fn is_valid(&self) -> bool {
        match self.expires_at {
            Some(exp) => now_secs() < exp,
            None => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityGrant {
    pub pid: Pid,
    pub agent_id: String,
    pub capability: OsCapability,
    pub receipt_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CapabilityPolicy {
    /// Capabilities implicitly granted to all processes at boot
    pub defaults: HashSet<CapabilityKind>,
    /// Capabilities that require T5 tier
    pub t5_only: HashSet<CapabilityKind>,
    /// Capabilities that require a signed grant
    pub gated: HashSet<CapabilityKind>,
}

impl Default for CapabilityPolicy {
    fn default() -> Self {
        Self {
            defaults: [
                CapabilityKind::MemoryReadOwn,
                CapabilityKind::MemoryWriteOwn,
                CapabilityKind::NetworkReceive,
                CapabilityKind::SkillExecute,
                CapabilityKind::WalletRead,
            ]
            .into(),
            t5_only: [
                CapabilityKind::CapabilityGrant,
                CapabilityKind::CapabilityRevoke,
                CapabilityKind::KernelInspect,
                CapabilityKind::KillProcess,
            ]
            .into(),
            gated: [
                CapabilityKind::GpuAccess,
                CapabilityKind::NpuAccess,
                CapabilityKind::WalletSign,
                CapabilityKind::TradeExecute,
                CapabilityKind::DroneControl,
                CapabilityKind::RobotControl,
                CapabilityKind::DaemonRegister,
                CapabilityKind::EvidenceWrite,
                CapabilityKind::NetworkTransmit,
                CapabilityKind::SpawnProcess,
            ]
            .into(),
        }
    }
}

pub struct CapabilityStore {
    grants: RwLock<HashMap<Pid, Vec<CapabilityGrant>>>,
    policy: CapabilityPolicy,
}

impl CapabilityStore {
    pub fn new(policy: CapabilityPolicy) -> Arc<Self> {
        Arc::new(Self {
            grants: RwLock::new(HashMap::new()),
            policy,
        })
    }

    pub fn grant(&self, grant: CapabilityGrant) {
        self.grants
            .write()
            .unwrap()
            .entry(grant.pid)
            .or_default()
            .push(grant);
    }

    pub fn revoke(&self, pid: Pid, kind: &CapabilityKind) {
        let mut map = self.grants.write().unwrap();
        if let Some(grants) = map.get_mut(&pid) {
            grants.retain(|g| &g.capability.kind != kind);
        }
    }

    pub fn check(&self, pid: Pid, kind: &CapabilityKind, _resource: &str) -> bool {
        if self.policy.defaults.contains(kind) {
            return true;
        }
        let map = self.grants.read().unwrap();
        map.get(&pid)
            .map(|grants| {
                grants
                    .iter()
                    .any(|g| &g.capability.kind == kind && g.capability.is_valid())
            })
            .unwrap_or(false)
    }

    pub fn list(&self, pid: Pid) -> Vec<CapabilityGrant> {
        self.grants
            .read()
            .unwrap()
            .get(&pid)
            .cloned()
            .unwrap_or_default()
    }

    pub fn evict(&self, pid: Pid) {
        self.grants.write().unwrap().remove(&pid);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
