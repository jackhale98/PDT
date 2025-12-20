//! Import components from CSV

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

    let output_dir = project.root().join("bom/components");
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

        // Assembly - use CSV column or --assembly flag
        let csv_assembly = get_field(&record, &header_map, "assembly");
        let assembly = csv_assembly.or_else(|| args.assembly.clone());

        let title = get_field(&record, &header_map, "title").unwrap_or_default();
        let part_number = get_field(&record, &header_map, "part_number").unwrap_or_default();

        if title.is_empty() && part_number.is_empty() {
            eprintln!(
                "{} Row {}: Missing required field 'title' or 'part_number'",
                style("✗").red(),
                row_num
            );
            stats.errors += 1;
            if !args.skip_errors {
                return Err(miette::miette!("Missing required field at row {}", row_num));
            }
            continue;
        }

        let effective_title = if title.is_empty() {
            &part_number
        } else {
            &title
        };
        let make_buy = get_field(&record, &header_map, "make_buy").unwrap_or("make".to_string());
        let category =
            get_field(&record, &header_map, "category").unwrap_or("mechanical".to_string());
        let description = get_field(&record, &header_map, "description");
        let material = get_field(&record, &header_map, "material");
        let finish = get_field(&record, &header_map, "finish");
        let mass: Option<f64> =
            get_field(&record, &header_map, "mass").and_then(|s| s.parse().ok());
        let cost: Option<f64> =
            get_field(&record, &header_map, "cost").and_then(|s| s.parse().ok());
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Cmp);
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(effective_title)
            .with_part_number(&part_number)
            .with_make_buy(&make_buy)
            .with_category(&category);

        // Set material via context if provided
        if let Some(ref mat) = material {
            if !mat.is_empty() {
                ctx = ctx.with_material(mat);
            }
        }

        let mut yaml = generator
            .generate_component(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Add optional fields
        // Description uses multi-line format with comments in template
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Detailed description of this component\n  # Include key specifications and requirements",
                    &format!("description: |\n  {}", desc.replace('\n', "\n  ")),
                );
            }
        }
        // Add finish field after material line if provided (finish isn't in template by default)
        if let Some(fin) = finish {
            if !fin.is_empty() {
                let mat_value = material.clone().unwrap_or_default();
                yaml = yaml.replace(
                    &format!("material: \"{}\"", mat_value),
                    &format!(
                        "material: \"{}\"\nfinish: \"{}\"",
                        mat_value,
                        fin.replace('"', "\\\"")
                    ),
                );
            }
        }
        // mass_kg in template (not mass)
        if let Some(m) = mass {
            yaml = yaml.replace("mass_kg: null", &format!("mass_kg: {}", m));
        }
        // Only replace the first unit_cost (in physical properties section)
        if let Some(c) = cost {
            yaml = yaml.replacen("unit_cost: null", &format!("unit_cost: {}", c), 1);
        }
        if let Some(tags_str) = tags {
            if !tags_str.is_empty() {
                let tags_yaml: Vec<String> = tags_str
                    .split(',')
                    .map(|t| format!("\"{}\"", t.trim()))
                    .collect();
                yaml = yaml.replace("tags: []", &format!("tags: [{}]", tags_yaml.join(", ")));
            }
        }

        // Add assembly link if provided
        if let Some(asm_id) = &assembly {
            yaml = yaml.replace("assembly: null", &format!("assembly: \"{}\"", asm_id));
        }

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {} ({})",
                style("○").dim(),
                row_num,
                style(format!("CMP-{}", &id.to_string()[4..12])).cyan(),
                truncate(effective_title, 30),
                part_number
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
                truncate(effective_title, 30),
                part_number
            );
            stats.entities_created += 1;
        }
    }

    if !args.dry_run {
        crate::cli::commands::utils::save_short_ids(&mut short_ids, project);
    }

    Ok(stats)
}
