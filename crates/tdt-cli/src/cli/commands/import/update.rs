//! Generic CSV update path for `tdt import --update`
//!
//! One implementation serves every CSV entity type: each row must carry an
//! `id` column identifying an existing entity (full ID or short ID); the
//! row's non-empty cells are patched into that entity's YAML, the result is
//! schema-validated, and only then written. Rows without an `id` are errors —
//! create new entities with a separate `tdt import` run without `--update`.

use console::style;
use csv::ReaderBuilder;
use miette::{IntoDiagnostic, Result};
use std::path::Path;

use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::schema::registry::SchemaRegistry;
use tdt_core::schema::validator::Validator;

use super::common::{build_header_map, get_field, ImportStats};

/// Fields that may never be patched from a CSV.
const PROTECTED_FIELDS: &[&str] = &["id", "created", "author", "links", "entity_revision"];

pub fn import_update(
    project: &Project,
    prefix: EntityPrefix,
    file_path: &Path,
    dry_run: bool,
    skip_errors: bool,
) -> Result<ImportStats> {
    let mut stats = ImportStats::default();
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)
        .into_diagnostic()?;

    let headers = rdr.headers().into_diagnostic()?.clone();
    let header_map = build_header_map(&headers);

    if !header_map.contains_key("id") {
        return Err(miette::miette!(
            "--update requires an 'id' column in the CSV (each row must identify \
             the existing entity to update)"
        ));
    }

    let short_ids = ShortIdIndex::load(project);
    let registry = SchemaRegistry::default();
    let validator = Validator::new(&registry);

    // Top-level schema properties for this entity type — columns must name
    // one of these (or a field already present in the file) to be patched.
    let schema_props: std::collections::HashSet<String> = registry
        .get(prefix)
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|s| {
            s.get("properties").and_then(|p| {
                p.as_object()
                    .map(|o| o.keys().cloned().collect::<std::collections::HashSet<_>>())
            })
        })
        .unwrap_or_default();

    // Column names that exist in the CSV but were never patched anywhere,
    // reported once at the end so typos in headers don't pass silently.
    let mut unknown_columns: std::collections::BTreeSet<String> = Default::default();

    for (row_idx, result) in rdr.records().enumerate() {
        let row_num = row_idx + 2; // 1-indexed plus header row
        stats.rows_processed += 1;

        macro_rules! row_error {
            ($($fmt:tt)*) => {{
                eprintln!("{} Row {}: {}", style("✗").red(), row_num, format!($($fmt)*));
                stats.errors += 1;
                if !skip_errors {
                    return Err(miette::miette!("Row {}: {}", row_num, format!($($fmt)*)));
                }
                continue;
            }};
        }

        let record = match result {
            Ok(r) => r,
            Err(e) => row_error!("CSV parse error: {}", e),
        };

        let raw_id = get_field(&record, &header_map, "id").unwrap_or_default();
        if raw_id.is_empty() {
            row_error!(
                "missing 'id' — --update only updates existing entities; import rows \
                 without ids in a separate run without --update"
            );
        }

        // Resolve short IDs (REQ@1) to full IDs; pass full/partial IDs through.
        let resolved = short_ids.resolve(&raw_id).unwrap_or_else(|| raw_id.clone());
        if !resolved.starts_with(prefix.as_str()) {
            row_error!("id '{}' is not a {} entity", raw_id, prefix.as_str());
        }

        // Locate the entity file across this type's search directories.
        let mut found = None;
        for dir in Project::entity_search_directories(prefix) {
            let dir = project.root().join(dir);
            if let Some(path) = tdt_core::core::loader::find_entity_file(&dir, &resolved) {
                found = Some(path);
                break;
            }
        }
        let Some(path) = found else {
            row_error!(
                "no existing {} entity found for id '{}'",
                prefix.as_str(),
                raw_id
            );
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => row_error!("failed to read {}: {}", path.display(), e),
        };
        let mut doc: serde_yml::Value = match serde_yml::from_str(&content) {
            Ok(d) => d,
            Err(e) => row_error!("failed to parse {}: {}", path.display(), e),
        };
        let Some(map) = doc.as_mapping_mut() else {
            row_error!("{} is not a YAML mapping", path.display());
        };

        // Patch every non-empty cell whose header names a patchable field.
        let mut patched = 0usize;
        for (col, &idx) in &header_map {
            if PROTECTED_FIELDS.contains(&col.as_str()) {
                continue;
            }
            let Some(cell) = record.get(idx) else {
                continue;
            };
            let cell = cell.trim();
            if cell.is_empty() {
                continue;
            }

            let key = serde_yml::Value::String(col.clone());
            let new_value = if col == "tags" {
                // Comma-separated list column
                serde_yml::Value::Sequence(
                    cell.split(',')
                        .map(|t| serde_yml::Value::String(t.trim().to_string()))
                        .filter(|t| t.as_str().is_some_and(|s| !s.is_empty()))
                        .collect(),
                )
            } else {
                match map.get(&key) {
                    // Anchor the patched type to the existing value's type:
                    // string fields stay strings even if the cell looks
                    // numeric ("revision: 1.5" stays a string title, etc.).
                    Some(serde_yml::Value::String(_)) => serde_yml::Value::String(cell.to_string()),
                    // Absent or non-string: parse as a typed YAML scalar so
                    // numbers/bools land with the right type; schema
                    // validation below catches mismatches.
                    _ => serde_yml::from_str::<serde_yml::Value>(cell)
                        .unwrap_or_else(|_| serde_yml::Value::String(cell.to_string())),
                }
            };

            // Only patch fields the schema knows about (or that already
            // exist), so a typo'd header can't inject junk.
            if !schema_props.contains(col.as_str()) && !map.contains_key(&key) {
                unknown_columns.insert(col.clone());
                continue;
            }

            map.insert(key, new_value);
            patched += 1;
        }

        if patched == 0 {
            row_error!("no updatable fields in this row (all cells empty or unknown columns)");
        }

        // Bump the entity revision like every other update path does. Field
        // name varies by entity: `entity_revision` on newer types, integer
        // `revision` on e.g. requirements. (`revision` holding a string is a
        // part/document revision like "A" — never auto-bumped.)
        for rev_name in ["entity_revision", "revision"] {
            let rev_key = serde_yml::Value::String(rev_name.to_string());
            if let Some(rev) = map.get(&rev_key).and_then(|v| v.as_u64()) {
                map.insert(rev_key, serde_yml::Value::Number((rev + 1).into()));
                break;
            }
        }

        // Never write a patched entity that fails its own schema.
        let new_yaml =
            tdt_core::yaml::template::to_block_scalars(&match serde_yml::to_string(&doc) {
                Ok(y) => y,
                Err(e) => row_error!("failed to serialize update: {}", e),
            });
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        match validator.validate(&new_yaml, &filename, prefix) {
            Ok(result) if result.valid => {}
            Ok(result) => {
                let msgs: Vec<String> = result.errors.iter().map(|e| e.message.clone()).collect();
                row_error!("update fails schema validation: {}", msgs.join("; "));
            }
            Err(e) => row_error!("update fails schema validation: {}", e),
        }

        if dry_run {
            println!(
                "{} Row {}: would update {} ({} field(s))",
                style("→").blue(),
                row_num,
                style(&resolved).cyan(),
                patched
            );
        } else {
            if let Err(e) = std::fs::write(&path, new_yaml) {
                row_error!("failed to write {}: {}", path.display(), e);
            }
            println!(
                "{} Row {}: updated {} ({} field(s))",
                style("✓").green(),
                row_num,
                style(&resolved).cyan(),
                patched
            );
        }
        stats.entities_updated += 1;
    }

    if !unknown_columns.is_empty() {
        eprintln!(
            "{} Ignored unknown column(s): {}",
            style("warning:").yellow(),
            unknown_columns.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    Ok(stats)
}
