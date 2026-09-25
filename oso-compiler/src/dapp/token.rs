/// Tokens for the Ọ̀ṢỌ́ dApp DSL (`dapp Name { ... }`).
/// Separate from the VM-level opcode token set in `oso-parser`.

#[derive(Debug, Clone, PartialEq)]
pub enum DappTokenKind {
    // Keywords
    Dapp, Class, Asset, Capability, Action, Evidence, Settlement, Witness, Meta,
    Require, Pay, Emit, Vessel, From,
    True, False,
    // Contract classes
    Financial, Agent, Work, Device, EvidenceClass, Governance,
    // Operators
    And, Or, Not, Eq, Neq, Lt, Gt, Lte, Gte,
    Dot, Colon, Semi, Comma, LParen, RParen, LBrace, RBrace,
    // Literals / identifiers
    Ident,
    Number,
    StringLit,
    Eof,
}

#[derive(Debug, Clone)]
pub struct DappToken {
    pub kind:  DappTokenKind,
    pub value: String,
    pub line:  usize,
}

impl DappToken {
    pub fn new(kind: DappTokenKind, value: impl Into<String>, line: usize) -> Self {
        Self { kind, value: value.into(), line }
    }
}

pub fn keyword(s: &str) -> Option<DappTokenKind> {
    match s {
        "dapp"        => Some(DappTokenKind::Dapp),
        "class"       => Some(DappTokenKind::Class),
        "asset"       => Some(DappTokenKind::Asset),
        "capability"  => Some(DappTokenKind::Capability),
        "action"      => Some(DappTokenKind::Action),
        "evidence"    => Some(DappTokenKind::Evidence),
        "settlement"  => Some(DappTokenKind::Settlement),
        "witness"     => Some(DappTokenKind::Witness),
        "meta"        => Some(DappTokenKind::Meta),
        "require"     => Some(DappTokenKind::Require),
        "pay"         => Some(DappTokenKind::Pay),
        "emit"        => Some(DappTokenKind::Emit),
        "vessel"      => Some(DappTokenKind::Vessel),
        "from"        => Some(DappTokenKind::From),
        "true"        => Some(DappTokenKind::True),
        "false"       => Some(DappTokenKind::False),
        "financial"   => Some(DappTokenKind::Financial),
        "agent"       => Some(DappTokenKind::Agent),
        "work"        => Some(DappTokenKind::Work),
        "device"      => Some(DappTokenKind::Device),
        "governance"  => Some(DappTokenKind::Governance),
        _             => None,
    }
}
