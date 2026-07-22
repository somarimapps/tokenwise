use std::path::Path;

use tokenwise_common::TokenwiseError;

/// Writes the 6 mandatory tokenwise rule files to `~/.claude/rules/`.
///
/// Creates the directory if it does not exist (idempotent).
/// Files are overwritten with the latest canonical content on each run.
pub struct RulesWriter;

impl RulesWriter {
    /// Ensure the rules directory exists and write all 6 rule files.
    pub fn write_all(&self, rules_dir: &Path) -> Result<(), TokenwiseError> {
        std::fs::create_dir_all(rules_dir)?;

        for (filename, content) in RULE_FILES {
            let path = rules_dir.join(filename);
            std::fs::write(&path, content)?;
        }
        Ok(())
    }
}

/// All 6 canonical rule file contents embedded at compile time.
///
/// Content mirrors the rules that live in `~/.claude/rules/` on the user's
/// machine. Updating these strings in a future release automatically updates
/// users' installs via `tokenwise sync`.
pub static RULE_FILES: &[(&str, &str)] = &[
    ("headroom-pipeline.md", HEADROOM_PIPELINE),
    ("unified-optimization-pipeline.md", UNIFIED_OPTIMIZATION_PIPELINE),
    ("headroom-mandatory-guarantee.md", HEADROOM_MANDATORY_GUARANTEE),
    ("markitdown-mcp.md", MARKITDOWN_MCP),
    ("response-statusbar.md", RESPONSE_STATUSBAR),
    ("code-file-optimization.md", CODE_FILE_OPTIMIZATION),
];

const HEADROOM_PIPELINE: &str = r#"# Headroom + MarkItDown Pipeline — MANDATORY Global Rule

## Purpose

Automatically minimize context cost whenever binary or large content enters the conversation.
MarkItDown alone gives 60–80% reduction on binary files; Headroom adds up to 90% on verbose content.

## Pipeline

1. Convert binary files via markitdown_convert_file_optimized
2. Compress markdown via headroom_compress → hash + summary
3. Use headroom_retrieve(hash, query) for specific detail on demand

Always. No exceptions except trivial < 500 token text files.
"#;

const UNIFIED_OPTIMIZATION_PIPELINE: &str = r#"# Unified Token Optimization Pipeline — MANDATORY Global Rule

## Purpose

Coordinate RTK + MarkItDown + Headroom + Engram into a single automatic pipeline.
Target: 90–95% token reduction on every content entry point.

## Pipeline Layers

| Layer | Tool | Savings |
|-------|------|---------|
| 0 | RTK (hook, auto) | 60–90% on shell ops |
| 1 | MarkItDown | 60–80% on binary files |
| 2 | Headroom | +20–40% on top of layer 1 |
| 3 | Engram | Avoids re-explaining project history |
| 4 | Caveman | 40–60% on output tokens |
"#;

const HEADROOM_MANDATORY_GUARANTEE: &str = r#"# Headroom Proxy — Mandatory Always-On Guarantee

Headroom (port 8788) MUST be running for every Claude Code session.

- LaunchAgent `com.headroom.proxy.plist` — RunAtLoad=true, KeepAlive=true
- Env var `ANTHROPIC_BASE_URL=http://127.0.0.1:8788` set in settings.json

If the session appears to bypass Headroom, fix the proxy — do NOT disable ANTHROPIC_BASE_URL.
"#;

const MARKITDOWN_MCP: &str = r#"# MarkItDown MCP — Auto-convert non-MD files

You have a MarkItDown MCP server connected. ALWAYS use it for non-text files.

## Tools

| Tool | When to use |
|------|-------------|
| markitdown_convert_file | PDF, DOCX, PPTX, XLSX, images, audio, ZIP |
| markitdown_convert_file_optimized | Same but token-optimized (preferred for large files) |

## Rule

Before reading any non-.md/.txt file, call markitdown_convert_file first.
"#;

const RESPONSE_STATUSBAR: &str = r#"# Response Status Bar — MANDATORY Global Rule

Append a one-line status footer to EVERY response:

```
─ savings: {pct}% (pipeline) | ctx: ~{pct}% {bar}
```

- `savings`: exact % from headroom_compress this turn, or `—` if no pipeline ran
- `ctx`: estimated context window usage based on turn count
- Bar: ▓░ characters, 10 wide

Footer is NOT optional. Always present, always one line.
"#;

const CODE_FILE_OPTIMIZATION: &str = r#"# Code & Text File Optimization — MANDATORY Global Rule

## Purpose

Minimize context cost when reading source code and plain-text files.
Binary files are handled by MarkItDown. This rule covers .py, .ts, .js, .jsx, .tsx, .rs, .go,
.json, .yaml, .yml, .txt, .md files larger than ~200 lines.

## Pipeline by file type

### Code files (.py .ts .js .jsx .tsx .rs .go .rb .java .cpp .c)

NEVER read the full file first. Use Serena MCP in this order:
1. `get_symbols_overview` — get the full symbol tree (functions, classes, exports)
2. `find_declaration` or `find_symbol` — only if you need a specific symbol
3. `Read` full file — ONLY if you need implementation details not available via Serena

Savings: 70–90% fewer tokens vs reading the entire file.

### Large structured files (.json .yaml .yml > 100 lines)

1. Call `headroom_compress` on the content before processing
2. Use `headroom_retrieve` for specific keys on demand

### Plain text (.txt .md > 200 lines)

1. Call `headroom_compress` on the content
2. Use `headroom_retrieve` for specific sections on demand

## Hard rule

If a file is > 100 lines and is a code or structured text file:
- Do NOT call Read as the first action
- Use the appropriate tool above first
- Justify in one line if you skip this rule
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_rules_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("tokenwise_rules_{}", name))
    }

    /// test::connect_claude::creates_rules_dir_if_absent
    #[test]
    fn creates_rules_dir_if_absent() {
        let rules_dir = temp_rules_dir("create_dir");
        // Ensure it does not exist
        if rules_dir.exists() {
            fs::remove_dir_all(&rules_dir).unwrap();
        }

        let writer = RulesWriter;
        writer.write_all(&rules_dir).unwrap();

        assert!(rules_dir.exists(), "Rules directory must be created");
        assert!(rules_dir.is_dir(), "Rules path must be a directory");

        fs::remove_dir_all(&rules_dir).ok();
    }

    #[test]
    fn writes_all_six_rule_files() {
        let rules_dir = temp_rules_dir("six_files");
        let writer = RulesWriter;
        writer.write_all(&rules_dir).unwrap();

        let expected = [
            "headroom-pipeline.md",
            "unified-optimization-pipeline.md",
            "headroom-mandatory-guarantee.md",
            "markitdown-mcp.md",
            "response-statusbar.md",
            "code-file-optimization.md",
        ];

        for name in expected {
            let path = rules_dir.join(name);
            assert!(path.exists(), "Rule file must exist: {name}");
            let content = fs::read_to_string(&path).unwrap();
            assert!(!content.is_empty(), "Rule file must not be empty: {name}");
        }

        fs::remove_dir_all(&rules_dir).ok();
    }

    #[test]
    fn write_all_is_idempotent() {
        let rules_dir = temp_rules_dir("idempotent");
        let writer = RulesWriter;

        writer.write_all(&rules_dir).unwrap();
        let first_content = fs::read_to_string(rules_dir.join("headroom-pipeline.md")).unwrap();

        writer.write_all(&rules_dir).unwrap();
        let second_content = fs::read_to_string(rules_dir.join("headroom-pipeline.md")).unwrap();

        assert_eq!(first_content, second_content, "Idempotent: content must not change");

        fs::remove_dir_all(&rules_dir).ok();
    }

    #[test]
    fn rule_files_have_positive_size() {
        for (name, content) in RULE_FILES {
            assert!(
                content.len() > 10,
                "Rule file '{}' must have meaningful content (>10 chars)",
                name
            );
        }
    }
}
