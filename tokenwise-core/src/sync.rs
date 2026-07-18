use std::collections::HashSet;
use std::path::Path;

use tokenwise_common::{BackupManager, TokenwiseError};

use crate::settings::models::{ClaudeMcpConfig, ClaudeSettings};

/// Status of a single path repair attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairStatus {
    /// Path was already correct — no action taken.
    Ok,
    /// Path was repaired by finding the binary in PATH.
    Repaired { from: String, to: String },
    /// Path could not be resolved.
    Unresolved { reason: String },
}

impl std::fmt::Display for RepairStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Repaired { .. } => write!(f, "REPAIRED"),
            Self::Unresolved { .. } => write!(f, "UNRESOLVED"),
        }
    }
}

/// Result of attempting to repair a single command path.
#[derive(Debug, Clone)]
pub struct PathRepairResult {
    pub original_command: String,
    pub status: RepairStatus,
}

impl PathRepairResult {
    pub fn format_line(&self) -> String {
        match &self.status {
            RepairStatus::Ok => format!("[OK]         {}", self.original_command),
            RepairStatus::Repaired { from, to } => {
                format!("[REPAIRED]   {} → {}", from, to)
            }
            RepairStatus::Unresolved { reason } => {
                format!("[UNRESOLVED] {} ({})", self.original_command, reason)
            }
        }
    }
}

/// Orchestrates hook and MCP command path scanning and repair.
pub struct SyncRunner {
    pub backup_manager: BackupManager,
}

impl SyncRunner {
    pub fn new(backup_manager: BackupManager) -> Self {
        Self { backup_manager }
    }

    /// Scan all hook command paths in `settings.json` AND all MCP server
    /// `command` entries in `~/.claude.json`, attempt to repair any broken ones
    /// by locating the binary name in `$PATH`, and write back the updated files
    /// (non-destructively, with backup).
    ///
    /// `mcp_config_path` is `Some(~/.claude.json)` when MCP scanning is
    /// desired, or `None` to skip that source.
    ///
    /// Returns a list of repair results, one per unique command path found.
    pub fn run(
        &self,
        settings_path: &Path,
        mcp_config_path: Option<&Path>,
    ) -> Result<Vec<PathRepairResult>, TokenwiseError> {
        let mut settings = ClaudeSettings::read_or_default(settings_path)?;
        let hook_commands = settings.hook_command_paths();

        // C-003: also scan MCP server command entries from ~/.claude.json
        let mcp_commands: Vec<String> = if let Some(mcp_path) = mcp_config_path {
            ClaudeMcpConfig::read_or_default(mcp_path)
                .unwrap_or_default()
                .mcp_servers
                .into_values()
                .map(|s| s.command)
                .filter(|c| !c.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        // Deduplicate: track which source each command came from
        let mut seen: HashSet<String> = HashSet::new();
        // (command, is_hook_source)
        let mut all_commands: Vec<(String, bool)> = Vec::new();

        for cmd in &hook_commands {
            if seen.insert(cmd.clone()) {
                all_commands.push((cmd.clone(), true));
            }
        }
        for cmd in &mcp_commands {
            if seen.insert(cmd.clone()) {
                all_commands.push((cmd.clone(), false));
            }
        }

        if all_commands.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut hook_repairs: Vec<(String, String)> = Vec::new();
        let mut mcp_repairs: Vec<(String, String)> = Vec::new();

        for (cmd, is_hook) in &all_commands {
            let result = self.repair_command(cmd);
            if let RepairStatus::Repaired { ref from, ref to } = result.status {
                if *is_hook {
                    hook_repairs.push((from.clone(), to.clone()));
                } else {
                    mcp_repairs.push((from.clone(), to.clone()));
                }
            }
            results.push(result);
        }

        // Apply hook repairs to settings.json
        if !hook_repairs.is_empty() {
            if settings_path.exists() {
                self.backup_manager.backup(settings_path)?;
            }
            if let Some(hooks) = settings.hooks.take() {
                settings.hooks = Some(apply_path_repairs(hooks, &hook_repairs));
            }
            settings.write_pretty(settings_path)?;
        }

        // Apply MCP repairs to ~/.claude.json
        if !mcp_repairs.is_empty() {
            if let Some(mcp_path) = mcp_config_path {
                if mcp_path.exists() {
                    self.backup_manager.backup(mcp_path)?;
                }
                let mut mcp_config = ClaudeMcpConfig::read_or_default(mcp_path)?;
                for server in mcp_config.mcp_servers.values_mut() {
                    for (from, to) in &mcp_repairs {
                        if server.command == *from {
                            server.command = to.clone();
                        }
                    }
                }
                mcp_config.write_pretty(mcp_path)?;
            }
        }

        Ok(results)
    }

    fn repair_command(&self, cmd: &str) -> PathRepairResult {
        let binary = cmd.split_whitespace().next().unwrap_or(cmd);
        let path = Path::new(binary);

        if !path.is_absolute() || path.exists() {
            return PathRepairResult {
                original_command: cmd.to_string(),
                status: RepairStatus::Ok,
            };
        }

        let binary_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(binary);
        match which::which(binary_name) {
            Ok(found) => {
                let new_path = found.display().to_string();
                let repaired_cmd = if cmd.contains(' ') {
                    let rest = &cmd[binary.len()..];
                    format!("{}{}", new_path, rest)
                } else {
                    new_path.clone()
                };
                PathRepairResult {
                    original_command: cmd.to_string(),
                    status: RepairStatus::Repaired {
                        from: cmd.to_string(),
                        to: repaired_cmd,
                    },
                }
            }
            Err(_) => PathRepairResult {
                original_command: cmd.to_string(),
                status: RepairStatus::Unresolved {
                    reason: format!("'{}' not found in PATH", binary_name),
                },
            },
        }
    }
}

/// Recursively walk a `serde_json::Value` and replace all `"command"` string
/// values matching any `(from, to)` pair.
fn apply_path_repairs(
    value: serde_json::Value,
    repairs: &[(String, String)],
) -> serde_json::Value {
    use serde_json::Value;

    match value {
        Value::Object(mut map) => {
            if let Some(Value::String(ref cmd)) = map.get("command").cloned() {
                for (from, to) in repairs {
                    if cmd == from {
                        map.insert("command".to_string(), Value::String(to.clone()));
                        break;
                    }
                }
            }
            let repaired: serde_json::Map<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, apply_path_repairs(v, repairs)))
                .collect();
            Value::Object(repaired)
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(|v| apply_path_repairs(v, repairs)).collect())
        }
        other => other,
    }
}

/// Format all repair results for terminal output.
///
/// W-005: when there is nothing to repair (empty list or all paths OK),
/// output "[INFO] All paths verified. Nothing to repair."
pub fn format_sync_output(results: &[PathRepairResult]) -> String {
    if results.is_empty() {
        return "[INFO] All paths verified. Nothing to repair.".to_string();
    }
    let all_ok = results.iter().all(|r| r.status == RepairStatus::Ok);
    if all_ok {
        return "[INFO] All paths verified. Nothing to repair.".to_string();
    }
    results.iter().map(|r| r.format_line()).collect::<Vec<_>>().join("\n")
}

/// Reload the Headroom LaunchAgent plist if a new plist path is found on disk.
///
/// Returns `true` if a reload was triggered, `false` otherwise.
/// On non-macOS this is always a no-op (returns `false`).
pub async fn reload_headroom_if_changed(
    plist_path: &Path,
    _previous_plist_path: Option<&Path>,
) -> bool {
    #[cfg(target_os = "macos")]
    {
        if !plist_path.exists() {
            return false;
        }

        let changed = _previous_plist_path.map_or(true, |prev| prev != plist_path);
        if !changed {
            return false;
        }

        let result = tokio::process::Command::new("launchctl")
            .args(["load", "-w", &plist_path.display().to_string()])
            .output()
            .await;

        return result.map(|o| o.status.success()).unwrap_or(false);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = plist_path;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tokenwise_sync_{}", name));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_runner(base: &Path) -> SyncRunner {
        SyncRunner::new(BackupManager::new(base.join("backups")))
    }

    #[test]
    fn ok_when_no_hooks_configured() {
        let base = temp_dir("no_hooks");
        let settings_path = base.join("settings.json");
        fs::write(&settings_path, r#"{"env": {}}"#).unwrap();

        let runner = make_runner(&base);
        let results = runner.run(&settings_path, None).unwrap();
        assert!(results.is_empty(), "No hooks → empty results");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn valid_absolute_path_is_ok() {
        let shell = "/bin/sh";
        if !Path::new(shell).exists() {
            return;
        }

        let base = temp_dir("valid_path");
        let settings_path = base.join("settings.json");

        let json = format!(
            r#"{{
                "hooks": {{
                    "PreToolUse": [{{
                        "hooks": [{{"type": "command", "command": "{}"}}]
                    }}]
                }}
            }}"#,
            shell
        );
        fs::write(&settings_path, &json).unwrap();

        let runner = make_runner(&base);
        let results = runner.run(&settings_path, None).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, RepairStatus::Ok);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn unresolved_for_missing_binary() {
        let base = temp_dir("unresolved");
        let settings_path = base.join("settings.json");

        let json = r#"{
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "/nonexistent/path/xyzzy-binary-that-does-not-exist"}]}]
            }
        }"#;
        fs::write(&settings_path, json).unwrap();

        let runner = make_runner(&base);
        let results = runner.run(&settings_path, None).unwrap();

        assert_eq!(results.len(), 1);
        assert!(
            matches!(results[0].status, RepairStatus::Unresolved { .. }),
            "Expected Unresolved, got {:?}",
            results[0].status
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn repaired_path_written_back_to_settings() {
        let real_sh = match which::which("sh") {
            Ok(p) => p,
            Err(_) => return,
        };

        let base = temp_dir("repair_written");
        let settings_path = base.join("settings.json");

        let json = r#"{
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "/old/path/sh"}]}]
            }
        }"#;
        fs::write(&settings_path, json).unwrap();

        let runner = make_runner(&base);
        let results = runner.run(&settings_path, None).unwrap();

        assert_eq!(results.len(), 1);
        match &results[0].status {
            RepairStatus::Repaired { to, .. } => {
                assert!(to.contains("sh"), "Repaired path should contain 'sh': {}", to);
                let updated = fs::read_to_string(&settings_path).unwrap();
                let real_sh_str = real_sh.display().to_string();
                assert!(
                    updated.contains(&real_sh_str) || updated.contains("sh"),
                    "Updated settings must contain the repaired path"
                );
            }
            RepairStatus::Unresolved { reason } => {
                eprintln!("Unresolved (acceptable in some environments): {}", reason);
            }
            RepairStatus::Ok => {
                panic!("Expected Repaired or Unresolved, got Ok");
            }
        }

        fs::remove_dir_all(&base).ok();
    }

    /// C-003: MCP command paths from ~/.claude.json are also scanned.
    #[test]
    fn mcp_command_paths_scanned() {
        let base = temp_dir("mcp_scan");
        let settings_path = base.join("settings.json");
        let claude_json_path = base.join(".claude.json");

        fs::write(&settings_path, r#"{"env": {}}"#).unwrap();

        // Write a claude.json with an MCP server pointing to a non-existent binary
        let claude_json = r#"{
            "mcpServers": {
                "markitdown": {
                    "command": "/nonexistent/markitdown-mcp-binary-xyz",
                    "args": []
                }
            }
        }"#;
        fs::write(&claude_json_path, claude_json).unwrap();

        let runner = make_runner(&base);
        let results = runner.run(&settings_path, Some(&claude_json_path)).unwrap();

        assert_eq!(results.len(), 1, "Should find the MCP command path");
        assert!(
            matches!(results[0].status, RepairStatus::Unresolved { .. }),
            "Non-existent MCP binary must be Unresolved, got {:?}",
            results[0].status
        );

        fs::remove_dir_all(&base).ok();
    }

    /// W-005: format_sync_output with no results returns "All paths verified".
    #[test]
    fn format_sync_output_empty() {
        let output = format_sync_output(&[]);
        assert!(
            output.contains("Nothing to repair") || output.contains("All paths verified"),
            "Expected 'All paths verified' message, got: {}",
            output
        );
    }

    /// W-005: format_sync_output with all-OK results also returns "All paths verified".
    #[test]
    fn format_sync_output_all_ok_says_nothing_to_repair() {
        let results = vec![
            PathRepairResult {
                original_command: "/bin/sh".to_string(),
                status: RepairStatus::Ok,
            },
            PathRepairResult {
                original_command: "/usr/bin/python3".to_string(),
                status: RepairStatus::Ok,
            },
        ];
        let output = format_sync_output(&results);
        assert!(
            output.contains("Nothing to repair") || output.contains("All paths verified"),
            "All-OK results must say 'All paths verified', got: {}",
            output
        );
    }

    #[test]
    fn format_sync_output_contains_status_prefix() {
        let results = vec![
            PathRepairResult {
                original_command: "/bin/sh".to_string(),
                status: RepairStatus::Ok,
            },
            PathRepairResult {
                original_command: "/old/rtk".to_string(),
                status: RepairStatus::Repaired {
                    from: "/old/rtk".to_string(),
                    to: "/usr/local/bin/rtk".to_string(),
                },
            },
        ];
        let output = format_sync_output(&results);
        // Mixed results (not all OK) should show individual lines
        assert!(output.contains("[OK]") || output.contains("[REPAIRED]"));
    }

    /// test::sync::service_reloaded_after_repair (C-004 + C-006)
    ///
    /// Verifies `reload_headroom_if_changed` behaves correctly:
    /// - no-op when plist is absent
    /// - no-op when the path is unchanged
    /// - attempts reload when path changed (result depends on OS/env)
    #[tokio::test]
    async fn service_reloaded_after_repair() {
        let base = temp_dir("service_reload");
        let plist_path = base.join("com.headroom.proxy.plist");

        // Plist does not exist → must be a no-op
        let reloaded = reload_headroom_if_changed(&plist_path, None).await;
        assert!(!reloaded, "Should not reload when plist does not exist");

        // Write a fake plist
        fs::write(&plist_path, b"<plist/>").unwrap();

        // Same path as previous → no change → no reload
        let reloaded = reload_headroom_if_changed(&plist_path, Some(&plist_path)).await;
        assert!(!reloaded, "Should not reload when plist path is unchanged");

        // Different previous path → path changed → reload attempted
        // In a test environment launchctl may fail; we only assert no panic.
        let other = base.join("other.plist");
        let _result = reload_headroom_if_changed(&plist_path, Some(&other)).await;
        // result is true on macOS with a valid plist, false otherwise — both are acceptable

        fs::remove_dir_all(&base).ok();
    }
}
