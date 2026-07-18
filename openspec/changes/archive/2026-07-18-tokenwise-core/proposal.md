# Proposal: Tokenwise - Token Optimization Stack Orchestrator

## Intent

Tokenwise solves the installation complexity, configuration drift, and cross-platform portability problems of the 9-layer token optimization stack for Claude Code and Hermes agent. Currently, users manually install and wire together RTK, MarkItDown, Headroom, ClawMem, Engram, and 4 other components through fragmented scripts, MCP configs, and hook files that break when temporary paths die or when migrating between machines. Tokenwise provides a single Rust binary that installs, connects, verifies, and repairs this entire stack automatically.

## Scope

### In Scope
- Single Rust binary orchestrator for the existing 9-layer stack
- OS detection and cross-platform installation (macOS/Linux/Windows)
- Automated service management (LaunchAgent/systemd/Task Scheduler)
- Claude Code and Hermes agent configuration writers
- Live health verification of all stack components
- Automatic repair of dead paths and broken configurations
- Token savings aggregation and reporting

### Out of Scope
- Reimplementing any existing stack component (except lightweight proxy if needed)
- GUI/dashboard interfaces
- Upstream tool version watching
- Tauri or Electron app wrappers
- Custom token optimization algorithms

## Capabilities

> This section is the CONTRACT between proposal and specs phases.
> The sdd-spec agent reads this to know exactly which spec files to create or update.
> Research `openspec/specs/` before filling this in.

### New Capabilities
<!-- Capabilities being introduced. Each becomes a new `openspec/specs/<name>/spec.md`.
     Use kebab-case names (e.g., user-auth, data-export, api-rate-limiting).
     Leave empty if no new capabilities. -->
- `stack-install`: Cross-platform detection and installation of all 9 stack components
- `claude-connect`: Claude Code settings.json and MCP server registration
- `hermes-connect`: Hermes agent configuration and skill definitions
- `health-check`: Live verification of all stack layers
- `path-repair`: Automatic fix for dead temporary paths and broken configs
- `stats-aggregation`: Token savings collection from all layers

### Modified Capabilities
<!-- Existing capabilities whose REQUIREMENTS are changing (not just implementation).
     Only list here if spec-level behavior changes. Each needs a delta spec.
     Use existing spec names from openspec/specs/. Leave empty if none. -->
None

## Approach

Cross-platform Rust CLI with static linking for portability. Uses native OS service APIs (launchctl/systemctl/sc.exe) for daemon management. Shells out to existing tools (RTK, MarkItDown, Headroom) rather than reimplementing them. Configuration generation through templated JSON/YAML writers. Health checks via HTTP probes (Headroom proxy), MCP server listing, and file presence verification. Distributed as GitHub Releases with platform-specific install scripts.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `~/.claude/settings.json` | Modified | Adds env vars, hooks, MCP servers |
| `~/.claude.json` | Modified | Registers MCP servers |
| `~/.claude/rules/` | New | Creates 5 auto-fire rule files |
| `~/Library/LaunchAgents/` | New | macOS service for Headroom proxy |
| `/etc/systemd/system/` | New | Linux service for Headroom proxy |
| `HKLM\SYSTEM\CurrentControlSet\Services` | New | Windows service for Headroom proxy |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Breaking existing Claude Code sessions | Low | Backup existing configs before modifying |
| Headroom proxy port conflict (8788) | Medium | Check port availability, offer alternative |
| Incompatible upstream tool versions | Medium | Pin known-good versions, test matrix in CI |
| Platform-specific service failures | Low | Fallback to user-mode execution |

## Rollback Plan

1. Run `tokenwise uninstall` (removes services, restores backed-up configs)
2. If uninstall fails: manually stop services (`launchctl unload` / `systemctl stop` / `sc delete`)
3. Restore configs from `~/.tokenwise/backups/` to original locations
4. Worst case: delete `~/.claude/settings.json` and `~/.claude.json` to force Claude Code config reset

## Dependencies

- Rust toolchain (for building from source)
- Git (for cloning stack components)
- Python 3.8+ (for MarkItDown, ClawMem, Engram MCPs)
- Node.js 18+ (for some MCP servers)
- Internet connection (for downloading components)

## Success Criteria

- [ ] Single command (`tokenwise install`) sets up entire stack on fresh machine
- [ ] `tokenwise doctor` correctly identifies and reports all 9 layer statuses
- [ ] `tokenwise sync` repairs at least 90% of common failure modes (dead paths, missing configs)
- [ ] Cross-platform: identical commands work on macOS, Linux, Windows
- [ ] Token savings visible: `tokenwise stats` shows aggregated compression percentages
- [ ] Zero manual configuration required after `tokenwise connect claude`
