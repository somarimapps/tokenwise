use std::collections::HashMap;

use tokenwise_core::settings::models::McpServerConfig;

/// Build the full map of 7 core MCP server configurations.
///
/// These match the exact server names in `CORE_MCP_SERVER_NAMES` and represent
/// the standard Claude Code + Hermes agent optimization stack.
pub fn core_mcp_servers() -> HashMap<String, McpServerConfig> {
    let mut map = HashMap::new();

    // markitdown — binary-to-markdown converter
    map.insert(
        "markitdown".to_string(),
        McpServerConfig::stdio("python3", vec!["-m".to_string(), "markitdown_mcp".to_string()]),
    );

    // headroom — token compression proxy
    map.insert(
        "headroom".to_string(),
        McpServerConfig::stdio("headroom", vec!["mcp".to_string()]),
    );

    // clawmem — semantic memory vault (Node/bun binary, installed via npm/bun)
    map.insert(
        "clawmem".to_string(),
        McpServerConfig::stdio("clawmem", vec!["mcp".to_string()]),
    );

    // serena — code intelligence / LSP bridge (installed via uvx from GitHub)
    map.insert(
        "serena".to_string(),
        McpServerConfig::stdio(
            "uvx",
            vec![
                "--from".to_string(),
                "git+https://github.com/oraios/serena".to_string(),
                "serena".to_string(),
                "start-mcp-server".to_string(),
                "--project-from-cwd".to_string(),
                "--context".to_string(),
                "claude-code".to_string(),
                "--open-web-dashboard".to_string(),
                "False".to_string(),
            ],
        ),
    );

    // codebase-memory-mcp — graph-based code memory
    map.insert(
        "codebase-memory-mcp".to_string(),
        McpServerConfig::stdio("npx", vec!["-y".to_string(), "codebase-memory-mcp".to_string()]),
    );

    map
}

/// Build the optional MCP server configurations.
/// These are not auto-installed — they require explicit user consent.
/// User-specific servers (e.g. private Odoo instances) must be added manually.
pub fn optional_mcp_servers() -> HashMap<String, McpServerConfig> {
    let mut map = HashMap::new();

    map.insert(
        "shopify".to_string(),
        McpServerConfig::stdio("npx", vec!["-y".to_string(), "@shopify/mcp-server".to_string()]),
    );

    map
}

/// Headroom proxy URL (the ANTHROPIC_BASE_URL value).
pub const HEADROOM_BASE_URL: &str = "http://127.0.0.1:8788";

/// Headroom LaunchAgent label on macOS.
pub const HEADROOM_LAUNCHD_LABEL: &str = "com.headroom.proxy";

#[cfg(test)]
mod tests {
    use super::*;
    use tokenwise_common::CORE_MCP_SERVER_NAMES;

    #[test]
    fn core_servers_covers_all_core_names() {
        let servers = core_mcp_servers();
        for name in CORE_MCP_SERVER_NAMES {
            assert!(
                servers.contains_key(*name),
                "core_mcp_servers() is missing '{}'",
                name
            );
        }
    }

    #[test]
    fn core_servers_has_exactly_five_entries() {
        assert_eq!(core_mcp_servers().len(), 5);
    }

    #[test]
    fn optional_servers_has_exactly_one_entry() {
        assert_eq!(optional_mcp_servers().len(), 1);
    }

    #[test]
    fn all_core_servers_have_non_empty_command() {
        for (name, cfg) in core_mcp_servers() {
            assert!(
                !cfg.command.is_empty(),
                "Server '{}' has empty command",
                name
            );
        }
    }

    #[test]
    fn headroom_base_url_uses_correct_port() {
        assert!(HEADROOM_BASE_URL.contains("8788"));
    }
}
