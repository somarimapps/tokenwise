use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

/// An MCP server entry in the Hermes config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HermesMcpServer {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

/// A hook entry in the Hermes config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HermesHook {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
}

/// Root structure of the Hermes agent YAML config (`~/.hermes/config.yaml`).
///
/// `BTreeMap` for `mcp_servers`, `env`, and `hooks` ensures alphabetical key
/// ordering in YAML output, making idempotent writes byte-stable.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HermesConfig {
    /// Registered MCP servers, keyed by name.
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: BTreeMap<String, HermesMcpServer>,

    /// Environment variables injected into the Hermes process.
    #[serde(default)]
    pub env: BTreeMap<String, String>,

    /// Hook definitions, keyed by event name (e.g. `preToolUse`).
    #[serde(default)]
    pub hooks: BTreeMap<String, Vec<HermesHook>>,
}

impl HermesConfig {
    /// Read the Hermes config from `path`, or return a default empty config
    /// if the file does not exist yet.
    pub fn read_or_default(
        path: &std::path::Path,
    ) -> Result<Self, tokenwise_common::TokenwiseError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&content).map_err(|e| {
            tokenwise_common::TokenwiseError::InvalidInvocation(format!(
                "Failed to parse Hermes config YAML: {e}"
            ))
        })?;
        Ok(config)
    }

    /// Write the config to `path` as YAML.
    pub fn write(&self, path: &std::path::Path) -> Result<(), tokenwise_common::TokenwiseError> {
        let yaml = serde_yaml::to_string(self).map_err(|e| {
            tokenwise_common::TokenwiseError::InvalidInvocation(format!(
                "Failed to serialize Hermes config: {e}"
            ))
        })?;
        std::fs::write(path, yaml)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermes_config_round_trips_yaml() {
        let mut config = HermesConfig::default();
        config.mcp_servers.insert(
            "markitdown".to_string(),
            HermesMcpServer {
                command: "python3".to_string(),
                args: vec!["-m".to_string(), "markitdown_mcp".to_string()],
                env: None,
            },
        );
        config
            .env
            .insert("ANTHROPIC_BASE_URL".to_string(), "http://127.0.0.1:8788".to_string());

        let yaml = serde_yaml::to_string(&config).unwrap();
        let restored: HermesConfig = serde_yaml::from_str(&yaml).unwrap();

        assert!(restored.mcp_servers.contains_key("markitdown"));
        assert_eq!(
            restored.env.get("ANTHROPIC_BASE_URL"),
            Some(&"http://127.0.0.1:8788".to_string())
        );
    }

    #[test]
    fn read_or_default_returns_default_for_missing_file() {
        let config =
            HermesConfig::read_or_default(std::path::Path::new("/nonexistent/config.yaml"))
                .unwrap();
        assert!(config.mcp_servers.is_empty());
        assert!(config.env.is_empty());
    }
}
