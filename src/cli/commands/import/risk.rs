//! Import risks from CSV

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

    let output_dir = project.root().join("risks");
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

        let risk_type = get_field(&record, &header_map, "type").unwrap_or("design".to_string());
        let description = get_field(&record, &header_map, "description").unwrap_or_default();
        let failure_mode = get_field(&record, &header_map, "failure_mode");
        let cause = get_field(&record, &header_map, "cause");
        let effect = get_field(&record, &header_map, "effect");
        let severity: Option<u8> =
            get_field(&record, &header_map, "severity").and_then(|s| s.parse().ok());
        let occurrence: Option<u8> =
            get_field(&record, &header_map, "occurrence").and_then(|s| s.parse().ok());
        let detection: Option<u8> =
            get_field(&record, &header_map, "detection").and_then(|s| s.parse().ok());
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Risk);

        // Build context with all available fields
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_risk_type(&risk_type);

        // Set severity/occurrence/detection on context if provided
        if let Some(s) = severity {
            ctx = ctx.with_severity(s);
        }
        if let Some(o) = occurrence {
            ctx = ctx.with_occurrence(o);
        }
        if let Some(d) = detection {
            ctx = ctx.with_detection(d);
        }

        let mut yaml = generator
            .generate_risk(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace description (template uses multi-line format with comments)
        if !description.is_empty() {
            yaml = yaml.replace(
                "description: |\n  # Describe the risk scenario here\n  # What could go wrong? Under what conditions?",
                &format!("description: |\n  {}", description.replace('\n', "\n  ")),
            );
        }

        // Add FMEA fields if provided (template uses multi-line format with comments)
        if let Some(fm) = failure_mode {
            if !fm.is_empty() {
                yaml = yaml.replace(
                    "failure_mode: |\n  # How does this failure manifest?",
                    &format!("failure_mode: \"{}\"", fm.replace('"', "\\\"")),
                );
            }
        }
        if let Some(c) = cause {
            if !c.is_empty() {
                yaml = yaml.replace(
                    "cause: |\n  # What is the root cause or mechanism?",
                    &format!("cause: \"{}\"", c.replace('"', "\\\"")),
                );
            }
        }
        if let Some(e) = effect {
            if !e.is_empty() {
                yaml = yaml.replace(
                    "effect: |\n  # What is the impact or consequence?",
                    &format!("effect: \"{}\"", e.replace('"', "\\\"")),
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

        // Determine subdirectory based on type
        let subdir = match risk_type.as_str() {
            "process" => "process",
            _ => "design",
        };
        let type_dir = output_dir.join(subdir);

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("RISK-{}", &id.to_string()[5..13])).cyan(),
                truncate(&title, 40)
            );
        } else {
            if !type_dir.exists() {
                fs::create_dir_all(&type_dir).into_diagnostic()?;
            }

            let file_path = type_dir.join(format!("{}.tdt.yaml", id));
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
