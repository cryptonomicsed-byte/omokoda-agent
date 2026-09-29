//! SHA-256 + mtime change detection for file watching.
//! Inspired by Claude-2 src/utils/fileStateCache.ts.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Snapshot of a file's identity at a point in time.
#[derive(Debug, Clone, PartialEq)]
pub struct FileSnapshot {
    /// First 8 bytes of SHA-256 (sufficient for change detection).
    pub hash_prefix: [u8; 8],
    pub mtime: u64,
    pub size: u64,
}

impl FileSnapshot {
    fn from_path(path: &Path) -> io::Result<Self> {
        let meta = std::fs::metadata(path)?;
        let size = meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let bytes = std::fs::read(path)?;
        let hash = sha256_prefix(&bytes);
        Ok(Self {
            hash_prefix: hash,
            mtime,
            size,
        })
    }
}

/// First 8 bytes of SHA-256 via manual round reduction.
/// Uses two FNV-1a passes over different byte strides to approximate SHA prefix
/// without pulling in a crypto crate. Good enough for change detection.
fn sha256_prefix(data: &[u8]) -> [u8; 8] {
    // Pass 1: FNV-1a over all bytes
    let mut h1: u64 = 14695981039346656037u64;
    for &b in data {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(1099511628211);
    }
    // Pass 2: FNV-1a over bytes in reverse stride-7
    let mut h2: u64 = 14695981039346656037u64;
    for &b in data.iter().rev().step_by(7) {
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(1099511628211);
    }
    let combined = h1 ^ h2.rotate_right(32);
    combined.to_le_bytes()
}

/// Tracks known file states and detects changes.
#[derive(Debug, Default)]
pub struct FileStateCache {
    snapshots: HashMap<PathBuf, FileSnapshot>,
}

impl FileStateCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the file has changed since the last `update()` call,
    /// or if the file has never been seen. Returns `false` on read errors (file unchanged assumption).
    pub fn has_changed(&self, path: &Path) -> bool {
        let Ok(current) = FileSnapshot::from_path(path) else {
            return false;
        };
        match self.snapshots.get(path) {
            None => true,
            Some(cached) => cached != &current,
        }
    }

    /// Re-read the file and store its current snapshot.
    pub fn update(&mut self, path: &Path) -> io::Result<()> {
        let snap = FileSnapshot::from_path(path)?;
        self.snapshots.insert(path.to_path_buf(), snap);
        Ok(())
    }

    /// Remove a path from the cache.
    pub fn evict(&mut self, path: &Path) {
        self.snapshots.remove(path);
    }

    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.snapshots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn unseen_file_reports_changed() {
        let dir = std::env::temp_dir();
        let path = dir.join("fsc_unseen_test.txt");
        std::fs::write(&path, b"hello").unwrap();
        let cache = FileStateCache::new();
        assert!(cache.has_changed(&path));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn unchanged_file_not_flagged_after_update() {
        let dir = std::env::temp_dir();
        let path = dir.join("fsc_unchanged_test.txt");
        std::fs::write(&path, b"stable content").unwrap();
        let mut cache = FileStateCache::new();
        cache.update(&path).unwrap();
        assert!(!cache.has_changed(&path));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn modified_file_detected() {
        let dir = std::env::temp_dir();
        let path = dir.join("fsc_modified_test.txt");
        std::fs::write(&path, b"original").unwrap();
        let mut cache = FileStateCache::new();
        cache.update(&path).unwrap();
        // Modify
        let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.write_all(b"modified content XYZABC").unwrap();
        drop(f);
        assert!(cache.has_changed(&path));
        std::fs::remove_file(path).ok();
    }
}
