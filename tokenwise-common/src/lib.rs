pub mod backup;
pub mod error;
pub mod exit_code;
pub mod output;
pub mod platform;

pub use backup::BackupManager;
pub use error::TokenwiseError;
pub use exit_code::ExitCode;
pub use output::{format_message, print_error, print_info, print_warn, Level};
pub use platform::Platform;

/// Names of the 6 core MCP servers that tokenwise manages.
pub const CORE_MCP_SERVER_NAMES: &[&str] = &[
    "markitdown",
    "headroom",
    "clawmem",
    "engram",
    "serena",
    "codebase-memory-mcp",
];

/// Names of the optional MCP servers (registered on request).
pub const OPTIONAL_MCP_SERVER_NAMES: &[&str] = &["odoo-customext", "shopify"];

/// Names of the 6 required rules files in `~/.claude/rules/`.
pub const REQUIRED_RULES_FILES: &[&str] = &[
    "headroom-pipeline.md",
    "unified-optimization-pipeline.md",
    "headroom-mandatory-guarantee.md",
    "markitdown-mcp.md",
    "response-statusbar.md",
    "code-file-optimization.md",
];
