use super::token::{DappToken, DappTokenKind, keyword};

pub struct DappLexer<'a> {
    src:  &'a str,
    pos:  usize,
    line: usize,
}

impl<'a> DappLexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { src, pos: 0, line: 1 }
    }

    pub fn tokenize(mut self) -> Result<Vec<DappToken>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.src.len() {
                tokens.push(DappToken::new(DappTokenKind::Eof, "", self.line));
                break;
            }
            let tok = self.next_token()?;
            tokens.push(tok);
        }
        Ok(tokens)
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.src.len() {
            let ch = self.cur();
            if ch == '\n' {
                self.line += 1;
                self.pos += 1;
            } else if ch.is_whitespace() {
                self.pos += 1;
            } else if self.starts_with("//") {
                while self.pos < self.src.len() && self.cur() != '\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<DappToken, String> {
        let line = self.line;
        let ch = self.cur();

        // Two-char operators
        if self.starts_with("&&") { self.pos += 2; return Ok(DappToken::new(DappTokenKind::And, "&&", line)); }
        if self.starts_with("||") { self.pos += 2; return Ok(DappToken::new(DappTokenKind::Or,  "||", line)); }
        if self.starts_with(">=") { self.pos += 2; return Ok(DappToken::new(DappTokenKind::Gte, ">=", line)); }
        if self.starts_with("<=") { self.pos += 2; return Ok(DappToken::new(DappTokenKind::Lte, "<=", line)); }
        if self.starts_with("==") { self.pos += 2; return Ok(DappToken::new(DappTokenKind::Eq,  "==", line)); }
        if self.starts_with("!=") { self.pos += 2; return Ok(DappToken::new(DappTokenKind::Neq, "!=", line)); }

        match ch {
            '.' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Dot,    ".", line)) }
            ':' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Colon,  ":", line)) }
            ';' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Semi,   ";", line)) }
            ',' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Comma,  ",", line)) }
            '(' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::LParen, "(", line)) }
            ')' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::RParen, ")", line)) }
            '{' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::LBrace, "{", line)) }
            '}' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::RBrace, "}", line)) }
            '<' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Lt,     "<", line)) }
            '>' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Gt,     ">", line)) }
            '!' => { self.pos += 1; Ok(DappToken::new(DappTokenKind::Not,    "!", line)) }
            '"' => self.lex_string(line),
            c if c.is_ascii_digit() => self.lex_number(line),
            c if c.is_alphabetic() || c == '_' => self.lex_ident(line),
            c => Err(format!("unexpected character '{}' at line {}", c, line)),
        }
    }

    fn lex_string(&mut self, line: usize) -> Result<DappToken, String> {
        self.pos += 1; // skip opening quote
        let start = self.pos;
        while self.pos < self.src.len() && self.cur() != '"' {
            if self.cur() == '\n' { self.line += 1; }
            self.pos += 1;
        }
        if self.pos >= self.src.len() {
            return Err(format!("unterminated string at line {}", line));
        }
        let s = self.src[start..self.pos].to_string();
        self.pos += 1; // skip closing quote
        Ok(DappToken::new(DappTokenKind::StringLit, s, line))
    }

    fn lex_number(&mut self, line: usize) -> Result<DappToken, String> {
        let start = self.pos;
        while self.pos < self.src.len() && (self.cur().is_ascii_digit() || self.cur() == '.') {
            self.pos += 1;
        }
        Ok(DappToken::new(DappTokenKind::Number, &self.src[start..self.pos], line))
    }

    fn lex_ident(&mut self, line: usize) -> Result<DappToken, String> {
        let start = self.pos;
        while self.pos < self.src.len() && (self.cur().is_alphanumeric() || self.cur() == '_') {
            self.pos += 1;
        }
        let ident = &self.src[start..self.pos];
        // Check for "evidence" as class keyword when used after "class"
        let kind = keyword(ident).unwrap_or(DappTokenKind::Ident);
        Ok(DappToken::new(kind, ident, line))
    }

    fn cur(&self) -> char {
        self.src[self.pos..].chars().next().unwrap()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.src[self.pos..].starts_with(s)
    }
}
