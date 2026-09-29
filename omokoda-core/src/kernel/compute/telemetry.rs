/// A single telemetry sample emitted by a GPU device during a lease window.
#[derive(Debug, Clone)]
pub struct TelemetryPoint {
    pub ts: u64,          // unix seconds
    pub utilization: f32, // 0.0 – 1.0 GPU SM utilization
    pub mem_used_mb: u64,
    pub power_w: f32,
    pub temp_c: f32,
    pub sm_clock_mhz: u32,
    pub mem_clock_mhz: u32,
    pub pcie_tx_mb: u64,
    pub pcie_rx_mb: u64,
}

/// Aggregated telemetry for a complete lease window.
#[derive(Debug, Clone)]
pub struct GpuTelemetry {
    pub lease_id: String,
    pub device_id: String,
    pub agent_id: String,
    pub samples: Vec<TelemetryPoint>,
    pub avg_utilization: f32,
    pub peak_utilization: f32,
    pub total_energy_wh: f64,    // watt-hours consumed
    pub commitment_hash: String, // SHA-256 of serialized samples
}

impl GpuTelemetry {
    pub fn from_samples(
        lease_id: impl Into<String>,
        device_id: impl Into<String>,
        agent_id: impl Into<String>,
        samples: Vec<TelemetryPoint>,
    ) -> Self {
        let avg_util = if samples.is_empty() {
            0.0
        } else {
            samples.iter().map(|s| s.utilization).sum::<f32>() / samples.len() as f32
        };
        let peak_util = samples.iter().map(|s| s.utilization).fold(0.0f32, f32::max);

        // Rough energy: average power × duration in hours
        let total_energy_wh = if samples.len() < 2 {
            0.0
        } else {
            let duration_h = (samples.last().unwrap().ts - samples[0].ts) as f64 / 3600.0;
            let avg_power =
                samples.iter().map(|s| s.power_w as f64).sum::<f64>() / samples.len() as f64;
            avg_power * duration_h
        };

        let commitment_hash = commitment_hash_of(&samples);

        Self {
            lease_id: lease_id.into(),
            device_id: device_id.into(),
            agent_id: agent_id.into(),
            samples,
            avg_utilization: avg_util,
            peak_utilization: peak_util,
            total_energy_wh,
            commitment_hash,
        }
    }
}

fn commitment_hash_of(samples: &[TelemetryPoint]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    for s in samples {
        s.ts.hash(&mut h);
        s.mem_used_mb.hash(&mut h);
        s.sm_clock_mhz.hash(&mut h);
    }
    format!(
        "{:016x}{:016x}",
        h.finish(),
        h.finish().wrapping_mul(0x1234_5678)
    )
}
