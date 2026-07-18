# Technical Design: Tokenwise Stack Orchestrator

## Executive Summary

Tokenwise is a single Rust binary that orchestrates the 9-layer token optimization stack for Claude Code and Hermes agent. Built as a Cargo workspace with 4 crates, it provides cross-platform installation, configuration management, health monitoring, and automatic repair capabilities through native OS service abstractions and JSON/YAML configuration writers.

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│              tokenwise CLI (entry)                  │
│         clap commands + orchestration               │
└───────────────┬─────────────────────────────────────┘
                │
      ┌─────────┴──────────┬──────────────┬──────────┐
      ▼                    ▼               ▼          ▼
┌──────────┐        ┌──────────┐   ┌──────────┐ ┌──────────┐
│   core   │        │ adapter- │   │ adapter- │ │  common  │
│          │        │  claude  │   │  hermes  │ │          │
├──────────┤        ├──────────┤   ├──────────┤ ├──────────┤
│• install │        │• settings│   │• config  │ │• models  │
│• doctor  │        │  .json   │   │  writer  │ │• errors  │
│• sync    │        │• MCP reg │   │• skills  │ │• paths   │
│• stats   │        │• hooks   │   │          │ │          │
└──────────┘        └──────────┘   └──────────┘ └──────────┘
      │                    │               │          │
      └────────────────────┴───────────────┴──────────┘
                           │
                    ┌──────┴──────┐
                    │   OS Layer  │
                    ├──────────────┤
                    │• launchctl  │
                    │• systemctl  │
                    │• schtasks   │
                    └──────────────┘
```

## Cargo Workspace Structure

### ADR-001: Five-Crate Architecture
**Decision**: Split into 5 crates: `tokenwise` (CLI), `tokenwise-core` (business logic), `adapter-claude`, `adapter-hermes`, `tokenwise-common`.

**Rationale**: 
- Clear separation of concerns between orchestration logic and agent-specific adapters
- Allows independent evolution of Claude vs Hermes configuration formats
- Common crate prevents duplication of models, errors, and utilities
- CLI remains thin, delegating all logic to core

**Alternatives Rejected**:
- Single monolithic crate: Would mix concerns, harder to test
- Many small crates (one per command): Over-engineering for current scope
- Two crates (CLI + lib): Insufficient separation for adapters

### Crate Responsibilities

#### `tokenwise` (Binary Crate)
```toml
[dependencies]
clap = { version = "4", features = ["derive", "env"] }
tokenwise-core = { path = "../tokenwise-core" }
adapter-claude = { path = "../adapter-claude" }
adapter-hermes = { path = "../adapter-hermes" }
tracing = "0.1"
tracing-subscriber = "0.3"
```
- Entry point (`main.rs`)
- Command parsing via clap
- Logging setup
- Delegates all work to core

#### `tokenwise-core` (Library Crate)
```toml
[dependencies]
tokenwise-common = { path = "../tokenwise-common" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
reqwest = "0.11"
which = "6"
dirs = "5"
tempfile = "3"
```
- Install orchestration
- Health check engine
- Path sync/repair logic
- Stats aggregation
- Service management trait

#### `adapter-claude` (Library Crate)
```toml
[dependencies]
tokenwise-common = { path = "../tokenwise-common" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```
- `settings.json` model and writer
- `.claude.json` MCP registration
- Hook definitions
- Environment variable management

#### `adapter-hermes` (Library Crate)
```toml
[dependencies]
tokenwise-common = { path = "../tokenwise-common" }
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
```
- Hermes config writer
- Skill definition management
- Agent configuration

#### `tokenwise-common` (Library Crate)
```toml
[dependencies]
thiserror = "1"
serde = { version = "1", features = ["derive"] }
```
- Error types
- Path utilities
- Shared models
- Platform detection

## Key Design Decisions

### ADR-002: Non-Destructive Configuration Merge
**Decision**: Always read existing configs, merge tokenwise entries, write back preserving user customizations.

**Rationale**:
- Users may have existing Claude/Hermes configurations we must not destroy
- Allows tokenwise to coexist with manual configurations
- Enables safe uninstall by tracking what we added

**Implementation**:
```rust
pub struct SettingsManager {
    backup_path: PathBuf,
}

impl SettingsManager {
    pub fn update_settings(&self, path: &Path) -> Result<()> {
        // 1. Create backup
        let backup = self.backup_existing(path)?;
        
        // 2. Read existing or create new
        let mut settings = ClaudeSettings::read_or_default(path)?;
        
        // 3. Merge our entries (marked with comments)
        settings.merge_tokenwise_entries()?;
        
        // 4. Write back preserving formatting where possible
        settings.write_pretty(path)?;
        
        Ok(())
    }
}
```

**Alternatives Rejected**:
- Full replacement: Would destroy user customizations
- Append-only: Could create duplicates and conflicts
- Separate config file: Claude/Hermes wouldn't recognize it

### ADR-003: OS Service Abstraction via Trait
**Decision**: Define `ServiceManager` trait with platform-specific implementations.

**Rationale**:
- Single interface for all platforms
- Testable through trait mocking
- Allows fallback strategies per platform

**Implementation**:
```rust
#[async_trait]
pub trait ServiceManager: Send + Sync {
    async fn install_service(&self, config: &ServiceConfig) -> Result<()>;
    async fn start_service(&self, name: &str) -> Result<()>;
    async fn stop_service(&self, name: &str) -> Result<()>;
    async fn uninstall_service(&self, name: &str) -> Result<()>;
    async fn service_status(&self, name: &str) -> Result<ServiceStatus>;
}

pub enum Platform {
    MacOS(MacOSServiceManager),
    Linux(SystemdServiceManager),
    Windows(WindowsServiceManager),
}

impl Platform {
    pub fn detect() -> Result<Self> {
        #[cfg(target_os = "macos")]
        return Ok(Self::MacOS(MacOSServiceManager::new()));
        
        #[cfg(target_os = "linux")]
        return Ok(Self::Linux(SystemdServiceManager::new()));
        
        #[cfg(target_os = "windows")]
        return Ok(Self::Windows(WindowsServiceManager::new()));
    }
}
```

**Alternatives Rejected**:
- Direct platform-specific code: Would scatter conditionals everywhere
- External service manager (systemd-only): Not portable
- Docker containers: Too heavyweight for user machines

### ADR-004: Parallel Health Checks with Severity Levels
**Decision**: Run all health checks in parallel, aggregate results with PASS/WARN/FAIL severity.

**Rationale**:
- Fast feedback even with many components
- Non-blocking checks can detect multiple issues at once
- Clear severity helps prioritize fixes

**Implementation**:
```rust
#[derive(Debug, Clone)]
pub enum CheckSeverity {
    Pass,
    Warn(String),
    Fail(String),
}

#[derive(Debug)]
pub struct HealthCheck {
    pub name: String,
    pub check: Box<dyn Fn() -> BoxFuture<'static, CheckResult> + Send + Sync>,
}

pub struct Doctor {
    checks: Vec<HealthCheck>,
}

impl Doctor {
    pub async fn run_all(&self) -> Vec<CheckResult> {
        let futures: Vec<_> = self.checks.iter()
            .map(|check| (check.check)())
            .collect();
        
        futures::future::join_all(futures).await
    }
    
    pub fn format_results(&self, results: &[CheckResult]) -> String {
        // Group by severity, show FAIL first, then WARN, then PASS
        // Use colored output for terminal
    }
}
```

**Alternatives Rejected**:
- Sequential checks: Too slow with 9+ components
- Binary pass/fail: Insufficient granularity
- External monitoring service: Overkill for local setup

## Configuration Models

### Claude settings.json Structure
```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSettings {
    pub env: HashMap<String, String>,
    pub hooks: Vec<Hook>,
    pub permissions: Permissions,
    pub enabled_plugins: Vec<String>,
    pub extra_known_marketplaces: Vec<String>,
    pub output_style: Option<String>,
    
    // Preserve unknown fields
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct Hook {
    pub event: HookEvent,
    pub command: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>, // tokenwise tracking
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    SessionStart,
    UserPromptSubmit,
    Stop,
    PreCompact,
}
```

### MCP Registration (.claude.json)
```rust
#[derive(Serialize, Deserialize)]
pub struct ClaudeMcpConfig {
    pub mcp_servers: HashMap<String, McpServer>,
}

#[derive(Serialize, Deserialize)]
pub struct McpServer {
    pub command: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}
```

## Command Architectures

### `tokenwise install`
1. Detect platform
2. Check prerequisites (git, python, node)
3. Clone/download each component to `~/.tokenwise/components/`
4. Install Python dependencies in venv
5. Build RTK from source or download binary
6. Install Headroom proxy as service
7. Create initial configs (backup existing)

### `tokenwise doctor`
Parallel checks:
- [ ] RTK binary exists and executable
- [ ] Headroom proxy responding on port 8788
- [ ] MarkItDown MCP server registered
- [ ] ClawMem database accessible
- [ ] Engram MCP responding
- [ ] Caveman plugin loaded
- [ ] Rules files present
- [ ] Hooks configured
- [ ] Service health (running/stopped)

### `tokenwise sync`
1. Parse settings.json hooks
2. For each command path:
   - Check if file exists
   - If not, search PATH and common locations
   - Update path in-place if found
   - Report unfixable paths
3. Verify MCP server commands
4. Update service configurations if paths changed

### `tokenwise stats`
Aggregation from:
```rust
pub struct StatsAggregator;

impl StatsAggregator {
    pub async fn collect(&self) -> Stats {
        let (rtk, headroom, clawmem) = tokio::join!(
            self.rtk_stats(),
            self.headroom_stats(), 
            self.clawmem_status()
        );
        
        Stats {
            total_saved: rtk.saved + headroom.saved,
            by_layer: vec![
                ("RTK", rtk),
                ("Headroom", headroom),
                ("ClawMem", clawmem),
            ],
            session_count: rtk.sessions,
        }
    }
    
    async fn rtk_stats(&self) -> LayerStats {
        // Call: rtk gain --json
    }
    
    async fn headroom_stats(&self) -> LayerStats {
        // HTTP GET http://localhost:8788/stats
    }
}
```

## Platform-Specific Implementations

### macOS (LaunchAgent)
```rust
impl MacOSServiceManager {
    fn generate_plist(&self, config: &ServiceConfig) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" 
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>"#, config.name, config.executable)
    }
}
```

### Linux (systemd)
```rust
impl SystemdServiceManager {
    fn generate_service(&self, config: &ServiceConfig) -> String {
        format!(r#"[Unit]
Description={}
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=always
RestartSec=10

[Install]
WantedBy=default.target"#, config.description, config.executable)
    }
}
```

### Windows (Task Scheduler)
```rust
impl WindowsServiceManager {
    async fn create_scheduled_task(&self, config: &ServiceConfig) -> Result<()> {
        // Use schtasks.exe with XML task definition
        let xml = self.generate_task_xml(config);
        Command::new("schtasks")
            .args(&["/create", "/tn", &config.name, "/xml", "-"])
            .stdin(Stdio::piped())
            .spawn()?
            .stdin.unwrap()
            .write_all(xml.as_bytes())?;
    }
}
```

## Testing Strategy (Strict TDD)

### Unit Tests
- Configuration merge logic (test with fixtures)
- Service manager trait implementations (mock filesystem)
- Path resolution algorithms
- Stats aggregation math

### Integration Tests
```rust
#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    
    #[test]
    fn test_settings_merge_preserves_user_config() {
        let temp = TempDir::new().unwrap();
        let settings_path = temp.path().join("settings.json");
        
        // Write initial user config
        std::fs::write(&settings_path, r#"{"custom": "value"}"#).unwrap();
        
        // Apply tokenwise updates
        let manager = SettingsManager::new();
        manager.update_settings(&settings_path).unwrap();
        
        // Verify both exist
        let result = std::fs::read_to_string(&settings_path).unwrap();
        assert!(result.contains("\"custom\": \"value\""));
        assert!(result.contains("ANTHROPIC_BASE_URL"));
    }
}
```

### End-to-End Tests
- Full install flow in Docker containers
- Service lifecycle (install → start → stop → uninstall)
- Multi-platform CI matrix

## GitHub Actions CI/CD Pipeline

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: tokenwise-linux-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: tokenwise-macos-arm64
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: tokenwise-macos-x64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: tokenwise-windows-x64.exe

    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Build Release
        run: cargo build --release --target ${{ matrix.target }}
      
      - name: Strip symbols (Unix)
        if: runner.os != 'Windows'
        run: strip target/${{ matrix.target }}/release/tokenwise
      
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.artifact }}
          path: target/${{ matrix.target }}/release/tokenwise*
  
  release:
    needs: build
    runs-on: ubuntu-latest
    
    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v3
      
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            tokenwise-*/*
          generate_release_notes: true
```

## Installation Scripts

### macOS/Linux (curl|bash)
```bash
#!/bin/bash
set -e

VERSION=${1:-latest}
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map architecture names
case "$ARCH" in
    x86_64) ARCH="x64" ;;
    aarch64|arm64) ARCH="arm64" ;;
esac

# Construct download URL
BINARY="tokenwise-${OS}-${ARCH}"
URL="https://github.com/somarimapps/tokenwise/releases/download/${VERSION}/${BINARY}"

# Download and install
echo "Downloading tokenwise ${VERSION} for ${OS}-${ARCH}..."
curl -L "$URL" -o /tmp/tokenwise
chmod +x /tmp/tokenwise
sudo mv /tmp/tokenwise /usr/local/bin/tokenwise

echo "Running initial setup..."
tokenwise install

echo "✓ Tokenwise installed successfully"
```

### Windows (PowerShell)
```powershell
param([string]$Version = "latest")

$Arch = if ([System.Environment]::Is64BitOperatingSystem) { "x64" } else { "x86" }
$Binary = "tokenwise-windows-$Arch.exe"
$Url = "https://github.com/somarimapps/tokenwise/releases/download/$Version/$Binary"

Write-Host "Downloading tokenwise $Version for Windows-$Arch..."
Invoke-WebRequest -Uri $Url -OutFile "$env:TEMP\tokenwise.exe"

$InstallPath = "$env:ProgramFiles\tokenwise"
New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
Move-Item -Force "$env:TEMP\tokenwise.exe" "$InstallPath\tokenwise.exe"

# Add to PATH
$Path = [Environment]::GetEnvironmentVariable("Path", "Machine")
if ($Path -notlike "*$InstallPath*") {
    [Environment]::SetEnvironmentVariable("Path", "$Path;$InstallPath", "Machine")
}

Write-Host "Running initial setup..."
& "$InstallPath\tokenwise.exe" install

Write-Host "✓ Tokenwise installed successfully"
```

## Risk Mitigations

### Port Conflict (8788)
```rust
impl HeadroomInstaller {
    async fn find_available_port(&self) -> u16 {
        let mut port = 8788;
        while !self.is_port_available(port).await {
            eprintln!("Port {} in use, trying {}", port, port + 1);
            port += 1;
            if port > 8800 {
                return Err(Error::NoPortAvailable);
            }
        }
        port
    }
}
```

### Backup and Rollback
```rust
pub struct BackupManager {
    backup_dir: PathBuf, // ~/.tokenwise/backups/
}

impl BackupManager {
    pub fn backup(&self, path: &Path) -> Result<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("{}.{}.backup", 
            path.file_name().unwrap().to_str().unwrap(), 
            timestamp);
        let backup_path = self.backup_dir.join(backup_name);
        
        std::fs::copy(path, &backup_path)?;
        Ok(backup_path)
    }
    
    pub fn restore(&self, backup_path: &Path, original: &Path) -> Result<()> {
        std::fs::copy(backup_path, original)?;
        Ok(())
    }
}
```

## Security Considerations

1. **No elevated privileges by default**: Run in user space where possible
2. **Validate downloaded components**: Check SHA256 hashes
3. **Sandboxed Python venv**: Isolate MCP server dependencies
4. **Config backup before modification**: Always preserve user data
5. **Explicit uninstall tracking**: Know what we installed to remove it cleanly

## Summary

Tokenwise architecture emphasizes modularity through a 5-crate workspace, non-destructive configuration management, cross-platform service abstractions, and parallel health monitoring. The design prioritizes safety (backups, rollback), user experience (single binary, automatic repair), and maintainability (clear separation of concerns, comprehensive testing).
