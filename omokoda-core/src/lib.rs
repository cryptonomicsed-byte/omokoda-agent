pub mod agent_catalog;
pub mod genesis;
pub mod goal_genesis;
pub mod bridge;
pub mod inference;
pub mod util;
pub mod kernel;
pub mod agents;
pub mod background;
pub mod behavioral;
pub mod bootstrap;
pub mod bus;
pub mod compact;
pub mod config;
pub mod coordination;
pub mod divination;
pub mod dream;
pub mod economics;
pub mod emotion;
pub mod error;
pub mod execution;
pub mod gates;
pub mod habitat;
pub mod identity;
pub mod intent;
pub mod interpreter;
pub mod justice;
pub mod lifecycle;
pub mod lsp;
pub mod ip_layer;
pub mod minipae_layer;
pub mod nostr_events;
pub mod oso_ir;
pub mod main_loop;
pub mod memory;
pub mod memory_vault;
pub mod mesh;
pub mod onchain;
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
pub mod session;
pub mod session_history;
pub mod constitution;
pub mod skill_patch;
pub mod skills;
pub mod seven;
pub mod steward;
pub mod tasks;
pub mod tools;
pub mod usage;
pub mod services;
pub mod ifscript_gate;
pub mod mutation;
pub mod vantage;
pub mod vault;
pub mod waggle;
pub mod integrations;
pub mod signing;
pub mod toc_constants;
pub mod ori;
pub mod home;
pub mod config_toml;
pub mod workspace_seed;
pub mod bootstrap_artifact;

pub use ori::{
    Ori, OriBirthReceipt, BirthMode, ParentOriCommitment, ConsentReceipt, ChildBirthContext,
    generate_ori_md, write_ori_projection,
};
pub use home::HomeDir;
pub use config_toml::OmokodaConfig;
pub use workspace_seed::{seed_workspace, workspace_seed_status};
pub use bootstrap_artifact::{
    generate_bootstrap_md, write_bootstrap_md, archive_bootstrap_md, bootstrap_is_archived,
};
pub use identity::user::{IdentityError, PrivacyMode, UserIdentity};
pub use identity::AgentId;
pub use intent::{
    IntentClass, IntentCompilation, IntentCompileContext, IntentCompiler, IntentPlan,
    SubAgentSuggestion,
};
pub use interpreter::{AgentCore, AgentSnapshot, ExecutionResult, Steward};
pub use parser::{parse, Statement};
pub use plugins::{PluginManifest, PluginRegistry, PluginState};
pub use receipt::{Receipt, ReceiptStore};
pub use constitution::AgentConstitution;
pub use session::{EncryptedSession, IdentityVaultData, MemoryVaultData, SensitiveKey};
pub use skills::{OduModule, OduRegistry, OduSource};
pub use steward::dispatch::{DispatchError, PrimitiveDispatcher};
pub use steward::privacy::PrivacyEnforcer;
pub use gates::{ActionIntent, DataSensitivity, Reversibility};
pub use execution::action_transaction::{ActionTransaction, ActionReceipt, ActionOutcome, ActionState, MemoryEvent};
pub use execution::action_compiler::{ActionCompiler, CompiledAction, CompiledStep, VerifySpec, CadenceSpec, CompileError};
pub use execution::verify::{Assertion, AssertionResult, run_assertions, all_pass};
pub use rhythm::{ActionCadence, TriggerKind};

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
