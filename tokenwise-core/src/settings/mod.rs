pub mod models;

use std::path::Path;

use tokenwise_common::{BackupManager, TokenwiseError};

use self::models::ClaudeSettings;

/// Manages non-destructive reads and writes of `~/.claude/settings.json`.
///
/// Safety contract:
/// - Always creates a timestamped backup before writing.
/// - Merges tokenwise-managed keys into the existing JSON.
/// - Preserves every key the user set (via `#[serde(flatten)]` + `extra`).
pub struct SettingsManager {
    pub backup_manager: BackupManager,
}

impl SettingsManager {
    pub fn new(backup_manager: BackupManager) -> Self {
        Self { backup_manager }
    }

    /// Read, merge tokenwise entries, and write back.
    ///
    /// Steps:
    /// 1. Backup the existing file (if present)
    /// 2. Read or create default settings
    /// 3. Apply the provided merge function
    /// 4. Write back as pretty JSON
    pub fn update_settings<F>(
        &self,
        path: &Path,
        merge: F,
    ) -> Result<(), TokenwiseError>
    where
        F: FnOnce(&mut ClaudeSettings),
    {
        // 1. Backup before modification
        if path.exists() {
            self.backup_manager.backup(path)?;
        }

        // 2. Read existing or default
        let mut settings = ClaudeSettings::read_or_default(path)?;

        // 3. Apply caller-provided merge
        merge(&mut settings);

        // 4. Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 5. Write back
        settings.write_pretty(path)?;
        Ok(())
    }

    /// Set `ANTHROPIC_BASE_URL` in the env block, preserving all other env keys.
    pub fn set_anthropic_base_url(settings: &mut ClaudeSettings, url: &str) {
        settings.env.insert("ANTHROPIC_BASE_URL".to_string(), url.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_manager(base: &std::path::Path) -> SettingsManager {
        let backup_dir = base.join("backups");
        SettingsManager::new(BackupManager::new(backup_dir))
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tokenwise_settings_{}", name));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// test::connect_claude::preserves_existing_env_keys
    #[test]
    fn preserves_existing_env_keys() {
        let base = temp_dir("preserve_env");
        let settings_path = base.join("settings.json");
        fs::write(
            &settings_path,
            r#"{"env": {"MY_KEY": "my_value"}, "enabledPlugins": []}"#,
        )
        .unwrap();

        let manager = make_manager(&base);
        manager
            .update_settings(&settings_path, |s| {
                SettingsManager::set_anthropic_base_url(s, "http://127.0.0.1:8788");
            })
            .unwrap();

        let result = fs::read_to_string(&settings_path).unwrap();
        assert!(
            result.contains("MY_KEY"),
            "User env key MY_KEY must be preserved: {result}"
        );
        assert!(
            result.contains("ANTHROPIC_BASE_URL"),
            "ANTHROPIC_BASE_URL must be added: {result}"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// test::connect_claude::writes_anthropic_base_url (partially — full connector is PR2)
    #[test]
    fn writes_anthropic_base_url() {
        let base = temp_dir("write_base_url");
        let settings_path = base.join("settings.json");

        let manager = make_manager(&base);
        manager
            .update_settings(&settings_path, |s| {
                SettingsManager::set_anthropic_base_url(s, "http://127.0.0.1:8788");
            })
            .unwrap();

        let result = fs::read_to_string(&settings_path).unwrap();
        assert!(result.contains("ANTHROPIC_BASE_URL"));
        assert!(result.contains("127.0.0.1:8788"));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn backup_written_before_update() {
        let base = temp_dir("backup_before_update");
        let settings_path = base.join("settings.json");
        let backup_dir = base.join("backups");
        fs::write(&settings_path, r#"{"env": {}}"#).unwrap();

        let manager = make_manager(&base);
        manager
            .update_settings(&settings_path, |_| {})
            .unwrap();

        // A backup must exist in the backups directory
        let backups: Vec<_> = fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(!backups.is_empty(), "Backup must be created before writing");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn unknown_keys_survive_update() {
        let base = temp_dir("unknown_keys");
        let settings_path = base.join("settings.json");
        fs::write(
            &settings_path,
            r#"{"custom_key": "custom_value", "env": {}}"#,
        )
        .unwrap();

        let manager = make_manager(&base);
        manager
            .update_settings(&settings_path, |s| {
                SettingsManager::set_anthropic_base_url(s, "http://127.0.0.1:8788");
            })
            .unwrap();

        let result = fs::read_to_string(&settings_path).unwrap();
        assert!(
            result.contains("custom_value"),
            "User custom_key must survive round-trip: {result}"
        );

        fs::remove_dir_all(&base).ok();
    }
}
