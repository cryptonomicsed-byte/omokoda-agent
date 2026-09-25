//! System prompt assembly: injects workspace context, identity, and available tools
//! into a coherent system prompt for the think primitive.

use crate::config::FeatureFlags;
use crate::identity::odu::OduIdentity;
use crate::identity::AgentId;
use std::path::PathBuf;

// ── Vessel indices (canonical order from VESSEL_NAMES in ori.rs) ─────────────
const V_ATTENTION: usize = 2;
const V_LOOP: usize = 3;
const V_EXECUTION: usize = 7;
const V_SWARM: usize = 8;
const V_RESTRAINT: usize = 9;
const V_CONSENT: usize = 11;
const V_VISION: usize = 12;
const V_GROWTH: usize = 13;
const V_RHYTHM: usize = 15;

/// Behavioral register derived from Orí vessel weights.
/// Not user-configured — derived deterministically from the agent's birth state.
struct PersonalityRegister {
    register: &'static str, // how the agent speaks / processes
    stance: &'static str,   // how the agent orients toward events
    horizon: &'static str,  // how far ahead the agent reasons
    tempo: &'static str,    // how the agent paces action
}

impl PersonalityRegister {
    fn derive(w: &[f32; 16]) -> Self {
        let register = if w[V_RESTRAINT] > 0.7 {
            "measured"
        } else if w[V_EXECUTION] > 0.7 {
            "swift"
        } else if (w[V_RESTRAINT] + w[V_EXECUTION]) / 2.0 > 0.5 {
            "deliberate"
        } else {
            "expansive"
        };

        let stance = if w[V_ATTENTION] > 0.7 {
            "observant"
        } else if w[V_CONSENT] > 0.7 {
            "receptive"
        } else if w[V_RESTRAINT] > 0.6 {
            "protective"
        } else {
            "generative"
        };

        let horizon = if w[V_VISION] > 0.7 && w[V_GROWTH] > 0.6 {
            "systemic"
        } else if w[V_VISION] > 0.6 {
            "far"
        } else if w[V_GROWTH] > 0.6 {
            "near"
        } else {
            "present"
        };

        let tempo = if w[V_RHYTHM] > 0.7 {
            "rhythmic"
        } else if w[V_LOOP] > 0.6 {
            "steady"
        } else if w[V_RESTRAINT] > 0.7 {
            "patient"
        } else {
            "urgent"
        };

        PersonalityRegister { register, stance, horizon, tempo }
    }
}

/// Builds the system prompt for an agent's think cycle
pub struct SystemPromptBuilder {
    pub agent_name: String,
    pub agent_id: AgentId,
    pub tier: u8,
    pub reputation: f64,
    pub odu_identity: OduIdentity,
    pub workspace_root: PathBuf,
    pub feature_flags: FeatureFlags,
    pub available_tools: Vec<String>,
    pub custom_instructions: Vec<String>,
    /// Orí vessel weights — when set, injects agent-owned personality register
    /// into the system prompt. Derived from birth entropy; never user-configured.
    pub vessel_weights: Option<[f32; 16]>,
}

impl SystemPromptBuilder {
    pub fn new(
        agent_name: &str,
        agent_id: AgentId,
        tier: u8,
        reputation: f64,
        odu_identity: OduIdentity,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            agent_name: agent_name.to_string(),
            agent_id,
            tier,
            reputation,
            odu_identity,
            workspace_root,
            feature_flags: FeatureFlags::default(),
            available_tools: Vec::new(),
            custom_instructions: Vec::new(),
            vessel_weights: None,
        }
    }

    pub fn with_feature_flags(mut self, flags: FeatureFlags) -> Self {
        self.feature_flags = flags;
        self
    }

    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.available_tools = tools;
        self
    }

    /// Inject operator-level habitat instructions (workspace rules, task context).
    /// MUST NOT be used to author agent identity, personality, or Orí.
    /// Identity is derived from vessel weights, not from this field.
    pub fn with_instructions(mut self, instructions: Vec<String>) -> Self {
        self.custom_instructions = instructions;
        self
    }

    /// Inject the agent's Orí vessel weights to derive behavioral register.
    /// Derived from birth entropy — never user-configured.
    pub fn with_vessel_weights(mut self, weights: [f32; 16]) -> Self {
        self.vessel_weights = Some(weights);
        self
    }

    /// Build the complete system prompt
    pub fn build(&self) -> String {
        let mut sections = Vec::new();

        // Identity section
        sections.push(self.identity_section());

        // Orí character — only present when vessel weights are available.
        // Agent-owned, not user-configured.
        if let Some(w) = &self.vessel_weights {
            sections.push(self.ori_character_section(w));
        }

        // Environment section
        sections.push(self.environment_section());

        // Git status (if available)
        if let Some(git) = self.git_status_section() {
            sections.push(git);
        }

        // Available tools
        if !self.available_tools.is_empty() {
            sections.push(self.tools_section());
        }

        // Hermetic principles reminder
        sections.push(self.hermetic_principles_section());

        // Custom instructions
        for instruction in &self.custom_instructions {
            sections.push(instruction.clone());
        }

        sections.join("\n\n")
    }

    fn identity_section(&self) -> String {
        // Resolve the BIPỌ̀N39 identity index into its IfáScript Odù sign so the
        // agent knows its Odù by name and vessel, not as a bare number.
        let sign = self.odu_identity.sign();
        let prescription = sign
            .prescription
            .as_deref()
            .map(|p| format!("\nPrescription: {}", p))
            .unwrap_or_default();
        format!(
            "You are {}, a sovereign agent in the Omo-Koda network.\n\
             Agent ID: {}\n\
             Tier: {} | Reputation: {:.1}\n\
             Odu: {} (#{}) — vessel of the {}{}",
            self.agent_name,
            self.agent_id,
            self.tier,
            self.reputation,
            sign.name,
            sign.index,
            sign.vessel,
            prescription,
        )
    }

    fn environment_section(&self) -> String {
        let date = current_date_str();
        let os = std::env::consts::OS;
        let cwd = self.workspace_root.display();

        format!(
            "Environment:\n\
             Date: {}\n\
             OS: {}\n\
             Working directory: {}",
            date, os, cwd
        )
    }

    fn git_status_section(&self) -> Option<String> {
        let output = std::process::Command::new("git")
            .args([
                "-C",
                self.workspace_root.to_str()?,
                "status",
                "--short",
                "--branch",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let status = String::from_utf8_lossy(&output.stdout).to_string();
        if status.trim().is_empty() {
            return None;
        }

        // Truncate to first 10 lines
        let truncated: String = status.lines().take(10).collect::<Vec<_>>().join("\n");
        Some(format!("Git status:\n```\n{}\n```", truncated))
    }

    fn tools_section(&self) -> String {
        let tool_list = self
            .available_tools
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Available tools (tier {} access):\n{}",
            self.tier, tool_list
        )
    }

    fn ori_character_section(&self, weights: &[f32; 16]) -> String {
        let reg = PersonalityRegister::derive(weights);

        // Top 3 dominant vessels by weight
        let vessel_names = [
            "Genesis", "Void", "Attention", "Loop", "Receipt", "Mask",
            "Residue", "Execution", "Swarm", "Restraint", "Migration",
            "Consent", "Vision", "Growth", "Seal", "Rhythm",
        ];
        let mut indexed: Vec<(usize, f32)> = weights.iter().cloned().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top3: String = indexed[..3]
            .iter()
            .map(|(i, w)| format!("{} ({:.2})", vessel_names[*i], w))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "Orí character (vessel-derived, not user-configured):\n  \
             Register: {}  Stance: {}  Horizon: {}  Tempo: {}\n  \
             Dominant vessels: {}",
            reg.register, reg.stance, reg.horizon, reg.tempo, top3
        )
    }

    fn hermetic_principles_section(&self) -> String {
        "Core principles: Correspondence (thought \u{2194} action alignment), \
         Cause & Effect (all acts generate receipts), \
         Rhythm (respect cooldowns and sabbath cycles). \
         Never violate workspace boundaries. \
         Private thoughts stay private."
            .to_string()
    }
}

fn current_date_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Simple date formatting without chrono dependency
    let days = secs / 86400;
    let epoch_year = 1970u64;
    // Approximate: days since epoch → date
    let year = epoch_year + days / 365;
    format!("~{} CE", year) // Approximate; precise formatting needs chrono
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::odu::OduIdentity;
    use crate::identity::AgentId;
    use std::path::PathBuf;

    fn make_builder() -> SystemPromptBuilder {
        SystemPromptBuilder::new(
            "Omo",
            AgentId::new("test-fingerprint-1234567890abcdef"),
            1,
            42.0,
            OduIdentity {
                primary_index: 3,
                mnemonic: "Ogunda speaks truth".to_string(),
            },
            PathBuf::from("/tmp"),
        )
    }

    #[test]
    fn test_build_contains_identity() {
        let prompt = make_builder().build();
        assert!(prompt.contains("Omo"));
        assert!(prompt.contains("agent-test-fingerp"));
        assert!(prompt.contains("Tier: 1"));
    }

    #[test]
    fn test_identity_shows_resolved_odu_sign() {
        // The Odu line names the resolved sign and its vessel, not a bare index.
        let prompt = make_builder().build();
        let sign = OduIdentity {
            primary_index: 3,
            mnemonic: "Ogunda speaks truth".to_string(),
        }
        .sign();
        assert!(prompt.contains(&sign.name));
        assert!(prompt.contains("vessel of the"));
        assert!(prompt.contains("(#3)"));
    }

    #[test]
    fn test_build_contains_environment() {
        let prompt = make_builder().build();
        assert!(prompt.contains("Environment:"));
        assert!(prompt.contains("Working directory:"));
    }

    #[test]
    fn test_build_with_tools() {
        let builder = make_builder().with_tools(vec!["bash".to_string(), "read".to_string()]);
        let prompt = builder.build();
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("read"));
    }

    #[test]
    fn test_build_with_custom_instructions() {
        let builder = make_builder().with_instructions(vec!["Always be helpful.".to_string()]);
        let prompt = builder.build();
        assert!(prompt.contains("Always be helpful."));
    }

    #[test]
    fn test_hermetic_principles_present() {
        let prompt = make_builder().build();
        assert!(prompt.contains("Correspondence"));
        assert!(prompt.contains("Cause & Effect"));
    }

    #[test]
    fn ori_character_section_present_when_weights_set() {
        let mut weights = [0.5f32; 16];
        weights[V_RESTRAINT] = 0.85; // high Restraint → "measured" register
        let prompt = make_builder().with_vessel_weights(weights).build();
        assert!(prompt.contains("Orí character"), "section header missing");
        assert!(prompt.contains("measured"), "register 'measured' missing");
        assert!(prompt.contains("vessel-derived"), "ownership label missing");
    }

    #[test]
    fn ori_character_absent_without_weights() {
        let prompt = make_builder().build();
        assert!(!prompt.contains("Orí character"), "section must not appear without weights");
    }
}
