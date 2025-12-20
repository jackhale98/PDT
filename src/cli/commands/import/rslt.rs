//! Import results from CSV

use console::style;
use csv::ReaderBuilder;
use miette::{IntoDiagnostic, Result};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::schema::template::{TemplateContext, TemplateGenerator};

use super::common::{build_header_map, get_field, ImportArgs, ImportStats};

pub fn import(project: &Project, file_path: &PathBuf, args: &ImportArgs) -> Result<ImportStats> {
    let mut stats = ImportStats::default();
    let config = Config::load();
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;

    let file = File::open(file_path).into_diagnostic()?;
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(BufReader::new(file));

    let headers = rdr.headers().into_diagnostic()?.clone();
    let header_map = build_header_map(&headers);

    let output_dir = project.root().join("verification/results");
    if !args.dry_run && !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let mut short_ids = ShortIdIndex::load(project);

    for (row_idx, result) in rdr.records().enumerate() {
        let row_num = row_idx + 2;
        stats.rows_processed += 1;

        let record = match result {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "{} Row {}: CSV parse error: {}",
                    style("✗").red(),
                    row_num,
                    e
                );
                stats.errors += 1;
                if !args.skip_errors {
                    return Err(miette::miette!("CSV parse error at row {}: {}", row_num, e));
                }
                continue;
            }
        };

        // Test ID - use CSV column or --test flag (required for results)
        let csv_test = get_field(&record, &header_map, "test");
        let test_id_str = csv_test.or_else(|| args.test.clone()).unwrap_or_default();
        if test_id_str.is_empty() {
            eprintln!(
                "{} Row {}: Missing required field 'test' (provide in CSV or use --test flag)",
                style("✗").red(),
                row_num
            );
            stats.errors += 1;
            if !args.skip_errors {
                return Err(miette::miette!(
                    "Missing required field 'test' at row {} (provide in CSV or use --test flag)",
                    row_num
                ));
            }
            continue;
        }

        // Resolve short ID to full ID
        let resolved_test_id = short_ids
            .resolve(&test_id_str)
            .unwrap_or_else(|| test_id_str.clone());
        let test_entity_id = match resolved_test_id.parse::<EntityId>() {
            Ok(eid) => eid,
            Err(e) => {
                eprintln!(
                    "{} Row {}: Invalid test ID '{}': {}",
                    style("✗").red(),
                    row_num,
                    test_id_str,
                    e
                );
                stats.errors += 1;
                if !args.skip_errors {
                    return Err(miette::miette!(
                        "Invalid test ID '{}' at row {}: {}",
                        test_id_str,
                        row_num,
                        e
                    ));
                }
                continue;
            }
        };

        let verdict = get_field(&record, &header_map, "verdict").unwrap_or("pass".to_string());
        let executed_by =
            get_field(&record, &header_map, "executed_by").unwrap_or_else(|| config.author());
        let executed_date = get_field(&record, &header_map, "executed_date");
        let description = get_field(&record, &header_map, "description");
        let notes = get_field(&record, &header_map, "notes");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Rslt);

        // Build a title from test ID and date
        let result_title = format!("Result for {}", test_id_str);

        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&result_title)
            .with_test_id(test_entity_id)
            .with_verdict(&verdict)
            .with_executed_by(&executed_by);

        let mut yaml = generator
            .generate_result(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Update executed_date if provided
        if let Some(date) = executed_date {
            if !date.is_empty() {
                // Replace the auto-generated date with the provided one
                // The template uses ISO format like "2024-01-15T10:30:00Z"
                // Allow either date-only or full ISO format
                if date.contains('T') {
                    yaml = yaml.replace(
                        &format!(
                            "executed_date: {}",
                            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
                        ),
                        &format!("executed_date: {}", date),
                    );
                } else {
                    // Convert date-only to full ISO format
                    yaml = yaml.replace(
                        &format!(
                            "executed_date: {}",
                            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
                        ),
                        &format!("executed_date: {}T00:00:00Z", date),
                    );
                }
            }
        }

        // Add description/notes
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: \"\"",
                    &format!("description: \"{}\"", desc.replace('"', "\\\"")),
                );
            }
        }
        if let Some(n) = notes {
            if !n.is_empty() {
                yaml = yaml.replace(
                    "notes: \"\"",
                    &format!("notes: \"{}\"", n.replace('"', "\\\"")),
                );
            }
        }

        // Add tags
        if let Some(tags_str) = tags {
            if !tags_str.is_empty() {
                let tags_yaml: Vec<String> = tags_str
                    .split(',')
                    .map(|t| format!("\"{}\"", t.trim()))
                    .collect();
                yaml = yaml.replace("tags: []", &format!("tags: [{}]", tags_yaml.join(", ")));
            }
        }

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {} ({})",
                style("○").dim(),
                row_num,
                style(format!("RSLT-{}", &id.to_string()[5..13])).cyan(),
                test_id_str,
                verdict
            );
        } else {
            let file_path = output_dir.join(format!("{}.tdt.yaml", id));
            fs::write(&file_path, &yaml).into_diagnostic()?;

            let short_id = short_ids.add(id.to_string());
            println!(
                "{} Row {}: Created {} - {} ({})",
                style("✓").green(),
                row_num,
                style(short_id.unwrap_or_else(|| id.to_string())).cyan(),
                test_id_str,
                verdict
            );
            stats.entities_created += 1;
        }
    }

    if !args.dry_run {
        crate::cli::commands::utils::save_short_ids(&mut short_ids, project);
    }

    Ok(stats)
}
