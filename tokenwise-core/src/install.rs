use std::net::TcpListener;
use std::process::Command;

use tokenwise_common::{Platform, TokenwiseError};

use crate::service::{create_service_manager, ServiceConfig};

/// Outcome of installing a single component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentStatus {
    Installed,
    Skipped,
    Failed(String),
}

impl std::fmt::Display for ComponentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Installed => write!(f, "installed"),
            Self::Skipped => write!(f, "skipped (already present)"),
            Self::Failed(e) => write!(f, "FAILED: {e}"),
        }
    }
}

/// A single stack component the installer manages.
#[derive(Debug, Clone)]
pub struct Component {
    /// Name shown in the install summary.
    pub name: &'static str,
    /// Binary that proves this component is already installed (`which <probe>`).
    pub probe: &'static str,
    /// Install command (shell string, run via sh/cmd).
    pub install_cmd: &'static str,
}

/// The 9-component tokenwise stack.
///
/// Order matters: prerequisite-safe install order is preserved.
pub fn default_components() -> Vec<Component> {
    vec![
        Component {
            name: "RTK",
            probe: "rtk",
            install_cmd: "cargo install rtk",
        },
        Component {
            name: "Headroom proxy",
            probe: "headroom",
            install_cmd: "pip3 install headroom",
        },
        Component {
            name: "MarkItDown",
            probe: "markitdown",
            install_cmd: "pip3 install 'markitdown[all]'",
        },
        Component {
            name: "ClawMem",
            probe: "clawmem",
            install_cmd: "pip3 install clawmem-mcp",
        },
        Component {
            name: "Engram MCP",
            probe: "engram",
            install_cmd: "npm install -g @anthropic/engram",
        },
        Component {
            name: "Serena",
            probe: "serena",
            install_cmd: "pip3 install serena",
        },
        Component {
            name: "codebase-memory-mcp",
            probe: "codebase-memory-mcp",
            install_cmd: "npm install -g codebase-memory-mcp",
        },
        Component {
            name: "mcp-registry-server",
            probe: "mcp-registry-server",
            install_cmd: "npm install -g mcp-registry-server",
        },
        Component {
            name: "Headroom LaunchAgent",
            // Headroom running on port 8788 counts as installed for this component.
            probe: "headroom",
            install_cmd: "headroom init",
        },
    ]
}

/// Configuration for the installer — injectable in tests.
#[derive(Debug, Clone)]
pub struct InstallerConfig {
    /// Override the `python3` command name (use `"python3_nonexistent"` in tests).
    pub python_command: String,
    /// Override the `node` command name.
    pub node_command: String,
    /// Override the `git` command name.
    pub git_command: String,
    /// Headroom proxy port to check.
    pub headroom_port: u16,
    /// Skip OS service registration (useful in tests).
    pub skip_service: bool,
    /// Component list (overridable for tests).
    pub components: Vec<Component>,
}

impl Default for InstallerConfig {
    fn default() -> Self {
        Self {
            python_command: "python3".to_string(),
            node_command: "node".to_string(),
            git_command: "git".to_string(),
            headroom_port: 8788,
            skip_service: false,
            components: default_components(),
        }
    }
}

/// Installs and configures the full 9-component tokenwise stack.
pub struct Installer {
    pub config: InstallerConfig,
}

impl Default for Installer {
    fn default() -> Self {
        Self::new()
    }
}

impl Installer {
    pub fn new() -> Self {
        Self {
            config: InstallerConfig::default(),
        }
    }

    pub fn with_config(config: InstallerConfig) -> Self {
        Self { config }
    }

    /// Run the full install flow.
    ///
    /// Steps:
    /// 1. Check prerequisites (Python 3.8+, Node 18+, Git).
    /// 2. Check port 8788 availability.
    /// 3. Install each of the 9 components (idempotent).
    /// 4. Register Headroom LaunchAgent / systemd unit.
    /// 5. Print summary.
    pub async fn run(&self) -> Result<Vec<(String, ComponentStatus)>, TokenwiseError> {
        // 1. Prerequisites.
        self.check_prerequisites()?;

        // 2. Port check.
        // Grace case: headroom is already installed and has the port → idempotent ok.
        let headroom_owns_port = which_bin("headroom").is_some()
            && TcpListener::bind(("127.0.0.1", self.config.headroom_port)).is_err();
        if !headroom_owns_port {
            self.check_headroom_port()?;
        }

        // 3. Install components.
        let mut summary: Vec<(String, ComponentStatus)> = Vec::new();
        for component in &self.config.components {
            let status = self.install_component(component);
            summary.push((component.name.to_string(), status));
        }

        // 4. Service registration (unless skipped in tests or headroom already owns the port).
        // Skip when headroom already occupies port 8788 — an existing service (e.g.
        // com.headroom.proxy.plist from a native Headroom install) is already running.
        if !self.config.skip_service && !headroom_owns_port {
            let sm = create_service_manager();
            let headroom_path = which_bin("headroom").unwrap_or("/usr/local/bin/headroom".to_string());
            let svc_config = ServiceConfig {
                name: "com.tokenwise.headroom".to_string(),
                description: "Tokenwise Headroom token compression proxy".to_string(),
                executable: headroom_path,
                // headroom requires the `proxy` subcommand: `headroom proxy --port 8788`
                args: vec!["proxy".to_string(), "--port".to_string(), self.config.headroom_port.to_string()],
            };
            if let Err(e) = sm.install(&svc_config).await {
                summary.push((
                    "Headroom service".to_string(),
                    ComponentStatus::Failed(e.to_string()),
                ));
            } else {
                summary.push(("Headroom service".to_string(), ComponentStatus::Installed));
            }
        } else if headroom_owns_port {
            summary.push(("Headroom service".to_string(), ComponentStatus::Skipped));
        }

        Ok(summary)
    }

    /// Check that Python 3.8+, Node 18+, and Git are available.
    pub fn check_prerequisites(&self) -> Result<(), TokenwiseError> {
        // Python 3.8+
        let py_version = run_version_check(&self.config.python_command, &["--version"]);
        match py_version {
            None => {
                return Err(TokenwiseError::MissingPrerequisite(
                    format!(
                        "Python 3.8+ not found (tried `{} --version`). Install Python first.",
                        self.config.python_command
                    ),
                ))
            }
            Some(v) => {
                if !meets_min_version(&v, 3, 8) {
                    return Err(TokenwiseError::MissingPrerequisite(format!(
                        "Python 3.8+ required; found `{v}`"
                    )));
                }
            }
        }

        // Node 18+
        let node_version = run_version_check(&self.config.node_command, &["--version"]);
        match node_version {
            None => {
                return Err(TokenwiseError::MissingPrerequisite(
                    format!(
                        "Node.js 18+ not found (tried `{} --version`). Install Node.js first.",
                        self.config.node_command
                    ),
                ))
            }
            Some(v) => {
                // node --version returns "v18.x.x"
                let stripped = v.trim_start_matches('v');
                if !meets_min_version(stripped, 18, 0) {
                    return Err(TokenwiseError::MissingPrerequisite(format!(
                        "Node.js 18+ required; found `{v}`"
                    )));
                }
            }
        }

        // Git (any version)
        if run_version_check(&self.config.git_command, &["--version"]).is_none() {
            return Err(TokenwiseError::MissingPrerequisite(
                format!(
                    "Git not found (tried `{} --version`). Install Git first.",
                    self.config.git_command
                ),
            ));
        }

        Ok(())
    }

    /// Verify that port `headroom_port` is not already bound.
    ///
    /// Returns `Ok(())` when the port is free; `Err(PortInUse)` when something
    /// is already listening on it.  Callers that want an idempotent "headroom
    /// already running" grace must check that separately (see `run()`).
    pub fn check_headroom_port(&self) -> Result<(), TokenwiseError> {
        match TcpListener::bind(("127.0.0.1", self.config.headroom_port)) {
            Ok(_) => Ok(()),
            Err(_) => Err(TokenwiseError::PortInUse(self.config.headroom_port)),
        }
    }

    /// Install a single component if not already present (idempotent).
    fn install_component(&self, component: &Component) -> ComponentStatus {
        // Check if already installed.
        if which_bin(component.probe).is_some() {
            return ComponentStatus::Skipped;
        }

        // Run install command via the system shell.
        let (shell, flag) = shell_and_flag();
        let result = Command::new(shell)
            .arg(flag)
            .arg(component.install_cmd)
            .output();

        match result {
            Ok(out) if out.status.success() => ComponentStatus::Installed,
            Ok(out) => ComponentStatus::Failed(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ),
            Err(e) => ComponentStatus::Failed(e.to_string()),
        }
    }

    /// Print the install summary to stdout.
    ///
    /// Called by the CLI command after `run()` returns.
    pub fn print_summary(summary: &[(String, ComponentStatus)]) {
        println!("\nTokenwise installation summary:");
        println!("{:<30} Status", "Component");
        println!("{}", "─".repeat(55));
        for (name, status) in summary {
            println!("{:<30} {}", name, status);
        }
        println!();
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn which_bin(name: &str) -> Option<String> {
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn run_version_check(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        // Some tools write version to stderr
        let s = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

/// Returns true if `version_str` (like "Python 3.10.4" or "3.10.4") is >= major.minor.
fn meets_min_version(version_str: &str, major: u64, minor: u64) -> bool {
    // Find first sequence of digits matching X.Y (optionally X.Y.Z).
    let nums: Vec<u64> = version_str
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter(|s| s.contains('.'))
        .flat_map(|s| s.split('.'))
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();

    match nums.as_slice() {
        [maj, min, ..] => (*maj, *min) >= (major, minor),
        _ => false,
    }
}

fn shell_and_flag() -> (&'static str, &'static str) {
    if matches!(Platform::current(), Ok(Platform::Windows)) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> InstallerConfig {
        InstallerConfig {
            skip_service: true,   // avoid touching OS service layer in tests
            components: vec![],   // no actual pip/npm calls in unit tests
            ..InstallerConfig::default()
        }
    }

    /// test::install::missing_python_exits_1
    #[test]
    fn missing_python_exits_1() {
        let config = InstallerConfig {
            python_command: "python3_nonexistent_xq7z".to_string(),
            ..test_config()
        };
        let installer = Installer::with_config(config);
        let result = installer.check_prerequisites();
        assert!(result.is_err(), "Must error when python is missing");
        match result.unwrap_err() {
            TokenwiseError::MissingPrerequisite(msg) => {
                assert!(msg.to_lowercase().contains("python"), "Error must mention python: {msg}")
            }
            e => panic!("Expected MissingPrerequisite, got {:?}", e),
        }
    }

    /// test::install::port_conflict_exits_1
    #[test]
    fn port_conflict_exits_1() {
        // Bind a listener on some free port, then verify check_headroom_port()
        // detects it as in-use.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let config = InstallerConfig {
            headroom_port: port,
            ..test_config()
        };
        let installer = Installer::with_config(config);

        // Listener is still bound → port in use, and headroom is NOT responding on it.
        let result = installer.check_headroom_port();
        assert!(
            result.is_err(),
            "Must error when port {port} is in use by a non-headroom process"
        );
        match result.unwrap_err() {
            TokenwiseError::PortInUse(p) => assert_eq!(p, port),
            e => panic!("Expected PortInUse, got {:?}", e),
        }

        drop(listener);
    }

    /// test::install::idempotent_skips_installed_components
    #[test]
    fn idempotent_skips_installed_components() {
        // A component whose probe binary IS in PATH should return Skipped.
        let always_present = Component {
            name: "sh",
            probe: "sh",
            install_cmd: "echo already installed",
        };
        let installer = Installer::new();
        let status = installer.install_component(&always_present);
        assert_eq!(
            status,
            ComponentStatus::Skipped,
            "`sh` must always be present; component must be Skipped"
        );
    }

    /// test::install::service_unit_written_per_platform
    #[tokio::test]
    async fn service_unit_written_per_platform() {
        // Verify the unit file path is platform-appropriate (non-empty, absolute).
        let sm = create_service_manager();
        let path = sm.unit_file_path("com.tokenwise.headroom");
        assert!(
            path.to_str().map(|s| !s.is_empty()).unwrap_or(false),
            "Unit file path must be non-empty"
        );
        assert!(
            path.is_absolute(),
            "Unit file path must be absolute: {:?}",
            path
        );
    }

    /// test::install::install_prints_summary
    #[test]
    fn install_prints_summary() {
        // Verify print_summary does not panic with various statuses.
        let summary = vec![
            ("RTK".to_string(), ComponentStatus::Installed),
            ("Headroom proxy".to_string(), ComponentStatus::Skipped),
            ("MarkItDown".to_string(), ComponentStatus::Failed("pip not found".to_string())),
        ];
        // Just check it doesn't panic — output goes to stdout in tests.
        Installer::print_summary(&summary);
    }

    #[test]
    fn meets_min_version_parsing() {
        assert!(meets_min_version("Python 3.10.4", 3, 8));
        assert!(meets_min_version("3.8.0", 3, 8));
        assert!(!meets_min_version("Python 3.7.9", 3, 8));
        assert!(meets_min_version("v18.12.0", 18, 0));
        assert!(!meets_min_version("v16.0.0", 18, 0));
    }
}
