//! Import suppliers from CSV

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

    let output_dir = project.root().join("bom/suppliers");
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
        let short_name = get_field(&record, &header_map, "short_name").unwrap_or_default();

        if title.is_empty() && short_name.is_empty() {
            eprintln!(
                "{} Row {}: Missing required field 'title' or 'short_name'",
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
            &short_name
        } else {
            &title
        };
        let effective_short = if short_name.is_empty() {
            // Generate short name from title (first word, uppercase)
            effective_title
                .split_whitespace()
                .next()
                .unwrap_or("SUP")
                .to_uppercase()
        } else {
            short_name.clone()
        };

        let website = get_field(&record, &header_map, "website");
        let contact_email = get_field(&record, &header_map, "contact_email");
        let contact_phone = get_field(&record, &header_map, "contact_phone");
        let address = get_field(&record, &header_map, "address");
        // Note: lead_time_days is parsed but not used - it's a per-component field, not supplier-level
        let _lead_time: Option<u32> =
            get_field(&record, &header_map, "lead_time_days").and_then(|s| s.parse().ok());
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Sup);
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(effective_title)
            .with_short_name(&effective_short);

        // Set website via context (template conditionally includes it)
        if let Some(ref web) = website {
            if !web.is_empty() {
                ctx = ctx.with_website(web);
            }
        }

        let mut yaml = generator
            .generate_supplier(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Add contact entry if email/phone provided (template uses contacts: [] array)
        if contact_email.is_some() || contact_phone.is_some() {
            let email_str = contact_email.as_deref().unwrap_or("");
            let phone_str = contact_phone.as_deref().unwrap_or("");
            let contact_entry = format!(
                "contacts:\n  - name: \"Primary Contact\"\n    role: \"Sales\"\n    email: \"{}\"\n    phone: \"{}\"\n    primary: true",
                email_str, phone_str
            );
            yaml = yaml.replace("contacts: []", &contact_entry);
        }

        // Add address entry if provided (template uses addresses: [] array)
        if let Some(addr) = address {
            if !addr.is_empty() {
                let address_entry = format!(
                    "addresses:\n  - type: headquarters\n    street: \"{}\"\n    city: \"\"\n    state: \"\"\n    postal: \"\"\n    country: \"\"",
                    addr.replace('"', "\\\"")
                );
                yaml = yaml.replace("addresses: []", &address_entry);
            }
        }

        // Note: lead_time_days is not a supplier-level field (it's per-component in suppliers list)
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
                style(format!("SUP-{}", &id.to_string()[4..12])).cyan(),
                truncate(effective_title, 30),
                effective_short
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
                effective_short
            );
            stats.entities_created += 1;
        }
    }

    if !args.dry_run {
        crate::cli::commands::utils::save_short_ids(&mut short_ids, project);
    }

    Ok(stats)
}
