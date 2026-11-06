//! oso-security — formal security analyzer CLI.
//!
//! Usage:
//!   oso-security analyze <program.json> [--strict]
//!   oso-security analyze <program.json> --pipeline --target testnet --signer-tier 3

use clap::{Parser, Subcommand};
use oso_security::{analyze, SecurityPipeline};

#[derive(Parser, Debug)]
#[command(
    name = "oso-security",
    about = "Formal capability, resource, and liveness analyzer for OSO-IR programs (Phase 26.5/26.7)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run formal security analysis on an OSO-IR program.
    Analyze {
        /// Path to the OSO-IR JSON file
        program: String,

        /// Enable strict mode: resource warnings become pass/fail errors
        #[arg(long)]
        strict: bool,

        /// Run the full 8-step security pipeline instead of standalone analysis
        #[arg(long)]
        pipeline: bool,

        /// Deployment target when using --pipeline (local|testnet|mainnet)
        #[arg(long, default_value = "local")]
        target: String,

        /// Signer tier (0–7) when using --pipeline
        #[arg(long, default_value = "0")]
        signer_tier: u8,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Analyze {
            program,
            strict,
            pipeline,
            target,
            signer_tier,
        } => {
            let json_text = std::fs::read_to_string(&program).unwrap_or_else(|e| {
                eprintln!("error reading {}: {}", program, e);
                std::process::exit(1);
            });

            if pipeline {
                let result = SecurityPipeline::run(&json_text, &target, signer_tier);
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                if !result.passed {
                    std::process::exit(1);
                }
            } else {
                let report = analyze(&json_text, strict);
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                if !report.passed {
                    std::process::exit(1);
                }
            }
        }
    }
}
