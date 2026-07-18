use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Root structure of `~/.claude/settings.json`.
///
/// Uses `#[serde(flatten)]` for the `extra` field so that any keys we don't
/// explicitly model are preserved unchanged through read → merge → write.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSettings {
    /// Environment variables injected into the Claude Code process.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Hook definitions. Stored as raw JSON to support the full hook DSL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Value>,

    /// Enabled marketplace plugin names (e.g. `["caveman", "ponytail"]`).
    #[serde(default)]
    pub enabled_plugins: Vec<String>,

    /// Permissions block (passed through opaquely).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Value>,

    /// All other keys present in the original file — never dropped.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ClaudeSettings {
    /// Read the settings file, or return a default empty struct if the file
    /// does not exist yet.
    pub fn read_or_default(path: &std::path::Path) -> Result<Self, tokenwise_common::TokenwiseError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let settings: Self = serde_json::from_str(&content)?;
        Ok(settings)
    }

    /// Write the settings struct back as pretty-printed JSON.
    pub fn write_pretty(&self, path: &std::path::Path) -> Result<(), tokenwise_common::TokenwiseError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Return all absolute command paths referenced by hook entries.
    ///
    /// Handles the nested structure:
    /// ```json
    /// { "PreToolUse": [{ "hooks": [{ "type": "command", "command": "/path/to/exe" }] }] }
    /// ```
    pub fn hook_command_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        if let Some(hooks) = &self.hooks {
            extract_commands(hooks, &mut paths);
        }
        paths
    }
}

fn extract_commands(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(cmd) = map.get("command").and_then(Value::as_str) {
                if !cmd.is_empty() {
                    out.push(cmd.to_string());
                }
            }
            for v in map.values() {
                extract_commands(v, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                extract_commands(v, out);
            }
        }
        _ => {}
    }
}

/// Entry in `~/.claude.json` mcpServers map.
///
/// `args` uses a tolerant custom deserializer that accepts arrays, objects,
/// null, or missing values — all map to `Vec<String>`. This handles the real
/// `~/.claude.json` where some MCP servers use non-array arg shapes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default, deserialize_with = "deserialize_args_tolerant")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Some MCP servers use `"type": "sse"` or similar.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
    /// Preserve unknown keys (e.g. `url`, `transport`, `headers`).
    #[serde(flatten, default)]
    pub extra: HashMap<String, Value>,
}

impl McpServerConfig {
    /// Convenience constructor for a standard stdio MCP server.
    pub fn stdio(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            env: None,
            server_type: None,
            extra: HashMap::new(),
        }
    }
}

/// Deserialize `args` tolerantly: accept array of strings, null, or any other
/// JSON value (treat non-array as empty vec, extract strings from mixed arrays).
fn deserialize_args_tolerant<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Array(arr) => Ok(arr
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s),
                other => other.as_str().map(String::from),
            })
            .collect()),
        _ => Ok(Vec::new()),
    }
}

/// Root of `~/.claude.json`.
///
/// `mcp_servers` uses `BTreeMap` so keys are serialized in alphabetical order,
/// which makes the JSON output deterministic across multiple write calls.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMcpConfig {
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, McpServerConfig>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ClaudeMcpConfig {
    pub fn read_or_default(path: &std::path::Path) -> Result<Self, tokenwise_common::TokenwiseError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn write_pretty(&self, path: &std::path::Path) -> Result<(), tokenwise_common::TokenwiseError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_settings_preserves_unknown_keys() {
        let json = r#"{"customKey": "userValue", "env": {}, "unknownArray": [1,2,3]}"#;
        let settings: ClaudeSettings = serde_json::from_str(json).unwrap();

        assert!(
            settings.extra.contains_key("customKey") || {
                // customKey might be captured in extra or env depending on serde
                let re_serialized = serde_json::to_string(&settings).unwrap();
                re_serialized.contains("customKey")
            },
            "Unknown keys must survive round-trip"
        );
    }

    #[test]
    fn hook_command_paths_extracts_nested_commands() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "hooks": [
                            {"type": "command", "command": "/usr/local/bin/rtk"}
                        ]
                    }
                ]
            }
        }"#;
        let settings: ClaudeSettings = serde_json::from_str(json).unwrap();
        let paths = settings.hook_command_paths();
        assert!(!paths.is_empty(), "Should extract command paths");
        assert!(
            paths.contains(&"/usr/local/bin/rtk".to_string()),
            "Should find /usr/local/bin/rtk"
        );
    }

    #[test]
    fn read_or_default_returns_default_for_missing_file() {
        let settings = ClaudeSettings::read_or_default(std::path::Path::new("/nonexistent/settings.json")).unwrap();
        assert!(settings.env.is_empty());
        assert!(settings.enabled_plugins.is_empty());
    }

    #[test]
    fn mcp_config_round_trips() {
        let mut config = ClaudeMcpConfig::default();
        config.mcp_servers.insert(
            "markitdown".to_string(),
            McpServerConfig::stdio("python3", vec!["-m".to_string(), "markitdown_mcp".to_string()]),
        );
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ClaudeMcpConfig = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.mcp_servers.contains_key("markitdown"));
    }

    #[test]
    fn args_tolerates_map_value() {
        // Real ~/.claude.json can have args as an object in some MCP server entries.
        let json = r#"{"command": "my-tool", "args": {"key": "val"}}"#;
        let cfg: McpServerConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.args.is_empty(), "args must default to empty when JSON value is a map");
    }
}
