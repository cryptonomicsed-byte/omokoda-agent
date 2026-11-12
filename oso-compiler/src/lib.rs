//! Ọ̀ṢỌ́ compiler — bridges If-Script decisions to OSOVM IR.
//!
//! See spec: sovereign-eco-blueprint/specs/OSO_IFSCRIPT_BOUNDARY.md
//!
//! Pipeline:
//!   IfDecision → oso_dispatch() → IrProgram
//!
//! The compiler layers context (gate_alignment, params) onto the Ọ̀ṢỌ́ source
//! before parsing, injecting them as `@LOAD_CONST` preamble instructions.

pub mod context;
pub mod decision;
pub mod dispatch;
pub mod registry;

pub use context::CompileContext;
pub use decision::{GateScore, IfDecision};
pub use dispatch::oso_dispatch;
pub use oso_parser::{IrInstruction, IrProgram, IrValue, OsoError, OsoResult};
pub use registry::RitualRegistry;
