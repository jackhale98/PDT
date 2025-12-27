//! Table formatting utilities for CLI list commands
//!
//! This module provides a unified table output system that eliminates
//! duplicated formatting code across entity commands.
//!
//! # Text Wrapping
//!
//! The table formatter supports text wrapping for mobile-friendly display:
//! - Use `TableConfig::with_wrap(width)` to enable word-wrapped multi-line rows
//! - CSV, ID, and ShortId formats remain single-line for pipability
//! - TSV and Md formats support wrapped output

use chrono::{DateTime, Local, Utc};
use console::style;

use crate::cli::helpers::{escape_csv, truncate_str};
use crate::cli::OutputFormat;
use crate::core::entity::{Priority, Status};
use crate::core::shortid::ShortIdIndex;

/// Configuration for table output
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// Maximum width for text columns before wrapping (None = truncate instead)
    pub wrap_width: Option<usize>,
    /// Show summary line after table (e.g., "5 requirement(s) found")
    pub show_summary: bool,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            wrap_width: None,
            show_summary: true,
        }
    }
}

impl TableConfig {
    /// Create config with text wrapping enabled at the specified width
    pub fn with_wrap(width: usize) -> Self {
        Self {
            wrap_width: Some(width),
            show_summary: true,
        }
    }

    /// Create config optimized for piping (no wrapping, no summary)
    pub fn for_pipe() -> Self {
        Self {
            wrap_width: None,
            show_summary: false,
        }
    }
}

/// Wrap text to fit within a maximum width, breaking at word boundaries
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    // Don't wrap if text already fits or width is too small to be useful
    if text.len() <= max_width || max_width < 5 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            // First word on line
            if word.len() > max_width {
                // Word is longer than max width, force break
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current_line = remaining.to_string();
            } else {
                current_line = word.to_string();
            }
        } else if current_line.len() + 1 + word.len() <= max_width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            // Start new line
            lines.push(current_line);
            if word.len() > max_width {
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current_line = remaining.to_string();
            } else {
                current_line = word.to_string();
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// A typed cell value with semantic meaning for formatting
#[derive(Debug, Clone)]
pub enum CellValue {
    /// Entity ID (displayed with short ID prefix, cyan colored)
    Id(String),
    /// Short ID reference (e.g., "REQ@1", cyan colored)
    ShortId(String),
    /// Plain text, optionally truncated
    Text(String),
    /// Status with color coding
    Status(Status),
    /// Priority with color coding
    Priority(Priority),
    /// Optional priority (displays "-" if None)
    OptionalPriority(Option<Priority>),
    /// Entity type or category
    Type(String),
    /// Test result verdict with color coding (pass/fail/conditional/incomplete/not_applicable)
    Verdict(String),
    /// NCR severity with color coding (critical/major/minor)
    NcrSeverity(String),
    /// Fit result with color coding (clearance=green, interference=yellow, transition=magenta)
    FitResult(String),
    /// Fit match indicator (match=✓ green, mismatch=⚠ yellow, unknown=- dim)
    FitMatch(String),
    /// Analysis result with color coding (pass=green, marginal=yellow, fail=red)
    AnalysisResult(String),
    /// Cpk value with color coding (≥1.33=green, ≥1.0=yellow, <1.0=red)
    Cpk(Option<f64>),
    /// Yield percentage with color coding (≥99.73%=green, ≥95%=yellow, <95%=red)
    YieldPct(Option<f64>),
    /// Critical flag with color coding (yes=red bold, no=dim)
    Critical(bool),
    /// DateTime displayed as date only
    Date(DateTime<Utc>),
    /// DateTime displayed with time
    DateTime(DateTime<Utc>),
    /// Numeric value
    Number(i64),
    /// Float value with precision
    Float(f64, usize),
    /// Tags/labels as comma-separated
    Tags(Vec<String>),
    /// Empty/placeholder
    Empty,
}

impl CellValue {
    /// Format for TSV output (with colors if terminal)
    pub fn format_tsv(&self, width: usize) -> String {
        match self {
            CellValue::Id(id) => {
                let display = if id.len() > 16 {
                    format!("{}...", &id[..13])
                } else {
                    id.clone()
                };
                format!("{:<width$}", style(&display).cyan(), width = width)
            }
            CellValue::ShortId(sid) => {
                format!("{:<width$}", style(sid).cyan(), width = width)
            }
            CellValue::Text(s) => {
                let truncated = truncate_str(s, width.saturating_sub(2));
                format!("{:<width$}", truncated, width = width)
            }
            CellValue::Status(status) => {
                let s = status.to_string();
                let styled = match status {
                    Status::Draft => style(&s).dim(),
                    Status::Review => style(&s).yellow(),
                    Status::Approved => style(&s).green(),
                    Status::Released => style(&s).cyan().bold(),
                    Status::Obsolete => style(&s).red().dim(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::Priority(priority) => {
                let s = priority.to_string();
                let styled = match priority {
                    Priority::Low => style(&s).dim(),
                    Priority::Medium => style(&s).white(),
                    Priority::High => style(&s).yellow(),
                    Priority::Critical => style(&s).red().bold(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::OptionalPriority(opt) => match opt {
                Some(p) => CellValue::Priority(*p).format_tsv(width),
                None => format!("{:<width$}", "-", width = width),
            },
            CellValue::Type(t) => {
                format!(
                    "{:<width$}",
                    truncate_str(t, width.saturating_sub(2)),
                    width = width
                )
            }
            CellValue::Verdict(v) => {
                let display = if v.to_lowercase() == "not_applicable" {
                    "n/a".to_string()
                } else {
                    v.clone()
                };
                let styled = match v.to_lowercase().as_str() {
                    "pass" => style(&display).green(),
                    "fail" => style(&display).red().bold(),
                    "conditional" | "incomplete" => style(&display).yellow(),
                    "not_applicable" => style(&display).dim(),
                    _ => style(&display).white(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::NcrSeverity(s) => {
                let styled = match s.to_lowercase().as_str() {
                    "critical" => style(s).red().bold(),
                    "major" => style(s).yellow(),
                    _ => style(s).white(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::FitResult(s) => {
                let styled = match s.to_lowercase().as_str() {
                    "clearance" => style(s).green(),
                    "interference" => style(s).yellow(),
                    "transition" => style(s).magenta(),
                    _ => style(s).dim(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::FitMatch(s) => {
                let styled = match s.as_str() {
                    "match" => style("✓").green(),
                    "mismatch" => style("⚠").yellow(),
                    _ => style("-").dim(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::AnalysisResult(s) => {
                let styled = match s.to_lowercase().as_str() {
                    "pass" => style(s).green(),
                    "marginal" => style(s).yellow(),
                    "fail" => style(s).red(),
                    _ => style(s).dim(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::Cpk(opt) => {
                let (display, styled) = match opt {
                    Some(c) => {
                        let s = format!("{:.2}", c);
                        let styled = if *c >= 1.33 {
                            style(s).green()
                        } else if *c >= 1.0 {
                            style(s).yellow()
                        } else {
                            style(s).red()
                        };
                        (styled.to_string(), styled)
                    }
                    None => {
                        let s = "-".to_string();
                        (s.clone(), style(s).dim())
                    }
                };
                let _ = display; // Silence unused warning
                format!("{:<width$}", styled, width = width)
            }
            CellValue::YieldPct(opt) => {
                let styled = match opt {
                    Some(y) => {
                        let s = format!("{:.1}%", y);
                        if *y >= 99.73 {
                            style(s).green()
                        } else if *y >= 95.0 {
                            style(s).yellow()
                        } else {
                            style(s).red()
                        }
                    }
                    None => style("-".to_string()).dim(),
                };
                format!("{:<width$}", styled, width = width)
            }
            CellValue::Critical(is_critical) => {
                let (text, styled) = if *is_critical {
                    ("yes", style("yes").red().bold())
                } else {
                    ("no", style("no").dim())
                };
                let _ = text;
                format!("{:<width$}", styled, width = width)
            }
            CellValue::Date(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                format!("{:<width$}", local.format("%Y-%m-%d"), width = width)
            }
            CellValue::DateTime(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                format!("{:<width$}", local.format("%Y-%m-%d %H:%M"), width = width)
            }
            CellValue::Number(n) => {
                format!("{:>width$}", n, width = width)
            }
            CellValue::Float(f, precision) => {
                format!("{:>width$.prec$}", f, width = width, prec = precision)
            }
            CellValue::Tags(tags) => {
                let joined = tags.join(", ");
                format!(
                    "{:<width$}",
                    truncate_str(&joined, width.saturating_sub(2)),
                    width = width
                )
            }
            CellValue::Empty => format!("{:<width$}", "-", width = width),
        }
    }

    /// Format for CSV output (RFC 4180, no colors)
    pub fn format_csv(&self) -> String {
        match self {
            CellValue::Id(id) => escape_csv(id),
            CellValue::ShortId(sid) => escape_csv(sid),
            CellValue::Text(s) => escape_csv(s),
            CellValue::Status(status) => status.to_string(),
            CellValue::Priority(priority) => priority.to_string(),
            CellValue::OptionalPriority(opt) => opt.map_or(String::new(), |p| p.to_string()),
            CellValue::Type(t) => escape_csv(t),
            CellValue::Verdict(v) => escape_csv(v),
            CellValue::NcrSeverity(s) => escape_csv(s),
            CellValue::FitResult(s) => s.clone(),
            CellValue::FitMatch(s) => match s.as_str() {
                "match" => "Y".to_string(),
                "mismatch" => "N".to_string(),
                _ => "-".to_string(),
            },
            CellValue::AnalysisResult(s) => s.clone(),
            CellValue::Cpk(opt) => opt.map(|c| format!("{:.2}", c)).unwrap_or_default(),
            CellValue::YieldPct(opt) => opt.map(|y| format!("{:.1}", y)).unwrap_or_default(),
            CellValue::Critical(b) => {
                if *b {
                    "yes".to_string()
                } else {
                    "no".to_string()
                }
            }
            CellValue::Date(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                local.format("%Y-%m-%d").to_string()
            }
            CellValue::DateTime(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                local.format("%Y-%m-%dT%H:%M:%S").to_string()
            }
            CellValue::Number(n) => n.to_string(),
            CellValue::Float(f, precision) => format!("{:.prec$}", f, prec = precision),
            CellValue::Tags(tags) => escape_csv(&tags.join(", ")),
            CellValue::Empty => String::new(),
        }
    }

    /// Format for Markdown output (no colors, escaped pipes)
    pub fn format_md(&self) -> String {
        let raw = match self {
            CellValue::Id(id) => id.clone(),
            CellValue::ShortId(sid) => sid.clone(),
            CellValue::Text(s) => s.clone(),
            CellValue::Status(status) => status.to_string(),
            CellValue::Priority(priority) => priority.to_string(),
            CellValue::OptionalPriority(opt) => opt.map_or("-".to_string(), |p| p.to_string()),
            CellValue::Type(t) => t.clone(),
            CellValue::Verdict(v) => v.clone(),
            CellValue::NcrSeverity(s) => s.clone(),
            CellValue::Date(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                local.format("%Y-%m-%d").to_string()
            }
            CellValue::DateTime(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                local.format("%Y-%m-%d %H:%M").to_string()
            }
            CellValue::Number(n) => n.to_string(),
            CellValue::Float(f, precision) => format!("{:.prec$}", f, prec = precision),
            CellValue::Tags(tags) => tags.join(", "),
            CellValue::FitResult(s) => s.clone(),
            CellValue::FitMatch(s) => match s.as_str() {
                "match" => "✓".to_string(),
                "mismatch" => "⚠".to_string(),
                _ => "-".to_string(),
            },
            CellValue::AnalysisResult(s) => s.clone(),
            CellValue::Cpk(opt) => opt
                .map(|c| format!("{:.2}", c))
                .unwrap_or_else(|| "-".to_string()),
            CellValue::YieldPct(opt) => opt
                .map(|y| format!("{:.1}%", y))
                .unwrap_or_else(|| "-".to_string()),
            CellValue::Critical(b) => {
                if *b {
                    "**yes**".to_string()
                } else {
                    "no".to_string()
                }
            }
            CellValue::Empty => "-".to_string(),
        };
        // Escape pipe characters for markdown tables
        raw.replace('|', "\\|")
    }

    /// Get raw string value (no formatting, for ID output)
    pub fn raw(&self) -> String {
        match self {
            CellValue::Id(id) => id.clone(),
            CellValue::ShortId(sid) => sid.clone(),
            CellValue::Text(s) => s.clone(),
            CellValue::Status(status) => status.to_string(),
            CellValue::Priority(priority) => priority.to_string(),
            CellValue::OptionalPriority(opt) => opt.map_or(String::new(), |p| p.to_string()),
            CellValue::Type(t) => t.clone(),
            CellValue::Verdict(v) => v.clone(),
            CellValue::NcrSeverity(s) => s.clone(),
            CellValue::FitResult(s) => s.clone(),
            CellValue::FitMatch(s) => s.clone(),
            CellValue::AnalysisResult(s) => s.clone(),
            CellValue::Cpk(opt) => opt.map(|c| format!("{:.2}", c)).unwrap_or_default(),
            CellValue::YieldPct(opt) => opt.map(|y| format!("{:.1}%", y)).unwrap_or_default(),
            CellValue::Critical(b) => {
                if *b {
                    "yes".to_string()
                } else {
                    "no".to_string()
                }
            }
            CellValue::Date(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                local.format("%Y-%m-%d").to_string()
            }
            CellValue::DateTime(dt) => {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                local.format("%Y-%m-%dT%H:%M:%S").to_string()
            }
            CellValue::Number(n) => n.to_string(),
            CellValue::Float(f, precision) => format!("{:.prec$}", f, prec = precision),
            CellValue::Tags(tags) => tags.join(", "),
            CellValue::Empty => String::new(),
        }
    }

    /// Get the display width of this cell's content (for dynamic column sizing)
    pub fn display_width(&self) -> usize {
        match self {
            CellValue::Id(id) => id.len().min(16), // IDs are truncated to 16
            CellValue::ShortId(sid) => sid.len(),
            CellValue::Text(s) => s.len(),
            CellValue::Status(status) => status.to_string().len(),
            CellValue::Priority(priority) => priority.to_string().len(),
            CellValue::OptionalPriority(opt) => opt.map_or(1, |p| p.to_string().len()),
            CellValue::Type(t) => t.len(),
            CellValue::Verdict(v) => v.len().max(3), // "n/a" minimum
            CellValue::NcrSeverity(s) => s.len(),
            CellValue::FitResult(s) => s.len(),
            CellValue::FitMatch(_) => 2, // "✓" or "⚠" or "-"
            CellValue::AnalysisResult(s) => s.len(),
            CellValue::Cpk(opt) => opt.map_or(1, |c| format!("{:.2}", c).len()),
            CellValue::YieldPct(opt) => opt.map_or(1, |y| format!("{:.1}%", y).len()),
            CellValue::Critical(_) => 3,  // "yes" or "no"
            CellValue::Date(_) => 10,     // "YYYY-MM-DD"
            CellValue::DateTime(_) => 16, // "YYYY-MM-DD HH:MM"
            CellValue::Number(n) => n.to_string().len(),
            CellValue::Float(f, precision) => format!("{:.prec$}", f, prec = precision).len(),
            CellValue::Tags(tags) => tags.join(", ").len(),
            CellValue::Empty => 1,
        }
    }
}

/// Column definition with header label and width
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub key: &'static str,
    pub header: &'static str,
    pub width: usize,
}

impl ColumnDef {
    pub const fn new(key: &'static str, header: &'static str, width: usize) -> Self {
        Self { key, header, width }
    }
}

/// A row of cell values for table output
pub struct TableRow {
    pub short_id: String,
    pub full_id: String,
    pub cells: Vec<(&'static str, CellValue)>,
}

impl TableRow {
    pub fn new(full_id: String, short_ids: &ShortIdIndex) -> Self {
        let short_id = short_ids.get_short_id(&full_id).unwrap_or_default();
        Self {
            short_id,
            full_id,
            cells: Vec::new(),
        }
    }

    pub fn cell(mut self, key: &'static str, value: CellValue) -> Self {
        self.cells.push((key, value));
        self
    }

    pub fn get(&self, key: &str) -> Option<&CellValue> {
        self.cells.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
}

/// Table formatter that outputs rows in various formats
pub struct TableFormatter<'a> {
    columns: &'a [ColumnDef],
    entity_name: &'static str,
    entity_name_plural: &'static str,
    entity_prefix: &'static str,
    config: TableConfig,
}

impl<'a> TableFormatter<'a> {
    pub fn new(
        columns: &'a [ColumnDef],
        entity_name: &'static str,
        entity_prefix: &'static str,
    ) -> Self {
        Self {
            columns,
            entity_name,
            entity_name_plural: entity_name, // Default to same name with (es) suffix
            entity_prefix,
            config: TableConfig::default(),
        }
    }

    /// Set a custom plural name (e.g., "processes" instead of "process(es)")
    pub fn with_plural(mut self, plural: &'static str) -> Self {
        self.entity_name_plural = plural;
        self
    }

    /// Configure the formatter with custom settings
    pub fn with_config(mut self, config: TableConfig) -> Self {
        self.config = config;
        self
    }

    /// Output rows in the specified format
    pub fn output<I>(&self, rows: I, format: OutputFormat, visible_columns: &[&str])
    where
        I: IntoIterator<Item = TableRow>,
    {
        let rows: Vec<TableRow> = rows.into_iter().collect();

        match format {
            OutputFormat::Tsv => self.output_tsv(&rows, visible_columns),
            OutputFormat::Csv => self.output_csv(&rows, visible_columns),
            OutputFormat::Md => self.output_md(&rows, visible_columns),
            OutputFormat::Id => self.output_ids(&rows, false),
            OutputFormat::ShortId => self.output_ids(&rows, true),
            _ => self.output_tsv(&rows, visible_columns),
        }
    }

    /// Calculate dynamic column widths based on actual content
    fn calculate_widths(&self, rows: &[TableRow], visible_columns: &[&str]) -> Vec<usize> {
        let mut widths = Vec::new();

        // SHORT column - find max short ID length, min 5 for header
        let short_width = rows
            .iter()
            .map(|r| r.short_id.len())
            .max()
            .unwrap_or(5)
            .max(5); // "SHORT" header
        widths.push(short_width);

        // Other columns
        for col in self.columns {
            if visible_columns.contains(&col.key) {
                let header_len = col.header.len();
                let max_content = rows
                    .iter()
                    .filter_map(|r| r.get(col.key))
                    .map(|v| v.display_width())
                    .max()
                    .unwrap_or(0);

                // Need +2 for truncation buffer (truncate_str uses width-2 for text)
                // Auto-size: use max of (header, content+2), but cap at col.width
                // if content is longer (prevents excessive expansion)
                let content_with_buffer = max_content.saturating_add(2);
                let natural_width = header_len.max(content_with_buffer);
                // Cap at defined width to prevent excessive expansion, but allow shrinking
                let width = natural_width.min(col.width);
                widths.push(width);
            }
        }

        widths
    }

    fn output_tsv(&self, rows: &[TableRow], visible_columns: &[&str]) {
        // Calculate dynamic widths based on content
        let widths = self.calculate_widths(rows, visible_columns);

        // Header row - always start with SHORT
        let mut header_parts = vec![format!(
            "{:<width$}",
            style("SHORT").bold().dim(),
            width = widths[0]
        )];
        let mut width_idx = 1;

        for col in self.columns {
            if visible_columns.contains(&col.key) {
                header_parts.push(format!(
                    "{:<width$}",
                    style(col.header).bold(),
                    width = widths[width_idx]
                ));
                width_idx += 1;
            }
        }
        println!("{}", header_parts.join(" "));

        // Separator
        let total_width: usize = widths.iter().sum::<usize>() + widths.len() - 1;
        println!("{}", "-".repeat(total_width));

        // Data rows
        for row in rows {
            if let Some(wrap_width) = self.config.wrap_width {
                self.output_tsv_row_wrapped(row, visible_columns, &widths, wrap_width);
            } else {
                self.output_tsv_row_truncated(row, visible_columns, &widths);
            }
        }

        // Summary (unless disabled for piping)
        if self.config.show_summary {
            println!();
            println!(
                "{} {}(s) found. Use {} to reference by short ID.",
                style(rows.len()).cyan(),
                self.entity_name,
                style(format!("{}@N", self.entity_prefix)).cyan()
            );
        }
    }

    fn output_tsv_row_truncated(&self, row: &TableRow, visible_columns: &[&str], widths: &[usize]) {
        let mut row_parts = vec![format!(
            "{:<width$}",
            style(&row.short_id).cyan(),
            width = widths[0]
        )];
        let mut width_idx = 1;

        for col in self.columns {
            if visible_columns.contains(&col.key) {
                let w = widths[width_idx];
                if let Some(value) = row.get(col.key) {
                    row_parts.push(value.format_tsv(w));
                } else {
                    row_parts.push(format!("{:<width$}", "-", width = w));
                }
                width_idx += 1;
            }
        }
        println!("{}", row_parts.join(" "));
    }

    fn output_tsv_row_wrapped(
        &self,
        row: &TableRow,
        visible_columns: &[&str],
        widths: &[usize],
        wrap_width: usize,
    ) {
        // Collect all cell values and their wrapped lines
        let mut wrapped_cells: Vec<Vec<String>> = Vec::new();

        // Short ID (first column, never wrapped)
        wrapped_cells.push(vec![row.short_id.clone()]);

        for col in self.columns {
            if visible_columns.contains(&col.key) {
                if let Some(value) = row.get(col.key) {
                    let raw = value.raw();
                    // Only wrap Text and Tags columns
                    let lines = match value {
                        CellValue::Text(_) | CellValue::Tags(_) => {
                            // Use wrap_width directly - user controls wrap point
                            wrap_text(&raw, wrap_width)
                        }
                        _ => vec![raw],
                    };
                    wrapped_cells.push(lines);
                } else {
                    wrapped_cells.push(vec!["-".to_string()]);
                }
            }
        }

        // Find the maximum number of lines needed
        let max_lines = wrapped_cells.iter().map(|c| c.len()).max().unwrap_or(1);

        // Output each line
        for line_idx in 0..max_lines {
            let mut row_parts = Vec::new();

            for (col_idx, cell_lines) in wrapped_cells.iter().enumerate() {
                let width = *widths.get(col_idx).unwrap_or(&10);
                let content = cell_lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");

                if col_idx == 0 {
                    // Short ID column - only show on first line
                    if line_idx == 0 {
                        row_parts.push(format!("{:<8}", style(content).cyan()));
                    } else {
                        row_parts.push(format!("{:<8}", ""));
                    }
                } else {
                    row_parts.push(format!("{:<width$}", content, width = width));
                }
            }
            println!("{}", row_parts.join(" "));
        }

        // Add blank line between multi-line rows for readability
        if max_lines > 1 {
            println!();
        }
    }

    fn output_csv(&self, rows: &[TableRow], visible_columns: &[&str]) {
        // Header row
        let mut headers = vec!["short_id".to_string(), "id".to_string()];
        for col in self.columns {
            if visible_columns.contains(&col.key) {
                headers.push(col.key.to_string());
            }
        }
        println!("{}", headers.join(","));

        // Data rows
        for row in rows {
            let mut values = vec![escape_csv(&row.short_id), escape_csv(&row.full_id)];
            for col in self.columns {
                if visible_columns.contains(&col.key) {
                    if let Some(value) = row.get(col.key) {
                        values.push(value.format_csv());
                    } else {
                        values.push(String::new());
                    }
                }
            }
            println!("{}", values.join(","));
        }
    }

    fn output_md(&self, rows: &[TableRow], visible_columns: &[&str]) {
        // Header row
        let mut headers = vec!["Short".to_string(), "ID".to_string()];
        for col in self.columns {
            if visible_columns.contains(&col.key) {
                headers.push(col.header.to_string());
            }
        }
        println!("| {} |", headers.join(" | "));

        // Separator
        let separators: Vec<&str> = headers.iter().map(|_| "---").collect();
        println!("|{}|", separators.join("|"));

        // Data rows
        for row in rows {
            let mut values = vec![row.short_id.clone(), row.full_id.clone()];
            for col in self.columns {
                if visible_columns.contains(&col.key) {
                    if let Some(value) = row.get(col.key) {
                        values.push(value.format_md());
                    } else {
                        values.push("-".to_string());
                    }
                }
            }
            println!("| {} |", values.join(" | "));
        }
    }

    fn output_ids(&self, rows: &[TableRow], use_short: bool) {
        for row in rows {
            if use_short {
                println!("{}", row.short_id);
            } else {
                println!("{}", row.full_id);
            }
        }
    }
}

/// Convert a list of column keys to their string representations
/// for use with TableFormatter::output
pub fn columns_to_keys<C: std::fmt::Display>(columns: &[C]) -> Vec<&'static str> {
    columns
        .iter()
        .filter_map(|c| {
            // Leak the string to get a static lifetime
            // This is acceptable because column names are fixed at compile time
            // and we only have a small number of them
            let s = c.to_string();
            Some(Box::leak(s.into_boxed_str()) as &'static str)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_value_text_format() {
        let cell = CellValue::Text("Hello World".to_string());
        let tsv = cell.format_tsv(20);
        assert!(tsv.contains("Hello World"));

        let csv = cell.format_csv();
        assert_eq!(csv, "Hello World");

        let md = cell.format_md();
        assert_eq!(md, "Hello World");
    }

    #[test]
    fn test_cell_value_status_format() {
        let cell = CellValue::Status(Status::Approved);
        let csv = cell.format_csv();
        assert_eq!(csv, "approved");

        let md = cell.format_md();
        assert_eq!(md, "approved");
    }

    #[test]
    fn test_cell_value_priority_format() {
        let cell = CellValue::Priority(Priority::Critical);
        let csv = cell.format_csv();
        assert_eq!(csv, "critical");
    }

    #[test]
    fn test_cell_value_optional_priority() {
        let some = CellValue::OptionalPriority(Some(Priority::High));
        assert_eq!(some.format_csv(), "high");

        let none = CellValue::OptionalPriority(None);
        assert_eq!(none.format_csv(), "");
        assert_eq!(none.format_md(), "-");
    }

    #[test]
    fn test_cell_value_tags() {
        let cell = CellValue::Tags(vec!["foo".to_string(), "bar".to_string()]);
        assert_eq!(cell.format_csv(), "\"foo, bar\"");
        assert_eq!(cell.format_md(), "foo, bar");
    }

    #[test]
    fn test_cell_value_md_escapes_pipes() {
        let cell = CellValue::Text("a|b|c".to_string());
        assert_eq!(cell.format_md(), "a\\|b\\|c");
    }

    #[test]
    fn test_column_def() {
        let col = ColumnDef::new("title", "TITLE", 30);
        assert_eq!(col.key, "title");
        assert_eq!(col.header, "TITLE");
        assert_eq!(col.width, 30);
    }

    #[test]
    fn test_table_row_builder() {
        let short_ids = ShortIdIndex::default();
        let row = TableRow::new("REQ-123".to_string(), &short_ids)
            .cell("title", CellValue::Text("My Title".to_string()))
            .cell("status", CellValue::Status(Status::Draft));

        assert_eq!(row.full_id, "REQ-123");
        assert!(row.get("title").is_some());
        assert!(row.get("status").is_some());
        assert!(row.get("missing").is_none());
    }

    #[test]
    fn test_wrap_text_short() {
        let result = wrap_text("hello", 20);
        assert_eq!(result, vec!["hello"]);
    }

    #[test]
    fn test_wrap_text_exact_fit() {
        let result = wrap_text("hello world", 11);
        assert_eq!(result, vec!["hello world"]);
    }

    #[test]
    fn test_wrap_text_word_boundary() {
        let result = wrap_text("hello world foo bar", 11);
        assert_eq!(result, vec!["hello world", "foo bar"]);
    }

    #[test]
    fn test_wrap_text_long_word() {
        let result = wrap_text("supercalifragilisticexpialidocious", 10);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], "supercalif");
        assert_eq!(result[1], "ragilistic");
        assert_eq!(result[2], "expialidoc");
        assert_eq!(result[3], "ious");
    }

    #[test]
    fn test_wrap_text_multiple_lines() {
        let text = "The quick brown fox jumps over the lazy dog";
        let result = wrap_text(text, 15);
        assert_eq!(
            result,
            vec!["The quick brown", "fox jumps over", "the lazy dog"]
        );
    }

    #[test]
    fn test_table_config_default() {
        let config = TableConfig::default();
        assert!(config.wrap_width.is_none());
        assert!(config.show_summary);
    }

    #[test]
    fn test_table_config_with_wrap() {
        let config = TableConfig::with_wrap(40);
        assert_eq!(config.wrap_width, Some(40));
        assert!(config.show_summary);
    }

    #[test]
    fn test_table_config_for_pipe() {
        let config = TableConfig::for_pipe();
        assert!(config.wrap_width.is_none());
        assert!(!config.show_summary);
    }
}
