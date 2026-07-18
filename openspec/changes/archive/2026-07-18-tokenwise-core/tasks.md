# Tasks: tokenwise-core

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~2,600–3,200 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR1 (scaffold + common + core + CLI + doctor + sync) → PR2 (connect + install + stats + CI) |
| Delivery strategy | single-pr (pre-authorized split when > 400 lines) |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | PR | Base | Notes |
|------|------|----|------|-------|
| 1 | Scaffold + common + core traits + CLI skeleton + doctor + sync | PR1 | main | ~1,600–1,900 lines; all PR1 tests pass; no install/connect/stats |
| 2 | connect (claude + hermes) + install + stats + CI/Release | PR2 | PR1 | ~900–1,200 lines; integration tests; GitHub Release |

---

## Group 1: Project Scaffold — PR1

> Sequential: T-001 → T-002. Unblocks all other groups.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-001 | Initialize Cargo workspace `Cargo.toml` with `[workspace]` members: `tokenwise`, `tokenwise-core`, `adapter-claude`, `adapter-hermes`, `tokenwise-common`. Create each crate via `cargo new`. | `/Cargo.toml`, `tokenwise/`, `tokenwise-core/`, `adapter-claude/`, `adapter-hermes/`, `tokenwise-common/` (6 Cargo.toml stubs) | — | `cargo build` passes | S |
| T-002 | Add all crate-level dependencies. `tokenwise-common`: `thiserror 1`, `serde/derive`, `chrono 0.4`. `tokenwise-core`: `tokenwise-common`, `serde_json 1`, `tokio full`, `reqwest 0.11`, `which 6`, `dirs 5`, `tempfile 3`, `async-trait 0.1`, `futures 0.3`. `adapter-claude`: `tokenwise-common`, `serde_json`. `adapter-hermes`: `tokenwise-common`, `serde_yaml 0.9`. `tokenwise`: `clap 4/derive+env`, `tokenwise-core`, `adapter-claude`, `adapter-hermes`, `tracing 0.1`, `tracing-subscriber 0.3`. | All 5 crate `Cargo.toml` files | T-001 | `cargo build` passes | S |

---

## Group 2: tokenwise-common — PR1

> T-003 through T-006 can run in parallel after T-002. T-007 depends on all four.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-003 | Implement `Platform` enum (`MacOS`, `Linux`, `Windows`) with `fn detect() -> Result<Platform>` using `#[cfg(target_os)]`; each variant carries service-manager path constants (plist dir, systemd dir, schtasks name). | `tokenwise-common/src/platform.rs` | T-002 | `test::platform::binary_detection_covers_all_targets` | S |
| T-004 | Implement `TokenwiseError` via `thiserror` (variants: `Io`, `Json`, `Http`, `NotFound`, `PortInUse`, `MissingPrerequisite`, `InvalidInvocation`). Implement `ExitCode` enum `Success=0 / Failure=1 / InvalidInvocation=2` with `fn into_code(self) -> i32`. | `tokenwise-common/src/error.rs`, `tokenwise-common/src/exit_code.rs` | T-002 | `test::cli::exit_codes_contract` | S |
| T-005 | Implement `Level` enum (`ERROR`, `WARN`, `INFO`) and `fn format_message(level: Level, msg: &str, suggestion: Option<&str>) -> String` producing `[LEVEL] Message. Suggestion.` (suggestion appended only when `Some`). | `tokenwise-common/src/output.rs` | T-002 | `test::cli::error_format_matches_pattern` | S |
| T-006 | Implement `BackupManager { backup_dir: PathBuf }` with `fn backup(&self, path: &Path) -> Result<PathBuf>`. Backup format: `{filename}.{iso8601}.bak` using `chrono::Utc::now().format("%Y-%m-%dT%H%M%SZ")` (e.g. `settings.json.2024-01-15T103000Z.bak`). Writes to `~/.tokenwise/backups/`. | `tokenwise-common/src/backup.rs` | T-002 | `test::backup::creates_timestamped_backup` | S |
| T-007 | Unit tests for all `tokenwise-common` modules: assert OS enum has all 3 variants + distinct path mappings; assert exit codes map to correct integers; assert format output matches regex `\[(ERROR\|WARN\|INFO)\] .+`; assert backup filename contains ISO 8601 timestamp and `.bak` suffix. | `tokenwise-common/src/lib.rs` (re-exports), `tokenwise-common/tests/` | T-003–T-006 | T-003–T-006 test anchors | M |

---

## Group 3: tokenwise-core — PR1

> T-008 first (trait def). T-009/T-010/T-011 parallel (platform impls). T-012/T-013/T-014 parallel (writers). T-015 last (tests).

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-008 | Define `#[async_trait] pub trait ServiceManager` with methods `install_service`, `start_service`, `stop_service`, `uninstall_service`, `service_status`. Define `ServiceConfig { name, description, executable, args }` and `ServiceStatus` enum. | `tokenwise-core/src/service/mod.rs` | T-003, T-007 | structural; exercised by T-015 | S |
| T-009 | Implement `MacOSServiceManager`: `generate_plist(config) -> String` (xml plist with `RunAtLoad` + `KeepAlive`); `launchctl load/unload`; write to `~/Library/LaunchAgents/{name}.plist`. | `tokenwise-core/src/service/macos.rs` | T-008 | `test::install::service_unit_written_per_platform` | M |
| T-010 | Implement `SystemdServiceManager`: `generate_unit(config) -> String` (`[Unit]/[Service]/[Install]`); `systemctl enable --now`; write to `/etc/systemd/system/{name}.service`. | `tokenwise-core/src/service/linux.rs` | T-008 | `test::install::service_unit_written_per_platform` | M |
| T-011 | Implement `WindowsServiceManager`: `generate_task_xml(config) -> String` (Task Scheduler XML); invoke `schtasks.exe /create /tn {name} /xml -` piping XML to stdin. Do NOT use `sc.exe`. | `tokenwise-core/src/service/windows.rs` | T-008 | `test::install::service_unit_written_per_platform` | M |
| T-012 | Implement `SettingsManager { backup_manager: BackupManager }` with `fn update_settings(&self, path: &Path) -> Result<()>`: backup → read-or-default → merge tokenwise entries (ANTHROPIC_BASE_URL, hooks) → write back; preserve all unknown keys via `#[serde(flatten)] extra: Value`. | `tokenwise-core/src/settings_manager.rs` | T-006, T-007 | `test::connect_claude::preserves_existing_env_keys` | M |
| T-013 | Implement `McpRegistry { backup_manager: BackupManager }` with `fn write_servers(&self, path: &Path, include_optional: bool) -> Result<()>`. Hardcode 7 core servers: `markitdown` (Python FastMCP `~/markitdown_env/mcp_markitdown_server.py`), `headroom`, `clawmem`, `engram` (plugin:engram:engram), `serena`, `codebase-memory-mcp`, `mcp-registry`. Optional servers (registered only if `include_optional=true` or user config): `odoo-customext`, `shopify`. Read-merge-write `~/.claude.json`. | `tokenwise-core/src/mcp_registry.rs` | T-006, T-007 | `test::connect_claude::writes_anthropic_base_url` (via connector) | M |
| T-014 | Implement `RulesWriter` with `fn write_all(&self, rules_dir: &Path) -> Result<()>`: create `rules_dir` if absent (`fs::create_dir_all`); write 5 rule files: `headroom-pipeline.md`, `unified-optimization-pipeline.md`, `headroom-mandatory-guarantee.md`, `markitdown-mcp.md`, `response-statusbar.md`. Content embedded as Rust string literals. | `tokenwise-core/src/rules_writer.rs` | T-006, T-007 | `test::connect_claude::creates_rules_dir_if_absent` | S |
| T-015 | Unit tests for tokenwise-core: plist XML contains `RunAtLoad`+`KeepAlive`; systemd unit contains `[Install]`; Windows XML passes schtasks format; SettingsManager preserves `"custom_key"` fixture; McpRegistry merge keeps pre-existing server; RulesWriter creates dir. | `tokenwise-core/tests/` | T-008–T-014 | multiple (see above) | M |

---

## Group 4: adapter-claude models — PR1

> T-016 parallel with T-009–T-014. T-017/T-018 are PR2.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-016 | Implement serde models: `ClaudeSettings { env: HashMap<String,String>, hooks: Vec<Hook>, #[serde(flatten)] extra: Value }`, `Hook { event: HookEvent, command: String, args: Vec<String>, marker: Option<String> }`, `HookEvent` enum (`PreToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`, `PreCompact`). `ClaudeMcpConfig { mcp_servers: HashMap<String, McpServer> }`, `McpServer { command, args, env? }`. | `adapter-claude/src/models.rs` | T-007 | structural | S |

---

## Group 5: tokenwise CLI skeleton — PR1

> T-021 after T-016. T-022 after T-021. T-023 after T-022.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-021 | Implement `main.rs` with clap derive: top-level `Cli` struct, subcommands `Install`, `Connect { agent: AgentKind }` (enum: `Claude`, `Hermes`), `Doctor`, `Sync`, `Stats`. Setup `tracing_subscriber`. Route each subcommand to stub `Err(ExitCode::Failure)` in PR1 (connect/install/stats stubs); doctor and sync are real. | `tokenwise/src/main.rs`, `tokenwise/src/commands/mod.rs` | T-016, T-004 | — | M |
| T-022 | Implement exit code contract in CLI dispatch: all command handlers return `Result<(), ExitCode>`; `main` calls `std::process::exit(code.into_code())`. Unknown subcommand → `ExitCode::InvalidInvocation`. | `tokenwise/src/main.rs` | T-004, T-021 | `test::cli::exit_codes_contract` | S |
| T-023 | CLI tests: assert `tokenwise --help` exits 0; assert unknown subcommand exits 2; assert mocked doctor-all-fail exits 1; assert stderr of any error matches `\[(ERROR\|WARN\|INFO)\] .+`. | `tokenwise/tests/cli_tests.rs` | T-022 | `test::cli::exit_codes_contract`, `test::cli::error_format_matches_pattern` | S |

---

## Group 6: doctor command (10 layers) — PR1

> T-024 → T-025.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-024 | Implement `Doctor { checks: Vec<HealthCheck> }` with `async fn run_all() -> Vec<CheckResult>` using `futures::future::join_all`. Register 10 checks: (1) Headroom HTTP GET `127.0.0.1:8788/health` 3s timeout, (2) `rtk --version` exits 0, (3) hook paths from `settings.json` exist+executable, (4) 9 MCP server keys in `~/.claude.json`, (5) 5 rules files exist+size>0, (6) ClawMem MCP responds 3s, (7) Engram MCP `mem_context` responds 3s, (8) OS service unit file on disk, (9) MarkItDown MCP responds 3s, (10) Caveman present in `settings.json` `enabledPlugins`. Exit 0 all-OK, exit 1 any-FAIL. FAIL output includes `Suggestion: run 'tokenwise sync' to repair.` for stale paths. **Note**: spec test `reports_exactly_nine_layers` renamed to `reports_exactly_ten_layers` due to Caveman addition. | `tokenwise-core/src/doctor.rs`, `tokenwise/src/commands/doctor.rs` | T-005, T-007, T-008, T-021 | `test::doctor::reports_exactly_ten_layers` | L |
| T-025 | Doctor tests: (1) assert exactly 10 `[OK]` lines on all-healthy mock; (2) assert exit 0 all-OK; (3) assert exit 1 any-FAIL; (4) Headroom fail shows port 8788 message; (5) stale hook shows sync suggestion; (6) MCP mock with 3s+ delay triggers FAIL. Use `tokio::test` + mock HTTP server. | `tokenwise-core/tests/doctor_tests.rs` | T-024 | all 6 doctor tests (renamed) | M |

---

## Group 7: sync command — PR1

> T-026 → T-027.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-026 | Implement `SyncRunner`: read hook commands from `settings.json` + MCP commands from `~/.claude.json`; for each path: (a) exists → `[OK]`; (b) missing → search PATH + `~/.tokenwise/components/` → if found: backup + rewrite → `[REPAIRED]`; if not found → `[UNRESOLVED]`; after repair: reload OS service (`launchctl unload+load` / `systemctl restart` / Windows schtasks restart). Exit 0 when all OK or all repaired; exit 1 when any UNRESOLVED. | `tokenwise-core/src/sync.rs`, `tokenwise/src/commands/sync.rs` | T-006, T-008, T-012, T-021 | `test::sync::repairs_tmp_hook_path` | M |
| T-027 | Sync tests: (1) fixture with `/private/tmp/hook` path → assert rewritten to canonical, backup exists, log contains `[REPAIRED]`; (2) all-valid paths → `[INFO] All paths verified. Nothing to repair.` exit 0; (3) unresolvable path → `[UNRESOLVED]` + install suggestion + exit 1; (4) assert service reload called after repair. | `tokenwise-core/tests/sync_tests.rs` | T-026 | all 4 sync tests | M |

---

## Group 8: adapter-claude connector — PR2

> T-017 → T-018.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-017 | Implement `ClaudeConnector::connect(include_optional_mcps: bool) -> Result<()>`: (1) locate `~/.claude/`; (2) `BackupManager::backup(settings.json)` + `BackupManager::backup(.claude.json)`; (3) `SettingsManager::update_settings` (set `ANTHROPIC_BASE_URL=http://127.0.0.1:8788`, add RTK pre-tool-use + session-start hooks with absolute non-tmp paths, marker `"tokenwise"`); (4) `McpRegistry::write_servers(.claude.json, include_optional)`; (5) `RulesWriter::write_all(~/.claude/rules/)`; (6) idempotent: detect existing marker and skip re-write. | `adapter-claude/src/connector.rs` | T-012, T-013, T-014, T-016 | `test::connect_claude::idempotent_no_diff` | L |
| T-018 | Tests for adapter-claude connector using `tempfile::TempDir`: (1) `writes_anthropic_base_url`; (2) `preserves_existing_env_keys` — pre-populate `MY_KEY`, assert still present after connect; (3) `hooks_use_absolute_non_tmp_paths` — assert no `command` starts with `/tmp` or `/private/tmp`; (4) `creates_rules_dir_if_absent` — start with no rules dir; (5) `idempotent_no_diff` — run twice, assert files byte-equal. | `adapter-claude/tests/connector_tests.rs` | T-017 | all 5 connect_claude tests | M |

---

## Group 9: adapter-hermes connector — PR2

> T-019 → T-020.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-019 | Implement `HermesConnector::connect() -> Result<()>`: (1) locate Hermes config dir via `which hermes` + known fallback paths; (2) error `[ERROR] Hermes agent not found. Install it first or run 'tokenwise install'.` + exit 1 if absent; (3) write MCP servers to Hermes MCP config YAML; (4) write 5 rule files to Hermes rules dir; (5) write hook definitions (absolute paths only); (6) set `ANTHROPIC_BASE_URL` in Hermes env config; (7) idempotent. | `adapter-hermes/src/connector.rs`, `adapter-hermes/src/models.rs` | T-012, T-013, T-014, T-007 | `test::connect_hermes::idempotent` | M |
| T-020 | Tests for adapter-hermes: (1) `registers_mcp_servers` — assert all 7 core MCP keys present in written YAML; (2) `exits_1_when_not_installed` — mock `which hermes` failure → assert exit 1 + correct error message; (3) `hooks_are_absolute_paths` — assert all hook commands are absolute and exist on fs (mock fs); (4) `idempotent` — run twice, assert config unchanged. | `adapter-hermes/tests/connector_tests.rs` | T-019 | all 4 connect_hermes tests | S |

---

## Group 10: stats command — PR2

> T-028 → T-029.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-028 | Implement `StatsAggregator` with `async fn collect() -> Stats` using `tokio::join!`: (a) `rtk gain --json` → parse JSON → `LayerStats`; (b) HTTP GET `http://127.0.0.1:8788/stats` → parse; (c) ClawMem MCP stats call or local stats file. Degrade to `LayerStats::Unavailable(reason)` per layer on error. Output table: per-layer % + totals row + ClawMem cache hit rate. If no layer has data → `[INFO] No savings data recorded yet.` exit 0. | `tokenwise-core/src/stats.rs`, `tokenwise/src/commands/stats.rs` | T-005, T-007, T-021 | `test::stats::handles_missing_headroom` | M |
| T-029 | Stats tests with mocked subprocess + HTTP: (1) `aggregates_rtk_json_output` — mock `rtk gain --json` → assert correct % in output; (2) `handles_missing_headroom` — no HTTP response → Headroom row shows `unavailable (proxy not running)`, RTK row still present; (3) `handles_fresh_install_no_data` — all layers return empty → INFO message + exit 0; (4) `rtk_not_installed_row_unavailable` — `which rtk` fails → RTK row `unavailable (rtk not installed)`. | `tokenwise-core/tests/stats_tests.rs` | T-028 | all 4 stats tests | S |

---

## Group 11: install command — PR2

> T-030 → T-031.

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-030 | Implement `Installer` with `async fn install() -> Result<()>`: (1) detect platform; (2) check prerequisites: Python 3.8+ (`python3 --version`), Node 18+ (`node --version`), Git (`git --version`) → exit 1 + `[ERROR] {dep} required.` on missing; (3) check port 8788 → exit 1 `[ERROR] Port 8788 is in use by PID {pid}.` if bound by non-Headroom; (4) install 9 components in order to `~/.tokenwise/components/` (pip install, binary download+sha256 verify, hook scripts); (5) idempotent: `rtk --version` passes → `[INFO] {component}: already installed`; (6) register OS service via `ServiceManager`; exit 0 on full success. | `tokenwise-core/src/install.rs`, `tokenwise/src/commands/install.rs` | T-006, T-008, T-009, T-010, T-011, T-021 | `test::install::idempotent_skips_installed_components` | L |
| T-031 | Install tests with mock command executor + mock fs: (1) `idempotent_skips_installed_components` — pre-mark all components as installed, assert no install commands called; (2) `port_conflict_exits_1` — mock port 8788 bound → assert exit 1 + PID in message; (3) `missing_python_exits_1` — mock `python3` not found → assert exit 1 + install URL in message; (4) `service_unit_written_per_platform` — assert plist/unit/xml written to correct path per target OS. | `tokenwise-core/tests/install_tests.rs` | T-030 | all 4 install tests | M |

---

## Group 12: CI/Release Pipeline — PR2

> T-032 and T-033 are independent (parallel).

| ID | Description | Files | Deps | TDD | Est |
|----|-------------|-------|------|-----|-----|
| T-032 | Write GitHub Actions release workflow: matrix (ubuntu-latest/x86_64-unknown-linux-gnu, macos-latest/aarch64-apple-darwin, macos-latest/x86_64-apple-darwin, windows-latest/x86_64-pc-windows-msvc); `dtolnay/rust-toolchain@stable`; `cargo build --release --target ${{ matrix.target }}`; strip symbols on unix; `upload-artifact@v4`; release job downloads all + `softprops/action-gh-release@v1` with `generate_release_notes: true`. | `.github/workflows/release.yml` | T-001 | `cargo build --release` passes in CI | M |
| T-033 | Write installation scripts: `install.sh` (bash: detect `uname -s` + `uname -m` → construct URL, `curl -L` download, `chmod +x`, `sudo mv /usr/local/bin/tokenwise`, run `tokenwise install`); `install.ps1` (PowerShell: detect arch, `Invoke-WebRequest`, copy to `$env:ProgramFiles\tokenwise`, add to Machine PATH, run install). | `scripts/install.sh`, `scripts/install.ps1` | T-001 | manual on target OS | S |

---

## Dependency Graph Summary

```
T-001 → T-002 → [T-003, T-004, T-005, T-006] → T-007
                                                    │
                      ┌─────────────────────────────┘
                      ▼
              T-008 → [T-009, T-010, T-011]
              T-007 → [T-012, T-013, T-014, T-016]
                      └──────────────┐
                                     ▼
                              T-015 (core tests)
                                     │
                      ┌──────────────┘
                      ▼
              T-021 → T-022 → T-023        (CLI skeleton — PR1)
              T-024 → T-025               (doctor — PR1)
              T-026 → T-027               (sync — PR1)

── PR1 merges to main ──────────────────────────────────────

              T-017 → T-018               (connect claude — PR2)
              T-019 → T-020               (connect hermes — PR2)
              T-028 → T-029               (stats — PR2)
              T-030 → T-031               (install — PR2)
              T-032, T-033 (parallel)     (CI/scripts — PR2)
```

## Spec Deviations Tracked (Gate Review Gaps)

| Gap | Resolution |
|-----|------------|
| ADR-001 says "4 crates" but lists 5 | Tasks use correct 5-crate structure (T-001) |
| `tokenwise connect claude/hermes` routing | Explicit in T-021 (clap `AgentKind` enum) |
| Exit code contract 0/1/2 | T-004 + T-022 |
| Structured error format `[LEVEL] Msg. Suggestion.` | T-005 |
| `futures`, `chrono`, `async-trait` deps | T-002 |
| Windows: `schtasks.exe` not `sc.exe` | T-011 explicit |
| Backup format `{filename}.{iso8601}.bak` | T-006 (chrono UTC format) |
| Caveman as 10th doctor layer | T-024 (check `enabledPlugins`) |
| `test::doctor::reports_exactly_nine_layers` | Renamed to `_ten_layers` in T-024/T-025 |
| MCP registry: 7 core + 2 optional | T-013 (explicit hardcode + `include_optional` flag) |
