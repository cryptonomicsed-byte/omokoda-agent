//! oso-move — OSO-IR → Move compiler CLI
//!
//! Usage:
//!   oso-move compile <program.json> --output <out.move>
//!   oso-move compile <program.json>               (prints to stdout)

#![allow(dead_code)]

mod codegen;
mod host_env;
mod ir;

#[cfg(test)]
mod tests;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "oso-move",
    version = "0.1.0",
    about = "Compile OSO-IR contract schemas to Sui Move modules"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile an OSO-IR JSON file to a Move module.
    Compile {
        /// Path to the OSO-IR JSON input file.
        input: PathBuf,

        /// Path to write the generated .move file.
        /// If omitted, output is written to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { input, output } => {
            run_compile(input, output);
        }
    }
}

fn run_compile(input: PathBuf, output: Option<PathBuf>) {
    // Read input JSON
    let json_bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read '{}': {}", input.display(), e);
            std::process::exit(1);
        }
    };

    // Deserialize OSO-IR
    let ir: ir::OsoIR = match serde_json::from_slice(&json_bytes) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: invalid OSO-IR JSON in '{}': {}", input.display(), e);
            std::process::exit(1);
        }
    };

    // Compile
    let move_src = match codegen::MoveCodegen::compile(&ir) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("compile error: {}", e);
            std::process::exit(1);
        }
    };

    // Write output
    match output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("error: cannot create output directory: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            match std::fs::write(&path, &move_src) {
                Ok(_) => {
                    eprintln!("written: {}", path.display());
                }
                Err(e) => {
                    eprintln!("error: cannot write '{}': {}", path.display(), e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            print!("{}", move_src);
        }
    }
}
