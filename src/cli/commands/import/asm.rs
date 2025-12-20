//! Import assemblies from CSV

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

    let output_dir = project.root().join("bom/assemblies");
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
        let description = get_field(&record, &header_map, "description");
        let parent = get_field(&record, &header_map, "parent");
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Asm);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(effective_title)
            .with_part_number(&part_number);

        let mut yaml = generator
            .generate_assembly(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace description if provided
        if let Some(desc) = description {
            if !desc.is_empty() {
                yaml = yaml.replace(
                    "description: |\n  # Detailed description of this assembly\n  # Include key specifications and sub-assemblies",
                    &format!("description: |\n  {}", desc.replace('\n', "\n  ")),
                );
            }
        }

        // Add parent link if provided
        if let Some(p) = parent {
            if !p.is_empty() {
                yaml = yaml.replace("parent: null", &format!("parent: \"{}\"", p));
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
                "{} Row {}: Would create {} - {} ({})",
                style("○").dim(),
                row_num,
                style(format!("ASM-{}", &id.to_string()[4..12])).cyan(),
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
