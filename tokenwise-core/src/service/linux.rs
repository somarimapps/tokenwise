use std::path::PathBuf;

use async_trait::async_trait;
use tokenwise_common::TokenwiseError;

use super::{ServiceConfig, ServiceManager};

/// Linux systemd user-unit service manager.
///
/// Writes `.service` files to `~/.config/systemd/user/` and controls them
/// via `systemctl --user`.
pub struct LinuxServiceManager {
    systemd_user_dir: PathBuf,
}

impl LinuxServiceManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            systemd_user_dir: home.join(".config").join("systemd").join("user"),
        }
    }

    /// Generate a systemd service unit file for the given configuration.
    pub fn generate_unit(&self, config: &ServiceConfig) -> String {
        let exec_start = if config.args.is_empty() {
            config.executable.clone()
        } else {
            format!("{} {}", config.executable, config.args.join(" "))
        };

        format!(
            r#"[Unit]
Description={description}
After=network.target

[Service]
Type=simple
ExecStart={exec}
Restart=always
RestartSec=10

[Install]
WantedBy=default.target"#,
            description = config.description,
            exec = exec_start,
        )
    }
}

impl Default for LinuxServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceManager for LinuxServiceManager {
    async fn install(&self, config: &ServiceConfig) -> Result<(), TokenwiseError> {
        std::fs::create_dir_all(&self.systemd_user_dir)?;

        let unit_path = self.unit_file_path(&config.name);
        let unit_content = self.generate_unit(config);
        std::fs::write(&unit_path, unit_content)?;

        // Enable and start the user service
        let status = tokio::process::Command::new("systemctl")
            .args(["--user", "enable", "--now"])
            .arg(&config.name)
            .status()
            .await?;

        if !status.success() {
            return Err(TokenwiseError::Http(format!(
                "systemctl enable failed with code {:?}",
                status.code()
            )));
        }
        Ok(())
    }

    async fn start(&self, name: &str) -> Result<(), TokenwiseError> {
        tokio::process::Command::new("systemctl")
            .args(["--user", "start", name])
            .status()
            .await?;
        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<(), TokenwiseError> {
        tokio::process::Command::new("systemctl")
            .args(["--user", "stop", name])
            .status()
            .await?;
        Ok(())
    }

    async fn is_running(&self, name: &str) -> Result<bool, TokenwiseError> {
        let output = tokio::process::Command::new("systemctl")
            .args(["--user", "is-active", name])
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == "active")
    }

    async fn uninstall(&self, name: &str) -> Result<(), TokenwiseError> {
        tokio::process::Command::new("systemctl")
            .args(["--user", "disable", "--now", name])
            .status()
            .await?;

        let unit_path = self.unit_file_path(name);
        if unit_path.exists() {
            std::fs::remove_file(&unit_path)?;
        }

        // Reload daemon so it notices the removed unit
        tokio::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status()
            .await?;

        Ok(())
    }

    fn unit_file_path(&self, name: &str) -> PathBuf {
        self.systemd_user_dir.join(format!("{}.service", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_unit_contains_required_sections() {
        let manager = LinuxServiceManager::new();
        let config = ServiceConfig {
            name: "headroom-proxy".to_string(),
            description: "Headroom token proxy".to_string(),
            executable: "/usr/local/bin/headroom".to_string(),
            args: vec!["--port".to_string(), "8788".to_string()],
        };
        let unit = manager.generate_unit(&config);

        assert!(unit.contains("[Unit]"), "Unit file must have [Unit] section");
        assert!(unit.contains("[Service]"), "Unit file must have [Service] section");
        assert!(unit.contains("[Install]"), "Unit file must have [Install] section");
        assert!(unit.contains("Headroom token proxy"), "Description must appear");
        assert!(unit.contains("/usr/local/bin/headroom"), "Executable must appear");
        assert!(unit.contains("Restart=always"), "Must restart on failure");
        assert!(unit.contains("WantedBy=default.target"), "Must be WantedBy default.target");
    }

    /// test::install::service_unit_written_per_platform (Linux branch)
    #[test]
    fn unit_file_path_is_under_systemd_user() {
        let manager = LinuxServiceManager::new();
        let path = manager.unit_file_path("headroom-proxy");
        assert!(
            path.to_string_lossy().contains("systemd"),
            "Unit path must be under systemd dir: {}",
            path.display()
        );
        assert!(
            path.to_string_lossy().ends_with(".service"),
            "Unit path must end with .service: {}",
            path.display()
        );
    }

    #[test]
    fn args_are_included_in_exec_start() {
        let manager = LinuxServiceManager::new();
        let config = ServiceConfig {
            name: "test".to_string(),
            description: "Test".to_string(),
            executable: "/usr/bin/test".to_string(),
            args: vec!["--arg1".to_string(), "value1".to_string()],
        };
        let unit = manager.generate_unit(&config);
        assert!(unit.contains("--arg1"), "Args must appear in ExecStart");
        assert!(unit.contains("value1"), "Arg values must appear in ExecStart");
    }
}
