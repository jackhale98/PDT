//! Import requirements from CSV

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

use super::common::{build_header_map, get_field, truncate, ImportArgs, ImportStats};

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

    let output_dir = project.root().join("requirements/inputs");
    if !args.dry_run && !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let mut short_ids = ShortIdIndex::load(project);

    for (row_idx, result) in rdr.records().enumerate() {
        let row_num = row_idx + 2; // +2 for 1-indexed and header row
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

        // Extract fields
        let title = get_field(&record, &header_map, "title").unwrap_or_default();
        if title.is_empty() {
            eprintln!(
                "{} Row {}: Missing required field 'title'",
                style("✗").red(),
                row_num
            );
            stats.errors += 1;
            if !args.skip_errors {
                return Err(miette::miette!(
                    "Missing required field 'title' at row {}",
                    row_num
                ));
            }
            continue;
        }

        let req_type = get_field(&record, &header_map, "type").unwrap_or("input".to_string());
        let priority = get_field(&record, &header_map, "priority").unwrap_or("medium".to_string());
        let status = get_field(&record, &header_map, "status").unwrap_or("draft".to_string());
        let text = get_field(&record, &header_map, "text").unwrap_or_default();
        let rationale = get_field(&record, &header_map, "rationale");
        let tags = get_field(&record, &header_map, "tags");

        // Generate entity
        let id = EntityId::new(EntityPrefix::Req);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_req_type(&req_type)
            .with_priority(&priority);

        let mut yaml = generator
            .generate_requirement(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace text if provided (template uses multi-line format with comments)
        if !text.is_empty() {
            yaml = yaml.replace(
                "text: |\n  # Enter requirement text here\n  # Use clear, testable language:\n  #   - \"shall\" for mandatory requirements\n  #   - \"should\" for recommended requirements\n  #   - \"may\" for optional requirements",
                &format!("text: |\n  {}", text.replace('\n', "\n  ")),
            );
        }

        // Add rationale if provided
        if let Some(rat) = rationale {
            if !rat.is_empty() {
                yaml = yaml.replace(
                    "rationale: \"\"",
                    &format!("rationale: \"{}\"", rat.replace('"', "\\\"")),
                );
            }
        }

        // Replace status if not draft
        if status != "draft" {
            yaml = yaml.replace("status: draft", &format!("status: {}", status));
        }

        // Add tags if provided
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
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("REQ-{}", &id.to_string()[4..12])).cyan(),
                truncate(&title, 40)
            );
        } else {
            // Write file
            let file_path = output_dir.join(format!("{}.tdt.yaml", id));
            fs::write(&file_path, &yaml).into_diagnostic()?;

            let short_id = short_ids.add(id.to_string());
            println!(
                "{} Row {}: Created {} - {}",
                style("✓").green(),
                row_num,
                style(short_id.unwrap_or_else(|| id.to_string())).cyan(),
                truncate(&title, 40)
            );
            stats.entities_created += 1;
        }
    }

    if !args.dry_run {
        crate::cli::commands::utils::save_short_ids(&mut short_ids, project);
    }

    Ok(stats)
}
