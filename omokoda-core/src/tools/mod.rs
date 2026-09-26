use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::execution::permission_enforcer::{enforce_mode, validate_path_boundary};
#[cfg(feature = "wasm")]
use crate::sandbox::WasmSandbox;

pub mod execution_log;
pub mod config_tool;
pub mod file_ops;
pub mod if_script_tool;
pub mod mesh_tools;
pub mod nostr_identity_tool;
pub mod onchain_tools;
pub mod python_bridge;
pub mod repl;
pub mod retry;
pub mod skillforge;
pub mod skillforge_bus;
pub mod ytforge;
pub mod skills;
pub mod sovereign;
pub mod streaming;
pub mod structured_output;
pub mod todo;
pub mod tool_definitions;
pub mod tor_tool;
pub mod osovm_tool;
pub mod sovereign_node;
pub mod twin_binding_tool;
pub mod validation;
pub mod walrus_tool;
pub mod wallet_tools;
pub mod zero_tool;
pub mod ucx_tool;
pub mod provider_tools;
pub mod ucx_receipt;
pub mod ucx_policy;
pub mod omohome_tool;
pub mod mail_tool;
pub mod mailbox_provisioner;
pub mod ifa_vm_tool;
pub mod larql_tool;
pub mod manifesto_tool;

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub agent_id: crate::identity::AgentId,
    pub name: String,
    pub tier: u8,
    pub reputation: f64,
    pub odu_identity: crate::identity::odu::OduIdentity,
    pub workspace_root: PathBuf,
    pub sandbox_mode: bool,
}

use tokio::sync::OnceCell;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn required_tier(&self) -> u8;
    fn is_write_operation(&self) -> bool;
    fn params_schema(&self) -> Option<serde_json::Value> {
        None
    }
    /// Max seconds the registry allows this tool to run before timing out.
    /// Default 60s; long-running tools (e.g. full security scans) raise it.
    fn timeout_secs(&self) -> u64 {
        60
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String>;
}

pub struct LazyTool {
    name: String,
    description: String,
    required_tier: u8,
    is_write: bool,
    factory: Box<dyn Fn() -> Box<dyn Tool> + Send + Sync>,
    instance: OnceCell<Box<dyn Tool>>,
}

impl LazyTool {
    pub fn new(
        name: &str,
        description: &str,
        required_tier: u8,
        is_write: bool,
        factory: Box<dyn Fn() -> Box<dyn Tool> + Send + Sync>,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            required_tier,
            is_write,
            factory,
            instance: OnceCell::new(),
        }
    }

    async fn get_instance(&self) -> &dyn Tool {
        self.instance
            .get_or_init(|| async { (self.factory)() })
            .await
            .as_ref()
    }
}

#[async_trait]
impl Tool for LazyTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn required_tier(&self) -> u8 {
        self.required_tier
    }
    fn is_write_operation(&self) -> bool {
        self.is_write
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        None
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        self.get_instance().await.execute(params, context).await
    }
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    external_skills: Arc<Mutex<Vec<skills::SkillManifestEntry>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
            external_skills: Arc::new(Mutex::new(Vec::new())),
        };
        registry.register(Box::new(ReadFileTool));
        registry.register(Box::new(WriteFileTool));
        registry.register(Box::new(EditFileTool));
        registry.register(Box::new(GlobTool));
        registry.register(Box::new(GrepTool));
        registry.register(Box::new(BashTool));
        // wasmtime 13.0.1 carries known sandbox-escape CVEs (RUSTSEC-2026-0089/
        // 0094/0021, 2025-0118, ...; fixed only in wasmtime 43+/47 = a full
        // WASI-preview2 rewrite of sandbox.rs). The `wasm` tool is already
        // DangerFullAccess-gated, but keep the vulnerable path UNREGISTERED by
        // default -- opt in with OMOKODA_ENABLE_WASM=1 -- until the per-agent
        // microVM/gVisor sandbox tier retires it (or wasmtime is bumped).
        // Also requires the "wasm" feature flag (disabled on ARM64/Termux where
        // cranelift-codegen crashes rustc).
        #[cfg(feature = "wasm")]
        if std::env::var("OMOKODA_ENABLE_WASM").as_deref() == Ok("1") {
            registry.register(Box::new(LazyTool::new(
                "wasm",
                "Execute a WASM module in the sandbox",
                2,
                true,
                Box::new(|| Box::new(WasmTool)),
            )));
        }
        registry.register(Box::new(WebSearchTool));
        registry.register(Box::new(AgentOrchestrationTool));
        registry.register(Box::new(NoteTakingTool));

        // Ògún (Python) tool-execution bridge -- real service, was written
        // but never registered (see docs/audit/inspiration-followthrough-
        // connectionmap-256.md). Only registered when OGUN_URL is
        // configured, matching the OSUN_URL-gated pattern used elsewhere in
        // this codebase: an absent bridge means these tools simply aren't
        // offered, not a broken/erroring tool. `py_web_search` is
        // distinctly named to avoid colliding with the existing native
        // `WebSearchTool` above -- both stay available, callers choose.
        if std::env::var("PYTHON_TOOL_URL").is_ok() || std::env::var("OGUN_URL").is_ok() {
            registry.register(Box::new(python_bridge::web_search_py()));
            registry.register(Box::new(python_bridge::code_runner()));
            registry.register(Box::new(python_bridge::data_analysis()));
            registry.register(Box::new(python_bridge::cosmos()));
        }

        // Register Sovereign tools
        registry.register(Box::new(sovereign::ApplyPatchTool));
        registry.register(Box::new(sovereign::ExecTool));
        registry.register(Box::new(sovereign::ProcessTool));
        registry.register(Box::new(sovereign::WebFetchTool));
        registry.register(Box::new(sovereign::BrowserTool));
        registry.register(Box::new(sovereign::CanvasTool));
        registry.register(Box::new(sovereign::NodesTool));
        registry.register(Box::new(sovereign::ImageTool));
        registry.register(Box::new(sovereign::MessageTool));
        registry.register(Box::new(sovereign::CronTool));
        registry.register(Box::new(sovereign::GatewayTool));
        registry.register(Box::new(sovereign::SessionsListTool));
        registry.register(Box::new(sovereign::SessionsHistoryTool));
        registry.register(Box::new(sovereign::SessionsSendTool));
        registry.register(Box::new(sovereign::SessionsSpawnTool));
        registry.register(Box::new(sovereign::SessionStatusTool));
        registry.register(Box::new(sovereign::AgentsListTool));

        // Pattern-matched tools
        registry.register(Box::new(todo::WriteTodoTool));
        registry.register(Box::new(todo::ReadTodoTool));
        registry.register(Box::new(structured_output::StructuredOutputTool));
        registry.register(Box::new(zero_tool::ZeroTool));
        registry.register(Box::new(walrus_tool::WalrusTool));
        registry.register(Box::new(repl::ReplTool));
        registry.register(Box::new(config_tool::ConfigReadTool));
        registry.register(Box::new(config_tool::ConfigWriteTool));
        registry.register(Box::new(tor_tool::TorTool::new()));

        // Mesh tools (Block Mesh topology layer)
        registry.register(Box::new(mesh_tools::MeshProposeTool));
        registry.register(Box::new(mesh_tools::MeshRespondTool));
        registry.register(Box::new(mesh_tools::MeshQueryResourcesTool));
        registry.register(Box::new(mesh_tools::MeshReserveResourceTool));
        registry.register(Box::new(mesh_tools::MeshReleaseResourceTool));
        registry.register(Box::new(mesh_tools::MeshQueryNeighborsTool));
        registry.register(Box::new(mesh_tools::MeshQueryTrustTool));
        registry.register(Box::new(mesh_tools::MeshSignalEventTool));
        registry.register(Box::new(mesh_tools::MeshDiscoverCapabilitiesTool));

        // Agent wallets via Vantage (/api/agents/{id}/wallets/*). Keys are held
        // and signed server-side; the agent only carries intent + its X-Agent-Key.
        registry.register(Box::new(wallet_tools::WalletListTool));
        registry.register(Box::new(wallet_tools::WalletGetTool));
        registry.register(Box::new(wallet_tools::WalletCreateTool));
        registry.register(Box::new(wallet_tools::WalletSignTool));
        registry.register(Box::new(wallet_tools::WalletAlchemyApproveTool));
        registry.register(Box::new(nostr_identity_tool::NostrIdentityTool));
        registry.register(Box::new(twin_binding_tool::TwinBindingTool));
        registry.register(Box::new(if_script_tool::IfScriptTool));
        registry.register(Box::new(ifa_vm_tool::IfaVmTool));
        registry.register(Box::new(larql_tool::LarqlTool));
        registry.register(Box::new(manifesto_tool::ManifestoTool));

        // Real on-chain settlement via OSOVM's elegbara_router (Sui testnet).
        // Env-gated: OMOKODA_ELEGBARA_PACKAGE / OMOKODA_ELEGBARA_ROUTER_ID must
        // be set to verified live ids or execute() fails closed with an
        // explicit error — see tools/onchain_tools.rs.
        registry.register(Box::new(onchain_tools::SettleTransactionTaxTool));

        // Sovereign-node MCP bridge — physical capture + DIP messaging.
        // Activated when SOVEREIGN_NODE_URL is set (e.g. "http://localhost:8080").
        if std::env::var("SOVEREIGN_NODE_URL").is_ok() {
            for tool in sovereign_node::sovereign_node_tools() {
                registry.register(tool);
            }
        }

        // OSOVM simulation engine bridge.
        // Activated when OSOVM_URL is set (e.g. "http://localhost:7780").
        if std::env::var("OSOVM_URL").is_ok() {
            for tool in osovm_tool::osovm_tools() {
                registry.register(tool);
            }
        }

        // UCX compute tools — activated when UCX_BROKER_URL is set.
        // Also available for provider registration when VANTAGE_URL is set.
        if std::env::var("UCX_BROKER_URL").is_ok() || std::env::var("VANTAGE_URL").is_ok() {
            for tool in ucx_tool::ucx_tools() {
                registry.register(tool);
            }
            registry.register(Box::new(provider_tools::UcxRegisterProviderTool));
            registry.register(Box::new(provider_tools::UcxListProvidersTool));
            registry.register(Box::new(provider_tools::UcxDeregisterProviderTool));
            registry.register(Box::new(ucx_receipt::UcxJobStatusTool));
            registry.register(Box::new(ucx_receipt::UcxJobReceiptTool));
            registry.register(Box::new(ucx_policy::UcxCheckPolicyTool));
        }

        // VCP device inhabitation tools — activated when VCP_BROKER_URL or VANTAGE_URL is set.
        if std::env::var("VCP_BROKER_URL").is_ok() || std::env::var("VANTAGE_URL").is_ok() {
            for tool in ucx_tool::vcp_tools() {
                registry.register(tool);
            }
        }

        // OmoHome sovereign habitat tools — activated when OMOHOME_URL is set.
        if std::env::var("OMOHOME_URL").is_ok() {
            registry.register(Box::new(omohome_tool::OmoHomeListDevicesTool));
            registry.register(Box::new(omohome_tool::OmoHomeGetStateTool));
            registry.register(Box::new(omohome_tool::OmoHomeCallServiceTool));
            registry.register(Box::new(omohome_tool::OmoHomeListAreasTool));
            registry.register(Box::new(omohome_tool::OmoHomeHealthTool));
        }

        // Config-driven external service skills (ships with Vantage).
        for entry in skills::default_manifest().skills {
            registry.register_skill(entry);
        }
        registry.register(Box::new(skills::SkillsListTool::new(
            registry.external_skills.clone(),
        )));
        registry.register(Box::new(skillforge::SkillForgeTool::new(
            registry.external_skills.clone(),
        )));
        registry.register(Box::new(ytforge::YtForgeTool::new(
            registry.external_skills.clone(),
        )));

        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Register one external service skill (a config-driven HTTP adapter). The
    /// skill becomes invocable as `act <name>` and appears in the `skills` list.
    /// Hot-add a skill for the current session without `&mut self`. It becomes
    /// invocable through `execute`/`is_allowed` immediately via dynamic
    /// resolution and shows up in the `skills` discovery list. Used by
    /// SkillForge to make a freshly forged skill usable in the same session.
    pub fn add_session_skill(&self, entry: skills::SkillManifestEntry) {
        if let Ok(mut g) = self.external_skills.lock() {
            g.retain(|s| s.name != entry.name);
            g.push(entry);
        }
    }

    pub fn register_skill(&mut self, entry: skills::SkillManifestEntry) {
        if let Ok(mut g) = self.external_skills.lock() {
            g.push(entry.clone());
        }
        self.register(Box::new(skills::ExternalServiceTool::new(entry)));
    }

    /// Load a JSON skill manifest from `path` and register each entry. Returns
    /// the number of skills registered.
    pub fn load_skills_manifest(&mut self, path: &Path) -> Result<usize, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let manifest: skills::SkillManifest =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let n = manifest.skills.len();
        for entry in manifest.skills {
            self.register_skill(entry);
        }
        Ok(n)
    }

    /// Whether a tool by this name exists at all (statically registered or a
    /// session-forged skill) — regardless of tier. Lets callers give an accurate
    /// "unknown tool" error instead of a misleading reputation message.
    pub fn exists(&self, name: &str) -> bool {
        if self.tools.contains_key(name) {
            return true;
        }
        self.external_skills
            .lock()
            .ok()
            .map(|g| g.iter().any(|e| e.name == name))
            .unwrap_or(false)
    }

    pub fn is_allowed(&self, name: &str, tier: u8) -> bool {
        if let Some(t) = self.tools.get(name) {
            return tier >= t.required_tier();
        }
        // Skills forged during this session live in external_skills until the
        // next registry load; honor their declared tier so they are invocable.
        if let Ok(g) = self.external_skills.lock() {
            if let Some(e) = g.iter().find(|e| e.name == name) {
                return tier >= e.required_tier;
            }
        }
        false
    }

    pub fn list_available(
        &self,
        context: &ExecutionContext,
        policy: &crate::permissions::PermissionPolicy,
    ) -> Vec<String> {
        let mut list: Vec<String> = self
            .tools
            .values()
            .filter(|t| {
                // 1. Tier check
                if context.tier < t.required_tier() {
                    return false;
                }
                // 2. Policy check (Hide denied tools)
                // We use a dummy input for check
                let action =
                    if t.name().contains("read") || t.name() == "glob" || t.name() == "grep" {
                        "read"
                    } else if t.name().contains("write") || t.name() == "note_taking" {
                        "write"
                    } else if t.name() == "bash" || t.name() == "wasm" {
                        "exec"
                    } else if t.name() == "web_search" || t.name() == "web_fetch" {
                        "net"
                    } else {
                        "tool"
                    };

                matches!(
                    policy.patterns.check(action, "*"),
                    crate::permissions::PermissionOutcome::Allow
                )
            })
            .map(|t| t.name().to_string())
            .collect();
        list.sort();
        list
    }

    pub async fn execute(
        &self,
        name: &str,
        params: &str,
        context: ExecutionContext,
        policy: &crate::permissions::PermissionPolicy,
        prompter: Option<&mut (dyn crate::permissions::PermissionPrompter + Send)>,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        // Resolve either a statically-registered tool or a skill forged during
        // this session (present in external_skills but not yet in the tool map).
        let dynamic_tool: Option<Box<dyn Tool>> = if self.tools.contains_key(name) {
            None
        } else {
            self.external_skills
                .lock()
                .ok()
                .and_then(|g| g.iter().find(|e| e.name == name).cloned())
                .map(|entry| Box::new(skills::ExternalServiceTool::new(entry)) as Box<dyn Tool>)
        };
        let tool: &dyn Tool = match self.tools.get(name) {
            Some(t) => t.as_ref(),
            None => dynamic_tool
                .as_deref()
                .ok_or_else(|| format!("tool not found: {}", name))?,
        };

        if context.tier < tool.required_tier() {
            return Err(format!(
                "tool '{}' requires tier {}, current tier is {}",
                name,
                tool.required_tier(),
                context.tier
            ));
        }

        // 1. JSON Schema Validation
        if let Some(schema) = tool.params_schema() {
            let instance: serde_json::Value = serde_json::from_str(params)
                .map_err(|e| format!("Invalid JSON input for tool '{}': {}", name, e))?;

            let compiled = jsonschema::JSONSchema::compile(&schema)
                .map_err(|e| format!("Invalid JSON Schema for tool '{}': {}", name, e))?;

            let validation_result = compiled.validate(&instance);
            if let Err(errors) = validation_result {
                let error_msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
                return Err(format!(
                    "JSON Schema validation failed for tool '{}': {}",
                    name,
                    error_msgs.join(", ")
                ));
            }
        }

        // 2. Enforce mode (read-only)
        if let Err(e) = enforce_mode(policy.active_mode(), name, tool.is_write_operation()) {
            return Err(e.to_string());
        }

        // 3. Pre-act check: Permission Policy enforcement
        let auth_result = policy.authorize(name, params, prompter);
        if let crate::permissions::PermissionOutcome::Deny { reason } = auth_result {
            return Err(format!("Permission denied: {}", reason));
        }

        // 4. Execute with timeout and output limit (per-tool budget)
        let secs = tool.timeout_secs();
        let timeout_duration = std::time::Duration::from_secs(secs);
        let exec_res = tokio::time::timeout(timeout_duration, tool.execute(params, &context))
            .await
            .map_err(|_| format!("Tool '{}' timed out after {}s", name, secs))??;

        let (mut output, usage) = exec_res;

        // 5. Output size limit (max 10MB)
        if output.len() > 10 * 1024 * 1024 {
            output.truncate(10 * 1024 * 1024);
            output.push_str("\n... [OUTPUT TRUNCATED: EXCEEDED 10MB LIMIT]");
        }

        Ok((output, usage))
    }
}

pub struct ToolSummary {
    pub name: String,
    pub description: String,
    pub params_schema:
        Option<std::collections::HashMap<String, crate::tools::tool_definitions::ToolProperty>>,
    pub required: Vec<String>,
}

impl ToolRegistry {
    pub fn get_definition(&self, name: &str) -> Option<ToolSummary> {
        self.tools.get(name).map(|t| {
            // Convert the tool's own JSON-Schema params_schema() (used to
            // validate real /v1/act calls) into the flat property map the
            // agentic tool-calling loop hands the LLM. Without this, every
            // tool -- even ones with a real schema like write_file -- fell
            // back to a generic single string field named "input", which a
            // well-behaved model dutifully wraps its real args JSON inside
            // of (confirmed live: DeepSeek called read_file with
            // {"input": "{\"path\": \"Cargo.toml\"}"}, and the tool then
            // failed with "missing path" since it looks for a top-level
            // `path` key, not a nested, re-stringified one).
            let (params_schema, required) = match t.params_schema() {
                Some(schema) => {
                    let props = schema
                        .get("properties")
                        .and_then(|p| p.as_object())
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| {
                                    let type_ = v
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("string")
                                        .to_string();
                                    let description = v
                                        .get("description")
                                        .and_then(|d| d.as_str())
                                        .map(str::to_string);
                                    let enum_values = v.get("enum").and_then(|e| e.as_array()).map(
                                        |arr| {
                                            arr.iter()
                                                .filter_map(|x| x.as_str().map(str::to_string))
                                                .collect()
                                        },
                                    );
                                    (
                                        k.clone(),
                                        crate::tools::tool_definitions::ToolProperty {
                                            type_,
                                            description,
                                            enum_values,
                                        },
                                    )
                                })
                                .collect()
                        });
                    let required = schema
                        .get("required")
                        .and_then(|r| r.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    (props, required)
                }
                None => (None, Vec::new()),
            };
            ToolSummary {
                name: t.name().to_string(),
                description: t.description().to_string(),
                params_schema,
                required,
            }
        })
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Result from a tool search query
#[derive(Debug, Clone)]
pub struct ToolSearchResult {
    pub name: String,
    pub description: String,
    pub required_tier: u8,
    pub score: f32,
}

impl ToolRegistry {
    /// Fuzzy search for tools by name/description.
    ///
    /// Supports:
    ///   `"select:name1,name2"` — exact match by name
    ///   `"+required_term other terms"` — require terms starting with `+`
    ///   `"search terms"` — fuzzy match against name and description
    pub fn search(&self, query: &str, tier_filter: u8) -> Vec<ToolSearchResult> {
        // Handle "select:" prefix for exact picks
        if let Some(names) = query.strip_prefix("select:") {
            return names
                .split(',')
                .filter_map(|name| {
                    let name = name.trim();
                    self.tools
                        .get(name)
                        .filter(|t| t.required_tier() <= tier_filter)
                        .map(|t| ToolSearchResult {
                            name: t.name().to_string(),
                            description: t.description().to_string(),
                            required_tier: t.required_tier(),
                            score: 1.0,
                        })
                })
                .collect();
        }

        let query_tokens = canonical_tokens(query);
        let required_tokens: Vec<&str> = query_tokens
            .iter()
            .filter(|t| t.starts_with('+'))
            .map(|t| &t[1..])
            .collect();
        let optional_tokens: Vec<&str> = query_tokens
            .iter()
            .filter(|t| !t.starts_with('+'))
            .map(|t| t.as_str())
            .collect();

        let mut results: Vec<ToolSearchResult> = self
            .tools
            .values()
            .filter(|t| t.required_tier() <= tier_filter)
            .filter_map(|t| {
                let tool_tokens = canonical_tokens(&format!("{} {}", t.name(), t.description()));

                // All required tokens must be present
                if !required_tokens
                    .iter()
                    .all(|rt| tool_tokens.iter().any(|tt| tt.contains(rt)))
                {
                    return None;
                }

                // Score based on optional token matches
                let score = if optional_tokens.is_empty() {
                    0.5 // Return all tools if no query
                } else {
                    let matches = optional_tokens
                        .iter()
                        .filter(|ot| tool_tokens.iter().any(|tt| tt.contains(*ot)))
                        .count();
                    if matches == 0 {
                        return None;
                    }
                    matches as f32 / optional_tokens.len() as f32
                };

                Some(ToolSearchResult {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    required_tier: t.required_tier(),
                    score,
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }
}

/// Normalize a string into search tokens.
/// Strips "tool" suffix, lowercases, removes non-alphanumeric, splits on delimiters.
pub fn canonical_tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '+')
        .filter(|s| !s.is_empty() && s.len() > 1)
        .map(|s| s.trim_end_matches("tool").to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys())
            .finish()
    }
}

struct ReadFileTool;
#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read a file from the workspace. Params: JSON with {path, offset?, limit?}"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        // Support both raw path and JSON
        let (path, offset, limit) = if params.starts_with('{') {
            let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
            let path = v["path"].as_str().ok_or("missing path")?.to_string();
            let offset = v["offset"].as_u64().map(|n| n as usize);
            let limit = v["limit"].as_u64().map(|n| n as usize);
            (path, offset, limit)
        } else {
            (params.to_string(), None, None)
        };

        let workspace_root = &context.workspace_root;
        if let Err(e) = validate_path_boundary(workspace_root, Path::new(&path)) {
            return Err(e.to_string());
        }

        let output = file_ops::read_file(&path, offset, limit)
            .map_err(|e| format!("failed to read file: {}", e))?;
        let resp = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        Ok((resp, crate::usage::TokenUsage::default()))
    }
}

struct WriteFileTool;
#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Write a file to the workspace. Params: JSON with {path, content}"
    }
    fn required_tier(&self) -> u8 {
        1 // Builder tier
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        // Strict schema: this is a write operation, so the registry rejects
        // malformed params (missing/mistyped fields) BEFORE the file is touched.
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 },
                "content": { "type": "string" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let path = v["path"].as_str().ok_or("missing path")?;
        let content = v["content"].as_str().ok_or("missing content")?;

        let workspace_root = &context.workspace_root;
        if let Err(e) = validate_path_boundary(workspace_root, Path::new(&path)) {
            return Err(e.to_string());
        }

        let output = file_ops::write_file(path, content)
            .map_err(|e| format!("failed to write file: {}", e))?;
        let resp = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        Ok((resp, crate::usage::TokenUsage::default()))
    }
}

struct EditFileTool;
#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }
    fn description(&self) -> &str {
        "Edit a file in the workspace. Params: JSON with {path, old_string, new_string, replace_all?}"
    }
    fn required_tier(&self) -> u8 {
        1 // Builder tier
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        // Strict schema for a write operation: reject malformed edits up front.
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 },
                "old_string": { "type": "string" },
                "new_string": { "type": "string" },
                "replace_all": { "type": "boolean" }
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false
        }))
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
        let path = v["path"].as_str().ok_or("missing path")?;
        let old_string = v["old_string"].as_str().ok_or("missing old_string")?;
        let new_string = v["new_string"].as_str().ok_or("missing new_string")?;
        let replace_all = v["replace_all"].as_bool().unwrap_or(false);

        let workspace_root = &context.workspace_root;
        if let Err(e) = validate_path_boundary(workspace_root, Path::new(&path)) {
            return Err(e.to_string());
        }

        let output = file_ops::edit_file(path, old_string, new_string, replace_all)
            .map_err(|e| format!("failed to edit file: {}", e))?;
        let resp = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        Ok((resp, crate::usage::TokenUsage::default()))
    }
}

struct BashTool;
#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }
    fn description(&self) -> &str {
        "Execute a bash command with optional isolation"
    }
    fn required_tier(&self) -> u8 {
        2 // Creator tier
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        // P0 Security: Validate bash commands to prevent injection
        if let Err(e) = crate::execution::bash_validation::validate_bash_command(params) {
            return Err(format!("Security blocked: {}", e.reason));
        }

        let sandbox = context.sandbox_mode;

        if sandbox && params.contains("..") {
            return Err("sandboxed bash commands must not contain '..'".to_string());
        }

        let workspace_root = &context.workspace_root;
        let mut cmd = if sandbox {
            let mut c = Command::new("unshare");
            c.args([
                "--map-root-user",
                "--net",
                "--mount",
                "--pid",
                "--fork",
                "bash",
                "-c",
                params,
            ]);
            c.current_dir(workspace_root);
            c
        } else {
            let mut c = Command::new("bash");
            c.args(["-c", params]);
            c.current_dir(workspace_root);
            c
        };

        let output = cmd
            .output()
            .map_err(|e| format!("failed to execute bash: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok((stdout, crate::usage::TokenUsage::default()))
        } else {
            Err(format!(
                "bash failed with status {}: {}",
                output.status, stderr
            ))
        }
    }
}

struct WebSearchTool;
#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Search the web via DuckDuckGo Lite"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://duckduckgo.com/lite/?q={}",
            urlencoding::encode(params)
        );

        let resp = client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .send()
            .await
            .map_err(|e| format!("web search failed: {}", e))?;

        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed to read web search body: {}", e))?;

        // Return first 2000 chars for now
        let output = body.chars().take(2000).collect();
        Ok((output, crate::usage::TokenUsage::default()))
    }
}

struct GlobTool;
#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "Find files matching a pattern. Params: JSON with {pattern, path?}"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let (pattern, path) = if params.starts_with('{') {
            let v: serde_json::Value = serde_json::from_str(params).map_err(|e| e.to_string())?;
            let pattern = v["pattern"].as_str().ok_or("missing pattern")?.to_string();
            let path = v["path"].as_str().map(|s| s.to_string());
            (pattern, path)
        } else {
            (params.to_string(), None)
        };

        let workspace_root = &context.workspace_root;
        if let Err(e) = validate_path_boundary(workspace_root, Path::new(&pattern)) {
            return Err(e.to_string());
        }
        if let Some(ref p) = path {
            if let Err(e) = validate_path_boundary(workspace_root, Path::new(p)) {
                return Err(e.to_string());
            }
        }

        let output = file_ops::glob_search(&pattern, path.as_deref())
            .map_err(|e| format!("glob search failed: {}", e))?;
        let resp = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        Ok((resp, crate::usage::TokenUsage::default()))
    }
}

struct GrepTool;
#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search for a pattern in files. Params: JSON GrepSearchInput"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let input: file_ops::GrepSearchInput =
            serde_json::from_str(params).map_err(|e| format!("grep requires JSON input: {}", e))?;

        let workspace_root = &context.workspace_root;
        if let Some(ref p) = input.path {
            if let Err(e) = validate_path_boundary(workspace_root, Path::new(p)) {
                return Err(e.to_string());
            }
        }
        if let Some(ref g) = input.glob {
            if let Err(e) = validate_path_boundary(workspace_root, Path::new(g)) {
                return Err(e.to_string());
            }
        }

        let output =
            file_ops::grep_search(&input).map_err(|e| format!("grep search failed: {}", e))?;
        let resp = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        Ok((resp, crate::usage::TokenUsage::default()))
    }
}

#[cfg(feature = "wasm")]
struct WasmTool;
#[cfg(feature = "wasm")]
#[async_trait]
impl Tool for WasmTool {
    fn name(&self) -> &str {
        "wasm"
    }
    fn description(&self) -> &str {
        "Execute a WASM module in the sandbox"
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let mut parts = params.split_whitespace();
        let module_path = parts
            .next()
            .ok_or_else(|| "wasm tool requires module path".to_string())?;
        if module_path.is_empty() {
            return Err("wasm tool requires module path".to_string());
        }

        let workspace_root = &context.workspace_root;
        if validate_path_boundary(workspace_root, Path::new(module_path)).is_err() {
            return Err("module path must be relative and within workspace".to_string());
        }

        let args: Vec<String> = parts.map(|s| s.to_string()).collect();
        let wasm_sandbox =
            WasmSandbox::new().map_err(|e| format!("failed to initialize wasm sandbox: {}", e))?;
        let output =
            wasm_sandbox.execute_module(Path::new(module_path), &args, context.sandbox_mode)?;
        Ok((output, crate::usage::TokenUsage::default()))
    }
}

/// Real subagent spawning via the Elixir swarm (Yemọja). Params: JSON
/// `{"role": "...", "budget_synapse": N}`. Posts to `{YEMOJA_URL}/spawn_agent`
/// (see omokoda-swarm's http_api.ex), which births a genuine guest agent on
/// this same Rust kernel process (`AppState.guests`, keyed by its own
/// agent_id -- see server.rs's `birth_handler`) and registers a supervised
/// OTP GenServer for it. Fail-open like every other cross-language client in
/// this codebase: no `YEMOJA_URL` configured means orchestration is simply
/// unavailable, not a crash.
struct AgentOrchestrationTool;
#[async_trait]
impl Tool for AgentOrchestrationTool {
    fn name(&self) -> &str {
        "agent_orchestration"
    }
    fn description(&self) -> &str {
        "Spawn a real subagent via the Elixir swarm. Params: JSON {\"role\": \"...\", \"budget_synapse\": N}"
    }
    fn required_tier(&self) -> u8 {
        4
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let Ok(base_url) = std::env::var("YEMOJA_URL") else {
            return Err("agent_orchestration unavailable: YEMOJA_URL not configured".to_string());
        };
        let parsed: serde_json::Value =
            serde_json::from_str(params).map_err(|e| format!("invalid params JSON: {e}"))?;
        let role = parsed
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or("params must include a \"role\" string")?;
        let budget_synapse = parsed
            .get("budget_synapse")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let client = crate::bus::clients::HttpYemojaClient::new(base_url);
        let agent_id =
            crate::bus::clients::YemojaClient::spawn_agent(&client, role, budget_synapse).await?;

        Ok((
            format!("spawned subagent {agent_id} (role={role})"),
            crate::usage::TokenUsage::default(),
        ))
    }
}

struct NoteTakingTool;
#[async_trait]
impl Tool for NoteTakingTool {
    fn name(&self) -> &str {
        "note_taking"
    }
    fn description(&self) -> &str {
        "Record a persistent note in the workspace. Params: JSON {title, content}"
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let v: serde_json::Value = if params.starts_with('{') {
            serde_json::from_str(params).map_err(|e| e.to_string())?
        } else {
            serde_json::json!({
                "title": "manual_note",
                "content": params
            })
        };
        let title = v["title"].as_str().ok_or("missing title")?;
        let content = v["content"].as_str().ok_or("missing content")?;

        let path = format!("notes/{}.md", title.to_lowercase().replace(' ', "_"));
        // Don't validate boundary here yet if we want to auto-create 'notes/'

        let output = file_ops::write_file(&path, content)
            .map_err(|e| format!("failed to record note: {}", e))?;
        let resp = serde_json::to_string(&output).map_err(|e| e.to_string())?;
        Ok((resp, crate::usage::TokenUsage::default()))
    }
}
