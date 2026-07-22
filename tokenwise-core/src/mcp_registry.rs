use std::collections::HashMap;
use std::path::Path;

use tokenwise_common::{BackupManager, TokenwiseError};

use crate::settings::models::{ClaudeMcpConfig, McpServerConfig};

/// Manages non-destructive reads and writes of the `mcpServers` map in
/// `~/.claude.json`.
pub struct McpRegistry {
    pub backup_manager: BackupManager,
}

impl McpRegistry {
    pub fn new(backup_manager: BackupManager) -> Self {
        Self { backup_manager }
    }

    /// Register or update MCP servers in `~/.claude.json`.
    ///
    /// - Backs up the existing file before writing.
    /// - Merges new servers into the existing map (idempotent).
    /// - Preserves any server entries not in `servers` (user-configured).
    pub fn write_servers(
        &self,
        path: &Path,
        servers: &HashMap<String, McpServerConfig>,
    ) -> Result<(), TokenwiseError> {
        // Backup before modification
        if path.exists() {
            self.backup_manager.backup(path)?;
        }

        // Read or create default
        let mut config = ClaudeMcpConfig::read_or_default(path)?;

        // Merge: add missing servers only; never overwrite existing entries.
        // This preserves user-customized commands (e.g. venv-specific python paths).
        for (name, server) in servers {
            config.mcp_servers.entry(name.clone()).or_insert_with(|| server.clone());
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        config.write_pretty(path)?;
        Ok(())
    }

    /// Check whether all given server names are registered.
    pub fn all_registered(path: &Path, names: &[&str]) -> Result<bool, TokenwiseError> {
        if !path.exists() {
            return Ok(false);
        }
        let config = ClaudeMcpConfig::read_or_default(path)?;
        Ok(names.iter().all(|n| config.mcp_servers.contains_key(*n)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tokenwise_mcp_{}", name));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_server(command: &str) -> McpServerConfig {
        McpServerConfig::stdio(command, vec![])
    }

    fn make_registry(base: &std::path::Path) -> McpRegistry {
        McpRegistry::new(BackupManager::new(base.join("backups")))
    }

    #[test]
    fn write_servers_is_idempotent() {
        let base = temp_dir("idempotent");
        let config_path = base.join("claude.json");

        let mut servers = HashMap::new();
        servers.insert("markitdown".to_string(), make_server("python3"));

        let registry = make_registry(&base);
        registry.write_servers(&config_path, &servers).unwrap();
        registry.write_servers(&config_path, &servers).unwrap();

        let config = ClaudeMcpConfig::read_or_default(&config_path).unwrap();
        assert!(config.mcp_servers.contains_key("markitdown"));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn preserves_existing_user_servers() {
        let base = temp_dir("preserve");
        let config_path = base.join("claude.json");

        // Pre-populate with a user server
        let existing = r#"{"mcpServers": {"user-server": {"command": "my-tool", "args": []}}}"#;
        fs::write(&config_path, existing).unwrap();

        let mut servers = HashMap::new();
        servers.insert("markitdown".to_string(), make_server("python3"));

        let registry = make_registry(&base);
        registry.write_servers(&config_path, &servers).unwrap();

        let config = ClaudeMcpConfig::read_or_default(&config_path).unwrap();
        assert!(config.mcp_servers.contains_key("user-server"), "Pre-existing server must be preserved");
        assert!(config.mcp_servers.contains_key("markitdown"), "New server must be added");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn all_registered_returns_false_for_missing_servers() {
        let base = temp_dir("check_missing");
        let config_path = base.join("claude.json");

        let mut servers = HashMap::new();
        servers.insert("markitdown".to_string(), make_server("python3"));

        let registry = make_registry(&base);
        registry.write_servers(&config_path, &servers).unwrap();

        // markitdown is registered, engram is not
        assert!(
            !McpRegistry::all_registered(&config_path, &["markitdown", "engram"]).unwrap(),
            "Should return false when some servers are missing"
        );
        assert!(
            McpRegistry::all_registered(&config_path, &["markitdown"]).unwrap(),
            "Should return true when all servers are present"
        );

        fs::remove_dir_all(&base).ok();
    }
}
