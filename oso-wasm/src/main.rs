#![allow(dead_code)]

mod codegen;
mod host_env;
mod ir;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "oso-wasm", about = "Ọ̀ṢỌ́-IR → WebAssembly Text (WAT) compiler")]
struct Cli {
    /// OSO-IR JSON input file (or - for stdin)
    input: PathBuf,
    /// Output WAT file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Validate only, do not emit WAT
    #[arg(long)]
    check: bool,
}

fn main() {
    let cli = Cli::parse();

    let src = if cli.input.to_str() == Some("-") {
        std::io::read_to_string(std::io::stdin()).expect("reading stdin")
    } else {
        std::fs::read_to_string(&cli.input)
            .unwrap_or_else(|e| panic!("cannot read {:?}: {e}", cli.input))
    };

    let ir: ir::OsoIR = serde_json::from_str(&src).unwrap_or_else(|e| {
        eprintln!("JSON parse error: {e}");
        std::process::exit(1);
    });

    if cli.check {
        println!("OK — {:?} parses as OsoIR (contract_class={})", cli.input, ir.contract_class);
        return;
    }

    let wat = codegen::WasmCodegen::compile(&ir).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });

    match cli.output {
        Some(path) => std::fs::write(&path, &wat)
            .unwrap_or_else(|e| panic!("cannot write {:?}: {e}", path)),
        None => print!("{wat}"),
    }
}
