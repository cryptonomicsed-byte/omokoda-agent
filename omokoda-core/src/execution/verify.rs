// Verify Assertions Engine — executable post-condition checks.
//
// The Calabash corpus declares a `Verify:` block for each directive.
// This engine evaluates those assertions after tool execution.
// Every assertion type returns AssertionResult (Pass/Fail with evidence).

use std::path::Path;
use serde::{Deserialize, Serialize};

/// The outcome of a single assertion evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssertionResult {
    Pass { name: String, evidence: String },
    Fail { name: String, reason: String },
}

impl AssertionResult {
    pub fn name(&self) -> &str {
        match self {
            Self::Pass { name, .. } | Self::Fail { name, .. } => name,
        }
    }
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass { .. })
    }
}

/// All supported assertion types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Assertion {
    /// A path must exist on the filesystem.
    FileExists { path: String },
    /// A file must contain a specific string.
    FileContains { path: String, expected: String },
    /// A file must match a given SHA-256 hex digest.
    HashMatch { path: String, expected_sha256: String },
    /// A git repository at `repo_path` must be at a clean commit (no dirty working tree).
    GitAtomicCommit { repo_path: String },
    /// An arbitrary key-value must be present in a JSON file.
    JsonFieldEquals { path: String, key: String, expected: String },
    /// The most recent process exit code must equal `expected`.
    ExitCode { expected: i32, actual: i32 },
}

impl Assertion {
    /// Evaluate this assertion and return a named result.
    pub fn evaluate(&self) -> AssertionResult {
        match self {
            Assertion::FileExists { path } => {
                let name = format!("file_exists:{}", path);
                if Path::new(path).exists() {
                    AssertionResult::Pass { name, evidence: format!("{path} exists") }
                } else {
                    AssertionResult::Fail { name, reason: format!("{path} not found") }
                }
            }

            Assertion::FileContains { path, expected } => {
                let name = format!("file_contains:{path}");
                match std::fs::read_to_string(path) {
                    Ok(content) if content.contains(expected.as_str()) => {
                        AssertionResult::Pass { name, evidence: format!("{path} contains expected string") }
                    }
                    Ok(_) => AssertionResult::Fail {
                        name,
                        reason: format!("{path} does not contain: {expected}"),
                    },
                    Err(e) => AssertionResult::Fail {
                        name,
                        reason: format!("could not read {path}: {e}"),
                    },
                }
            }

            Assertion::HashMatch { path, expected_sha256 } => {
                let name = format!("hash_match:{path}");
                match std::fs::read(path) {
                    Ok(bytes) => {
                        let digest = sha256_hex(&bytes);
                        if digest == *expected_sha256 {
                            AssertionResult::Pass { name, evidence: format!("sha256 matched: {digest}") }
                        } else {
                            AssertionResult::Fail {
                                name,
                                reason: format!("sha256 mismatch: got {digest}, expected {expected_sha256}"),
                            }
                        }
                    }
                    Err(e) => AssertionResult::Fail {
                        name,
                        reason: format!("could not read {path}: {e}"),
                    },
                }
            }

            Assertion::GitAtomicCommit { repo_path } => {
                let name = format!("git_atomic_commit:{repo_path}");
                let output = std::process::Command::new("git")
                    .args(["-C", repo_path, "status", "--porcelain"])
                    .output();
                match output {
                    Ok(o) if o.status.success() => {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        if stdout.trim().is_empty() {
                            AssertionResult::Pass {
                                name,
                                evidence: "working tree is clean".to_string(),
                            }
                        } else {
                            AssertionResult::Fail {
                                name,
                                reason: format!("dirty working tree:\n{}", stdout.trim()),
                            }
                        }
                    }
                    Ok(o) => AssertionResult::Fail {
                        name,
                        reason: format!("git status failed: {}", String::from_utf8_lossy(&o.stderr)),
                    },
                    Err(e) => AssertionResult::Fail {
                        name,
                        reason: format!("could not run git: {e}"),
                    },
                }
            }

            Assertion::JsonFieldEquals { path, key, expected } => {
                let name = format!("json_field:{path}:{key}");
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        match serde_json::from_str::<serde_json::Value>(&content) {
                            Ok(json) => {
                                // Support dotted key paths: "a.b.c"
                                let mut cursor = &json;
                                for part in key.split('.') {
                                    match cursor.get(part) {
                                        Some(v) => cursor = v,
                                        None => {
                                            return AssertionResult::Fail {
                                                name,
                                                reason: format!("key '{key}' not found in {path}"),
                                            };
                                        }
                                    }
                                }
                                let actual = match cursor {
                                    serde_json::Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                };
                                if actual == *expected {
                                    AssertionResult::Pass { name, evidence: format!("{key}={actual}") }
                                } else {
                                    AssertionResult::Fail {
                                        name,
                                        reason: format!("{key}: expected '{expected}', got '{actual}'"),
                                    }
                                }
                            }
                            Err(e) => AssertionResult::Fail {
                                name,
                                reason: format!("invalid JSON in {path}: {e}"),
                            },
                        }
                    }
                    Err(e) => AssertionResult::Fail {
                        name,
                        reason: format!("could not read {path}: {e}"),
                    },
                }
            }

            Assertion::ExitCode { expected, actual } => {
                let name = format!("exit_code:{expected}");
                if actual == expected {
                    AssertionResult::Pass { name, evidence: format!("exit code {actual}") }
                } else {
                    AssertionResult::Fail {
                        name,
                        reason: format!("expected exit code {expected}, got {actual}"),
                    }
                }
            }
        }
    }
}

/// Run a list of assertions and return all results.
pub fn run_assertions(assertions: &[Assertion]) -> Vec<AssertionResult> {
    assertions.iter().map(|a| a.evaluate()).collect()
}

/// Returns true only if all assertions pass.
pub fn all_pass(results: &[AssertionResult]) -> bool {
    results.iter().all(|r| r.is_pass())
}

fn sha256_hex(bytes: &[u8]) -> String {
    // Simple SHA-256 without external crate dependency — uses std + ring if available,
    // falls back to a placeholder that still exercises the contract in tests.
    // Production deployments wire this to the sha2 crate via the existing Cargo.toml.
    #[cfg(feature = "sha2")]
    {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }
    #[cfg(not(feature = "sha2"))]
    {
        // Fallback: deterministic but NOT cryptographic — for test scaffolding only.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{:016x}{:016x}{:016x}{:016x}", h, h ^ 0xdead, h ^ 0xbeef, h ^ 0xcafe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn file_exists_passes_for_existing_file() {
        let f = NamedTempFile::new().unwrap();
        let a = Assertion::FileExists { path: f.path().to_string_lossy().to_string() };
        assert!(a.evaluate().is_pass());
    }

    #[test]
    fn file_exists_fails_for_missing_file() {
        let a = Assertion::FileExists { path: "/tmp/no-such-file-omokoda-test".to_string() };
        assert!(!a.evaluate().is_pass());
    }

    #[test]
    fn file_contains_passes_when_string_present() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "hello sovereign world").unwrap();
        let a = Assertion::FileContains {
            path: f.path().to_string_lossy().to_string(),
            expected: "sovereign".to_string(),
        };
        assert!(a.evaluate().is_pass());
    }

    #[test]
    fn file_contains_fails_when_string_absent() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "hello world").unwrap();
        let a = Assertion::FileContains {
            path: f.path().to_string_lossy().to_string(),
            expected: "sovereign".to_string(),
        };
        assert!(!a.evaluate().is_pass());
    }

    #[test]
    fn exit_code_assertion() {
        assert!(Assertion::ExitCode { expected: 0, actual: 0 }.evaluate().is_pass());
        assert!(!Assertion::ExitCode { expected: 0, actual: 1 }.evaluate().is_pass());
    }

    #[test]
    fn run_assertions_all_pass() {
        let f = NamedTempFile::new().unwrap();
        let assertions = vec![
            Assertion::FileExists { path: f.path().to_string_lossy().to_string() },
            Assertion::ExitCode { expected: 0, actual: 0 },
        ];
        let results = run_assertions(&assertions);
        assert!(all_pass(&results));
    }

    #[test]
    fn json_field_equals_pass() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"name": "omokoda", "tier": 1}}"#).unwrap();
        let a = Assertion::JsonFieldEquals {
            path: f.path().to_string_lossy().to_string(),
            key: "name".to_string(),
            expected: "omokoda".to_string(),
        };
        assert!(a.evaluate().is_pass());
    }
}
