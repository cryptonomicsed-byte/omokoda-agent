pub mod engineering;
pub mod osovm;
pub mod sovereign;

use serde::{Deserialize, Serialize};

/// Manifest for a named agent role — seeded from agency-agents taxonomy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRole {
    pub id: String,
    pub title: String,
    pub division: String,
    pub context: String,
    pub responsibilities: Vec<String>,
    pub capabilities: Vec<String>,
    pub output_format: String,
}

/// In-memory catalog with search helpers.
#[derive(Default)]
pub struct AgentCatalog {
    roles: Vec<AgentRole>,
}

impl AgentCatalog {
    pub fn new() -> Self {
        let mut cat = Self::default();
        cat.roles.extend(engineering::roles());
        cat.roles.extend(osovm::roles());
        cat.roles.extend(sovereign::roles());
        cat
    }

    pub fn all(&self) -> &[AgentRole] {
        &self.roles
    }

    pub fn find_by_id(&self, id: &str) -> Option<&AgentRole> {
        self.roles.iter().find(|r| r.id == id)
    }

    pub fn find_by_division(&self, division: &str) -> Vec<&AgentRole> {
        self.roles
            .iter()
            .filter(|r| r.division == division)
            .collect()
    }
}

pub use AgentCatalog as Catalog;
