# tokenwise

Cross-platform Rust CLI that installs, connects, and maintains the 9-layer token optimization stack for Claude Code and Hermes agent.

## Measured savings

| Layer | What it compresses | Measured |
|-------|-------------------|---------|
| RTK | Shell command output (git, ps, find…) | 22.9% avg on CLI tokens ([source](https://github.com/somarimapps/tokenwise)) |
| Headroom proxy | API responses via compression middleware | Reports via `http://127.0.0.1:8788/stats` |
| MarkItDown | Binary files (PDF/DOCX/images) → Markdown | ~60-80% file size reduction |
| ClawMem + Engram | Eliminates re-explaining cross-session context | Immeasurable without baseline |

Run `tokenwise stats` to see your real numbers.

> RTK example (2,224 commands): **1.98M tokens saved** at 22.9% average. Your results depend on which shell commands Claude runs in your projects.

## What it does

The stack already exists (RTK, MarkItDown, Headroom proxy, ClawMem, Engram, Caveman, Ponytail, Serena, codebase-memory-mcp). `tokenwise` wires it all together so you don't have to do it manually on every machine.

```
tokenwise install     # detect platform → install Headroom service → wire Claude Code
tokenwise connect     # connect claude | hermes
tokenwise doctor      # 10-layer health check
tokenwise sync        # repair dead hook/MCP paths after updates
tokenwise stats       # show compression numbers
```

## Install

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/somarimapps/tokenwise/main/scripts/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/somarimapps/tokenwise/main/scripts/install.ps1 | iex
```

## Quick start

```bash
tokenwise install          # one-time setup — installs stack AND wires Claude Code automatically
tokenwise connect hermes   # wire Hermes agent (if installed)
tokenwise doctor           # verify all 10 layers are green
```

## The 10 layers

| # | Layer | What it does |
|---|-------|-------------|
| 1 | Headroom proxy | Intercepts all API traffic, compresses responses |
| 2 | RTK | Rewrites CLI commands to strip verbose output |
| 3 | Hooks | PreToolUse / PostToolUse shell hooks |
| 4 | MCP servers | 7 core servers (MarkItDown, ClawMem, Engram, Serena…) |
| 5 | Rules files | Auto-fire pipeline rules in `~/.claude/rules/` |
| 6 | ClawMem | Semantic vector memory across sessions |
| 7 | Engram | Structured persistent memory (decisions, bugs, patterns) |
| 8 | OS service | Headroom proxy keep-alive (LaunchAgent / systemd / schtasks) |
| 9 | MarkItDown | Binary → Markdown conversion for PDF/DOCX/images |
| 10 | Caveman | Response compression plugin |

## Workspace structure

```
tokenwise/           # CLI binary (clap)
tokenwise-core/      # ServiceManager, SettingsManager, McpRegistry, Doctor, Sync, Stats
tokenwise-common/    # Platform, error types, exit codes, output format, BackupManager
adapter-claude/      # Claude Code connector (settings.json, ~/.claude.json)
adapter-hermes/      # Hermes agent connector (config.yaml)
```

## Build from source

```bash
cargo build --release
```

Targets: `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success (or warnings — non-fatal) |
| 1 | One or more failures |
| 2 | Fatal / invalid invocation |

## License

MIT
