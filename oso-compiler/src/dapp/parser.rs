use super::token::{DappToken, DappTokenKind};
use super::ast::*;

pub struct DappParser {
    tokens: Vec<DappToken>,
    pos:    usize,
}

impl DappParser {
    pub fn new(tokens: Vec<DappToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(mut self) -> Result<DappAst, String> {
        self.expect_kind(DappTokenKind::Dapp)?;
        let name = self.expect_ident()?;
        self.expect_kind(DappTokenKind::LBrace)?;

        let mut items = Vec::new();
        while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
            items.push(self.parse_item()?);
        }
        self.expect_kind(DappTokenKind::RBrace)?;

        Ok(DappAst { name, items })
    }

    fn parse_item(&mut self) -> Result<DappItem, String> {
        match self.cur_kind() {
            DappTokenKind::Class      => self.parse_class(),
            DappTokenKind::Asset      => self.parse_asset(),
            DappTokenKind::Capability => self.parse_capability(),
            DappTokenKind::Action     => self.parse_action(),
            DappTokenKind::Evidence   => self.parse_evidence(),
            DappTokenKind::Settlement => self.parse_settlement(),
            DappTokenKind::Witness    => self.parse_witness(),
            DappTokenKind::Meta       => self.parse_meta(),
            _ => Err(format!("unexpected token {:?} at line {}", self.cur().kind, self.cur().line)),
        }
    }

    fn parse_class(&mut self) -> Result<DappItem, String> {
        self.advance(); // consume 'class'
        let class_name = match self.cur_kind() {
            DappTokenKind::Financial   => { self.advance(); "financial".to_string() }
            DappTokenKind::Agent       => { self.advance(); "agent".to_string() }
            DappTokenKind::Work        => { self.advance(); "work".to_string() }
            DappTokenKind::Device      => { self.advance(); "device".to_string() }
            DappTokenKind::EvidenceClass | DappTokenKind::Evidence => { self.advance(); "evidence".to_string() }
            DappTokenKind::Governance  => { self.advance(); "governance".to_string() }
            DappTokenKind::Ident       => { let s = self.cur().value.clone(); self.advance(); s }
            _ => return Err(format!("expected contract class at line {}", self.cur().line)),
        };
        self.skip_semi();
        Ok(DappItem::Class(class_name))
    }

    fn parse_asset(&mut self) -> Result<DappItem, String> {
        self.advance(); // consume 'asset'
        let name = self.expect_ident()?;
        self.expect_kind(DappTokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
            let fname = self.expect_ident()?;
            self.expect_kind(DappTokenKind::Colon)?;
            let ftype = self.expect_ident()?;
            self.skip_semi();
            fields.push(FieldDecl { name: fname, field_type: ftype });
        }
        self.expect_kind(DappTokenKind::RBrace)?;
        self.skip_semi();
        Ok(DappItem::Asset(AssetDecl { name, fields }))
    }

    fn parse_capability(&mut self) -> Result<DappItem, String> {
        self.advance(); // consume 'capability'
        let name = self.expect_ident()?;
        let mut minimum_tier = 0u8;
        let mut required = true;
        if self.at(DappTokenKind::LBrace) {
            self.advance();
            while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
                let key = self.expect_ident()?;
                self.expect_kind(DappTokenKind::Colon)?;
                match key.as_str() {
                    "tier" => {
                        // expect TN or number
                        let val = self.expect_ident_or_num()?;
                        if val.starts_with('T') {
                            minimum_tier = val[1..].parse().unwrap_or(0);
                        } else {
                            minimum_tier = val.parse().unwrap_or(0);
                        }
                    }
                    "required" => {
                        required = self.expect_bool()?;
                    }
                    _ => { self.advance(); }
                }
                self.skip_semi();
            }
            self.expect_kind(DappTokenKind::RBrace)?;
        }
        self.skip_semi();
        Ok(DappItem::Capability(CapDecl { name, minimum_tier, required }))
    }

    fn parse_action(&mut self) -> Result<DappItem, String> {
        self.advance(); // consume 'action'
        let name = self.expect_ident()?;
        self.expect_kind(DappTokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(DappTokenKind::RParen) && !self.at(DappTokenKind::Eof) {
            params.push(self.expect_ident()?);
            if self.at(DappTokenKind::Comma) { self.advance(); }
        }
        self.expect_kind(DappTokenKind::RParen)?;

        let mut stmts = Vec::new();
        if self.at(DappTokenKind::LBrace) {
            self.advance();
            while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
                stmts.push(self.parse_action_stmt()?);
            }
            self.expect_kind(DappTokenKind::RBrace)?;
        }
        self.skip_semi();
        Ok(DappItem::Action(ActionDecl { name, params, stmts }))
    }

    fn parse_action_stmt(&mut self) -> Result<ActionStmt, String> {
        match self.cur_kind() {
            DappTokenKind::Require => {
                self.advance();
                let expr = self.parse_policy_expr()?;
                self.skip_semi();
                Ok(ActionStmt::Require(expr))
            }
            DappTokenKind::Pay => {
                self.advance();
                let to = self.expect_ident()?;
                self.expect_kind(DappTokenKind::From)?;
                let from = self.expect_ident()?;
                self.skip_semi();
                Ok(ActionStmt::Pay { from, to })
            }
            DappTokenKind::Emit => {
                self.advance();
                let name = self.expect_ident()?;
                self.skip_semi();
                Ok(ActionStmt::Emit(name))
            }
            DappTokenKind::Vessel => {
                self.advance();
                let name = self.expect_ident()?;
                self.skip_semi();
                Ok(ActionStmt::Vessel(name))
            }
            _ => Err(format!("unexpected action stmt {:?} at line {}", self.cur().kind, self.cur().line)),
        }
    }

    fn parse_policy_expr(&mut self) -> Result<PolicyExpr, String> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<PolicyExpr, String> {
        let mut left = self.parse_and_expr()?;
        while self.at(DappTokenKind::Or) {
            self.advance();
            let right = self.parse_and_expr()?;
            left = PolicyExpr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<PolicyExpr, String> {
        let mut left = self.parse_not_expr()?;
        while self.at(DappTokenKind::And) {
            self.advance();
            let right = self.parse_not_expr()?;
            left = PolicyExpr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<PolicyExpr, String> {
        if self.at(DappTokenKind::Not) {
            self.advance();
            let inner = self.parse_not_expr()?;
            return Ok(PolicyExpr::Not(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<PolicyExpr, String> {
        if self.at(DappTokenKind::LParen) {
            self.advance();
            let expr = self.parse_policy_expr()?;
            self.expect_kind(DappTokenKind::RParen)?;
            return Ok(expr);
        }

        // capability(NAME) or capability.NAME
        if self.at(DappTokenKind::Capability) {
            self.advance();
            if self.at(DappTokenKind::LParen) {
                self.advance();
                let cap = self.expect_ident()?;
                self.expect_kind(DappTokenKind::RParen)?;
                return Ok(PolicyExpr::Capability(cap));
            } else if self.at(DappTokenKind::Dot) {
                self.advance();
                let cap = self.expect_ident()?;
                return Ok(PolicyExpr::Capability(cap));
            } else {
                return Err(format!(
                    "expected '(' or '.' after 'capability' at line {}", self.cur().line
                ));
            }
        }

        // true / false literals
        if self.at(DappTokenKind::True) {
            self.advance();
            return Ok(PolicyExpr::BoolLit(true));
        }
        if self.at(DappTokenKind::False) {
            self.advance();
            return Ok(PolicyExpr::BoolLit(false));
        }

        // principal.role / proof.valid / evidence.accepted / field >= value
        let ident = self.expect_ident()?;
        if self.at(DappTokenKind::Dot) {
            self.advance();
            let attr = self.expect_ident()?;
            match ident.as_str() {
                "principal" => return Ok(PolicyExpr::Principal(attr)),
                "proof"     => return Ok(PolicyExpr::Proof(attr)),
                "evidence"  => return Ok(PolicyExpr::Evidence(attr)),
                _ => {
                    let field = format!("{}.{}", ident, attr);
                    if let Some(op) = self.try_comp_op() {
                        let value = self.expect_ident_or_num()?;
                        return Ok(PolicyExpr::Compare { field, op, value });
                    }
                    return Ok(PolicyExpr::Principal(format!("{}.{}", ident, attr)));
                }
            }
        }
        if let Some(op) = self.try_comp_op() {
            let value = self.expect_ident_or_num()?;
            return Ok(PolicyExpr::Compare { field: ident, op, value });
        }
        Ok(PolicyExpr::Principal(ident))
    }

    fn try_comp_op(&mut self) -> Option<String> {
        match self.cur_kind() {
            DappTokenKind::Gte => { self.advance(); Some(">=".to_string()) }
            DappTokenKind::Lte => { self.advance(); Some("<=".to_string()) }
            DappTokenKind::Gt  => { self.advance(); Some(">".to_string()) }
            DappTokenKind::Lt  => { self.advance(); Some("<".to_string()) }
            DappTokenKind::Eq  => { self.advance(); Some("==".to_string()) }
            DappTokenKind::Neq => { self.advance(); Some("!=".to_string()) }
            _ => None,
        }
    }

    fn parse_evidence(&mut self) -> Result<DappItem, String> {
        self.advance();
        self.expect_kind(DappTokenKind::LBrace)?;
        let mut required = false;
        let mut evidence_type = String::new();
        let mut minimum_count = 1u32;
        while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
            let key = self.expect_ident()?;
            self.expect_kind(DappTokenKind::Colon)?;
            match key.as_str() {
                "required"       => { required = self.expect_bool()?; }
                "type"           => { evidence_type = self.expect_ident()?; }
                "minimum_count"  => { minimum_count = self.expect_num_u32()?; }
                _ => { self.advance(); }
            }
            self.skip_semi();
        }
        self.expect_kind(DappTokenKind::RBrace)?;
        self.skip_semi();
        Ok(DappItem::Evidence(EvidenceDecl { required, evidence_type, minimum_count }))
    }

    fn parse_settlement(&mut self) -> Result<DappItem, String> {
        self.advance();
        self.expect_kind(DappTokenKind::LBrace)?;
        let mut currency = String::new();
        let mut fee_routing = String::new();
        let mut treasury_pct = 0.0f64;
        while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
            let key = self.expect_ident()?;
            self.expect_kind(DappTokenKind::Colon)?;
            match key.as_str() {
                "currency"     => { currency = self.expect_ident()?; }
                "fee_routing"  => { fee_routing = self.expect_string_or_ident()?; }
                "treasury_pct" => { treasury_pct = self.cur().value.parse().unwrap_or(0.0); self.advance(); }
                _ => { self.advance(); }
            }
            self.skip_semi();
        }
        self.expect_kind(DappTokenKind::RBrace)?;
        self.skip_semi();
        Ok(DappItem::Settlement(SettlementDecl { currency, fee_routing, treasury_pct }))
    }

    fn parse_witness(&mut self) -> Result<DappItem, String> {
        self.advance();
        self.expect_kind(DappTokenKind::LBrace)?;
        let mut quorum = 1u32;
        let mut upgradeable = false;
        let mut requires_council = false;
        while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
            let key = self.expect_ident()?;
            self.expect_kind(DappTokenKind::Colon)?;
            match key.as_str() {
                "quorum"           => { quorum = self.expect_num_u32()?; }
                "upgradeable"      => { upgradeable = self.expect_bool()?; }
                "requires_council" => { requires_council = self.expect_bool()?; }
                _ => { self.advance(); }
            }
            self.skip_semi();
        }
        self.expect_kind(DappTokenKind::RBrace)?;
        self.skip_semi();
        Ok(DappItem::Witness(WitnessDecl { quorum, upgradeable, requires_council }))
    }

    fn parse_meta(&mut self) -> Result<DappItem, String> {
        self.advance();
        self.expect_kind(DappTokenKind::LBrace)?;
        let mut pairs = Vec::new();
        while !self.at(DappTokenKind::RBrace) && !self.at(DappTokenKind::Eof) {
            let key = self.expect_ident()?;
            self.expect_kind(DappTokenKind::Colon)?;
            let val = match self.cur_kind() {
                DappTokenKind::StringLit => { let s = self.cur().value.clone(); self.advance(); MetaValue::Str(s) }
                DappTokenKind::Number    => { let n = self.cur().value.parse().unwrap_or(0.0); self.advance(); MetaValue::Num(n) }
                DappTokenKind::True      => { self.advance(); MetaValue::Bool(true) }
                DappTokenKind::False     => { self.advance(); MetaValue::Bool(false) }
                _                        => { let s = self.cur().value.clone(); self.advance(); MetaValue::Ident(s) }
            };
            self.skip_semi();
            pairs.push((key, val));
        }
        self.expect_kind(DappTokenKind::RBrace)?;
        self.skip_semi();
        Ok(DappItem::Meta(pairs))
    }

    // --- helpers ---

    fn expect_ident(&mut self) -> Result<String, String> {
        let tok = self.cur().clone();
        match tok.kind {
            DappTokenKind::Ident | DappTokenKind::Financial | DappTokenKind::Agent
            | DappTokenKind::Work | DappTokenKind::Device | DappTokenKind::Evidence
            | DappTokenKind::EvidenceClass | DappTokenKind::Governance
            | DappTokenKind::From | DappTokenKind::Class => {
                self.advance();
                Ok(tok.value)
            }
            _ => {
                // Accept any keyword as an identifier (for field names like "required", "type")
                if !matches!(tok.kind, DappTokenKind::LBrace | DappTokenKind::RBrace
                    | DappTokenKind::LParen | DappTokenKind::RParen | DappTokenKind::Eof
                    | DappTokenKind::Colon | DappTokenKind::Semi | DappTokenKind::And
                    | DappTokenKind::Or | DappTokenKind::Not | DappTokenKind::Number
                    | DappTokenKind::StringLit) {
                    self.advance();
                    Ok(tok.value)
                } else {
                    Err(format!("expected identifier, got {:?} ('{}') at line {}", tok.kind, tok.value, tok.line))
                }
            }
        }
    }

    fn expect_ident_or_num(&mut self) -> Result<String, String> {
        let tok = self.cur().clone();
        if matches!(tok.kind, DappTokenKind::Number | DappTokenKind::StringLit) {
            self.advance();
            Ok(tok.value)
        } else {
            self.expect_ident()
        }
    }

    fn expect_string_or_ident(&mut self) -> Result<String, String> {
        let tok = self.cur().clone();
        if tok.kind == DappTokenKind::StringLit {
            self.advance();
            return Ok(tok.value);
        }
        self.expect_ident()
    }

    fn expect_bool(&mut self) -> Result<bool, String> {
        match self.cur_kind() {
            DappTokenKind::True  => { self.advance(); Ok(true) }
            DappTokenKind::False => { self.advance(); Ok(false) }
            _ => Err(format!("expected bool at line {}", self.cur().line)),
        }
    }

    fn expect_num_u32(&mut self) -> Result<u32, String> {
        if self.cur().kind == DappTokenKind::Number {
            let n = self.cur().value.parse::<u32>().unwrap_or(0);
            self.advance();
            Ok(n)
        } else {
            Err(format!("expected number at line {}", self.cur().line))
        }
    }

    fn expect_kind(&mut self, kind: DappTokenKind) -> Result<(), String> {
        if self.cur().kind == kind {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?} ('{}') at line {}", kind, self.cur().kind, self.cur().value, self.cur().line))
        }
    }

    fn skip_semi(&mut self) {
        while self.at(DappTokenKind::Semi) { self.advance(); }
    }

    fn at(&self, kind: DappTokenKind) -> bool {
        self.cur().kind == kind
    }

    fn cur_kind(&self) -> DappTokenKind {
        self.cur().kind.clone()
    }

    fn cur(&self) -> &DappToken {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) {
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
    }
}
