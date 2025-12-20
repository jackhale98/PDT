//! Import tests from CSV

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

    let output_dir = project.root().join("verification/protocols");
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

        let test_type =
            get_field(&record, &header_map, "type").unwrap_or("verification".to_string());
        let test_level = get_field(&record, &header_map, "level").unwrap_or("unit".to_string());
        let test_method =
            get_field(&record, &header_map, "method").unwrap_or("inspection".to_string());
        let category = get_field(&record, &header_map, "category").unwrap_or_default();
        let priority = get_field(&record, &header_map, "priority").unwrap_or("medium".to_string());
        let objective = get_field(&record, &header_map, "objective");
        let description = get_field(&record, &header_map, "description");
        let estimated_duration =
            get_field(&record, &header_map, "estimated_duration").unwrap_or("1 hour".to_string());
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Test);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_test_type(&test_type)
            .with_test_level(&test_level)
            .with_test_method(&test_method)
            .with_category(&category)
            .with_priority(&priority)
            .with_estimated_duration(&estimated_duration);

        let mut yaml = generator
            .generate_test(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace objective if provided
        if let Some(obj) = objective {
            if !obj.is_empty() {
                yaml = yaml.replace(
                    "objective: |\n  # What does this test verify or validate?\n  # Be specific about success criteria",
                    &format!("objective: |\n  {}", obj.replace('\n', "\n  ")),
                );
            }
        }

        // Replace description if provided
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Detailed description of the test\n  # Include any background or context",
                    &format!("description: |\n  {}", desc.replace('\n', "\n  ")),
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

        // Determine output directory based on test type
        let type_dir = match test_type.as_str() {
            "validation" => project.root().join("validation/protocols"),
            _ => project.root().join("verification/protocols"),
        };

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("TEST-{}", &id.to_string()[5..13])).cyan(),
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
