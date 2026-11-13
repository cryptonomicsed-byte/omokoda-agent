mod doctor;
mod onboard;
mod ori_cmd;
mod tui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use omokoda_core::{
    interpreter::Steward,
    parser::{parse, Statement, ThinkModifiers},
    AgentId,
};
use rustyline::{error::ReadlineError, DefaultEditor};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "omokoda", version, about = "Ọmọ Kọ́dà sovereign agent CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Birth a new sovereign agent
    Birth {
        /// Agent name
        name: String,
        /// Optional metadata as key=value pairs
        #[arg(short, long, value_parser = parse_kv)]
        meta: Vec<(String, String)>,
    },
    /// Send a think primitive to the active agent
    Think {
        /// Prompt text
        prompt: String,
        /// Private mode (local provider only)
        #[arg(short, long)]
        private: bool,
    },
    /// Execute an act primitive via the active agent
    Act {
        /// Tool name
        tool: String,
        /// Tool parameters (JSON or plain string)
        #[arg(default_value = "{}")]
        params: String,
        /// Enable sandbox mode
        #[arg(short, long)]
        sandbox: bool,
    },
    /// Run a .swibe script file
    Run {
        /// Path to .swibe script, or "-" / "--stdin" to read from stdin
        #[arg(default_value = "-")]
        script: String,
        /// Read from stdin (alias for script="-")
        #[arg(long)]
        stdin: bool,
    },
    /// Print agent status
    Status,
    /// Start the interactive REPL
    Repl,
    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Start the HTTP/SSE server
    Serve {
        /// Port to listen on (7777 is ares-buzz-relay on the VPS)
        #[arg(short, long, default_value_t = 7778)]
        port: u16,
    },
    /// Launch the full-screen TUI
    Tui,
    /// Audit the local agent environment
    Doctor,
    /// Display the agent's constitution (read-only)
    Constitution {
        /// Print raw JSON instead of a human-readable summary
        #[arg(short, long)]
        raw: bool,
    },
    /// Display the agent's identity (read-only)
    Identity {
        /// Print raw JSON instead of a human-readable summary
        #[arg(short, long)]
        raw: bool,
    },
    /// Display the agent's Ori state (read-only)
    Ori {
        #[command(subcommand)]
        subcommand: Option<ori_cmd::OriSubcommand>,
    },
    /// Run the 16-station first-time setup wizard
    Onboard,
    /// Agent birth certificate (SID) — emit, verify, or print canonical form
    Sid {
        #[command(subcommand)]
        action: SidAction,
    },
}

#[derive(Subcommand)]
enum SidAction {
    /// Project the steward's genesis receipt into a sid/0.1 certificate
    Emit {
        /// Write cert.json + SID.md here (omit to print JSON)
        #[arg(short, long)]
        out: Option<String>,
        /// Name override (defaults to the receipt's symbolic address)
        #[arg(long)]
        name: Option<String>,
        /// Mycelium trace chain head to bind into provenance
        #[arg(long, env = "MYCELIUM_CHAIN_HEAD")]
        trace_head: Option<String>,
        /// Runtime profile id to bind into provenance
        #[arg(long, env = "OMOKODA_PROFILE_ID")]
        profile_id: Option<String>,
    },
    /// Verify a certificate (accepts cert JSON or SID.md)
    Verify { path: String },
    /// Print the canonical JSON and both hashes
    Canonical { path: String },
    /// Apply an OKF anchor artifact, minting the chained v2 (cast -> live)
    Anchor {
        /// Path to the cast certificate JSON
        #[arg(long)]
        cert: String,
        /// Path to the OKF anchor artifact (cert.anchor.json)
        #[arg(long)]
        anchor: String,
        /// Write v2 cert.json + SID.md here (default: alongside --cert)
        #[arg(short, long)]
        out: Option<String>,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List persisted sessions
    List,
    /// Resume a session by agent-id prefix
    Resume { id: String },
    /// Archive (seal) the current session
    Archive,
}

fn parse_kv(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("expected key=value, got '{s}'"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Birth { name, meta }) => {
            let mut steward = Steward::new();
            run_statement(
                &mut steward,
                Statement::Birth {
                    name,
                    metadata: meta
                        .into_iter()
                        .map(|(k, v)| omokoda_core::parser::MetadataPair { key: k, value: v })
                        .collect(),
                },
            )
            .await?;
        }

        Some(Command::Think { prompt, private }) => {
            let mut steward = load_or_new_steward();
            run_statement(
                &mut steward,
                Statement::Think {
                    prompt,
                    private,
                    modifiers: ThinkModifiers::default(),
                },
            )
            .await?;
        }

        Some(Command::Act {
            tool,
            params,
            sandbox,
        }) => {
            let mut steward = load_or_new_steward();
            run_statement(
                &mut steward,
                Statement::Act {
                    tool,
                    params,
                    sandbox,
                },
            )
            .await?;
        }

        Some(Command::Run { script, stdin }) => {
            let source = if stdin || script == "-" {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            } else {
                std::fs::read_to_string(&script)
                    .with_context(|| format!("reading script '{script}'"))?
            };
            run_script(source).await?;
        }

        Some(Command::Status) => {
            let mut steward = load_or_new_steward();
            run_slash(&mut steward, "status", None).await?;
            cmd_status_extra()?;
        }

        Some(Command::Repl) | None => {
            repl().await?;
        }

        Some(Command::Serve { port }) => {
            omokoda_core::server::start_server(port)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Some(Command::Tui) => {
            tui::run()?;
        }

        Some(Command::Doctor) => {
            doctor::run();
        }

        Some(Command::Constitution { raw }) => {
            cmd_constitution(raw)?;
        }

        Some(Command::Identity { raw }) => {
            cmd_identity(raw)?;
        }

        Some(Command::Ori { subcommand }) => {
            ori_cmd::run(subcommand)?;
        }

        Some(Command::Onboard) => {
            onboard::run()?;
        }

        Some(Command::Sid { action }) => {
            cmd_sid(action)?;
        }

        Some(Command::Session { action }) => match action {
            SessionAction::List => session_list()?,
            SessionAction::Resume { id } => session_resume(&id).await?,
            SessionAction::Archive => {
                let mut steward = load_or_new_steward();
                run_slash(&mut steward, "seal", None).await?;
            }
        },
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// REPL
// ---------------------------------------------------------------------------

async fn repl() -> Result<()> {
    println!(
        "{}",
        "Ọmọ Kọ́dà  •  Àṣẹ CLI  •  type 'help' or Ctrl-D to exit"
            .bold()
            .cyan()
    );
    let mut rl = DefaultEditor::new()?;
    let mut steward = load_or_new_steward();

    loop {
        let prompt = agent_prompt(&steward);
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(&line);
                if line == "exit" || line == "quit" {
                    break;
                }
                if let Err(e) = handle_repl_line(&mut steward, &line).await {
                    eprintln!("{} {}", "Error:".red(), e);
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("{}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn handle_repl_line(steward: &mut Steward, line: &str) -> Result<()> {
    if let Some(stripped) = line.strip_prefix('/') {
        let mut parts = stripped.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().map(|s| s.to_string());
        run_slash(steward, cmd, arg).await
    } else {
        let stmts = parse(line).map_err(|e| anyhow::anyhow!("{}", e))?;
        for stmt in stmts {
            run_statement(steward, stmt).await?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Script runner
// ---------------------------------------------------------------------------

async fn run_script(source: String) -> Result<()> {
    let mut steward = load_or_new_steward();
    let stmts = parse(&source).map_err(|e| anyhow::anyhow!("{}", e))?;
    for stmt in stmts {
        run_statement(&mut steward, stmt).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Core dispatch
// ---------------------------------------------------------------------------

async fn run_statement(steward: &mut Steward, stmt: Statement) -> Result<()> {
    let result = steward
        .dispatch(stmt)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(output) = result.tool_output {
        if result.private_mode {
            println!("{}", output.dimmed());
        } else {
            println!("{}", output);
        }
    }
    if let Some(receipt) = result.receipt {
        println!("{}  {}", "receipt:".dimmed(), receipt.receipt_id.dimmed());
    }
    Ok(())
}

async fn run_slash(steward: &mut Steward, cmd: &str, arg: Option<String>) -> Result<()> {
    use omokoda_core::parser::Statement;
    let stmt = Statement::SlashCmd {
        command: cmd.to_string(),
        arg,
    };
    run_statement(steward, stmt).await
}

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

fn session_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omokoda")
        .join("sessions")
}

fn load_or_new_steward() -> Steward {
    let mut steward = Steward::new();
    steward.try_load_owner();
    // The escalation prompter existed but was never installed by the CLI, so a
    // Prompt-mode agent (tier 3-4 — the default once reputed) could never be
    // approved interactively. Install it for terminal sessions only; scripted
    // and piped runs stay fail-closed.
    {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() {
            steward.permission_prompter = Some(Box::new(
                omokoda_core::execution::prompter::TerminalPrompter::new("omokoda"),
            ));
        }
    }
    steward
}

fn session_list() -> Result<()> {
    let dir = session_dir();
    if !dir.exists() {
        println!("No sessions found ({})", dir.display());
        return Ok(());
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name();
        println!("  {}", name.to_string_lossy().cyan());
        count += 1;
    }
    if count == 0 {
        println!("No sessions found.");
    }
    Ok(())
}

async fn session_resume(id: &str) -> Result<()> {
    println!("{} {}", "Resuming session".yellow(), id);

    // Resolve a (possibly partial) id against the session dir, same
    // convention as `session list`, so `resume q2m2` works like `resume
    // agent-q2m2RdlB3qEnn1ib`.
    let dir = session_dir();
    let mut matches: Vec<String> = Vec::new();
    if dir.exists() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains(id) {
                matches.push(name);
            }
        }
    }

    let resolved_id = match matches.len() {
        0 => {
            println!("{} no session matches '{}'", "Error:".red(), id);
            return Ok(());
        }
        1 => matches.remove(0),
        _ => {
            println!(
                "{} ambiguous id '{}' matches {} sessions: {}",
                "Error:".red(),
                id,
                matches.len(),
                matches.join(", ")
            );
            return Ok(());
        }
    };

    let mut steward = Steward::new();
    let agent_id = AgentId::from_str(&resolved_id);
    if let Err(e) = steward.load_agent(&agent_id) {
        println!("{} failed to load {}: {}", "Error:".red(), resolved_id, e);
        return Ok(());
    }
    run_slash(&mut steward, "status", None).await
}

fn agent_prompt(steward: &Steward) -> String {
    if let Some(agent) = steward.agent_core() {
        format!("{}> ", agent.name().cyan())
    } else {
        "omokoda> ".to_string()
    }
}

// ---------------------------------------------------------------------------
// Status extra sections (Ori + Config)
// ---------------------------------------------------------------------------

fn cmd_status_extra() -> Result<()> {
    let home = omokoda_core::HomeDir::new();

    // ── Ori State ────────────────────────────────────────────────────────────
    println!();
    println!(
        "{}",
        "── Ori State ──────────────────────────────────────────".cyan()
    );

    let ori_path = home.ori_json();
    if !ori_path.exists() {
        println!("  Ori: not initialized (run `omokoda birth`)");
    } else {
        let contents = std::fs::read_to_string(&ori_path)
            .with_context(|| format!("reading {}", ori_path.display()))?;
        let ori: omokoda_core::Ori =
            serde_json::from_str(&contents).with_context(|| "parsing ori.json")?;

        let state_hash_short = if ori.state_hash.len() > 16 {
            format!("{}...", &ori.state_hash[..16])
        } else {
            ori.state_hash.clone()
        };
        let birth_hash_short = if ori.birth_entropy_hash.len() > 16 {
            format!("{}...", &ori.birth_entropy_hash[..16])
        } else {
            ori.birth_entropy_hash.clone()
        };

        println!("  {:<16} {}", "Revision:".bold(), ori.ori_revision);
        println!("  {:<16} {}", "Experience:".bold(), ori.experience_count);
        println!("  {:<16} {}", "State Hash:".bold(), state_hash_short);
        println!("  {:<16} {}", "Birth Hash:".bold(), birth_hash_short);
        println!();
        println!("  {}", "Top Vessels:".bold());

        // Sort vessels by weight descending, take top 5
        let display = ori.vessel_display();
        let mut indexed: Vec<(usize, &(String, String))> = display.iter().enumerate().collect();
        indexed.sort_by(|a, b| {
            let wa = ori.vessel_weights[a.0];
            let wb = ori.vessel_weights[b.0];
            wb.partial_cmp(&wa).unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, (name, bullets)) in indexed.iter().take(5) {
            let weight = ori.vessel_weights[*i];
            println!("    {:<12} {}  {:.2}", name, bullets, weight);
        }
    }

    // ── Config ───────────────────────────────────────────────────────────────
    println!();
    println!(
        "{}",
        "── Config ─────────────────────────────────────────────".cyan()
    );

    let cfg = omokoda_core::OmokodaConfig::load_or_default(&home.config_toml());

    let enabled_str = |b: bool| if b { "enabled" } else { "disabled" };

    let name_val = if cfg.agent.name.is_empty() {
        "(not set)".to_string()
    } else {
        cfg.agent.name.clone()
    };
    let provider_val = if cfg.model.provider.is_empty() {
        "(not set)".to_string()
    } else {
        cfg.model.provider.clone()
    };
    let env_val = if cfg.agent.environment.is_empty() {
        "(not set)".to_string()
    } else {
        cfg.agent.environment.clone()
    };
    let mode_val = if cfg.agent.mode.is_empty() {
        "(not set)".to_string()
    } else {
        cfg.agent.mode.clone()
    };

    println!("  {:<16} {}", "Name:".bold(), name_val);
    println!("  {:<16} {}", "Environment:".bold(), env_val);
    println!("  {:<16} {}", "Mode:".bold(), mode_val);
    println!("  {:<16} {}", "Provider:".bold(), provider_val);
    println!(
        "  {:<16} {}",
        "Vantage:".bold(),
        enabled_str(cfg.vantage.enabled)
    );
    println!(
        "  {:<16} {}",
        "OSOVM:".bold(),
        enabled_str(cfg.osovm.enabled)
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Constitution command
// ---------------------------------------------------------------------------

fn omokoda_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".omokoda")
}

fn cmd_constitution(raw: bool) -> Result<()> {
    let path = omokoda_home()
        .join("constitution")
        .join("constitution.json");

    if !path.exists() {
        println!("No constitution found. Run `omokoda birth` to initialize.");
        return Ok(());
    }

    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;

    if raw {
        println!("{}", contents);
        return Ok(());
    }

    let v: serde_json::Value =
        serde_json::from_str(&contents).with_context(|| "parsing constitution.json as JSON")?;

    let pretty = serde_json::to_string_pretty(&v)?;

    println!("{}", "Agent Constitution".bold().cyan());
    println!("{}", "─".repeat(40).dimmed());

    let str_field = |key: &str| -> String {
        v.get(key)
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string()
    };
    let num_field = |key: &str| -> String {
        v.get(key)
            .map(|x| x.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    };

    println!("{:<22} {}", "Odu:".bold(), str_field("odu_name"));
    println!("{:<22} {}", "Odu Index:".bold(), num_field("odu_index"));
    println!(
        "{:<22} {}",
        "Archetype:".bold(),
        str_field("behavioral_archetype")
    );
    println!(
        "{:<22} {}",
        "Nostr pubkey:".bold(),
        str_field("nostr_pubkey_hex")
    );
    println!(
        "{:<22} {}",
        "Sui soul object:".bold(),
        v.get("sui_soul_object_id")
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(not set)")
    );
    println!(
        "{:<22} {}",
        "Birth timestamp:".bold(),
        num_field("birth_timestamp")
    );
    println!(
        "{:<22} {}",
        "BTC block height:".bold(),
        v.get("birth_btc_height")
            .and_then(|x| if x.is_null() {
                None
            } else {
                Some(x.to_string())
            })
            .unwrap_or_else(|| "(not set)".to_string())
    );
    println!(
        "{:<22} {}",
        "Gate alignment seed:".bold(),
        num_field("gate_alignment_seed")
    );
    println!(
        "{:<22} {}",
        "Koodu birth score:".bold(),
        num_field("koodu_birth_score")
    );

    if let Some(dna) = v.get("hermetic_dna").and_then(|x| x.as_array()) {
        let dna_str: Vec<String> = dna.iter().map(|x| x.to_string()).collect();
        println!("{:<22} [{}]", "Hermetic DNA:".bold(), dna_str.join(", "));
    }

    println!(
        "{:<22} {}",
        "Signature:".bold(),
        v.get("signature_hex")
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(unsigned)")
    );

    // Show BIPON39 phrase — abbreviated for security
    let phrase = str_field("bipon39_phrase");
    let words: Vec<&str> = phrase.split_whitespace().collect();
    let abbreviated = if words.len() > 3 {
        format!(
            "{} {} {} ... ({} words)",
            words[0],
            words[1],
            words[2],
            words.len()
        )
    } else {
        phrase.clone()
    };
    println!("{:<22} {}", "BIPON39 phrase:".bold(), abbreviated.dimmed());

    // Hint: use --raw for full JSON
    println!();
    println!("{}", "Tip: use --raw to print full JSON.".dimmed());
    let _ = pretty; // pretty is computed but only used in --raw path implicitly
    Ok(())
}

// ---------------------------------------------------------------------------
// Identity command
// ---------------------------------------------------------------------------

/// SID — the agent's birth certificate, projected from its genesis receipt.
///
/// The kernel already performs the birth ceremony; this command renders that
/// receipt as a portable, hash-chained document and verifies one. It never
/// mints identity on its own: no receipt, no certificate.
fn cmd_sid(action: SidAction) -> Result<()> {
    use omokoda_core::identity::sid;

    match action {
        SidAction::Emit {
            out,
            name,
            trace_head,
            profile_id,
        } => {
            let steward = load_or_new_steward();
            let receipt = steward.genesis_receipt().context(
                "no genesis receipt on this steward — run `omokoda birth` first",
            )?;
            // Bind every subsystem that is actually present: the manifest
            // (capabilities + minipae memory + network), the mycelium trace
            // head when supplied, and the runtime profile.
            let ctx = sid::SidContext {
                manifest: steward.agent_manifest(),
                trace_chain_head: trace_head.as_deref(),
                profile_id: profile_id.as_deref(),
                waggle_field: None,
            };
            let cert = sid::from_genesis_receipt_with(receipt, name.as_deref(), &ctx)
                .map_err(anyhow::Error::msg)?;
            let changelog: Vec<String> = cert["changelog"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            match out {
                Some(dir) => {
                    std::fs::create_dir_all(&dir)
                        .with_context(|| format!("creating {dir}"))?;
                    let cpath = std::path::Path::new(&dir).join("cert.json");
                    let mpath = std::path::Path::new(&dir).join("SID.md");
                    std::fs::write(&cpath, serde_json::to_string_pretty(&cert)?)
                        .with_context(|| format!("writing {}", cpath.display()))?;
                    std::fs::write(&mpath, sid::emit_md(&cert, &changelog).map_err(anyhow::Error::msg)?)
                        .with_context(|| format!("writing {}", mpath.display()))?;
                    println!("{}", "SID certificate".bold().cyan());
                    println!("{}", "─".repeat(40).dimmed());
                    println!("{:<14} {}", "id:".bold(), cert["id"].as_str().unwrap_or("?"));
                    println!("{:<14} {}", "status:".bold(), cert["status"].as_str().unwrap_or("?"));
                    println!("{:<14} {}", "hash:".bold(), cert["hash"].as_str().unwrap_or("?"));
                    println!("{:<14} {}", "odu:".bold(), format!(
                        "{}/{}/{}",
                        cert["identity"]["odu_primary"],
                        cert["identity"]["odu_secondary"],
                        cert["identity"]["odu_link"]
                    ));
                    println!("wrote {} and {}", cpath.display(), mpath.display());
                }
                None => println!("{}", serde_json::to_string_pretty(&cert)?),
            }
        }
        SidAction::Verify { path } => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {path}"))?;
            let cert: serde_json::Value = if raw.trim_start().starts_with('{') {
                serde_json::from_str(&raw).with_context(|| format!("{path} is not JSON"))?
            } else {
                sid::parse_md(&raw).map_err(anyhow::Error::msg).with_context(|| format!("{path} is not SID.md"))?
            };
            let problems = sid::verify(&cert);
            if problems.is_empty() {
                println!(
                    "{} {} — certificate verifies",
                    "OK".green().bold(),
                    cert["id"].as_str().unwrap_or("?")
                );
            } else {
                for p in &problems {
                    println!("  - {p}");
                }
                anyhow::bail!("certificate failed verification ({} problem(s))", problems.len());
            }
        }
        SidAction::Anchor { cert, anchor, out } => {
            let cert_json = std::fs::read_to_string(&cert)
                .with_context(|| format!("reading {cert}"))?;
            let mut v1: serde_json::Value = serde_json::from_str(&cert_json)
                .with_context(|| format!("{cert} is not JSON"))?;
            let anchor_json = std::fs::read_to_string(&anchor)
                .with_context(|| format!("reading {anchor}"))?;
            let a: serde_json::Value = serde_json::from_str(&anchor_json)
                .with_context(|| format!("{anchor} is not JSON"))?;

            // The proof must be over THIS certificate — an anchor for a
            // different hash proves nothing about this document.
            let cert_hash = v1.get("hash").and_then(|v| v.as_str()).unwrap_or("");
            let anchor_hash = a.get("cert_hash").and_then(|v| v.as_str()).unwrap_or("");
            if cert_hash.is_empty() || cert_hash != anchor_hash {
                anyhow::bail!(
                    "anchor is for {} but the certificate is {} — refusing",
                    if anchor_hash.is_empty() { "an unknown hash" } else { anchor_hash },
                    if cert_hash.is_empty() { "(no hash)" } else { cert_hash }
                );
            }
            let ots_proof = a
                .get("ots_proof")
                .and_then(|v| v.as_str())
                .context("the anchor carries no ots_proof — the clock has not stamped this")?;
            let ots_status = a
                .get("ots_status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let block = a
                .get("anchor_block")
                .and_then(|v| v.as_u64())
                .context("the anchor carries no anchor_block — the clock has not verified a tip")?;
            let block_hash = a.get("anchor_block_hash").and_then(|v| v.as_str());

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let at = omokoda_core::identity::sid::rfc3339_from_millis(now_ms);

            let v2 = sid::anchored_v2(&v1, ots_proof, ots_status, block, block_hash, &at)
                .map_err(anyhow::Error::msg)?;
            let changelog: Vec<String> = v2["changelog"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            let dir = out.unwrap_or_else(|| {
                std::path::Path::new(&cert)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".into())
            });
            std::fs::create_dir_all(&dir).with_context(|| format!("creating {dir}"))?;
            let cpath = std::path::Path::new(&dir).join("cert.json");
            let mpath = std::path::Path::new(&dir).join("SID.md");
            std::fs::write(&cpath, serde_json::to_string_pretty(&v2)?)
                .with_context(|| format!("writing {}", cpath.display()))?;
            std::fs::write(&mpath, sid::emit_md(&v2, &changelog).map_err(anyhow::Error::msg)?)
                .with_context(|| format!("writing {}", mpath.display()))?;
            v1 = v2.clone();

            println!("{}", "SID anchored".bold().cyan());
            println!("{}", "─".repeat(40).dimmed());
            println!("{:<14} {}  (was cast)", "status:".bold(), v2["status"].as_str().unwrap_or("?"));
            println!("{:<14} v{}  previous_hash {}…", "version:".bold(),
                v2["version"], &v2["previous_hash"].as_str().unwrap_or("")[..12.min(v2["previous_hash"].as_str().unwrap_or("").len())]);
            println!("{:<14} block {} ({})", "clock:".bold(), block, ots_status);
            println!("{:<14} e{} c{} p{}", "calendar:".bold(),
                v2["time"]["calendar"]["epoch"], v2["time"]["calendar"]["cycle"], v2["time"]["calendar"]["phase"]);
            println!("wrote {} and {}", cpath.display(), mpath.display());
        }
        SidAction::Canonical { path } => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {path}"))?;
            let cert: serde_json::Value =
                serde_json::from_str(&raw).with_context(|| format!("{path} is not JSON"))?;
            println!("{}", sid::canonical_json(&cert).map_err(anyhow::Error::msg)?);
            println!("hash        {}", sid::sid_hash(&cert).map_err(anyhow::Error::msg)?);
            println!("acting_hash {}", sid::acting_hash(&cert).map_err(anyhow::Error::msg)?);
        }
    }
    Ok(())
}

fn cmd_identity(raw: bool) -> Result<()> {
    let path = omokoda_home().join("identity").join("identity.json");

    if !path.exists() {
        println!("No identity found. Run `omokoda birth` to initialize.");
        return Ok(());
    }

    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;

    if raw {
        println!("{}", contents);
        return Ok(());
    }

    let v: serde_json::Value =
        serde_json::from_str(&contents).with_context(|| "parsing identity.json as JSON")?;

    let str_field = |key: &str| -> String {
        v.get(key)
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string()
    };

    println!("{}", "Agent Identity".bold().cyan());
    println!("{}", "─".repeat(40).dimmed());
    println!("{:<22} {}", "Name:".bold(), str_field("name"));
    println!("{:<22} {}", "Agent ID:".bold(), str_field("agent_id"));
    println!("{:<22} {}", "Birth Hash:".bold(), str_field("birth_hash"));
    println!(
        "{:<22} {}",
        "Nostr pubkey:".bold(),
        v.get("nostr_pubkey")
            .or_else(|| v.get("nostr_pubkey_hex"))
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(not set)")
    );
    println!(
        "{:<22} {}",
        "Sui address:".bold(),
        v.get("sui_address")
            .or_else(|| v.get("sui_soul_object_id"))
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(not set)")
    );

    println!();
    println!("{}", "Tip: use --raw to print full JSON.".dimmed());
    Ok(())
}
