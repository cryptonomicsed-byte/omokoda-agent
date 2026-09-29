use std::path::Path;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

/// Filesystem isolation mode for tool execution
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FilesystemMode {
    Off,
    WorkspaceOnly,
    AllowList(Vec<std::path::PathBuf>),
    ReadOnly,
}

/// Network isolation mode
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NetworkMode {
    Isolated,
    Full,
    AllowList(Vec<String>),
}

/// Per-tool sandbox profile — determines isolation level for tool execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxProfile {
    pub name: String,
    pub filesystem: FilesystemMode,
    pub network: NetworkMode,
    pub use_namespaces: bool,
    pub timeout_secs: u64,
    pub max_output_bytes: usize,
}

impl SandboxProfile {
    pub fn strict() -> Self {
        Self {
            name: "strict".to_string(),
            filesystem: FilesystemMode::WorkspaceOnly,
            network: NetworkMode::Isolated,
            use_namespaces: true,
            timeout_secs: 30,
            max_output_bytes: 1024 * 1024,
        }
    }

    pub fn read_only() -> Self {
        Self {
            name: "read_only".to_string(),
            filesystem: FilesystemMode::ReadOnly,
            network: NetworkMode::Isolated,
            use_namespaces: false,
            timeout_secs: 10,
            max_output_bytes: 512 * 1024,
        }
    }

    pub fn networked() -> Self {
        Self {
            name: "networked".to_string(),
            filesystem: FilesystemMode::WorkspaceOnly,
            network: NetworkMode::Full,
            use_namespaces: false,
            timeout_secs: 60,
            max_output_bytes: 2 * 1024 * 1024,
        }
    }

    pub fn unrestricted() -> Self {
        Self {
            name: "unrestricted".to_string(),
            filesystem: FilesystemMode::Off,
            network: NetworkMode::Full,
            use_namespaces: false,
            timeout_secs: 0,
            max_output_bytes: 10 * 1024 * 1024,
        }
    }

    pub fn for_tool(tool_name: &str) -> Self {
        match tool_name {
            "bash" | "wasm" | "exec" => Self::strict(),
            "write_file" | "edit_file" | "apply_patch" => Self::strict(),
            "read_file" | "glob" | "grep" => Self::read_only(),
            "web_search" | "web_fetch" => Self::networked(),
            _ => Self::strict(),
        }
    }

    pub fn is_container_environment() -> bool {
        std::path::Path::new("/.dockerenv").exists()
            || std::env::var("KUBERNETES_SERVICE_HOST").is_ok()
            || std::env::var("CI").is_ok()
    }
}

/// Adapter that bridges a `SandboxProfile` to actual tool execution.
/// Manages the toggle (enabled/disabled per tool) and dependency checks.
#[derive(Debug, Clone)]
pub struct SandboxAdapter {
    pub profile: SandboxProfile,
    pub enabled: bool,
}

impl SandboxAdapter {
    pub fn for_tool(tool_name: &str) -> Self {
        Self {
            profile: SandboxProfile::for_tool(tool_name),
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            profile: SandboxProfile::unrestricted(),
            enabled: false,
        }
    }

    /// Toggle sandbox on or off for this tool
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Returns the effective profile — None if sandbox is disabled
    pub fn effective_profile(&self) -> Option<&SandboxProfile> {
        if self.enabled {
            Some(&self.profile)
        } else {
            None
        }
    }

    /// Check if Linux namespace support (unshare) is available on this host
    pub fn has_namespace_support() -> bool {
        if SandboxProfile::is_container_environment() {
            return false;
        }
        std::process::Command::new("unshare")
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Return the effective timeout — 0 means no limit
    pub fn effective_timeout(&self) -> u64 {
        if self.enabled {
            self.profile.timeout_secs
        } else {
            0
        }
    }

    /// Return the effective max output size in bytes
    pub fn effective_max_output(&self) -> usize {
        if self.enabled {
            self.profile.max_output_bytes
        } else {
            usize::MAX
        }
    }
}

#[cfg(test)]
mod sandbox_adapter_tests {
    use super::*;

    #[test]
    fn test_adapter_for_tool_enabled() {
        let adapter = SandboxAdapter::for_tool("bash");
        assert!(adapter.enabled);
        assert!(adapter.effective_profile().is_some());
    }

    #[test]
    fn test_adapter_toggle() {
        let mut adapter = SandboxAdapter::for_tool("read_file");
        assert!(adapter.enabled);
        adapter.toggle();
        assert!(!adapter.enabled);
        assert!(adapter.effective_profile().is_none());
        adapter.toggle();
        assert!(adapter.enabled);
    }

    #[test]
    fn test_adapter_disabled_returns_no_profile() {
        let adapter = SandboxAdapter::disabled();
        assert!(adapter.effective_profile().is_none());
    }

    #[test]
    fn test_read_only_profile() {
        let adapter = SandboxAdapter::for_tool("grep");
        let profile = adapter.effective_profile().unwrap();
        assert_eq!(profile.filesystem, FilesystemMode::ReadOnly);
    }
}

// ── Container detection + Linux namespace sandbox ─────────────────────────────

/// Detected container environment markers
#[derive(Debug, Clone, Default)]
pub struct ContainerEnvironment {
    pub in_container: bool,
    pub markers: Vec<String>,
}

/// Inputs for the testable container-detection variant
pub struct ContainerDetectionInputs<'a> {
    pub env_pairs: Vec<(String, String)>,
    pub dockerenv_exists: bool,
    pub containerenv_exists: bool,
    pub proc_1_cgroup: Option<&'a str>,
}

/// Detect if running inside Docker/Podman/k8s by checking filesystem markers,
/// environment variables, and `/proc/1/cgroup`.
pub fn detect_container_environment() -> ContainerEnvironment {
    let proc_1_cgroup = std::fs::read_to_string("/proc/1/cgroup").ok();
    detect_container_environment_from(ContainerDetectionInputs {
        env_pairs: std::env::vars().collect(),
        dockerenv_exists: std::path::Path::new("/.dockerenv").exists(),
        containerenv_exists: std::path::Path::new("/run/.containerenv").exists(),
        proc_1_cgroup: proc_1_cgroup.as_deref(),
    })
}

/// Testable variant that takes explicit inputs instead of reading the filesystem.
pub fn detect_container_environment_from(
    inputs: ContainerDetectionInputs<'_>,
) -> ContainerEnvironment {
    let mut markers = Vec::new();
    if inputs.dockerenv_exists {
        markers.push("/.dockerenv".to_string());
    }
    if inputs.containerenv_exists {
        markers.push("/run/.containerenv".to_string());
    }
    for (key, value) in inputs.env_pairs {
        let normalized = key.to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "container" | "docker" | "podman" | "kubernetes_service_host"
        ) && !value.is_empty()
        {
            markers.push(format!("env:{key}={value}"));
        }
    }
    if let Some(cgroup) = inputs.proc_1_cgroup {
        for needle in ["docker", "containerd", "kubepods", "podman", "libpod"] {
            if cgroup.contains(needle) {
                markers.push(format!("/proc/1/cgroup:{needle}"));
            }
        }
    }
    markers.sort();
    markers.dedup();
    ContainerEnvironment {
        in_container: !markers.is_empty(),
        markers,
    }
}

/// A bash command wrapped in Linux unshare namespaces for isolation.
pub struct LinuxSandboxCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// Build a Linux sandbox command using `unshare` namespaces.
/// Returns `None` if not on Linux, or if `unshare` is not found in PATH.
pub fn build_linux_sandbox_command(
    command: &str,
    cwd: &std::path::Path,
    use_network_isolation: bool,
) -> Option<LinuxSandboxCommand> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (command, cwd, use_network_isolation);
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        // Check that unshare is available in PATH
        let unshare_available = std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join("unshare").exists())
        });
        if !unshare_available {
            return None;
        }

        let mut args = vec![
            "--user".to_string(),
            "--map-root-user".to_string(),
            "--mount".to_string(),
            "--ipc".to_string(),
            "--pid".to_string(),
            "--uts".to_string(),
            "--fork".to_string(),
        ];
        if use_network_isolation {
            args.push("--net".to_string());
        }
        args.push("sh".to_string());
        args.push("-lc".to_string());
        args.push(command.to_string());

        let sandbox_home = cwd.join(".sandbox-home");
        let sandbox_tmp = cwd.join(".sandbox-tmp");
        let mut env = vec![
            ("HOME".to_string(), sandbox_home.display().to_string()),
            ("TMPDIR".to_string(), sandbox_tmp.display().to_string()),
        ];
        if let Ok(path) = std::env::var("PATH") {
            env.push(("PATH".to_string(), path));
        }

        Some(LinuxSandboxCommand {
            program: "unshare".to_string(),
            args,
            env,
        })
    }
}

#[cfg(test)]
mod container_sandbox_tests {
    use super::*;

    #[test]
    fn detects_container_from_dockerenv() {
        let detected = detect_container_environment_from(ContainerDetectionInputs {
            env_pairs: Vec::new(),
            dockerenv_exists: true,
            containerenv_exists: false,
            proc_1_cgroup: None,
        });
        assert!(detected.in_container);
        assert!(detected.markers.iter().any(|m| m == "/.dockerenv"));
    }

    #[test]
    fn build_sandbox_command_includes_network_flag_when_requested() {
        let cwd = std::path::Path::new("/workspace");
        let with_net = build_linux_sandbox_command("printf hi", cwd, true);
        let without_net = build_linux_sandbox_command("printf hi", cwd, false);

        // On non-Linux these will both be None; on Linux with unshare available they test flags.
        if let Some(cmd) = with_net {
            assert_eq!(cmd.program, "unshare");
            assert!(
                cmd.args.iter().any(|a| a == "--net"),
                "--net should be present"
            );
        }
        if let Some(cmd) = without_net {
            assert!(
                !cmd.args.iter().any(|a| a == "--net"),
                "--net should be absent"
            );
        }
    }
}

pub struct WasmSandbox {
    engine: Engine,
}

impl WasmSandbox {
    pub fn new() -> Result<Self, String> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    pub fn execute_module(
        &self,
        module_path: &Path,
        _args: &[String],
        _sandboxed: bool,
    ) -> Result<String, String> {
        let wasi: WasiP1Ctx = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build_p1();

        let mut store = Store::new(&self.engine, wasi);
        let module = Module::from_file(&self.engine, module_path)
            .map_err(|e| format!("failed to load module: {}", e))?;

        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |s| s)
            .map_err(|e| format!("failed to add wasi: {}", e))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("failed to instantiate: {}", e))?;

        let func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "main"))
            .map_err(|_| "module lacks _start or main".to_string())?;

        func.call(&mut store, ())
            .map_err(|e| format!("execution error: {}", e))?;

        Ok("WASM execution completed".to_string())
    }
}
