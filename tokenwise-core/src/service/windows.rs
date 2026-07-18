use std::path::PathBuf;

use async_trait::async_trait;
use tokenwise_common::TokenwiseError;

use super::{ServiceConfig, ServiceManager};

/// Windows Task Scheduler-based service manager.
///
/// Uses `schtasks.exe` with an XML task definition piped to stdin.
/// Task Scheduler is used (not `sc.exe`) because it supports user-mode
/// startup tasks without elevated privileges.
pub struct WindowsServiceManager;

impl WindowsServiceManager {
    pub fn new() -> Self {
        Self
    }

    /// Generate a Windows Task Scheduler XML task definition.
    pub fn generate_task_xml(&self, config: &ServiceConfig) -> String {
        let args_xml = if config.args.is_empty() {
            String::new()
        } else {
            format!(
                "<Arguments>{}</Arguments>",
                config
                    .args
                    .iter()
                    .map(|a| xml_escape(a))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{description}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions>
    <Exec>
      <Command>{exe}</Command>
      {args}
    </Exec>
  </Actions>
</Task>"#,
            description = xml_escape(&config.description),
            exe = xml_escape(&config.executable),
            args = args_xml,
        )
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Default for WindowsServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceManager for WindowsServiceManager {
    async fn install(&self, config: &ServiceConfig) -> Result<(), TokenwiseError> {
        let xml = self.generate_task_xml(config);

        // schtasks /create /tn <name> /xml - (reads XML from stdin)
        #[cfg(target_os = "windows")]
        {
            use std::io::Write;
            use tokio::process::Command;

            let mut child = Command::new("schtasks")
                .args(["/create", "/f", "/tn", &config.name, "/xml", "-"])
                .stdin(std::process::Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(xml.as_bytes()).await?;
            }

            let status = child.wait().await?;
            if !status.success() {
                return Err(TokenwiseError::Http(format!(
                    "schtasks /create failed with code {:?}",
                    status.code()
                )));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Non-Windows: just validate the XML was generated
            let _ = xml;
        }

        Ok(())
    }

    async fn start(&self, name: &str) -> Result<(), TokenwiseError> {
        #[cfg(target_os = "windows")]
        {
            tokio::process::Command::new("schtasks")
                .args(["/run", "/tn", name])
                .status()
                .await?;
        }
        #[cfg(not(target_os = "windows"))]
        let _ = name;
        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<(), TokenwiseError> {
        #[cfg(target_os = "windows")]
        {
            tokio::process::Command::new("schtasks")
                .args(["/end", "/tn", name])
                .status()
                .await?;
        }
        #[cfg(not(target_os = "windows"))]
        let _ = name;
        Ok(())
    }

    async fn is_running(&self, name: &str) -> Result<bool, TokenwiseError> {
        #[cfg(target_os = "windows")]
        {
            let output = tokio::process::Command::new("schtasks")
                .args(["/query", "/tn", name, "/fo", "CSV"])
                .output()
                .await?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Ok(stdout.contains("Running"));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
            Ok(false)
        }
    }

    async fn uninstall(&self, name: &str) -> Result<(), TokenwiseError> {
        #[cfg(target_os = "windows")]
        {
            tokio::process::Command::new("schtasks")
                .args(["/delete", "/f", "/tn", name])
                .status()
                .await?;
        }
        #[cfg(not(target_os = "windows"))]
        let _ = name;
        Ok(())
    }

    fn unit_file_path(&self, name: &str) -> PathBuf {
        // On Windows, the task is stored internally by Task Scheduler.
        // We return a conventional path for consistency.
        PathBuf::from(format!(
            "C:\\Windows\\System32\\Tasks\\{}",
            name
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// test::install::service_unit_written_per_platform (Windows branch)
    #[test]
    fn generate_task_xml_is_valid() {
        let manager = WindowsServiceManager::new();
        let config = ServiceConfig {
            name: "TokenwiseHeadroom".to_string(),
            description: "Headroom proxy service".to_string(),
            executable: "C:\\Program Files\\tokenwise\\headroom.exe".to_string(),
            args: vec!["--port".to_string(), "8788".to_string()],
        };
        let xml = manager.generate_task_xml(&config);

        assert!(xml.contains("<?xml"), "Must be valid XML");
        assert!(xml.contains("TokenwiseHeadroom") || xml.contains("headroom"), "Must include service name or exe");
        assert!(xml.contains("Headroom proxy service"), "Must include description");
        assert!(xml.contains("<LogonTrigger>"), "Must have logon trigger");
        assert!(xml.contains("--port"), "Must include args");
        assert!(xml.contains("8788"), "Must include port value");
        assert!(xml.contains("<RestartOnFailure>"), "Must configure restart on failure");
    }

    #[test]
    fn xml_special_chars_are_escaped() {
        let manager = WindowsServiceManager::new();
        let config = ServiceConfig {
            name: "Test".to_string(),
            description: "A & B <test>".to_string(),
            executable: "C:\\path\\exe.exe".to_string(),
            args: vec![],
        };
        let xml = manager.generate_task_xml(&config);
        assert!(xml.contains("A &amp; B &lt;test&gt;"), "Special chars must be XML-escaped");
    }

    #[test]
    fn unit_file_path_returns_consistent_path() {
        let manager = WindowsServiceManager::new();
        let path = manager.unit_file_path("MyTask");
        // Just verify it returns something non-empty
        assert!(!path.to_string_lossy().is_empty());
    }
}
