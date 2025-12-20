//! Import processes from CSV

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

    let output_dir = project.root().join("manufacturing/processes");
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

        let process_type =
            get_field(&record, &header_map, "type").unwrap_or("machining".to_string());
        let operation_number = get_field(&record, &header_map, "operation_number");
        let description = get_field(&record, &header_map, "description");
        let cycle_time: Option<f64> =
            get_field(&record, &header_map, "cycle_time_minutes").and_then(|s| s.parse().ok());
        let setup_time: Option<f64> =
            get_field(&record, &header_map, "setup_time_minutes").and_then(|s| s.parse().ok());
        let operator_skill = get_field(&record, &header_map, "operator_skill");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Proc);
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_process_type(&process_type);

        if let Some(ref op_num) = operation_number {
            ctx = ctx.with_operation_number(op_num);
        }

        let mut yaml = generator
            .generate_process(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace description if provided
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Detailed description of this manufacturing process\n  # Include key steps and requirements",
                    &format!("description: |\n  {}", desc.replace('\n', "\n  ")),
                );
            }
        }

        // Add cycle/setup times
        if let Some(ct) = cycle_time {
            yaml = yaml.replace(
                "cycle_time_minutes: null",
                &format!("cycle_time_minutes: {}", ct),
            );
        }
        if let Some(st) = setup_time {
            yaml = yaml.replace(
                "setup_time_minutes: null",
                &format!("setup_time_minutes: {}", st),
            );
        }

        // Replace operator skill if provided
        if let Some(skill) = operator_skill {
            if !skill.is_empty() {
                yaml = yaml.replace(
                    "operator_skill: intermediate",
                    &format!("operator_skill: {}", skill),
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
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("PROC-{}", &id.to_string()[5..13])).cyan(),
                truncate(&title, 40)
            );
        } else {
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
