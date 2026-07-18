use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::error::TokenwiseError;

/// Manages timestamped backups of config files.
///
/// Backup format: `{filename}.{iso8601}.bak`
/// Example:       `settings.json.2024-01-15T103000Z.bak`
pub struct BackupManager {
    /// Directory where backups are written (e.g. `~/.tokenwise/backups/`).
    pub backup_dir: PathBuf,
}

impl BackupManager {
    pub fn new(backup_dir: PathBuf) -> Self {
        Self { backup_dir }
    }

    /// Write a timestamped backup of `path` to `self.backup_dir`.
    ///
    /// Returns the path of the newly created backup file.
    pub fn backup(&self, path: &Path) -> Result<PathBuf, TokenwiseError> {
        if !path.exists() {
            return Err(TokenwiseError::NotFound(format!(
                "File to backup not found: {}",
                path.display()
            )));
        }

        std::fs::create_dir_all(&self.backup_dir)?;

        let filename = path
            .file_name()
            .ok_or_else(|| {
                TokenwiseError::NotFound(format!(
                    "Path has no filename: {}",
                    path.display()
                ))
            })?
            .to_string_lossy();

        // ISO 8601 compact UTC timestamp: 2024-01-15T103000Z
        let timestamp = Utc::now().format("%Y-%m-%dT%H%M%SZ");
        let backup_name = format!("{}.{}.bak", filename, timestamp);
        let backup_path = self.backup_dir.join(&backup_name);

        std::fs::copy(path, &backup_path)?;
        Ok(backup_path)
    }

    /// Restore a backup file to its original location.
    pub fn restore(&self, backup_path: &Path, original: &Path) -> Result<(), TokenwiseError> {
        std::fs::copy(backup_path, original)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tokenwise_backup_test_{}", name));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// test::backup::creates_timestamped_backup
    #[test]
    fn creates_timestamped_backup() {
        let base = temp_test_dir("create");
        let source_dir = base.join("source");
        let backup_dir = base.join("backups");
        fs::create_dir_all(&source_dir).unwrap();

        let source_file = source_dir.join("settings.json");
        fs::write(&source_file, r#"{"test": "value"}"#).unwrap();

        let manager = BackupManager::new(backup_dir.clone());
        let backup_path = manager.backup(&source_file).unwrap();

        assert!(backup_path.exists(), "Backup file must exist");

        let filename = backup_path.file_name().unwrap().to_str().unwrap();
        assert!(
            filename.starts_with("settings.json."),
            "Backup must start with original filename: {filename}"
        );
        assert!(filename.ends_with(".bak"), "Backup must end with .bak: {filename}");

        // Cleanup
        fs::remove_dir_all(&base).ok();
    }

    /// Backup filename must contain ISO 8601 timestamp with T separator and Z suffix.
    #[test]
    fn backup_filename_contains_iso8601_timestamp() {
        let base = temp_test_dir("timestamp");
        let source_dir = base.join("source");
        let backup_dir = base.join("backups");
        fs::create_dir_all(&source_dir).unwrap();

        let source_file = source_dir.join("settings.json");
        fs::write(&source_file, r#"{"key": 1}"#).unwrap();

        let manager = BackupManager::new(backup_dir);
        let backup_path = manager.backup(&source_file).unwrap();

        let filename = backup_path.file_name().unwrap().to_str().unwrap();
        // Expected: settings.json.2024-01-15T103000Z.bak
        assert!(
            filename.contains('T'),
            "Filename must contain ISO 8601 'T' separator: {filename}"
        );
        assert!(
            filename.ends_with("Z.bak"),
            "Filename must end with 'Z.bak': {filename}"
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn backup_content_matches_original() {
        let base = temp_test_dir("content");
        let source_dir = base.join("source");
        let backup_dir = base.join("backups");
        fs::create_dir_all(&source_dir).unwrap();

        let original_content = r#"{"custom_key": "user_value"}"#;
        let source_file = source_dir.join("settings.json");
        fs::write(&source_file, original_content).unwrap();

        let manager = BackupManager::new(backup_dir);
        let backup_path = manager.backup(&source_file).unwrap();

        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, original_content, "Backup content must match original");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn backup_missing_file_returns_error() {
        let base = temp_test_dir("missing");
        let backup_dir = base.join("backups");

        let manager = BackupManager::new(backup_dir);
        let result = manager.backup(Path::new("/nonexistent/file.json"));
        assert!(result.is_err(), "Backup of non-existent file must return error");

        fs::remove_dir_all(&base).ok();
    }
}
