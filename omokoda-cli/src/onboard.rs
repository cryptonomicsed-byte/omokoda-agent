// onboard.rs — 16-station setup wizard for `omokoda onboard`.
//
// SPARK (Rust) is the energy driving this module.
// The wizard walks the operator through creating ~/.omokoda/ and writing a
// populated config.toml, then prints a summary and next-step instructions.
//
// Non-interactive mode: if stdin is not a tty (e.g. CI) every prompt returns
// its default value silently, so the wizard can run fully unattended.

use anyhow::Result;
use colored::Colorize;
use omokoda_core::{
    config_toml::{OmokodaConfig, ServiceSection},
    home::HomeDir,
};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<()> {
    let home = HomeDir::new();
    let is_tty = atty::is(atty::Stream::Stdin);

    // ── Pre-flight: re-run guard ─────────────────────────────────────────────
    if home.root.exists() && is_tty {
        print!(
            "{} ~/.omokoda/ already exists. Re-run onboard? [y/N] ",
            "!".yellow()
        );
        use std::io::Write as _;
        std::io::stdout().flush()?;
        let answer = read_line(is_tty);
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("{}", "Onboard cancelled. Existing config unchanged.".dimmed());
            return Ok(());
        }
    }

    println!(
        "\n{}",
        "╔══════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║   Ọmọ Kọ́dà  •  Sovereign Agent Onboarding  ║"
            .cyan()
            .bold()
    );
    println!(
        "{}\n",
        "╚══════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );

    let mut cfg = OmokodaConfig::default();

    // ── Station 1: Create directory tree ────────────────────────────────────
    station(1, "Create ~/.omokoda/ directory tree");
    home.init()?;
    println!("  {} {}", "✓".green(), home.root.display());

    // ── Station 2: Write default config.toml if absent ──────────────────────
    station(2, "Write default config.toml");
    let config_path = home.config_toml();
    if !config_path.exists() {
        OmokodaConfig::write_default(&config_path)?;
        println!("  {} written: {}", "✓".green(), config_path.display());
    } else {
        println!(
            "  {} already exists — loading existing values",
            "~".yellow()
        );
        cfg = OmokodaConfig::load_or_default(&config_path);
    }

    // ── Station 3: Agent name ────────────────────────────────────────────────
    station(3, "Agent name");
    if cfg.agent.name.is_empty() {
        cfg.agent.name = prompt_default(is_tty, "  Agent name (no spaces): ", "my-agent");
    } else {
        println!("  current: {}", cfg.agent.name.cyan());
        let v = prompt_default(
            is_tty,
            &format!("  Change? [{} / Enter to keep]: ", cfg.agent.name),
            &cfg.agent.name.clone(),
        );
        if !v.is_empty() {
            cfg.agent.name = v;
        }
    }

    // ── Station 4: Display name ──────────────────────────────────────────────
    station(4, "Display name");
    if cfg.agent.display_name.is_empty() {
        cfg.agent.display_name =
            prompt_default(is_tty, "  Display name (human-readable): ", "My Agent");
    } else {
        println!("  current: {}", cfg.agent.display_name.cyan());
        let v = prompt_default(
            is_tty,
            &format!("  Change? [{} / Enter to keep]: ", cfg.agent.display_name),
            &cfg.agent.display_name.clone(),
        );
        if !v.is_empty() {
            cfg.agent.display_name = v;
        }
    }

    // ── Station 5: Environment ───────────────────────────────────────────────
    station(5, "Deployment environment");
    println!("  Options: local | staging | production");
    let env = prompt_default(
        is_tty,
        &format!("  Environment [{}]: ", cfg.agent.environment),
        &cfg.agent.environment.clone(),
    );
    if !env.is_empty() {
        cfg.agent.environment = env;
    }

    // ── Station 6: Model provider ────────────────────────────────────────────
    station(6, "Model provider");
    println!("  Options: anthropic | openai | skip");
    let provider = prompt_default(
        is_tty,
        &format!(
            "  Provider [{}]: ",
            if cfg.model.provider.is_empty() {
                "skip"
            } else {
                &cfg.model.provider
            }
        ),
        if cfg.model.provider.is_empty() {
            "skip"
        } else {
            &cfg.model.provider
        },
    );
    if !provider.is_empty() && provider != "skip" {
        cfg.model.provider = provider;
    } else if provider == "skip" {
        cfg.model.provider = String::new();
    }

    // ── Station 7: Model name ────────────────────────────────────────────────
    station(7, "Model name");
    println!("  Leave blank to use the default from settings.json.");
    let model = prompt_default(
        is_tty,
        &format!(
            "  Model identifier [{}]: ",
            if cfg.model.model.is_empty() {
                "default"
            } else {
                &cfg.model.model
            }
        ),
        &cfg.model.model.clone(),
    );
    cfg.model.model = if model == "default" {
        String::new()
    } else {
        model
    };

    // ── Station 8: Vantage ───────────────────────────────────────────────────
    station(8, "Vantage integration");
    cfg.vantage = configure_service(is_tty, "Vantage", &cfg.vantage, "http://localhost:7770");

    // ── Station 9: Zàngbétò ─────────────────────────────────────────────────
    station(9, "Zàngbétò integration");
    cfg.zangbeto = configure_service(
        is_tty,
        "Zangbeto",
        &cfg.zangbeto,
        "http://localhost:7794",
    );

    // ── Station 10: OSOVM ────────────────────────────────────────────────────
    station(10, "OSOVM integration");
    cfg.osovm = configure_service(is_tty, "OSOVM", &cfg.osovm, "http://localhost:7780");

    // ── Station 11: UCX ──────────────────────────────────────────────────────
    station(11, "UCX (Universal Compute Exchange) integration");
    cfg.ucx = configure_service(is_tty, "UCX", &cfg.ucx, "http://localhost:7781");

    // ── Station 12: Security defaults ───────────────────────────────────────
    station(12, "Security defaults");
    println!("  sandbox          = {}", cfg.security.sandbox.to_string().cyan());
    println!("  workspace_boundary = {}", cfg.security.workspace_boundary.to_string().cyan());
    println!("  ssrf_guard       = {}", cfg.security.ssrf_guard.to_string().cyan());
    println!("  receipt_required = {}", cfg.security.receipt_required.to_string().cyan());
    let keep = prompt_default(
        is_tty,
        "  Keep security defaults? [Y/n]: ",
        "y",
    );
    if keep.trim().eq_ignore_ascii_case("n") {
        cfg.security.sandbox = confirm(is_tty, "  Enable sandbox? [Y/n]: ", true);
        cfg.security.workspace_boundary =
            confirm(is_tty, "  Enable workspace_boundary? [Y/n]: ", true);
        cfg.security.ssrf_guard = confirm(is_tty, "  Enable ssrf_guard? [Y/n]: ", true);
        cfg.security.receipt_required =
            confirm(is_tty, "  Enable receipt_required? [Y/n]: ", true);
    } else {
        println!("  {} Security defaults kept.", "✓".green());
    }

    // ── Station 13: Memory defaults ─────────────────────────────────────────
    station(13, "Memory subsystem defaults");
    println!("  episodic={} semantic={} procedural={} dream={} gix={}",
        cfg.memory.episodic.to_string().cyan(),
        cfg.memory.semantic.to_string().cyan(),
        cfg.memory.procedural.to_string().cyan(),
        cfg.memory.dream.to_string().cyan(),
        cfg.memory.gix.to_string().cyan(),
    );
    let keep_mem = prompt_default(is_tty, "  Keep all memory subsystems enabled? [Y/n]: ", "y");
    if keep_mem.trim().eq_ignore_ascii_case("n") {
        cfg.memory.episodic = confirm(is_tty, "  Enable episodic memory? [Y/n]: ", true);
        cfg.memory.semantic = confirm(is_tty, "  Enable semantic memory? [Y/n]: ", true);
        cfg.memory.procedural = confirm(is_tty, "  Enable procedural memory? [Y/n]: ", true);
        cfg.memory.dream = confirm(is_tty, "  Enable dream (REM) consolidation? [Y/n]: ", true);
        cfg.memory.gix = confirm(is_tty, "  Enable GlyphIndex (gix)? [Y/n]: ", true);
    } else {
        println!("  {} Memory defaults kept.", "✓".green());
    }

    // ── Station 14: Write updated config.toml ───────────────────────────────
    station(14, "Write config.toml");
    write_config(&cfg, &config_path)?;
    println!("  {} wrote {}", "✓".green(), config_path.display());

    // ── Station 15: Summary ──────────────────────────────────────────────────
    station(15, "Configuration summary");
    print_summary(&cfg);

    // ── Station 16: Done ─────────────────────────────────────────────────────
    station(16, "Onboarding complete");
    println!(
        "\n  {}",
        "Run `omokoda birth` to birth your sovereign agent."
            .bold()
            .green()
    );
    println!("  Config: {}", config_path.display().to_string().dimmed());
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Print a station header: `\n[Station N/16] Title\n`.
fn station(n: u8, title: &str) {
    println!(
        "\n{} {}\n",
        format!("[Station {n}/16]").bold().yellow(),
        title.bold()
    );
}

/// Read a trimmed line from stdin.
/// In non-tty mode returns an empty string (callers fall through to default).
fn read_line(is_tty: bool) -> String {
    if !is_tty {
        return String::new();
    }
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
    buf.trim().to_string()
}

/// Flush the prompt text then read one line.
/// If the result is empty, return `default`.
fn prompt_default(is_tty: bool, prompt: &str, default: &str) -> String {
    use std::io::Write as _;
    if is_tty {
        print!("{}", prompt);
        let _ = std::io::stdout().flush();
        let line = read_line(is_tty);
        if line.is_empty() {
            default.to_string()
        } else {
            line
        }
    } else {
        println!("  {} {} (non-tty, using default)", prompt.dimmed(), default.cyan());
        default.to_string()
    }
}

/// Yes/no confirmation prompt; returns `default` in non-tty mode.
fn confirm(is_tty: bool, prompt: &str, default: bool) -> bool {
    let answer = prompt_default(is_tty, prompt, if default { "y" } else { "n" });
    !answer.trim().eq_ignore_ascii_case("n")
}

/// Interactive ServiceSection configurator.
fn configure_service(
    is_tty: bool,
    name: &str,
    current: &ServiceSection,
    default_url: &str,
) -> ServiceSection {
    let status = if current.enabled { "enabled" } else { "disabled" };
    let enable = confirm(
        is_tty,
        &format!("  Enable {name}? (currently {status}) [y/N]: "),
        current.enabled,
    );

    // If not enable AND nothing was previously set, keep disabled + empty.
    if !enable {
        println!("  {} {} disabled.", "–".dimmed(), name);
        return ServiceSection {
            enabled: false,
            url: current.url.clone(),
        };
    }

    let url_prompt = format!(
        "  {} URL [{}]: ",
        name,
        if current.url.is_empty() {
            default_url
        } else {
            &current.url
        }
    );
    let fallback = if current.url.is_empty() {
        default_url
    } else {
        &current.url
    };
    let url = prompt_default(is_tty, &url_prompt, fallback);
    println!("  {} {} enabled → {}", "✓".green(), name, url.cyan());
    ServiceSection { enabled: true, url }
}

/// Serialise the config struct to a well-formed TOML string using `toml::to_string_pretty`.
/// We write a minimal header comment then the serialised body.
fn write_config(cfg: &OmokodaConfig, path: &std::path::Path) -> std::io::Result<()> {
    // SPARK: serialise via toml crate; fields absent from the struct get
    // their defaults on next load thanks to #[serde(default)].
    let body = toml::to_string_pretty(cfg)
        .expect("OmokodaConfig must always serialise cleanly (all fields are basic types)");

    let header = "# ~/.omokoda/config.toml — generated by `omokoda onboard`\n\
                  # Edit freely; unknown keys are silently ignored on load.\n\n";

    std::fs::write(path, format!("{header}{body}"))
}

/// Pretty-print a one-page summary of the collected config.
fn print_summary(cfg: &OmokodaConfig) {
    let yes = |b: bool| -> colored::ColoredString {
        if b {
            "yes".green()
        } else {
            "no".red()
        }
    };

    println!("  {:20} {}", "agent.name:", cfg.agent.name.cyan());
    println!("  {:20} {}", "agent.display_name:", cfg.agent.display_name.cyan());
    println!("  {:20} {}", "agent.environment:", cfg.agent.environment.cyan());
    println!(
        "  {:20} {}",
        "model.provider:",
        if cfg.model.provider.is_empty() {
            "(settings.json default)".dimmed()
        } else {
            cfg.model.provider.as_str().cyan()
        }
    );
    println!(
        "  {:20} {}",
        "model.model:",
        if cfg.model.model.is_empty() {
            "(settings.json default)".dimmed()
        } else {
            cfg.model.model.as_str().cyan()
        }
    );
    println!("  {:20} {}", "vantage.enabled:", yes(cfg.vantage.enabled));
    println!("  {:20} {}", "zangbeto.enabled:", yes(cfg.zangbeto.enabled));
    println!("  {:20} {}", "osovm.enabled:", yes(cfg.osovm.enabled));
    println!("  {:20} {}", "ucx.enabled:", yes(cfg.ucx.enabled));
    println!("  {:20} {}", "security.sandbox:", yes(cfg.security.sandbox));
    println!("  {:20} {}", "security.receipt_required:", yes(cfg.security.receipt_required));
    println!("  {:20} {}", "memory.enabled:", yes(cfg.memory.enabled));
    println!("  {:20} gix={} episodic={} semantic={} procedural={} dream={}",
        "memory subsystems:",
        yes(cfg.memory.gix),
        yes(cfg.memory.episodic),
        yes(cfg.memory.semantic),
        yes(cfg.memory.procedural),
        yes(cfg.memory.dream),
    );
}
