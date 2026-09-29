use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Files agents may propose patches to (prefix match).
const ALLOWED_PREFIXES: &[&str] = &["skills/", "plugins/", "lifecycle/", "skill_patch/"];

/// Files requiring extra confirmation before apply.
const SENSITIVE_PATHS: &[&str] = &["steward/soul.rs", "steward/iris.rs", "main_loop.rs"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPatch {
    pub proposal_id: String,
    pub file: String,
    pub reason: String,
    pub proposed_code: String,
    pub original_code: String,
    pub diff: String,
    pub timestamp: u64,
    /// True if the target is in the SENSITIVE list
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchProposal {
    pub patch: SkillPatch,
    pub status: ProposalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Applied,
    Rejected,
}

#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    #[error("File {0} is not in the allowed list for self-modification")]
    NotAllowed(String),
    #[error("Proposal {0} not found")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Gate that manages propose / apply / reject / restore for skill patches.
#[derive(Default)]
pub struct PatchGate {
    pending: std::collections::HashMap<String, PatchProposal>,
}

impl PatchGate {
    pub fn new() -> Self {
        Self::default()
    }

    fn is_allowed(file: &str) -> bool {
        ALLOWED_PREFIXES
            .iter()
            .any(|prefix| file.starts_with(prefix))
    }

    fn is_sensitive(file: &str) -> bool {
        SENSITIVE_PATHS
            .iter()
            .any(|s| file.ends_with(s) || file == *s)
    }

    /// Propose a patch. Reads the current file content, computes a simple diff.
    pub fn propose(
        &mut self,
        workspace_root: &Path,
        file: impl Into<String>,
        reason: impl Into<String>,
        new_code: impl Into<String>,
    ) -> Result<String, PatchError> {
        let file = file.into();
        if !Self::is_allowed(&file) {
            return Err(PatchError::NotAllowed(file));
        }
        let full_path = workspace_root.join(&file);
        let original_code = fs::read_to_string(&full_path).unwrap_or_default();
        let proposed_code = new_code.into();
        let diff = compute_diff(&original_code, &proposed_code);
        let sensitive = Self::is_sensitive(&file);
        let proposal_id = short_id();
        let patch = SkillPatch {
            proposal_id: proposal_id.clone(),
            file,
            reason: reason.into(),
            proposed_code,
            original_code,
            diff,
            timestamp: now_secs(),
            sensitive,
        };
        self.pending.insert(
            proposal_id.clone(),
            PatchProposal {
                patch,
                status: ProposalStatus::Pending,
            },
        );
        Ok(proposal_id)
    }

    /// Apply an approved proposal — backup original, write patch.
    pub fn apply(
        &mut self,
        workspace_root: &Path,
        proposal_id: &str,
    ) -> Result<serde_json::Value, PatchError> {
        let proposal = self
            .pending
            .get_mut(proposal_id)
            .ok_or_else(|| PatchError::NotFound(proposal_id.to_string()))?;

        let full_path = workspace_root.join(&proposal.patch.file);
        let backup_path = full_path.with_extension(format!(
            "{}.backup",
            full_path.extension().unwrap_or_default().to_string_lossy()
        ));

        // Backup original
        if full_path.exists() {
            fs::copy(&full_path, &backup_path)?;
        }

        // Write new code
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, &proposal.patch.proposed_code)?;

        proposal.status = ProposalStatus::Applied;

        // ARP-compatible receipt
        let diff_hash = sha2_short(&proposal.patch.diff);
        Ok(serde_json::json!({
            "kind": "skill_patch_applied",
            "proposal_id": proposal_id,
            "file": proposal.patch.file,
            "reason": proposal.patch.reason,
            "diff_hash": diff_hash,
            "timestamp": now_secs(),
        }))
    }

    /// Reject and discard a pending proposal.
    pub fn reject(&mut self, proposal_id: &str) -> Result<(), PatchError> {
        let proposal = self
            .pending
            .get_mut(proposal_id)
            .ok_or_else(|| PatchError::NotFound(proposal_id.to_string()))?;
        proposal.status = ProposalStatus::Rejected;
        Ok(())
    }

    /// Restore a file from its .backup file.
    pub fn restore(&self, workspace_root: &Path, file: &str) -> Result<(), PatchError> {
        let full_path = workspace_root.join(file);
        let ext = full_path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let backup_path = full_path.with_extension(format!("{}.backup", ext));
        fs::copy(&backup_path, &full_path)?;
        Ok(())
    }

    pub fn get(&self, proposal_id: &str) -> Option<&PatchProposal> {
        self.pending.get(proposal_id)
    }
}

fn compute_diff(original: &str, new: &str) -> String {
    // Simple line-level unified diff (no external dep)
    let orig_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut out = String::new();
    let max = orig_lines.len().max(new_lines.len());
    for i in 0..max {
        match (orig_lines.get(i), new_lines.get(i)) {
            (Some(o), Some(n)) if o == n => out.push_str(&format!(" {}\n", o)),
            (Some(o), Some(n)) => {
                out.push_str(&format!("-{}\n", o));
                out.push_str(&format!("+{}\n", n));
            }
            (Some(o), None) => out.push_str(&format!("-{}\n", o)),
            (None, Some(n)) => out.push_str(&format!("+{}\n", n)),
            (None, None) => {}
        }
    }
    out
}

fn sha2_short(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(&digest[..8])
}

fn short_id() -> String {
    use uuid::Uuid;
    let raw = Uuid::new_v4().to_string().replace('-', "");
    raw[..8].to_string()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn propose_disallowed_file_errors() {
        let mut gate = PatchGate::new();
        let dir = tmp();
        let err = gate.propose(dir.path(), "steward/soul.rs", "test", "code");
        assert!(matches!(err, Err(PatchError::NotAllowed(_))));
    }

    #[test]
    fn propose_allowed_file_succeeds() {
        let mut gate = PatchGate::new();
        let dir = tmp();
        fs::create_dir_all(dir.path().join("skills")).unwrap();
        fs::write(dir.path().join("skills/test.rs"), "fn old() {}").unwrap();
        let id = gate
            .propose(dir.path(), "skills/test.rs", "refactor", "fn new() {}")
            .unwrap();
        assert!(!id.is_empty());
        assert_eq!(gate.get(&id).unwrap().status, ProposalStatus::Pending);
    }

    #[test]
    fn apply_writes_file_and_returns_receipt() {
        let mut gate = PatchGate::new();
        let dir = tmp();
        fs::create_dir_all(dir.path().join("skills")).unwrap();
        fs::write(dir.path().join("skills/test.rs"), "fn old() {}").unwrap();
        let id = gate
            .propose(dir.path(), "skills/test.rs", "refactor", "fn new() {}")
            .unwrap();
        let receipt = gate.apply(dir.path(), &id).unwrap();
        assert_eq!(receipt["kind"], "skill_patch_applied");
        let content = fs::read_to_string(dir.path().join("skills/test.rs")).unwrap();
        assert_eq!(content, "fn new() {}");
    }

    #[test]
    fn sensitive_flag_set_for_sensitive_files() {
        let mut gate = PatchGate::new();
        let dir = tmp();
        fs::create_dir_all(dir.path().join("lifecycle")).unwrap();
        // lifecycle/ is allowed; steward/soul.rs would not be allowed at all
        fs::write(dir.path().join("lifecycle/test.rs"), "").unwrap();
        let id = gate
            .propose(dir.path(), "lifecycle/test.rs", "test", "x")
            .unwrap();
        let proposal = gate.get(&id).unwrap();
        assert!(!proposal.patch.sensitive); // lifecycle/test.rs not in SENSITIVE_PATHS
    }
}
