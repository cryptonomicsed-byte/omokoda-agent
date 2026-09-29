use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Agent access tier — controls what tools and capabilities are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentTier {
    /// Read-only. Can observe state but cannot execute tools.
    Observer = 1,
    /// Standard tools allowed. No self-modification.
    #[default]
    Resident = 2,
    /// Full tool access including self-modify (skill_patch).
    Sovereign = 3,
}

impl AgentTier {
    pub fn can_execute_tools(&self) -> bool {
        *self >= Self::Resident
    }

    pub fn can_self_modify(&self) -> bool {
        *self >= Self::Sovereign
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Observer => "observer",
            Self::Resident => "resident",
            Self::Sovereign => "sovereign",
        }
    }
}

impl std::fmt::Display for AgentTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConfigSource {
    User,
    Project,
    Local,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub source: ConfigSource,
    pub path: PathBuf,
}

/// Feature flags controlling emergent agent behaviors
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Auto-compact session when message count exceeds threshold (default: true)
    pub auto_compact: bool,
    /// Message count threshold to trigger compaction (default: 50)
    pub compact_threshold: usize,
    /// Keep N most recent messages after compaction (default: 10)
    pub compact_keep_recent: usize,
    /// Auto-save memory to Living Odu after each think (default: true)
    pub auto_memory: bool,
    /// Enable hook pipeline for pre/post tool execution (default: true)
    pub hooks_enabled: bool,
    /// Auto-generate receipts for all acts (default: true)
    pub auto_receipts: bool,
    /// Maximum agentic loop iterations for think_agentic (default: 25)
    pub max_agentic_turns: u32,
    /// Sync vault after each think/broadcast (default: false)
    pub auto_export: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            auto_compact: true,
            compact_threshold: 50,
            compact_keep_recent: 10,
            auto_memory: true,
            hooks_enabled: true,
            auto_receipts: true,
            max_agentic_turns: 25,
            auto_export: false,
        }
    }
}

/// Full agent configuration with layered settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub source: ConfigSource,
    pub feature_flags: FeatureFlags,
    pub default_provider: String,
    pub default_privacy: bool,
    pub default_sandbox: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            source: ConfigSource::User,
            feature_flags: FeatureFlags::default(),
            default_provider: "default".to_string(),
            default_privacy: true,
            default_sandbox: true,
        }
    }
}

pub struct ConfigLoader {
    cwd: PathBuf,
    config_home: PathBuf,
}

impl ConfigLoader {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, config_home: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            config_home: config_home.into(),
        }
    }

    #[must_use]
    pub fn discover(&self) -> Vec<ConfigEntry> {
        let user_legacy_path = self.config_home.parent().map_or_else(
            || PathBuf::from(".omokoda.json"),
            |parent| parent.join(".omokoda.json"),
        );
        vec![
            ConfigEntry {
                source: ConfigSource::User,
                path: user_legacy_path,
            },
            ConfigEntry {
                source: ConfigSource::User,
                path: self.config_home.join("settings.json"),
            },
            ConfigEntry {
                source: ConfigSource::Project,
                path: self.cwd.join(".omokoda.json"),
            },
            ConfigEntry {
                source: ConfigSource::Project,
                path: self.cwd.join(".omokoda").join("settings.json"),
            },
            ConfigEntry {
                source: ConfigSource::Local,
                path: self.cwd.join(".omokoda").join("settings.local.json"),
            },
        ]
    }

    /// Load and merge all config sources, with Local > Project > User priority
    #[must_use]
    pub fn load_merged(&self) -> AgentConfig {
        let entries = self.discover();
        let mut config = AgentConfig::default();

        // Load in order: User, then Project, then Local (each overrides previous)
        for entry in &entries {
            if let Ok(content) = std::fs::read_to_string(&entry.path) {
                if let Ok(partial) = serde_json::from_str::<serde_json::Value>(&content) {
                    config = merge_config(config, partial, entry.source.clone());
                }
            }
        }
        config
    }
}

fn merge_config(
    mut base: AgentConfig,
    overlay: serde_json::Value,
    source: ConfigSource,
) -> AgentConfig {
    base.source = source;
    if let Some(v) = overlay["default_provider"].as_str() {
        base.default_provider = v.to_string();
    }
    if let Some(v) = overlay["default_privacy"].as_bool() {
        base.default_privacy = v;
    }
    if let Some(v) = overlay["default_sandbox"].as_bool() {
        base.default_sandbox = v;
    }
    // Feature flags
    if let Some(ff) = overlay.get("feature_flags") {
        if let Some(v) = ff["auto_compact"].as_bool() {
            base.feature_flags.auto_compact = v;
        }
        if let Some(v) = ff["compact_threshold"].as_u64() {
            base.feature_flags.compact_threshold = v as usize;
        }
        if let Some(v) = ff["compact_keep_recent"].as_u64() {
            base.feature_flags.compact_keep_recent = v as usize;
        }
        if let Some(v) = ff["auto_memory"].as_bool() {
            base.feature_flags.auto_memory = v;
        }
        if let Some(v) = ff["hooks_enabled"].as_bool() {
            base.feature_flags.hooks_enabled = v;
        }
        if let Some(v) = ff["auto_receipts"].as_bool() {
            base.feature_flags.auto_receipts = v;
        }
        if let Some(v) = ff["max_agentic_turns"].as_u64() {
            base.feature_flags.max_agentic_turns = v as u32;
        }
        if let Some(v) = ff["auto_export"].as_bool() {
            base.feature_flags.auto_export = v;
        }
    }
    base
}
