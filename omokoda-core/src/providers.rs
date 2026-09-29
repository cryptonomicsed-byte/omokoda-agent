pub mod aliases;
pub mod streaming;

use crate::session::ConversationMessage;
use crate::tools::tool_definitions::{LlmResponse, ToolCall, ToolDefinition};
use crate::usage::TokenUsage;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Strip any `<think>...</think>` (or `<thinking>...</thinking>`) block a
/// reasoning model embedded directly in `message.content` rather than a
/// separate `reasoning_content` field -- observed live with OmniRoute's
/// default free model (a DeepSeek reasoning variant), which sometimes
/// returns raw chain-of-thought as plain prose inside content with no tag
/// at all. Tag-stripping only catches the tagged case; the untagged case
/// is addressed separately via an explicit system-prompt instruction not
/// to narrate reasoning. If stripping empties the string (a tag-only
/// response), fall back to the original text rather than losing content.
fn strip_reasoning_tags(text: &str) -> String {
    let mut result = text.to_string();
    for (open, close) in [("<think>", "</think>"), ("<thinking>", "</thinking>")] {
        while let Some(start) = result.find(open) {
            if let Some(end) = result[start..].find(close) {
                result.replace_range(start..start + end + close.len(), "");
            } else {
                break;
            }
        }
    }
    let trimmed = result.trim().to_string();
    if trimmed.is_empty() {
        text.trim().to_string()
    } else {
        trimmed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderClass {
    Local,
    BrowserLocal,
    RegisteredLocal,
    External,
    Hive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub name: String,
    pub class: ProviderClass,
    pub endpoint: String,
}

/// IRIS-derived sampling overrides for a single `generate` call. Optional --
/// a provider that doesn't override `generate_with_params` (the default
/// impl below) just ignores it and falls back to its own hardcoded values,
/// so adding this never breaks an existing implementor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenerationParams {
    pub temperature: f32,
    pub max_tokens: u32,
}

/// Coarse reasoning tier — used by the Steward to decide whether to delegate
/// complex planning to a stronger model or handle it locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReasoningTier {
    #[default]
    Basic,
    Standard,
    Deep,
}

/// Describes what a model instance can actually do. The Spark/Steward uses
/// this to decide routing, context compression, tool-call delegation, and
/// whether structured output or vision inputs are safe to pass.
///
/// All fields default to conservative values so existing `LlmProvider`
/// implementors need not change — override `capabilities()` only where
/// a model genuinely supports the feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Maximum context tokens this model accepts (prompt + completion).
    pub context_length: u32,
    /// Model can produce tool/function calls in a structured envelope.
    pub tool_calling: bool,
    /// Model reliably returns valid JSON when a schema is requested.
    pub structured_output: bool,
    /// Model accepts image/vision inputs.
    pub vision: bool,
    /// Model supports streaming token output.
    pub streaming: bool,
    /// Coarse quality tier for complex reasoning / planning tasks.
    pub reasoning: ReasoningTier,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            context_length: 8192,
            tool_calling: false,
            structured_output: false,
            vision: false,
            streaming: false,
            reasoning: ReasoningTier::Basic,
        }
    }
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn metadata(&self) -> &ProviderMetadata;

    /// Return what this model can do. The Steward uses this for routing.
    /// Default = conservative baseline; override per provider where known.
    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities::default()
    }

    async fn generate(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
    ) -> Result<(String, TokenUsage), String>;

    /// Same as `generate`, but honoring IRIS-routed temperature/max_tokens
    /// when the caller has them. Default impl ignores `_params` and defers
    /// to `generate` -- override only in providers where honoring real
    /// sampling params is worth the added surface (see OpenAIProvider).
    async fn generate_with_params(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        _params: Option<&GenerationParams>,
    ) -> Result<(String, TokenUsage), String> {
        self.generate(prompt, history).await
    }

    fn supports_tools(&self) -> bool {
        false
    }

    async fn generate_with_tools(
        &self,
        messages: &[ConversationMessage],
        _tools: &[ToolDefinition],
        _private: bool,
    ) -> Result<LlmResponse, String> {
        // Default: ignore tools, use last user message as prompt
        let prompt = messages
            .iter()
            .rev()
            .find(|m| m.role == crate::session::MessageRole::User)
            .map(|m| {
                m.blocks
                    .iter()
                    .filter_map(|b| match b {
                        crate::session::ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let (text, usage) = self.generate(&prompt, messages).await?;
        Ok(LlmResponse::Text {
            content: text,
            usage,
        })
    }
}

/// Capability requirements a caller can attach to a think/route call.
/// The registry uses these to skip providers that cannot satisfy them,
/// falling through to the next candidate in class-priority order.
/// All fields are optional — unset means "no requirement".
#[derive(Debug, Clone, Default)]
pub struct ThinkRequirements {
    /// Minimum context window needed (prompt + expected completion).
    pub min_context: Option<u32>,
    /// Provider must support structured tool/function calls.
    pub needs_tools: Option<bool>,
    /// Provider must support schema-constrained JSON output.
    pub needs_structured: Option<bool>,
    /// Provider must support image inputs.
    pub needs_vision: Option<bool>,
    /// Minimum reasoning quality tier.
    pub min_reasoning: Option<ReasoningTier>,
}

impl ThinkRequirements {
    /// Returns true when the given capabilities satisfy every set requirement.
    pub fn satisfied_by(&self, caps: &ModelCapabilities) -> bool {
        if let Some(ctx) = self.min_context {
            if caps.context_length < ctx {
                return false;
            }
        }
        if self.needs_tools == Some(true) && !caps.tool_calling {
            return false;
        }
        if self.needs_structured == Some(true) && !caps.structured_output {
            return false;
        }
        if self.needs_vision == Some(true) && !caps.vision {
            return false;
        }
        if let Some(tier) = self.min_reasoning {
            let order = |t: ReasoningTier| match t {
                ReasoningTier::Basic => 0u8,
                ReasoningTier::Standard => 1,
                ReasoningTier::Deep => 2,
            };
            if order(caps.reasoning) < order(tier) {
                return false;
            }
        }
        true
    }
}

pub struct ProviderRegistry {
    pub providers: Vec<Box<dyn LlmProvider>>,
    /// True when an in-process mock provider is active (tests). Private
    /// thoughts may use it — it never leaves the process — unlike a remote
    /// `default` provider, which the local-only gate below rejects.
    has_mock: bool,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: Vec::new(),
            has_mock: false,
        };
        // OSO Brain — the sovereign local LLM (QLoRA-fine-tuned GGUF served via
        // llama.cpp --server or Ollama). Registered first so it wins routing over
        // all external providers. Set OSO_BRAIN_URL to activate (e.g. http://localhost:8080).
        // Phase 28.3: try local sovereign brain first, external LLM only as fallback.
        if let Ok(url) = std::env::var("OSO_BRAIN_URL") {
            if !url.is_empty() {
                let token = std::env::var("OSO_BRAIN_TOKEN").unwrap_or_default();
                let model =
                    std::env::var("OSO_BRAIN_MODEL").unwrap_or_else(|_| "oso-brain".to_string());
                registry.register(Box::new(OpenAIProvider::compatible(
                    "oso-brain",
                    ProviderClass::RegisteredLocal,
                    token,
                    model,
                    url,
                )));
            }
        }

        registry.register(Box::new(OllamaProvider::new(
            "http://localhost:11434".to_string(),
        )));

        // OmniRoute — free OpenAI-compatible AI gateway (default localhost:8300),
        // no API key required. Registered ahead of Ollama so it is the working
        // default when no local model or BYOK key is present: this is how an
        // agent thinks "for free" out of the box. It routes to external models,
        // so it is class External (NOT eligible for /private thoughts, which
        // must stay on a local provider). Any BYOK provider (OpenAI/Anthropic/
        // LARQL) registered below lands ahead of it and takes priority.
        {
            let url = std::env::var("OMNIROUTE_URL")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "http://localhost:8300".to_string());
            // Default to a free, reliably-available direct model. The auto/*
            // combo router 503s under load ("Maximum combo retry limit"); a
            // pinned free model is steadier. Override with OMNIROUTE_MODEL.
            let model = std::env::var("OMNIROUTE_MODEL")
                .unwrap_or_else(|_| "oc/deepseek-v4-flash-free".to_string());
            // OmniRoute needs no key; send a non-empty dummy to avoid an empty
            // Authorization: Bearer header being rejected by some frontends.
            let token =
                std::env::var("OMNIROUTE_TOKEN").unwrap_or_else(|_| "omniroute-free".to_string());
            registry.register(Box::new(OpenAIProvider::compatible(
                "omniroute",
                ProviderClass::External,
                token,
                model,
                url,
            )));
        }

        // LARQL — a local transformer decompiled into a queryable vindex,
        // served by larql-server's OpenAI-compatible surface. Weights stay on
        // this machine, so it qualifies as a Local (private-thought-eligible)
        // provider when LARQL_URL points at localhost.
        if let Ok(url) = std::env::var("LARQL_URL") {
            if !url.is_empty() {
                let token = std::env::var("LARQL_TOKEN").unwrap_or_default();
                let model = std::env::var("LARQL_MODEL").unwrap_or_else(|_| "default".to_string());
                registry.register(Box::new(OpenAIProvider::compatible(
                    "larql",
                    ProviderClass::Local,
                    token,
                    model,
                    url,
                )));
            }
        }

        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            registry.register(Box::new(OpenAIProvider::new(api_key, None, None)));
        }
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            registry.register(Box::new(AnthropicProvider::new(api_key, None, None)));
        }

        registry
    }

    pub fn with_mock(response: String) -> Self {
        let mut registry = Self {
            providers: Vec::new(),
            has_mock: false,
        };
        registry.register(Box::new(MockProvider::new(response)));
        registry.has_mock = true;
        registry
    }

    /// An empty registry (no providers) — for tests that register their own.
    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
            has_mock: false,
        }
    }

    /// Whether an in-process mock provider is active (test mode).
    pub fn has_mock(&self) -> bool {
        self.has_mock
    }

    pub fn register(&mut self, provider: Box<dyn LlmProvider>) {
        self.providers.insert(0, provider);
    }

    pub fn register_openai(
        &mut self,
        api_key: String,
        model: Option<String>,
        endpoint: Option<String>,
    ) {
        self.register(Box::new(OpenAIProvider::new(api_key, model, endpoint)));
    }

    pub fn register_anthropic(
        &mut self,
        api_key: String,
        model: Option<String>,
        endpoint: Option<String>,
    ) {
        self.register(Box::new(AnthropicProvider::new(api_key, model, endpoint)));
    }

    pub fn is_allowed_in_private(&self, provider: &ProviderMetadata) -> bool {
        match provider.class {
            ProviderClass::Local => {
                provider.endpoint.contains("localhost")
                    || provider.endpoint.contains("127.0.0.1")
                    || provider.endpoint.contains("mock://")
            }
            ProviderClass::BrowserLocal => true,
            ProviderClass::RegisteredLocal => true,
            ProviderClass::External => false,
            ProviderClass::Hive => false,
        }
    }

    pub fn get_provider(&self, provider_name: &str) -> Option<&dyn LlmProvider> {
        let normalized = provider_name.to_lowercase();
        self.providers.iter().map(Box::as_ref).find(|provider| {
            let metadata = provider.metadata();
            metadata.name.to_lowercase() == normalized
                || metadata.endpoint.to_lowercase() == normalized
        })
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|provider| provider.metadata().name.clone())
            .collect()
    }

    pub fn is_known_provider(&self, provider_name: &str) -> bool {
        self.get_provider(provider_name).is_some()
    }

    pub async fn complete_with_tools(
        &self,
        provider_name: &str,
        messages: &[ConversationMessage],
        tools: &[ToolDefinition],
        private_mode: bool,
    ) -> Result<LlmResponse, String> {
        let provider = if provider_name.is_empty() || provider_name.eq_ignore_ascii_case("default")
        {
            // When tools are present, prefer a provider that natively supports
            // tool/function calling — falls back to the first available provider
            // if none declare tool_calling = true (e.g. Ollama without tools).
            let needs_tools = !tools.is_empty();
            self.providers
                .iter()
                .map(Box::as_ref)
                .find(|p| {
                    if private_mode && !self.is_allowed_in_private(p.metadata()) {
                        return false;
                    }
                    !needs_tools || p.capabilities().tool_calling
                })
                .or_else(|| {
                    // Fallback: ignore tool_calling requirement, just return first allowed
                    self.providers
                        .iter()
                        .map(Box::as_ref)
                        .find(|p| !private_mode || self.is_allowed_in_private(p.metadata()))
                })
        } else {
            self.get_provider(provider_name)
        };

        let provider = provider.ok_or_else(|| "no provider available".to_string())?;

        if private_mode && !self.is_allowed_in_private(provider.metadata()) {
            return Err("No local provider available in /private mode (HARD FAIL)".to_string());
        }

        match tokio::time::timeout(
            Duration::from_secs(60),
            provider.generate_with_tools(messages, tools, private_mode),
        )
        .await
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => Err(format!("provider error: {}", e)),
            Err(_) => Err("provider timed out".to_string()),
        }
    }

    pub async fn think(
        &self,
        provider: &str,
        prompt: &str,
        history: &[ConversationMessage],
        private_mode: bool,
    ) -> Result<(String, TokenUsage), String> {
        let provider_name = provider.trim();
        if provider_name.is_empty() || provider_name.eq_ignore_ascii_case("default") {
            return self.route_think(prompt, history, private_mode).await;
        }

        let provider = self
            .get_provider(provider_name)
            .ok_or_else(|| format!("provider '{}' not found", provider_name))?;

        let metadata = provider.metadata();
        if private_mode && !self.is_allowed_in_private(metadata) {
            return Err("No local provider available in /private mode (HARD FAIL)".to_string());
        }

        match tokio::time::timeout(Duration::from_secs(30), provider.generate(prompt, history))
            .await
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => Err(format!("provider '{}' error: {}", provider_name, e)),
            Err(_) => Err(format!("provider '{}' timed out", provider_name)),
        }
    }

    fn provider_order(private_mode: bool) -> &'static [ProviderClass] {
        if private_mode {
            &[
                ProviderClass::Local,
                ProviderClass::BrowserLocal,
                ProviderClass::RegisteredLocal,
            ]
        } else {
            &[
                ProviderClass::Local,
                ProviderClass::BrowserLocal,
                ProviderClass::RegisteredLocal,
                ProviderClass::External,
                ProviderClass::Hive,
            ]
        }
    }

    /// Select the first provider (in class-priority order) whose capabilities
    /// satisfy `reqs`. Returns `None` when no provider qualifies.
    pub fn select_provider_for(
        &self,
        reqs: &ThinkRequirements,
        private_mode: bool,
    ) -> Option<&dyn LlmProvider> {
        let order = Self::provider_order(private_mode);
        for provider_class in order {
            for provider in self
                .providers
                .iter()
                .filter(|p| p.metadata().class == *provider_class)
            {
                if private_mode && !self.is_allowed_in_private(provider.metadata()) {
                    continue;
                }
                if reqs.satisfied_by(&provider.capabilities()) {
                    return Some(provider.as_ref());
                }
            }
        }
        None
    }

    pub async fn route_think(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        private_mode: bool,
    ) -> Result<(String, TokenUsage), String> {
        self.route_think_with_requirements(
            prompt,
            history,
            private_mode,
            &ThinkRequirements::default(),
        )
        .await
    }

    /// Like `route_think` but skips any provider that cannot satisfy `reqs`.
    pub async fn route_think_with_requirements(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        private_mode: bool,
        reqs: &ThinkRequirements,
    ) -> Result<(String, TokenUsage), String> {
        let order = Self::provider_order(private_mode);
        for provider_class in order {
            for provider in self
                .providers
                .iter()
                .filter(|p| p.metadata().class == *provider_class)
            {
                let metadata = provider.metadata();

                if private_mode && !self.is_allowed_in_private(metadata) {
                    continue;
                }
                if !reqs.satisfied_by(&provider.capabilities()) {
                    continue;
                }

                match tokio::time::timeout(
                    Duration::from_secs(30),
                    provider.generate(prompt, history),
                )
                .await
                {
                    Ok(Ok(response)) => return Ok(response),
                    Ok(Err(_e)) => {}
                    Err(_) => {}
                }
            }
        }

        if private_mode {
            Err("No local provider available in /private mode (HARD FAIL)".to_string())
        } else {
            Err("Reasoning failed: no provider responded".to_string())
        }
    }

    /// Same as `think`, but threads IRIS-derived `GenerationParams` through
    /// to the chosen provider's `generate_with_params`.
    pub async fn think_with_params(
        &self,
        provider: &str,
        prompt: &str,
        history: &[ConversationMessage],
        private_mode: bool,
        params: Option<&GenerationParams>,
    ) -> Result<(String, TokenUsage), String> {
        let provider_name = provider.trim();
        if provider_name.is_empty() || provider_name.eq_ignore_ascii_case("default") {
            return self
                .route_think_with_params(prompt, history, private_mode, params)
                .await;
        }

        let provider = self
            .get_provider(provider_name)
            .ok_or_else(|| format!("provider '{}' not found", provider_name))?;

        let metadata = provider.metadata();
        if private_mode && !self.is_allowed_in_private(metadata) {
            return Err("No local provider available in /private mode (HARD FAIL)".to_string());
        }

        match tokio::time::timeout(
            Duration::from_secs(30),
            provider.generate_with_params(prompt, history, params),
        )
        .await
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => Err(format!("provider '{}' error: {}", provider_name, e)),
            Err(_) => Err(format!("provider '{}' timed out", provider_name)),
        }
    }

    /// `route_think`, but threading IRIS `GenerationParams` through each
    /// candidate provider's `generate_with_params`.
    pub async fn route_think_with_params(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        private_mode: bool,
        params: Option<&GenerationParams>,
    ) -> Result<(String, TokenUsage), String> {
        self.route_think_with_params_and_requirements(
            prompt,
            history,
            private_mode,
            params,
            &ThinkRequirements::default(),
        )
        .await
    }

    /// `route_think_with_params` + capability requirements. Skips providers
    /// that cannot satisfy `reqs` before attempting generation.
    pub async fn route_think_with_params_and_requirements(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        private_mode: bool,
        params: Option<&GenerationParams>,
        reqs: &ThinkRequirements,
    ) -> Result<(String, TokenUsage), String> {
        let order = Self::provider_order(private_mode);
        for provider_class in order {
            for provider in self
                .providers
                .iter()
                .filter(|p| p.metadata().class == *provider_class)
            {
                let metadata = provider.metadata();

                if private_mode && !self.is_allowed_in_private(metadata) {
                    continue;
                }
                if !reqs.satisfied_by(&provider.capabilities()) {
                    continue;
                }

                match tokio::time::timeout(
                    Duration::from_secs(30),
                    provider.generate_with_params(prompt, history, params),
                )
                .await
                {
                    Ok(Ok(response)) => return Ok(response),
                    Ok(Err(_e)) => {}
                    Err(_) => {}
                }
            }
        }

        if private_mode {
            Err("No local provider available in /private mode (HARD FAIL)".to_string())
        } else {
            Err("Reasoning failed: no provider responded".to_string())
        }
    }
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field(
                "providers",
                &self
                    .providers
                    .iter()
                    .map(|p| p.metadata().name.clone())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct OllamaProvider {
    metadata: ProviderMetadata,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: String) -> Self {
        Self {
            metadata: ProviderMetadata {
                name: "Ollama".to_string(),
                class: ProviderClass::Local,
                endpoint,
            },
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn generate(
        &self,
        prompt: &str,
        _history: &[ConversationMessage],
    ) -> Result<(String, TokenUsage), String> {
        let url = format!("{}/api/generate", self.metadata.endpoint);
        // Was hardcoded to "llama3", a model tag that was never actually
        // pulled on the live host -- every real call 404'd against a real,
        // running Ollama instance (found live during onboarding smoke
        // testing 2026-08-29). OLLAMA_MODEL lets each host point at
        // whatever it actually has pulled (`ollama list`); the fallback is
        // just the smallest model observed live on that host, not a claim
        // that every deployment has it.
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2:3b".to_string());
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        });

        let resp = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Ollama status error: {}", resp.status()));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let response = json["response"].as_str().unwrap_or("").to_string();

        let usage = TokenUsage {
            input_tokens: json["prompt_eval_count"].as_u64().unwrap_or(0) as u32,
            output_tokens: json["eval_count"].as_u64().unwrap_or(0) as u32,
            ..Default::default()
        };

        Ok((response, usage))
    }
}

#[derive(Debug)]
pub struct WebLLMProvider {
    metadata: ProviderMetadata,
}

impl WebLLMProvider {
    pub fn new() -> Self {
        Self {
            metadata: ProviderMetadata {
                name: "WebLLM".to_string(),
                class: ProviderClass::BrowserLocal,
                endpoint: "browserllm://local".to_string(),
            },
        }
    }
}

impl Default for WebLLMProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProvider for WebLLMProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn generate(
        &self,
        _prompt: &str,
        _history: &[ConversationMessage],
    ) -> Result<(String, TokenUsage), String> {
        Err("WebLLM provider not implemented".to_string())
    }
}

#[derive(Debug)]
pub struct OpenAIProvider {
    metadata: ProviderMetadata,
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: Option<String>, endpoint: Option<String>) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| "https://api.openai.com".to_string());
        let model = model.unwrap_or_else(|| "gpt-4o-mini".to_string());
        Self {
            metadata: ProviderMetadata {
                name: "OpenAI".to_string(),
                class: ProviderClass::External,
                endpoint,
            },
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    /// Any server speaking the OpenAI chat-completions wire format, under its
    /// own provider name and class. This is how self-hosted engines (e.g. a
    /// local LARQL `larql-server`) join the registry without new wire code.
    pub fn compatible(
        name: &str,
        class: ProviderClass,
        api_key: String,
        model: String,
        endpoint: String,
    ) -> Self {
        Self {
            metadata: ProviderMetadata {
                name: name.to_string(),
                class,
                endpoint,
            },
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    /// Shared chat-completions call, parameterized by temperature/max_tokens
    /// so both `generate` (hardcoded defaults) and `generate_with_params`
    /// (IRIS-routed values) share one implementation.
    async fn generate_impl(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        temperature: f32,
        max_tokens: u32,
    ) -> Result<(String, TokenUsage), String> {
        let url = if self.metadata.endpoint.contains("/v1/") {
            self.metadata.endpoint.clone()
        } else {
            format!(
                "{}/v1/chat/completions",
                self.metadata.endpoint.trim_end_matches('/')
            )
        };

        let mut messages = Vec::new();
        for message in history {
            let role = match message.role {
                crate::session::MessageRole::User => "user",
                crate::session::MessageRole::Assistant => "assistant",
                crate::session::MessageRole::System => "system",
                _ => "user",
            };
            let content = message
                .blocks
                .iter()
                .map(|block| match block {
                    crate::session::ContentBlock::Text { text } => text.clone(),
                    crate::session::ContentBlock::ToolResult { output, .. } => output.clone(),
                    crate::session::ContentBlock::ToolUse { input, .. } => input.clone(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            if !content.is_empty() {
                messages.push(serde_json::json!({"role": role, "content": content}));
            }
        }
        messages.push(serde_json::json!({"role": "user", "content": prompt}));

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": temperature,
            // Reasoning models (e.g. deepseek-v4-flash) spend tokens on hidden
            // reasoning before emitting the answer; too small a budget returns
            // empty content with finish_reason=length. Give real headroom.
            "max_tokens": max_tokens,
            // Must be explicit: OmniRoute (and some OpenAI-compatible gateways)
            // stream by default, returning text/event-stream chunks that would
            // break the single-object JSON parse below.
            "stream": false,
        });

        let resp = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("OpenAI status error: {}", resp.status()));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let message = &json["choices"][0]["message"];
        // Prefer the answer; if a reasoning model left content empty, fall back
        // to its reasoning so a thought is never silently lost.
        let mut response = strip_reasoning_tags(message["content"].as_str().unwrap_or(""));
        if response.is_empty() {
            response = message["reasoning_content"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string();
        }
        if response.is_empty() {
            return Err("provider returned empty content".to_string());
        }

        let usage = TokenUsage {
            input_tokens: json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            ..Default::default()
        };

        Ok((response, usage))
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn generate(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
    ) -> Result<(String, TokenUsage), String> {
        // Defaults match the pre-IRIS hardcoded values exactly, so behavior
        // is unchanged for any caller still using plain `generate`.
        self.generate_impl(prompt, history, 0.7, 2000).await
    }

    async fn generate_with_params(
        &self,
        prompt: &str,
        history: &[ConversationMessage],
        params: Option<&GenerationParams>,
    ) -> Result<(String, TokenUsage), String> {
        let (temperature, max_tokens) = params
            .map(|p| (p.temperature, p.max_tokens))
            .unwrap_or((0.7, 2000));
        self.generate_impl(prompt, history, temperature, max_tokens)
            .await
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            context_length: 128_000,
            tool_calling: true,
            structured_output: true,
            vision: false,
            streaming: true,
            reasoning: ReasoningTier::Standard,
        }
    }

    fn supports_tools(&self) -> bool {
        true
    }

    /// OpenAI-compatible function calling (works for DeepSeek, OmniRoute, and any
    /// OpenAI-compatible gateway). Maps the conversation — including prior tool
    /// calls and their results — into the chat/completions schema, advertises the
    /// tools as `type:function`, and parses `tool_calls` back out.
    async fn generate_with_tools(
        &self,
        messages: &[ConversationMessage],
        tools: &[ToolDefinition],
        _private: bool,
    ) -> Result<LlmResponse, String> {
        use crate::session::{ContentBlock, MessageRole};
        let url = if self.metadata.endpoint.contains("/v1/") {
            self.metadata.endpoint.clone()
        } else {
            format!(
                "{}/v1/chat/completions",
                self.metadata.endpoint.trim_end_matches('/')
            )
        };

        // Map conversation → OpenAI messages. Tool results become separate
        // role:"tool" messages (one per result); assistant tool calls carry a
        // tool_calls array.
        let mut api_messages: Vec<serde_json::Value> = Vec::new();
        for msg in messages {
            match msg.role {
                MessageRole::System | MessageRole::User => {
                    let text: String = msg
                        .blocks
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let role = if msg.role == MessageRole::System {
                        "system"
                    } else {
                        "user"
                    };
                    if !text.is_empty() {
                        api_messages.push(serde_json::json!({"role": role, "content": text}));
                    }
                }
                MessageRole::Assistant => {
                    let mut text = String::new();
                    let mut tool_calls = Vec::new();
                    for b in &msg.blocks {
                        match b {
                            ContentBlock::Text { text: t } => text.push_str(t),
                            ContentBlock::ToolUse { id, name, input } => {
                                tool_calls.push(serde_json::json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {"name": name, "arguments": input},
                                }));
                            }
                            _ => {}
                        }
                    }
                    let mut m = serde_json::json!({"role": "assistant"});
                    m["content"] = if text.is_empty() {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String(text)
                    };
                    if !tool_calls.is_empty() {
                        m["tool_calls"] = serde_json::Value::Array(tool_calls);
                    }
                    api_messages.push(m);
                }
                MessageRole::Tool => {
                    // Each tool result is its own role:"tool" message.
                    for b in &msg.blocks {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            output,
                            ..
                        } = b
                        {
                            api_messages.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": output,
                            }));
                        }
                    }
                }
            }
        }

        let api_tools: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": {
                            "type": t.input_schema.type_,
                            "properties": t.input_schema.properties.iter().map(|(k, v)| {
                                let mut prop = serde_json::json!({"type": v.type_});
                                if let Some(desc) = &v.description {
                                    prop["description"] = serde_json::Value::String(desc.clone());
                                }
                                (k.clone(), prop)
                            }).collect::<std::collections::HashMap<_, _>>(),
                            "required": t.input_schema.required,
                        },
                    },
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "messages": api_messages,
            "tools": api_tools,
            "max_tokens": 2000,
            "stream": false,
        });

        let resp = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI status error {}: {}", status, err));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let usage = TokenUsage {
            input_tokens: json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            ..Default::default()
        };
        let message = &json["choices"][0]["message"];

        // Tool calls requested?
        if let Some(calls) = message["tool_calls"].as_array() {
            if !calls.is_empty() {
                let parsed: Vec<crate::tools::tool_definitions::ToolCall> = calls
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        let f = &c["function"];
                        let name = f["name"].as_str()?.to_string();
                        let input = f["arguments"].as_str().unwrap_or("{}").to_string();
                        let id = c["id"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("call_{i}"));
                        Some(crate::tools::tool_definitions::ToolCall { id, name, input })
                    })
                    .collect();
                let text_prefix = message["content"].as_str().map(str::to_string);
                return Ok(LlmResponse::ToolUse {
                    text_prefix,
                    calls: parsed,
                    usage,
                });
            }
        }

        // Plain text (with reasoning fallback, mirroring generate()).
        let mut content = strip_reasoning_tags(message["content"].as_str().unwrap_or(""));
        if content.is_empty() {
            content = message["reasoning_content"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string();
        }
        Ok(LlmResponse::Text { content, usage })
    }
}

#[derive(Debug)]
pub struct AnthropicProvider {
    metadata: ProviderMetadata,
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: Option<String>, endpoint: Option<String>) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| "https://api.anthropic.com".to_string());
        let model = model.unwrap_or_else(|| "claude-3.0".to_string());
        Self {
            metadata: ProviderMetadata {
                name: "Anthropic".to_string(),
                class: ProviderClass::External,
                endpoint,
            },
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    async fn generate_with_tools_impl(
        &self,
        messages: &[ConversationMessage],
        tools: &[ToolDefinition],
        _private: bool,
    ) -> Result<LlmResponse, String> {
        let url = format!(
            "{}/v1/messages",
            self.metadata.endpoint.trim_end_matches('/')
        );

        // Build messages array for the Messages API
        let mut api_messages = Vec::new();
        for msg in messages {
            let role = match msg.role {
                crate::session::MessageRole::User => "user",
                crate::session::MessageRole::Assistant => "assistant",
                crate::session::MessageRole::Tool => "user", // tool results go as user role
                crate::session::MessageRole::System => continue,
            };

            let mut content_blocks = Vec::new();
            for block in &msg.blocks {
                match block {
                    crate::session::ContentBlock::Text { text } => {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": text
                        }));
                    }
                    crate::session::ContentBlock::ToolUse { id, name, input } => {
                        let input_value: serde_json::Value =
                            serde_json::from_str(input).unwrap_or(serde_json::json!({}));
                        content_blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input_value
                        }));
                    }
                    crate::session::ContentBlock::ToolResult {
                        tool_use_id,
                        output,
                        is_error,
                    } => {
                        content_blocks.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": output,
                            "is_error": is_error
                        }));
                    }
                }
            }

            if !content_blocks.is_empty() {
                api_messages.push(serde_json::json!({
                    "role": role,
                    "content": content_blocks
                }));
            }
        }

        // Build tools array
        let api_tools: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": {
                        "type": t.input_schema.type_,
                        "properties": t.input_schema.properties.iter().map(|(k, v)| {
                            let mut prop = serde_json::json!({
                                "type": v.type_
                            });
                            if let Some(desc) = &v.description {
                                prop["description"] = serde_json::Value::String(desc.clone());
                            }
                            (k.clone(), prop)
                        }).collect::<std::collections::HashMap<_, _>>(),
                        "required": t.input_schema.required
                    }
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": api_messages,
            "tools": api_tools,
        });

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic status error {}: {}", status, err_body));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

        let usage = TokenUsage {
            input_tokens: json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            ..Default::default()
        };

        let stop_reason = json["stop_reason"].as_str().unwrap_or("");
        let content = json["content"].as_array().cloned().unwrap_or_default();

        // Collect text and tool_use blocks
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in &content {
            match block["type"].as_str().unwrap_or("") {
                "text" => {
                    if let Some(t) = block["text"].as_str() {
                        text_parts.push(t.to_string());
                    }
                }
                "tool_use" => {
                    let id = block["id"].as_str().unwrap_or("").to_string();
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    let input =
                        serde_json::to_string(&block["input"]).unwrap_or_else(|_| "{}".to_string());
                    tool_calls.push(ToolCall { id, name, input });
                }
                _ => {}
            }
        }

        if stop_reason == "tool_use" || !tool_calls.is_empty() {
            let text_prefix = if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join(""))
            };
            Ok(LlmResponse::ToolUse {
                text_prefix,
                calls: tool_calls,
                usage,
            })
        } else {
            Ok(LlmResponse::Text {
                content: text_parts.join(""),
                usage,
            })
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            context_length: 200_000,
            tool_calling: true,
            structured_output: true,
            vision: true,
            streaming: true,
            reasoning: ReasoningTier::Deep,
        }
    }

    fn supports_tools(&self) -> bool {
        true
    }

    async fn generate_with_tools(
        &self,
        messages: &[ConversationMessage],
        tools: &[ToolDefinition],
        private: bool,
    ) -> Result<LlmResponse, String> {
        self.generate_with_tools_impl(messages, tools, private)
            .await
    }

    async fn generate(
        &self,
        prompt: &str,
        _history: &[ConversationMessage],
    ) -> Result<(String, TokenUsage), String> {
        let url = if self.metadata.endpoint.contains("/v1/") {
            self.metadata.endpoint.clone()
        } else {
            format!(
                "{}/v1/complete",
                self.metadata.endpoint.trim_end_matches('/')
            )
        };

        let body = serde_json::json!({
            "model": self.model,
            "prompt": format!("\n\nHuman: {}\n\nAssistant:", prompt),
            "max_tokens_to_sample": 500,
            "temperature": 0.7,
        });

        let resp = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Anthropic status error: {}", resp.status()));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let response = json["completion"].as_str().unwrap_or("").to_string();

        // Anthropic legacy /v1/complete might not return usage in the same way,
        // but we'll try to extract it if it's there.
        let usage = TokenUsage {
            input_tokens: json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            ..Default::default()
        };

        Ok((response, usage))
    }
}

#[derive(Debug)]
pub struct MockProvider {
    pub metadata: ProviderMetadata,
    pub response: String,
}

impl MockProvider {
    pub fn new(response: String) -> Self {
        Self {
            metadata: ProviderMetadata {
                name: "ollama".to_string(),
                class: ProviderClass::Local,
                endpoint: "mock://".to_string(),
            },
            response,
        }
    }

    pub fn new_external(name: &str, response: String) -> Self {
        Self {
            metadata: ProviderMetadata {
                name: name.to_string(),
                class: ProviderClass::External,
                endpoint: "https://mock.api".to_string(),
            },
            response,
        }
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn generate(
        &self,
        prompt: &str,
        _history: &[ConversationMessage],
    ) -> Result<(String, TokenUsage), String> {
        let usage = TokenUsage {
            input_tokens: prompt.split_whitespace().count() as u32,
            output_tokens: self.response.split_whitespace().count() as u32,
            ..Default::default()
        };
        Ok((self.response.clone(), usage))
    }

    fn supports_tools(&self) -> bool {
        true
    }

    async fn generate_with_tools(
        &self,
        _messages: &[ConversationMessage],
        _tools: &[ToolDefinition],
        _private: bool,
    ) -> Result<LlmResponse, String> {
        // Test helper: if response is "tool_call:name:input", emit a ToolUse response
        if let Some(rest) = self.response.strip_prefix("tool_call:") {
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() >= 2 {
                return Ok(LlmResponse::ToolUse {
                    text_prefix: None,
                    calls: vec![ToolCall {
                        id: "test-call-1".to_string(),
                        name: parts[0].to_string(),
                        input: if parts.len() > 2 {
                            parts[2].to_string()
                        } else {
                            "{}".to_string()
                        },
                    }],
                    usage: TokenUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                        ..Default::default()
                    },
                });
            }
        }
        let usage = TokenUsage {
            input_tokens: 10,
            output_tokens: self.response.split_whitespace().count() as u32,
            ..Default::default()
        };
        Ok(LlmResponse::Text {
            content: self.response.clone(),
            usage,
        })
    }
}

#[cfg(test)]
mod reasoning_strip_tests {
    use super::strip_reasoning_tags;

    #[test]
    fn strips_a_single_think_block() {
        let raw = "<think>let me analyze this</think>The actual answer.";
        assert_eq!(strip_reasoning_tags(raw), "The actual answer.");
    }

    #[test]
    fn strips_thinking_variant_tag() {
        let raw = "<thinking>step one, step two</thinking>Final answer here.";
        assert_eq!(strip_reasoning_tags(raw), "Final answer here.");
    }

    #[test]
    fn plain_answer_with_no_tags_is_untouched() {
        let raw = "Just a normal response, no reasoning tags at all.";
        assert_eq!(strip_reasoning_tags(raw), raw);
    }

    #[test]
    fn tag_only_response_falls_back_to_original_rather_than_empty() {
        let raw = "<think>only reasoning, no final answer</think>";
        // Stripping would leave an empty string -- fall back to the
        // original rather than silently discarding the only content a
        // reasoning model produced.
        assert_eq!(strip_reasoning_tags(raw), raw);
    }

    #[test]
    fn untagged_scratchpad_prose_is_not_touched_by_this_layer() {
        // The real-world case that motivated this fix (OmniRoute's
        // default DeepSeek model) doesn't use tags at all -- it writes
        // "Thinking. 1. **Analyze the Request:**..." as plain prose. This
        // function intentionally does NOT try to heuristically detect
        // that (too fragile/model-specific); the real fix for the
        // untagged case is the system-prompt instruction not to narrate
        // reasoning at all, applied in interpreter.rs.
        let raw = "Thinking. 1. **Analyze the Request:** ...";
        assert_eq!(strip_reasoning_tags(raw), raw);
    }
}
