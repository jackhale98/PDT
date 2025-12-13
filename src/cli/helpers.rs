//! Shared helper functions for CLI commands
//!
//! This module contains utility functions that are used across multiple
//! command modules to avoid code duplication.

use crate::core::identity::EntityId;
use std::io::{self, BufRead, IsTerminal};

/// Format an EntityId for display, truncating if too long
///
/// IDs longer than 16 characters are truncated to 13 chars with "..." suffix.
/// This provides a consistent display format across all list/table outputs.
pub fn format_short_id(id: &EntityId) -> String {
    let s = id.to_string();
    if s.len() > 16 {
        format!("{}...", &s[..13])
    } else {
        s
    }
}

/// Format a string ID for display, truncating if too long
///
/// Same behavior as format_short_id but works with &str instead of EntityId.
pub fn format_short_id_str(id: &str) -> String {
    if id.len() > 16 {
        format!("{}...", &id[..13])
    } else {
        id.to_string()
    }
}

/// Truncate a string to max_len, adding "..." if truncated
///
/// Useful for table columns that need fixed-width output.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Escape a string for CSV output
///
/// Handles commas, quotes, and newlines according to RFC 4180.
pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Read entity IDs from stdin if available (Unix philosophy support)
///
/// Returns `Some(Vec<String>)` with IDs if stdin is piped (not a terminal),
/// or `None` if stdin is a terminal (interactive mode).
///
/// This enables Unix-style pipelines like:
/// ```bash
/// tdt req list --format id | tdt bulk set-status approved
/// ```
///
/// IDs are read one per line, with empty lines and whitespace-only lines ignored.
pub fn read_ids_from_stdin() -> Option<Vec<String>> {
    let stdin = io::stdin();

    // Only read from stdin if it's piped (not a terminal)
    if stdin.is_terminal() {
        return None;
    }

    let ids: Vec<String> = stdin
        .lock()
        .lines()
        .filter_map(|line| line.ok())
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

/// Check if stdin has piped input available
///
/// Returns `true` if stdin is not a terminal (i.e., data is being piped in).
pub fn stdin_has_data() -> bool {
    !io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::identity::EntityPrefix;

    #[test]
    fn test_format_short_id() {
        let id = EntityId::new(EntityPrefix::Req);
        let formatted = format_short_id(&id);
        // ULID IDs are 30 chars (4 prefix + 1 dash + 26 ULID), so should truncate
        assert!(formatted.len() <= 16);
        assert!(formatted.ends_with("..."));
    }

    #[test]
    fn test_format_short_id_str() {
        assert_eq!(format_short_id_str("SHORT"), "SHORT");
        assert_eq!(format_short_id_str("REQ-01J123456789ABCDEF123456"), "REQ-01J123456...");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("hi", 2), "hi");
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("simple"), "simple");
        assert_eq!(escape_csv("with,comma"), "\"with,comma\"");
        assert_eq!(escape_csv("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(escape_csv("with\nnewline"), "\"with\nnewline\"");
    }
}
