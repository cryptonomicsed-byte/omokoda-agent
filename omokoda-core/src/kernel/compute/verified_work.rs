use super::attestation::HardwareAttestation;
use super::telemetry::GpuTelemetry;

/// The atomic compute unit in the Omo-Koda2 / OSOVM economy.
///
/// Represents the complete chain:
///   GPU contribution → hardware identity → attestation → lease
///   → actual GPU-seconds → telemetry → Zàngbétò receipt → OSOVM verification
///   → Dopamine credit
///
/// NOT created by self-report. Created only after full chain is satisfied.
#[derive(Debug, Clone)]
pub struct VerifiedGPUWork {
    pub work_id: String,
    pub contributor_id: String, // agent_id
    pub device_id: String,      // /devices/gpu/0001
    pub gpu_model: String,

    // Lease provenance
    pub lease_id: String,
    pub start_time: u64,
    pub end_time: u64,
    pub gpu_seconds: f64,

    // Workload identity
    pub workload_hash: String,
    pub workload_type: WorkloadKind,
    pub utilization: f32, // avg over the lease

    // Commitments — verified before Dopamine is issued
    pub output_commitment: String,    // SHA-256 of workload output
    pub telemetry_commitment: String, // from GpuTelemetry.commitment_hash

    // Proof chain
    pub witness_receipts: Vec<String>, // receipt_ids from independent witnesses
    pub hardware_attestation: HardwareAttestation,
    pub osovm_proof: Option<String>, // filled after OSOVM verification

    // Economic output
    pub compute_score: Option<f64>,
    pub dopamine_allocation: Option<u64>, // micro-Dopamine units

    // Zàngbétò anchor
    pub zangbeto_receipt_id: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkloadKind {
    Training,
    Inference,
    Simulation,
    Rendering,
    ZkProof,
    Encoding,
    Custom(String),
}

impl std::fmt::Display for WorkloadKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Training => write!(f, "training"),
            Self::Inference => write!(f, "inference"),
            Self::Simulation => write!(f, "simulation"),
            Self::Rendering => write!(f, "rendering"),
            Self::ZkProof => write!(f, "zk_proof"),
            Self::Encoding => write!(f, "encoding"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl VerifiedGPUWork {
    pub fn new(
        contributor_id: impl Into<String>,
        device_id: impl Into<String>,
        gpu_model: impl Into<String>,
        lease_id: impl Into<String>,
        start_time: u64,
        end_time: u64,
        gpu_seconds: f64,
        workload_hash: impl Into<String>,
        workload_type: WorkloadKind,
        telemetry: &GpuTelemetry,
        attestation: HardwareAttestation,
        output_commitment: impl Into<String>,
    ) -> Self {
        Self {
            work_id: uuid_v4(),
            contributor_id: contributor_id.into(),
            device_id: device_id.into(),
            gpu_model: gpu_model.into(),
            lease_id: lease_id.into(),
            start_time,
            end_time,
            gpu_seconds,
            workload_hash: workload_hash.into(),
            workload_type,
            utilization: telemetry.avg_utilization,
            output_commitment: output_commitment.into(),
            telemetry_commitment: telemetry.commitment_hash.clone(),
            witness_receipts: vec![],
            hardware_attestation: attestation,
            osovm_proof: None,
            compute_score: None,
            dopamine_allocation: None,
            zangbeto_receipt_id: None,
            created_at: now_secs(),
        }
    }

    pub fn add_witness(&mut self, receipt_id: impl Into<String>) {
        self.witness_receipts.push(receipt_id.into());
    }

    pub fn set_osovm_proof(&mut self, proof: impl Into<String>) {
        self.osovm_proof = Some(proof.into());
    }

    pub fn set_dopamine(&mut self, score: f64, allocation: u64, zangbeto_id: impl Into<String>) {
        self.compute_score = Some(score);
        self.dopamine_allocation = Some(allocation);
        self.zangbeto_receipt_id = Some(zangbeto_id.into());
    }

    /// True only when the full proof chain is satisfied (OSOVM + witnesses + Zàngbétò)
    pub fn is_fully_verified(&self) -> bool {
        self.osovm_proof.is_some()
            && !self.witness_receipts.is_empty()
            && self.zangbeto_receipt_id.is_some()
            && self.hardware_attestation.verified
    }
}

fn uuid_v4() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;
    let mut h = DefaultHasher::new();
    SystemTime::now().hash(&mut h);
    format!(
        "{:016x}{:016x}",
        h.finish(),
        h.finish().wrapping_mul(0xdead_beef)
    )
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── VerifiedPrintJob ──────────────────────────────────────────────────────────

/// A verified 3D print job — the sim-to-real domain analog of VerifiedGPUWork.
///
/// NOT created by self-report.  Created only after:
///   1. Slicer simulation ran and produced a predicted-outcome hash.
///   2. The physical print completed.
///   3. A dimension/mass measurement was taken and hashed.
///   4. A VCP CapabilityGrant authorized the print action.
///   5. A Zàngbétò anchor was issued.
///
/// The sim-to-real bonus (5x base multiplier) is earned when
/// `dimension_within_tolerance` is true — i.e. reality matched the prediction.
#[derive(Debug, Clone)]
pub struct VerifiedPrintJob {
    pub work_id: String,
    pub contributor_id: String, // agent_id
    pub device_id: String,      // VCP device ID of the printer

    // Model provenance
    pub model_hash: String,         // SHA-256 of STL/3MF source
    pub slicer_params_hash: String, // hash of slicer config (layer height, infill, …)
    pub gcode_hash: String,         // hash of the emitted G-code

    // Material
    pub material_type: String, // "PLA" | "PETG" | "ABS" | "TPU" | …
    pub material_batch: Option<String>,

    // Sim predictions (set before printing)
    pub predicted_print_secs: u64,
    pub predicted_filament_g: f32,
    pub predicted_warp_risk: f32,    // 0.0..1.0 from thermal sim
    pub sim_prediction_hash: String, // hash of all predicted fields

    // Actual outcome (set after printing)
    pub actual_print_secs: Option<u64>,
    pub actual_filament_g: Option<f32>,

    // Physical measurement (dim check via calipers or scan)
    pub nominal_dimensions: Option<[f32; 3]>, // [x, y, z] mm
    pub measured_dimensions: Option<[f32; 3]>,
    pub measurement_hash: Option<String>, // SHA-256 of caliper/scan data
    /// True when all measured dims are within 2% of nominal (the sim-to-real bonus condition).
    pub dimension_within_tolerance: bool,

    // Outcome
    pub outcome: PrintOutcome,

    // Proof chain
    pub vcp_grant_id: String, // CapabilityGrant that authorized the print
    pub witness_receipts: Vec<String>,
    pub osovm_proof: Option<String>,
    pub zangbeto_receipt_id: Option<String>,

    // Economic output
    pub compute_score: Option<f64>,
    pub dopamine_allocation: Option<u64>,

    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintOutcome {
    /// Print completed successfully.
    Success,
    /// Print failed — stores the failure mode.
    Failed(PrintFailureMode),
    /// Print still in progress.
    InProgress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintFailureMode {
    Warping,
    NozzleClog,
    LayerDelamination,
    PowerLoss,
    FilamentRunOut,
    Other(String),
}

impl VerifiedPrintJob {
    pub fn new(
        contributor_id: impl Into<String>,
        device_id: impl Into<String>,
        model_hash: impl Into<String>,
        slicer_params_hash: impl Into<String>,
        gcode_hash: impl Into<String>,
        material_type: impl Into<String>,
        predicted_print_secs: u64,
        predicted_filament_g: f32,
        predicted_warp_risk: f32,
        sim_prediction_hash: impl Into<String>,
        vcp_grant_id: impl Into<String>,
    ) -> Self {
        Self {
            work_id: uuid_v4(),
            contributor_id: contributor_id.into(),
            device_id: device_id.into(),
            model_hash: model_hash.into(),
            slicer_params_hash: slicer_params_hash.into(),
            gcode_hash: gcode_hash.into(),
            material_type: material_type.into(),
            material_batch: None,
            predicted_print_secs,
            predicted_filament_g,
            predicted_warp_risk,
            sim_prediction_hash: sim_prediction_hash.into(),
            actual_print_secs: None,
            actual_filament_g: None,
            nominal_dimensions: None,
            measured_dimensions: None,
            measurement_hash: None,
            dimension_within_tolerance: false,
            outcome: PrintOutcome::InProgress,
            vcp_grant_id: vcp_grant_id.into(),
            witness_receipts: vec![],
            osovm_proof: None,
            zangbeto_receipt_id: None,
            compute_score: None,
            dopamine_allocation: None,
            created_at: now_secs(),
        }
    }

    pub fn complete_success(
        &mut self,
        actual_print_secs: u64,
        actual_filament_g: f32,
        measured_dimensions: Option<[f32; 3]>,
        measurement_hash: Option<String>,
    ) {
        self.actual_print_secs = Some(actual_print_secs);
        self.actual_filament_g = Some(actual_filament_g);
        self.measured_dimensions = measured_dimensions;
        self.measurement_hash = measurement_hash;
        self.outcome = PrintOutcome::Success;
        // Check tolerance if dimensions are available
        if let (Some(nom), Some(meas)) = (self.nominal_dimensions, self.measured_dimensions) {
            self.dimension_within_tolerance = nom.iter().zip(meas.iter()).all(|(n, m)| {
                if *n == 0.0 {
                    return true;
                }
                ((m - n).abs() / n) <= 0.02 // 2% tolerance
            });
        }
    }

    pub fn complete_failed(&mut self, mode: PrintFailureMode) {
        self.outcome = PrintOutcome::Failed(mode);
    }

    pub fn add_witness(&mut self, receipt_id: impl Into<String>) {
        self.witness_receipts.push(receipt_id.into());
    }

    pub fn set_osovm_proof(&mut self, proof: impl Into<String>) {
        self.osovm_proof = Some(proof.into());
    }

    pub fn set_dopamine(&mut self, score: f64, allocation: u64, zangbeto_id: impl Into<String>) {
        self.compute_score = Some(score);
        self.dopamine_allocation = Some(allocation);
        self.zangbeto_receipt_id = Some(zangbeto_id.into());
    }

    /// True only when the full proof chain is satisfied.
    pub fn is_fully_verified(&self) -> bool {
        matches!(self.outcome, PrintOutcome::Success)
            && self.osovm_proof.is_some()
            && !self.witness_receipts.is_empty()
            && self.zangbeto_receipt_id.is_some()
            && self.measurement_hash.is_some()
    }

    /// Effective compute score — earns 5x sim-to-real bonus if dims in tolerance.
    pub fn compute_score_effective(&self) -> f64 {
        if !self.is_fully_verified() {
            return 0.0;
        }
        let base = self.actual_print_secs.unwrap_or(0) as f64
            * self.actual_filament_g.unwrap_or(0.0) as f64;
        let mult = if self.dimension_within_tolerance {
            5.0
        } else {
            2.0
        };
        base * mult
    }

    /// Convert to a generalized WorkClaim for OSOVM verification gate.
    pub fn to_work_claim(&self) -> serde_json::Value {
        serde_json::json!({
            "claim_id":    self.work_id,
            "agent_id":    self.contributor_id,
            "device_id":   self.device_id,
            "domain":      if self.dimension_within_tolerance { "sim_to_real" } else { "print_job" },
            "claimed_quantity": self.actual_print_secs.unwrap_or(0) as f64,
            "quantity_unit": "print_seconds",
            "input_commitment":  self.model_hash,
            "output_commitment": self.gcode_hash,
            "measurement_commitment": self.measurement_hash,
            "witness_receipts": self.witness_receipts,
            "zangbeto_anchor": self.zangbeto_receipt_id,
            "bonus_multiplier_override": serde_json::Value::Null,
            "evidence": {
                "require_input_hash": true,
                "require_output_hash": true,
                "require_measurement": true,
            },
            "witness_policy": {
                "min_witnesses": 1,
                "require_zangbeto": true,
            }
        })
    }
}

#[cfg(test)]
mod print_tests {
    use super::*;

    fn stub_job() -> VerifiedPrintJob {
        let mut j = VerifiedPrintJob::new(
            "agent-1",
            "printer-001",
            "stl-hash",
            "slicer-hash",
            "gcode-hash",
            "PLA",
            3600,
            18.5,
            0.05,
            "pred-hash",
            "grant-1",
        );
        j.nominal_dimensions = Some([50.0, 50.0, 10.0]);
        j
    }

    #[test]
    fn not_verified_in_progress() {
        let j = stub_job();
        assert!(!j.is_fully_verified());
    }

    #[test]
    fn verified_after_full_chain() {
        let mut j = stub_job();
        j.complete_success(
            3580,
            18.2,
            Some([50.1, 49.9, 10.0]),
            Some("meas-hash".into()),
        );
        j.add_witness("w-1");
        j.set_osovm_proof("proof-1");
        j.set_dopamine(1000.0, 500, "z-1");
        assert!(j.is_fully_verified());
        assert!(j.dimension_within_tolerance); // within 2%
    }

    #[test]
    fn sim_to_real_multiplier_when_in_tolerance() {
        let mut j = stub_job();
        j.complete_success(
            3600,
            18.5,
            Some([50.0, 50.0, 10.0]),
            Some("meas-hash".into()),
        );
        j.add_witness("w-1");
        j.set_osovm_proof("proof-1");
        j.set_dopamine(0.0, 0, "z-1");
        assert!(j.dimension_within_tolerance);
        let score = j.compute_score_effective();
        // 3600 * 18.5 * 5.0
        assert!((score - 3600.0 * 18.5 * 5.0).abs() < 0.1);
    }

    #[test]
    fn print_multiplier_when_out_of_tolerance() {
        let mut j = stub_job();
        // 10% out of tolerance
        j.complete_success(
            3600,
            18.5,
            Some([55.0, 50.0, 10.0]),
            Some("meas-hash".into()),
        );
        j.add_witness("w-1");
        j.set_osovm_proof("proof-1");
        j.set_dopamine(0.0, 0, "z-1");
        assert!(!j.dimension_within_tolerance);
        let score = j.compute_score_effective();
        assert!((score - 3600.0 * 18.5 * 2.0).abs() < 0.1);
    }

    #[test]
    fn to_work_claim_has_correct_domain() {
        let mut j = stub_job();
        j.complete_success(
            3600,
            18.5,
            Some([50.0, 50.0, 10.0]),
            Some("meas-hash".into()),
        );
        j.add_witness("w-1");
        j.set_osovm_proof("proof-1");
        j.set_dopamine(0.0, 0, "z-1");
        let claim = j.to_work_claim();
        assert_eq!(claim["domain"], "sim_to_real");
    }
}
