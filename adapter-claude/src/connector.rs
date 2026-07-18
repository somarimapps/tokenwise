use std::path::PathBuf;

use serde_json::{json, Value};
use tokenwise_common::{BackupManager, TokenwiseError};
use tokenwise_core::{
    mcp_registry::McpRegistry,
    rules_writer::RulesWriter,
    settings::{models::ClaudeSettings, SettingsManager},
};

use crate::{core_mcp_servers, optional_mcp_servers, HEADROOM_BASE_URL};

const TOKENWISE_MARKER: &str = "tokenwise";

/// Connects Claude Code to the tokenwise optimization stack.
///
/// Responsibilities:
/// - Set `ANTHROPIC_BASE_URL` in `~/.claude/settings.json`
/// - Inject RTK pre-tool-use and session-start hooks
/// - Register all core MCP servers in `~/.claude.json`
/// - Write 5 rule files to `~/.claude/rules/`
///
/// All operations are idempotent: calling `connect` twice produces the same result.
pub struct ClaudeConnector {
    /// `~/.claude/` directory (injectable for tests).
    pub claude_dir: PathBuf,
    /// `~/.claude.json` path (the MCP config file).
    pub claude_json_path: PathBuf,
    /// Directory where backups are written.
    pub backup_dir: PathBuf,
}

impl ClaudeConnector {
    /// Create a connector pointing at real user paths under `$HOME`.
    pub fn new() -> Result<Self, TokenwiseError> {
        let home = dirs::home_dir().ok_or_else(|| {
            TokenwiseError::NotFound("Could not locate home directory".to_string())
        })?;
        Ok(Self::with_dirs(
            home.join(".claude"),
            home.join(".claude.json"),
            home.join(".tokenwise").join("backups"),
        ))
    }

    /// Create a connector with explicit paths — used by tests.
    pub fn with_dirs(
        claude_dir: PathBuf,
        claude_json_path: PathBuf,
        backup_dir: PathBuf,
    ) -> Self {
        Self {
            claude_dir,
            claude_json_path,
            backup_dir,
        }
    }

    fn backup_manager(&self) -> BackupManager {
        BackupManager::new(self.backup_dir.clone())
    }

    /// Run the full connect flow (idempotent).
    ///
    /// Steps:
    /// 1. Ensure `~/.claude/` exists.
    /// 2. Backup + update `settings.json` (env + hooks).
    /// 3. Backup + write MCP servers to `.claude.json`.
    /// 4. Write 5 rule files to `~/.claude/rules/`.
    pub fn connect(&self, include_optional_mcps: bool) -> Result<(), TokenwiseError> {
        // 1. Ensure the Claude directory exists.
        std::fs::create_dir_all(&self.claude_dir)?;

        // 2. Update settings.json (idempotent: hook marker check happens inside).
        let settings_path = self.claude_dir.join("settings.json");
        let rtk_path = Self::resolve_rtk_path();

        let settings_manager = SettingsManager::new(self.backup_manager());
        settings_manager.update_settings(&settings_path, |s| {
            // Always (re)set ANTHROPIC_BASE_URL — it's a no-op if already correct.
            SettingsManager::set_anthropic_base_url(s, HEADROOM_BASE_URL);

            // Only inject hooks if the tokenwise marker is not already present.
            if !Self::has_tokenwise_hooks(s) {
                Self::merge_rtk_hooks(s, &rtk_path);
            }
        })?;

        // 3. Write MCP servers (idempotent merge).
        let mut servers = core_mcp_servers();
        if include_optional_mcps {
            servers.extend(optional_mcp_servers());
        }
        let registry = McpRegistry::new(self.backup_manager());
        registry.write_servers(&self.claude_json_path, &servers)?;

        // 4. Write rules (idempotent).
        let rules_dir = self.claude_dir.join("rules");
        RulesWriter.write_all(&rules_dir)?;

        Ok(())
    }

    /// Check whether tokenwise hooks are already present in `settings.hooks`.
    fn has_tokenwise_hooks(settings: &ClaudeSettings) -> bool {
        if let Some(hooks) = &settings.hooks {
            return hooks.to_string().contains(TOKENWISE_MARKER);
        }
        false
    }

    /// Merge RTK hook entries into `settings.hooks`.
    ///
    /// Adds a `PreToolUse` and a `SessionStart` entry, each with an inline
    /// hook object carrying `"marker": "tokenwise"` for idempotency detection.
    fn merge_rtk_hooks(settings: &mut ClaudeSettings, rtk_path: &str) {
        let pre_hook = json!({
            "type": "command",
            "command": rtk_path,
            "args": [],
            "marker": TOKENWISE_MARKER
        });

        let session_hook = json!({
            "type": "command",
            "command": rtk_path,
            "args": ["init"],
            "marker": TOKENWISE_MARKER
        });

        let hooks = settings
            .hooks
            .get_or_insert_with(|| Value::Object(Default::default()));

        // Ensure it's an object (defensive: it could be null in edge cases).
        if !hooks.is_object() {
            *hooks = Value::Object(Default::default());
        }

        let obj = hooks.as_object_mut().expect("hooks must be an object");

        // PreToolUse
        let pre_tool_use = obj
            .entry("PreToolUse")
            .or_insert_with(|| Value::Array(vec![]));
        if let Value::Array(arr) = pre_tool_use {
            arr.push(json!({ "matcher": "", "hooks": [pre_hook] }));
        }

        // SessionStart
        let session_start = obj
            .entry("SessionStart")
            .or_insert_with(|| Value::Array(vec![]));
        if let Value::Array(arr) = session_start {
            arr.push(json!({ "hooks": [session_hook] }));
        }
    }

    /// Resolve the canonical RTK binary path.
    ///
    /// Tries `which rtk` first; falls back to `/usr/local/bin/rtk`.
    /// Rejects paths under `/tmp` or `/private/tmp` (macOS post-reboot wipe).
    pub fn resolve_rtk_path() -> String {
        if let Ok(output) = std::process::Command::new("which").arg("rtk").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty()
                    && !path.starts_with("/tmp")
                    && !path.starts_with("/private/tmp")
                {
                    return path;
                }
            }
        }
        "/usr/local/bin/rtk".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokenwise_core::settings::models::{ClaudeMcpConfig, ClaudeSettings};

    fn setup(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("tw_claude_conn_{}", name));
        fs::create_dir_all(&base).unwrap();
        let claude_dir = base.join("claude");
        let claude_json = base.join("claude.json");
        let backup_dir = base.join("backups");
        (base, claude_dir, claude_json, backup_dir)
    }

    fn make_connector(
        claude_dir: PathBuf,
        claude_json: PathBuf,
        backup_dir: PathBuf,
    ) -> ClaudeConnector {
        ClaudeConnector::with_dirs(claude_dir, claude_json, backup_dir)
    }

    /// test::connect_claude::writes_anthropic_base_url
    #[test]
    fn writes_anthropic_base_url() {
        let (base, claude_dir, claude_json, backup_dir) = setup("write_url");
        let conn = make_connector(claude_dir.clone(), claude_json, backup_dir);
        conn.connect(false).unwrap();

        let content = fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        assert!(
            content.contains("ANTHROPIC_BASE_URL"),
            "settings.json must contain ANTHROPIC_BASE_URL: {content}"
        );
        assert!(
            content.contains("127.0.0.1:8788"),
            "settings.json must contain the headroom URL: {content}"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_claude::preserves_existing_env_keys
    #[test]
    fn preserves_existing_env_keys() {
        let (base, claude_dir, claude_json, backup_dir) = setup("preserve_env");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("settings.json"),
            r#"{"env": {"MY_KEY": "my_value"}, "enabledPlugins": []}"#,
        )
        .unwrap();

        let conn = make_connector(claude_dir.clone(), claude_json, backup_dir);
        conn.connect(false).unwrap();

        let content = fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        assert!(
            content.contains("MY_KEY"),
            "Existing env key must be preserved: {content}"
        );
        assert!(
            content.contains("ANTHROPIC_BASE_URL"),
            "ANTHROPIC_BASE_URL must be added: {content}"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_claude::hooks_use_absolute_non_tmp_paths
    #[test]
    fn hooks_use_absolute_non_tmp_paths() {
        let (base, claude_dir, claude_json, backup_dir) = setup("hook_paths");
        let conn = make_connector(claude_dir.clone(), claude_json, backup_dir);
        conn.connect(false).unwrap();

        let settings: ClaudeSettings = serde_json::from_str(
            &fs::read_to_string(claude_dir.join("settings.json")).unwrap(),
        )
        .unwrap();

        let paths = settings.hook_command_paths();
        assert!(!paths.is_empty(), "At least one hook command path must be present");
        for path in &paths {
            assert!(
                path.starts_with('/'),
                "Hook command must be an absolute path: {path}"
            );
            assert!(
                !path.starts_with("/tmp"),
                "Hook command must not start with /tmp: {path}"
            );
            assert!(
                !path.starts_with("/private/tmp"),
                "Hook command must not start with /private/tmp: {path}"
            );
        }

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_claude::creates_rules_dir_if_absent
    #[test]
    fn creates_rules_dir_if_absent() {
        let (base, claude_dir, claude_json, backup_dir) = setup("rules_dir");
        // Do NOT pre-create claude_dir; connect() must handle creation.
        let conn = make_connector(claude_dir.clone(), claude_json, backup_dir);
        conn.connect(false).unwrap();

        let rules_dir = claude_dir.join("rules");
        assert!(rules_dir.exists(), "Rules directory must be created");
        assert!(
            rules_dir.join("headroom-pipeline.md").exists(),
            "headroom-pipeline.md must be written"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_claude::idempotent_no_diff
    #[test]
    fn idempotent_no_diff() {
        let (base, claude_dir, claude_json, backup_dir) = setup("idempotent");
        let conn = make_connector(claude_dir.clone(), claude_json.clone(), backup_dir);

        conn.connect(false).unwrap();
        let settings_first = fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let mcp_first = fs::read_to_string(&claude_json).unwrap();

        conn.connect(false).unwrap();
        let settings_second = fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let mcp_second = fs::read_to_string(&claude_json).unwrap();

        assert_eq!(
            settings_first, settings_second,
            "settings.json must be byte-equal after second connect"
        );
        assert_eq!(
            mcp_first, mcp_second,
            ".claude.json must be byte-equal after second connect"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rtk_path_is_absolute_and_not_tmp() {
        let path = ClaudeConnector::resolve_rtk_path();
        assert!(path.starts_with('/'), "RTK path must be absolute: {path}");
        assert!(!path.starts_with("/tmp"), "RTK path must not be under /tmp: {path}");
        assert!(
            !path.starts_with("/private/tmp"),
            "RTK path must not be under /private/tmp: {path}"
        );
    }

    #[test]
    fn connect_writes_all_seven_core_mcp_servers() {
        let (base, claude_dir, claude_json, backup_dir) = setup("mcp_all_servers");
        let conn = make_connector(claude_dir, claude_json.clone(), backup_dir);
        conn.connect(false).unwrap();

        let config: ClaudeMcpConfig = serde_json::from_str(
            &fs::read_to_string(&claude_json).unwrap(),
        )
        .unwrap();

        for name in tokenwise_common::CORE_MCP_SERVER_NAMES {
            assert!(
                config.mcp_servers.contains_key(*name),
                "MCP server '{}' must be registered",
                name
            );
        }

        fs::remove_dir_all(&base).ok();
    }
}
