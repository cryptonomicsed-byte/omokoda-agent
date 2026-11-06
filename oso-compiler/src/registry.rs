//! RitualRegistry — maps ritual names → Ọ̀ṢỌ́ source strings.

use std::collections::HashMap;

/// In-memory map from ritual name → Ọ̀ṢỌ́ source.
/// In production this is loaded from Walrus blobs or the agent's memory store.
pub struct RitualRegistry {
    rituals: HashMap<String, String>,
}

impl RitualRegistry {
    pub fn new() -> Self {
        Self {
            rituals: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, source: impl Into<String>) {
        self.rituals.insert(name.into(), source.into());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.rituals.get(name).map(|s| s.as_str())
    }

    pub fn names(&self) -> Vec<&str> {
        self.rituals.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for RitualRegistry {
    fn default() -> Self {
        Self::new()
    }
}
