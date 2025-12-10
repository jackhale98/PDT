//! Output formatting utilities

use crate::cli::OutputFormat;

/// Determine the effective output format based on context
pub fn effective_format(format: OutputFormat, is_list: bool) -> OutputFormat {
    match format {
        OutputFormat::Auto => {
            if is_list {
                OutputFormat::Tsv
            } else {
                OutputFormat::Yaml
            }
        }
        other => other,
    }
}
