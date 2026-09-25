//! dapp-backend — Phase 27 reference dApp agent CLI.
//!
//! Usage:
//!   dapp-backend gpu-market
//!   dapp-backend agent-hiring
//!   dapp-backend sim-market
//!   dapp-backend all

use clap::{Parser, Subcommand};
use reference_dapps_agent::{DappBackend, contracts};

#[derive(Parser, Debug)]
#[command(name = "dapp-backend", about = "Phase 27 Ọ̀ṢỌ́ reference dApp compiler (Phase 27.1-27.3)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// GPU Compute Marketplace (dApp 1)
    GpuMarket,
    /// Agent Employment (dApp 2)
    AgentHiring,
    /// Simulation Marketplace (dApp 3)
    SimMarket,
    /// Compile and validate all three reference dApps
    All,
}

fn main() {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::GpuMarket => {
            let r = DappBackend::compile_and_validate(contracts::GPU_MARKETPLACE_OSO);
            print_result(&r);
        }
        Cmd::AgentHiring => {
            let r = DappBackend::compile_and_validate(contracts::AGENT_EMPLOYMENT_OSO);
            print_result(&r);
        }
        Cmd::SimMarket => {
            let r = DappBackend::compile_and_validate(contracts::SIM_MARKETPLACE_OSO);
            print_result(&r);
        }
        Cmd::All => {
            let results = DappBackend::compile_all();
            let mut all_pass = true;
            for r in &results {
                print_result(r);
                if !r.valid { all_pass = false; }
                println!();
            }
            if !all_pass {
                std::process::exit(1);
            }
        }
    }
}

fn print_result(r: &reference_dapps_agent::CompileResult) {
    println!(
        "{} {}  errors={}  warnings={}",
        if r.valid { "✓" } else { "✗" },
        r.dapp_name,
        r.errors.len(),
        r.warnings.len(),
    );
    for e in &r.errors   { println!("  ERR: {}", e); }
    for w in &r.warnings { println!("  WARN: {}", w); }
    if r.valid {
        if let Some(ir) = &r.ir {
            println!(
                "  class={:?}  actions={}  capabilities={}",
                ir.contract_class, ir.actions.len(), ir.capabilities.len()
            );
        }
    }
    println!("{}", serde_json::to_string_pretty(r).unwrap_or_default());
}
