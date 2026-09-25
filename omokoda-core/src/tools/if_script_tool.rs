//! if_script tool — real field-grounded Odù divination via If-Script's
//! `FieldDiviner`, not the static corpus lookup (`ifascript::get_odu`,
//! already used elsewhere for identity derivation and unrelated to this).
//!
//! `FieldDiviner::cast(uri_pattern)` reads the live Waggle field (present
//! channel state) and the journal (`hours_back` ago), composes the two into
//! an 8-bit figure, and resolves that figure against the real Odù corpus --
//! the cast emerges from actual operational history, not a random throw or
//! a fixed table (see `ifascript::field_divination`'s own module docs).
//! Fails soft (`CastError::FieldUnreachable`) when Waggle isn't reachable --
//! this tool surfaces that as a normal error, never a panic.
//!
//! `FieldDiviner` uses `reqwest::blocking` internally (If-Script is a
//! synchronous crate); wrapped in `spawn_blocking` here so it never stalls
//! the async executor.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::tools::{ExecutionContext, Tool};

#[derive(Deserialize)]
struct IfScriptCastParams {
    /// Waggle field URI pattern to cast against, e.g. "agent/*" or a
    /// specific resource path. Empty/omitted casts against the root field.
    #[serde(default)]
    uri_pattern: String,
    /// How far back to read the "past" field state, in hours. Defaults to
    /// FieldDiviner's own default (24h) when omitted.
    #[serde(default)]
    hours_back: Option<f64>,
}

pub struct IfScriptTool;

#[async_trait]
impl Tool for IfScriptTool {
    fn name(&self) -> &str {
        "if_script_cast"
    }
    fn description(&self) -> &str {
        "Cast an Odù figure from the live Waggle field's real operational \
         history (present state composed over past state), via If-Script's \
         FieldDiviner -- not a static lookup table. Params: uri_pattern \
         (string, which field resource to read), hours_back (optional \
         number, default 24)."
    }
    fn required_tier(&self) -> u8 {
        0
    }
    fn is_write_operation(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        params: &str,
        _context: &ExecutionContext,
    ) -> Result<(String, crate::usage::TokenUsage), String> {
        let parsed: IfScriptCastParams = if params.trim().is_empty() {
            IfScriptCastParams {
                uri_pattern: String::new(),
                hours_back: None,
            }
        } else {
            serde_json::from_str(params).map_err(|e| format!("invalid params: {e}"))?
        };

        let uri_pattern = parsed.uri_pattern;
        let hours_back = parsed.hours_back;

        let cast = tokio::task::spawn_blocking(move || {
            let diviner = ifascript::field_divination::FieldDiviner::default();
            match hours_back {
                Some(hb) => diviner.cast_at(&uri_pattern, hb),
                None => diviner.cast(&uri_pattern),
            }
        })
        .await
        .map_err(|e| format!("if_script_cast task join error: {e}"))?
        .map_err(|e| format!("field cast failed: {e}"))?;

        // Compile the Calabash directive for the resolved Odù so callers can
        // feed it directly into ActionTransaction without re-deriving it.
        let directive = crate::execution::calabash_dispatch::CalabashDispatcher::directive_for(cast.binary);

        let result = json!({
            "odu_name": cast.odu.name,
            "universal_name": cast.odu.universal_name,
            "archetype": cast.odu.archetype,
            "binary": cast.binary,
            "vessel": directive.vessel,
            "present_signature": cast.present_signature,
            "past_signature": cast.past_signature,
            "opcode": directive.opcode,
            "prescription": directive.prescription,
            "prescriptions_spiritual": cast.odu.prescriptions,
        });
        Ok((result.to_string(), crate::usage::TokenUsage::default()))
    }
}
