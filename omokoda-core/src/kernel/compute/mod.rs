pub mod accounting;
pub mod attestation;
pub mod gpu;
pub mod lease;
pub mod telemetry;
pub mod verified_work;
pub mod wallet;

pub use accounting::{ComputeScore, DopamineAllocation};
pub use attestation::HardwareAttestation;
pub use gpu::{GpuCapability, GpuDevice};
pub use lease::{GpuLease, LeaseState};
pub use telemetry::{GpuTelemetry, TelemetryPoint};
pub use verified_work::VerifiedGPUWork;
pub use wallet::{
    AgentComputeWallet, ComputeLedgerEntry, StakeLock, AGENT_DOPAMINE_ENDOWMENT,
    AGENT_SYNAPSE_ENDOWMENT, DOPAMINE_DAILY_DECAY, FORK_STAKE_FRACTION, LOW_WATER_MARK,
    SYNAPSE_CONVERSION_RATIO,
};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Top-level facade: owns the GPU device registry and lease table.
pub struct ComputeManager {
    pub devices: RwLock<HashMap<String, GpuDevice>>,
    pub leases: RwLock<HashMap<String, GpuLease>>,
}

impl ComputeManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: RwLock::new(HashMap::new()),
            leases: RwLock::new(HashMap::new()),
        })
    }

    pub fn register_gpu(&self, device: GpuDevice) -> String {
        let id = device.device_id.clone();
        self.devices.write().unwrap().insert(id.clone(), device);
        id
    }

    pub fn open_lease(&self, lease: GpuLease) -> String {
        let id = lease.lease_id.clone();
        self.leases.write().unwrap().insert(id.clone(), lease);
        id
    }

    pub fn close_lease(&self, lease_id: &str) -> Option<GpuLease> {
        self.leases.write().unwrap().remove(lease_id)
    }

    pub fn available_gpus(&self) -> Vec<GpuDevice> {
        self.devices
            .read()
            .unwrap()
            .values()
            .filter(|d| d.available)
            .cloned()
            .collect()
    }
}

impl Default for ComputeManager {
    fn default() -> Self {
        Self {
            devices: RwLock::new(HashMap::new()),
            leases: RwLock::new(HashMap::new()),
        }
    }
}
