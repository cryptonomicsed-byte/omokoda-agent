use super::client::WorkspaceClient;
use serde_json::Value;

impl WorkspaceClient {
    /// Read a guild memory key scoped to calling agent.
    pub async fn memory_read(&self, key: &str) -> Result<Option<Value>, String> {
        let url = format!("{}/{}", self.memory_base(), key);
        match self.get(&url).await {
            Ok(v) => Ok(Some(v["value"].clone())),
            Err(e) if e.contains("404") => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Write a guild memory key.
    pub async fn memory_write(
        &self,
        key: &str,
        value: &str,
        visibility: &str,
    ) -> Result<(), String> {
        let url = format!("{}/{}", self.memory_base(), key);
        self.put_form(&url, &[("value", value), ("visibility", visibility)])
            .await?;
        Ok(())
    }

    /// Read shared guild memory (visibility: guild or public).
    pub async fn memory_read_shared(&self) -> Result<Vec<Value>, String> {
        let url = format!("{}/shared", self.memory_base());
        let val = self.get(&url).await?;
        Ok(val["entries"].as_array().cloned().unwrap_or_default())
    }
}
