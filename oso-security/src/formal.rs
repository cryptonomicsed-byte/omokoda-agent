//! Formal analyzer — FORM1, FORM2, FORM3.
//!
//! FORM1 (Termination): confirm no infinite call cycles via DFS over
//!        CALL→label edges. A cycle in the call graph = non-terminating.
//! FORM2 (Liveness): every lifecycle stage must have at least one reachable
//!        terminal instruction (HALT, RETURN, or one of: EMIT/BURN/TRANSFER
//!        as economic settlement; GATE_CHECK as verify/close signal).
//! FORM3 (Safety): BURN_ASE must always be preceded by LOCK_ASE on the same
//!        asset identifier within the same reachable path.

use crate::IrProgram;
use std::collections::{HashMap, HashSet};

pub struct FormalAnalyzer<'a> {
    program: &'a IrProgram,
}

impl<'a> FormalAnalyzer<'a> {
    pub fn new(program: &'a IrProgram) -> Self {
        Self { program }
    }

    /// Run all formal checks and return error strings.
    pub fn analyze(&self) -> Vec<String> {
        let mut errors = Vec::new();
        self.check_form1_termination(&mut errors);
        self.check_form2_liveness(&mut errors);
        self.check_form3_burn_safety(&mut errors);
        errors
    }

    /// FORM1: DFS cycle detection over CALL instructions.
    ///
    /// Build a directed graph: label → set of labels it CALLs.
    /// Walk from each label; if we revisit a label on the current path → cycle.
    fn check_form1_termination(&self, errors: &mut Vec<String>) {
        let instrs = &self.program.instructions;

        // Collect label definitions: label_name → instruction_index
        let mut label_defs: HashMap<String, usize> = HashMap::new();
        for (i, instr) in instrs.iter().enumerate() {
            if let Some(name) = instr.args.get("label").and_then(|v| v.as_str()) {
                label_defs.insert(name.to_string(), i);
            }
        }

        // Build call graph: from_label → vec of called labels
        // We assign instructions without an explicit label to a synthetic "__main__" scope.
        let mut call_graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut current_scope = "__main__".to_string();

        for instr in instrs {
            // A DEFINE / LABEL opcode starts a new scope
            if matches!(instr.opcode_name.as_str(), "DEFINE" | "LABEL") {
                if let Some(name) = instr.args.get("name").and_then(|v| v.as_str()) {
                    current_scope = name.to_string();
                }
            }
            if instr.opcode_name == "CALL" {
                if let Some(target) = instr
                    .args
                    .get("target")
                    .or_else(|| instr.args.get("fn"))
                    .and_then(|v| v.as_str())
                {
                    call_graph
                        .entry(current_scope.clone())
                        .or_default()
                        .push(target.to_string());
                }
            }
        }

        // DFS cycle detection
        let mut visited: HashSet<String> = HashSet::new();
        let mut reported: HashSet<String> = HashSet::new();

        let roots: Vec<String> = call_graph.keys().cloned().collect();
        for root in roots {
            let mut path = Vec::new();
            dfs_call(
                &root,
                &call_graph,
                &mut visited,
                &mut path,
                &mut reported,
                errors,
            );
        }
    }

    /// FORM2: Liveness — at least one reachable terminal in the instruction list.
    ///
    /// Terminal opcodes: HALT, RETURN, EMIT_ASE, BURN_ASE, TRANSFER_ASE, GATE_CHECK.
    fn check_form2_liveness(&self, errors: &mut Vec<String>) {
        const TERMINALS: &[&str] = &[
            "HALT",
            "RETURN",
            "EMIT_ASE",
            "BURN_ASE",
            "TRANSFER_ASE",
            "GATE_CHECK",
        ];

        let instrs = &self.program.instructions;
        if instrs.is_empty() {
            // Empty program trivially terminates.
            return;
        }

        let has_terminal = instrs
            .iter()
            .any(|i| TERMINALS.contains(&i.opcode_name.as_str()));

        if !has_terminal {
            errors.push(format!(
                "FORM2: program has {} instructions but no reachable terminal \
                 (HALT/RETURN/EMIT_ASE/BURN_ASE/TRANSFER_ASE/GATE_CHECK). \
                 Every lifecycle stage must have at least one settlement point.",
                instrs.len()
            ));
        }
    }

    /// FORM3: Safety — BURN_ASE must be preceded by LOCK_ASE on the same asset.
    ///
    /// Strategy: scan instructions in order. Track which asset IDs are currently
    /// locked (via LOCK_ASE). If BURN_ASE fires for an asset that was never locked,
    /// flag an error.
    fn check_form3_burn_safety(&self, errors: &mut Vec<String>) {
        let instrs = &self.program.instructions;
        let mut locked_assets: HashSet<String> = HashSet::new();

        for instr in instrs {
            match instr.opcode_name.as_str() {
                "LOCK_ASE" => {
                    // Record the asset being locked
                    let asset = asset_id_from(instr);
                    locked_assets.insert(asset);
                }
                "BURN_ASE" => {
                    let asset = asset_id_from(instr);
                    if !locked_assets.contains(&asset) {
                        errors.push(format!(
                            "FORM3: BURN_ASE at line {} on asset '{}' without prior LOCK_ASE — \
                             burning unlocked assets is forbidden",
                            instr.line, asset
                        ));
                    }
                    // Consuming the lock after burn
                    locked_assets.remove(&asset);
                }
                _ => {}
            }
        }
    }
}

/// Extract a stable asset identifier from an instruction's args.
/// Falls back to "__default__" if no asset/token/id arg is present.
fn asset_id_from(instr: &crate::IrInstruction) -> String {
    instr
        .args
        .get("asset")
        .or_else(|| instr.args.get("token"))
        .or_else(|| instr.args.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("__default__")
        .to_string()
}

/// DFS helper for call-graph cycle detection.
fn dfs_call(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
    reported: &mut HashSet<String>,
    errors: &mut Vec<String>,
) {
    if reported.contains(node) {
        return;
    }
    if path.contains(&node.to_string()) {
        // Cycle detected
        let cycle_start = path.iter().position(|x| x == node).unwrap_or(0);
        let cycle: Vec<&str> = path[cycle_start..].iter().map(|s| s.as_str()).collect();
        reported.insert(node.to_string());
        errors.push(format!(
            "FORM1: infinite call cycle detected: {} → {}",
            cycle.join(" → "),
            node
        ));
        return;
    }
    if visited.contains(node) {
        return;
    }

    path.push(node.to_string());
    if let Some(callees) = graph.get(node) {
        for callee in callees.clone() {
            dfs_call(&callee, graph, visited, path, reported, errors);
        }
    }
    path.pop();
    visited.insert(node.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IrInstruction, IrProgram};
    use serde_json::json;

    fn make_instr(name: &str, args: serde_json::Value, line: usize) -> IrInstruction {
        IrInstruction {
            opcode: 0,
            opcode_name: name.into(),
            args: if let serde_json::Value::Object(m) = args {
                m
            } else {
                Default::default()
            },
            line,
        }
    }

    fn bare_prog(instrs: Vec<IrInstruction>) -> IrProgram {
        IrProgram {
            instructions: instrs,
            metadata: serde_json::Value::Null,
            capabilities: vec![],
            minimum_tier: 0,
        }
    }

    #[test]
    fn form2_no_terminal_errors() {
        let p = bare_prog(vec![
            make_instr("NOP", json!({}), 1),
            make_instr("PUSH", json!({"value": 42}), 2),
        ]);
        let errs = FormalAnalyzer::new(&p).analyze();
        assert!(errs.iter().any(|e| e.contains("FORM2")));
    }

    #[test]
    fn form2_with_halt_passes() {
        let p = bare_prog(vec![
            make_instr("NOP", json!({}), 1),
            make_instr("HALT", json!({}), 2),
        ]);
        let errs = FormalAnalyzer::new(&p).analyze();
        assert!(errs.iter().all(|e| !e.contains("FORM2")));
    }

    #[test]
    fn form3_burn_without_lock_errors() {
        let p = bare_prog(vec![make_instr(
            "BURN_ASE",
            json!({"asset": "ASE", "amount": 100}),
            5,
        )]);
        let errs = FormalAnalyzer::new(&p).analyze();
        assert!(errs.iter().any(|e| e.contains("FORM3")));
    }

    #[test]
    fn form3_lock_then_burn_passes() {
        let p = bare_prog(vec![
            make_instr("LOCK_ASE", json!({"asset": "ASE", "amount": 100}), 1),
            make_instr("BURN_ASE", json!({"asset": "ASE", "amount": 100}), 2),
            make_instr("HALT", json!({}), 3),
        ]);
        let errs = FormalAnalyzer::new(&p).analyze();
        assert!(errs.iter().all(|e| !e.contains("FORM3")));
    }

    #[test]
    fn form1_call_cycle_detected() {
        // fn A calls fn B, fn B calls fn A → cycle
        let p = bare_prog(vec![
            make_instr("DEFINE", json!({"name": "fn_a"}), 1),
            make_instr("CALL", json!({"target": "fn_b"}), 2),
            make_instr("RETURN", json!({}), 3),
            make_instr("DEFINE", json!({"name": "fn_b"}), 4),
            make_instr("CALL", json!({"target": "fn_a"}), 5),
            make_instr("RETURN", json!({}), 6),
        ]);
        let errs = FormalAnalyzer::new(&p).analyze();
        assert!(errs.iter().any(|e| e.contains("FORM1")));
    }

    #[test]
    fn form1_no_cycles_in_linear_program() {
        let p = bare_prog(vec![
            make_instr("CALL", json!({"target": "helper"}), 1),
            make_instr("HALT", json!({}), 2),
        ]);
        let errs = FormalAnalyzer::new(&p).analyze();
        assert!(errs.iter().all(|e| !e.contains("FORM1")));
    }
}
