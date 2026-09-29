use std::collections::HashMap;
use std::sync::RwLock;

/// Identity-aware virtual filesystem for Omo-Koda2.
///
/// Mounts:
///   /agents/{agent_id}/memory/    — private agent memory
///   /agents/{agent_id}/vault/     — encrypted secrets
///   /agents/{agent_id}/receipts/  — Zàngbétò receipt chain
///   /twins/{twin_id}/             — digital twin state
///   /evidence/{receipt_id}        — immutable evidence blobs
///   /devices/                     — device tree (read-only mirror)
///   /shared/                      — agent-to-agent shared space
pub struct SovereignFS {
    nodes: RwLock<HashMap<String, FsNode>>,
}

#[derive(Debug, Clone)]
pub enum FsNodeKind {
    Directory,
    File { content: Vec<u8> },
    Symlink { target: String },
}

#[derive(Debug, Clone)]
pub struct FsNode {
    pub path: String,
    pub kind: FsNodeKind,
    pub owner: String,            // agent_id
    pub readable_by: Vec<String>, // agent_ids; empty = owner only
    pub created_at: u64,
    pub modified_at: u64,
}

impl FsNode {
    fn dir(path: impl Into<String>, owner: impl Into<String>) -> Self {
        let now = now_secs();
        Self {
            path: path.into(),
            kind: FsNodeKind::Directory,
            owner: owner.into(),
            readable_by: vec![],
            created_at: now,
            modified_at: now,
        }
    }

    fn file(path: impl Into<String>, owner: impl Into<String>, content: Vec<u8>) -> Self {
        let now = now_secs();
        Self {
            path: path.into(),
            kind: FsNodeKind::File { content },
            owner: owner.into(),
            readable_by: vec![],
            created_at: now,
            modified_at: now,
        }
    }
}

impl SovereignFS {
    pub fn new() -> Self {
        let fs = Self {
            nodes: RwLock::new(HashMap::new()),
        };
        // Bootstrap permanent mount points
        for root in ["/agents", "/twins", "/evidence", "/devices", "/shared"] {
            fs.nodes
                .write()
                .unwrap()
                .insert(root.to_string(), FsNode::dir(root, "kernel"));
        }
        fs
    }

    /// Materialise the canonical agent subtree at first boot or agent spawn.
    pub fn init_agent(&self, agent_id: &str) {
        let base = format!("/agents/{agent_id}");
        for subdir in ["memory", "vault", "receipts", "skills", "daemons"] {
            let path = format!("{base}/{subdir}");
            self.nodes
                .write()
                .unwrap()
                .entry(path.clone())
                .or_insert_with(|| FsNode::dir(path, agent_id));
        }
    }

    pub fn write(&self, path: &str, owner: &str, content: Vec<u8>) -> Result<(), FsError> {
        self.assert_ownership(path, owner)?;
        let mut nodes = self.nodes.write().unwrap();
        let node = nodes
            .entry(path.to_string())
            .or_insert_with(|| FsNode::file(path, owner, vec![]));
        node.kind = FsNodeKind::File { content };
        node.modified_at = now_secs();
        Ok(())
    }

    pub fn read(&self, path: &str, requestor: &str) -> Result<Vec<u8>, FsError> {
        let nodes = self.nodes.read().unwrap();
        let node = nodes.get(path).ok_or(FsError::NotFound)?;
        if node.owner != requestor && !node.readable_by.contains(&requestor.to_string()) {
            return Err(FsError::PermissionDenied);
        }
        match &node.kind {
            FsNodeKind::File { content } => Ok(content.clone()),
            _ => Err(FsError::NotAFile),
        }
    }

    pub fn mkdir(&self, path: &str, owner: &str) {
        self.nodes
            .write()
            .unwrap()
            .entry(path.to_string())
            .or_insert_with(|| FsNode::dir(path, owner));
    }

    pub fn list(&self, dir: &str, requestor: &str) -> Vec<String> {
        let nodes = self.nodes.read().unwrap();
        let prefix = if dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{dir}/")
        };
        nodes
            .keys()
            .filter(|k| {
                k.starts_with(&prefix)
                    && !k[prefix.len()..].contains('/')
                    && nodes
                        .get(*k)
                        .map(|n| {
                            n.owner == requestor || n.readable_by.contains(&requestor.to_string())
                        })
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    pub fn grant_read(&self, path: &str, owner: &str, grantee: &str) -> Result<(), FsError> {
        self.assert_ownership(path, owner)?;
        let mut nodes = self.nodes.write().unwrap();
        if let Some(n) = nodes.get_mut(path) {
            if !n.readable_by.contains(&grantee.to_string()) {
                n.readable_by.push(grantee.to_string());
            }
            Ok(())
        } else {
            Err(FsError::NotFound)
        }
    }

    pub fn delete(&self, path: &str, owner: &str) -> Result<(), FsError> {
        self.assert_ownership(path, owner)?;
        self.nodes.write().unwrap().remove(path);
        Ok(())
    }

    fn assert_ownership(&self, path: &str, requestor: &str) -> Result<(), FsError> {
        let nodes = self.nodes.read().unwrap();
        if let Some(n) = nodes.get(path) {
            if n.owner != requestor {
                return Err(FsError::PermissionDenied);
            }
        }
        Ok(())
    }

    /// Canonical path helpers
    pub fn agent_memory_path(agent_id: &str, key: &str) -> String {
        format!("/agents/{agent_id}/memory/{key}")
    }

    pub fn agent_receipt_path(agent_id: &str, receipt_id: &str) -> String {
        format!("/agents/{agent_id}/receipts/{receipt_id}")
    }

    pub fn twin_path(twin_id: &str) -> String {
        format!("/twins/{twin_id}")
    }

    pub fn evidence_path(receipt_id: &str) -> String {
        format!("/evidence/{receipt_id}")
    }
}

impl Default for SovereignFS {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("path not found")]
    NotFound,
    #[error("permission denied")]
    PermissionDenied,
    #[error("target is not a file")]
    NotAFile,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
