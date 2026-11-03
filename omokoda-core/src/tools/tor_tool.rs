/// Oniux-backed Tor isolation tool.
///
/// Provides `/tor on|off|status|check|run` slash commands that wrap
/// outbound execution in Linux network namespaces via
/// [oniux](https://gitlab.torproject.org/tpo/core/oniux).
///
/// Oniux is fundamentally different from torsocks: it uses Linux `unshare`
/// namespaces to ensure the routed process has *no* clearnet path, not just
/// a library intercept.  The result is strong traffic-isolation appropriate
/// for agent research and sensitive tasks.
///
/// # State model
///
/// `TorState` (Enabled/Disabled) is held in an `Arc<Mutex<TorState>>` that
/// lives for the lifetime of the `ToolRegistry` — i.e., the session.  A
/// `/tor on` in one turn persists through later turns because the registry is
/// not rebuilt between dispatches.
///
/// # Binary resolution
///
/// `ONIUX_BIN` env var → `oniux` on `PATH` → `~/.cargo/bin/oniux`
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::tools::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

// ── subcommand allow-list ──────────────────────────────────────────────────

/// External programs that `/tor run` will forward through oniux.
/// Deliberately narrow: no shells, no package managers.
const ALLOWED_RUN_CMDS: &[&str] = &[
    "curl",
    "wget",
    "dig",
    "nslookup",
    "host",
    "ping",
    "traceroute",
    "ssh",
    "git",
    "nc",
    "ncat",
    "openssl",
    "python3",
    "node",
];

// ── binary resolution ──────────────────────────────────────────────────────

/// Locate the `oniux` binary: `ONIUX_BIN` → PATH → `~/.cargo/bin/oniux`.
pub fn resolve_oniux() -> Option<PathBuf> {
    if let Ok(bin) = std::env::var("ONIUX_BIN") {
        if !bin.is_empty() {
            let p = PathBuf::from(&bin);
            if p.exists() {
                return Some(p);
            }
        }
    }
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("oniux");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let home = std::env::var("HOME").ok()?;
    let fallback = PathBuf::from(home).join(".cargo/bin/oniux");
    fallback.exists().then_some(fallback)
}

// ── state ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorState {
    Disabled,
    Enabled,
}

// ── params parsing ─────────────────────────────────────────────────────────

/// Parsed representation of `/tor` params.
#[derive(Debug)]
pub enum TorParams {
    On,
    Off,
    Status,
    Check,
    Run { cmd: String, args: Vec<String> },
}

pub fn parse_tor_params(params: &str) -> Result<TorParams, String> {
    let v: serde_json::Value =
        serde_json::from_str(params).map_err(|e| format!("invalid params JSON: {e}"))?;

    let route = v
        .get("route")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();

    match route.as_str() {
        "on" => Ok(TorParams::On),
        "off" => Ok(TorParams::Off),
        "status" => Ok(TorParams::Status),
        "check" => Ok(TorParams::Check),
        "run" => {
            let cmd = v
                .get("cmd")
                .and_then(|c| c.as_str())
                .ok_or("missing 'cmd' for /tor run — e.g. {\"route\":\"run\",\"cmd\":\"curl\",\"args\":[\"-s\",\"https://...\"]}")?
                .to_string();

            if !ALLOWED_RUN_CMDS.contains(&cmd.as_str()) {
                return Err(format!(
                    "cmd '{}' is not in the allow-list. Allowed: {}",
                    cmd,
                    ALLOWED_RUN_CMDS.join(", ")
                ));
            }

            let args: Vec<String> = v
                .get("args")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|x| match x {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            if args.len() > 32 {
                return Err("too many args (max 32)".to_string());
            }

            Ok(TorParams::Run { cmd, args })
        }
        other => Err(format!(
            "unknown /tor route '{other}'. Use: on | off | status | check | run"
        )),
    }
}

// ── tool ───────────────────────────────────────────────────────────────────

/// Executes commands through Tor using oniux Linux-namespace isolation.
///
/// Invoked as a slash command: `/tor on|off|status|check|run`.
/// The parser maps `/tor <route>` → `Act { tool: "tor", params: {"route":"<route>"} }`.
pub struct TorTool {
    state: Arc<Mutex<TorState>>,
}

impl TorTool {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TorState::Disabled)),
        }
    }

    /// Returns a clone of the state handle so external code can inspect or
    /// set the Tor state (e.g. for transparent routing in BashTool).
    pub fn state_handle(&self) -> Arc<Mutex<TorState>> {
        self.state.clone()
    }

    fn run_oniux(bin: &PathBuf, cmd: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(bin)
            .arg(cmd)
            .args(args)
            .output()
            .map_err(|e| format!("oniux exec failed: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(stdout)
        } else {
            Err(format!(
                "oniux {} exited {:?}: {}{}",
                cmd,
                output.status.code(),
                stderr,
                if stdout.is_empty() {
                    String::new()
                } else {
                    format!("\nstdout: {stdout}")
                }
            ))
        }
    }
}

impl Default for TorTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TorTool {
    fn name(&self) -> &str {
        "tor"
    }

    fn description(&self) -> &str {
        "Tor network isolation via oniux (Linux namespace routing). \
         Routes: on | off | status | check | run. \
         /tor on — enable Tor for this session. \
         /tor off — disable. \
         /tor status — show state + exit node IP. \
         /tor check — verify anonymity via check.torproject.org. \
         /tor run — execute a command through Tor (params: {\"cmd\":\"curl\",\"args\":[\"-s\",\"https://...\"]}). \
         Requires oniux binary (ONIUX_BIN, PATH, or ~/.cargo/bin/oniux)."
    }

    fn required_tier(&self) -> u8 {
        2 // Requires elevated reputation — anonymised network access
    }

    fn is_write_operation(&self) -> bool {
        false // read-only from the vault perspective; network ops are ephemeral
    }

    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let tor_params = parse_tor_params(params)?;

        match tor_params {
            TorParams::On => {
                // Verify oniux is reachable before toggling state
                let bin = resolve_oniux().ok_or_else(|| {
                    "oniux not found. Install with:\n  \
                     cargo install --git https://gitlab.torproject.org/tpo/core/oniux oniux\n\
                     or set ONIUX_BIN to the binary path."
                        .to_string()
                })?;

                // The probe result used to be discarded, so a present-but-broken
                // `oniux` still flipped Tor to "enabled" and every /tor run then
                // failed at exec time with a confusing error.
                let probe = Command::new(&bin)
                    .arg("--version")
                    .output()
                    .map_err(|e| format!("oniux --version failed: {e}"))?;
                if !probe.status.success() {
                    return Err(format!(
                        "oniux at {} is not usable: `oniux --version` exited with {}{}",
                        bin.display(),
                        probe.status,
                        match String::from_utf8_lossy(&probe.stderr).trim() {
                            "" => String::new(),
                            e => format!(" ({e})"),
                        }
                    ));
                }

                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| "tor state lock poisoned".to_string())?;
                *state = TorState::Enabled;

                Ok((
                    format!(
                        "🧅 Tor enabled via oniux ({})\n\
                         All /tor run commands will now be isolated through Tor.\n\
                         Use /tor status to verify your exit node.",
                        bin.display()
                    ),
                    TokenUsage::default(),
                ))
            }

            TorParams::Off => {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| "tor state lock poisoned".to_string())?;
                *state = TorState::Disabled;
                Ok((
                    "🔓 Tor disabled. Traffic will route over clearnet.".to_string(),
                    TokenUsage::default(),
                ))
            }

            TorParams::Status => {
                let state = *self
                    .state
                    .lock()
                    .map_err(|_| "tor state lock poisoned".to_string())?;

                if state == TorState::Disabled {
                    return Ok((
                        "🔓 Tor is OFF (clearnet)".to_string(),
                        TokenUsage::default(),
                    ));
                }

                let bin = resolve_oniux()
                    .ok_or("oniux not found. Run /tor on to re-verify installation.")?;

                let ip_output = Self::run_oniux(
                    &bin,
                    "curl",
                    &["-s", "--max-time", "10", "https://api.ipify.org"],
                )
                .unwrap_or_else(|e| format!("[IP lookup failed: {e}]"));

                Ok((
                    format!("🧅 Tor is ON\nExit IP: {}", ip_output.trim()),
                    TokenUsage::default(),
                ))
            }

            TorParams::Check => {
                let bin = resolve_oniux().ok_or(
                    "oniux not found. Install oniux first — see /tor on for instructions.",
                )?;

                let state = *self
                    .state
                    .lock()
                    .map_err(|_| "tor state lock poisoned".to_string())?;

                let result = Self::run_oniux(
                    &bin,
                    "curl",
                    &[
                        "-s",
                        "--max-time",
                        "15",
                        "https://check.torproject.org/api/ip",
                    ],
                )?;

                let on_tor = result.contains("\"IsTor\":true");
                let status_icon = if on_tor { "✅" } else { "⚠️" };
                let session_state = if state == TorState::Enabled {
                    "session state: ON"
                } else {
                    "session state: OFF (but oniux ran directly)"
                };

                Ok((
                    format!("{status_icon} Tor check result ({session_state}):\n{result}"),
                    TokenUsage::default(),
                ))
            }

            TorParams::Run { cmd, args } => {
                let state = *self
                    .state
                    .lock()
                    .map_err(|_| "tor state lock poisoned".to_string())?;

                let bin = resolve_oniux()
                    .ok_or("oniux not found. Run /tor on first to verify installation.")?;

                let note = if state == TorState::Disabled {
                    "\n⚠️  Note: Tor session state is OFF — running isolated anyway via oniux."
                } else {
                    ""
                };

                let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let output = Self::run_oniux(&bin, &cmd, &arg_refs)?;

                Ok((format!("{output}{note}"), TokenUsage::default()))
            }
        }
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    /// `ONIUX_BIN` is process-global; tests that mutate or depend on it must
    /// serialize or they race under cargo's parallel harness.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    use super::*;

    /// `ONIUX_BIN` is process-global. Two tests mutate it, so without a lock
    /// `on_errors_without_oniux`'s resolve-on-enter check can land inside the
    /// window `run_via_echo_bin` holds the var set — the pair then fails
    /// intermittently under the default parallel test runner.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Restores `ONIUX_BIN` on drop, including when the test panics (the old
    /// manual restore at the end of the body leaked `/bin/echo` into every
    /// later test in the binary).
    struct OniuxBinGuard(Option<String>);

    impl OniuxBinGuard {
        fn set(value: &str) -> Self {
            let prev = std::env::var("ONIUX_BIN").ok();
            std::env::set_var("ONIUX_BIN", value);
            Self(prev)
        }
    }

    impl Drop for OniuxBinGuard {
        fn drop(&mut self) {
            match self.0.take() {
                Some(v) => std::env::set_var("ONIUX_BIN", v),
                None => std::env::remove_var("ONIUX_BIN"),
            }
        }
    }

    #[test]
    fn parse_on_off_status() {
        assert!(matches!(
            parse_tor_params(r#"{"route":"on"}"#).unwrap(),
            TorParams::On
        ));
        assert!(matches!(
            parse_tor_params(r#"{"route":"off"}"#).unwrap(),
            TorParams::Off
        ));
        assert!(matches!(
            parse_tor_params(r#"{"route":"status"}"#).unwrap(),
            TorParams::Status
        ));
        assert!(matches!(
            parse_tor_params(r#"{"route":"check"}"#).unwrap(),
            TorParams::Check
        ));
    }

    #[test]
    fn parse_run_valid() {
        let p =
            parse_tor_params(r#"{"route":"run","cmd":"curl","args":["-s","https://example.com"]}"#)
                .unwrap();
        if let TorParams::Run { cmd, args } = p {
            assert_eq!(cmd, "curl");
            assert_eq!(args, vec!["-s", "https://example.com"]);
        } else {
            panic!("expected Run variant");
        }
    }

    #[test]
    fn parse_run_rejects_shell() {
        assert!(parse_tor_params(r#"{"route":"run","cmd":"bash","args":["-c","evil"]}"#).is_err());
        assert!(
            parse_tor_params(r#"{"route":"run","cmd":"sh","args":["-c","rm -rf /"]}"#).is_err()
        );
    }

    #[test]
    fn parse_run_requires_cmd() {
        assert!(parse_tor_params(r#"{"route":"run"}"#).is_err());
    }

    #[test]
    fn parse_rejects_unknown_route() {
        let err = parse_tor_params(r#"{"route":"evil"}"#).unwrap_err();
        assert!(err.contains("unknown /tor route"));
    }

    #[test]
    fn parse_rejects_bad_json() {
        assert!(parse_tor_params("not json").is_err());
    }

    #[test]
    fn tool_metadata() {
        let t = TorTool::new();
        assert_eq!(t.name(), "tor");
        assert_eq!(t.required_tier(), 2);
        assert!(!t.is_write_operation());
    }

    #[tokio::test]
    async fn on_errors_without_oniux() {
        // Hold the env lock: `run_via_echo_bin` sets ONIUX_BIN process-wide.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // If oniux is not installed (typical CI), /tor on should return an
        // informative Err, not panic.
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if resolve_oniux().is_some() {
            return; // skip when oniux is actually installed
        }
        let t = TorTool::new();
        let ctx = ExecutionContext {
            agent_id: crate::identity::AgentId::from_str("test-agent"),
            name: "test".to_string(),
            tier: 2,
            reputation: 50.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: String::new(),
            },
            workspace_root: std::env::temp_dir(),
            sandbox_mode: false,
        };
        let result = t.execute(r#"{"route":"on"}"#, &ctx).await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("oniux not found"), "got: {msg}");
        assert!(msg.contains("cargo install"), "got: {msg}");
    }

    #[tokio::test]
    async fn off_works_without_oniux() {
        // /tor off only changes state, no binary needed
        let t = TorTool::new();
        let ctx = ExecutionContext {
            agent_id: crate::identity::AgentId::from_str("test-agent"),
            name: "test".to_string(),
            tier: 2,
            reputation: 50.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: String::new(),
            },
            workspace_root: std::env::temp_dir(),
            sandbox_mode: false,
        };
        let (msg, _) = t
            .execute(r#"{"route":"off"}"#, &ctx)
            .await
            .expect("off should succeed without oniux");
        assert!(msg.contains("disabled"), "got: {msg}");
    }

    #[tokio::test]
    async fn status_when_disabled() {
        let t = TorTool::new();
        let ctx = ExecutionContext {
            agent_id: crate::identity::AgentId::from_str("test-agent"),
            name: "test".to_string(),
            tier: 2,
            reputation: 50.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: String::new(),
            },
            workspace_root: std::env::temp_dir(),
            sandbox_mode: false,
        };
        let (msg, _) = t
            .execute(r#"{"route":"status"}"#, &ctx)
            .await
            .expect("status always returns Ok when disabled");
        assert!(msg.contains("OFF"), "got: {msg}");
    }

    #[tokio::test]
    async fn run_via_echo_bin() {
        // Point ONIUX_BIN at /bin/echo to test the exec path without real Tor.
        // oniux is called as: oniux <cmd> <args...>, so echo will emit
        // "curl -s https://example.com" which proves we exec without a shell.
        let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = OniuxBinGuard::set("/bin/echo");
        assert!(resolve_oniux().is_some(), "guard must make resolve succeed");

        let t = TorTool::new();
        let ctx = ExecutionContext {
            agent_id: crate::identity::AgentId::from_str("test-agent"),
            name: "test".to_string(),
            tier: 2,
            reputation: 50.0,
            odu_identity: crate::identity::odu::OduIdentity {
                primary_index: 0,
                mnemonic: String::new(),
            },
            workspace_root: std::env::temp_dir(),
            sandbox_mode: false,
        };
        let (out, _) = t
            .execute(
                r#"{"route":"run","cmd":"curl","args":["-s","https://example.com"]}"#,
                &ctx,
            )
            .await
            .expect("echo succeeds");
        assert!(out.contains("curl"), "got: {out}");
        assert!(out.contains("https://example.com"), "got: {out}");
    }
}
