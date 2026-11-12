use thiserror::Error;

pub type OsoResult<T> = Result<T, OsoError>;

#[derive(Debug, Error)]
pub enum OsoError {
    #[error("lexer error at line {line} col {col}: {msg}")]
    Lex {
        line: usize,
        col: usize,
        msg: String,
    },
    #[error("parse error at line {line} col {col}: expected {expected}, got {got}")]
    Parse {
        line: usize,
        col: usize,
        expected: String,
        got: String,
    },
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("unknown opcode '{0}'")]
    UnknownOpcode(String),
}
