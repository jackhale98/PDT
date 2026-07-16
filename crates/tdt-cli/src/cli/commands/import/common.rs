//! Common utilities for CSV import

use console::style;
use csv::StringRecord;
use miette::Result;
use std::collections::HashMap;

use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::shortid::ShortIdIndex;

/// Import options passed to entity-specific import functions
#[derive(Debug)]
pub struct ImportArgs {
    pub dry_run: bool,
    pub skip_errors: bool,
    /// Default component ID for feature/quote imports
    pub component: Option<String>,
    /// Default supplier ID for quote imports
    pub supplier: Option<String>,
    /// Default test ID for result imports
    pub test: Option<String>,
    /// Default process ID for control imports
    pub process: Option<String>,
    /// Default assembly ID for component imports
    pub assembly: Option<String>,
}

/// Truncate a string to max length (in characters) with ellipsis
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Resolve a short ID (e.g. "CMP@1") or full entity ID to a validated full ID.
/// Returns Err with a message when the reference is not a resolvable entity ID.
pub fn resolve_entity_ref(short_ids: &ShortIdIndex, reference: &str) -> Result<String, String> {
    let resolved = short_ids
        .resolve(reference)
        .unwrap_or_else(|| reference.to_string());
    resolved
        .parse::<EntityId>()
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_handles_multibyte_utf8() {
        // Multibyte characters: byte-based slicing would panic here
        let s = "日本語のテキストです。";
        assert_eq!(truncate(s, 8), "日本語のテ...");
        // Short strings are returned unchanged
        assert_eq!(truncate("héllo", 10), "héllo");
        // ASCII truncation still works
        assert_eq!(truncate("abcdefghij", 8), "abcde...");
    }
}

/// Import statistics
#[derive(Default)]
pub struct ImportStats {
    pub rows_processed: usize,
    pub entities_created: usize,
    pub entities_updated: usize,
    pub errors: usize,
    pub skipped: usize,
}

/// Build a map from header name to column index
///
/// Warns when duplicate headers are present (the last occurrence wins).
pub fn build_header_map(headers: &StringRecord) -> HashMap<String, usize> {
    let mut map: HashMap<String, usize> = HashMap::new();
    let mut duplicates: Vec<String> = Vec::new();

    for (i, h) in headers.iter().enumerate() {
        let key = h.to_lowercase().trim().to_string();
        if map.insert(key.clone(), i).is_some() && !duplicates.contains(&key) {
            duplicates.push(key);
        }
    }

    if !duplicates.is_empty() {
        eprintln!(
            "{} Duplicate CSV header(s): {} (last occurrence wins)",
            style("!").yellow(),
            duplicates.join(", ")
        );
    }

    map
}

/// Get a field value from a CSV record
pub fn get_field(
    record: &StringRecord,
    header_map: &HashMap<String, usize>,
    field: &str,
) -> Option<String> {
    header_map
        .get(field)
        .and_then(|&idx| record.get(idx))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Generate a CSV template for an entity type
pub fn generate_template(entity_type: EntityPrefix) -> Result<()> {
    let headers = get_csv_headers(entity_type);
    let example = get_csv_example(entity_type);

    // Output to stdout (can be redirected to file)
    println!("{}", headers.join(","));
    if !example.is_empty() {
        println!("{}", example.join(","));
    }

    // Print usage hint to stderr so it doesn't interfere with redirected output
    eprintln!();
    eprintln!(
        "{} Template generated. Redirect to file: tdt import --template {} > {}.csv",
        style("→").blue(),
        entity_type.as_str().to_lowercase(),
        entity_type.as_str().to_lowercase()
    );

    Ok(())
}

/// Get CSV headers for an entity type
pub fn get_csv_headers(entity_type: EntityPrefix) -> Vec<&'static str> {
    match entity_type {
        EntityPrefix::Req => vec![
            "title",
            "type",
            "priority",
            "status",
            "text",
            "rationale",
            "tags",
        ],
        EntityPrefix::Risk => vec![
            "title",
            "type",
            "description",
            "failure_mode",
            "cause",
            "effect",
            "severity",
            "occurrence",
            "detection",
            "tags",
        ],
        EntityPrefix::Cmp => vec![
            "assembly",
            "part_number",
            "title",
            "make_buy",
            "category",
            "description",
            "material",
            "finish",
            "mass",
            "cost",
            "tags",
        ],
        EntityPrefix::Asm => vec!["part_number", "title", "description", "parent", "tags"],
        EntityPrefix::Sup => vec![
            "short_name",
            "title",
            "website",
            "contact_email",
            "contact_phone",
            "address",
            "lead_time_days",
            "tags",
        ],
        EntityPrefix::Quot => vec![
            "title",
            "supplier",
            "component",
            "currency",
            "unit_price",
            "lead_time_days",
            "moq",
            "description",
            "tags",
        ],
        EntityPrefix::Test => vec![
            "title",
            "type",
            "level",
            "method",
            "category",
            "priority",
            "objective",
            "description",
            "estimated_duration",
            "tags",
        ],
        EntityPrefix::Rslt => vec![
            "test",
            "verdict",
            "executed_by",
            "executed_date",
            "description",
            "notes",
            "tags",
        ],
        EntityPrefix::Proc => vec![
            "title",
            "type",
            "operation_number",
            "description",
            "cycle_time_minutes",
            "setup_time_minutes",
            "operator_skill",
            "tags",
        ],
        EntityPrefix::Ctrl => vec![
            "process",
            "title",
            "type",
            "category",
            "description",
            "characteristic_name",
            "nominal",
            "upper_limit",
            "lower_limit",
            "units",
            "critical",
            "tags",
        ],
        EntityPrefix::Ncr => vec![
            "title",
            "type",
            "severity",
            "category",
            "description",
            "part_number",
            "quantity_affected",
            "characteristic",
            "specification",
            "actual",
            "tags",
        ],
        EntityPrefix::Capa => vec![
            "title",
            "type",
            "source_type",
            "source_ref",
            "problem_statement",
            "root_cause",
            "tags",
        ],
        EntityPrefix::Feat => vec![
            "component",
            "title",
            "feature_type",
            "nominal",
            "plus_tolerance",
            "minus_tolerance",
            "units",
            "datum",
            "critical",
            "description",
            "tags",
        ],
        _ => vec!["title", "description", "tags"],
    }
}

/// Get example CSV row for an entity type
pub fn get_csv_example(entity_type: EntityPrefix) -> Vec<&'static str> {
    match entity_type {
        EntityPrefix::Req => vec![
            "\"Stroke Length\"",
            "input",
            "critical",
            "draft",
            "\"The actuator shall have a minimum stroke length of 100mm\"",
            "\"Required for full range of motion\"",
            "\"mechanical,critical\"",
        ],
        EntityPrefix::Risk => vec![
            "\"Seal Failure\"",
            "design",
            "\"O-ring may fail under pressure\"",
            "\"Seal extrusion\"",
            "\"Excessive pressure differential\"",
            "\"Fluid leakage and system failure\"",
            "8",
            "4",
            "6",
            "\"seal,pressure\"",
        ],
        EntityPrefix::Cmp => vec![
            "\"ASM@1\"",
            "\"PN-001\"",
            "\"Housing Assembly\"",
            "make",
            "mechanical",
            "\"Main structural housing\"",
            "\"6061-T6 Aluminum\"",
            "\"Anodize\"",
            "0.5",
            "125.00",
            "\"structural,machined\"",
        ],
        EntityPrefix::Asm => vec![
            "\"ASM-001\"",
            "\"Actuator Assembly\"",
            "\"Main actuator assembly with housing and internals\"",
            "",
            "\"assembly,mechanical\"",
        ],
        EntityPrefix::Sup => vec![
            "\"ACME\"",
            "\"ACME Manufacturing Co.\"",
            "\"https://acme.example.com\"",
            "\"sales@acme.example.com\"",
            "\"+1-555-123-4567\"",
            "\"123 Industrial Way, City, ST 12345\"",
            "14",
            "\"machining,precision\"",
        ],
        EntityPrefix::Quot => vec![
            "\"Housing Quote - Acme\"",
            "\"SUP@1\"",
            "\"CMP@1\"",
            "USD",
            "125.00",
            "14",
            "100",
            "\"Quote for housing assembly\"",
            "\"machining\"",
        ],
        EntityPrefix::Test => vec![
            "\"Housing Dimensional Inspection\"",
            "verification",
            "unit",
            "inspection",
            "\"mechanical\"",
            "high",
            "\"Verify housing dimensions meet specification\"",
            "\"Measure critical dimensions of machined housing\"",
            "\"30 min\"",
            "\"verification,dimensional\"",
        ],
        EntityPrefix::Rslt => vec![
            "\"TEST@1\"",
            "pass",
            "\"John Smith\"",
            "2024-01-15",
            "\"All dimensions within tolerance\"",
            "\"See attached measurement report\"",
            "\"verification\"",
        ],
        EntityPrefix::Proc => vec![
            "\"CNC Rough Machining\"",
            "machining",
            "\"OP-010\"",
            "\"Initial rough machining of housing blank\"",
            "45",
            "30",
            "intermediate",
            "\"machining,cnc\"",
        ],
        EntityPrefix::Ctrl => vec![
            "\"PROC@1\"",
            "\"Bore Diameter Check\"",
            "inspection",
            "variable",
            "\"In-process check of bore diameter\"",
            "\"Bore Diameter\"",
            "25.0",
            "25.02",
            "24.98",
            "mm",
            "true",
            "\"dimensional,critical\"",
        ],
        EntityPrefix::Ncr => vec![
            "\"Out-of-spec bore diameter\"",
            "internal",
            "minor",
            "dimensional",
            "\"Bore diameter measured outside tolerance\"",
            "\"PN-001\"",
            "5",
            "\"Bore Diameter\"",
            "\"25.0 +/- 0.02mm\"",
            "\"25.05mm\"",
            "\"dimensional,machining\"",
        ],
        EntityPrefix::Capa => vec![
            "\"Improve bore machining process\"",
            "corrective",
            "ncr",
            "\"NCR@1\"",
            "\"Recurring out-of-spec bore diameters\"",
            "\"Tool wear not being monitored\"",
            "\"machining,process\"",
        ],
        EntityPrefix::Feat => vec![
            "\"CMP@1\"",
            "\"Bore Diameter\"",
            "internal",
            "25.0",
            "0.025",
            "-0.025",
            "mm",
            "\"A\"",
            "true",
            "\"Main bearing bore\"",
            "\"critical,dimensional\"",
        ],
        _ => vec![],
    }
}
