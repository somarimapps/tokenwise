use std::path::PathBuf;

use async_trait::async_trait;
use tokenwise_common::TokenwiseError;

use super::{ServiceConfig, ServiceManager};

/// macOS LaunchAgent-based service manager.
///
/// Writes plist files to `~/Library/LaunchAgents/` and controls them via `launchctl`.
pub struct MacOsServiceManager {
    launch_agents_dir: PathBuf,
}

impl MacOsServiceManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            launch_agents_dir: home.join("Library").join("LaunchAgents"),
        }
    }

    /// Generate a macOS plist XML for the given service configuration.
    pub fn generate_plist(&self, config: &ServiceConfig) -> String {
        let args_xml = config
            .args
            .iter()
            .map(|a| format!("        <string>{}</string>", a))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{name}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
{args}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/{name}.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/{name}.stderr.log</string>
</dict>
</plist>"#,
            name = config.name,
            exe = config.executable,
            args = args_xml,
        )
    }
}

impl Default for MacOsServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceManager for MacOsServiceManager {
    async fn install(&self, config: &ServiceConfig) -> Result<(), TokenwiseError> {
        std::fs::create_dir_all(&self.launch_agents_dir)?;

        let plist_path = self.unit_file_path(&config.name);
        let plist_content = self.generate_plist(config);
        std::fs::write(&plist_path, plist_content)?;

        // Load the plist into launchd
        let status = tokio::process::Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&plist_path)
            .status()
            .await?;

        if !status.success() {
            return Err(TokenwiseError::Http(format!(
                "launchctl load failed with code {:?}",
                status.code()
            )));
        }
        Ok(())
    }

    async fn start(&self, name: &str) -> Result<(), TokenwiseError> {
        tokio::process::Command::new("launchctl")
            .args(["start", name])
            .status()
            .await?;
        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<(), TokenwiseError> {
        tokio::process::Command::new("launchctl")
            .args(["stop", name])
            .status()
            .await?;
        Ok(())
    }

    async fn is_running(&self, name: &str) -> Result<bool, TokenwiseError> {
        let output = tokio::process::Command::new("launchctl")
            .args(["list", name])
            .output()
            .await?;
        Ok(output.status.success())
    }

    async fn uninstall(&self, name: &str) -> Result<(), TokenwiseError> {
        let plist_path = self.unit_file_path(name);
        if plist_path.exists() {
            tokio::process::Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&plist_path)
                .status()
                .await?;
            std::fs::remove_file(&plist_path)?;
        }
        Ok(())
    }

    fn unit_file_path(&self, name: &str) -> PathBuf {
        self.launch_agents_dir.join(format!("{}.plist", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_plist_contains_required_keys() {
        let manager = MacOsServiceManager::new();
        let config = ServiceConfig {
            name: "com.tokenwise.headroom".to_string(),
            description: "Headroom proxy".to_string(),
            executable: "/usr/local/bin/headroom".to_string(),
            args: vec!["--port".to_string(), "8788".to_string()],
        };
        let plist = manager.generate_plist(&config);

        assert!(plist.contains("com.tokenwise.headroom"), "Plist must include service label");
        assert!(plist.contains("/usr/local/bin/headroom"), "Plist must include executable");
        assert!(plist.contains("<key>RunAtLoad</key>"), "Plist must have RunAtLoad key");
        assert!(plist.contains("<true/>"), "RunAtLoad must be true");
        assert!(plist.contains("<key>KeepAlive</key>"), "Plist must have KeepAlive key");
        assert!(plist.contains("--port"), "Plist must include args");
        assert!(plist.contains("8788"), "Plist must include port arg value");
    }

    /// test::install::service_unit_written_per_platform (macOS branch)
    #[test]
    fn unit_file_path_is_under_launch_agents() {
        let manager = MacOsServiceManager::new();
        let path = manager.unit_file_path("com.tokenwise.headroom");
        assert!(
            path.to_string_lossy().contains("LaunchAgents"),
            "Plist must be under LaunchAgents: {}",
            path.display()
        );
        assert!(
            path.to_string_lossy().ends_with(".plist"),
            "Plist path must end with .plist: {}",
            path.display()
        );
    }

    #[test]
    fn plist_is_valid_xml() {
        let manager = MacOsServiceManager::new();
        let config = ServiceConfig {
            name: "test.service".to_string(),
            description: "Test".to_string(),
            executable: "/usr/bin/test".to_string(),
            args: vec![],
        };
        let plist = manager.generate_plist(&config);
        assert!(plist.starts_with("<?xml"), "Plist must be valid XML");
        assert!(plist.contains("</plist>"), "Plist must close plist tag");
    }
}
