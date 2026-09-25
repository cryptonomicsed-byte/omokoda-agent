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
        /// Port to listen on
        #[arg(short, long, default_value_t = 7777)]
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
        let ori: omokoda_core::Ori = serde_json::from_str(&contents)
            .with_context(|| "parsing ori.json")?;

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
    let path = omokoda_home().join("constitution").join("constitution.json");

    if !path.exists() {
        println!("No constitution found. Run `omokoda birth` to initialize.");
        return Ok(());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;

    if raw {
        println!("{}", contents);
        return Ok(());
    }

    let v: serde_json::Value = serde_json::from_str(&contents)
        .with_context(|| "parsing constitution.json as JSON")?;

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
    println!("{:<22} {}", "Archetype:".bold(), str_field("behavioral_archetype"));
    println!("{:<22} {}", "Nostr pubkey:".bold(), str_field("nostr_pubkey_hex"));
    println!("{:<22} {}", "Sui soul object:".bold(),
        v.get("sui_soul_object_id")
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(not set)"));
    println!("{:<22} {}", "Birth timestamp:".bold(), num_field("birth_timestamp"));
    println!("{:<22} {}", "BTC block height:".bold(),
        v.get("birth_btc_height")
            .and_then(|x| if x.is_null() { None } else { Some(x.to_string()) })
            .unwrap_or_else(|| "(not set)".to_string()));
    println!("{:<22} {}", "Gate alignment seed:".bold(), num_field("gate_alignment_seed"));
    println!("{:<22} {}", "Koodu birth score:".bold(), num_field("koodu_birth_score"));

    if let Some(dna) = v.get("hermetic_dna").and_then(|x| x.as_array()) {
        let dna_str: Vec<String> = dna.iter().map(|x| x.to_string()).collect();
        println!("{:<22} [{}]", "Hermetic DNA:".bold(), dna_str.join(", "));
    }

    println!("{:<22} {}", "Signature:".bold(),
        v.get("signature_hex")
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(unsigned)"));

    // Show BIPON39 phrase — abbreviated for security
    let phrase = str_field("bipon39_phrase");
    let words: Vec<&str> = phrase.split_whitespace().collect();
    let abbreviated = if words.len() > 3 {
        format!("{} {} {} ... ({} words)", words[0], words[1], words[2], words.len())
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

fn cmd_identity(raw: bool) -> Result<()> {
    let path = omokoda_home().join("identity").join("identity.json");

    if !path.exists() {
        println!("No identity found. Run `omokoda birth` to initialize.");
        return Ok(());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;

    if raw {
        println!("{}", contents);
        return Ok(());
    }

    let v: serde_json::Value = serde_json::from_str(&contents)
        .with_context(|| "parsing identity.json as JSON")?;

    let str_field = |key: &str| -> String {
        v.get(key)
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string()
    };

    println!("{}", "Agent Identity".bold().cyan());
    println!("{}", "─".repeat(40).dimmed());
    println!("{:<22} {}", "Name:".bold(),        str_field("name"));
    println!("{:<22} {}", "Agent ID:".bold(),    str_field("agent_id"));
    println!("{:<22} {}", "Birth Hash:".bold(),  str_field("birth_hash"));
    println!("{:<22} {}", "Nostr pubkey:".bold(),
        v.get("nostr_pubkey")
            .or_else(|| v.get("nostr_pubkey_hex"))
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(not set)"));
    println!("{:<22} {}", "Sui address:".bold(),
        v.get("sui_address")
            .or_else(|| v.get("sui_soul_object_id"))
            .and_then(|x| if x.is_null() { None } else { x.as_str() })
            .unwrap_or("(not set)"));

    println!();
    println!("{}", "Tip: use --raw to print full JSON.".dimmed());
    Ok(())
}
