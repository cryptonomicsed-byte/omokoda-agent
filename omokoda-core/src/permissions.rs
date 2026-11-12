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

    /// Precedence used by the permission matrix.
    ///
    /// This exists because the derived `Ord` ranks enum *variants* in
    /// declaration order, which puts `Prompt` ABOVE `DangerFullAccess`. Any
    /// `current_mode >= required_mode` check therefore auto-allowed every
    /// dangerous tool in Prompt mode — the tier 3-4 default — turning the
    /// "ask before escalating" mode into the most permissive one. `Prompt` is
    /// an interaction mode, not a capability level, so it is ranked by the
    /// capability it grants (workspace-write) and handled explicitly.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::ReadOnly => 0,
            Self::WorkspaceWrite | Self::Prompt => 1,
            Self::DangerFullAccess => 2,
            Self::Allow => 3,
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
        // Path actions are matched against a lexically normalised path so a
        // deny pattern like `read:/etc/*` cannot be stepped around with
        // `read:../../etc/passwd` (the previous code compared raw strings, so
        // traversal, `//` and `./` spelling all bypassed it).
        let resource_norm = if action == "read" || action == "write" {
            normalize_path_lexically(resource)
        } else {
            resource.to_string()
        };
        let target = format!("{}:{}", action, resource_norm);

        // 1. Deny always wins
        for pattern in &self.deny {
            if self.match_pattern(pattern, &target) || matches_exec_deny(pattern, action, resource)
            {
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

    /// Collapse `.` / `..` segments in a slash-separated string without
    /// touching the filesystem. A `..` that would climb above the root is
    /// recorded and forces the result to be treated as absolute, so
    /// `workspace/../../etc/passwd` normalizes to `/etc/passwd` and is then
    /// caught by the `read:/etc/*` deny rule instead of silently escaping.
    fn normalize_segments(rest: &str) -> String {
        let mut absolute = rest.starts_with('/');
        let mut out: Vec<&str> = Vec::new();
        for seg in rest.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if out.pop().is_none() {
                        absolute = true;
                    }
                }
                s => out.push(s),
            }
        }
        let mut joined = out.join("/");
        if absolute {
            joined.insert(0, '/');
        }
        joined
    }

    /// Normalize the resource half of an `action:resource` target.
    fn normalize_target(target: &str) -> String {
        match target.split_once(':') {
            Some((action, rest)) => format!("{}:{}", action, Self::normalize_segments(rest)),
            None => Self::normalize_segments(target),
        }
    }

    fn match_pattern(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        let normalized = Self::normalize_target(target);
        if let Some(prefix) = pattern.strip_suffix('*') {
            // Prefix match on the traversal-normalized target. Normalizing
            // (rather than string-matching the raw target) is what closes
            // `workspace/../../etc/passwd` -> `read:workspace/*`.
            return normalized.starts_with(prefix);
        }
        pattern == normalized
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

        // 2. Check Traditional Tier/Mode matrix.
        // `rank()` (not the derived Ord: Prompt must never outrank
        // DangerFullAccess — see PermissionMode::rank).
        let current_mode = self.active_mode();
        let required_mode = self.required_mode_for(tool_name);
        if current_mode == PermissionMode::Allow || current_mode.rank() >= required_mode.rank() {
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

/// Split a shell command into words at the separators bash treats as command
/// boundaries, so `exec:sudo` can be matched against `cd /tmp && sudo rm -rf /`.
fn shell_words(cmd: &str) -> Vec<String> {
    cmd.split(|c: char| {
        c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '`' | '$' | '\n' | '>')
    })
    .filter(|w| !w.is_empty())
    .map(|w| {
        w.trim_matches(|c| c == '"' || c == '\'' || c == ',')
            .to_string()
    })
    .collect()
}

/// Match an `exec:*` deny pattern against the *command*, not the raw string.
///
/// The deny list is written as `exec:sudo` / `exec:rm_rf`, but `check()` used to
/// compare it against the whole target `exec:sudo apt install -y`, which never
/// matches — both entries were decorative. `rm_rf` additionally has to cover
/// every flag spelling (`-rf`, `-fr`, `-r -f`, `--recursive --force`).
fn matches_exec_deny(pattern: &str, action: &str, cmd: &str) -> bool {
    if action != "exec" {
        return false;
    }
    let Some(denied) = pattern.strip_prefix("exec:") else {
        return false;
    };
    if denied == "rm_rf" {
        let words = shell_words(cmd);
        let mut recursive = false;
        let mut force = false;
        for w in &words {
            match w.as_str() {
                "--recursive" => recursive = true,
                "--force" => force = true,
                _ if w.starts_with('-') && !w.starts_with("--") => {
                    recursive |= w.contains('r') || w.contains('R');
                    force |= w.contains('f');
                }
                _ => {}
            }
        }
        if !(recursive && force) || !words.iter().any(|w| w == "rm") {
            return false;
        }
        // Only the catastrophic form is denied: a recursive force delete aimed
        // at an absolute path, a parent, a glob root, `*`, `~` or `$HOME`.
        // `rm -rf build` inside the workspace stays allowed (the bash tool's
        // own validator additionally hard-blocks `rm -rf /`).
        const DANGEROUS_TARGETS: [&str; 6] = ["/", "..", "../", "*", "~", "$HOME"];
        words.iter().skip(1).any(|w| {
            w.starts_with('/')
                || DANGEROUS_TARGETS.contains(&w.as_str())
                || w.starts_with("~/")
                || w.starts_with("$HOME")
        })
    } else {
        shell_words(cmd).iter().any(|w| w == denied)
    }
}

/// Resolve `.` and `..` segments without touching the filesystem (the target
/// may not exist yet).
fn normalize_path_lexically(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
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

    /// Regression: Prompt (tier 3-4 default) must never grant DangerFullAccess
    /// without an approver. The derived Ord ranked Prompt above
    /// DangerFullAccess, so `current_mode >= required_mode` allowed `bash`
    /// outright.
    #[test]
    fn prompt_mode_does_not_auto_allow_danger_full_access() {
        let policy = PermissionPolicy::default_steward_policy(PermissionMode::Prompt);
        match policy.authorize("bash", "echo hi", None) {
            PermissionOutcome::Deny { reason } => assert!(reason.contains("approval")),
            PermissionOutcome::Allow => panic!("Prompt mode auto-allowed bash"),
        }
        // ...while read-only / workspace tools stay usable without an approver.
        assert_eq!(
            policy.authorize("read_file", "/tmp/x", None),
            PermissionOutcome::Allow
        );
        assert_eq!(
            policy.authorize("edit_file", "notes/a.md", None),
            PermissionOutcome::Allow
        );
    }

    /// Regression: the deny list entries `exec:sudo` / `exec:rm_rf` were
    /// compared to the whole command string and never matched anything.
    #[test]
    fn exec_deny_patterns_match_real_commands() {
        let policy = PatternPolicy::default();
        for cmd in [
            "sudo apt install -y foo",
            "cd /tmp && sudo rm -rf /var",
            "rm -rf /",
            "rm -fr /tmp/x",
            "rm -r -f /var/lib",
            "rm --recursive --force /opt",
            "rm -rf ~/projects",
            "rm -rf *",
        ] {
            assert!(
                matches!(policy.check("exec", cmd), PermissionOutcome::Deny { .. }),
                "expected deny for {cmd}"
            );
        }
        // Ordinary commands must still pass the pattern layer — including an
        // in-workspace recursive delete, which is not catastrophic.
        assert_eq!(
            policy.check("exec", "rm build/tmp.o"),
            PermissionOutcome::Allow
        );
        assert_eq!(
            policy.check("exec", "rm -rf build/target"),
            PermissionOutcome::Allow
        );
        assert_eq!(policy.check("exec", "ls -la"), PermissionOutcome::Allow);
    }

    /// Regression: a `read:/etc/*` deny was bypassable with `..` traversal.
    #[test]
    fn path_traversal_does_not_bypass_deny_patterns() {
        let policy = PatternPolicy::default();
        for p in ["/etc/passwd", "/etc/./passwd", "/tmp/../etc/passwd"] {
            assert!(
                matches!(policy.check("read", p), PermissionOutcome::Deny { .. }),
                "expected deny for {p}"
            );
        }
    }
}
