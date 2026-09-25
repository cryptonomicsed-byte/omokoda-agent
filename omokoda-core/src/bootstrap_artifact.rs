// bootstrap_artifact.rs — BOOTSTRAP.md generation and archive lifecycle.
// BOOTSTRAP.md is a first-run artifact generated at birth.
// It is archived (renamed to BOOTSTRAP.{timestamp}.md) after the first
// successful session, so it is never regenerated unless the agent re-runs onboard.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::home::HomeDir;
use crate::ori::{Ori, VESSEL_NAMES};

/// Generate the full content of BOOTSTRAP.md as a String.
///
/// Sections:
///   - `# BOOTSTRAP — {agent_name}` heading
///   - Generated timestamp line
///   - Agent identity summary (name, ID, first 16 chars of birth_entropy_hash)
///   - Ori state summary (revision, vessel count, state_hash first 16 chars)
///   - `## Getting Started` section with quick-start commands
///   - `## Vessel Map` section showing all 16 vessels as a table
///   - `## Next Steps` with three recommended actions
pub fn generate_bootstrap_md(
    agent_name: &str,
    agent_id: &str,
    ori: &Ori,
    ifascript_version: &str,
) -> String {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let birth_hash_preview = if ori.birth_entropy_hash.len() >= 16 {
        &ori.birth_entropy_hash[..16]
    } else {
        ori.birth_entropy_hash.as_str()
    };

    let state_hash_preview = if ori.state_hash.len() >= 16 {
        &ori.state_hash[..16]
    } else {
        ori.state_hash.as_str()
    };

    let vessel_display = ori.vessel_display();

    let vessel_rows: String = vessel_display
        .iter()
        .enumerate()
        .map(|(i, (name, bars))| {
            format!(
                "| {:>2} | {:<11} | {} | {:.3} |\n",
                i,
                name,
                bars,
                ori.vessel_weights[i]
            )
        })
        .collect();

    format!(
        r#"# BOOTSTRAP — {agent_name}

> Generated at unix timestamp: {now_secs}
> IfáScript version: {ifascript_version}

---

## Agent Identity

| Field              | Value                              |
|--------------------|------------------------------------|
| Name               | {agent_name}                       |
| Agent ID           | {agent_id}                         |
| Birth Entropy Hash | `{birth_hash_preview}...`          |

---

## Ori State

| Field              | Value                              |
|--------------------|------------------------------------|
| Revision           | {ori_revision}                     |
| Vessel Count       | {vessel_count}                     |
| Experience Count   | {experience_count}                 |
| State Hash         | `{state_hash_preview}...`          |

---

## Getting Started

```sh
# Show full agent status
omokoda doctor

# Begin a reasoning session
omokoda think

# List available tools
omokoda tools list

# View current constitution
omokoda constitution show
```

---

## Vessel Map

| Idx | Name        | Weight      | Raw   |
|-----|-------------|-------------|-------|
{vessel_rows}
---

## Next Steps

1. Run `omokoda doctor` — verify all systems are initialised and no gaps exist.
2. Run `omokoda think` — start your first reasoning session; this file will be
   archived automatically on first successful think turn.
3. Review `constitution/` — inspect your hermetic gates, permissions, and
   constitutional policy before accepting external tasks.
"#,
        agent_name = agent_name,
        now_secs = now_secs,
        ifascript_version = ifascript_version,
        agent_id = agent_id,
        birth_hash_preview = birth_hash_preview,
        ori_revision = ori.ori_revision,
        vessel_count = VESSEL_NAMES.len(),
        experience_count = ori.experience_count,
        state_hash_preview = state_hash_preview,
        vessel_rows = vessel_rows,
    )
}

/// Write BOOTSTRAP.md to `home.bootstrap_md()`.
///
/// Only writes if the file does NOT already exist — never overwrites an
/// existing BOOTSTRAP.md (including archived variants).
pub fn write_bootstrap_md(
    home: &HomeDir,
    agent_name: &str,
    agent_id: &str,
    ori: &Ori,
    ifascript_version: &str,
) -> std::io::Result<()> {
    let path = home.bootstrap_md();
    if path.exists() {
        return Ok(());
    }
    let content = generate_bootstrap_md(agent_name, agent_id, ori, ifascript_version);
    std::fs::write(&path, content)?;
    Ok(())
}

/// Archive BOOTSTRAP.md by renaming it to `BOOTSTRAP.{unix_timestamp}.md`
/// in the same directory.
///
/// Returns `Ok(Some(new_path))` if the file was archived, or `Ok(None)` if
/// BOOTSTRAP.md was not present.
pub fn archive_bootstrap_md(home: &HomeDir) -> std::io::Result<Option<PathBuf>> {
    let src = home.bootstrap_md();
    if !src.exists() {
        return Ok(None);
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let archive_name = format!("BOOTSTRAP.{}.md", timestamp);
    let dest = home.root.join(archive_name);
    std::fs::rename(&src, &dest)?;
    Ok(Some(dest))
}

/// Returns `true` if any `BOOTSTRAP.*.md` archive file exists in `home.root`.
///
/// Uses a simple `read_dir` scan — does not require a glob crate.
pub fn bootstrap_is_archived(home: &HomeDir) -> bool {
    let Ok(entries) = std::fs::read_dir(&home.root) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Match BOOTSTRAP.<something>.md but NOT BOOTSTRAP.md itself
        if name_str.starts_with("BOOTSTRAP.")
            && name_str.ends_with(".md")
            && name_str != "BOOTSTRAP.md"
        {
            return true;
        }
    }
    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ori::Ori;

    fn sample_ori() -> Ori {
        Ori::genesis(b"test-bootstrap-entropy", "0.1.0")
    }

    fn tmp_home() -> (tempfile::TempDir, HomeDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = HomeDir::from_root(dir.path().join(".omokoda"));
        home.init().expect("init");
        (dir, home)
    }

    #[test]
    fn generate_contains_agent_name() {
        let ori = sample_ori();
        let md = generate_bootstrap_md("TestAgent", "agent-001", &ori, "0.1.0");
        assert!(md.contains("TestAgent"), "missing agent name");
        assert!(md.contains("agent-001"), "missing agent id");
        assert!(md.contains("## Vessel Map"), "missing vessel map section");
        assert!(md.contains("## Next Steps"), "missing next steps section");
        assert!(md.contains("## Getting Started"), "missing getting started");
    }

    #[test]
    fn generate_contains_all_16_vessels() {
        let ori = sample_ori();
        let md = generate_bootstrap_md("TestAgent", "agent-001", &ori, "0.1.0");
        for name in VESSEL_NAMES.iter() {
            assert!(md.contains(name), "missing vessel: {name}");
        }
    }

    #[test]
    fn write_bootstrap_md_creates_file() {
        let (_dir, home) = tmp_home();
        let ori = sample_ori();
        write_bootstrap_md(&home, "TestAgent", "agent-001", &ori, "0.1.0")
            .expect("write should succeed");
        assert!(home.bootstrap_md().exists(), "BOOTSTRAP.md not created");
    }

    #[test]
    fn write_bootstrap_md_does_not_overwrite() {
        let (_dir, home) = tmp_home();
        let ori = sample_ori();
        write_bootstrap_md(&home, "First", "agent-001", &ori, "0.1.0").unwrap();
        let first_content = std::fs::read_to_string(home.bootstrap_md()).unwrap();
        // Write again with different name — should be a no-op
        write_bootstrap_md(&home, "Second", "agent-001", &ori, "0.1.0").unwrap();
        let second_content = std::fs::read_to_string(home.bootstrap_md()).unwrap();
        assert_eq!(first_content, second_content, "file was overwritten");
    }

    #[test]
    fn archive_renames_file() {
        let (_dir, home) = tmp_home();
        let ori = sample_ori();
        write_bootstrap_md(&home, "TestAgent", "agent-001", &ori, "0.1.0").unwrap();
        assert!(home.bootstrap_md().exists());

        let result = archive_bootstrap_md(&home).unwrap();
        assert!(result.is_some(), "expected Some(path) after archive");
        assert!(!home.bootstrap_md().exists(), "BOOTSTRAP.md should be gone");
        let archived = result.unwrap();
        assert!(archived.exists(), "archived file should exist");
        let fname = archived.file_name().unwrap().to_string_lossy();
        assert!(fname.starts_with("BOOTSTRAP."), "wrong prefix: {fname}");
        assert!(fname.ends_with(".md"), "wrong suffix: {fname}");
    }

    #[test]
    fn archive_returns_none_when_no_file() {
        let (_dir, home) = tmp_home();
        let result = archive_bootstrap_md(&home).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn bootstrap_is_archived_detects_archive() {
        let (_dir, home) = tmp_home();
        assert!(!bootstrap_is_archived(&home));

        let ori = sample_ori();
        write_bootstrap_md(&home, "TestAgent", "agent-001", &ori, "0.1.0").unwrap();
        // BOOTSTRAP.md present but not archived yet
        assert!(!bootstrap_is_archived(&home));

        archive_bootstrap_md(&home).unwrap();
        assert!(bootstrap_is_archived(&home));
    }
}
