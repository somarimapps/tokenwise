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

    // clawmem — semantic memory vault
    map.insert(
        "clawmem".to_string(),
        McpServerConfig::stdio("python3", vec!["-m".to_string(), "clawmem_mcp".to_string()]),
    );

    // engram — structured persistent memory
    map.insert(
        "engram".to_string(),
        McpServerConfig::stdio(
            "npx",
            vec![
                "-y".to_string(),
                "@anthropic/engram".to_string(),
                "mcp".to_string(),
            ],
        ),
    );

    // serena — code intelligence / LSP bridge
    map.insert(
        "serena".to_string(),
        McpServerConfig::stdio("python3", vec!["-m".to_string(), "serena".to_string()]),
    );

    // codebase-memory-mcp — graph-based code memory
    map.insert(
        "codebase-memory-mcp".to_string(),
        McpServerConfig::stdio("npx", vec!["-y".to_string(), "codebase-memory-mcp".to_string()]),
    );

    // mcp-registry — MCP registry / discovery
    map.insert(
        "mcp-registry".to_string(),
        McpServerConfig::stdio("npx", vec!["-y".to_string(), "mcp-registry-server".to_string()]),
    );

    map
}

/// Build the optional MCP server configurations (Odoo, Shopify, etc.).
/// These are not auto-installed — they require explicit user consent.
pub fn optional_mcp_servers() -> HashMap<String, McpServerConfig> {
    let mut map = HashMap::new();

    map.insert(
        "odoo-customext".to_string(),
        McpServerConfig::stdio("python3", vec!["-m".to_string(), "odoo_mcp".to_string()]),
    );

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
    fn core_servers_has_exactly_seven_entries() {
        assert_eq!(core_mcp_servers().len(), 7);
    }

    #[test]
    fn optional_servers_has_exactly_two_entries() {
        assert_eq!(optional_mcp_servers().len(), 2);
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
