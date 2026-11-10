//! Ọ̀ṢỌ́ language tokenizer, parser, and IR.
//!
//! Rust port of `OSOVM/src/oso_compiler.jl`.
//!
//! The Ọ̀ṢỌ́ grammar is an attribute-based DSL:
//!
//! ```text
//! @opcode_name(key=value, key=value)
//! @opcode_name { @inner_attr(...) }
//! ```
//!
//! The parser emits a flat `Vec<IrInstruction>` — the same structure as the
//! Julia `IR = Vector{Instruction}`.  Each instruction carries an opcode byte
//! and a JSON-compatible argument map.

pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod token;

pub use error::{OsoError, OsoResult};
pub use ir::{IrInstruction, IrProgram, IrValue};
pub use lexer::Lexer;
pub use parser::OsoParser;
pub use token::{Token, TokenKind};

/// Convenience: tokenize + parse in one call.
pub fn compile(source: &str) -> OsoResult<IrProgram> {
    let tokens = Lexer::new(source).tokenize()?;
    OsoParser::new(tokens).parse()
}
