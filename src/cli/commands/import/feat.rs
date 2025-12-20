//! Import features from CSV

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

    let output_dir = project.root().join("tolerances/features");
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

        // Component is required for features - use CSV column or --component flag
        let csv_component = get_field(&record, &header_map, "component");
        let component = csv_component
            .or_else(|| args.component.clone())
            .unwrap_or_default();
        if component.is_empty() {
            eprintln!(
                "{} Row {}: Missing required field 'component' (provide in CSV or use --component flag)",
                style("✗").red(),
                row_num
            );
            stats.errors += 1;
            if !args.skip_errors {
                return Err(miette::miette!(
                    "Missing required field 'component' at row {} (provide in CSV or use --component flag)",
                    row_num
                ));
            }
            continue;
        }

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

        let feature_type =
            get_field(&record, &header_map, "feature_type").unwrap_or("external".to_string());
        let nominal: Option<f64> =
            get_field(&record, &header_map, "nominal").and_then(|s| s.parse().ok());
        let plus_tolerance: Option<f64> =
            get_field(&record, &header_map, "plus_tolerance").and_then(|s| s.parse().ok());
        let minus_tolerance: Option<f64> =
            get_field(&record, &header_map, "minus_tolerance").and_then(|s| s.parse().ok());
        let units = get_field(&record, &header_map, "units").unwrap_or("mm".to_string());
        let datum = get_field(&record, &header_map, "datum");
        let critical = get_field(&record, &header_map, "critical")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(false);
        let description = get_field(&record, &header_map, "description");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Feat);
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_feature_type(&feature_type)
            .with_component_id(&component);

        ctx.critical = critical;

        let mut yaml = generator
            .generate_feature(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Update dimension fields
        if let Some(nom) = nominal {
            yaml = yaml.replace("nominal: 0.0", &format!("nominal: {}", nom));
        }
        if let Some(plus) = plus_tolerance {
            yaml = yaml.replace("plus_tolerance: 0.0", &format!("plus_tolerance: {}", plus));
        }
        if let Some(minus) = minus_tolerance {
            yaml = yaml.replace(
                "minus_tolerance: 0.0",
                &format!("minus_tolerance: {}", minus),
            );
        }
        yaml = yaml.replace("units: \"mm\"", &format!("units: \"{}\"", units));

        // Add datum if provided
        if let Some(d) = datum {
            if !d.is_empty() {
                yaml = yaml.replace("datum: null", &format!("datum: \"{}\"", d));
            }
        }

        // Replace description if provided
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Additional notes about this feature",
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

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {} (on {})",
                style("○").dim(),
                row_num,
                style(format!("FEAT-{}", &id.to_string()[5..13])).cyan(),
                truncate(&title, 30),
                component
            );
        } else {
            let file_path = output_dir.join(format!("{}.tdt.yaml", id));
            fs::write(&file_path, &yaml).into_diagnostic()?;

            let short_id = short_ids.add(id.to_string());
            println!(
                "{} Row {}: Created {} - {} (on {})",
                style("✓").green(),
                row_num,
                style(short_id.unwrap_or_else(|| id.to_string())).cyan(),
                truncate(&title, 30),
                component
            );
            stats.entities_created += 1;
        }
    }

    if !args.dry_run {
        crate::cli::commands::utils::save_short_ids(&mut short_ids, project);
    }

    Ok(stats)
}
