use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
    Prompt,
    Allow,
}

impl PermissionMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
            Self::Prompt => "prompt",
            Self::Allow => "allow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub input: String,
    pub current_mode: PermissionMode,
    pub required_mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionPromptDecision {
    Allow,
    Deny { reason: String },
}

pub trait PermissionPrompter: std::fmt::Debug + Send {
    fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allow,
    Deny { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternPolicy {
    pub version: u32,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

impl Default for PatternPolicy {
    fn default() -> Self {
        Self {
            version: 1,
            allow: vec![
                "read:workspace/*".to_string(),
                "read:*".to_string(),
                "write:workspace/*".to_string(),
                "write:notes/*".to_string(),
                "write:*".to_string(),
                "exec:*".to_string(),
                "net:*".to_string(),
                // Tool-category actions (mesh_*, and any non-file tool) pass the
                // granular pattern layer here; the real gate for them is the
                // tier/reputation matrix (tools.is_allowed) + Hermetic gates.
                // Without this, every mesh tool is denied "no matching allow
                // pattern" regardless of tier.
                "tool:*".to_string(),
            ],
            deny: vec![
                "read:/etc/*".to_string(),
                "exec:sudo".to_string(),
                "exec:rm_rf".to_string(),
            ],
        }
    }
}

impl PatternPolicy {
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    pub fn check(&self, action: &str, resource: &str) -> PermissionOutcome {
        let target = format!("{}:{}", action, resource);

        // 1. Deny always wins
        for pattern in &self.deny {
            if self.match_pattern(pattern, &target) {
                return PermissionOutcome::Deny {
                    reason: format!(
                        "PatternPolicy: action '{}' is explicitly denied by pattern '{}'",
                        target, pattern
                    ),
                };
            }
        }

        // 2. Check allow list
        for pattern in &self.allow {
            if self.match_pattern(pattern, &target) {
                return PermissionOutcome::Allow;
            }
        }

        PermissionOutcome::Deny {
            reason: format!("PatternPolicy: no matching allow pattern for '{}'", target),
        }
    }

    fn match_pattern(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            if prefix.is_empty() {
                return true;
            }
            // Split action from resource on the first ':' so that action labels
            // (e.g. "net:") are not fed into Path::components(), which would treat
            // "net:" as a single path component and break wildcard matching.
            let (action_prefix, resource_prefix) = match prefix.find(':') {
                Some(i) => (&prefix[..=i], &prefix[i + 1..]),
                None => (prefix, ""),
            };
            if !target.starts_with(action_prefix) {
                return false;
            }
            if resource_prefix.is_empty() {
                return true; // "action:*" — any resource with this action
            }
            let target_resource = &target[action_prefix.len()..];
            // Normalize both sides to block path traversal and handle absolute paths.
            // Filtering RootDir/ParentDir components makes "/etc/passwd" → "etc/passwd"
            // and "workspace/../../etc" → "workspace/etc" (both sides normalized equally).
            let normalize = |s: &str| -> String {
                std::path::Path::new(s)
                    .components()
                    .filter(|c| matches!(c, std::path::Component::Normal(_)))
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            };
            let norm_target = normalize(target_resource);
            let norm_prefix = normalize(resource_prefix);
            norm_target.starts_with(&norm_prefix)
        } else {
            pattern == target
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionPolicy {
    active_mode: PermissionMode,
    tool_requirements: BTreeMap<String, PermissionMode>,
    #[serde(default)]
    pub patterns: PatternPolicy,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self::new(PermissionMode::WorkspaceWrite)
    }
}

impl PermissionPolicy {
    #[must_use]
    pub fn new(active_mode: PermissionMode) -> Self {
        Self {
            active_mode,
            tool_requirements: BTreeMap::new(),
            patterns: PatternPolicy::default(),
        }
    }

    #[must_use]
    pub fn default_steward_policy(active_mode: PermissionMode) -> Self {
        let mut policy = Self::new(active_mode);
        // NOTE: any tool NOT listed here falls back to DangerFullAccess
        // (see required_mode_for). Read-only discovery tools MUST be classified
        // explicitly or the HTTP /v1/act path (no prompter) denies them — which
        // silently broke capability discovery in the Axiom UI (`skills` → 400).

        // ── Read-only: pure reads / discovery, safe to run without escalation ──
        for t in [
            "read_file",
            "web_search",
            "glob",
            "grep",
            "skills",
            "session_status",
            "sessions_list",
            "sessions_history",
            "agents_list",
            "nodes",
            "todo_read",
            "config_read",
            "structured_output",
            "mesh_query_neighbors",
            "mesh_query_resources",
            "mesh_query_trust",
            "mesh_discover_capabilities",
            "wallet_list",
            "wallet_get",
            "nostr_identity",
            // wigolo's manifest is write:false -- search/fetch/crawl/extract/
            // cache/find_similar/research/agent/diff/watch are all read-only
            // web intelligence, no side effects on the agent's own sphere.
            "wigolo",
        ] {
            policy = policy.with_tool_requirement(t, PermissionMode::ReadOnly);
        }

        // ── Workspace-write: mutations within the agent's own sphere ──
        for t in [
            "note_taking",
            "todo_write",
            "edit_file",
            "apply_patch",
            "config_write",
            "message",
            "canvas",
            "image",
            "cron",
            "skillforge",
            "ytforge",
            "mesh_propose",
            "mesh_respond",
            "mesh_signal_event",
            "mesh_reserve_resource",
            "mesh_release_resource",
            "sessions_send",
            "sessions_spawn",
        ] {
            policy = policy.with_tool_requirement(t, PermissionMode::WorkspaceWrite);
        }

        // ── Danger-full-access: arbitrary host exec / system power ──
        // (bash, wasm, exec, process, repl, browser, gateway, agent_orchestration)
        // are intentionally left unlisted so they keep the DangerFullAccess default.
        for t in ["bash", "wasm", "agent_orchestration"] {
            policy = policy.with_tool_requirement(t, PermissionMode::DangerFullAccess);
        }

        policy
    }

    #[must_use]
    pub fn with_tool_requirement(
        mut self,
        tool_name: impl Into<String>,
        required_mode: PermissionMode,
    ) -> Self {
        self.tool_requirements
            .insert(tool_name.into(), required_mode);
        self
    }

    #[must_use]
    pub fn active_mode(&self) -> PermissionMode {
        self.active_mode
    }

    #[must_use]
    pub fn required_mode_for(&self, tool_name: &str) -> PermissionMode {
        self.tool_requirements
            .get(tool_name)
            .copied()
            .unwrap_or(PermissionMode::DangerFullAccess)
    }

    #[must_use]
    pub fn authorize(
        &self,
        tool_name: &str,
        input: &str,
        mut prompter: Option<&mut (dyn PermissionPrompter + Send)>,
    ) -> PermissionOutcome {
        // 1. Pattern policy check (granular allow/deny layer)
        // Map tool_name to action type
        let action = if tool_name.contains("read") || tool_name == "glob" || tool_name == "grep" {
            "read"
        } else if tool_name.contains("write") || tool_name == "note_taking" {
            "write"
        } else if tool_name == "bash" || tool_name == "wasm" {
            "exec"
        } else if tool_name == "web_search" || tool_name == "web_fetch" {
            "net"
        } else {
            "tool"
        };

        let pattern_outcome = self.patterns.check(action, input);
        if let PermissionOutcome::Deny { reason } = pattern_outcome {
            // Deny always wins — pattern policy is the strictest gate.
            return PermissionOutcome::Deny { reason };
        }

        // 2. Check Traditional Tier/Mode matrix
        let current_mode = self.active_mode();
        let required_mode = self.required_mode_for(tool_name);
        if current_mode == PermissionMode::Allow || current_mode >= required_mode {
            return PermissionOutcome::Allow;
        }

        let request = PermissionRequest {
            tool_name: tool_name.to_string(),
            input: input.to_string(),
            current_mode,
            required_mode,
        };

        if current_mode == PermissionMode::Prompt
            || (current_mode == PermissionMode::WorkspaceWrite
                && required_mode == PermissionMode::DangerFullAccess)
        {
            return match prompter.as_mut() {
                Some(prompter) => match prompter.decide(&request) {
                    PermissionPromptDecision::Allow => PermissionOutcome::Allow,
                    PermissionPromptDecision::Deny { reason } => PermissionOutcome::Deny { reason },
                },
                None => PermissionOutcome::Deny {
                    reason: format!(
                        "tool '{tool_name}' requires approval to escalate from {} to {}",
                        current_mode.as_str(),
                        required_mode.as_str()
                    ),
                },
            };
        }

        PermissionOutcome::Deny {
            reason: format!(
                "tool '{tool_name}' requires {} permission; current mode is {}",
                required_mode.as_str(),
                current_mode.as_str()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_policy_defaults() {
        let policy = PatternPolicy::default();

        // Allowed
        assert_eq!(
            policy.check("read", "workspace/file.txt"),
            PermissionOutcome::Allow
        );
        assert_eq!(
            policy.check("write", "workspace/note.txt"),
            PermissionOutcome::Allow
        );
        assert_eq!(policy.check("net", "google.com"), PermissionOutcome::Allow);

        // Denied by pattern
        assert!(matches!(
            policy.check("exec", "sudo"),
            PermissionOutcome::Deny { .. }
        ));

        // Denied by default (no matching allow)
        assert!(matches!(
            policy.check("read", "/etc/passwd"),
            PermissionOutcome::Deny { .. }
        ));
    }

    #[test]
    fn test_pattern_policy_wildcard() {
        let yaml = r#"
version: 1
allow:
  - "read:*"
deny:
  - "read:/etc/*"
"#;
        let policy = PatternPolicy::from_yaml(yaml).unwrap();

        assert_eq!(policy.check("read", "anything"), PermissionOutcome::Allow);
        assert!(matches!(
            policy.check("read", "/etc/passwd"),
            PermissionOutcome::Deny { .. }
        ));
    }
}
