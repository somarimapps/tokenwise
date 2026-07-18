# Tokenwise Core Specification

> **Change**: tokenwise-core | **Type**: New capabilities (6) | **TDD**: cargo test

## Purpose

Tokenwise is a statically-linked Rust CLI binary that installs, connects, verifies, and repairs the 9-layer token optimization stack (RTK, MarkItDown, Headroom, ClawMem, Engram, OS service, hooks, MCP registrations, rules files) for Claude Code and Hermes agent across macOS, Linux, and Windows.

---

## Global Requirements

### Requirement: Cross-Platform Binary

The system MUST compile to a statically-linked binary for macOS (arm64, x86_64), Linux (x86_64), and Windows (x86_64). All commands MUST detect the OS at runtime and select the correct service manager, config path, and shell integration.

#### Scenario: Binary runs on each supported OS

- GIVEN a fresh machine of any supported platform with no prior installation
- WHEN the user runs `tokenwise --version`
- THEN the binary outputs `tokenwise {semver}` and exits 0

*Cargo test*: `test::platform::binary_detection_covers_all_targets` — asserts OS detection enum has variants for macOS, Linux, Windows with distinct service-manager and path mappings.

---

### Requirement: Config Backup

Before modifying any existing config file, the system MUST write a timestamped backup to `~/.tokenwise/backups/{filename}.{iso8601}.bak`.

#### Scenario: Backup written before modification

- GIVEN `~/.claude/settings.json` contains user content
- WHEN any tokenwise command modifies that file
- THEN a backup is present at `~/.tokenwise/backups/settings.json.{iso8601}.bak` before the write completes

*Cargo test*: `test::backup::creates_timestamped_backup` — writes fixture file, runs backup fn, asserts file exists with ISO 8601 name.

---

### Requirement: Exit Code Contract

| Exit code | Meaning |
|-----------|---------|
| 0 | Success — all checks passed or all repairs completed |
| 1 | Failure — at least one component failed or error unresolved |
| 2 | Invalid invocation — unknown command or missing required argument |

*Cargo test*: `test::cli::exit_codes_contract` — asserts each code under mocked success, failure, and invalid-invocation scenarios.

---

### Requirement: Structured Error Format

All error output MUST follow: `[LEVEL] Message. Suggestion.` where LEVEL is `ERROR`, `WARN`, or `INFO`.

*Cargo test*: `test::cli::error_format_matches_pattern` — regex `\[(ERROR|WARN|INFO)\] .+` against all captured stderr lines.

---

## Capability: stack-install (`tokenwise install`)

### Requirement: Full Stack Installation

`tokenwise install` MUST detect and install all 9 stack components in dependency order. The 9 components are:

| # | Component | Install Method |
|---|-----------|---------------|
| 1 | RTK binary | Download from GitHub Releases or build from source |
| 2 | MarkItDown MCP | `pip install markitdown-mcp` |
| 3 | Headroom proxy binary | Download from GitHub Releases |
| 4 | ClawMem MCP | `pip install clawmem-mcp` |
| 5 | Engram MCP | `pip install engram-mcp` |
| 6 | OS service unit | Write plist / systemd unit / Windows service |
| 7 | Claude Code hook scripts | Write to `~/.tokenwise/hooks/` |
| 8 | MCP JSON registrations | Write to `~/.claude.json` |
| 9 | 5 rules Markdown files | Write to `~/.claude/rules/` |

The command MUST be idempotent: already-installed components MUST be skipped.

Preconditions: internet access; Python 3.8+, Node.js 18+, and Git are available.

#### Scenario: Fresh install on macOS

- GIVEN no stack components are installed
- WHEN the user runs `tokenwise install`
- THEN all 9 components are installed, the Headroom plist is written to `~/Library/LaunchAgents/com.headroom.proxy.plist`, and exit code is 0 with a per-component success summary

#### Scenario: Idempotent re-install

- GIVEN all 9 components are already installed and healthy
- WHEN the user runs `tokenwise install`
- THEN no components are reinstalled, each line shows `[INFO] {component}: already installed`, and exit code is 0

#### Scenario: Missing Python dependency

- GIVEN Python 3.8+ is not present
- WHEN the user runs `tokenwise install`
- THEN exit code is 1 with `[ERROR] Python 3.8+ required. Install from https://python.org and retry.`

#### Scenario: Port 8788 occupied by a non-Headroom process

- GIVEN port 8788 is already bound by another process
- WHEN `tokenwise install` tries to register the Headroom service
- THEN exit code is 1 with `[ERROR] Port 8788 is in use by PID {pid}. Free the port or configure an alternative.`

#### Scenario: Fresh install on Linux

- GIVEN a clean Linux x86_64 system with Python 3.8+ and Git
- WHEN the user runs `tokenwise install`
- THEN the systemd unit is written to `/etc/systemd/system/headroom-proxy.service`, `systemctl enable --now` is invoked, and exit code is 0

#### Scenario: Fresh install on Windows

- GIVEN a clean Windows x86_64 system with Python 3.8+ and Git
- WHEN the user runs `tokenwise install`
- THEN a Windows service is registered via `sc.exe` and exit code is 0

*Cargo tests*:
- `test::install::idempotent_skips_installed_components`
- `test::install::port_conflict_exits_1`
- `test::install::missing_python_exits_1`
- `test::install::service_unit_written_per_platform`

---

## Capability: claude-connect (`tokenwise connect claude`)

### Requirement: Claude Code Configuration Writer

`tokenwise connect claude` MUST write or merge all required entries into the Claude Code config surface without deleting existing user keys.

Preconditions: `tokenwise install` completed; Claude Code is installed with `~/.claude/` directory present.

| Target | Change |
|--------|--------|
| `~/.claude/settings.json` `.env.ANTHROPIC_BASE_URL` | Set to `"http://127.0.0.1:8788"` |
| `~/.claude/settings.json` `.hooks` | Add pre-tool-use and session-start RTK hook entries |
| `~/.claude.json` `.mcpServers` | Register all 9 MCP servers by name |
| `~/.claude/rules/headroom-pipeline.md` | Write rule file (create directory if absent) |
| `~/.claude/rules/unified-optimization-pipeline.md` | Write rule file |
| `~/.claude/rules/headroom-mandatory-guarantee.md` | Write rule file |
| `~/.claude/rules/markitdown-mcp.md` | Write rule file |
| `~/.claude/rules/response-statusbar.md` | Write rule file |

All hook `command` values MUST be absolute paths to installed binaries. Paths pointing to `/tmp` or `/private/tmp` are PROHIBITED.

#### Scenario: Fresh connect

- GIVEN Claude Code is installed and `~/.claude/` exists with no tokenwise entries
- WHEN the user runs `tokenwise connect claude`
- THEN all entries in the table above are written, existing configs are backed up, and exit code is 0

#### Scenario: Existing env keys are preserved

- GIVEN `~/.claude/settings.json` already contains `env: { "MY_KEY": "value" }`
- WHEN `tokenwise connect claude` runs
- THEN `MY_KEY` is still present and `ANTHROPIC_BASE_URL` is added — no keys deleted

#### Scenario: rules directory does not exist

- GIVEN `~/.claude/rules/` does not exist
- WHEN `tokenwise connect claude` runs
- THEN the directory is created and all 5 rule files are written

#### Scenario: Hook paths are absolute and non-temporary

- GIVEN `tokenwise connect claude` writes hook entries
- WHEN the written `~/.claude/settings.json` is inspected
- THEN every hook `command` value is an absolute path that does not start with `/tmp` or `/private/tmp`

#### Scenario: Connect is idempotent

- GIVEN all config entries are already correctly written
- WHEN `tokenwise connect claude` runs again
- THEN no config files change and exit code is 0

*Cargo tests*:
- `test::connect_claude::writes_anthropic_base_url`
- `test::connect_claude::preserves_existing_env_keys`
- `test::connect_claude::hooks_use_absolute_non_tmp_paths`
- `test::connect_claude::creates_rules_dir_if_absent`
- `test::connect_claude::idempotent_no_diff`

---

## Capability: hermes-connect (`tokenwise connect hermes`)

### Requirement: Hermes Agent Configuration Writer

`tokenwise connect hermes` MUST write the equivalent configuration surface for Hermes agent as `connect claude` does for Claude Code.

Preconditions: `tokenwise install` completed; Hermes agent is installed and its config directory is discoverable.

The command MUST:
1. Locate the Hermes config directory using OS-specific discovery
2. Register the same MCP servers in Hermes MCP config
3. Write equivalent hook definitions pointing to installed RTK binaries (absolute paths only)
4. Write the 5 optimization rule files to the Hermes rules directory
5. Set `ANTHROPIC_BASE_URL=http://127.0.0.1:8788` in Hermes env config

#### Scenario: Fresh Hermes connect

- GIVEN Hermes agent is installed and its config directory is found
- WHEN the user runs `tokenwise connect hermes`
- THEN MCP servers are registered, rule files written, hooks configured with absolute paths, `ANTHROPIC_BASE_URL` set, and exit code is 0

#### Scenario: Hermes not installed

- GIVEN Hermes agent binary is not found on PATH or in known install locations
- WHEN the user runs `tokenwise connect hermes`
- THEN exit code is 1 with `[ERROR] Hermes agent not found. Install it first or run 'tokenwise install'.`

#### Scenario: Hook paths are absolute

- GIVEN Hermes connect writes hook definitions
- WHEN the Hermes hook config is inspected
- THEN all hook `command` values are absolute paths that exist on the filesystem

#### Scenario: Connect is idempotent

- GIVEN Hermes config is already fully written
- WHEN `tokenwise connect hermes` runs again
- THEN no changes are made and exit code is 0

*Cargo tests*:
- `test::connect_hermes::registers_mcp_servers`
- `test::connect_hermes::exits_1_when_not_installed`
- `test::connect_hermes::hooks_are_absolute_paths`
- `test::connect_hermes::idempotent`

---

## Capability: health-check (`tokenwise doctor`)

### Requirement: 10-Layer Health Verification

`tokenwise doctor` MUST verify all 10 stack layers and report `OK`, `WARN`, or `FAIL` for each. The command MUST run on unconfigured systems (no preconditions).

| Layer | # | Verification method |
|-------|---|---------------------|
| Headroom proxy | 1 | HTTP GET `http://127.0.0.1:8788/health` — expect 200 within 3s |
| RTK binary | 2 | `rtk --version` exits 0 |
| Claude hooks executable | 3 | Hook paths from `~/.claude/settings.json` exist AND are executable (`+x`) |
| MCP servers registered | 4 | All expected keys present in `~/.claude.json` `.mcpServers` |
| Rules files present | 5 | All 5 `~/.claude/rules/*.md` files exist with size > 0 |
| ClawMem MCP | 6 | MCP responds within 3s |
| Engram MCP | 7 | MCP `mem_context` responds within 3s |
| OS service registered | 8 | Plist / systemd unit / Windows service entry exists on disk |
| MarkItDown MCP | 9 | MCP responds within 3s |
| Caveman plugin | 10 | Present in `settings.json` `enabledPlugins` |

Exit 0 when all 10 are OK. Exit 1 when any layer is FAIL.

#### Scenario: All layers healthy

- GIVEN all 10 stack components are installed and responding
- WHEN the user runs `tokenwise doctor`
- THEN output contains exactly 10 lines each with `[OK]` prefix and exit code is 0

#### Scenario: Headroom proxy not running

- GIVEN port 8788 is not bound
- WHEN the user runs `tokenwise doctor`
- THEN layer 1 shows `[FAIL] Headroom proxy not responding on port 8788` and exit code is 1

#### Scenario: Hook path is stale (primary sync trigger)

- GIVEN a hook entry references a path that does not exist on disk
- WHEN the user runs `tokenwise doctor`
- THEN layer 3 shows `[FAIL] hook path not found: {path}` and output includes `Suggestion: run 'tokenwise sync' to repair.`

#### Scenario: MCP timeout

- GIVEN ClawMem MCP does not respond within 3 seconds
- WHEN the user runs `tokenwise doctor`
- THEN layer 6 shows `[FAIL] ClawMem MCP timeout (>3s)` and exit code is 1

#### Scenario: Partial health (some OK, some FAIL)

- GIVEN layers 1–5 are healthy and layers 6–10 are unreachable
- WHEN the user runs `tokenwise doctor`
- THEN layers 1–5 show `[OK]`, layers 6–10 show `[FAIL]`, and exit code is 1

*Cargo tests*:
- `test::doctor::reports_exactly_ten_layers`
- `test::doctor::exits_0_all_ok`
- `test::doctor::exits_1_on_any_fail`
- `test::doctor::headroom_fail_shows_port_message`
- `test::doctor::stale_hook_suggests_sync`
- `test::doctor::mcp_timeout_after_3s`

---

## Capability: path-repair (`tokenwise sync`)

### Requirement: Dead Path Detection and Repair

`tokenwise sync` MUST scan all hook paths and MCP command paths, detect missing paths, and rewrite entries to current installed locations.

Primary failure mode: macOS wipes `/private/tmp/*` on reboot. Any hook or MCP `command` field pointing to `/tmp` or `/private/tmp` MUST be detected and repaired.

Preconditions: `tokenwise install` completed (so canonical install paths are known).

The command MUST:
1. Read all hook `command` entries from `~/.claude/settings.json`
2. Read all MCP `command` entries from `~/.claude.json`
3. For each path: verify existence; if missing, locate the installed binary at canonical install path and rewrite the entry
4. Reload the OS service if service-related paths were changed
5. Report `[REPAIRED]`, `[OK]`, or `[UNRESOLVED]` per path

#### Scenario: Hook path in /tmp wiped after macOS reboot (primary bug)

- GIVEN a hook entry references `/private/tmp/tokenwise-hook` which does not exist
- AND the installed hook is at `/usr/local/bin/rtk-hook`
- WHEN the user runs `tokenwise sync`
- THEN the entry is rewritten to `/usr/local/bin/rtk-hook`, a backup is written, and output logs `[REPAIRED] hook 'rtk-pre-tool' /private/tmp/tokenwise-hook → /usr/local/bin/rtk-hook`

#### Scenario: No dead paths found

- GIVEN all hook and MCP command paths exist and are executable
- WHEN the user runs `tokenwise sync`
- THEN output is `[INFO] All paths verified. Nothing to repair.` and exit code is 0

#### Scenario: Unresolvable path

- GIVEN a hook references a path that does not exist AND the component is not installed at any known location
- WHEN the user runs `tokenwise sync`
- THEN that path is marked `[UNRESOLVED] {path}`, output suggests `Run 'tokenwise install' to reinstall missing components.`, and exit code is 1

#### Scenario: OS service reloaded after path repair

- GIVEN a service unit references a binary path that was stale and repaired
- WHEN `tokenwise sync` completes
- THEN the OS service is restarted (`launchctl unload + load` / `systemctl restart` / Windows service restart) and output confirms reload

*Cargo tests*:
- `test::sync::repairs_tmp_hook_path`
- `test::sync::noop_when_all_paths_valid`
- `test::sync::unresolvable_path_exits_1`
- `test::sync::service_reloaded_after_repair`

---

## Capability: stats-aggregation (`tokenwise stats`)

### Requirement: Token Savings Reporting

`tokenwise stats` MUST read savings data from RTK, Headroom proxy, and ClawMem and display a per-layer and total summary. Unavailable layers MUST display "unavailable" without blocking the others.

Preconditions: at least one optimization layer has recorded savings data.

| Source | Collection method |
|--------|------------------|
| RTK savings | Parse JSON from `rtk gain --json` |
| Headroom compression rate | HTTP GET `http://127.0.0.1:8788/stats` |
| ClawMem hit rate | MCP stats call or local stats file |

Output MUST include:
- Per-layer savings percentage
- Total tokens saved (session and cumulative where available)
- ClawMem cache hit rate

#### Scenario: All layers reporting

- GIVEN RTK, Headroom, and ClawMem all have recorded data
- WHEN the user runs `tokenwise stats`
- THEN output contains a table with per-layer percentages and a totals row, and exit code is 0

#### Scenario: Headroom proxy offline

- GIVEN Headroom proxy is not running
- WHEN the user runs `tokenwise stats`
- THEN the Headroom row shows `unavailable (proxy not running)` and RTK and ClawMem rows still display correctly

#### Scenario: No data recorded yet

- GIVEN no optimization events have been recorded (fresh install)
- WHEN the user runs `tokenwise stats`
- THEN exit code is 0 with `[INFO] No savings data recorded yet. Run Claude Code or Hermes sessions first.`

#### Scenario: RTK not installed

- GIVEN RTK binary is not found
- WHEN the user runs `tokenwise stats`
- THEN the RTK row shows `unavailable (rtk not installed)` and other rows still display

*Cargo tests*:
- `test::stats::aggregates_rtk_json_output`
- `test::stats::handles_missing_headroom`
- `test::stats::handles_fresh_install_no_data`
- `test::stats::rtk_not_installed_row_unavailable`
