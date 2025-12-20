//! Import CAPAs from CSV

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

    let output_dir = project.root().join("quality/capas");
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

        let capa_type = get_field(&record, &header_map, "type").unwrap_or("corrective".to_string());
        let source_type =
            get_field(&record, &header_map, "source_type").unwrap_or("ncr".to_string());
        let source_ref = get_field(&record, &header_map, "source_ref").unwrap_or_default();
        let problem_statement = get_field(&record, &header_map, "problem_statement");
        let root_cause = get_field(&record, &header_map, "root_cause");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Capa);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_capa_type(&capa_type)
            .with_source_type(&source_type)
            .with_source_ref(&source_ref);

        let mut yaml = generator
            .generate_capa(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace problem statement if provided
        if let Some(ps) = problem_statement {
            if !ps.is_empty() {
                yaml = yaml.replace(
                    "problem_statement: |\n  # Describe the problem being addressed\n  # Include scope and impact",
                    &format!("problem_statement: |\n  {}", ps.replace('\n', "\n  ")),
                );
            }
        }

        // Replace root cause if provided
        if let Some(rc) = root_cause {
            if !rc.is_empty() {
                yaml = yaml.replace(
                    "root_cause: |\n    # Document the root cause",
                    &format!("root_cause: |\n    {}", rc.replace('\n', "\n    ")),
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
                style(format!("CAPA-{}", &id.to_string()[5..13])).cyan(),
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
