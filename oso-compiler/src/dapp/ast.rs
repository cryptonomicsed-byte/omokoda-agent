/// AST nodes for the Ọ̀ṢỌ́ dApp DSL.

#[derive(Debug, Clone)]
pub struct DappAst {
    pub name:  String,
    pub items: Vec<DappItem>,
}

#[derive(Debug, Clone)]
pub enum DappItem {
    Class(String),
    Asset(AssetDecl),
    Capability(CapDecl),
    Action(ActionDecl),
    Evidence(EvidenceDecl),
    Settlement(SettlementDecl),
    Witness(WitnessDecl),
    Meta(Vec<(String, MetaValue)>),
}

#[derive(Debug, Clone)]
pub struct AssetDecl {
    pub name:   String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name:       String,
    pub field_type: String,
}

#[derive(Debug, Clone)]
pub struct CapDecl {
    pub name:         String,
    pub minimum_tier: u8,
    pub required:     bool,
}

#[derive(Debug, Clone)]
pub struct ActionDecl {
    pub name:   String,
    pub params: Vec<String>,
    pub stmts:  Vec<ActionStmt>,
}

#[derive(Debug, Clone)]
pub enum ActionStmt {
    Require(PolicyExpr),
    Pay { from: String, to: String },
    Emit(String),
    Vessel(String),
}

#[derive(Debug, Clone)]
pub enum PolicyExpr {
    And(Box<PolicyExpr>, Box<PolicyExpr>),
    Or(Box<PolicyExpr>, Box<PolicyExpr>),
    Not(Box<PolicyExpr>),
    Capability(String),
    Principal(String),
    Proof(String),
    Evidence(String),
    Compare { field: String, op: String, value: String },
    BoolLit(bool),
}

#[derive(Debug, Clone)]
pub struct EvidenceDecl {
    pub required:       bool,
    pub evidence_type:  String,
    pub minimum_count:  u32,
}

#[derive(Debug, Clone)]
pub struct SettlementDecl {
    pub currency:     String,
    pub fee_routing:  String,
    pub treasury_pct: f64,
}

#[derive(Debug, Clone)]
pub struct WitnessDecl {
    pub quorum:           u32,
    pub upgradeable:      bool,
    pub requires_council: bool,
}

#[derive(Debug, Clone)]
pub enum MetaValue {
    Str(String),
    Num(f64),
    Bool(bool),
    Ident(String),
}
