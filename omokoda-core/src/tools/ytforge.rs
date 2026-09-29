//! YtForge — SkillForge YouTube intake.
//!
//! Paste a YouTube link (single video, playlist, or channel) and get back
//! every repo linked from the description(s), each one classified for
//! *frankenstein* potential — can it be bolted onto other repos:
//!   category (recon/exploitation/instrumentation/network/...),
//!   stack (languages + framework hints), android_capable, termux_runnable,
//!   license, size, description, entry_points.
//!
//! The harvest + classification is done by `scripts/yt_harvest.py` (key-free:
//! yt-dlp if present, else a plain HTTP GET of the public watch page).
//! Optionally (`max_forge` > 0), each repo is then forged through the
//! existing [`SkillForgeTool`] pipeline — unchanged, same security gates —
//! and its full receipt is embedded next to its frankenstein profile.
//!
//! The tool is a *layer* over SkillForge, never a fork: per-repo behavior
//! (prescan hard gate, dynamic discovery, dedup, transform, sandbox, Strix
//! storage scan, Smithers review tickets, on-chain audit) is exactly the
//! existing SkillForgeTool::execute path. Default `max_forge` is 0 —
//! classification only — so a video with 40 repos can't silently trigger 40
//! forges; callers opt in with `max_forge`.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use super::skillforge::SkillForgeTool;
use super::skills::SkillManifestEntry;
use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

pub struct YtForgeTool {
    /// The existing SkillForge pipeline, reused per-repo untouched.
    inner: SkillForgeTool,
    scripts_dir: PathBuf,
}

impl YtForgeTool {
    pub fn new(skills: Arc<Mutex<Vec<SkillManifestEntry>>>) -> Self {
        let base = std::env::var("SKILLFORGE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/opt/ares/Omo-Koda2/omokoda-core"));
        Self {
            inner: SkillForgeTool::new(skills),
            scripts_dir: base.join("scripts"),
        }
    }

    /// Run scripts/yt_harvest.py — must print exactly one JSON object.
    /// When `description` is provided, paste-mode is used: no network fetch
    /// (datacenter IPs are bot-walled by YouTube), straight to repo
    /// extraction + clone + classification.
    fn harvest(
        &self,
        url: &str,
        description: Option<&str>,
        title: Option<&str>,
    ) -> Result<Value, String> {
        let script = self.scripts_dir.join("yt_harvest.py");
        let mut cmd = Command::new("python3");
        cmd.arg(&script).arg(url);
        if let Some(desc) = description {
            cmd.arg("--description").arg(desc);
            if let Some(t) = title {
                cmd.arg("--title").arg(t);
            }
        }
        let out = cmd
            .output()
            .map_err(|e| format!("failed to launch yt_harvest.py: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let v: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
            format!(
                "yt_harvest.py returned non-JSON: {e}; raw: {}",
                stdout.trim()
            )
        })?;
        if v.get("ok").and_then(|b| b.as_bool()) != Some(true) {
            let reason = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("yt_harvest.py failed");
            return Err(reason.to_string());
        }
        Ok(v)
    }
}

#[async_trait]
impl Tool for YtForgeTool {
    fn name(&self) -> &str {
        "ytforge"
    }

    fn description(&self) -> &str {
        "Harvest repo links from a YouTube video/channel description, classify \
         each for frankenstein potential (category/stack/android/termux/license), \
         optionally forge each through SkillForge. \
         act ytforge {\"url\":\"https://youtu.be/...\"[,\"max_forge\":0,\"approve\":false,\"store\":true]}"
    }

    fn required_tier(&self) -> u8 {
        2
    }

    fn is_write_operation(&self) -> bool {
        true
    }

    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "YouTube video, playlist, or channel URL"},
                "description": {"type": "string",
                    "description": "paste the video description to skip the network fetch \
                     (works from bot-walled datacenter IPs); fetch it anywhere first"},
                "title": {"type": "string",
                    "description": "video title, used with description"},
                "max_forge": {"type": "integer",
                    "description": "max repos to forge through SkillForge (0 = classify only)"},
                "approve": {"type": "boolean",
                    "description": "human override to register a review-gated skill"},
                "store": {"type": "boolean",
                    "description": "store + security-scan forged skills (default true)"},
                "transform": {"type": "boolean",
                    "description": "generate an agent-native gateway when surfaces are missing"},
                "sandbox": {"type": "boolean",
                    "description": "boot the generated gateway in docker to prove it runs"}
            },
            "required": ["url"]
        }))
    }

    fn timeout_secs(&self) -> u64 {
        // Harvest + up to max_forge × the full SkillForge pipeline. Overridable
        // via YTFORGE_TIMEOUT_SECS.
        std::env::var("YTFORGE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7200)
    }

    async fn execute(
        &self,
        params: &str,
        context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let v: Value = if params.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?
        };

        let url = v
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or("missing 'url'")?
            .trim()
            .to_string();
        if !(url.contains("youtu.be/") || url.contains("youtube.com/")) {
            return Err("ytforge: only youtube.com / youtu.be URLs are accepted".into());
        }
        let approve = v.get("approve").and_then(|b| b.as_bool()).unwrap_or(false);
        let store = v.get("store").and_then(|b| b.as_bool()).unwrap_or(true);
        let transform = v.get("transform").and_then(|b| b.as_bool()).unwrap_or(true);
        let sandbox = v.get("sandbox").and_then(|b| b.as_bool()).unwrap_or(true);
        let max_forge = v.get("max_forge").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
        let description = v.get("description").and_then(|s| s.as_str());
        let title = v.get("title").and_then(|s| s.as_str());

        // ---- Harvest + classify (Stage 0-0.5 of the locked plan) -----------
        // Paste-mode (description provided) skips the network fetch entirely
        // — YouTube bot-walls datacenter IPs, so the description can be
        // fetched from a residential network and handed in.
        let harvest = self.harvest(&url, description, title)?;
        let repos = harvest.get("repos").cloned().unwrap_or_else(|| json!([]));
        let repo_list: Vec<Value> = repos.as_array().cloned().unwrap_or_default();

        // ---- Optional forge per repo through the existing pipeline ----------
        let mut rows: Vec<Value> = Vec::new();
        let mut forged_count = 0usize;
        for repo in &repo_list {
            let repo_url = repo.get("url").and_then(|u| u.as_str()).unwrap_or("");
            if repo_url.is_empty() {
                continue;
            }
            let mut row = repo.clone();
            if forged_count < max_forge {
                let repo_params = json!({
                    "url": repo_url,
                    "approve": approve,
                    "store": store,
                    "transform": transform,
                    "sandbox": sandbox,
                })
                .to_string();
                match self.inner.execute(&repo_params, context).await {
                    Ok((receipt_json, _)) => match serde_json::from_str::<Value>(&receipt_json) {
                        Ok(receipt) => {
                            row["forge"] = receipt;
                            row["forged"] = json!(true);
                            forged_count += 1;
                        }
                        Err(_) => {
                            row["forge_error"] = json!("skillforge returned non-JSON");
                        }
                    },
                    Err(e) => {
                        row["forge_error"] = json!(e);
                    }
                }
            } else {
                row["forge"] = Value::Null;
                row["forged"] = json!(false);
            }
            rows.push(row);
        }

        let summary = json!({
            "total_repos": repo_list.len(),
            "forged": forged_count,
            "harvest_only": repo_list.len().saturating_sub(forged_count),
        });

        let receipt = json!({
            "status": "harvested",
            "source_url": url,
            "kind": harvest.get("kind"),
            "videos": harvest.get("videos"),
            "harvest_error": harvest.get("harvest_error"),
            "summary": summary,
            "repos": rows,
            "note": "Each repo carries a frankenstein profile (category, stack, \
                     android_capable, termux_runnable, license, size, entry_points). \
                     Pass max_forge=N to forge repos through SkillForge.",
        });

        Ok((
            serde_json::to_string_pretty(&receipt).unwrap_or_else(|_| "{}".into()),
            TokenUsage::default(),
        ))
    }
}
