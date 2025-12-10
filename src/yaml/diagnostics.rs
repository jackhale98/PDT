//! YAML error diagnostics with beautiful error messages

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// YAML syntax error with source location
#[derive(Debug, Error, Diagnostic)]
#[error("YAML syntax error")]
#[diagnostic(code(pdt::yaml::syntax))]
pub struct YamlSyntaxError {
    #[source_code]
    src: NamedSource<String>,

    #[label("error here")]
    span: SourceSpan,

    #[help]
    help: Option<String>,

    /// The underlying error message
    message: String,
}

impl YamlSyntaxError {
    /// Create a syntax error from a serde_yml error
    pub fn from_serde_error(err: &serde_yml::Error, source: &str, filename: &str) -> Self {
        let (line, column) = err
            .location()
            .map(|loc| (loc.line(), loc.column()))
            .unwrap_or((1, 1));

        let offset = line_col_to_offset(source, line, column);
        let message = err.to_string();
        let help = generate_help(&message);

        Self {
            src: NamedSource::new(filename, source.to_string()),
            span: SourceSpan::from(offset..offset.saturating_add(1)),
            help,
            message,
        }
    }

    /// Create a syntax error at a specific location
    pub fn at_location(
        message: impl Into<String>,
        source: &str,
        filename: &str,
        line: usize,
        column: usize,
        help: Option<String>,
    ) -> Self {
        let offset = line_col_to_offset(source, line, column);

        Self {
            src: NamedSource::new(filename, source.to_string()),
            span: SourceSpan::from(offset..offset.saturating_add(1)),
            help,
            message: message.into(),
        }
    }
}

/// Generic YAML error wrapper
#[derive(Debug, Error, Diagnostic)]
pub enum YamlError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Syntax(#[from] YamlSyntaxError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convert line/column to byte offset
fn line_col_to_offset(source: &str, line: usize, column: usize) -> usize {
    let mut offset = 0;
    let mut current_line = 1;

    for (i, ch) in source.char_indices() {
        if current_line == line {
            // Find the column within this line
            let line_start = i;
            let mut col = 1;
            for (j, c) in source[line_start..].char_indices() {
                if col == column {
                    return line_start + j;
                }
                if c == '\n' {
                    break;
                }
                col += 1;
            }
            return line_start + column.saturating_sub(1);
        }
        if ch == '\n' {
            current_line += 1;
        }
        offset = i;
    }

    offset
}

/// Generate helpful suggestions based on error message
fn generate_help(message: &str) -> Option<String> {
    let msg_lower = message.to_lowercase();

    if msg_lower.contains("expected ','") || msg_lower.contains("expected comma") {
        return Some("Add commas between list items: [item1, item2, item3]".to_string());
    }

    if msg_lower.contains("tab") {
        return Some(
            "YAML requires spaces for indentation, not tabs. Replace tabs with spaces.".to_string(),
        );
    }

    if msg_lower.contains("duplicate key") {
        return Some("Each key can only appear once. Remove or rename the duplicate key.".to_string());
    }

    if msg_lower.contains("expected block end") {
        return Some("Check your indentation - it may be inconsistent.".to_string());
    }

    if msg_lower.contains("mapping values are not allowed") {
        return Some("You may be missing a space after ':' or have incorrect indentation.".to_string());
    }

    if msg_lower.contains("found unexpected ':'") {
        return Some("Colons in values need to be quoted: \"value:with:colons\"".to_string());
    }

    if msg_lower.contains("@") || msg_lower.contains("special character") {
        return Some("Special characters like @ need to be quoted: \"@value\"".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_col_to_offset() {
        let source = "line1\nline2\nline3";
        assert_eq!(line_col_to_offset(source, 1, 1), 0);
        assert_eq!(line_col_to_offset(source, 2, 1), 6);
        assert_eq!(line_col_to_offset(source, 3, 1), 12);
    }

    #[test]
    fn test_help_generation() {
        assert!(generate_help("expected ','").is_some());
        assert!(generate_help("found tab character").is_some());
        assert!(generate_help("duplicate key").is_some());
        assert!(generate_help("some random error").is_none());
    }
}
