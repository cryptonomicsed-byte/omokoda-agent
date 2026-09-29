//! SkillForge — turn any GitHub repo into a safe, agent-native service skill.
//!
//! [`SkillForgeTool`] (Rust/Steward) coordinates seven internal stages. Naming
//! is universal (no project-specific mythology): the flow is circular, always
//! entering and leaving through the **Steward**.
//!
//!   Steward (intake)  ─▶ Analysis ─▶ Memory ─▶ Creation
//!        ▲                                        │
//!        └──── Coordination ◀── Audit ◀── Execution
//!
//! Four stages have a polyglot leg (`skillforge_bus.rs`), each calling the
//! ecosystem's already-live language-native service for that domain — real
//! computation there, not a relabeled Rust call:
//!   - **Analysis** also asks Ọbàtálá (Clojure/Babashka, `/skillforge/analyze`)
//!     to symbolically classify the repo from Python-extracted facts.
//!   - **Memory** also asks Ọ̀ṣun (Julia, `/skillforge/similar`) for a
//!     word-overlap similarity check beyond exact-name dedup.
//!   - **Creation** also asks Yemọja (Elixir, `/skillforge/manifest`) to
//!     assemble the `SkillManifestEntry`.
//!   - **Coordination** reports run-state to Ọya (Go, `/skillforge/{start,
//!     transition,finish}`) for observability across the whole pipeline.
//!
//! **Audit** has a Move leg instead — not an HTTP service (Move only makes
//! sense as on-chain state, not a request handler): `crate::onchain::
//! record_skillforge_audit` anchors a content-addressed receipt (hashes only,
//! never the raw name/URL) via the published `skillforge_audit::audit::record`
//! Move package on Sui testnet, fail-open on `OMOKODA_SUI_REGISTRY` unset —
//! the same discipline as birth's on-chain minting.
//!
//! Every polyglot call is fail-soft (`OBATALA_URL`/`OSUN_URL`/`YEMOJA_URL`/
//! `OYA_URL` unset or unreachable → the Rust/Python-only path, unchanged from
//! before this leg existed, decides instead). Nothing about the safety gate
//! below depends on any of these services being up.
//!
//! The tool builds on the existing [`SkillManifestEntry`] / [`ExternalServiceTool`]
//! machinery: a forged skill is just a new manifest entry, registered live for
//! discovery and persisted to disk. Auto-approved skills are invocable as
//! `act <name>` immediately in the same session (dynamic resolution) and
//! survive restarts via the persisted manifest.
//!
//! Safety is fail-closed. A forged skill is only auto-approved when it is
//! read-only, high-confidence, and carries no risk signals. Anything else
//! (`write: true`, confidence < 0.70, or a flagged risk) is held for human
//! review and written out as a review ticket — never silently registered.

use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use super::skills::SkillManifestEntry;
use super::{ExecutionContext, Tool};
use crate::usage::TokenUsage;

const CONFIDENCE_GATE: f64 = 0.70;

/// Analyzer output, deserialized from `scripts/analyze_repo.py` stdout.
#[derive(Debug, Clone, Deserialize)]
struct RepoAnalysis {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    classification: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    base_url_hint: Option<String>,
    #[serde(default)]
    auth_hint: Option<AuthHint>,
    #[serde(default)]
    candidate_routes: HashMap<String, String>,
    #[serde(default)]
    clone_path: Option<String>,
    #[serde(default)]
    security_prescan: Option<SecurityPrescan>,
    #[serde(default)]
    missing_agent_surfaces: Vec<String>,
    #[serde(default)]
    risk_signals: Vec<String>,
    #[serde(default)]
    nuclei: Option<NucleiScan>,
    #[serde(default)]
    language: String,
    #[serde(default)]
    notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AuthHint {
    header: String,
    env: String,
}

/// Stage 0.5 output: mandatory pre-execution security scan on the raw
/// clone, before anything from the repo is ever built or run. `hard_fail`
/// is the one gate in the whole pipeline that never soft-falls-back --
/// betterleaks verified secrets, nuclei criticals, or a malware-signature
/// hit here kill the forge immediately, before Stage 1c ever boots it.
#[derive(Debug, Clone, Deserialize, Default)]
struct SecurityPrescan {
    #[serde(default)]
    hard_fail: bool,
    #[serde(default)]
    reasons: Vec<String>,
}

/// Stage 1c output (`boot_and_discover.py`): mandatory, soft-fallback.
/// `booted` is the honest signal of whether real dynamic discovery
/// happened at all -- when false the skill proceeds on static routes only
/// and must be stamped `fully_forged: false`, never silently treated as
/// fully covered.
#[derive(Debug, Clone, Deserialize, Default)]
struct Discovery {
    #[serde(default)]
    booted: bool,
    #[serde(default)]
    ui_only: bool,
    #[serde(default)]
    discovered_routes: HashMap<String, String>,
    #[serde(default)]
    notes: Vec<String>,
}

/// Native Nuclei file-template scan result (secrets / keys / misconfigs).
#[derive(Debug, Clone, Deserialize)]
struct NucleiScan {
    #[serde(default)]
    ran: bool,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    critical: u64,
    #[serde(default)]
    high: u64,
}

/// Output of the Transformation stage (`scripts/transform_repo.py`): an
/// agent-native gateway generated in front of a human-first repo.
#[derive(Debug, Clone, Deserialize)]
struct Transformation {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    output_dir: String,
    #[serde(default)]
    port: u32,
    #[serde(default)]
    wrapper_base_url: String,
    #[serde(default)]
    generated_files: Vec<String>,
    #[serde(default)]
    added_surfaces: Vec<String>,
    #[serde(default)]
    gateway_routes: HashMap<String, String>,
}

/// The forge tool. Holds a handle to the live skills registry (for immediate
/// discovery) and the path to the durable manifest file (for `act` invocation
/// after reload).
pub struct SkillForgeTool {
    skills: Arc<Mutex<Vec<SkillManifestEntry>>>,
    manifest_path: PathBuf,
    scripts_dir: PathBuf,
    personas_dir: PathBuf,
    review_dir: PathBuf,
    forge_dir: PathBuf,
}

impl SkillForgeTool {
    pub fn new(skills: Arc<Mutex<Vec<SkillManifestEntry>>>) -> Self {
        let base = std::env::var("SKILLFORGE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/opt/ares/Omo-Koda2/omokoda-core"));
        Self {
            skills,
            manifest_path: std::env::var("SKILLFORGE_MANIFEST")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base.join("skills.forged.json")),
            scripts_dir: base.join("scripts"),
            personas_dir: std::env::var("SKILLFORGE_PERSONAS")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/opt/ares/agency-agents")),
            review_dir: std::env::var("SKILLFORGE_REVIEW")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base.join("skillforge_review")),
            forge_dir: std::env::var("SKILLFORGE_FORGE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base.join("skillforge_forge")),
        }
    }

    // ---- Stage 1: Analysis -------------------------------------------------

    fn analysis(&self, url: &str) -> Result<RepoAnalysis, String> {
        let script = self.scripts_dir.join("analyze_repo.py");
        let out = Command::new("python3")
            .arg(&script)
            .arg(url)
            .output()
            .map_err(|e| format!("failed to launch analyzer: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let analysis: RepoAnalysis = serde_json::from_str(stdout.trim())
            .map_err(|e| format!("analyzer returned non-JSON: {e}; raw: {}", stdout.trim()))?;
        if !analysis.ok {
            return Err(format!(
                "analysis failed: {}",
                analysis.error.unwrap_or_else(|| "unknown".into())
            ));
        }
        Ok(analysis)
    }

    /// Stage 1b: symbolic classification via Ọbàtálá (Clojure), over the
    /// facts Python already extracted. Fail-soft: on any transport/parse
    /// error `analysis` keeps `analyze_repo.py`'s own classification and
    /// confidence unchanged — this only ever refines, never blocks.
    async fn classify_symbolic(&self, analysis: &mut RepoAnalysis) {
        let missing = |surface: &str| analysis.missing_agent_surfaces.iter().any(|m| m == surface);
        let facts = super::skillforge_bus::AnalyzeFacts {
            has_openapi: !missing("openapi_spec"),
            has_mcp: !missing("mcp_server"),
            has_rest: !analysis.candidate_routes.is_empty(),
            // Not carried as a discrete field in RepoAnalysis; CliOnly-vs-dockerfile
            // is already folded into Python's own classification, so the
            // "dockerfile with no other surface" rule only fires when Python
            // already called it CliOnly.
            dockerfile: analysis.classification == "CliOnly",
            base_url_hint: analysis.base_url_hint.as_deref(),
            risk_signals: &analysis.risk_signals,
            nuclei_critical: analysis.nuclei.as_ref().map(|n| n.critical).unwrap_or(0),
            nuclei_high: analysis.nuclei.as_ref().map(|n| n.high).unwrap_or(0),
        };
        if let Some(result) = super::skillforge_bus::classify(&facts).await {
            // OSOVM_CODEX.md §42: this note lands directly in the tool's JSON
            // output (`analysis.notes`, see the `notes` field this stage feeds),
            // so the internal service label ("Ọbàtálá") must not appear here --
            // Ọbàtálá's §42 universal term is Policy / Rules.
            analysis.notes.push(format!(
                "Policy (symbolic): {} (confidence {:.2}) — {}",
                result.classification, result.confidence, result.reason
            ));
            analysis.classification = result.classification;
            analysis.confidence = result.confidence;
        }
    }

    // ---- Stage 1c: Discovery (mandatory, soft-fallback) --------------------

    /// Boots the already-security-cleared clone and drives it (Playwright
    /// click-through + network capture, gobuster brute-force) to find real
    /// capability surface static analysis structurally cannot see. Only
    /// ever called after Stage 0.5's hard gate has passed -- never on
    /// unscanned code. Soft-fallback by design: if boot fails for any
    /// reason (language undetectable, install/run failure, port never
    /// opens), returns `booted: false` and the pipeline proceeds on
    /// static-only routes -- the caller stamps `fully_forged` accordingly,
    /// this never lies about what it accomplished.
    async fn boot_and_discover(&self, a: &RepoAnalysis) -> Discovery {
        let Some(clone_path) = a.clone_path.as_deref() else {
            return Discovery::default();
        };
        let script = self.scripts_dir.join("boot_and_discover.py");
        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg(&script)
            .arg("--dir")
            .arg(clone_path)
            .arg("--language")
            .arg(&a.language)
            .arg("--boot-timeout")
            .arg("120")
            .arg("--ready-wait")
            .arg("20");
        if let Some(hint) = &a.base_url_hint {
            cmd.arg("--base-url-hint").arg(hint);
        }
        let discovery = match cmd.output().await {
            Ok(out) => serde_json::from_slice::<Discovery>(&out.stdout).unwrap_or_default(),
            Err(_) => Discovery::default(),
        };
        // The clone is only kept alive for this stage (analyze_repo.py
        // deliberately leaves it on disk); clean it up now regardless of
        // outcome so forged-and-rejected/soft-fallback runs don't leak
        // clones under /tmp.
        let _ = tokio::fs::remove_dir_all(
            std::path::Path::new(clone_path)
                .parent()
                .unwrap_or(std::path::Path::new(clone_path)),
        )
        .await;
        discovery
    }

    // ---- Stage 2: Memory (dedup + canonical naming) ------------------------

    fn memory_dedup(&self, proposed: &str) -> String {
        let guard = match self.skills.lock() {
            Ok(g) => g,
            Err(_) => return proposed.to_string(),
        };
        let existing: Vec<String> = guard.iter().map(|s| s.name.clone()).collect();
        if !existing.iter().any(|n| n == proposed) {
            return proposed.to_string();
        }
        // collision → suffix a version until unique
        for v in 2..1000 {
            let candidate = format!("{proposed}-v{v}");
            if !existing.contains(&candidate) {
                return candidate;
            }
        }
        proposed.to_string()
    }

    /// Stage 2b: Ọ̀ṣun (Julia) similarity check beyond exact-name matching —
    /// catches "same project, different slug" (e.g. a repo that renamed, or a
    /// fork). Fail-soft: `None`/no likely match leaves the exact-name dedup
    /// above as the only signal, exactly as it works today.
    async fn similarity_dedup(&self, name: &str, description: &str) -> Option<String> {
        let existing: Vec<(String, String)> = {
            let guard = self.skills.lock().ok()?;
            guard
                .iter()
                .map(|s| (s.name.clone(), s.description.clone()))
                .collect()
        };
        let entries: Vec<super::skillforge_bus::ExistingSkill> = existing
            .iter()
            .map(|(n, d)| super::skillforge_bus::ExistingSkill {
                name: n,
                description: d,
            })
            .collect();
        let result = super::skillforge_bus::similar(name, description, &entries).await?;
        if result.likely_duplicate {
            result.closest_match
        } else {
            None
        }
    }

    // ---- Stage 3: Creation -------------------------------------------------

    fn creation(&self, name: &str, a: &RepoAnalysis) -> SkillManifestEntry {
        let base_url = a
            .base_url_hint
            .clone()
            .unwrap_or_else(|| format!("${{{}_URL}}", name.to_uppercase().replace('-', "_")));

        let (auth_header, auth_env) = match &a.auth_hint {
            Some(h) => (Some(h.header.clone()), Some(h.env.clone())),
            None => (None, None),
        };

        // A write flag is inferred from any mutating route or risk signal.
        let write = a.candidate_routes.values().any(|r| !r.starts_with("GET"))
            || !a.risk_signals.is_empty();

        let mut routes = a.candidate_routes.clone();
        if routes.is_empty() {
            // No surface found: synthesize a discovery stub so the skill is
            // still shaped for an agent once a wrapper is added (Execution P2).
            routes.insert("health".into(), "GET /health".into());
            routes.insert("discover".into(), "GET /".into());
        }

        SkillManifestEntry {
            name: name.to_string(),
            description: format!(
                "{} [forged by SkillForge from {}; lang={}]",
                a.description, a.classification, a.language
            ),
            base_url,
            auth_header,
            auth_env,
            auth_value: None,
            required_tier: if write { 2 } else { 1 },
            write,
            routes,
        }
    }

    /// Stage 3b: Yemọja (Elixir) manifest assembly. Fail-soft: `None` leaves
    /// the Rust-built `entry` from [`Self::creation`] unchanged — this only
    /// ever replaces it with an equivalent-shape manifest, never removes the
    /// fallback path.
    async fn creation_elixir(&self, name: &str, a: &RepoAnalysis) -> Option<SkillManifestEntry> {
        let auth_hint = a
            .auth_hint
            .as_ref()
            .map(|h| super::skillforge_bus::AuthHintOut {
                header: &h.header,
                env: &h.env,
            });
        let facts = super::skillforge_bus::ManifestFacts {
            name,
            classification: &a.classification,
            language: &a.language,
            description: &a.description,
            base_url_hint: a.base_url_hint.as_deref(),
            auth_hint,
            candidate_routes: &a.candidate_routes,
            risk_signals: &a.risk_signals,
        };
        let m = super::skillforge_bus::build_manifest(&facts).await?;
        Some(SkillManifestEntry {
            name: m.name,
            description: m.description,
            base_url: m.base_url,
            auth_header: m.auth_header,
            auth_env: m.auth_env,
            auth_value: None,
            required_tier: m.required_tier,
            write: m.write,
            routes: m.routes,
        })
    }

    // ---- Stage 4: Execution (best-effort smoke) ----------------------------

    /// A conservative smoke test. If the base URL is already resolvable and a
    /// GET route exists, probe it. Never fails the pipeline — smoke results are
    /// advisory input to Audit. Full Docker sandboxing is Phase 2.
    fn execution(&self, entry: &SkillManifestEntry) -> String {
        let resolved = super::skills::resolve_env(&entry.base_url);
        match resolved {
            None => "skipped: base_url not configured in this environment".into(),
            Some(base) => {
                let get_route = entry.routes.values().find(|r| r.starts_with("GET"));
                match get_route {
                    None => "skipped: no GET route to probe".into(),
                    Some(_) => format!("resolvable at {base}; live probe deferred to sandbox"),
                }
            }
        }
    }

    // ---- Stage 4b: Transformation (make a human-first repo agent-native) ---

    /// Generate an agent-native gateway (MCP discovery + REST proxy + OpenAPI +
    /// agent profile + Dockerfile) in front of the repo. The upstream project is
    /// never modified — the gateway is a sidecar, so this is safe and reversible.
    fn transformation(&self, name: &str, a: &RepoAnalysis) -> Result<Transformation, String> {
        let out_dir = self.forge_dir.join(name);
        let _ = std::fs::create_dir_all(&out_dir);
        let port: u32 = std::env::var("SKILLFORGE_GATEWAY_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8900);
        let analysis_json = serde_json::json!({
            "candidate_routes": a.candidate_routes,
            "base_url_hint": a.base_url_hint,
            "language": a.language,
            "classification": a.classification,
        })
        .to_string();
        let script = self.scripts_dir.join("transform_repo.py");
        let mut child = Command::new("python3")
            .arg(&script)
            .arg("--name")
            .arg(name)
            .arg("--analysis")
            .arg("-")
            .arg("--out")
            .arg(&out_dir)
            .arg("--port")
            .arg(port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to launch transformer: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(analysis_json.as_bytes())
            .map_err(|e| format!("failed to write analysis: {e}"))?;
        let out = child
            .wait_with_output()
            .map_err(|e| format!("transformer failed: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        serde_json::from_str(stdout.trim())
            .map_err(|e| format!("transformer returned non-JSON: {e}; raw: {}", stdout.trim()))
    }

    /// Stage 4b, Ọbàtálá (Clojure) leg: ask Ọbàtálá to shape the gateway's
    /// file contents, write them to disk here (Rust keeps ownership of the
    /// filesystem/Docker side regardless of which language generated the
    /// text). Fail-soft: `None` (unreachable service, missing/partial
    /// response, or a write failure) leaves `transformation()`'s Python path
    /// as the only generator, unchanged from before this leg existed.
    async fn transformation_clojure(
        &self,
        name: &str,
        a: &RepoAnalysis,
        port: u32,
    ) -> Option<Transformation> {
        let facts = super::skillforge_bus::TemplateFacts {
            name,
            port,
            language: &a.language,
            classification: &a.classification,
            base_url_hint: a.base_url_hint.as_deref(),
            candidate_routes: &a.candidate_routes,
        };
        let t = super::skillforge_bus::template_gateway(&facts).await?;
        let out_dir = self.forge_dir.join(name);
        std::fs::create_dir_all(&out_dir).ok()?;
        for (fname, content) in &t.files {
            std::fs::write(out_dir.join(fname), content).ok()?;
        }
        Some(Transformation {
            ok: true,
            error: None,
            output_dir: out_dir.to_string_lossy().to_string(),
            port: t.port,
            wrapper_base_url: t.wrapper_base_url,
            generated_files: t.files.keys().cloned().collect(),
            added_surfaces: t.added_surfaces,
            gateway_routes: t.gateway_routes,
        })
    }

    /// Build and boot the generated gateway in a Docker sandbox and smoke-test
    /// its agent surfaces. Returns the raw sandbox report (advisory to Audit).
    fn sandbox(&self, name: &str, t: &Transformation) -> serde_json::Value {
        let script = self.scripts_dir.join("sandbox_smoke.py");
        match Command::new("python3")
            .arg(&script)
            .arg("--dir")
            .arg(&t.output_dir)
            .arg("--name")
            .arg(name)
            .arg("--port")
            .arg(t.port.to_string())
            .output()
        {
            Ok(o) => serde_json::from_slice(&o.stdout)
                .unwrap_or_else(|_| serde_json::json!({"ok": false, "error": "sandbox non-JSON"})),
            Err(e) => {
                serde_json::json!({"ok": false, "error": format!("sandbox launch failed: {e}")})
            }
        }
    }

    // ---- Stage 5: Audit (risk scoring + persona + human gate) --------------

    fn audit(&self, a: &RepoAnalysis, entry: &SkillManifestEntry) -> Audit {
        let mut reasons = Vec::new();
        let mut risk = 0u32;

        if entry.write {
            risk += 3;
            reasons.push("write operations require human approval".to_string());
        }
        if a.confidence < CONFIDENCE_GATE {
            risk += 2;
            reasons.push(format!(
                "low analysis confidence {:.2} < {:.2}",
                a.confidence, CONFIDENCE_GATE
            ));
        }
        for sig in &a.risk_signals {
            risk += 2;
            reasons.push(format!("risk signal: {sig}"));
        }
        if a.classification == "Unknown" {
            risk += 1;
            reasons.push("could not classify repository".to_string());
        }
        if let Some(n) = &a.nuclei {
            if n.ran && (n.critical > 0 || n.high > 0) {
                risk += 3;
                reasons.push(format!(
                    "nuclei file scan found {} critical / {} high finding(s) (secrets/misconfig)",
                    n.critical, n.high
                ));
            }
        }

        let tier = if risk >= 5 {
            3
        } else if risk >= 2 {
            2
        } else {
            1
        };
        let persona = self.pick_persona(a);
        let requires_review = !reasons.is_empty();

        Audit {
            risk_score: risk,
            recommended_tier: tier,
            persona,
            requires_review,
            reasons,
        }
    }

    /// Choose the security persona best suited to review this repo. Falls back
    /// to the app-sec engineer for generic API work.
    fn pick_persona(&self, a: &RepoAnalysis) -> String {
        let hay = format!("{} {} {}", a.description, a.language, a.name).to_lowercase();
        let slug = if hay.contains("chain")
            || hay.contains("solidity")
            || hay.contains("move")
            || hay.contains("web3")
        {
            "security/security-blockchain-security-auditor.md"
        } else if hay.contains("cloud") || hay.contains("kubernetes") || hay.contains("terraform") {
            "security/security-cloud-security-architect.md"
        } else if !a.risk_signals.is_empty() {
            "security/security-penetration-tester.md"
        } else {
            "security/security-appsec-engineer.md"
        };
        let p = self.personas_dir.join(slug);
        if p.exists() {
            slug.to_string()
        } else {
            "security/security-appsec-engineer.md".to_string()
        }
    }

    // ---- Stage 5b: Storage + full security scan (Gitea via Vantage) --------

    /// Resolve the Vantage agent key: env override, then the standard key file,
    /// then a generic VANTAGE_KEY.
    fn vantage_key(&self) -> Option<String> {
        if let Ok(k) = std::env::var("SKILLFORGE_VANTAGE_KEY") {
            if !k.is_empty() {
                return Some(k);
            }
        }
        let keyfile = std::env::var("SKILLFORGE_VANTAGE_KEYFILE")
            .unwrap_or_else(|_| "/opt/ares/.vantage_key".to_string());
        if let Ok(k) = std::fs::read_to_string(&keyfile) {
            let k = k.trim().to_string();
            if !k.is_empty() {
                return Some(k);
            }
        }
        std::env::var("VANTAGE_KEY").ok().filter(|k| !k.is_empty())
    }

    /// Read a key from the process env, falling back to `/opt/ares/.env`.
    fn env_or_dotenv(key: &str) -> Option<String> {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
        let dotenv =
            std::env::var("SKILLFORGE_DOTENV").unwrap_or_else(|_| "/opt/ares/.env".to_string());
        let text = std::fs::read_to_string(dotenv).ok()?;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{key}=")) {
                let v = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Register the OKF/Strix security webhook on the repo (correct owner) so
    /// every push triggers the full pipeline at the :9876 receiver. Vantage's
    /// own auto-registration targets the wrong owner, so we do it directly.
    async fn register_security_webhook(
        client: &reqwest::Client,
        gitea_url: &str,
        gitea_token: &str,
        owner: &str,
        repo: &str,
    ) -> bool {
        // idempotent: skip if a :9876 push hook already exists
        if let Ok(resp) = client
            .get(format!("{gitea_url}/api/v1/repos/{owner}/{repo}/hooks"))
            .header("Authorization", format!("token {gitea_token}"))
            .send()
            .await
        {
            if let Ok(hooks) = resp.json::<serde_json::Value>().await {
                if hooks.as_array().is_some_and(|a| {
                    a.iter().any(|h| {
                        h.pointer("/config/url")
                            .and_then(|u| u.as_str())
                            .is_some_and(|u| u.contains("9876"))
                    })
                }) {
                    return true;
                }
            }
        }
        let r = client
            .post(format!("{gitea_url}/api/v1/repos/{owner}/{repo}/hooks"))
            .header("Authorization", format!("token {gitea_token}"))
            .json(&serde_json::json!({
                "type": "gitea",
                "config": {
                    "url": "http://localhost:9876/",
                    "content_type": "json",
                    "secret": "vantage-stix-webhook-2026"
                },
                "events": ["push"],
                "active": true
            }))
            .send()
            .await;
        matches!(r, Ok(resp) if resp.status().is_success())
    }

    /// Store the forged skill as a Gitea repo and run the full security test.
    /// Every skill is stored under the Gitea token's owner and scanned; the
    /// push (re)triggers the OKF/Strix pipeline. `ok=false` or `critical>0`
    /// trips human review (fail-closed).
    async fn store_and_scan(
        &self,
        name: &str,
        entry: &SkillManifestEntry,
        forge_dir: Option<&str>,
        files: &[String],
    ) -> serde_json::Value {
        let base =
            std::env::var("VANTAGE_URL").unwrap_or_else(|_| "http://localhost:8001".to_string());
        let key = match self.vantage_key() {
            Some(k) => k,
            None => {
                return serde_json::json!({"ok": false,
                    "error": "no Vantage agent key (set SKILLFORGE_VANTAGE_KEY or /opt/ares/.vantage_key)"})
            }
        };
        let repo = format!("skill-{name}");
        let client = reqwest::Client::new();

        // 1) create repo via Vantage; parse the real owner from the response
        //    (Vantage creates under the Gitea token's user, not a fixed owner).
        let create = client
            .post(format!("{base}/api/code/repo/create"))
            .header("X-Agent-Key", &key)
            .json(&serde_json::json!({
                "name": repo, "description": entry.description, "private": false
            }))
            .send()
            .await;
        let mut owner =
            std::env::var("SKILLFORGE_GITEA_OWNER").unwrap_or_else(|_| "vantage".to_string());
        if let Ok(resp) = create {
            if let Ok(j) = resp.json::<serde_json::Value>().await {
                if let Some(full) = j.get("repo").and_then(|v| v.as_str()) {
                    if let Some((o, _)) = full.split_once('/') {
                        owner = o.to_string();
                    }
                }
            }
        }

        // 2) register the security webhook on the correct owner (before pushes)
        let gitea_url =
            Self::env_or_dotenv("GITEA_URL").unwrap_or_else(|| "http://localhost:3001".to_string());
        let gitea_token = Self::env_or_dotenv("GITEA_TOKEN").unwrap_or_default();
        let webhook_ok = if gitea_token.is_empty() {
            false
        } else {
            Self::register_security_webhook(&client, &gitea_url, &gitea_token, &owner, &repo).await
        };

        // 3) push manifest + generated gateway files (each push triggers OKF)
        let manifest = serde_json::to_string_pretty(entry).unwrap_or_default();
        let mut pushed: Vec<String> = Vec::new();
        let do_push = |path: String, content: String| {
            let (base, key, repo, owner) = (base.clone(), key.clone(), repo.clone(), owner.clone());
            let client = client.clone();
            async move {
                client
                    .post(format!("{base}/api/code/repo/{owner}/{repo}/push"))
                    .header("X-Agent-Key", &key)
                    .json(&serde_json::json!({
                        "path": path, "content": content,
                        "message": "SkillForge: store forged skill", "branch": "main"
                    }))
                    .send()
                    .await
                    .map(|resp| {
                        // push_result=="ok" means the git push actually landed
                        resp.status().is_success()
                    })
                    .unwrap_or(false)
            }
        };
        if do_push("skill.json".to_string(), manifest).await {
            pushed.push("skill.json".to_string());
        }
        if let Some(dir) = forge_dir {
            for f in files {
                if let Ok(content) = std::fs::read_to_string(std::path::Path::new(dir).join(f)) {
                    if do_push(format!("gateway/{f}"), content).await {
                        pushed.push(format!("gateway/{f}"));
                    }
                }
            }
        }

        // 4a) fast synchronous regex pre-scan (immediate signal)
        let regex_scan = client
            .post(format!(
                "{base}/api/code/repo/{owner}/{repo}/scan?engine=regex"
            ))
            .header("X-Agent-Key", &key)
            .send()
            .await;
        let (regex_total, regex_critical, regex_ran) = match regex_scan {
            Ok(r) if r.status().is_success() => {
                let j: serde_json::Value = r.json().await.unwrap_or_default();
                let total = j
                    .get("total_findings")
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        j.get("findings")
                            .and_then(|f| f.as_array())
                            .map(|a| a.len() as u64)
                    })
                    .unwrap_or(0);
                let critical = j.get("critical").and_then(|v| v.as_u64()).unwrap_or(0);
                (total, critical, true)
            }
            _ => (0, 0, false),
        };

        // 4b) full Strix AI pentest (engine=strix). Dispatched async; we poll a
        //     bounded window. If it finishes it is the authoritative gate; if it
        //     is still running we fall back to the regex verdict and surface the
        //     scan_id so the full result is trackable.
        let mut strix_status = "not_dispatched".to_string();
        let mut strix_scan_id: Option<u64> = None;
        let mut strix_total: u64 = 0;
        let mut strix_critical: u64 = 0;
        let strix_dispatch = client
            .post(format!(
                "{base}/api/code/repo/{owner}/{repo}/scan?engine=strix"
            ))
            .header("X-Agent-Key", &key)
            .send()
            .await;
        if let Ok(r) = strix_dispatch {
            if r.status().is_success() {
                let j: serde_json::Value = r.json().await.unwrap_or_default();
                strix_scan_id = j.get("scan_id").and_then(|v| v.as_u64());
                strix_status = "running".to_string();
            }
        }
        if let Some(sid) = strix_scan_id {
            let wait_secs: u64 = std::env::var("SKILLFORGE_SCAN_WAIT")
                .ok()
                .and_then(|w| w.parse().ok())
                .unwrap_or(20);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_secs);
            while std::time::Instant::now() < deadline {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let poll = client
                    .get(format!("{base}/api/code/repo/{owner}/{repo}/scan/{sid}"))
                    .header("X-Agent-Key", &key)
                    .send()
                    .await;
                if let Ok(r) = poll {
                    if let Ok(j) = r.json::<serde_json::Value>().await {
                        let st = j
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("running");
                        if st == "complete" || st == "error" {
                            strix_status = st.to_string();
                            if let Some(f) = j.get("findings").and_then(|f| f.as_array()) {
                                strix_total = f.len() as u64;
                                strix_critical = f
                                    .iter()
                                    .filter(|x| {
                                        x.get("severity").and_then(|s| s.as_f64()).unwrap_or(0.0)
                                            >= 0.90
                                    })
                                    .count()
                                    as u64;
                            }
                            break;
                        }
                    }
                }
            }
        }

        // Effective gate: prefer Strix when it completed, else the regex verdict.
        let strix_completed = strix_status == "complete";
        let critical = if strix_completed {
            strix_critical
        } else {
            regex_critical
        };
        let scan_ran = regex_ran || strix_scan_id.is_some();

        serde_json::json!({
            "ok": scan_ran && !pushed.is_empty(),
            "repo": format!("{owner}/{repo}"),
            "html_url": format!("{gitea_url}/{owner}/{repo}"),
            "pushed": pushed,
            "security_webhook_registered": webhook_ok,
            "security_scan": {
                "gate_engine": if strix_completed { "strix" } else { "regex" },
                "audit_complete": strix_completed,
                "critical": critical,
                "regex": {"ran": regex_ran, "total": regex_total, "critical": regex_critical},
                "strix": {
                    "status": strix_status,
                    "scan_id": strix_scan_id,
                    "total": strix_total,
                    "critical": strix_critical,
                },
            },
            "okf_pipeline": "full chain (betterleaks/Strix/Nuclei/...) also triggered via push webhook; findings post as Gitea issues + Vantage feed"
        })
    }

    /// Register the forged skill in Vantage's platform-wide skill registry so
    /// every Vantage-born agent can discover it. Idempotent (409 = already
    /// registered is treated as success).
    async fn register_in_vantage(&self, entry: &SkillManifestEntry) -> serde_json::Value {
        let base =
            std::env::var("VANTAGE_URL").unwrap_or_else(|_| "http://localhost:8001".to_string());
        let key = match self.vantage_key() {
            Some(k) => k,
            None => return serde_json::json!({"registered": false, "error": "no vantage key"}),
        };
        let client = reqwest::Client::new();
        let r = client
            .post(format!("{base}/api/collectives/skills"))
            .header("X-Agent-Key", &key)
            .json(&serde_json::json!({
                "name": entry.name,
                "description": entry.description,
                "input_schema": {
                    "routes": entry.routes,
                    "base_url": entry.base_url,
                    "write": entry.write,
                },
                "runtime": "external-http"
            }))
            .send()
            .await;
        match r {
            Ok(resp) => {
                let status = resp.status();
                serde_json::json!({
                    "registered": status.is_success() || status.as_u16() == 409,
                    "already_registered": status.as_u16() == 409,
                    "http_status": status.as_u16(),
                })
            }
            Err(e) => serde_json::json!({"registered": false, "error": e.to_string()}),
        }
    }

    // ---- Stage 6/7: Coordination + Steward finalize ------------------------

    /// Persist an approved skill: append to the durable manifest and push into
    /// the live registry so it surfaces in `skills` immediately.
    fn register_live(&self, entry: &SkillManifestEntry) -> Result<(), String> {
        // durable manifest (create-or-append)
        let mut manifest: super::skills::SkillManifest =
            match std::fs::read_to_string(&self.manifest_path) {
                Ok(txt) => serde_json::from_str(&txt).unwrap_or_default(),
                Err(_) => Default::default(),
            };
        manifest.skills.retain(|s| s.name != entry.name);
        manifest.skills.push(entry.clone());
        let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        std::fs::write(&self.manifest_path, json)
            .map_err(|e| format!("failed to persist manifest: {e}"))?;

        // live discovery registry
        if let Ok(mut g) = self.skills.lock() {
            g.retain(|s| s.name != entry.name);
            g.push(entry.clone());
        }
        Ok(())
    }

    /// Write a human-review ticket for a skill that failed the safety gate.
    fn write_review_ticket(&self, entry: &SkillManifestEntry, audit: &Audit, url: &str) -> String {
        let _ = std::fs::create_dir_all(&self.review_dir);
        let ticket = serde_json::json!({
            "status": "pending_human_review",
            "source_url": url,
            "skill": entry,
            "risk_score": audit.risk_score,
            "recommended_tier": audit.recommended_tier,
            "review_persona": audit.persona,
            "reasons": audit.reasons,
        });
        let path = self.review_dir.join(format!("{}.review.json", entry.name));
        let _ = std::fs::write(
            &path,
            serde_json::to_string_pretty(&ticket).unwrap_or_default(),
        );
        path.to_string_lossy().to_string()
    }
}

struct Audit {
    risk_score: u32,
    recommended_tier: u8,
    persona: String,
    requires_review: bool,
    reasons: Vec<String>,
}

#[async_trait]
impl Tool for SkillForgeTool {
    fn name(&self) -> &str {
        "skillforge"
    }
    fn description(&self) -> &str {
        "Forge an agent-native service skill from a GitHub repo. \
         act skillforge {\"url\":\"https://github.com/owner/repo.git\"[,\"approve\":true]}. \
         Read-only, high-confidence skills auto-register; write/low-confidence/risky \
         skills are held for human review and a ticket is written."
    }
    fn required_tier(&self) -> u8 {
        2
    }
    fn is_write_operation(&self) -> bool {
        true
    }
    fn timeout_secs(&self) -> u64 {
        // The full Strix pentest runs up to 30 min; allow the forge to block on
        // it so registration is gated by the complete security audit. Overridable
        // via SKILLFORGE_TIMEOUT_SECS.
        std::env::var("SKILLFORGE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2000)
    }
    fn params_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "GitHub repo URL"},
                "approve": {"type": "boolean",
                    "description": "human override to register a review-gated skill"}
            },
            "required": ["url"]
        }))
    }

    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, TokenUsage), String> {
        let v: serde_json::Value = if params.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?
        };

        // ---- Stage 0: Steward intake / validation --------------------------
        let url = v
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or("missing 'url'")?
            .trim()
            .to_string();
        if !(url.starts_with("https://github.com/")
            || url.starts_with("http://github.com/")
            || url.starts_with("git@github.com:"))
        {
            return Err("only github.com URLs are accepted".into());
        }
        let approve = v.get("approve").and_then(|b| b.as_bool()).unwrap_or(false);

        let transform_req = v.get("transform").and_then(|b| b.as_bool()).unwrap_or(true);
        let sandbox_req = v.get("sandbox").and_then(|b| b.as_bool()).unwrap_or(true);

        // Ọya (Go) Coordination: best-effort run tracking across the whole
        // pipeline. Never affects the outcome — pure observability.
        let run_id = format!(
            "sf-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            std::process::id()
        );
        super::skillforge_bus::coordinate_start(&run_id, &url).await;

        // ---- Pipeline ------------------------------------------------------
        let mut analysis = match self.analysis(&url) {
            Ok(a) => a,
            Err(e) => {
                super::skillforge_bus::coordinate_finish(&run_id, false, &e).await;
                return Err(e);
            }
        };
        // Stage 1b: Ọbàtálá (Clojure) symbolic classification — refines
        // classification/confidence in place; no-op if the service is down.
        self.classify_symbolic(&mut analysis).await;
        super::skillforge_bus::coordinate_transition(&run_id, "analysis").await;

        // Stage 0.5 hard gate: analyze_repo.py already ran betterleaks +
        // nuclei + malware-signature checks on the raw clone before this
        // point. This is the one gate in the pipeline that never soft-falls
        // back -- a verified secret, critical nuclei finding, or malware
        // signature kills the forge here, before anything is ever booted.
        if let Some(prescan) = &analysis.security_prescan {
            if prescan.hard_fail {
                let reason = format!(
                    "security prescan hard-failed: {}",
                    prescan.reasons.join("; ")
                );
                super::skillforge_bus::coordinate_finish(&run_id, false, &reason).await;
                crate::onchain::record_skillforge_audit(&analysis.name, &url, 10, true, false)
                    .await;
                return Err(reason);
            }
        }

        // Stage 1c: mandatory dynamic discovery, soft-fallback. Always
        // attempted (not gated on missing_agent_surfaces) so a repo whose
        // static analysis looks "complete" doesn't skip real capability
        // discovery -- the whole point is maximum agent exposure, not just
        // whatever a route-declaration regex happened to find.
        let discovery = self.boot_and_discover(&analysis).await;
        for (k, v) in &discovery.discovered_routes {
            analysis
                .candidate_routes
                .entry(k.clone())
                .or_insert_with(|| v.clone());
        }
        let fully_forged = discovery.booted;
        super::skillforge_bus::coordinate_transition(&run_id, "discovery").await;

        let name = self.memory_dedup(&analysis.name);
        // Stage 2b: Ọ̀ṣun (Julia) similarity dedup — if it finds a likely
        // duplicate under a different name than the exact-match check above
        // caught, fold that name into the same versioning path.
        let name = match self.similarity_dedup(&name, &analysis.description).await {
            Some(dup) if dup != name => self.memory_dedup(&dup),
            _ => name,
        };
        super::skillforge_bus::coordinate_transition(&run_id, "memory").await;

        let mut entry = match self.creation_elixir(&name, &analysis).await {
            Some(e) => e,
            None => self.creation(&name, &analysis),
        };
        super::skillforge_bus::coordinate_transition(&run_id, "creation").await;

        // Stage 4b: transform human-first repos into an agent-native gateway.
        // When surfaces are missing and transformation succeeds, the skill is
        // repointed at the generated gateway (MCP + REST) instead of the bare
        // upstream, and sandboxed to prove it boots.
        let mut transformation_receipt = serde_json::Value::Null;
        let mut sandbox_receipt = serde_json::Value::Null;
        let mut sandbox_failed = false;
        let mut forge_output_dir: Option<String> = None;
        let mut forge_files: Vec<String> = Vec::new();
        if transform_req && !analysis.missing_agent_surfaces.is_empty() {
            let port: u32 = std::env::var("SKILLFORGE_GATEWAY_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8900);
            let transform_result = match self.transformation_clojure(&name, &analysis, port).await {
                Some(t) => Ok(t),
                None => self.transformation(&name, &analysis),
            };
            match transform_result {
                Ok(t) if t.ok => {
                    entry.base_url = t.wrapper_base_url.clone();
                    forge_output_dir = Some(t.output_dir.clone());
                    forge_files = t.generated_files.clone();
                    for (k, val) in &t.gateway_routes {
                        entry.routes.insert(k.clone(), val.clone());
                    }
                    transformation_receipt = serde_json::json!({
                        "output_dir": t.output_dir,
                        "wrapper_base_url": t.wrapper_base_url,
                        "added_surfaces": t.added_surfaces,
                        "generated_files": t.generated_files,
                    });
                    if sandbox_req {
                        let report = self.sandbox(&name, &t);
                        // A real failure (ran but not ok) trips the review gate;
                        // a skip (no docker) does not.
                        let ran = report
                            .get("sandboxed")
                            .and_then(|b| b.as_bool())
                            .unwrap_or(false);
                        let ok = report.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
                        sandbox_failed = ran && !ok;
                        sandbox_receipt = report;
                    }
                }
                Ok(t) => {
                    transformation_receipt = serde_json::json!({"ok": false, "error": t.error});
                }
                Err(e) => {
                    transformation_receipt = serde_json::json!({"ok": false, "error": e});
                }
            }
        }

        // Stage 5b: mandatory storage in Gitea + full security scan. Every
        // forged skill is stored and scanned; this is the fail-closed gate.
        let store_req = v.get("store").and_then(|b| b.as_bool()).unwrap_or(true);
        let mut store_receipt = serde_json::Value::Null;
        let mut security_gate = false;
        if store_req {
            store_receipt = self
                .store_and_scan(&name, &entry, forge_output_dir.as_deref(), &forge_files)
                .await;
            let ok = store_receipt
                .get("ok")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            let critical = store_receipt
                .pointer("/security_scan/critical")
                .and_then(|c| c.as_u64())
                .unwrap_or(0);
            // The full security audit (Strix) must complete cleanly. If it did
            // not finish, or found criticals, or storage failed → human review.
            let audit_complete = store_receipt
                .pointer("/security_scan/audit_complete")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            // Strict full-audit gating is opt-in until Strix can run here (needs
            // its sandbox image + disk on the new VPS). Default: gate on the
            // fast synchronous scan; Strix + OKF still run as best-effort/async.
            let require_full_audit = std::env::var("SKILLFORGE_REQUIRE_FULL_AUDIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            security_gate = !ok || critical > 0 || (require_full_audit && !audit_complete);
        }

        let smoke = self.execution(&entry);
        let mut audit = self.audit(&analysis, &entry);
        if sandbox_failed {
            audit.requires_review = true;
            audit.risk_score += 2;
            audit.reasons.push("sandbox smoke test failed".to_string());
        }
        if security_gate {
            audit.requires_review = true;
            audit.risk_score += 3;
            let _ok = store_receipt
                .get("ok")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            let crit = store_receipt
                .pointer("/security_scan/critical")
                .and_then(|c| c.as_u64())
                .unwrap_or(0);
            let audit_complete = store_receipt
                .pointer("/security_scan/audit_complete")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            audit.reasons.push(if crit > 0 {
                format!("full security audit (Strix) found {crit} critical finding(s)")
            } else if !audit_complete {
                "full security audit (Strix) did not complete — cannot verify".to_string()
            } else {
                "storage did not complete — cannot verify".to_string()
            });
        }

        // ---- Steward finalize (fail-closed gate) ---------------------------
        let (status, activation, review_ticket) = if !audit.requires_review {
            self.register_live(&entry)?;
            (
                "registered",
                format!(
                    "act {} {{\"route\":\"...\"}} — live now in this session",
                    entry.name
                ),
                serde_json::Value::Null,
            )
        } else if approve {
            self.register_live(&entry)?;
            (
                "registered_with_override",
                format!("act {} {{\"route\":\"...\"}}", entry.name),
                serde_json::Value::Null,
            )
        } else {
            let ticket = self.write_review_ticket(&entry, &audit, &url);
            (
                "pending_human_review",
                "re-run with \"approve\": true after review to register".to_string(),
                serde_json::Value::String(ticket),
            )
        };
        super::skillforge_bus::coordinate_transition(&run_id, "audit").await;

        // Sango (Move) on-chain audit receipt -- fail-open, see onchain.rs.
        let approved_final = status.starts_with("registered");
        let onchain_receipt = crate::onchain::record_skillforge_audit(
            &entry.name,
            &url,
            audit.risk_score,
            audit.requires_review,
            approved_final,
        )
        .await;

        // Stage 7: register in Vantage platform skill registry (discoverable by
        // all Vantage-born agents) once the skill is actually registered.
        let vantage_registry = if status.starts_with("registered") {
            self.register_in_vantage(&entry).await
        } else {
            serde_json::json!({"registered": false, "skipped": "held for human review"})
        };

        let receipt = serde_json::json!({
            "status": status,
            "source_url": url,
            "skill": {
                "name": entry.name,
                "description": entry.description,
                "base_url": entry.base_url,
                "write": entry.write,
                "required_tier": entry.required_tier,
                "routes": entry.routes,
            },
            "analysis": {
                "classification": analysis.classification,
                "confidence": analysis.confidence,
                "language": analysis.language,
                "missing_agent_surfaces": analysis.missing_agent_surfaces,
                "risk_signals": analysis.risk_signals,
                "nuclei": analysis.nuclei.as_ref().map(|n| serde_json::json!({
                    "ran": n.ran, "total": n.total,
                    "critical": n.critical, "high": n.high
                })),
                "notes": analysis.notes,
            },
            "audit": {
                "risk_score": audit.risk_score,
                "recommended_tier": audit.recommended_tier,
                "review_persona": audit.persona,
                "requires_review": audit.requires_review,
                "reasons": audit.reasons,
            },
            "discovery": {
                "fully_forged": fully_forged,
                "ui_only": discovery.ui_only,
                "discovered_route_count": discovery.discovered_routes.len(),
                "notes": discovery.notes,
            },
            "execution": smoke,
            "transformation": transformation_receipt,
            "sandbox": sandbox_receipt,
            "storage": store_receipt,
            "vantage_registry": vantage_registry,
            "onchain_audit_receipt": onchain_receipt,
            "activation": activation,
            "review_ticket": review_ticket,
        });

        super::skillforge_bus::coordinate_finish(&run_id, true, "").await;

        Ok((
            serde_json::to_string_pretty(&receipt).unwrap_or_else(|_| "{}".into()),
            TokenUsage::default(),
        ))
    }
}
