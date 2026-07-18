pub mod macos;
pub mod linux;
pub mod windows;

use async_trait::async_trait;
use tokenwise_common::TokenwiseError;

/// Configuration for an OS-managed service unit.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Unique service name (e.g. `com.tokenwise.headroom`).
    pub name: String,
    /// Human-readable description shown in service listings.
    pub description: String,
    /// Absolute path to the executable to run.
    pub executable: String,
    /// Arguments passed to the executable.
    pub args: Vec<String>,
}

/// Current lifecycle state of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Unknown,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Trait for cross-platform service management.
///
/// Each platform provides a concrete implementation:
/// - macOS: `MacOsServiceManager` (LaunchAgent plist)
/// - Linux: `LinuxServiceManager` (systemd user unit)
/// - Windows: `WindowsServiceManager` (Task Scheduler XML)
#[async_trait]
pub trait ServiceManager: Send + Sync {
    /// Register and enable the service so it starts at login/boot.
    async fn install(&self, config: &ServiceConfig) -> Result<(), TokenwiseError>;

    /// Start a previously installed service.
    async fn start(&self, name: &str) -> Result<(), TokenwiseError>;

    /// Stop a running service.
    async fn stop(&self, name: &str) -> Result<(), TokenwiseError>;

    /// Query whether the service process is currently active.
    async fn is_running(&self, name: &str) -> Result<bool, TokenwiseError>;

    /// Remove the service registration from the OS.
    async fn uninstall(&self, name: &str) -> Result<(), TokenwiseError>;

    /// Return the path on disk where the service unit file is written.
    fn unit_file_path(&self, name: &str) -> std::path::PathBuf;
}

/// Return the platform-native service manager.
pub fn create_service_manager() -> Box<dyn ServiceManager> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOsServiceManager::new());

    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxServiceManager::new());

    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsServiceManager::new());

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    panic!("No service manager available for this platform");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_config_is_cloneable() {
        let cfg = ServiceConfig {
            name: "com.tokenwise.headroom".to_string(),
            description: "Headroom token proxy".to_string(),
            executable: "/usr/local/bin/headroom".to_string(),
            args: vec!["--port".to_string(), "8788".to_string()],
        };
        let cloned = cfg.clone();
        assert_eq!(cfg.name, cloned.name);
    }

    #[test]
    fn service_status_display() {
        assert_eq!(format!("{}", ServiceStatus::Running), "running");
        assert_eq!(format!("{}", ServiceStatus::Stopped), "stopped");
        assert_eq!(format!("{}", ServiceStatus::Unknown), "unknown");
    }
}
