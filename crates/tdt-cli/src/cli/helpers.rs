//! Shared helper functions for CLI commands
//!
//! This module contains utility functions that are used across multiple
//! command modules to avoid code duplication.

use chrono::{DateTime, Local, Utc};
use std::collections::HashSet;
use std::io::{self, BufRead, IsTerminal};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::EntityId;
use tdt_core::core::shortid::ShortIdIndex;

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
    // Count and slice by chars, not bytes: byte slicing panics mid-codepoint
    // on multibyte characters (Ø, ±, µ, ° are common in engineering titles).
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
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
        .map_while(|line| line.ok())
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

/// Read a single entity ID from stdin if available
///
/// Returns `Some(String)` with the first ID if stdin is piped (not a terminal),
/// or `None` if stdin is a terminal (interactive mode).
///
/// This enables patterns like:
/// ```bash
/// echo "REQ@1" | tdt req show
/// tdt req list --format id | head -1 | tdt req show
/// ```
///
/// Only reads the first line/ID from stdin.
pub fn read_single_id_from_stdin() -> Option<String> {
    let stdin = io::stdin();

    // Only read from stdin if it's piped (not a terminal)
    if stdin.is_terminal() {
        return None;
    }

    let mut line = String::new();
    if stdin.lock().read_line(&mut line).ok()? > 0 {
        let id = line.trim().to_string();
        if !id.is_empty() {
            return Some(id);
        }
    }

    None
}

/// Resolve an ID argument, falling back to stdin if the argument is empty or "-"
///
/// Returns the ID to use, or an error message if no ID is available.
///
/// This enables:
/// ```bash
/// tdt req show REQ@1           # explicit ID
/// echo "REQ@1" | tdt req show  # piped ID (no arg)
/// tdt req show -               # explicit stdin read
/// ```
pub fn resolve_id_arg(arg: &Option<String>) -> Result<String, &'static str> {
    match arg {
        Some(id) if id == "-" => {
            // Explicit stdin request
            read_single_id_from_stdin().ok_or("No ID provided on stdin")
        }
        Some(id) if !id.is_empty() => {
            // Explicit ID provided
            Ok(id.clone())
        }
        _ => {
            // No arg - try stdin
            read_single_id_from_stdin()
                .ok_or("No ID provided. Use: tdt <cmd> show <ID> or pipe an ID")
        }
    }
}

/// Resolve --linked-to args: comma-separated IDs, "-" for stdin, or auto-detect piped stdin.
/// Queries cache for all linked entity IDs and returns the allowed set.
pub fn resolve_linked_to(
    linked_to: &[String],
    via: Option<&str>,
    short_ids: &ShortIdIndex,
    cache: &EntityCache,
) -> Option<HashSet<String>> {
    if linked_to.is_empty() {
        return None;
    }

    // Handle "-" → read from stdin
    let raw_ids = if linked_to.len() == 1 && linked_to[0] == "-" {
        read_ids_from_stdin()?
    } else {
        linked_to.to_vec()
    };

    // Resolve short IDs to full IDs
    let resolved: Vec<String> = raw_ids
        .iter()
        .map(|id| short_ids.resolve(id).unwrap_or_else(|| id.clone()))
        .collect();

    Some(cache.get_ids_linked_to(&resolved, via))
}

/// Format a UTC datetime as local date only
///
/// Displays in user's local timezone as "YYYY-MM-DD"
pub fn format_date_local(dt: &DateTime<Utc>) -> String {
    let local: DateTime<Local> = dt.with_timezone(&Local);
    local.format("%Y-%m-%d").to_string()
}

/// Round a floating-point value to avoid floating-point artifacts
///
/// Determines precision from input tolerances and rounds to one more decimal place.
/// This prevents display issues like `-0.019999999999999574` showing instead of `-0.02`.
///
/// # Arguments
/// * `value` - The value to round
/// * `reference_precision` - A reference value (e.g., smallest tolerance) to determine precision
///
/// # Examples
/// ```
/// use tdt::cli::helpers::smart_round;
/// // If tolerance is 0.01, round to 3 decimal places (one more)
/// let result = smart_round(0.019999999999999574, 0.01);
/// assert!((result - 0.02).abs() < 1e-10);
/// ```
pub fn smart_round(value: f64, reference_precision: f64) -> f64 {
    let decimal_places = determine_decimal_places(reference_precision);
    round_to_places(value, decimal_places + 1)
}

/// Round a floating-point value to a specific number of decimal places
pub fn round_to_places(value: f64, decimal_places: u32) -> f64 {
    let multiplier = 10_f64.powi(decimal_places as i32);
    (value * multiplier).round() / multiplier
}

/// Determine the number of decimal places in a reference value
///
/// Returns the number of significant decimal places (max 6 to avoid floating-point issues).
fn determine_decimal_places(reference: f64) -> u32 {
    if reference == 0.0 {
        return 4; // Default to 4 decimal places
    }

    let abs_ref = reference.abs();

    // Check common engineering tolerances
    if abs_ref >= 1.0 {
        1
    } else if abs_ref >= 0.1 {
        2
    } else if abs_ref >= 0.01 {
        3
    } else if abs_ref >= 0.001 {
        4
    } else if abs_ref >= 0.0001 {
        5
    } else {
        6 // Max precision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdt_core::core::identity::EntityPrefix;

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
        assert_eq!(
            format_short_id_str("REQ-01J123456789ABCDEF123456"),
            "REQ-01J123456..."
        );
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

    #[test]
    fn test_round_to_places() {
        assert!((round_to_places(1.23456, 2) - 1.23).abs() < 1e-10);
        assert!((round_to_places(1.23456, 3) - 1.235).abs() < 1e-10);
        assert!((round_to_places(1.23456, 4) - 1.2346).abs() < 1e-10);
        assert!((round_to_places(-0.019999999999999574, 4) - (-0.02)).abs() < 1e-10);
    }

    #[test]
    fn test_smart_round() {
        // Tolerance of 0.01 -> 3 decimal places -> round to 4
        assert!((smart_round(0.019999999999999574, 0.01) - 0.02).abs() < 1e-10);
        assert!((smart_round(-0.019999999999999574, 0.01) - (-0.02)).abs() < 1e-10);

        // Tolerance of 0.1 -> 2 decimal places -> round to 3
        assert!((smart_round(0.1234567, 0.1) - 0.123).abs() < 1e-10);

        // Tolerance of 0.001 -> 4 decimal places -> round to 5
        assert!((smart_round(0.00123456789, 0.001) - 0.00123).abs() < 1e-10);
    }

    #[test]
    fn test_determine_decimal_places() {
        assert_eq!(determine_decimal_places(1.0), 1);
        assert_eq!(determine_decimal_places(0.5), 2); // 0.5 >= 0.1 -> 2 decimal places
        assert_eq!(determine_decimal_places(0.1), 2);
        assert_eq!(determine_decimal_places(0.05), 3); // 0.05 >= 0.01 -> 3 decimal places
        assert_eq!(determine_decimal_places(0.01), 3);
        assert_eq!(determine_decimal_places(0.001), 4);
        assert_eq!(determine_decimal_places(0.0), 4); // Default
    }

    #[test]
    fn test_markdown_table() {
        let mut table = MarkdownTable::new(vec!["ID", "Name", "Value"]);
        table.add_row(vec!["1", "Alpha", "100"]);
        table.add_row(vec!["2", "Beta", "2000"]);

        let output = table.render();
        assert!(output.contains("| ID | Name  | Value |"));
        assert!(output.contains("| 1  | Alpha | 100   |"));
        assert!(output.contains("| 2  | Beta  | 2000  |"));
    }

    #[test]
    fn test_markdown_table_empty() {
        let table = MarkdownTable::new(vec!["A", "B"]);
        assert!(table.is_empty());
    }
}
