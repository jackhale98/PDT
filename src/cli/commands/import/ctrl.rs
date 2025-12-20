//! Import controls from CSV

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

    let output_dir = project.root().join("manufacturing/controls");
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

        // Process - use CSV column or --process flag
        let csv_process = get_field(&record, &header_map, "process");
        let process = csv_process.or_else(|| args.process.clone());

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

        let control_type =
            get_field(&record, &header_map, "type").unwrap_or("inspection".to_string());
        let control_category =
            get_field(&record, &header_map, "category").unwrap_or("variable".to_string());
        let description = get_field(&record, &header_map, "description");
        let characteristic_name = get_field(&record, &header_map, "characteristic_name");
        let nominal: Option<f64> =
            get_field(&record, &header_map, "nominal").and_then(|s| s.parse().ok());
        let upper_limit: Option<f64> =
            get_field(&record, &header_map, "upper_limit").and_then(|s| s.parse().ok());
        let lower_limit: Option<f64> =
            get_field(&record, &header_map, "lower_limit").and_then(|s| s.parse().ok());
        let units = get_field(&record, &header_map, "units").unwrap_or("mm".to_string());
        let critical = get_field(&record, &header_map, "critical")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(false);
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Ctrl);
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_control_type(&control_type);

        ctx.critical = critical;

        let mut yaml = generator
            .generate_control(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace control_category
        yaml = yaml.replace(
            "control_category: variable",
            &format!("control_category: {}", control_category),
        );

        // Replace description if provided
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Detailed description of this control plan item\n  # Include what is being controlled and why",
                    &format!("description: |\n  {}", desc.replace('\n', "\n  ")),
                );
            }
        }

        // Update characteristic fields
        if let Some(char_name) = characteristic_name {
            yaml = yaml.replace("name: \"\"", &format!("name: \"{}\"", char_name));
        }
        if let Some(nom) = nominal {
            yaml = yaml.replace("nominal: 0.0", &format!("nominal: {}", nom));
        }
        if let Some(upper) = upper_limit {
            yaml = yaml.replace("upper_limit: 0.0", &format!("upper_limit: {}", upper));
        }
        if let Some(lower) = lower_limit {
            yaml = yaml.replace("lower_limit: 0.0", &format!("lower_limit: {}", lower));
        }
        yaml = yaml.replace("units: \"mm\"", &format!("units: \"{}\"", units));

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

        // Add process link if provided
        if let Some(proc_id) = &process {
            yaml = yaml.replace("process: null", &format!("process: \"{}\"", proc_id));
        }

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("CTRL-{}", &id.to_string()[5..13])).cyan(),
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
