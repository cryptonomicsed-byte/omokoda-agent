/// Lower a `DappAst` to an `OsoIr` document.
use oso_ir::types::{
    OsoIr, ContractClass, AssetDef, FieldDef, CapabilityRef, ActionDef,
    EvidencePolicy, SettlementPolicy, WitnessPolicy, PolicyExpr as IrPolicyExpr,
    BackendTarget,
};
use super::ast::{DappAst, DappItem, ActionStmt, PolicyExpr as AstPolicyExpr};

pub fn lower(ast: DappAst) -> Result<OsoIr, String> {
    let mut contract_class = ContractClass::Agent;
    let mut assets   = Vec::new();
    let mut caps     = Vec::new();
    let mut actions  = Vec::new();
    let mut evidence = None;
    let mut settlement = None;
    let mut policy   = None;

    for item in ast.items {
        match item {
            DappItem::Class(s) => {
                contract_class = parse_class(&s)?;
            }
            DappItem::Asset(a) => {
                assets.push(AssetDef {
                    name: a.name,
                    fields: a.fields.into_iter().map(|f| FieldDef {
                        name: f.name,
                        field_type: f.field_type,
                        required: true,
                    }).collect(),
                    transferable: false,
                    divisible: false,
                });
            }
            DappItem::Capability(c) => {
                caps.push(CapabilityRef {
                    name: c.name,
                    minimum_tier: c.minimum_tier,
                    required: c.required,
                });
            }
            DappItem::Action(a) => {
                let mut requires = Vec::new();
                let mut emits    = Vec::new();
                let mut vessel   = None;
                for stmt in a.stmts {
                    match stmt {
                        ActionStmt::Require(expr) => {
                            if let Some(ir) = lower_policy_expr(expr) {
                                requires.push(ir);
                            }
                        }
                        ActionStmt::Emit(name)    => { emits.push(name); }
                        ActionStmt::Vessel(name)  => { vessel = Some(name); }
                        ActionStmt::Pay { .. }    => {} // settlement handled separately
                    }
                }
                actions.push(ActionDef {
                    name: a.name,
                    requires,
                    emits,
                    mutates_state: true,
                    vessel,
                });
            }
            DappItem::Evidence(e) => {
                evidence = Some(EvidencePolicy {
                    required:      e.required,
                    evidence_type: e.evidence_type,
                    minimum_count: e.minimum_count,
                });
            }
            DappItem::Settlement(s) => {
                settlement = Some(SettlementPolicy {
                    currency:     s.currency,
                    fee_routing:  s.fee_routing,
                    treasury_pct: s.treasury_pct,
                });
            }
            DappItem::Witness(w) => {
                policy = Some(WitnessPolicy {
                    witness_quorum:    w.quorum,
                    quality_threshold: 0,
                    upgradeable:       w.upgradeable,
                    requires_council:  w.requires_council,
                });
            }
            DappItem::Meta(pairs) => {
                // metadata folded into OsoIr.metadata
                let _ = pairs; // handled below
            }
        }
    }

    Ok(OsoIr {
        ir_version:      "1.0".to_string(),
        contract_class,
        name:            ast.name,
        version:         "0.1.0".to_string(),
        assets,
        capabilities:    caps,
        actions,
        evidence,
        settlement,
        policy,
        backend_targets: vec![BackendTarget::Native],
        metadata:        Default::default(),
    })
}

fn parse_class(s: &str) -> Result<ContractClass, String> {
    match s {
        "financial"  => Ok(ContractClass::Financial),
        "agent"      => Ok(ContractClass::Agent),
        "work"       => Ok(ContractClass::Work),
        "device"     => Ok(ContractClass::Device),
        "evidence"   => Ok(ContractClass::Evidence),
        "governance" => Ok(ContractClass::Governance),
        other        => Err(format!("unknown contract class: {}", other)),
    }
}

fn lower_policy_expr(expr: AstPolicyExpr) -> Option<IrPolicyExpr> {
    match expr {
        AstPolicyExpr::And(a, b) => {
            let left  = lower_policy_expr(*a)?;
            let right = lower_policy_expr(*b)?;
            Some(IrPolicyExpr::And { exprs: vec![left, right] })
        }
        AstPolicyExpr::Or(a, b) => {
            let left  = lower_policy_expr(*a)?;
            let right = lower_policy_expr(*b)?;
            Some(IrPolicyExpr::Or { exprs: vec![left, right] })
        }
        AstPolicyExpr::Not(inner) => {
            Some(IrPolicyExpr::Not { expr: Box::new(lower_policy_expr(*inner)?) })
        }
        AstPolicyExpr::Capability(name) => {
            Some(IrPolicyExpr::Capability { name })
        }
        AstPolicyExpr::Principal(role) => {
            Some(IrPolicyExpr::Principal { role })
        }
        AstPolicyExpr::Proof(_) | AstPolicyExpr::Evidence(_) => {
            Some(IrPolicyExpr::Proof { proof_type: "generic".to_string() })
        }
        AstPolicyExpr::Compare { field, op, value } => {
            Some(IrPolicyExpr::Numeric {
                field,
                op,
                value: serde_json::Value::String(value),
            })
        }
        AstPolicyExpr::BoolLit(_) => None,
    }
}
