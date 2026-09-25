// home.rs — ~/.omokoda/ canonical home directory structure.
// Provides typed path accessors and idempotent directory initialisation.

use std::fs;
use std::path::PathBuf;

/// Canonical layout of the agent's home directory at `~/.omokoda/`.
///
/// Call `HomeDir::new()` to build paths, then `init()` to create them.
/// All methods are pure path arithmetic — no I/O except `init()`.
#[derive(Debug, Clone)]
pub struct HomeDir {
    /// Root: `~/.omokoda/`
    pub root: PathBuf,
    /// Identity data: `~/.omokoda/identity/`
    pub identity: PathBuf,
    /// Constitutional policies: `~/.omokoda/constitution/`
    pub constitution: PathBuf,
    /// Active workspace: `~/.omokoda/workspace/`
    pub workspace: PathBuf,
    /// Encrypted sessions: `~/.omokoda/sessions/`
    pub sessions: PathBuf,
    /// Runtime state (Ori, receipts index, etc.): `~/.omokoda/state/`
    pub state: PathBuf,
    /// Log files: `~/.omokoda/logs/`
    pub logs: PathBuf,
    /// Hook scripts: `~/.omokoda/hooks/`
    pub hooks: PathBuf,
    /// Local receipt store: `~/.omokoda/receipts/`
    pub receipts: PathBuf,
}

impl HomeDir {
    /// Build all paths relative to `$HOME/.omokoda/`.
    /// Falls back to `./.omokoda/` if `$HOME` is unset.
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::from_root(home.join(".omokoda"))
    }

    /// Build all paths relative to an explicit `root`.
    /// Useful for testing with a temp directory.
    pub fn from_root(root: PathBuf) -> Self {
        HomeDir {
            identity: root.join("identity"),
            constitution: root.join("constitution"),
            workspace: root.join("workspace"),
            sessions: root.join("sessions"),
            state: root.join("state"),
            logs: root.join("logs"),
            hooks: root.join("hooks"),
            receipts: root.join("receipts"),
            root,
        }
    }

    /// Idempotently create the full directory tree under `root`.
    ///
    /// Safe to call on every startup — existing directories are left untouched.
    pub fn init(&self) -> std::io::Result<()> {
        let dirs = self.all_dirs();
        for dir in &dirs {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// All directories that `init()` will create.
    pub fn all_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.root.clone(),
            // identity subtree
            self.identity.clone(),
            self.identity.join("keys"),
            self.identity.join("nostr"),
            self.identity.join("sui"),
            // constitution subtree
            self.constitution.clone(),
            self.constitution.join("hermetic"),
            self.constitution.join("permissions"),
            self.constitution.join("vessels"),
            // workspace
            self.workspace.clone(),
            self.workspace.join("scratch"),
            // sessions
            self.sessions.clone(),
            self.sessions.join("active"),
            self.sessions.join("archive"),
            // state subtree
            self.state.clone(),
            self.state.join("ori"),
            self.state.join("memory"),
            self.state.join("gix"),
            self.state.join("vantage"),
            // logs
            self.logs.clone(),
            self.logs.join("turns"),
            self.logs.join("receipts"),
            self.logs.join("errors"),
            // hooks
            self.hooks.clone(),
            self.hooks.join("pre-tool"),
            self.hooks.join("post-tool"),
            self.hooks.join("on-birth"),
            self.hooks.join("on-act"),
            // receipts
            self.receipts.clone(),
            self.receipts.join("acts"),
            self.receipts.join("births"),
        ]
    }

    // ── Typed file path helpers ───────────────────────────────────────────────

    /// `~/.omokoda/state/ori/ori.json` — current Ori state
    pub fn ori_json(&self) -> PathBuf {
        self.state.join("ori").join("ori.json")
    }

    /// `~/.omokoda/identity/identity.json` — agent identity record
    pub fn identity_json(&self) -> PathBuf {
        self.identity.join("identity.json")
    }

    /// `~/.omokoda/identity/birth.json` — immutable birth record
    pub fn birth_json(&self) -> PathBuf {
        self.identity.join("birth.json")
    }

    /// `~/.omokoda/constitution/constitution.json` — active constitution
    pub fn constitution_json(&self) -> PathBuf {
        self.constitution.join("constitution.json")
    }

    /// `~/.omokoda/constitution/hermetic/hermetic.toml`
    pub fn hermetic_toml(&self) -> PathBuf {
        self.constitution.join("hermetic").join("hermetic.toml")
    }

    /// `~/.omokoda/constitution/permissions/permissions.toml`
    pub fn permissions_toml(&self) -> PathBuf {
        self.constitution.join("permissions").join("permissions.toml")
    }

    /// `~/.omokoda/config.toml` — operator configuration (OmokodaConfig)
    pub fn config_toml(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// `~/.omokoda/BOOTSTRAP.md` — first-run onboarding artifact
    pub fn bootstrap_md(&self) -> PathBuf {
        self.root.join("BOOTSTRAP.md")
    }

    /// `~/.omokoda/receipts/births/birth.receipt.json` — sealed birth receipt
    pub fn birth_receipt(&self) -> PathBuf {
        self.receipts.join("births").join("birth.receipt.json")
    }
}

impl Default for HomeDir {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_home() -> (tempfile::TempDir, HomeDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = HomeDir::from_root(dir.path().join(".omokoda"));
        (dir, home)
    }

    #[test]
    fn init_creates_all_dirs() {
        let (_dir, home) = tmp_home();
        home.init().expect("init failed");
        for d in home.all_dirs() {
            assert!(d.exists(), "missing dir: {}", d.display());
        }
    }

    #[test]
    fn init_is_idempotent() {
        let (_dir, home) = tmp_home();
        home.init().expect("first init");
        home.init().expect("second init should not fail");
    }

    #[test]
    fn typed_paths_are_under_root() {
        let (_dir, home) = tmp_home();
        assert!(home.ori_json().starts_with(&home.root));
        assert!(home.identity_json().starts_with(&home.root));
        assert!(home.birth_json().starts_with(&home.root));
        assert!(home.constitution_json().starts_with(&home.root));
        assert!(home.hermetic_toml().starts_with(&home.root));
        assert!(home.permissions_toml().starts_with(&home.root));
        assert!(home.config_toml().starts_with(&home.root));
        assert!(home.bootstrap_md().starts_with(&home.root));
        assert!(home.birth_receipt().starts_with(&home.root));
    }

    #[test]
    fn config_toml_path_is_direct_child_of_root() {
        let (_dir, home) = tmp_home();
        assert_eq!(
            home.config_toml().parent().unwrap(),
            home.root.as_path()
        );
    }

    #[test]
    fn from_root_explicit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("myagent");
        let home = HomeDir::from_root(root.clone());
        assert_eq!(home.root, root);
        assert_eq!(home.identity, root.join("identity"));
    }

    #[test]
    fn all_dirs_count_stable() {
        let (_dir, home) = tmp_home();
        // Regression guard: 31 dirs defined. Update this number if dirs are added.
        assert_eq!(home.all_dirs().len(), 31);
    }

    #[test]
    fn init_allows_writing_files_into_dirs() {
        let (_dir, home) = tmp_home();
        home.init().expect("init");
        fs::write(home.config_toml(), b"[agent]\nname=\"test\"").expect("write config");
        assert!(home.config_toml().exists());
    }
}
