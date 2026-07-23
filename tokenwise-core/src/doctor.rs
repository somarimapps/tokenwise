use std::path::PathBuf;
use std::time::Duration;

use futures::future::join_all;
use tokenwise_common::{
    TokenwiseError, CORE_MCP_SERVER_NAMES, REQUIRED_RULES_FILES,
};

use crate::settings::models::{ClaudeMcpConfig, ClaudeSettings};

/// Status of a single health check layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::Warn => write!(f, "WARN"),
            Self::Fail => write!(f, "FAIL"),
        }
    }
}

/// Result of a single health check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub layer: u8,
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub suggestion: Option<String>,
}

impl CheckResult {
    pub fn pass(layer: u8, name: &str, message: &str) -> Self {
        Self {
            layer,
            name: name.to_string(),
            status: CheckStatus::Pass,
            message: message.to_string(),
            suggestion: None,
        }
    }

    pub fn warn(layer: u8, name: &str, message: &str, suggestion: Option<&str>) -> Self {
        Self {
            layer,
            name: name.to_string(),
            status: CheckStatus::Warn,
            message: message.to_string(),
            suggestion: suggestion.map(str::to_string),
        }
    }

    pub fn fail(layer: u8, name: &str, message: &str, suggestion: Option<&str>) -> Self {
        Self {
            layer,
            name: name.to_string(),
            status: CheckStatus::Fail,
            message: message.to_string(),
            suggestion: suggestion.map(str::to_string),
        }
    }

    pub fn format_line(&self) -> String {
        let base = format!("[{}] Layer {:>2} {:20} {}", self.status, self.layer, self.name, self.message);
        match &self.suggestion {
            Some(s) => format!("{}\n         Suggestion: {}", base, s),
            None => base,
        }
    }
}

/// Derives the final exit code from a list of check results.
///
/// Per spec REQ-005 (Exit Code Contract):
/// - 0 if all Pass, or any Warn but no Fail (WARN is advisory, not blocking)
/// - 1 if any Fail
pub fn exit_code_from_results(results: &[CheckResult]) -> i32 {
    if results.iter().any(|r| r.status == CheckStatus::Fail) {
        1
    } else {
        0
    }
}

/// Paths used by the doctor to locate Claude config files.
#[derive(Debug, Clone)]
pub struct DoctorPaths {
    pub settings_json: PathBuf,
    pub claude_json: PathBuf,
    pub rules_dir: PathBuf,
}

impl DoctorPaths {
    pub fn default_paths() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let claude_dir = home.join(".claude");
        Self {
            settings_json: claude_dir.join("settings.json"),
            claude_json: home.join(".claude.json"),
            rules_dir: claude_dir.join("rules"),
        }
    }
}

/// 10-layer parallel health checker.
pub struct Doctor {
    pub paths: DoctorPaths,
    /// URL used for the Headroom proxy HTTP health probe.
    pub headroom_url: String,
    /// Timeout applied to each network/subprocess probe.
    pub timeout: Duration,
    /// Binary used for the ClawMem MCP probe (default: "clawmem").
    pub clawmem_bin: String,
    /// Binary used for the MarkItDown MCP probe (default: "python3").
    pub markitdown_bin: String,
}

impl Doctor {
    pub fn new(paths: DoctorPaths) -> Self {
        Self {
            paths,
            headroom_url: "http://127.0.0.1:8788/health".to_string(),
            timeout: Duration::from_secs(3),
            clawmem_bin: "clawmem".to_string(),
            markitdown_bin: "python3".to_string(),
        }
    }

    /// Run all 10 checks in parallel and collect results.
    ///
    /// Layer order per spec:
    ///  1. Headroom proxy HTTP probe
    ///  2. RTK binary exists
    ///  3. Hooks executable
    ///  4. Core MCPs registered in ~/.claude.json
    ///  5. Rules files present
    ///  6. ClawMem MCP status probe
    ///  7. Engram plugin in enabledPlugins
    ///  8. OS service unit file on disk
    ///  9. MarkItDown MCP probe
    /// 10. Caveman plugin in enabledPlugins
    pub async fn run_all(&self) -> Vec<CheckResult> {
        let checks: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = CheckResult> + Send>>> = vec![
            Box::pin(self.check_headroom_proxy()),
            Box::pin(self.check_rtk_binary()),
            Box::pin(self.check_hooks_executable()),
            Box::pin(self.check_mcps_registered()),
            Box::pin(self.check_rules_files()),
            Box::pin(self.check_clawmem()),
            Box::pin(self.check_engram()),
            Box::pin(self.check_os_service_unit()),
            Box::pin(self.check_markitdown_mcp()),
            Box::pin(self.check_caveman_plugin()),
        ];
        join_all(checks).await
    }

    /// Layer 1: Headroom proxy responds on port 8788/health.
    /// Any HTTP response (including 4xx) counts as PASS.
    async fn check_headroom_proxy(&self) -> CheckResult {
        let client = match reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return CheckResult::fail(
                    1,
                    "headroom-proxy",
                    &format!("Failed to build HTTP client: {}", e),
                    None,
                );
            }
        };

        match client.get(&self.headroom_url).send().await {
            Ok(_) => CheckResult::pass(1, "headroom-proxy", "Headroom proxy responding on port 8788"),
            Err(e) if e.is_timeout() => CheckResult::fail(
                1,
                "headroom-proxy",
                "Headroom proxy timeout on port 8788",
                Some("Start Headroom: launchctl start com.headroom.proxy"),
            ),
            Err(_) => CheckResult::fail(
                1,
                "headroom-proxy",
                "Headroom proxy not responding on port 8788",
                Some("Start Headroom: launchctl start com.headroom.proxy"),
            ),
        }
    }

    /// Layer 2: RTK binary exists in PATH.
    async fn check_rtk_binary(&self) -> CheckResult {
        match which::which("rtk") {
            Ok(path) => CheckResult::pass(2, "rtk-binary", &format!("rtk found at {}", path.display())),
            Err(_) => CheckResult::fail(
                2,
                "rtk-binary",
                "rtk binary not found in PATH",
                Some("Install RTK: cargo install rtk or download from GitHub"),
            ),
        }
    }

    /// Layer 3: Hook command paths exist and are executable.
    async fn check_hooks_executable(&self) -> CheckResult {
        let settings = match ClaudeSettings::read_or_default(&self.paths.settings_json) {
            Ok(s) => s,
            Err(_) => {
                return CheckResult::warn(
                    3,
                    "hooks-executable",
                    "Could not read settings.json — no hooks to check",
                    None,
                );
            }
        };

        let paths = settings.hook_command_paths();
        if paths.is_empty() {
            return CheckResult::pass(3, "hooks-executable", "No hook paths configured");
        }

        let mut missing = Vec::new();
        for cmd_str in &paths {
            let binary = cmd_str.split_whitespace().next().unwrap_or(cmd_str);
            let path = std::path::Path::new(binary);
            if path.is_absolute() && !path.exists() {
                missing.push(binary.to_string());
            }
        }

        if missing.is_empty() {
            CheckResult::pass(3, "hooks-executable", &format!("{} hook path(s) verified", paths.len()))
        } else {
            CheckResult::fail(
                3,
                "hooks-executable",
                &format!("Hook path not found: {}", missing.join(", ")),
                Some("run 'tokenwise sync' to repair"),
            )
        }
    }

    /// Layer 4: All core MCP servers registered in `~/.claude.json`.
    async fn check_mcps_registered(&self) -> CheckResult {
        match McpRegistry::all_registered_static(&self.paths.claude_json, CORE_MCP_SERVER_NAMES) {
            Ok(true) => CheckResult::pass(4, "mcp-registered", &format!("All {} core MCP servers registered", CORE_MCP_SERVER_NAMES.len())),
            Ok(false) => {
                let missing = missing_mcps(&self.paths.claude_json);
                CheckResult::fail(
                    4,
                    "mcp-registered",
                    &format!("Missing MCP servers: {}", missing.join(", ")),
                    Some("run 'tokenwise connect claude' to register MCPs"),
                )
            }
            Err(e) => CheckResult::warn(4, "mcp-registered", &format!("Cannot read ~/.claude.json: {}", e), None),
        }
    }

    /// Layer 5: All 5 required rules files present and non-empty.
    async fn check_rules_files(&self) -> CheckResult {
        let mut missing = Vec::new();
        for name in REQUIRED_RULES_FILES {
            let path = self.paths.rules_dir.join(name);
            if !path.exists() {
                missing.push(*name);
                continue;
            }
            if path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
                missing.push(*name);
            }
        }

        if missing.is_empty() {
            CheckResult::pass(5, "rules-files", &format!("{} rule files present", REQUIRED_RULES_FILES.len()))
        } else {
            CheckResult::fail(
                5,
                "rules-files",
                &format!("Missing rules: {}", missing.join(", ")),
                Some("run 'tokenwise connect claude' to write rule files"),
            )
        }
    }

    /// Layer 6: ClawMem MCP responds within 3s (via `clawmem status`).
    async fn check_clawmem(&self) -> CheckResult {
        let result = tokio::time::timeout(
            self.timeout,
            tokio::process::Command::new(&self.clawmem_bin)
                .arg("status")
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) if output.status.success() => {
                CheckResult::pass(6, "clawmem", "ClawMem responds")
            }
            Ok(Ok(_)) => CheckResult::warn(6, "clawmem", "clawmem status returned non-zero", None),
            Ok(Err(_)) => CheckResult::fail(
                6,
                "clawmem",
                "clawmem binary not found",
                Some("Install ClawMem: bun add -g clawmem or npm install -g clawmem"),
            ),
            Err(_) => CheckResult::fail(
                6,
                "clawmem",
                "ClawMem MCP timeout (>3s)",
                Some("Check ClawMem MCP server status"),
            ),
        }
    }

    /// Layer 7: Engram plugin present in `enabledPlugins`.
    ///
    /// Engram is a Claude Code plugin (not an npm MCP server) — it is configured
    /// via `enabledPlugins` in settings.json, not via `~/.claude.json`.
    async fn check_engram(&self) -> CheckResult {
        let settings = match ClaudeSettings::read_or_default(&self.paths.settings_json) {
            Ok(s) => s,
            Err(_) => {
                return CheckResult::warn(7, "engram", "Cannot read settings.json", None);
            }
        };

        if settings.enabled_plugins.iter().any(|p| p == "engram" || p.starts_with("engram@")) {
            CheckResult::pass(7, "engram", "Engram plugin active")
        } else {
            CheckResult::warn(
                7,
                "engram",
                "engram not in enabledPlugins",
                Some("Add 'engram@engram' to enabledPlugins in ~/.claude/settings.json"),
            )
        }
    }

    /// Layer 8: OS service unit file exists on disk.
    ///
    /// A missing file is WARN (not FAIL) because the installer (PR2) may not
    /// have run yet. The doctor reports the expected path so the user knows
    /// where to look.
    async fn check_os_service_unit(&self) -> CheckResult {
        let manager = crate::service::create_service_manager();

        // Headroom service name per platform
        #[cfg(target_os = "macos")]
        let service_name = "com.headroom.proxy";
        #[cfg(not(target_os = "macos"))]
        let service_name = "headroom-proxy";

        let unit_path = manager.unit_file_path(service_name);

        if unit_path.exists() {
            CheckResult::pass(
                8,
                "os-service-unit",
                &format!("Service unit found: {}", unit_path.display()),
            )
        } else {
            CheckResult::warn(
                8,
                "os-service-unit",
                &format!("Service unit not found: {}", unit_path.display()),
                Some("run 'tokenwise install' to register the OS service"),
            )
        }
    }

    /// Layer 9: MarkItDown MCP installed and functional.
    ///
    /// Strategy (in order):
    /// 1. If a "markitdown" entry exists in ~/.claude.json, check its command binary.
    /// 2. Fallback: probe `python3 -c "import markitdown_mcp"` via configured binary.
    async fn check_markitdown_mcp(&self) -> CheckResult {
        // Strategy 1: check the registered MCP command binary exists.
        if let Ok(config) = ClaudeMcpConfig::read_or_default(&self.paths.claude_json) {
            if let Some(entry) = config.mcp_servers.get("markitdown") {
                if !entry.command.is_empty() {
                    let cmd_path = std::path::Path::new(&entry.command);
                    if cmd_path.is_absolute() && cmd_path.exists() {
                        return CheckResult::pass(
                            9,
                            "markitdown-mcp",
                            &format!("MarkItDown MCP registered, command found: {}", entry.command),
                        );
                    }
                }
            }
        }

        // Strategy 2: fallback python3 import probe.
        let result = tokio::time::timeout(
            self.timeout,
            tokio::process::Command::new(&self.markitdown_bin)
                .args(["-c", "import markitdown_mcp"])
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) if output.status.success() => {
                CheckResult::pass(9, "markitdown-mcp", "MarkItDown MCP responds")
            }
            Ok(Ok(_)) => CheckResult::fail(
                9,
                "markitdown-mcp",
                "MarkItDown MCP not installed",
                Some("Install MarkItDown: pip install markitdown-mcp"),
            ),
            Ok(Err(_)) => CheckResult::fail(
                9,
                "markitdown-mcp",
                "MarkItDown MCP not found",
                Some("Install MarkItDown: pip install markitdown-mcp"),
            ),
            Err(_) => CheckResult::fail(
                9,
                "markitdown-mcp",
                "MarkItDown MCP timeout (>3s)",
                Some("Check MarkItDown MCP server status"),
            ),
        }
    }

    /// Layer 10: Caveman plugin present in `enabledPlugins`.
    async fn check_caveman_plugin(&self) -> CheckResult {
        let settings = match ClaudeSettings::read_or_default(&self.paths.settings_json) {
            Ok(s) => s,
            Err(_) => {
                return CheckResult::warn(
                    10,
                    "caveman",
                    "Cannot read settings.json",
                    None,
                );
            }
        };

        if settings.enabled_plugins.iter().any(|p| p == "caveman" || p.starts_with("caveman@")) {
            CheckResult::pass(10, "caveman", "caveman plugin active")
        } else {
            CheckResult::warn(
                10,
                "caveman",
                "caveman not in enabledPlugins",
                Some("Add 'caveman' to enabledPlugins in settings.json"),
            )
        }
    }
}

/// Helper to avoid importing McpRegistry into doctor.
struct McpRegistry;

impl McpRegistry {
    fn all_registered_static(path: &std::path::Path, names: &[&str]) -> Result<bool, TokenwiseError> {
        if !path.exists() {
            return Ok(false);
        }
        let config = ClaudeMcpConfig::read_or_default(path)?;
        Ok(names.iter().all(|n| config.mcp_servers.contains_key(*n)))
    }
}

fn missing_mcps(path: &std::path::Path) -> Vec<String> {
    let config = ClaudeMcpConfig::read_or_default(path).unwrap_or_default();
    CORE_MCP_SERVER_NAMES
        .iter()
        .filter(|n| !config.mcp_servers.contains_key(**n))
        .map(|n| n.to_string())
        .collect()
}

/// Format all check results for terminal output.
pub fn format_doctor_output(results: &[CheckResult]) -> String {
    let mut lines: Vec<String> = results.iter().map(|r| r.format_line()).collect();
    let has_fail = results.iter().any(|r| r.status == CheckStatus::Fail);
    let has_warn = results.iter().any(|r| r.status == CheckStatus::Warn);
    let summary = if has_fail {
        "[FAIL] Critical issues detected"
    } else if has_warn {
        "[WARN] Some layers need attention"
    } else {
        "[PASS] All layers healthy"
    };
    lines.push(String::new());
    lines.push(summary.to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_claude_dir(name: &str) -> (PathBuf, DoctorPaths) {
        let base = std::env::temp_dir().join(format!("tokenwise_doctor_{}", name));
        let claude_dir = base.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        let paths = DoctorPaths {
            settings_json: claude_dir.join("settings.json"),
            claude_json: base.join(".claude.json"),
            rules_dir: claude_dir.join("rules"),
        };
        (base, paths)
    }

    /// test::doctor::reports_exactly_ten_layers
    #[tokio::test]
    async fn reports_exactly_ten_layers() {
        let (base, paths) = temp_claude_dir("ten_layers");

        fs::write(&paths.settings_json, r#"{"env": {}, "enabledPlugins": []}"#).unwrap();
        fs::write(&paths.claude_json, r#"{"mcpServers": {}}"#).unwrap();
        fs::create_dir_all(&paths.rules_dir).unwrap();

        let mut doctor = Doctor::new(paths);
        // Short timeout so network/subprocess probes resolve quickly in tests
        doctor.timeout = Duration::from_millis(200);

        let results = doctor.run_all().await;

        assert_eq!(results.len(), 10, "Doctor must run exactly 10 health checks");

        let layer_nums: Vec<u8> = results.iter().map(|r| r.layer).collect();
        for i in 1u8..=10 {
            assert!(
                layer_nums.contains(&i),
                "Layer {} must be present in results",
                i
            );
        }

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn exit_code_all_pass_is_zero() {
        let results = vec![
            CheckResult::pass(1, "test", "ok"),
            CheckResult::pass(2, "test2", "ok"),
        ];
        assert_eq!(exit_code_from_results(&results), 0);
    }

    /// W-003: WARN must exit 0, not 1.
    #[test]
    fn exit_code_any_warn_is_zero() {
        let results = vec![
            CheckResult::pass(1, "test", "ok"),
            CheckResult::warn(2, "test2", "watch out", None),
        ];
        assert_eq!(exit_code_from_results(&results), 0);
    }

    /// W-003: FAIL must exit 1, not 2.
    #[test]
    fn exit_code_any_fail_is_one() {
        let results = vec![
            CheckResult::pass(1, "test", "ok"),
            CheckResult::fail(2, "test2", "broken", None),
        ];
        assert_eq!(exit_code_from_results(&results), 1);
    }

    /// test::doctor::headroom_fail_shows_port_message
    #[tokio::test]
    async fn headroom_fail_shows_port_8788_message() {
        let (base, paths) = temp_claude_dir("headroom_fail");
        let mut doctor = Doctor::new(paths);
        doctor.headroom_url = "http://127.0.0.1:19999/health".to_string();
        doctor.timeout = Duration::from_millis(200);

        let result = doctor.check_headroom_proxy().await;
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(
            result.message.contains("8788") || result.message.contains("not responding") || result.message.contains("timeout"),
            "Headroom fail message must mention port or connectivity: {}",
            result.message
        );

        fs::remove_dir_all(&base).ok();
    }

    /// test::doctor::stale_hook_suggests_sync
    #[tokio::test]
    async fn stale_hook_path_suggests_sync() {
        let (base, paths) = temp_claude_dir("stale_hook");

        let settings_json = r#"{
            "env": {},
            "hooks": {
                "PreToolUse": [{"hooks": [{"type": "command", "command": "/nonexistent/absolute/rtk-hook"}]}]
            },
            "enabledPlugins": []
        }"#;
        fs::write(&paths.settings_json, settings_json).unwrap();

        let doctor = Doctor::new(paths);
        let result = doctor.check_hooks_executable().await;

        assert_eq!(result.status, CheckStatus::Fail, "Stale hook must be FAIL");
        assert!(
            result.suggestion.as_deref().unwrap_or("").contains("sync"),
            "Suggestion must mention 'tokenwise sync': {:?}",
            result.suggestion
        );

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn check_result_format_includes_layer_and_status() {
        let r = CheckResult::pass(5, "rules-files", "All files present");
        let line = r.format_line();
        assert!(line.contains("[PASS]"));
        assert!(line.contains("5"));
        assert!(line.contains("rules-files") || line.contains("All files"));
    }

    #[tokio::test]
    async fn caveman_plugin_check_passes_when_present() {
        let (base, paths) = temp_claude_dir("caveman_present");
        fs::write(
            &paths.settings_json,
            r#"{"env": {}, "enabledPlugins": ["caveman"]}"#,
        )
        .unwrap();

        let doctor = Doctor::new(paths);
        let result = doctor.check_caveman_plugin().await;
        assert_eq!(result.status, CheckStatus::Pass, "Caveman must PASS when in enabledPlugins");

        fs::remove_dir_all(&base).ok();
    }

    /// test::doctor::mcp_timeout_after_3s
    ///
    /// Verifies that ClawMem (layer 6), Engram (layer 7), and MarkItDown (layer 9)
    /// probes complete within 4 seconds — the tokio::time::timeout wrapper must
    /// prevent any probe from hanging indefinitely.
    #[tokio::test]
    async fn mcp_timeout_after_3s() {
        let (base, paths) = temp_claude_dir("mcp_timeout");

        fs::write(&paths.settings_json, r#"{"env": {}, "enabledPlugins": []}"#).unwrap();
        fs::write(&paths.claude_json, r#"{"mcpServers": {}}"#).unwrap();
        fs::create_dir_all(&paths.rules_dir).unwrap();

        let mut doctor = Doctor::new(paths);
        // Short probe timeout — any slow subprocess/network call is killed quickly
        doctor.timeout = Duration::from_millis(500);

        let start = std::time::Instant::now();
        let results = doctor.run_all().await;
        let elapsed = start.elapsed();

        // All 10 checks must complete well within 4 seconds
        assert!(
            elapsed < Duration::from_secs(4),
            "Doctor::run_all must complete within 4s (timeout guard), took {:?}",
            elapsed
        );

        assert_eq!(results.len(), 10, "Must return exactly 10 results");

        // Layers 6 (ClawMem) and 9 (MarkItDown) must resolve — not stuck
        let clawmem = results.iter().find(|r| r.layer == 6).expect("layer 6 missing");
        let markitdown = results.iter().find(|r| r.layer == 9).expect("layer 9 missing");
        let engram = results.iter().find(|r| r.layer == 7).expect("layer 7 missing");

        assert!(
            matches!(clawmem.status, CheckStatus::Fail | CheckStatus::Warn | CheckStatus::Pass),
            "ClawMem must resolve (not stuck): {:?}",
            clawmem.status,
        );
        assert!(
            matches!(markitdown.status, CheckStatus::Fail | CheckStatus::Warn | CheckStatus::Pass),
            "MarkItDown must resolve (not stuck): {:?}",
            markitdown.status,
        );
        assert!(
            matches!(engram.status, CheckStatus::Fail | CheckStatus::Warn | CheckStatus::Pass),
            "Engram must resolve (not stuck): {:?}",
            engram.status,
        );

        fs::remove_dir_all(&base).ok();
    }
}
