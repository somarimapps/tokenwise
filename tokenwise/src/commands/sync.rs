use std::path::PathBuf;

use tokenwise_common::{BackupManager, ExitCode};
use tokenwise_core::service::create_service_manager;
use tokenwise_core::sync::{format_sync_output, reload_headroom_if_changed, RepairStatus, SyncRunner};

/// Scan and repair hook command paths in `settings.json` and MCP command paths
/// in `~/.claude.json`. After any repair, reload the Headroom LaunchAgent plist.
pub async fn run() -> Result<(), ExitCode> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let settings_path = home.join(".claude").join("settings.json");

    // C-003: pass ~/.claude.json so MCP command paths are also scanned
    let claude_json_path = home.join(".claude.json");
    let mcp_config_path = if claude_json_path.exists() {
        Some(claude_json_path.clone())
    } else {
        None
    };

    // W-004: backup dir changed from ~/.claude/tokenwise-backups to ~/.tokenwise/backups
    let backup_dir = home.join(".tokenwise").join("backups");
    let runner = SyncRunner::new(BackupManager::new(backup_dir));

    match runner.run(&settings_path, mcp_config_path.as_deref()) {
        Ok(results) => {
            let output = format_sync_output(&results);
            println!("{}", output);

            let paths_repaired = results
                .iter()
                .filter(|r| matches!(r.status, RepairStatus::Repaired { .. }))
                .count();

            // C-004: reload Headroom if any paths were repaired
            if paths_repaired > 0 {
                let service_manager = create_service_manager();
                #[cfg(target_os = "macos")]
                let service_name = "com.headroom.proxy";
                #[cfg(not(target_os = "macos"))]
                let service_name = "headroom-proxy";

                let unit_path = service_manager.unit_file_path(service_name);
                let reloaded = reload_headroom_if_changed(&unit_path, None).await;
                if reloaded {
                    println!("[INFO] Headroom service reloaded.");
                }
            }

            let any_unresolved = results
                .iter()
                .any(|r| matches!(r.status, RepairStatus::Unresolved { .. }));

            if any_unresolved {
                Err(ExitCode::Failure)
            } else {
                Ok(())
            }
        }
        Err(e) => {
            eprintln!("[FAIL] sync error: {}", e);
            Err(ExitCode::Failure)
        }
    }
}
