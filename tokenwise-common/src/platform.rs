use crate::error::TokenwiseError;

/// Detected runtime operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

impl Platform {
    /// Detect the current platform at runtime via compile-time cfg.
    pub fn current() -> Result<Self, TokenwiseError> {
        #[cfg(target_os = "macos")]
        return Ok(Self::MacOS);

        #[cfg(target_os = "linux")]
        return Ok(Self::Linux);

        #[cfg(target_os = "windows")]
        return Ok(Self::Windows);

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        Err(TokenwiseError::NotFound(
            "Unsupported platform — only macOS, Linux, and Windows are supported".to_string(),
        ))
    }

    /// Directory path (unexpanded) where the OS service unit is written.
    pub fn service_dir(&self) -> &'static str {
        match self {
            Self::MacOS => "~/Library/LaunchAgents",
            Self::Linux => "~/.config/systemd/user",
            Self::Windows => "%APPDATA%\\Microsoft\\Windows\\Task Scheduler",
        }
    }

    /// File extension used for service unit files on this platform.
    pub fn service_file_ext(&self) -> &'static str {
        match self {
            Self::MacOS => ".plist",
            Self::Linux => ".service",
            Self::Windows => ".xml",
        }
    }

    /// Human-readable name of the service manager.
    pub fn service_manager_name(&self) -> &'static str {
        match self {
            Self::MacOS => "launchctl",
            Self::Linux => "systemctl",
            Self::Windows => "schtasks",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MacOS => write!(f, "macOS"),
            Self::Linux => write!(f, "Linux"),
            Self::Windows => write!(f, "Windows"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// test::platform::binary_detection_covers_all_targets
    /// Asserts all 3 OS enum variants exist with distinct service-manager and path mappings.
    #[test]
    fn binary_detection_covers_all_targets() {
        let variants = [Platform::MacOS, Platform::Linux, Platform::Windows];

        // Each variant must have a distinct service directory
        let dirs: Vec<&str> = variants.iter().map(|p| p.service_dir()).collect();
        assert_ne!(dirs[0], dirs[1], "macOS and Linux service dirs must differ");
        assert_ne!(dirs[1], dirs[2], "Linux and Windows service dirs must differ");
        assert_ne!(dirs[0], dirs[2], "macOS and Windows service dirs must differ");

        // Each variant must have a distinct service file extension
        let exts: Vec<&str> = variants.iter().map(|p| p.service_file_ext()).collect();
        assert_ne!(exts[0], exts[1], "macOS and Linux extensions must differ");
        assert_ne!(exts[1], exts[2], "Linux and Windows extensions must differ");
        assert_ne!(exts[0], exts[2], "macOS and Windows extensions must differ");

        // Each variant must have a distinct service manager name
        let managers: Vec<&str> = variants.iter().map(|p| p.service_manager_name()).collect();
        assert_ne!(managers[0], managers[1]);
        assert_ne!(managers[1], managers[2]);
        assert_ne!(managers[0], managers[2]);
    }

    #[test]
    fn current_returns_valid_platform() {
        let platform = Platform::current().expect("Should detect current platform");
        assert!(
            matches!(platform, Platform::MacOS | Platform::Linux | Platform::Windows),
            "Detected platform must be one of the three supported variants"
        );
    }

    #[test]
    fn display_is_human_readable() {
        assert_eq!(format!("{}", Platform::MacOS), "macOS");
        assert_eq!(format!("{}", Platform::Linux), "Linux");
        assert_eq!(format!("{}", Platform::Windows), "Windows");
    }
}
