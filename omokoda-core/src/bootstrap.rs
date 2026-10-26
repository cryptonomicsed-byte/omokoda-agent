use crate::lifecycle::soma_lifecycle::SomaLifecycle;
use serde::{Deserialize, Serialize};

/// Phases of agent initialization, in order
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BirthPhase {
    Identity = 0,
    Memory = 1,
    Tools = 2,
    Hooks = 3,
    Providers = 4,
    Skills = 5,
    Plugins = 6,
    Ready = 7,
}

impl BirthPhase {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Memory => "memory",
            Self::Tools => "tools",
            Self::Hooks => "hooks",
            Self::Providers => "providers",
            Self::Skills => "skills",
            Self::Plugins => "plugins",
            Self::Ready => "ready",
        }
    }

    pub fn all() -> &'static [BirthPhase] {
        &[
            Self::Identity,
            Self::Memory,
            Self::Tools,
            Self::Hooks,
            Self::Providers,
            Self::Skills,
            Self::Plugins,
            Self::Ready,
        ]
    }
}

/// Result of a single bootstrap phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: BirthPhase,
    pub success: bool,
    pub message: String,
    pub duration_ms: u64,
}

/// The full bootstrap graph — sequence of phase results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootstrapGraph {
    pub phases: Vec<PhaseResult>,
    pub current_phase: Option<BirthPhase>,
    pub completed: bool,
}

impl BootstrapGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_phase(
        &mut self,
        phase: BirthPhase,
        success: bool,
        message: impl Into<String>,
        duration_ms: u64,
    ) {
        self.current_phase = Some(phase.clone());
        self.phases.push(PhaseResult {
            phase,
            success,
            message: message.into(),
            duration_ms,
        });
    }

    pub fn mark_ready(&mut self) {
        self.completed = true;
        self.current_phase = Some(BirthPhase::Ready);
    }

    /// Run SOMA wake_up as part of the bootstrap sequence (after Identity phase).
    /// `agent_id` is set on the habitat address (W-22: sensor → HabitatAddress wire).
    /// Returns the initialized SomaLifecycle to be held by the agent runtime.
    pub fn soma_wake_up(&mut self) -> SomaLifecycle {
        let mut soma = SomaLifecycle::wake_up();
        self.record_phase(
            BirthPhase::Memory,
            true,
            format!(
                "SOMA awake: energy={:.2} tension={:.2} area={:?}",
                soma.emotion.energy, soma.emotion.tension, soma.habitat.area_id,
            ),
            0,
        );
        soma
    }

    /// Same as `soma_wake_up` but binds the agent_id to the habitat address immediately.
    pub fn soma_wake_up_with_id(&mut self, agent_id: &str) -> SomaLifecycle {
        let mut soma = self.soma_wake_up();
        soma.habitat.set_agent_id(agent_id);
        soma
    }

    pub fn has_failed(&self) -> bool {
        self.phases.iter().any(|p| !p.success)
    }

    pub fn summary(&self) -> String {
        let passed = self.phases.iter().filter(|p| p.success).count();
        let total = self.phases.len();
        let failed: Vec<&str> = self
            .phases
            .iter()
            .filter(|p| !p.success)
            .map(|p| p.phase.name())
            .collect();
        if failed.is_empty() {
            format!("Bootstrap complete: {}/{} phases passed", passed, total)
        } else {
            format!(
                "Bootstrap partial: {}/{} phases passed, failed: {}",
                passed,
                total,
                failed.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_birth_phases_ordered() {
        let phases = BirthPhase::all();
        assert_eq!(phases[0], BirthPhase::Identity);
        assert_eq!(phases[phases.len() - 1], BirthPhase::Ready);
        for window in phases.windows(2) {
            assert!(window[0] < window[1]);
        }
    }

    #[test]
    fn test_bootstrap_graph() {
        let mut graph = BootstrapGraph::new();
        graph.record_phase(BirthPhase::Identity, true, "Identity created", 5);
        graph.record_phase(BirthPhase::Memory, true, "Memory initialized", 3);
        assert!(!graph.has_failed());
        assert!(!graph.completed);

        graph.mark_ready();
        assert!(graph.completed);
        assert_eq!(graph.current_phase, Some(BirthPhase::Ready));
    }

    #[test]
    fn test_bootstrap_summary_with_failure() {
        let mut graph = BootstrapGraph::new();
        graph.record_phase(BirthPhase::Identity, true, "ok", 1);
        graph.record_phase(BirthPhase::Providers, false, "no provider found", 100);
        assert!(graph.has_failed());
        assert!(graph.summary().contains("failed: providers"));
    }
}
