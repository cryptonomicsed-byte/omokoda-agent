use crate::lifecycle::soma_lifecycle::{SessionSummary, SomaLifecycle};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio::sync::mpsc;

use crate::query::{QueryConfig, QueryEngine, QueryState};
use crate::usage::TokenUsage;

/// REPL slash-command interceptor: routes /commands before think dispatch.
/// Covers identity (Odu), economy (Synapse), reputation tiers, and session utilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplCommand {
    /// Show the agent's Odu identity (mnemonic + DNA fingerprint)
    Odu,
    /// Show the Synapse economy state (balance, escrow, burn rate)
    Synapse,
    /// Show the current reputation tier and tool access gates
    Tier,
    /// Show the receipt log (last N receipts)
    Receipts { limit: usize },
    /// Force context compaction now
    Compact,
    /// Show the active permission policy
    Permissions,
    /// List all tools available at the current tier
    Tools,
    /// Show help — list available slash commands
    Help,
}

impl ReplCommand {
    pub fn help_text() -> &'static str {
        "\
/odu         — Show Odu identity (mnemonic + DNA fingerprint)
/synapse     — Show Synapse economy (balance, escrow, burn rate)
/tier        — Show reputation tier and tool gates
/receipts    — Show receipt log (last 10); /receipts N for last N
/compact     — Force context compaction
/permissions — Show active permission policy
/tools       — List tools available at current tier
/help        — Show this help message"
    }
}

/// Try to parse a raw REPL input string as a slash command.
/// Returns `Some(ReplCommand)` if the input starts with `/`, `None` otherwise.
/// Unrecognised `/commands` return `None` so the string falls through to the LLM.
pub fn intercept_slash_command(input: &str) -> Option<ReplCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").to_lowercase();
    let arg = parts.next().unwrap_or("").trim();

    match cmd.as_str() {
        "/odu" => Some(ReplCommand::Odu),
        "/synapse" => Some(ReplCommand::Synapse),
        "/tier" => Some(ReplCommand::Tier),
        "/receipts" => {
            let limit = arg.parse::<usize>().unwrap_or(10);
            Some(ReplCommand::Receipts { limit })
        }
        "/compact" => Some(ReplCommand::Compact),
        "/permissions" => Some(ReplCommand::Permissions),
        "/tools" => Some(ReplCommand::Tools),
        "/help" => Some(ReplCommand::Help),
        _ => None,
    }
}

/// Events that drive the main loop — inputs from user, tools, timers, or the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopEvent {
    /// User submitted a new prompt
    UserInput { prompt: String, private: bool },
    /// A tool returned output (from an agentic inner loop)
    ToolResult { tool: String, output: String },
    /// A background task completed
    BackgroundDone { task_id: String, output: String },
    /// Switch the active provider/model
    SwitchModel { provider: String },
    /// Trigger context compaction
    Compact,
    /// Pause the loop (e.g. awaiting confirmation)
    Pause,
    /// Resume after a pause
    Resume,
    /// Shutdown the loop
    Shutdown,
}

/// Observable state of the main loop
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopState {
    /// Waiting for user input
    Idle,
    /// Executing an LLM think turn
    Thinking,
    /// Executing a tool act
    Acting { tool: String },
    /// Compacting the context window
    Compacting,
    /// Switching provider/model
    SwitchingModel,
    /// Paused, awaiting user confirmation
    Paused,
    /// Loop has stopped
    Stopped,
}

impl LoopState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Thinking => "thinking",
            Self::Acting { .. } => "acting",
            Self::Compacting => "compacting",
            Self::SwitchingModel => "switching_model",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
        }
    }

    pub fn is_terminal(&self) -> bool {
        *self == Self::Stopped
    }

    pub fn can_accept_input(&self) -> bool {
        matches!(self, Self::Idle | Self::Paused)
    }
}

/// Configuration for the main loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainLoopConfig {
    pub queue_capacity: usize,
    pub query: QueryConfig,
    pub auto_compact_threshold: usize,
    /// Automatically switch to a cheaper model after N turns
    pub model_switch_after_turns: Option<u32>,
    pub fallback_provider: Option<String>,
}

impl Default for MainLoopConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 64,
            query: QueryConfig::default(),
            auto_compact_threshold: 50,
            model_switch_after_turns: None,
            fallback_provider: None,
        }
    }
}

/// Outcome of processing one event
#[derive(Debug, Clone)]
pub enum TurnOutcome {
    /// Loop advanced normally; new state and optional output
    Continue {
        new_state: LoopState,
        output: Option<String>,
        usage: TokenUsage,
    },
    /// Loop is switching models; processing continues after switch
    ModelSwitch { to_provider: String },
    /// Loop should compact before continuing
    CompactNeeded,
    /// Loop stopped
    Stopped,
}

/// The main event-driven loop — queue processor + state machine.
/// Mirrors `useMainLoopModel` / `queueProcessor`.
pub struct MainLoop {
    pub state: LoopState,
    pub config: MainLoopConfig,
    queue: VecDeque<LoopEvent>,
    pub query_engine: QueryEngine,
    pub query_state: QueryState,
    pub active_provider: String,
    pub turn_history: Vec<(LoopEvent, TurnOutcome)>,
    /// SOMA lifecycle — emotion + sensor state, updated each user message
    pub soma: Option<SomaLifecycle>,
}

impl MainLoop {
    pub fn new(
        config: MainLoopConfig,
        initial_provider: impl Into<String>,
        initial_synapse: f64,
    ) -> Self {
        let query_engine = QueryEngine::new(config.query.clone());
        let query_state = QueryState::new(&config.query, initial_synapse);
        Self {
            state: LoopState::Idle,
            queue: VecDeque::with_capacity(config.queue_capacity),
            active_provider: initial_provider.into(),
            turn_history: Vec::new(),
            query_engine,
            query_state,
            config,
            soma: None,
        }
    }

    /// Attach a SOMA lifecycle to this loop (called from bootstrap after wake_up).
    pub fn attach_soma(&mut self, soma: SomaLifecycle) {
        self.soma = Some(soma);
    }

    /// Graceful shutdown: run SOMA sleep with a session summary, then stop.
    pub fn shutdown_with_soma(&mut self, key_topics: Vec<String>) {
        if let Some(soma) = &self.soma {
            let summary = SessionSummary {
                message_count: soma.message_count,
                key_topics,
                notable_events: vec![],
            };
            soma.sleep(&summary);
        }
        self.state = LoopState::Stopped;
    }

    pub fn with_defaults(provider: impl Into<String>, initial_synapse: f64) -> Self {
        Self::new(MainLoopConfig::default(), provider, initial_synapse)
    }

    /// Enqueue an event for processing
    pub fn push(&mut self, event: LoopEvent) -> Result<(), String> {
        if self.state.is_terminal() {
            return Err("Loop is stopped".to_string());
        }
        if self.queue.len() >= self.config.queue_capacity {
            return Err("Event queue full".to_string());
        }
        self.queue.push_back(event);
        Ok(())
    }

    /// Drain up to `max` queued events, returning their outcomes
    pub fn drain(&mut self, max: usize) -> Vec<TurnOutcome> {
        let mut outcomes = Vec::new();
        let n = max.min(self.queue.len());
        for _ in 0..n {
            if let Some(event) = self.queue.pop_front() {
                let outcome = self.process(event);
                if matches!(outcome, TurnOutcome::Stopped) {
                    self.state = LoopState::Stopped;
                    outcomes.push(outcome);
                    break;
                }
                outcomes.push(outcome);
            }
        }
        outcomes
    }

    /// Process a single event, transitioning state and returning the outcome
    pub fn process(&mut self, event: LoopEvent) -> TurnOutcome {
        if self.state.is_terminal() {
            return TurnOutcome::Stopped;
        }

        // Auto-compact check
        if self.query_state.turn_count > 0
            && (self.query_state.turn_count as usize)
                .is_multiple_of(self.config.auto_compact_threshold)
            && !matches!(self.state, LoopState::Compacting)
        {
            self.state = LoopState::Compacting;
            return TurnOutcome::CompactNeeded;
        }

        // Auto model-switch
        if let Some(after) = self.config.model_switch_after_turns {
            if self.query_state.turn_count > 0 && self.query_state.turn_count.is_multiple_of(after)
            {
                if let Some(fallback) = self.config.fallback_provider.clone() {
                    if fallback != self.active_provider {
                        let to = fallback.clone();
                        self.active_provider = fallback;
                        self.state = LoopState::SwitchingModel;
                        return TurnOutcome::ModelSwitch { to_provider: to };
                    }
                }
            }
        }

        match &event {
            LoopEvent::UserInput { prompt, private: _ } => {
                if !self.state.can_accept_input() {
                    self.queue.push_front(event.clone());
                    return TurnOutcome::Continue {
                        new_state: self.state.clone(),
                        output: Some("queued: loop is busy".to_string()),
                        usage: TokenUsage::default(),
                    };
                }
                // SOMA pulse — update emotion from message content
                if let Some(soma) = &mut self.soma {
                    soma.pulse(prompt, "user");
                }
                self.state = LoopState::Thinking;
                let usage = TokenUsage::default();
                self.query_engine
                    .advance_turn(&mut self.query_state, &usage, 1000.0);
                let outcome = TurnOutcome::Continue {
                    new_state: LoopState::Idle,
                    output: Some(format!(
                        "[turn {}] thinking...",
                        self.query_state.turn_count
                    )),
                    usage,
                };
                self.state = LoopState::Idle;
                self.turn_history.push((event, outcome.clone()));
                outcome
            }

            LoopEvent::ToolResult { tool, output } => {
                let outcome = TurnOutcome::Continue {
                    new_state: LoopState::Idle,
                    output: Some(format!("[tool:{}] {}", tool, output)),
                    usage: TokenUsage::default(),
                };
                self.state = LoopState::Idle;
                self.turn_history.push((event, outcome.clone()));
                outcome
            }

            LoopEvent::BackgroundDone { task_id, output } => {
                let outcome = TurnOutcome::Continue {
                    new_state: self.state.clone(),
                    output: Some(format!("[bg:{}] {}", task_id, output)),
                    usage: TokenUsage::default(),
                };
                self.turn_history.push((event, outcome.clone()));
                outcome
            }

            LoopEvent::SwitchModel { provider } => {
                let to = provider.clone();
                self.active_provider = provider.clone();
                self.state = LoopState::Idle;
                let outcome = TurnOutcome::ModelSwitch { to_provider: to };
                self.turn_history.push((event, outcome.clone()));
                outcome
            }

            LoopEvent::Compact => {
                self.state = LoopState::Compacting;
                let outcome = TurnOutcome::CompactNeeded;
                self.state = LoopState::Idle;
                self.turn_history.push((event, outcome.clone()));
                outcome
            }

            LoopEvent::Pause => {
                self.state = LoopState::Paused;
                TurnOutcome::Continue {
                    new_state: LoopState::Paused,
                    output: None,
                    usage: TokenUsage::default(),
                }
            }

            LoopEvent::Resume => {
                if self.state == LoopState::Paused {
                    self.state = LoopState::Idle;
                }
                TurnOutcome::Continue {
                    new_state: self.state.clone(),
                    output: None,
                    usage: TokenUsage::default(),
                }
            }

            LoopEvent::Shutdown => {
                // SOMA sleep — consolidate session on graceful shutdown
                if let Some(soma) = &self.soma {
                    let summary = SessionSummary {
                        message_count: soma.message_count,
                        key_topics: vec![],
                        notable_events: vec!["shutdown".to_string()],
                    };
                    soma.sleep(&summary);
                }
                self.state = LoopState::Stopped;
                TurnOutcome::Stopped
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.state.is_terminal()
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn turn_count(&self) -> u32 {
        self.query_state.turn_count
    }
}

/// Channel-based interface to drive the loop from async code.
/// Spawn this with `tokio::spawn` to run the loop in the background.
pub struct LoopHandle {
    pub sender: mpsc::Sender<LoopEvent>,
}

impl LoopHandle {
    pub fn spawn(
        provider: impl Into<String>,
        initial_synapse: f64,
    ) -> (Self, mpsc::Receiver<TurnOutcome>) {
        let (event_tx, mut event_rx) = mpsc::channel::<LoopEvent>(64);
        let (outcome_tx, outcome_rx) = mpsc::channel::<TurnOutcome>(64);
        let mut loop_ = MainLoop::with_defaults(provider, initial_synapse);

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let outcome = loop_.process(event);
                let stopped = matches!(outcome, TurnOutcome::Stopped);
                let _ = outcome_tx.send(outcome).await;
                if stopped {
                    break;
                }
            }
        });

        (Self { sender: event_tx }, outcome_rx)
    }

    pub async fn send(&self, event: LoopEvent) -> Result<(), String> {
        self.sender
            .send(event)
            .await
            .map_err(|_| "loop has stopped".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_loop() -> MainLoop {
        MainLoop::with_defaults("mock", 100_000.0)
    }

    #[test]
    fn test_loop_starts_idle() {
        let loop_ = make_loop();
        assert_eq!(loop_.state, LoopState::Idle);
    }

    #[test]
    fn test_user_input_advances_turn() {
        let mut loop_ = make_loop();
        loop_
            .push(LoopEvent::UserInput {
                prompt: "hello".to_string(),
                private: false,
            })
            .unwrap();
        let outcomes = loop_.drain(1);
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0], TurnOutcome::Continue { .. }));
        assert_eq!(loop_.turn_count(), 1);
    }

    #[test]
    fn test_shutdown_stops_loop() {
        let mut loop_ = make_loop();
        loop_.push(LoopEvent::Shutdown).unwrap();
        let outcomes = loop_.drain(1);
        assert!(matches!(outcomes[0], TurnOutcome::Stopped));
        assert!(loop_.is_stopped());
    }

    #[test]
    fn test_model_switch_event() {
        let mut loop_ = make_loop();
        loop_
            .push(LoopEvent::SwitchModel {
                provider: "ollama".to_string(),
            })
            .unwrap();
        loop_.drain(1);
        assert_eq!(loop_.active_provider, "ollama");
    }

    #[test]
    fn test_pause_and_resume() {
        let mut loop_ = make_loop();
        loop_.push(LoopEvent::Pause).unwrap();
        loop_.drain(1);
        assert_eq!(loop_.state, LoopState::Paused);

        loop_.push(LoopEvent::Resume).unwrap();
        loop_.drain(1);
        assert_eq!(loop_.state, LoopState::Idle);
    }

    #[test]
    fn test_queue_overflow_rejected() {
        let config = MainLoopConfig {
            queue_capacity: 2,
            ..Default::default()
        };
        let mut loop_ = MainLoop::new(config, "mock", 100_000.0);
        assert!(loop_.push(LoopEvent::Compact).is_ok());
        assert!(loop_.push(LoopEvent::Compact).is_ok());
        assert!(loop_.push(LoopEvent::Compact).is_err());
    }

    // --- Slash command intercept tests ---

    #[test]
    fn slash_odu_parses() {
        assert_eq!(intercept_slash_command("/odu"), Some(ReplCommand::Odu));
    }

    #[test]
    fn slash_synapse_parses() {
        assert_eq!(
            intercept_slash_command("/synapse"),
            Some(ReplCommand::Synapse)
        );
    }

    #[test]
    fn slash_tier_parses() {
        assert_eq!(intercept_slash_command("/tier"), Some(ReplCommand::Tier));
    }

    #[test]
    fn slash_receipts_default_limit() {
        assert_eq!(
            intercept_slash_command("/receipts"),
            Some(ReplCommand::Receipts { limit: 10 })
        );
    }

    #[test]
    fn slash_receipts_custom_limit() {
        assert_eq!(
            intercept_slash_command("/receipts 25"),
            Some(ReplCommand::Receipts { limit: 25 })
        );
    }

    #[test]
    fn slash_compact_parses() {
        assert_eq!(
            intercept_slash_command("/compact"),
            Some(ReplCommand::Compact)
        );
    }

    #[test]
    fn slash_permissions_parses() {
        assert_eq!(
            intercept_slash_command("/permissions"),
            Some(ReplCommand::Permissions)
        );
    }

    #[test]
    fn slash_tools_parses() {
        assert_eq!(intercept_slash_command("/tools"), Some(ReplCommand::Tools));
    }

    #[test]
    fn slash_help_parses() {
        assert_eq!(intercept_slash_command("/help"), Some(ReplCommand::Help));
    }

    #[test]
    fn non_slash_input_passes_through() {
        assert_eq!(intercept_slash_command("hello world"), None);
        assert_eq!(intercept_slash_command("think about this"), None);
    }

    #[test]
    fn unknown_slash_command_passes_through() {
        assert_eq!(intercept_slash_command("/doesnotexist"), None);
    }

    #[test]
    fn slash_command_trims_whitespace() {
        assert_eq!(intercept_slash_command("  /odu  "), Some(ReplCommand::Odu));
    }

    #[test]
    fn help_text_contains_all_commands() {
        let help = ReplCommand::help_text();
        for cmd in &[
            "/odu",
            "/synapse",
            "/tier",
            "/receipts",
            "/compact",
            "/permissions",
            "/tools",
            "/help",
        ] {
            assert!(help.contains(cmd), "help text missing {}", cmd);
        }
    }
}
