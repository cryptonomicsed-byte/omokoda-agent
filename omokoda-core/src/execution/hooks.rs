#![allow(clippy::double_must_use)]
use crate::interpreter::ExecutionResult;
use crate::receipt::Receipt;
use async_trait::async_trait;

/// AgentPlugin — lifecycle event hooks wired into birth, think, act, and receipt.
/// Register plugins to observe and respond to sovereign agent lifecycle events.
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    /// Called when an agent is born.
    async fn on_birth(&self, name: &str, entropy: &[u8; 32]);

    /// Called before a 'think' statement is executed.
    async fn on_think(&self, prompt: &str);

    /// Called after a receipt is generated for an action.
    async fn on_receipt(&self, receipt: &Receipt);

    /// Called when a value settlement (e.g. escrow release) occurs.
    async fn on_settle(&self, result: &ExecutionResult);
}

pub struct HookManager {
    plugins: Vec<Box<dyn AgentPlugin>>,
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn AgentPlugin>) {
        self.plugins.push(plugin);
    }

    pub async fn fire_birth(&self, name: &str, entropy: &[u8; 32]) {
        for plugin in &self.plugins {
            plugin.on_birth(name, entropy).await;
        }
    }

    pub async fn fire_think(&self, prompt: &str) {
        for plugin in &self.plugins {
            plugin.on_think(prompt).await;
        }
    }

    pub async fn fire_receipt(&self, receipt: &Receipt) {
        for plugin in &self.plugins {
            plugin.on_receipt(receipt).await;
        }
    }

    pub async fn fire_settle(&self, result: &ExecutionResult) {
        for plugin in &self.plugins {
            plugin.on_settle(result).await;
        }
    }
}
