/// Token types matching the Julia lexer's Symbol set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// `@`
    AtSym,
    /// identifier (letter or `_` start)
    Ident,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `=`
    Eq,
    /// `,`
    Comma,
    /// `;`
    Semi,
    /// `"..."` string literal (value is content without quotes)
    StringLit,
    /// `0x[0-9a-fA-F]+` hex literal
    HexLit,
    /// decimal / float number
    Number,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, value: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            kind,
            value: value.into(),
            line,
            col,
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}({:?}) @{}:{}",
            self.kind, self.value, self.line, self.col
        )
    }
}
