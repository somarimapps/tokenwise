/// Severity level for structured output messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// Format a structured output message.
///
/// Produces: `[LEVEL] Message` or `[LEVEL] Message. Suggestion.`
///
/// The suggestion is appended only when `Some`.
pub fn format_message(level: Level, msg: &str, suggestion: Option<&str>) -> String {
    match suggestion {
        Some(s) => format!("[{}] {}. {}", level, msg, s),
        None => format!("[{}] {}", level, msg),
    }
}

/// Print a formatted message to stdout.
pub fn print_info(msg: &str) {
    println!("{}", format_message(Level::Info, msg, None));
}

/// Print a formatted warning to stderr.
pub fn print_warn(msg: &str, suggestion: Option<&str>) {
    eprintln!("{}", format_message(Level::Warn, msg, suggestion));
}

/// Print a formatted error to stderr.
pub fn print_error(msg: &str, suggestion: Option<&str>) {
    eprintln!("{}", format_message(Level::Error, msg, suggestion));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// test::cli::error_format_matches_pattern
    /// Regex `\[(ERROR|WARN|INFO)\] .+` must match all output lines.
    #[test]
    fn error_format_matches_pattern() {
        let cases = vec![
            format_message(Level::Error, "Something failed", None),
            format_message(Level::Warn, "Something suspicious", None),
            format_message(Level::Info, "All good", None),
        ];
        for msg in &cases {
            assert!(
                msg.starts_with("[ERROR]") || msg.starts_with("[WARN]") || msg.starts_with("[INFO]"),
                "Message does not match [LEVEL] pattern: {msg}"
            );
        }
    }

    #[test]
    fn format_with_suggestion_appends_suggestion() {
        let msg = format_message(
            Level::Warn,
            "Hook path not found: /tmp/rtk",
            Some("run 'tokenwise sync' to repair"),
        );
        assert!(msg.starts_with("[WARN]"));
        assert!(msg.contains("sync"), "Suggestion should appear in output");
    }

    #[test]
    fn format_without_suggestion_has_no_extra_dot() {
        let msg = format_message(Level::Info, "All checks passed", None);
        assert_eq!(msg, "[INFO] All checks passed");
    }

    #[test]
    fn all_levels_render_correctly() {
        assert!(format_message(Level::Info, "x", None).contains("[INFO]"));
        assert!(format_message(Level::Warn, "x", None).contains("[WARN]"));
        assert!(format_message(Level::Error, "x", None).contains("[ERROR]"));
    }
}
