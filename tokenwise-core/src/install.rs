use std::net::TcpListener;
use std::process::Command;

use tokenwise_common::{Platform, TokenwiseError};

use crate::service::{create_service_manager, ServiceConfig};

// ── OS / distro detection ─────────────────────────────────────────────────────

/// Linux distribution family — used to pick the right package manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxDistro {
    /// Debian, Ubuntu, Mint, Pop!_OS, etc.
    Debian,
    /// Fedora, RHEL, CentOS, AlmaLinux, Rocky, etc.
    Rpm,
    /// Arch, Alpine, Gentoo, or anything we don't recognise.
    Other,
}

/// Probe `/etc/os-release` to classify the Linux distro.
pub fn detect_linux_distro() -> LinuxDistro {
    let content = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let is = |key: &str| content.contains(key);

    if is("ID=ubuntu") || is("ID=debian") || is("ID=linuxmint") || is("ID=pop")
        || is("ID_LIKE=debian") || is("ID_LIKE=ubuntu")
    {
        LinuxDistro::Debian
    } else if is("ID=fedora") || is("ID=rhel") || is("ID=centos") || is("ID=almalinux")
        || is("ID=rocky") || is("ID_LIKE=fedora") || is("ID_LIKE=rhel")
    {
        LinuxDistro::Rpm
    } else {
        LinuxDistro::Other
    }
}

// ── Install outcome ───────────────────────────────────────────────────────────

/// Outcome of installing a single item (prerequisite or component).
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

// ── Prerequisites ─────────────────────────────────────────────────────────────

/// A system prerequisite (Python, Node, Bun, uv…) with per-OS install commands.
#[derive(Debug, Clone)]
pub struct Prerequisite {
    /// Display name.
    pub name: &'static str,
    /// Binary to probe with `which` — empty = always try to install.
    pub probe: &'static str,
    /// macOS install command (via Homebrew when available).
    pub macos_cmd: Option<&'static str>,
    /// Debian/Ubuntu install command.
    pub debian_cmd: Option<&'static str>,
    /// Fedora/RHEL install command.
    pub rpm_cmd: Option<&'static str>,
    /// Windows install command (winget).
    pub windows_cmd: Option<&'static str>,
    /// Universal fallback (curl script, works on all Unix).
    pub universal_cmd: Option<&'static str>,
}

impl Prerequisite {
    /// Return the best install command for the current platform.
    pub fn install_cmd(&self, platform: &Platform, distro: &LinuxDistro) -> Option<&'static str> {
        match platform {
            Platform::MacOS => self.macos_cmd.or(self.universal_cmd),
            Platform::Linux => match distro {
                LinuxDistro::Debian => self.debian_cmd.or(self.universal_cmd),
                LinuxDistro::Rpm => self.rpm_cmd.or(self.universal_cmd),
                LinuxDistro::Other => self.universal_cmd,
            },
            Platform::Windows => self.windows_cmd,
        }
    }
}

/// Prerequisites that must be present before stack components are installed.
///
/// Order matters: Homebrew first (macOS), then runtimes, then package tools.
pub fn default_prerequisites() -> Vec<Prerequisite> {
    vec![
        // ── Homebrew (macOS only) ──────────────────────────────────────────
        Prerequisite {
            name: "Homebrew",
            probe: "brew",
            macos_cmd: Some(r#"/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)""#),
            debian_cmd: None,
            rpm_cmd: None,
            windows_cmd: None,
            universal_cmd: None,
        },
        // ── Python 3 ──────────────────────────────────────────────────────
        Prerequisite {
            name: "Python 3",
            probe: "python3",
            macos_cmd: Some("brew install python3"),
            debian_cmd: Some("sudo apt-get install -y python3 python3-pip"),
            rpm_cmd: Some("sudo dnf install -y python3 python3-pip"),
            windows_cmd: Some("winget install Python.Python.3"),
            universal_cmd: None,
        },
        // ── pipx (Python tool installer — avoids pip --break-system) ──────
        Prerequisite {
            name: "pipx",
            probe: "pipx",
            macos_cmd: Some("brew install pipx && pipx ensurepath"),
            debian_cmd: Some("sudo apt-get install -y pipx && pipx ensurepath"),
            rpm_cmd: Some("pip3 install --user pipx && pipx ensurepath"),
            windows_cmd: Some("pip install --user pipx"),
            universal_cmd: Some("pip3 install --user pipx && pipx ensurepath"),
        },
        // ── Node.js 18+ ───────────────────────────────────────────────────
        Prerequisite {
            name: "Node.js",
            probe: "node",
            macos_cmd: Some("brew install node"),
            debian_cmd: Some("curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash - && sudo apt-get install -y nodejs"),
            rpm_cmd: Some("curl -fsSL https://rpm.nodesource.com/setup_lts.x | sudo bash - && sudo dnf install -y nodejs"),
            windows_cmd: Some("winget install OpenJS.NodeJS.LTS"),
            universal_cmd: None,
        },
        // ── Bun (needed for clawmem) ──────────────────────────────────────
        Prerequisite {
            name: "Bun",
            probe: "bun",
            macos_cmd: Some("brew install oven-sh/bun/bun"),
            debian_cmd: None,
            rpm_cmd: None,
            windows_cmd: Some("powershell -c \"irm bun.sh/install.ps1 | iex\""),
            universal_cmd: Some("curl -fsSL https://bun.sh/install | bash"),
        },
        // ── uv / uvx (needed for Serena MCP) ─────────────────────────────
        Prerequisite {
            name: "uv",
            probe: "uvx",
            macos_cmd: Some("brew install uv"),
            debian_cmd: None,
            rpm_cmd: None,
            windows_cmd: Some("winget install astral-sh.uv"),
            universal_cmd: Some("curl -LsSf https://astral.sh/uv/install.sh | sh"),
        },
    ]
}

// ── Stack components ──────────────────────────────────────────────────────────

/// A tokenwise stack component installed after prerequisites.
#[derive(Debug, Clone)]
pub struct Component {
    pub name: &'static str,
    /// Binary to probe with `which` — skips install if already present.
    pub probe: &'static str,
    /// macOS install command.
    pub macos_cmd: &'static str,
    /// Linux install command (works for all distros — uses pipx/bun/npm).
    pub linux_cmd: &'static str,
    /// Windows install command.
    pub windows_cmd: &'static str,
}

impl Component {
    pub fn install_cmd(&self, platform: &Platform) -> &'static str {
        match platform {
            Platform::MacOS => self.macos_cmd,
            Platform::Linux => self.linux_cmd,
            Platform::Windows => self.windows_cmd,
        }
    }
}

pub fn default_components() -> Vec<Component> {
    vec![
        Component {
            name: "RTK",
            probe: "rtk",
            macos_cmd: "brew install rtk",
            linux_cmd: "curl -fsSL https://raw.githubusercontent.com/reachingforthejack/rtk/main/install.sh | bash",
            windows_cmd: "cargo install rtk",
        },
        Component {
            name: "Headroom proxy",
            probe: "headroom",
            macos_cmd: "pipx install headroom",
            linux_cmd: "pipx install headroom",
            windows_cmd: "pip install headroom",
        },
        Component {
            name: "MarkItDown",
            probe: "markitdown",
            macos_cmd: "pipx install 'markitdown[all]'",
            linux_cmd: "pipx install 'markitdown[all]'",
            windows_cmd: "pip install markitdown[all]",
        },
        Component {
            name: "ClawMem MCP",
            probe: "clawmem",
            macos_cmd: "bun add -g clawmem",
            linux_cmd: "bun add -g clawmem || npm install -g clawmem",
            windows_cmd: "npm install -g clawmem",
        },
        Component {
            name: "codebase-memory-mcp",
            probe: "npx",
            macos_cmd: "npm install -g codebase-memory-mcp",
            linux_cmd: "npm install -g codebase-memory-mcp",
            windows_cmd: "npm install -g codebase-memory-mcp",
        },
        Component {
            name: "Headroom LaunchAgent",
            probe: "headroom",
            macos_cmd: "headroom init",
            linux_cmd: "headroom init",
            windows_cmd: "headroom init",
        },
    ]
}

// ── Installer ─────────────────────────────────────────────────────────────────

/// Configuration for the installer — injectable in tests.
#[derive(Debug, Clone)]
pub struct InstallerConfig {
    pub python_command: String,
    pub node_command: String,
    pub git_command: String,
    pub headroom_port: u16,
    pub skip_service: bool,
    pub components: Vec<Component>,
    pub prerequisites: Vec<Prerequisite>,
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
            prerequisites: default_prerequisites(),
        }
    }
}

/// Installs the full tokenwise optimization stack in three phases:
/// 1. System prerequisites (Python, Node, Bun, uv…)
/// 2. Stack components (RTK, Headroom, MarkItDown, ClawMem…)
/// 3. OS service registration (launchd / systemd)
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
        Self { config: InstallerConfig::default() }
    }

    pub fn with_config(config: InstallerConfig) -> Self {
        Self { config }
    }

    /// Run the full install flow.
    ///
    /// Returns a list of (name, status) tuples — one per item across all phases.
    pub async fn run(&self) -> Result<Vec<(String, ComponentStatus)>, TokenwiseError> {
        let mut summary: Vec<(String, ComponentStatus)> = Vec::new();

        let platform = Platform::current().unwrap_or(Platform::Linux);
        let distro = if matches!(platform, Platform::Linux) {
            detect_linux_distro()
        } else {
            LinuxDistro::Other
        };

        // Phase 1: Prerequisites
        for prereq in &self.config.prerequisites {
            let status = self.install_prerequisite(prereq, &platform, &distro);
            summary.push((prereq.name.to_string(), status));
        }

        // Phase 2: Port check (grace: headroom already installed → skip)
        let headroom_owns_port = which_bin("headroom").is_some()
            && TcpListener::bind(("127.0.0.1", self.config.headroom_port)).is_err();

        if !headroom_owns_port {
            if let Err(e) = self.check_headroom_port() {
                summary.push(("Port 8788".to_string(), ComponentStatus::Failed(e.to_string())));
                return Ok(summary);
            }
        }

        // Phase 3: Stack components
        for component in &self.config.components {
            let cmd = component.install_cmd(&platform);
            let status = run_install_cmd(component.probe, cmd);
            summary.push((component.name.to_string(), status));
        }

        // Phase 4: OS service
        if !self.config.skip_service && !headroom_owns_port {
            let sm = create_service_manager();
            let headroom_path = which_bin("headroom")
                .unwrap_or_else(|| "/usr/local/bin/headroom".to_string());
            let svc_config = ServiceConfig {
                name: "com.tokenwise.headroom".to_string(),
                description: "Tokenwise Headroom token compression proxy".to_string(),
                executable: headroom_path,
                args: vec![
                    "proxy".to_string(),
                    "--port".to_string(),
                    self.config.headroom_port.to_string(),
                ],
            };
            match sm.install(&svc_config).await {
                Ok(_) => summary.push(("Headroom service".to_string(), ComponentStatus::Installed)),
                Err(e) => summary.push((
                    "Headroom service".to_string(),
                    ComponentStatus::Failed(e.to_string()),
                )),
            }
        } else if headroom_owns_port {
            summary.push(("Headroom service".to_string(), ComponentStatus::Skipped));
        }

        Ok(summary)
    }

    /// Install a prerequisite if its probe binary is not in PATH.
    fn install_prerequisite(
        &self,
        prereq: &Prerequisite,
        platform: &Platform,
        distro: &LinuxDistro,
    ) -> ComponentStatus {
        // Already installed?
        if !prereq.probe.is_empty() && which_bin(prereq.probe).is_some() {
            return ComponentStatus::Skipped;
        }
        // No command for this platform? Skip silently.
        let Some(cmd) = prereq.install_cmd(platform, distro) else {
            return ComponentStatus::Skipped;
        };
        run_shell_cmd(cmd)
    }

    pub fn check_headroom_port(&self) -> Result<(), TokenwiseError> {
        match TcpListener::bind(("127.0.0.1", self.config.headroom_port)) {
            Ok(_) => Ok(()),
            Err(_) => Err(TokenwiseError::PortInUse(self.config.headroom_port)),
        }
    }

    /// Print a two-section summary: prerequisites then components.
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
    if name.is_empty() {
        return None;
    }
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Install a component: check probe, then run the install command.
fn run_install_cmd(probe: &str, cmd: &str) -> ComponentStatus {
    if which_bin(probe).is_some() {
        return ComponentStatus::Skipped;
    }
    run_shell_cmd(cmd)
}

fn run_shell_cmd(cmd: &str) -> ComponentStatus {
    let (shell, flag) = shell_and_flag();
    match Command::new(shell).arg(flag).arg(cmd).output() {
        Ok(out) if out.status.success() => ComponentStatus::Installed,
        Ok(out) => ComponentStatus::Failed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ),
        Err(e) => ComponentStatus::Failed(e.to_string()),
    }
}

fn shell_and_flag() -> (&'static str, &'static str) {
    if matches!(Platform::current(), Ok(Platform::Windows)) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    }
}

fn run_version_check(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let s = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

fn meets_min_version(version_str: &str, major: u64, minor: u64) -> bool {
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

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> InstallerConfig {
        InstallerConfig {
            skip_service: true,
            components: vec![],
            prerequisites: vec![],
            ..InstallerConfig::default()
        }
    }

    #[test]
    fn detect_linux_distro_returns_a_variant() {
        // Just verify it doesn't panic on the current OS.
        let _distro = detect_linux_distro();
    }

    #[test]
    fn prerequisite_install_cmd_selects_platform() {
        let prereq = Prerequisite {
            name: "test",
            probe: "test-bin",
            macos_cmd: Some("brew install test"),
            debian_cmd: Some("apt-get install -y test"),
            rpm_cmd: Some("dnf install -y test"),
            windows_cmd: Some("winget install test"),
            universal_cmd: Some("curl test"),
        };
        assert_eq!(
            prereq.install_cmd(&Platform::MacOS, &LinuxDistro::Other),
            Some("brew install test"),
        );
        assert_eq!(
            prereq.install_cmd(&Platform::Linux, &LinuxDistro::Debian),
            Some("apt-get install -y test"),
        );
        assert_eq!(
            prereq.install_cmd(&Platform::Linux, &LinuxDistro::Rpm),
            Some("dnf install -y test"),
        );
    }

    #[test]
    fn prerequisite_falls_back_to_universal() {
        let prereq = Prerequisite {
            name: "bun",
            probe: "bun",
            macos_cmd: None,
            debian_cmd: None,
            rpm_cmd: None,
            windows_cmd: None,
            universal_cmd: Some("curl -fsSL https://bun.sh/install | bash"),
        };
        assert_eq!(
            prereq.install_cmd(&Platform::MacOS, &LinuxDistro::Other),
            Some("curl -fsSL https://bun.sh/install | bash"),
        );
        assert_eq!(
            prereq.install_cmd(&Platform::Linux, &LinuxDistro::Debian),
            Some("curl -fsSL https://bun.sh/install | bash"),
        );
    }

    #[test]
    fn component_install_cmd_selects_platform() {
        let comp = Component {
            name: "RTK",
            probe: "rtk",
            macos_cmd: "brew install rtk",
            linux_cmd: "curl ... | bash",
            windows_cmd: "cargo install rtk",
        };
        assert_eq!(comp.install_cmd(&Platform::MacOS), "brew install rtk");
        assert_eq!(comp.install_cmd(&Platform::Linux), "curl ... | bash");
        assert_eq!(comp.install_cmd(&Platform::Windows), "cargo install rtk");
    }

    #[test]
    fn port_conflict_exits_1() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let config = InstallerConfig { headroom_port: port, ..test_config() };
        let installer = Installer::with_config(config);
        assert!(installer.check_headroom_port().is_err());
        drop(listener);
    }

    #[test]
    fn idempotent_skips_installed_components() {
        let comp = Component {
            name: "sh",
            probe: "sh",
            macos_cmd: "echo ok",
            linux_cmd: "echo ok",
            windows_cmd: "echo ok",
        };
        let status = run_install_cmd(comp.probe, comp.macos_cmd);
        assert_eq!(status, ComponentStatus::Skipped);
    }

    #[test]
    fn install_prints_summary() {
        let summary = vec![
            ("RTK".to_string(), ComponentStatus::Installed),
            ("Headroom proxy".to_string(), ComponentStatus::Skipped),
            ("MarkItDown".to_string(), ComponentStatus::Failed("not found".to_string())),
        ];
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

    #[test]
    fn default_prerequisites_cover_key_tools() {
        let prereqs = default_prerequisites();
        let names: Vec<&str> = prereqs.iter().map(|p| p.name).collect();
        assert!(names.contains(&"Python 3"), "must include Python 3");
        assert!(names.contains(&"Node.js"), "must include Node.js");
        assert!(names.contains(&"Bun"), "must include Bun");
        assert!(names.contains(&"uv"), "must include uv");
    }

    #[test]
    fn default_components_cover_key_stack() {
        let comps = default_components();
        let names: Vec<&str> = comps.iter().map(|c| c.name).collect();
        assert!(names.contains(&"RTK"));
        assert!(names.contains(&"Headroom proxy"));
        assert!(names.contains(&"ClawMem MCP"));
        assert!(names.contains(&"MarkItDown"));
    }

    #[tokio::test]
    async fn service_unit_written_per_platform() {
        let sm = create_service_manager();
        let path = sm.unit_file_path("com.tokenwise.headroom");
        assert!(path.to_str().map(|s| !s.is_empty()).unwrap_or(false));
        assert!(path.is_absolute(), "path must be absolute: {:?}", path);
    }
}
