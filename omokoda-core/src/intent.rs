use crate::parser::ThinkModifiers;
use hkdf::Hkdf;
use omokoda_hermetic::HermeticState;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashSet;

pub const MAX_TOOL_ITERATIONS_PER_TURN: u32 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntentClass {
    SimpleQuery,
    ComplexTask,
    Creative,
    Monitoring,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanStepKind {
    Reason,
    Tool,
    DirectAct,
    SubAgent,
    Confirm,
    Respond,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanStep {
    pub kind: PlanStepKind,
    pub description: String,
    pub tool: Option<String>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentPlan {
    pub steps: Vec<PlanStep>,
    pub max_iterations: u32,
    pub priority: String,
    pub sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectActCall {
    pub tool: String,
    pub params: String,
    pub sandbox: bool,
    pub high_risk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubAgentSuggestion {
    pub purpose: String,
    pub reason: String,
    pub required_tier: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentCompilation {
    pub class: IntentClass,
    pub plan: IntentPlan,
    pub tool_sequence: Vec<String>,
    pub direct_act_calls: Vec<DirectActCall>,
    pub sub_agent_suggestion: Option<SubAgentSuggestion>,
    pub validation: ValidationReport,
    pub private: bool,
    pub router_fingerprint: String,
    pub hermetic_depth: f64,
}

pub struct IntentCompileContext<'a> {
    pub private: bool,
    pub tier: u8,
    pub reputation: f64,
    pub odu_seed: &'a [u8; 32],
    pub hermetic: &'a HermeticState,
    pub available_tools: &'a [String],
}

impl<'a> IntentCompileContext<'a> {
    pub fn to_exec_context(
        &self,
        agent_id: crate::identity::AgentId,
        name: String,
        default_sandbox: bool,
    ) -> crate::tools::ExecutionContext {
        crate::tools::ExecutionContext {
            agent_id,
            name,
            tier: self.tier,
            reputation: self.reputation,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0, // Not strictly used for boundary checks
                mnemonic: String::new(),
            },
            workspace_root: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".")),
            sandbox_mode: default_sandbox,
        }
    }
}

pub struct IntentCompiler;

impl IntentCompiler {
    pub fn compile(
        prompt: &str,
        modifiers: &ThinkModifiers,
        context: IntentCompileContext<'_>,
    ) -> IntentCompilation {
        let neural_params = derive_neural_params(context.odu_seed);
        let class = classify_intent(prompt, modifiers, &neural_params);
        let available: HashSet<&str> = context.available_tools.iter().map(String::as_str).collect();
        let mut validation = ValidationReport {
            allowed: true,
            requires_confirmation: false,
            reasons: Vec::new(),
            warnings: Vec::new(),
        };

        let tier_iteration_cap = iteration_cap_for_tier(context.tier);
        let requested_iterations = modifiers
            .max_iterations
            .unwrap_or_else(|| default_iterations_for(&class, modifiers));
        let max_iterations = requested_iterations
            .min(tier_iteration_cap)
            .min(MAX_TOOL_ITERATIONS_PER_TURN);
        if requested_iterations > tier_iteration_cap {
            validation.warnings.push(format!(
                "max_iterations clamped from {requested_iterations} to {max_iterations} for tier {tier}",
                tier = context.tier
            ));
        }

        if contains_ethics_block(prompt) {
            validation.allowed = false;
            validation
                .reasons
                .push("Hermetic ethics blocked harmful or secret-exfiltration intent".to_string());
        }

        if modifiers.priority.as_deref() == Some("high") && context.reputation < 1.0 {
            validation.warnings.push(
                "high priority accepted as a scheduling hint only at the current reputation"
                    .to_string(),
            );
        }

        let sandbox = modifiers.sandbox;
        let priority = modifiers
            .priority
            .clone()
            .unwrap_or_else(|| "normal".to_string());
        let mut steps = Vec::new();
        let mut tool_sequence = Vec::new();
        let mut direct_act_calls = Vec::new();
        let mut sub_agent_suggestion = None;

        match class {
            IntentClass::SimpleQuery => {
                if let Some(call) = compile_simple_direct_act(
                    prompt,
                    sandbox,
                    context.private,
                    &available,
                    &mut validation,
                ) {
                    tool_sequence.push(call.tool.clone());
                    steps.push(PlanStep {
                        kind: PlanStepKind::DirectAct,
                        description: format!("Execute {} through Steward act path", call.tool),
                        tool: Some(call.tool.clone()),
                        requires_confirmation: call.high_risk,
                    });
                    direct_act_calls.push(call);
                } else {
                    steps.push(PlanStep {
                        kind: PlanStepKind::Respond,
                        description: "Answer directly with the configured reasoning provider"
                            .to_string(),
                        tool: None,
                        requires_confirmation: false,
                    });
                }
            }
            IntentClass::Creative => {
                steps.push(PlanStep {
                    kind: PlanStepKind::Reason,
                    description: "Extract creative constraints, tone, and deliverable shape"
                        .to_string(),
                    tool: None,
                    requires_confirmation: false,
                });
                steps.push(PlanStep {
                    kind: PlanStepKind::Respond,
                    description: "Generate the creative output without leaving the think primitive"
                        .to_string(),
                    tool: None,
                    requires_confirmation: false,
                });
            }
            IntentClass::ComplexTask => {
                steps.push(PlanStep {
                    kind: PlanStepKind::Reason,
                    description: "Decompose the goal into executable checkpoints".to_string(),
                    tool: None,
                    requires_confirmation: false,
                });
                if available.contains("grep") {
                    tool_sequence.push("grep".to_string());
                    steps.push(PlanStep {
                        kind: PlanStepKind::Tool,
                        description: "Search relevant workspace context if the goal references local code or files".to_string(),
                        tool: Some("grep".to_string()),
                        requires_confirmation: false,
                    });
                }
                if available.contains("read_file") {
                    tool_sequence.push("read_file".to_string());
                    steps.push(PlanStep {
                        kind: PlanStepKind::Tool,
                        description: "Read targeted files through tier-gated tools".to_string(),
                        tool: Some("read_file".to_string()),
                        requires_confirmation: false,
                    });
                }
                steps.push(PlanStep {
                    kind: PlanStepKind::Respond,
                    description:
                        "Return a compiled plan, execution result, or ask a narrow follow-up"
                            .to_string(),
                    tool: None,
                    requires_confirmation: false,
                });
            }
            IntentClass::Monitoring => {
                steps.push(PlanStep {
                    kind: PlanStepKind::Reason,
                    description: "Define monitored signal, risk threshold, and escalation path"
                        .to_string(),
                    tool: None,
                    requires_confirmation: false,
                });
                validation.requires_confirmation = true;
                validation.reasons.push(
                    "monitoring or funds movement requires explicit confirmation before action"
                        .to_string(),
                );
                steps.push(PlanStep {
                    kind: PlanStepKind::Confirm,
                    description:
                        "Ask for confirmation before any high-risk transfer or fund-security action"
                            .to_string(),
                    tool: None,
                    requires_confirmation: true,
                });
                let suggestion = SubAgentSuggestion {
                    purpose: "wallet-risk-monitor".to_string(),
                    reason: "Long-running monitoring should be isolated as a born sub-agent once authorized".to_string(),
                    required_tier: 4,
                };
                if context.tier < suggestion.required_tier {
                    validation.allowed = false;
                    validation.reasons.push(format!(
                        "sub-agent orchestration requires tier {}, current tier is {}",
                        suggestion.required_tier, context.tier
                    ));
                }
                steps.push(PlanStep {
                    kind: PlanStepKind::SubAgent,
                    description: suggestion.reason.clone(),
                    tool: Some("agent_orchestration".to_string()),
                    requires_confirmation: true,
                });
                tool_sequence.push("agent_orchestration".to_string());
                sub_agent_suggestion = Some(suggestion);
            }
        }

        IntentCompilation {
            class,
            plan: IntentPlan {
                steps,
                max_iterations,
                priority,
                sandbox,
            },
            tool_sequence,
            direct_act_calls,
            sub_agent_suggestion,
            validation,
            private: context.private,
            router_fingerprint: router_fingerprint(&neural_params),
            hermetic_depth: context.hermetic.think_abstraction_depth(),
        }
    }
}

fn derive_neural_params(seed: &[u8; 32]) -> [u8; 86] {
    let hk = Hkdf::<Sha256>::new(None, seed);
    let mut params = [0u8; 86];
    hk.expand(b"omokoda-neural-router-v1", &mut params)
        .expect("HKDF expansion failed");
    params
}

fn router_fingerprint(params: &[u8; 86]) -> String {
    blake3::hash(params).to_hex().to_string()
}

fn classify_intent(
    prompt: &str,
    modifiers: &ThinkModifiers,
    neural_params: &[u8; 86],
) -> IntentClass {
    let lower = prompt.to_lowercase();
    if contains_any(
        &lower,
        &[
            "monitor",
            "watch",
            "alert",
            "large transfer",
            "wallet risk",
            "auto-secure",
        ],
    ) {
        return IntentClass::Monitoring;
    }
    if contains_any(
        &lower,
        &[
            "write", "compose", "design", "imagine", "story", "poem", "creative",
        ],
    ) {
        return IntentClass::Creative;
    }
    let complexity_bias = neural_params[0] as usize % 24;
    if modifiers.loop_enabled
        || prompt.contains('\n')
        || prompt.len() > 80 + complexity_bias
        || contains_any(
            &lower,
            &[
                "plan",
                "build",
                "implement",
                "analyze",
                "compare",
                "automate",
                "refactor",
            ],
        )
    {
        return IntentClass::ComplexTask;
    }
    IntentClass::SimpleQuery
}

fn compile_simple_direct_act(
    prompt: &str,
    sandbox: bool,
    private: bool,
    available: &HashSet<&str>,
    validation: &mut ValidationReport,
) -> Option<DirectActCall> {
    let lower = prompt.to_lowercase();
    if contains_any(&lower, &["search web for ", "web search ", "look up "]) {
        if private {
            validation.warnings.push(
                "private mode blocks external-capable web_search; using direct reasoning instead"
                    .to_string(),
            );
            return None;
        }
        if available.contains("web_search") {
            return Some(DirectActCall {
                tool: "web_search".to_string(),
                params: strip_search_prefix(prompt),
                sandbox,
                high_risk: false,
            });
        }
    }

    if let Some(path) = extract_after_any(&lower, prompt, &["read file ", "open file "]) {
        if available.contains("read_file") {
            return Some(DirectActCall {
                tool: "read_file".to_string(),
                params: path,
                sandbox,
                high_risk: false,
            });
        }
    }

    None
}

fn strip_search_prefix(prompt: &str) -> String {
    let lower = prompt.to_lowercase();
    extract_after_any(
        &lower,
        prompt,
        &["search web for ", "web search ", "look up "],
    )
    .unwrap_or_else(|| prompt.to_string())
}

fn extract_after_any(lower: &str, original: &str, prefixes: &[&str]) -> Option<String> {
    for prefix in prefixes {
        if let Some(pos) = lower.find(prefix) {
            let start = pos + prefix.len();
            let value = original[start..]
                .trim()
                .trim_matches('"')
                .trim_matches('`')
                .to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn contains_ethics_block(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    contains_any(
        &lower,
        &[
            "steal",
            "drain wallet",
            "exfiltrate",
            "bypass safety",
            "seed phrase",
            "private key",
        ],
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn default_iterations_for(class: &IntentClass, modifiers: &ThinkModifiers) -> u32 {
    if modifiers.loop_enabled {
        return 3;
    }
    match class {
        IntentClass::SimpleQuery => 1,
        IntentClass::Creative => 2,
        IntentClass::ComplexTask => 4,
        IntentClass::Monitoring => 4,
    }
}

/// Public wrapper for the agentic path's tier-based iteration cap.
pub fn tier_iteration_cap(tier: u8) -> u32 {
    iteration_cap_for_tier(tier)
}

fn iteration_cap_for_tier(tier: u8) -> u32 {
    match tier {
        0 => 3,
        1 => 5,
        2 => 8,
        3 => 12,
        _ => MAX_TOOL_ITERATIONS_PER_TURN,
    }
}
