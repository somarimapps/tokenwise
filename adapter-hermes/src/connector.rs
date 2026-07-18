use std::path::PathBuf;

use tokenwise_common::{BackupManager, TokenwiseError};

use crate::models::{HermesConfig, HermesHook, HermesMcpServer};
use adapter_claude::{core_mcp_servers, ClaudeConnector, HEADROOM_BASE_URL};

const TOKENWISE_MARKER: &str = "tokenwise";
const PRE_TOOL_USE_KEY: &str = "preToolUse";
const SESSION_START_KEY: &str = "sessionStart";

/// Connects the Hermes agent to the tokenwise optimization stack.
///
/// Responsibilities:
/// - Set `ANTHROPIC_BASE_URL` in `~/.hermes/config.yaml`
/// - Register all core MCP servers in the Hermes YAML config
/// - Inject RTK pre-tool-use and session-start hooks
///
/// All operations are idempotent: calling `connect` twice produces the same result.
pub struct HermesConnector {
    /// `~/.hermes/` config directory (injectable for tests).
    pub config_dir: PathBuf,
    /// Directory where backups are written.
    pub backup_dir: PathBuf,
    /// Test-only override for `is_installed()`. `None` = use real detection.
    installed_override: Option<bool>,
}

impl HermesConnector {
    /// Create a connector pointing at real user paths under `$HOME`.
    pub fn new() -> Result<Self, TokenwiseError> {
        let home = dirs::home_dir().ok_or_else(|| {
            TokenwiseError::NotFound("Could not locate home directory".to_string())
        })?;
        Ok(Self::with_config_dir(
            home.join(".hermes"),
            home.join(".tokenwise").join("backups"),
        ))
    }

    /// Create a connector with explicit paths — used by tests.
    pub fn with_config_dir(config_dir: PathBuf, backup_dir: PathBuf) -> Self {
        Self {
            config_dir,
            backup_dir,
            installed_override: None,
        }
    }

    /// Force-set the result of `is_installed()` — test helper only.
    #[cfg(test)]
    pub fn with_installed_override(mut self, installed: bool) -> Self {
        self.installed_override = Some(installed);
        self
    }

    fn backup_manager(&self) -> BackupManager {
        BackupManager::new(self.backup_dir.clone())
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.yaml")
    }

    /// Check whether hermes is installed on this machine.
    ///
    /// Returns `true` if:
    /// - `self.installed_override` is `Some(true)` (test injection), OR
    /// - `self.config_dir` already exists (previously installed / test-injected), OR
    /// - `which hermes` succeeds (binary in PATH)
    pub fn is_installed(&self) -> bool {
        if let Some(v) = self.installed_override {
            return v;
        }
        if self.config_dir.exists() {
            return true;
        }
        std::process::Command::new("which")
            .arg("hermes")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Run the full connect flow (idempotent).
    ///
    /// Steps:
    /// 1. Verify hermes is installed.
    /// 2. Ensure `~/.hermes/` exists.
    /// 3. Backup + read existing config (or default).
    /// 4. Merge MCP servers (non-destructive: skip keys already present).
    /// 5. Set `ANTHROPIC_BASE_URL` in env.
    /// 6. Inject RTK hooks if tokenwise marker absent.
    /// 7. Write back.
    pub fn connect(&self) -> Result<(), TokenwiseError> {
        // 1. Guard: hermes must be installed.
        if !self.is_installed() {
            return Err(TokenwiseError::MissingPrerequisite(
                "Hermes agent is not installed. Install it first with `pip install hermes-agent`."
                    .to_string(),
            ));
        }

        // 2. Ensure config directory exists.
        std::fs::create_dir_all(&self.config_dir)?;

        let config_path = self.config_path();

        // 3. Backup before modification.
        if config_path.exists() {
            self.backup_manager().backup(&config_path)?;
        }

        // 4. Read or default.
        let mut config = HermesConfig::read_or_default(&config_path)?;

        // 5. Merge MCP servers (skip servers already registered).
        for (name, server_cfg) in core_mcp_servers() {
            config.mcp_servers.entry(name).or_insert(HermesMcpServer {
                command: server_cfg.command,
                args: server_cfg.args,
                env: None,
            });
        }

        // 6. Set ANTHROPIC_BASE_URL (idempotent — insert only if absent).
        config
            .env
            .entry("ANTHROPIC_BASE_URL".to_string())
            .or_insert_with(|| HEADROOM_BASE_URL.to_string());

        // 7. Inject RTK hooks if the tokenwise marker is absent.
        if !Self::has_tokenwise_hooks(&config) {
            Self::merge_rtk_hooks(&mut config);
        }

        // 8. Write back.
        config.write(&config_path)?;

        Ok(())
    }

    /// Return true if any hook entry carries the tokenwise marker.
    fn has_tokenwise_hooks(config: &HermesConfig) -> bool {
        for hooks in config.hooks.values() {
            for hook in hooks {
                if hook.marker.as_deref() == Some(TOKENWISE_MARKER) {
                    return true;
                }
            }
        }
        false
    }

    /// Add RTK `preToolUse` and `sessionStart` hooks with the tokenwise marker.
    fn merge_rtk_hooks(config: &mut HermesConfig) {
        let rtk_path = ClaudeConnector::resolve_rtk_path();

        let pre_hook = HermesHook {
            command: rtk_path.clone(),
            args: vec![],
            marker: Some(TOKENWISE_MARKER.to_string()),
        };
        let session_hook = HermesHook {
            command: rtk_path,
            args: vec!["init".to_string()],
            marker: Some(TOKENWISE_MARKER.to_string()),
        };

        config
            .hooks
            .entry(PRE_TOOL_USE_KEY.to_string())
            .or_default()
            .push(pre_hook);

        config
            .hooks
            .entry(SESSION_START_KEY.to_string())
            .or_default()
            .push(session_hook);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("tw_hermes_conn_{}", name));
        fs::create_dir_all(&base).unwrap();
        let config_dir = base.join("hermes");
        let backup_dir = base.join("backups");
        (base, config_dir, backup_dir)
    }

    /// test::connect_hermes::registers_mcp_servers
    #[test]
    fn registers_mcp_servers() {
        let (base, config_dir, backup_dir) = setup("mcp_servers");
        // Pre-create config_dir so is_installed() returns true.
        fs::create_dir_all(&config_dir).unwrap();

        let conn = HermesConnector::with_config_dir(config_dir.clone(), backup_dir);
        conn.connect().unwrap();

        let config = HermesConfig::read_or_default(&config_dir.join("config.yaml")).unwrap();
        for name in tokenwise_common::CORE_MCP_SERVER_NAMES {
            assert!(
                config.mcp_servers.contains_key(*name),
                "Hermes config must register MCP server '{name}'"
            );
        }

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_hermes::exits_1_when_not_installed
    #[test]
    fn exits_1_when_not_installed() {
        let base = std::env::temp_dir().join("tw_hermes_conn_not_installed");
        let config_dir = base.join("hermes_nonexistent_xq7z");
        let backup_dir = base.join("backups");

        // Force is_installed() to return false regardless of PATH.
        let conn = HermesConnector::with_config_dir(config_dir, backup_dir)
            .with_installed_override(false);

        let result = conn.connect();
        assert!(
            result.is_err(),
            "connect() must fail when hermes is not installed"
        );
        match result.unwrap_err() {
            TokenwiseError::MissingPrerequisite(_) => {}
            e => panic!("Expected MissingPrerequisite, got {:?}", e),
        }

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_hermes::hooks_are_absolute_paths
    #[test]
    fn hooks_are_absolute_paths() {
        let (base, config_dir, backup_dir) = setup("hook_paths");
        fs::create_dir_all(&config_dir).unwrap();

        let conn = HermesConnector::with_config_dir(config_dir.clone(), backup_dir);
        conn.connect().unwrap();

        let config = HermesConfig::read_or_default(&config_dir.join("config.yaml")).unwrap();
        for hooks in config.hooks.values() {
            for hook in hooks {
                assert!(
                    hook.command.starts_with('/'),
                    "Hook command must be absolute: {}",
                    hook.command
                );
                assert!(
                    !hook.command.starts_with("/tmp"),
                    "Hook command must not be under /tmp: {}",
                    hook.command
                );
                assert!(
                    !hook.command.starts_with("/private/tmp"),
                    "Hook command must not be under /private/tmp: {}",
                    hook.command
                );
            }
        }

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_hermes::idempotent
    #[test]
    fn idempotent() {
        let (base, config_dir, backup_dir) = setup("idempotent");
        fs::create_dir_all(&config_dir).unwrap();

        let conn = HermesConnector::with_config_dir(config_dir.clone(), backup_dir);
        conn.connect().unwrap();
        let first = fs::read_to_string(config_dir.join("config.yaml")).unwrap();

        conn.connect().unwrap();
        let second = fs::read_to_string(config_dir.join("config.yaml")).unwrap();

        assert_eq!(first, second, "config.yaml must be byte-equal after second connect");

        fs::remove_dir_all(&base).ok();
    }
}
