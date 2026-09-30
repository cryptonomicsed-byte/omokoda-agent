//! Ọ̀ṢỌ́ Agent SDK — high-level Rust client for the sovereign job/contract API.
//!
//! Wraps UCX protocol + oso-compiler into ergonomic builder pattern:
//!
//! ```rust,ignore
//! let sdk = OsoSdk::new("did:v:agent:abc");
//! let job = sdk.jobs().create()
//!     .workload(WorkloadType::Inference)
//!     .runtime_spec(json!({"model": "llama3"}))
//!     .submit()?;
//! let receipt = sdk.jobs().wait_for_proof(&job.id)?;
//! ```

pub mod agent;
pub mod contract;
pub mod contracts;
pub mod error;
pub mod job;

pub use agent::{AgentClient, AgentSdk};
pub use contract::{ContractClient, NativeContract};
pub use error::{SdkError, SdkResult};
pub use job::{JobBuilder, JobClient, PendingJob};

/// Root SDK handle — holds identity + config.
pub struct OsoSdk {
    pub agent_id: String,
    pub principal_id: String,
}

impl OsoSdk {
    pub fn new(agent_id: impl Into<String>, principal_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            principal_id: principal_id.into(),
        }
    }

    pub fn jobs(&self) -> JobClient {
        JobClient::new(self.agent_id.clone())
    }

    pub fn contracts(&self) -> ContractClient {
        ContractClient::new(self.agent_id.clone())
    }

    pub fn agent(&self) -> AgentClient {
        AgentClient::new(self.agent_id.clone(), self.principal_id.clone())
    }
}
