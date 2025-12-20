//! Import NCRs from CSV

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

    let output_dir = project.root().join("quality/ncrs");
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

        let ncr_type = get_field(&record, &header_map, "type").unwrap_or("internal".to_string());
        let ncr_severity =
            get_field(&record, &header_map, "severity").unwrap_or("minor".to_string());
        let ncr_category =
            get_field(&record, &header_map, "category").unwrap_or("dimensional".to_string());
        let description = get_field(&record, &header_map, "description");
        let part_number = get_field(&record, &header_map, "part_number");
        let quantity_affected: Option<u32> =
            get_field(&record, &header_map, "quantity_affected").and_then(|s| s.parse().ok());
        let characteristic = get_field(&record, &header_map, "characteristic");
        let specification = get_field(&record, &header_map, "specification");
        let actual = get_field(&record, &header_map, "actual");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Ncr);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_ncr_type(&ncr_type)
            .with_ncr_severity(&ncr_severity)
            .with_ncr_category(&ncr_category);

        let mut yaml = generator
            .generate_ncr(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Add affected items fields
        if let Some(pn) = part_number {
            yaml = yaml.replace("part_number: \"\"", &format!("part_number: \"{}\"", pn));
        }
        if let Some(qty) = quantity_affected {
            yaml = yaml.replace(
                "quantity_affected: 1",
                &format!("quantity_affected: {}", qty),
            );
        }

        // Add defect details
        if let Some(char_name) = characteristic {
            yaml = yaml.replace(
                "characteristic: \"\"",
                &format!("characteristic: \"{}\"", char_name),
            );
        }
        if let Some(spec) = specification {
            yaml = yaml.replace(
                "specification: \"\"",
                &format!("specification: \"{}\"", spec),
            );
        }
        if let Some(act) = actual {
            yaml = yaml.replace("actual: \"\"", &format!("actual: \"{}\"", act));
        }

        // Note: description in NCR template is not a multi-line field, so we skip it
        // The defect details serve as the description
        let _ = description; // suppress unused warning

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
                style(format!("NCR-{}", &id.to_string()[4..12])).cyan(),
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
