//! Security rules:
//!   SEC1 — no self-escalation: an instruction cannot grant the caller a tier
//!           higher than their declared tier
//!   SEC2 — no infinite loops: cycle detection in the JUMP/JUMP_IF graph
//!           (a backward jump to an already-visited target is a cycle)

use std::collections::{HashSet, HashMap};
use crate::{Diagnostic, IrInstruction};

#[allow(clippy::ptr_arg)]
pub fn check(
    instructions: &[IrInstruction],
    _warnings: &mut Vec<Diagnostic>,
    errors: &mut Vec<Diagnostic>,
) {
    check_self_escalation(instructions, errors);
    check_infinite_loops(instructions, errors);
}

/// SEC1: self-escalation check.
///
/// Walk all instructions that carry both a `caller_tier` (or `current_tier`) arg
/// and a `grant_tier` (or `new_tier`) arg. If grant_tier > caller_tier, it's
/// a self-escalation attempt and is flagged as an error.
fn check_self_escalation(instructions: &[IrInstruction], errors: &mut Vec<Diagnostic>) {
    for instr in instructions {
        let caller_tier = instr.args.get("caller_tier")
            .or_else(|| instr.args.get("current_tier"))
            .and_then(|v| v.as_i64());

        let grant_tier = instr.args.get("grant_tier")
            .or_else(|| instr.args.get("new_tier"))
            .and_then(|v| v.as_i64());

        if let (Some(caller), Some(grant)) = (caller_tier, grant_tier) {
            if grant > caller {
                errors.push(Diagnostic {
                    rule: "security.SEC1.self_escalation".into(),
                    message: format!(
                        "{} at line {} attempts tier escalation: caller_tier={} → grant_tier={}",
                        instr.opcode_name, instr.line, caller, grant
                    ),
                    line: Some(instr.line),
                });
            }
        }

        // Also catch AGENT_BIRTH trying to birth an agent at higher tier than caller
        if instr.opcode_name == "AGENT_BIRTH" {
            let caller_tier = instr.args.get("caller_tier").and_then(|v| v.as_i64());
            let birth_tier  = instr.args.get("tier").and_then(|v| v.as_i64());
            if let (Some(caller), Some(birth)) = (caller_tier, birth_tier) {
                if birth > caller {
                    errors.push(Diagnostic {
                        rule: "security.SEC1.birth_escalation".into(),
                        message: format!(
                            "AGENT_BIRTH at line {} births agent at tier {} but caller is tier {}",
                            instr.line, birth, caller
                        ),
                        line: Some(instr.line),
                    });
                }
            }
        }
    }
}

/// SEC2: cycle detection in the JUMP/JUMP_IF instruction graph.
///
/// Strategy:
///   - Build a directed graph: instruction_index → jump_target_index
///   - JUMP/JUMP_IF args may carry a numeric `target` (instruction index) or
///     a string `label`. We only handle numeric targets here; label resolution
///     would require a full symbol table pass.
///   - DFS from index 0, tracking the current path. A back-edge = infinite loop.
fn check_infinite_loops(instructions: &[IrInstruction], errors: &mut Vec<Diagnostic>) {
    if instructions.is_empty() {
        return;
    }

    // Build jump graph: source_idx → Vec<target_idx>
    let mut graph: HashMap<usize, Vec<usize>> = HashMap::new();

    for (i, instr) in instructions.iter().enumerate() {
        if matches!(instr.opcode_name.as_str(), "JUMP" | "JUMP_IF") {
            if let Some(target_val) = instr.args.get("target") {
                if let Some(target_idx) = target_val.as_u64() {
                    let t = target_idx as usize;
                    if t < instructions.len() {
                        graph.entry(i).or_default().push(t);
                    }
                }
            }
        }
        // Fall-through edge (non-terminal instructions proceed to i+1)
        if !matches!(instr.opcode_name.as_str(), "JUMP" | "HALT" | "RETURN") {
            let next = i + 1;
            if next < instructions.len() {
                graph.entry(i).or_default().push(next);
            }
        }
    }

    // DFS cycle detection
    let mut visited: HashSet<usize> = HashSet::new();
    let mut path: Vec<usize> = Vec::new();
    let mut cycle_reported: HashSet<usize> = HashSet::new();

    dfs(0, &graph, &mut visited, &mut path, &mut cycle_reported, instructions, errors);
}

fn dfs(
    node: usize,
    graph: &HashMap<usize, Vec<usize>>,
    visited: &mut HashSet<usize>,
    path: &mut Vec<usize>,
    cycle_reported: &mut HashSet<usize>,
    instructions: &[IrInstruction],
    errors: &mut Vec<Diagnostic>,
) {
    if cycle_reported.contains(&node) {
        return; // already reported this cycle root
    }
    if path.contains(&node) {
        // Back-edge found → infinite loop
        if !cycle_reported.contains(&node) {
            cycle_reported.insert(node);
            let cycle_start = path.iter().position(|&x| x == node).unwrap_or(0);
            let cycle: Vec<usize> = path[cycle_start..].to_vec();
            let instr_line = instructions.get(node).map(|i| i.line);
            errors.push(Diagnostic {
                rule: "security.SEC2.infinite_loop".into(),
                message: format!(
                    "infinite loop detected: cycle through instruction indices {:?}",
                    cycle
                ),
                line: instr_line,
            });
        }
        return;
    }
    if visited.contains(&node) {
        return; // already fully explored this node
    }

    path.push(node);
    if let Some(neighbors) = graph.get(&node) {
        for &next in neighbors {
            dfs(next, graph, visited, path, cycle_reported, instructions, errors);
        }
    }
    path.pop();
    visited.insert(node);
}
