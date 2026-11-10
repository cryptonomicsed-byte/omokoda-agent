//! IR types — mirrors Julia's `Instruction` struct and opcode table.
//!
//! `IrInstruction` = Julia `Instruction { opcode::UInt8, args::Dict{Symbol,Any} }`.
//! `IrValue`       = the argument value union.
//! `IrProgram`     = Julia `IR = Vector{Instruction}`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single IR instruction — opcode byte + named arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrInstruction {
    pub opcode: u8,
    pub opcode_name: String,
    pub args: HashMap<String, IrValue>,
    /// Source location for error reporting.
    pub line: usize,
}

/// Argument value types supported in Ọ̀ṢỌ́ IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IrValue {
    Int(i64),
    Float(f64),
    Str(String),
    Hex(u64),
    /// Nested instructions (block body).
    Block(Vec<IrInstruction>),
}

impl IrValue {
    pub fn as_int(&self) -> Option<i64> {
        if let Self::Int(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Self::Str(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_hex(&self) -> Option<u64> {
        if let Self::Hex(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}

/// A compiled Ọ̀ṢỌ́ program — flat list of instructions.
pub type IrProgram = Vec<IrInstruction>;

/// Canonical opcode table — matches OSOVM/src/opcodes.jl byte assignments.
/// Attribute name (uppercase, snake) → opcode byte.
pub fn opcode_for(name: &str) -> Option<u8> {
    // Core VM opcodes (0x01..0x0F)
    match name {
        "NOP" => Some(0x00),
        "PUSH" => Some(0x01),
        "POP" => Some(0x02),
        "LOAD_CONST" => Some(0x03),
        "STORE" => Some(0x04),
        "LOAD" => Some(0x05),
        "ADD" => Some(0x10),
        "SUB" => Some(0x11),
        "MUL" => Some(0x12),
        "DIV" => Some(0x13),
        "MOD" => Some(0x14),
        "EQ" => Some(0x20),
        "NEQ" => Some(0x21),
        "LT" => Some(0x22),
        "GT" => Some(0x23),
        "AND" => Some(0x30),
        "OR" => Some(0x31),
        "NOT" => Some(0x32),
        "JUMP" => Some(0x40),
        "JUMP_IF" => Some(0x41),
        "CALL" => Some(0x50),
        "RETURN" => Some(0x51),
        "HALT" => Some(0x52),
        // Agent ops (0x60..0x6F)
        "AGENT_BIRTH" => Some(0x60),
        "AGENT_THINK" => Some(0x61),
        "AGENT_ACT" => Some(0x62),
        "AGENT_SENSE" => Some(0x63),
        "AGENT_MEMORY" => Some(0x64),
        // Economy ops (0x70..0x7F)
        "EMIT_ASE" => Some(0x70),
        "BURN_ASE" => Some(0x71),
        "TRANSFER_ASE" => Some(0x72),
        "LOCK_ASE" => Some(0x73),
        // Storage ops (0x80..0x8F)
        "STORE_BLOB" => Some(0x80),
        "LOAD_BLOB" => Some(0x81),
        "SEAL_DATA" => Some(0x82),
        "UNSEAL_DATA" => Some(0x83),
        // Governance ops (0x90..0x9F)
        "PROPOSE" => Some(0x90),
        "VOTE" => Some(0x91),
        "EXECUTE" => Some(0x92),
        // Simulation ops (0xA0..0xAF)
        "SIM_STEP" => Some(0xA0),
        "SIM_VERIFY" => Some(0xA1),
        "GPU_CONTRIB" => Some(0xA2),
        // Hermetic ops (0xB0..0xBF)
        "GATE_CHECK" => Some(0xB0),
        "DNA_BIND" => Some(0xB1),
        // Custom extension
        "CUSTOM" => Some(0xFF),
        _ => None,
    }
}

/// Convert attribute name to canonical uppercase_snake form.
/// Mirrors Julia: `uppercase(replace(name, r"([a-z])([A-Z])" => s"\1_\2"))`
pub fn canonicalize_name(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && chars[i - 1].is_lowercase() {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_camel() {
        assert_eq!(canonicalize_name("loadConst"), "LOAD_CONST");
        assert_eq!(canonicalize_name("PUSH"), "PUSH");
        assert_eq!(canonicalize_name("agentBirth"), "AGENT_BIRTH");
    }

    #[test]
    fn opcode_lookup() {
        assert_eq!(opcode_for("PUSH"), Some(0x01));
        assert_eq!(opcode_for("AGENT_BIRTH"), Some(0x60));
        assert_eq!(opcode_for("UNKNOWN_XYZ"), None);
    }
}
