/// A physical GPU device registered to the Omo-Koda2 device tree.
#[derive(Debug, Clone)]
pub struct GpuDevice {
    pub device_id: String,   // /devices/gpu/0001
    pub hardware_id: String, // PCIe BDF or UUID from driver
    pub owner: String,       // agent_id
    pub model: String,       // e.g. "NVIDIA A40"
    pub vram_mb: u64,
    pub compute_units: u32,
    pub available: bool,
    pub driver_version: String,
    pub registered_at: u64,
}

impl GpuDevice {
    pub fn new(
        device_id: impl Into<String>,
        hardware_id: impl Into<String>,
        owner: impl Into<String>,
        model: impl Into<String>,
        vram_mb: u64,
        compute_units: u32,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            hardware_id: hardware_id.into(),
            owner: owner.into(),
            model: model.into(),
            vram_mb,
            compute_units,
            available: true,
            driver_version: String::new(),
            registered_at: now_secs(),
        }
    }
}

/// Capability profile derived from the device's hardware specs.
#[derive(Debug, Clone)]
pub struct GpuCapability {
    pub device_id: String,
    pub fp32_tflops: f64,
    pub fp16_tflops: f64,
    pub mem_bandwidth_gbs: f64,
    pub nvlink: bool,
    pub cuda_arch: Option<String>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
