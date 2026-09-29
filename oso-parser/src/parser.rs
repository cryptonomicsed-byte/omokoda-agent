//! Recursive descent parser: `Vec<Token>` → `IrProgram`.

use std::collections::HashMap;
use crate::token::{Token, TokenKind};
use crate::ir::{IrInstruction, IrValue, IrProgram, canonicalize_name, opcode_for};
use crate::error::{OsoError, OsoResult};

pub struct OsoParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl OsoParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() { self.pos += 1; }
        tok
    }

    fn expect(&mut self, kind: &TokenKind) -> OsoResult<&Token> {
        match self.tokens.get(self.pos) {
            Some(t) if &t.kind == kind => {
                self.pos += 1;
                Ok(&self.tokens[self.pos - 1])
            }
            Some(t) => Err(OsoError::Parse {
                line: t.line, col: t.col,
                expected: format!("{:?}", kind),
                got: format!("{:?}({})", t.kind, t.value),
            }),
            None => Err(OsoError::UnexpectedEof),
        }
    }

    /// Parse one Ọ̀ṢỌ́ attribute: `@name(key=val, ...) [{ @inner... }]`
    fn parse_attribute(&mut self) -> OsoResult<IrInstruction> {
        let at = self.expect(&TokenKind::AtSym)?;
        let line = at.line;
        let name_tok = self.expect(&TokenKind::Ident)?;
        let raw_name = name_tok.value.clone();
        let canonical = canonicalize_name(&raw_name);
        let opcode = opcode_for(&canonical).ok_or_else(|| OsoError::UnknownOpcode(canonical.clone()))?;

        let mut args: HashMap<String, IrValue> = HashMap::new();

        // Optional paren args: @name(key=val, ...)
        if self.peek().is_some_and(|t| t.kind == TokenKind::LParen) {
            self.advance(); // consume `(`
            loop {
                if self.peek().is_none_or(|t| t.kind == TokenKind::RParen) { break; }
                let key_tok = self.expect(&TokenKind::Ident)?;
                let key = key_tok.value.clone();
                self.expect(&TokenKind::Eq)?;
                let val = self.parse_value()?;
                args.insert(key, val);
                // Consume optional comma
                if self.peek().is_some_and(|t| t.kind == TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(&TokenKind::RParen)?;
        }

        // Optional block body: { @inner... }
        if self.peek().is_some_and(|t| t.kind == TokenKind::LBrace) {
            self.advance(); // consume `{`
            let mut nested = Vec::new();
            while self.peek().is_some_and(|t| t.kind != TokenKind::RBrace) {
                nested.push(self.parse_attribute()?);
            }
            self.expect(&TokenKind::RBrace)?;
            args.insert("__body__".into(), IrValue::Block(nested));
        }

        Ok(IrInstruction { opcode, opcode_name: canonical, args, line })
    }

    fn parse_value(&mut self) -> OsoResult<IrValue> {
        match self.peek() {
            Some(t) => match t.kind {
                TokenKind::Number => {
                    let s = t.value.clone();
                    self.advance();
                    if s.contains('.') {
                        Ok(IrValue::Float(s.parse::<f64>().map_err(|_| OsoError::Lex {
                            line: 0, col: 0, msg: format!("bad float: {s}"),
                        })?))
                    } else {
                        Ok(IrValue::Int(s.parse::<i64>().map_err(|_| OsoError::Lex {
                            line: 0, col: 0, msg: format!("bad int: {s}"),
                        })?))
                    }
                }
                TokenKind::StringLit => {
                    let s = t.value.clone();
                    self.advance();
                    Ok(IrValue::Str(s))
                }
                TokenKind::HexLit => {
                    let s = t.value.clone();
                    self.advance();
                    let n = u64::from_str_radix(&s[2..], 16).map_err(|_| OsoError::Lex {
                        line: 0, col: 0, msg: format!("bad hex: {s}"),
                    })?;
                    Ok(IrValue::Hex(n))
                }
                TokenKind::Ident => {
                    // Bare identifiers treated as string values (e.g. `mode=active`)
                    let s = t.value.clone();
                    self.advance();
                    Ok(IrValue::Str(s))
                }
                _ => Err(OsoError::Parse {
                    line: t.line, col: t.col,
                    expected: "value".into(),
                    got: format!("{:?}({})", t.kind, t.value),
                }),
            },
            None => Err(OsoError::UnexpectedEof),
        }
    }

    /// Parse the full program — stop at EOF.
    pub fn parse(mut self) -> OsoResult<IrProgram> {
        let mut program = Vec::new();
        while self.peek().is_some() {
            // Skip stray semicolons at top level
            if self.peek().is_some_and(|t| t.kind == TokenKind::Semi) {
                self.advance();
                continue;
            }
            program.push(self.parse_attribute()?);
        }
        Ok(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> IrProgram {
        let toks = Lexer::new(src).tokenize().expect("lex");
        OsoParser::new(toks).parse().expect("parse")
    }

    #[test]
    fn parse_push_int() {
        let prog = parse("@PUSH(value=99)");
        assert_eq!(prog.len(), 1);
        assert_eq!(prog[0].opcode, 0x01);
        assert_eq!(prog[0].args["value"].as_int(), Some(99));
    }

    #[test]
    fn parse_load_const_float() {
        let prog = parse("@loadConst(value=3.14)");
        assert_eq!(prog[0].opcode_name, "LOAD_CONST");
        assert_eq!(prog[0].opcode, 0x03);
        if let IrValue::Float(f) = &prog[0].args["value"] {
            assert!((f - 3.14).abs() < 1e-9);
        } else { panic!("expected float"); }
    }

    #[test]
    fn parse_multiple_instructions() {
        let src = "@PUSH(value=1)\n@PUSH(value=2)\n@ADD()";
        let prog = parse(src);
        assert_eq!(prog.len(), 3);
        assert_eq!(prog[2].opcode, 0x10); // ADD
    }

    #[test]
    fn parse_string_arg() {
        let prog = parse("@STORE(key=\"agent_id\", value=\"abc123\")");
        assert_eq!(prog[0].args["key"].as_str(), Some("agent_id"));
        assert_eq!(prog[0].args["value"].as_str(), Some("abc123"));
    }

    #[test]
    fn parse_block_body() {
        let src = "@AGENT_THINK { @LOAD(key=\"goal\") }";
        let prog = parse(src);
        assert_eq!(prog[0].opcode, 0x61); // AGENT_THINK
        if let IrValue::Block(inner) = &prog[0].args["__body__"] {
            assert_eq!(inner[0].opcode, 0x05); // LOAD
        } else { panic!("expected block"); }
    }

    #[test]
    fn parse_hex_arg() {
        let prog = parse("@LOAD_CONST(value=0xFF)");
        assert_eq!(prog[0].args["value"].as_hex(), Some(0xFF));
    }

    #[test]
    fn compile_roundtrips_to_json() {
        let prog = parse("@EMIT_ASE(amount=1000)");
        let json = serde_json::to_string(&prog).unwrap();
        let back: IrProgram = serde_json::from_str(&json).unwrap();
        assert_eq!(back[0].opcode, prog[0].opcode);
    }

    #[test]
    fn unknown_opcode_errors() {
        let toks = Lexer::new("@UNKNOWN_XYZ(x=1)").tokenize().unwrap();
        let err = OsoParser::new(toks).parse();
        assert!(matches!(err, Err(crate::error::OsoError::UnknownOpcode(_))));
    }
}
