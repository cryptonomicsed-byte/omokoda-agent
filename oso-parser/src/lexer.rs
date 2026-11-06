use crate::error::{OsoError, OsoResult};
use crate::token::{Token, TokenKind};

pub struct Lexer<'src> {
    src: &'src str,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self { src }
    }

    pub fn tokenize(&self) -> OsoResult<Vec<Token>> {
        let mut tokens = Vec::new();
        for (line_num, line) in self.src.lines().enumerate() {
            let line_num = line_num + 1; // 1-indexed
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0usize;

            while i < chars.len() {
                let col = i + 1; // 1-indexed
                let c = chars[i];

                // Whitespace
                if c.is_whitespace() {
                    i += 1;
                    continue;
                }

                // Line comment
                if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
                    break; // skip rest of line
                }

                match c {
                    '@' => {
                        tokens.push(Token::new(TokenKind::AtSym, "@", line_num, col));
                        i += 1;
                    }
                    '(' => {
                        tokens.push(Token::new(TokenKind::LParen, "(", line_num, col));
                        i += 1;
                    }
                    ')' => {
                        tokens.push(Token::new(TokenKind::RParen, ")", line_num, col));
                        i += 1;
                    }
                    '{' => {
                        tokens.push(Token::new(TokenKind::LBrace, "{", line_num, col));
                        i += 1;
                    }
                    '}' => {
                        tokens.push(Token::new(TokenKind::RBrace, "}", line_num, col));
                        i += 1;
                    }
                    ';' => {
                        tokens.push(Token::new(TokenKind::Semi, ";", line_num, col));
                        i += 1;
                    }
                    ',' => {
                        tokens.push(Token::new(TokenKind::Comma, ",", line_num, col));
                        i += 1;
                    }
                    '=' => {
                        tokens.push(Token::new(TokenKind::Eq, "=", line_num, col));
                        i += 1;
                    }

                    // String literal
                    '"' => {
                        i += 1;
                        let start = i;
                        while i < chars.len() && chars[i] != '"' {
                            i += 1;
                        }
                        let s: String = chars[start..i].iter().collect();
                        tokens.push(Token::new(TokenKind::StringLit, s, line_num, col));
                        if i < chars.len() {
                            i += 1;
                        } // consume closing "
                    }

                    // Hex literal 0x...
                    '0' if i + 1 < chars.len() && chars[i + 1] == 'x' => {
                        let start = i;
                        i += 2;
                        while i < chars.len() && chars[i].is_ascii_hexdigit() {
                            i += 1;
                        }
                        let s: String = chars[start..i].iter().collect();
                        tokens.push(Token::new(TokenKind::HexLit, s, line_num, col));
                    }

                    // Number (decimal, float, or negative)
                    c if c.is_ascii_digit()
                        || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) =>
                    {
                        let start = i;
                        i += 1;
                        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                            i += 1;
                        }
                        let s: String = chars[start..i].iter().collect();
                        tokens.push(Token::new(TokenKind::Number, s, line_num, col));
                    }

                    // Identifier
                    c if c.is_alphabetic() || c == '_' => {
                        let start = i;
                        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            i += 1;
                        }
                        let s: String = chars[start..i].iter().collect();
                        tokens.push(Token::new(TokenKind::Ident, s, line_num, col));
                    }

                    other => {
                        return Err(OsoError::Lex {
                            line: line_num,
                            col,
                            msg: format!("unexpected character {:?}", other),
                        });
                    }
                }
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_attr() {
        let src = "@LOAD_CONST(value=42)";
        let toks = Lexer::new(src).tokenize().unwrap();
        assert_eq!(toks[0].kind, TokenKind::AtSym);
        assert_eq!(toks[1].kind, TokenKind::Ident);
        assert_eq!(toks[1].value, "LOAD_CONST");
        assert_eq!(toks[2].kind, TokenKind::LParen);
        assert_eq!(toks[3].value, "value");
        assert_eq!(toks[4].kind, TokenKind::Eq);
        assert_eq!(toks[5].kind, TokenKind::Number);
        assert_eq!(toks[5].value, "42");
    }

    #[test]
    fn skips_line_comments() {
        let src = "// this is a comment\n@PUSH(v=1)";
        let toks = Lexer::new(src).tokenize().unwrap();
        assert_eq!(toks[0].kind, TokenKind::AtSym);
    }

    #[test]
    fn tokenizes_hex_literal() {
        let toks = Lexer::new("0xFF").tokenize().unwrap();
        assert_eq!(toks[0].kind, TokenKind::HexLit);
        assert_eq!(toks[0].value, "0xFF");
    }

    #[test]
    fn tokenizes_string_literal() {
        let toks = Lexer::new("@SET(k=\"hello\")").tokenize().unwrap();
        let str_tok = toks
            .iter()
            .find(|t| t.kind == TokenKind::StringLit)
            .unwrap();
        assert_eq!(str_tok.value, "hello");
    }
}
