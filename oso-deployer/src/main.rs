//! oso-deploy — authorization-tiered deployment pipeline CLI.
//!
//! Usage:
//!   oso-deploy <program.json> --target local
//!   oso-deploy <program.json> --target testnet --signer-tier 2
//!   oso-deploy <program.json> --target mainnet --signer-tier 5

mod auth;
mod manifest;
mod pipeline;

use clap::Parser;
use pipeline::DeployPipeline;

#[derive(Parser, Debug)]
#[command(
    name = "oso-deploy",
    about = "Authorization-tiered OSO-IR deployment pipeline (Phase 26.6)"
)]
struct Cli {
    /// Path to the OSO-IR JSON program file
    program: String,

    /// Deployment target: local | testnet | mainnet
    #[arg(long, default_value = "local")]
    target: String,

    /// Signer tier (0–7)
    #[arg(long, default_value = "0")]
    signer_tier: u8,
}

fn main() {
    let cli = Cli::parse();

    let json_text = std::fs::read_to_string(&cli.program).unwrap_or_else(|e| {
        eprintln!("error reading {}: {}", cli.program, e);
        std::process::exit(1);
    });

    let output = DeployPipeline::run(&json_text, &cli.target, cli.signer_tier);
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    if !output.passed {
        std::process::exit(1);
    }
}
