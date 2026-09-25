// doctor.rs — `omokoda doctor` environment audit.
//
// Prints ✓ PASS / ✗ FAIL / ⚠ WARN for every check, then a summary line.
// FAIL = hard requirement.  WARN = only relevant when the service is enabled.

use colored::Colorize;
use omokoda_core::{config_toml::OmokodaConfig, home::HomeDir};
use std::path::Path;

// ---------------------------------------------------------------------------
// Result accumulators
// ---------------------------------------------------------------------------

struct Report {
    passed: usize,
    failed: usize,
    warned: usize,
}

impl Report {
    fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            warned: 0,
        }
    }

    fn pass(&mut self, label: &str) {
        println!("  {}  {}", "✓ PASS".green().bold(), label);
        self.passed += 1;
    }

    fn fail(&mut self, label: &str) {
        println!("  {}  {}", "✗ FAIL".red().bold(), label);
        self.failed += 1;
    }

    fn warn(&mut self, label: &str) {
        println!("  {}  {}", "⚠ WARN".yellow().bold(), label);
        self.warned += 1;
    }
}

// ---------------------------------------------------------------------------
// Helper: directory / file existence
// ---------------------------------------------------------------------------

fn check_dir(report: &mut Report, path: &Path, label: &str) {
    if path.is_dir() {
        report.pass(label);
    } else {
        report.fail(label);
    }
}

fn check_file(report: &mut Report, path: &Path, label: &str) {
    if path.is_file() {
        report.pass(label);
    } else {
        report.fail(label);
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run() {
    let home = HomeDir::new();
    let mut r = Report::new();

    // ── Category: Home Directory ─────────────────────────────────────────────
    println!("\n{}", "Home Directory".bold().cyan());
    check_dir(&mut r, &home.root, "~/.omokoda/ exists");
    check_file(&mut r, &home.config_toml(), "~/.omokoda/config.toml exists");
    check_dir(&mut r, &home.identity, "~/.omokoda/identity/ exists");
    check_dir(&mut r, &home.state, "~/.omokoda/state/ exists");
    check_dir(&mut r, &home.receipts, "~/.omokoda/receipts/ exists");

    // ── Category: Config ─────────────────────────────────────────────────────
    println!("\n{}", "Config".bold().cyan());
    let config_path = home.config_toml();
    let config_opt: Option<OmokodaConfig> = if config_path.is_file() {
        match OmokodaConfig::load(&config_path) {
            Ok(cfg) => {
                r.pass("config.toml parses without error");
                Some(cfg)
            }
            Err(e) => {
                r.fail(&format!("config.toml parses without error ({})", e));
                None
            }
        }
    } else {
        r.fail("config.toml parses without error (file missing)");
        None
    };

    if let Some(ref cfg) = config_opt {
        if !cfg.agent.name.is_empty() {
            r.pass("agent.name is non-empty");
        } else {
            r.fail("agent.name is non-empty");
        }
        if !cfg.agent.display_name.is_empty() {
            r.pass("agent.display_name is non-empty");
        } else {
            r.fail("agent.display_name is non-empty");
        }
    } else {
        r.fail("agent.name is non-empty (config not loaded)");
        r.fail("agent.display_name is non-empty (config not loaded)");
    }

    // ── Category: Identity ───────────────────────────────────────────────────
    println!("\n{}", "Identity".bold().cyan());
    check_file(&mut r, &home.identity_json(), "~/.omokoda/identity/identity.json exists");
    check_file(&mut r, &home.birth_json(), "~/.omokoda/identity/birth.json exists");

    // ── Category: Constitution ───────────────────────────────────────────────
    println!("\n{}", "Constitution".bold().cyan());
    check_dir(&mut r, &home.constitution, "~/.omokoda/constitution/ exists");

    let constitution_json = home.constitution_json();
    let has_constitution_json = constitution_json.is_file();
    let has_any_file = has_constitution_json || {
        home.constitution
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    };
    if has_any_file {
        r.pass(
            "constitution.json exists OR constitution/ has at least 1 file",
        );
    } else {
        r.fail(
            "constitution.json exists OR constitution/ has at least 1 file",
        );
    }

    // ── Category: Memory ─────────────────────────────────────────────────────
    println!("\n{}", "Memory".bold().cyan());
    check_dir(
        &mut r,
        &home.state.join("memory"),
        "~/.omokoda/state/memory/ exists",
    );
    check_dir(
        &mut r,
        &home.state.join("gix"),
        "~/.omokoda/state/gix/ exists",
    );

    // ── Category: Services (WARN not FAIL) ───────────────────────────────────
    println!("\n{}", "Services".bold().cyan());
    if let Some(ref cfg) = config_opt {
        check_service(&mut r, cfg.vantage.enabled, &cfg.vantage.url, "vantage");
        check_service(&mut r, cfg.osovm.enabled, &cfg.osovm.url, "osovm");
        check_service(&mut r, cfg.zangbeto.enabled, &cfg.zangbeto.url, "zangbeto");
    } else {
        println!(
            "  {}  {}",
            "⚠ WARN".yellow().bold(),
            "Services check skipped (config not loaded)"
        );
        r.warned += 1;
    }

    // ── Summary ───────────────────────────────────────────────────────────────
    println!(
        "\n{} checks passed, {} checks failed, {} warnings",
        r.passed.to_string().green().bold(),
        r.failed.to_string().red().bold(),
        r.warned.to_string().yellow().bold(),
    );

    if r.failed > 0 {
        println!(
            "{}",
            "Run `omokoda onboard` to fix missing directories."
                .yellow()
        );
    } else {
        println!("{}", "Agent environment looks healthy.".green().bold());
    }
}

fn check_service(report: &mut Report, enabled: bool, url: &str, name: &str) {
    if !enabled {
        // Not enabled — skip silently (no output, no counter increment).
        return;
    }
    if !url.is_empty() {
        report.pass(&format!("{}.url is non-empty (service enabled)", name));
    } else {
        report.warn(&format!(
            "{} is enabled but url is empty — agent cannot connect",
            name
        ));
    }
}
