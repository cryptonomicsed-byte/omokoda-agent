// ori_cmd.rs — Read-only `omokoda ori` command.
// Displays the agent's current Ori state. Never allows editing vessel weights.

use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use omokoda_core::{HomeDir, Ori};
use std::process;

#[derive(Subcommand, Debug)]
pub enum OriSubcommand {
    /// Display full Ori state (default)
    Show,
    /// Display just the 16 vessel weights as a table
    Vessels,
    /// Recompute state_hash and confirm it matches stored value
    Verify,
}

pub fn run(subcommand: Option<OriSubcommand>) -> Result<()> {
    let home = HomeDir::new();
    let ori_path = home.ori_json();

    if !ori_path.exists() {
        eprintln!(
            "No Ori state found. Run {} to initialize.",
            "`omokoda birth`".yellow()
        );
        process::exit(1);
    }

    let raw = std::fs::read_to_string(&ori_path)?;
    let ori: Ori = serde_json::from_str(&raw).map_err(|e| {
        anyhow::anyhow!(
            "Failed to deserialise Ori from {}: {}",
            ori_path.display(),
            e
        )
    })?;

    match subcommand.unwrap_or(OriSubcommand::Show) {
        OriSubcommand::Show => print_show(&ori),
        OriSubcommand::Vessels => print_vessels(&ori),
        OriSubcommand::Verify => print_verify(&ori),
    }

    Ok(())
}

// ── show ──────────────────────────────────────────────────────────────────────

fn print_show(ori: &Ori) {
    println!(
        "{}",
        format!("Ori State — revision {}", ori.ori_revision)
            .bold()
            .cyan()
    );
    println!(
        "  {:<22} {}",
        "Birth Entropy Hash:".dimmed(),
        ori.birth_entropy_hash
    );
    println!(
        "  {:<22} {}",
        "IfáScript Version:".dimmed(),
        ori.ifascript_version
    );
    println!(
        "  {:<22} {}",
        "Experience Count:".dimmed(),
        ori.experience_count
    );
    println!(
        "  {:<22} {}",
        "Previous Hash:".dimmed(),
        ori.previous_ori_hash
    );
    println!(
        "  {:<22} {}",
        "State Hash:".dimmed(),
        ori.state_hash
    );
    println!();
    println!("{}", "Vessel Weights:".bold());
    print_vessel_table(ori);
}

// ── vessels ───────────────────────────────────────────────────────────────────

fn print_vessels(ori: &Ori) {
    print_vessel_table(ori);
}

// ── verify ────────────────────────────────────────────────────────────────────

fn print_verify(ori: &Ori) {
    let computed = ori.state_hash();
    let stored = &ori.state_hash;

    println!("  {:<16} {}", "Stored hash:".dimmed(), stored);
    println!("  {:<16} {}", "Computed hash:".dimmed(), computed);
    println!();
    if computed == *stored {
        println!(
            "{}",
            "VALID — Ori state has not been tampered with.".green().bold()
        );
    } else {
        println!(
            "{}",
            "TAMPERED — hash mismatch! Ori state may be corrupted."
                .red()
                .bold()
        );
    }
}

// ── shared helper ─────────────────────────────────────────────────────────────

fn print_vessel_table(ori: &Ori) {
    for (i, (name, bullets)) in ori.vessel_display().iter().enumerate() {
        let weight = ori.vessel_weights[i];
        println!(
            "  {:<14} {}  {:.2}",
            name.cyan(),
            bullets,
            weight
        );
    }
}
