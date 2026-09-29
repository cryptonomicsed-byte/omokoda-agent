pub mod agent_catalog;
pub mod agents;
pub mod background;
pub mod behavioral;
pub mod bootstrap;
pub mod bootstrap_artifact;
pub mod bridge;
pub mod bus;
pub mod compact;
pub mod config;
pub mod config_toml;
pub mod constitution;
pub mod coordination;
pub mod divination;
pub mod dream;
pub mod economics;
pub mod emotion;
pub mod error;
pub mod execution;
pub mod gates;
pub mod genesis;
pub mod goal_genesis;
pub mod habitat;
pub mod home;
pub mod identity;
pub mod ifscript_gate;
pub mod inference;
pub mod integrations;
pub mod intent;
pub mod interpreter;
pub mod ip_layer;
pub mod justice;
pub mod kernel;
pub mod lifecycle;
pub mod lsp;
pub mod main_loop;
pub mod memory;
pub mod memory_vault;
pub mod mesh;
pub mod minipae_layer;
pub mod mutation;
pub mod nostr_events;
pub mod onchain;
pub mod ori;
pub mod oso_ir;
pub mod parser;
pub mod permissions;
pub mod plugins;
pub mod policy;
pub mod prompt;
pub mod providers;
pub mod query;
pub mod receipt;
pub mod reputation;
pub mod rhythm;
#[cfg(feature = "wasm")]
pub mod sandbox;
pub mod server;
pub mod services;
pub mod session;
pub mod session_history;
pub mod seven;
pub mod signing;
pub mod skill_patch;
pub mod skills;
pub mod steward;
pub mod tasks;
pub mod toc_constants;
pub mod tools;
pub mod usage;
pub mod util;
pub mod vantage;
pub mod vault;
pub mod waggle;
pub mod workspace_seed;

pub use bootstrap_artifact::{
    archive_bootstrap_md, bootstrap_is_archived, generate_bootstrap_md, write_bootstrap_md,
};
pub use config_toml::OmokodaConfig;
pub use constitution::AgentConstitution;
pub use execution::action_compiler::{
    ActionCompiler, CadenceSpec, CompileError, CompiledAction, CompiledStep, VerifySpec,
};
pub use execution::action_interpreter::{
    ActionInterpreter, InterpretDecision, InterpretOutcome, InterpretReceipt, VerifyOutcome,
};
pub use execution::action_schema::{
    build_composed_schema, build_schema, ActionSchema, ActivationMode, BehavioralConstraint,
    ExecutionMode, OperationalStep,
};
pub use execution::action_transaction::{
    ActionOutcome, ActionReceipt, ActionState, ActionTransaction, MemoryEvent,
};
pub use execution::calabash_dispatch::{
    odu_prompt_context, vessel_description, CalabashDirective, CalabashDispatcher,
};
pub use execution::verify::{all_pass, run_assertions, Assertion, AssertionResult};
pub use gates::{ActionIntent, DataSensitivity, Reversibility};
pub use goal_genesis::{DerivedGoal, GoalGenesisEngine, GoalGenesisInput, GoalSet, GoalSource};
pub use home::HomeDir;
pub use identity::user::{IdentityError, PrivacyMode, UserIdentity};
pub use identity::AgentId;
pub use intent::{
    IntentClass, IntentCompilation, IntentCompileContext, IntentCompiler, IntentPlan,
    SubAgentSuggestion,
};
pub use interpreter::{AgentCore, AgentSnapshot, ExecutionResult, Steward};
pub use ori::{
    generate_ori_md, write_ori_projection, BirthMode, ChildBirthContext, ConsentReceipt, Ori,
    OriBirthReceipt, ParentOriCommitment,
};
pub use parser::{parse, Statement};
pub use plugins::{PluginManifest, PluginRegistry, PluginState};
pub use receipt::{Receipt, ReceiptStore};
pub use rhythm::{ActionCadence, TriggerKind};
pub use session::{EncryptedSession, IdentityVaultData, MemoryVaultData, SensitiveKey};
pub use skills::{OduModule, OduRegistry, OduSource};
pub use steward::dispatch::{DispatchError, PrimitiveDispatcher};
pub use steward::privacy::PrivacyEnforcer;
pub use workspace_seed::{seed_workspace, workspace_seed_status};

#[derive(Debug, Clone)]
pub enum Primitive {
    Birth {
        name: String,
        metadata: Vec<(String, String)>,
    },
    Think {
        prompt: String,
        private: bool,
    },
    Act {
        tool: String,
        params: String,
        sandbox: bool,
    },
}
