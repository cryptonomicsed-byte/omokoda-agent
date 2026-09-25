// workspace_seed.rs — Generate workspace seed files during onboarding.
//
// Writes AGENTS.md, SOUL.md, USER.md, RULES.md, and MEMORY.md into the agent's
// workspace directory (`~/.omokoda/workspace/`). Files are never overwritten —
// if a file already exists it is left untouched.

use crate::home::HomeDir;
use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Seed file definitions
// ---------------------------------------------------------------------------

const SEED_FILES: [&str; 5] = ["AGENTS.md", "SOUL.md", "USER.md", "RULES.md", "MEMORY.md"];

fn agents_md(agent_display_name: &str, agent_name: &str) -> String {
    format!(
        r#"# Agents

## {agent_display_name} ({agent_name})

Primary sovereign agent. Role: sovereign operator.

Add additional agents here as your ecosystem expands.
"#,
        agent_display_name = agent_display_name,
        agent_name = agent_name,
    )
}

fn soul_md(agent_display_name: &str) -> String {
    format!(
        r#"# Soul Context — {agent_display_name}

This file is observational context only.
The agent's true identity is sealed in ~/.omokoda/state/ori/ori.json
and derived deterministically from birth entropy.

## Notes

Add human-readable observations about this agent's character here.
"#,
        agent_display_name = agent_display_name,
    )
}

fn user_md() -> &'static str {
    r#"# Human Operator

Add context about yourself here. This helps the agent understand your preferences.

## Preferences
- Language: English
- Verbosity: concise

## Background
(Add your background here)
"#
}

fn rules_md() -> &'static str {
    r#"# Workspace Rules

These rules apply to this workspace only.
For constitutional policies (what the agent may/may not do), see ~/.omokoda/constitution/.

## Conventions
- Use clear, descriptive names
- Document non-obvious decisions

## Prohibited
- Do not commit secrets
- Do not run destructive operations without confirmation
"#
}

fn memory_md() -> &'static str {
    r#"# Memory Index

Auto-populated by the agent during sessions.
Do not edit manually.
"#
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Write workspace seed files into `home.workspace`.
///
/// Each file is written only if it does not already exist.
/// Returns `Ok(())` even if some files were skipped (already present).
pub fn seed_workspace(
    home: &HomeDir,
    agent_name: &str,
    agent_display_name: &str,
) -> std::io::Result<()> {
    let workspace = &home.workspace;

    // Ensure the workspace directory exists (HomeDir::init() should have done
    // this, but we guard here so seed_workspace is self-contained).
    fs::create_dir_all(workspace)?;

    let files: [(&str, String); 5] = [
        ("AGENTS.md", agents_md(agent_display_name, agent_name)),
        ("SOUL.md", soul_md(agent_display_name)),
        ("USER.md", user_md().to_string()),
        ("RULES.md", rules_md().to_string()),
        ("MEMORY.md", memory_md().to_string()),
    ];

    for (filename, content) in &files {
        let path: PathBuf = workspace.join(filename);
        if !path.exists() {
            fs::write(&path, content.as_bytes())?;
        }
    }

    Ok(())
}

/// Return the existence status of each seed file.
///
/// Used by `omokoda doctor` to report workspace health.
/// Each element is `(filename, exists)`.
pub fn workspace_seed_status(home: &HomeDir) -> Vec<(String, bool)> {
    SEED_FILES
        .iter()
        .map(|name| {
            let exists = home.workspace.join(name).exists();
            (name.to_string(), exists)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::HomeDir;

    fn tmp_home() -> (tempfile::TempDir, HomeDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = HomeDir::from_root(dir.path().join(".omokoda"));
        home.init().expect("init");
        (dir, home)
    }

    #[test]
    fn seed_creates_all_files() {
        let (_dir, home) = tmp_home();
        seed_workspace(&home, "hermes", "Hermes").expect("seed");

        for name in &SEED_FILES {
            let path = home.workspace.join(name);
            assert!(path.exists(), "missing seed file: {name}");
        }
    }

    #[test]
    fn seed_does_not_overwrite() {
        let (_dir, home) = tmp_home();
        let agents_path = home.workspace.join("AGENTS.md");
        fs::write(&agents_path, b"custom content").unwrap();

        seed_workspace(&home, "hermes", "Hermes").expect("seed");

        let content = fs::read_to_string(&agents_path).unwrap();
        assert_eq!(content, "custom content", "seed must not overwrite existing file");
    }

    #[test]
    fn seed_is_idempotent() {
        let (_dir, home) = tmp_home();
        seed_workspace(&home, "hermes", "Hermes").expect("first seed");
        seed_workspace(&home, "hermes", "Hermes").expect("second seed should not fail");
    }

    #[test]
    fn status_all_missing_before_seed() {
        let (_dir, home) = tmp_home();
        let status = workspace_seed_status(&home);
        assert_eq!(status.len(), 5);
        for (name, exists) in &status {
            assert!(!exists, "{name} should not exist before seeding");
        }
    }

    #[test]
    fn status_all_present_after_seed() {
        let (_dir, home) = tmp_home();
        seed_workspace(&home, "hermes", "Hermes").expect("seed");
        let status = workspace_seed_status(&home);
        for (name, exists) in &status {
            assert!(exists, "{name} should exist after seeding");
        }
    }

    #[test]
    fn agents_md_contains_agent_name() {
        let (_dir, home) = tmp_home();
        seed_workspace(&home, "my-agent", "My Agent").expect("seed");
        let content = fs::read_to_string(home.workspace.join("AGENTS.md")).unwrap();
        assert!(content.contains("My Agent"), "display name missing");
        assert!(content.contains("my-agent"), "agent name missing");
    }

    #[test]
    fn soul_md_contains_display_name() {
        let (_dir, home) = tmp_home();
        seed_workspace(&home, "x", "Phoenix").expect("seed");
        let content = fs::read_to_string(home.workspace.join("SOUL.md")).unwrap();
        assert!(content.contains("Phoenix"), "display name missing from SOUL.md");
    }

    #[test]
    fn status_returns_correct_filenames() {
        let (_dir, home) = tmp_home();
        let status = workspace_seed_status(&home);
        let names: Vec<&str> = status.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["AGENTS.md", "SOUL.md", "USER.md", "RULES.md", "MEMORY.md"]);
    }
}
