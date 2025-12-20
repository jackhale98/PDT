//! Import quotes from CSV

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

    let output_dir = project.root().join("bom/quotes");
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

        // Supplier - use CSV column or --supplier flag
        let csv_supplier = get_field(&record, &header_map, "supplier");
        let supplier = csv_supplier
            .or_else(|| args.supplier.clone())
            .unwrap_or_default();

        // Component - use CSV column or --component flag
        let csv_component = get_field(&record, &header_map, "component");
        let component = csv_component
            .or_else(|| args.component.clone())
            .unwrap_or_default();
        let currency = get_field(&record, &header_map, "currency").unwrap_or("USD".to_string());
        let unit_price: Option<f64> =
            get_field(&record, &header_map, "unit_price").and_then(|s| s.parse().ok());
        let lead_time_days: Option<u32> =
            get_field(&record, &header_map, "lead_time_days").and_then(|s| s.parse().ok());
        let moq: Option<u32> = get_field(&record, &header_map, "moq").and_then(|s| s.parse().ok());
        let description = get_field(&record, &header_map, "description");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Quot);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_supplier(&supplier)
            .with_component_id(&component);

        let mut yaml = generator
            .generate_quote(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Update currency
        yaml = yaml.replace("currency: USD", &format!("currency: {}", currency));

        // Update price break
        if let Some(price) = unit_price {
            yaml = yaml.replace("unit_price: 0.00", &format!("unit_price: {:.2}", price));
        }
        if let Some(lt) = lead_time_days {
            // Replace in price_breaks section
            yaml = yaml.replacen("lead_time_days: 14", &format!("lead_time_days: {}", lt), 1);
            // Also update the main lead_time_days
            yaml = yaml.replacen("lead_time_days: 14", &format!("lead_time_days: {}", lt), 1);
        }

        // Update MOQ
        if let Some(m) = moq {
            yaml = yaml.replace("moq: null", &format!("moq: {}", m));
        }

        // Replace description if provided
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Notes about this quote\n  # Include any special terms or conditions",
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
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("QUOT-{}", &id.to_string()[5..13])).cyan(),
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
