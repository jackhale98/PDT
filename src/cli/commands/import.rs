//! `tdt import` command - Import entities from CSV files

use console::style;
use csv::ReaderBuilder;
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::schema::template::{TemplateContext, TemplateGenerator};

#[derive(clap::Args, Debug)]
pub struct ImportArgs {
    /// Entity type to import (req, risk, cmp, sup)
    #[arg(value_parser = parse_entity_type)]
    pub entity_type: Option<EntityPrefix>,

    /// CSV file to import
    pub file: Option<PathBuf>,

    /// Generate a CSV template for the entity type
    #[arg(long)]
    pub template: bool,

    /// Validate CSV without creating files
    #[arg(long)]
    pub dry_run: bool,

    /// Continue importing after errors (default: stop on first error)
    #[arg(long)]
    pub skip_errors: bool,

    /// Update existing entities if ID column matches
    #[arg(long)]
    pub update: bool,
}

fn parse_entity_type(s: &str) -> Result<EntityPrefix, String> {
    match s.to_lowercase().as_str() {
        "req" => Ok(EntityPrefix::Req),
        "risk" => Ok(EntityPrefix::Risk),
        "cmp" => Ok(EntityPrefix::Cmp),
        "sup" => Ok(EntityPrefix::Sup),
        "test" => Ok(EntityPrefix::Test),
        "proc" => Ok(EntityPrefix::Proc),
        "ctrl" => Ok(EntityPrefix::Ctrl),
        "ncr" => Ok(EntityPrefix::Ncr),
        "capa" => Ok(EntityPrefix::Capa),
        "quote" | "quot" => Ok(EntityPrefix::Quot),
        _ => Err(format!(
            "Unsupported entity type: '{}'. Supported: req, risk, cmp, sup, test, proc, ctrl, ncr, capa, quote",
            s
        )),
    }
}

/// Import statistics
#[derive(Default)]
struct ImportStats {
    rows_processed: usize,
    entities_created: usize,
    entities_updated: usize,
    errors: usize,
    skipped: usize,
}

pub fn run(args: ImportArgs) -> Result<()> {
    // Handle template generation
    if args.template {
        let entity_type = args.entity_type.ok_or_else(|| {
            miette::miette!(
                "Entity type required for template generation. Usage: tdt import --template req"
            )
        })?;
        return generate_template(entity_type);
    }

    // Require both entity type and file for import
    let entity_type = args
        .entity_type
        .ok_or_else(|| miette::miette!("Entity type required. Usage: tdt import req data.csv"))?;

    let file_path = args
        .file
        .clone()
        .ok_or_else(|| miette::miette!("CSV file required. Usage: tdt import req data.csv"))?;

    if !file_path.exists() {
        return Err(miette::miette!("File not found: {}", file_path.display()));
    }

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    println!(
        "{} Importing {} entities from {}{}",
        style("→").blue(),
        style(entity_type.as_str()).cyan(),
        style(file_path.display()).yellow(),
        if args.dry_run {
            style(" (dry run)").dim().to_string()
        } else {
            String::new()
        }
    );
    println!();

    let stats = match entity_type {
        EntityPrefix::Req => import_requirements(&project, &file_path, &args)?,
        EntityPrefix::Risk => import_risks(&project, &file_path, &args)?,
        EntityPrefix::Cmp => import_components(&project, &file_path, &args)?,
        EntityPrefix::Sup => import_suppliers(&project, &file_path, &args)?,
        EntityPrefix::Test => import_tests(&project, &file_path, &args)?,
        EntityPrefix::Proc => import_processes(&project, &file_path, &args)?,
        EntityPrefix::Ctrl => import_controls(&project, &file_path, &args)?,
        EntityPrefix::Ncr => import_ncrs(&project, &file_path, &args)?,
        EntityPrefix::Capa => import_capas(&project, &file_path, &args)?,
        EntityPrefix::Quot => import_quotes(&project, &file_path, &args)?,
        _ => {
            return Err(miette::miette!(
                "Import not yet implemented for {}",
                entity_type.as_str()
            ));
        }
    };

    // Print summary
    println!();
    println!("{}", style("─".repeat(50)).dim());
    println!("{}", style("Import Summary").bold());
    println!("{}", style("─".repeat(50)).dim());
    println!("  Rows processed:   {}", style(stats.rows_processed).cyan());
    println!(
        "  Entities created: {}",
        style(stats.entities_created).green()
    );
    if stats.entities_updated > 0 {
        println!(
            "  Entities updated: {}",
            style(stats.entities_updated).yellow()
        );
    }
    if stats.errors > 0 {
        println!("  Errors:           {}", style(stats.errors).red());
    }
    if stats.skipped > 0 {
        println!("  Skipped:          {}", style(stats.skipped).dim());
    }

    if args.dry_run {
        println!();
        println!(
            "{}",
            style("Dry run complete. No files were created.").yellow()
        );
    }

    if stats.errors > 0 && !args.skip_errors {
        return Err(miette::miette!(
            "Import completed with {} error(s)",
            stats.errors
        ));
    }

    Ok(())
}

/// Generate a CSV template for an entity type
fn generate_template(entity_type: EntityPrefix) -> Result<()> {
    let headers = get_csv_headers(entity_type);
    let example = get_csv_example(entity_type);

    // Output to stdout (can be redirected to file)
    println!("{}", headers.join(","));
    if !example.is_empty() {
        println!("{}", example.join(","));
    }

    // Print usage hint to stderr so it doesn't interfere with redirected output
    eprintln!();
    eprintln!(
        "{} Template generated. Redirect to file: tdt import --template {} > {}.csv",
        style("→").blue(),
        entity_type.as_str().to_lowercase(),
        entity_type.as_str().to_lowercase()
    );

    Ok(())
}

/// Get CSV headers for an entity type
fn get_csv_headers(entity_type: EntityPrefix) -> Vec<&'static str> {
    match entity_type {
        EntityPrefix::Req => vec![
            "title",
            "type",
            "priority",
            "status",
            "text",
            "rationale",
            "tags",
        ],
        EntityPrefix::Risk => vec![
            "title",
            "type",
            "description",
            "failure_mode",
            "cause",
            "effect",
            "severity",
            "occurrence",
            "detection",
            "tags",
        ],
        EntityPrefix::Cmp => vec![
            "part_number",
            "title",
            "make_buy",
            "category",
            "description",
            "material",
            "finish",
            "mass",
            "cost",
            "tags",
        ],
        EntityPrefix::Sup => vec![
            "short_name",
            "title",
            "website",
            "contact_email",
            "contact_phone",
            "address",
            "lead_time_days",
            "tags",
        ],
        EntityPrefix::Test => vec![
            "title",
            "type",
            "level",
            "method",
            "category",
            "priority",
            "objective",
            "description",
            "estimated_duration",
            "tags",
        ],
        EntityPrefix::Proc => vec![
            "title",
            "type",
            "operation_number",
            "description",
            "cycle_time_minutes",
            "setup_time_minutes",
            "operator_skill",
            "tags",
        ],
        EntityPrefix::Ctrl => vec![
            "title",
            "type",
            "category",
            "description",
            "characteristic_name",
            "nominal",
            "upper_limit",
            "lower_limit",
            "units",
            "critical",
            "tags",
        ],
        EntityPrefix::Ncr => vec![
            "title",
            "type",
            "severity",
            "category",
            "description",
            "part_number",
            "quantity_affected",
            "characteristic",
            "specification",
            "actual",
            "tags",
        ],
        EntityPrefix::Capa => vec![
            "title",
            "type",
            "source_type",
            "source_ref",
            "problem_statement",
            "root_cause",
            "tags",
        ],
        EntityPrefix::Quot => vec![
            "title",
            "supplier",
            "component",
            "currency",
            "unit_price",
            "lead_time_days",
            "moq",
            "description",
            "tags",
        ],
        _ => vec!["title", "description", "tags"],
    }
}

/// Get example CSV row for an entity type
fn get_csv_example(entity_type: EntityPrefix) -> Vec<&'static str> {
    match entity_type {
        EntityPrefix::Req => vec![
            "\"Stroke Length\"",
            "input",
            "critical",
            "draft",
            "\"The actuator shall have a minimum stroke length of 100mm\"",
            "\"Required for full range of motion\"",
            "\"mechanical,critical\"",
        ],
        EntityPrefix::Risk => vec![
            "\"Seal Failure\"",
            "design",
            "\"O-ring may fail under pressure\"",
            "\"Seal extrusion\"",
            "\"Excessive pressure differential\"",
            "\"Fluid leakage and system failure\"",
            "8",
            "4",
            "6",
            "\"seal,pressure\"",
        ],
        EntityPrefix::Cmp => vec![
            "\"PN-001\"",
            "\"Housing Assembly\"",
            "make",
            "mechanical",
            "\"Main structural housing\"",
            "\"6061-T6 Aluminum\"",
            "\"Anodize\"",
            "0.5",
            "125.00",
            "\"structural,machined\"",
        ],
        EntityPrefix::Sup => vec![
            "\"ACME\"",
            "\"ACME Manufacturing Co.\"",
            "\"https://acme.example.com\"",
            "\"sales@acme.example.com\"",
            "\"+1-555-123-4567\"",
            "\"123 Industrial Way, City, ST 12345\"",
            "14",
            "\"machining,precision\"",
        ],
        EntityPrefix::Test => vec![
            "\"Housing Dimensional Inspection\"",
            "verification",
            "unit",
            "inspection",
            "\"mechanical\"",
            "high",
            "\"Verify housing dimensions meet specification\"",
            "\"Measure critical dimensions of machined housing\"",
            "\"30 min\"",
            "\"verification,dimensional\"",
        ],
        EntityPrefix::Proc => vec![
            "\"CNC Rough Machining\"",
            "machining",
            "\"OP-010\"",
            "\"Initial rough machining of housing blank\"",
            "45",
            "30",
            "intermediate",
            "\"machining,cnc\"",
        ],
        EntityPrefix::Ctrl => vec![
            "\"Bore Diameter Check\"",
            "inspection",
            "variable",
            "\"In-process check of bore diameter\"",
            "\"Bore Diameter\"",
            "25.0",
            "25.02",
            "24.98",
            "mm",
            "true",
            "\"dimensional,critical\"",
        ],
        EntityPrefix::Ncr => vec![
            "\"Out-of-spec bore diameter\"",
            "internal",
            "minor",
            "dimensional",
            "\"Bore diameter measured outside tolerance\"",
            "\"PN-001\"",
            "5",
            "\"Bore Diameter\"",
            "\"25.0 +/- 0.02mm\"",
            "\"25.05mm\"",
            "\"dimensional,machining\"",
        ],
        EntityPrefix::Capa => vec![
            "\"Improve bore machining process\"",
            "corrective",
            "ncr",
            "\"NCR@1\"",
            "\"Recurring out-of-spec bore diameters\"",
            "\"Tool wear not being monitored\"",
            "\"machining,process\"",
        ],
        EntityPrefix::Quot => vec![
            "\"Housing Quote - Acme\"",
            "\"SUP@1\"",
            "\"CMP@1\"",
            "USD",
            "125.00",
            "14",
            "100",
            "\"Quote for housing assembly\"",
            "\"machining\"",
        ],
        _ => vec![],
    }
}

/// Import requirements from CSV
fn import_requirements(
    project: &Project,
    file_path: &PathBuf,
    args: &ImportArgs,
) -> Result<ImportStats> {
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

    let output_dir = project.root().join("requirements/inputs");
    if !args.dry_run && !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let mut short_ids = ShortIdIndex::load(project);

    for (row_idx, result) in rdr.records().enumerate() {
        let row_num = row_idx + 2; // +2 for 1-indexed and header row
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

        // Extract fields
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

        let req_type = get_field(&record, &header_map, "type").unwrap_or("input".to_string());
        let priority = get_field(&record, &header_map, "priority").unwrap_or("medium".to_string());
        let status = get_field(&record, &header_map, "status").unwrap_or("draft".to_string());
        let text = get_field(&record, &header_map, "text").unwrap_or_default();
        let rationale = get_field(&record, &header_map, "rationale");
        let tags = get_field(&record, &header_map, "tags");

        // Generate entity
        let id = EntityId::new(EntityPrefix::Req);
        let ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_req_type(&req_type)
            .with_priority(&priority);

        let mut yaml = generator
            .generate_requirement(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace text if provided (template uses multi-line format with comments)
        if !text.is_empty() {
            yaml = yaml.replace(
                "text: |\n  # Enter requirement text here\n  # Use clear, testable language:\n  #   - \"shall\" for mandatory requirements\n  #   - \"should\" for recommended requirements\n  #   - \"may\" for optional requirements",
                &format!("text: |\n  {}", text.replace('\n', "\n  ")),
            );
        }

        // Add rationale if provided
        if let Some(rat) = rationale {
            if !rat.is_empty() {
                yaml = yaml.replace(
                    "rationale: \"\"",
                    &format!("rationale: \"{}\"", rat.replace('"', "\\\"")),
                );
            }
        }

        // Replace status if not draft
        if status != "draft" {
            yaml = yaml.replace("status: draft", &format!("status: {}", status));
        }

        // Add tags if provided
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
                style(format!("REQ-{}", &id.to_string()[4..12])).cyan(),
                truncate(&title, 40)
            );
        } else {
            // Write file
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import risks from CSV
fn import_risks(project: &Project, file_path: &PathBuf, args: &ImportArgs) -> Result<ImportStats> {
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

    let output_dir = project.root().join("risks");
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

        let risk_type = get_field(&record, &header_map, "type").unwrap_or("design".to_string());
        let description = get_field(&record, &header_map, "description").unwrap_or_default();
        let failure_mode = get_field(&record, &header_map, "failure_mode");
        let cause = get_field(&record, &header_map, "cause");
        let effect = get_field(&record, &header_map, "effect");
        let severity: Option<u8> =
            get_field(&record, &header_map, "severity").and_then(|s| s.parse().ok());
        let occurrence: Option<u8> =
            get_field(&record, &header_map, "occurrence").and_then(|s| s.parse().ok());
        let detection: Option<u8> =
            get_field(&record, &header_map, "detection").and_then(|s| s.parse().ok());
        let tags = get_field(&record, &header_map, "tags");

        let id = EntityId::new(EntityPrefix::Risk);

        // Build context with all available fields
        let mut ctx = TemplateContext::new(id.clone(), config.author())
            .with_title(&title)
            .with_risk_type(&risk_type);

        // Set severity/occurrence/detection on context if provided
        if let Some(s) = severity {
            ctx = ctx.with_severity(s);
        }
        if let Some(o) = occurrence {
            ctx = ctx.with_occurrence(o);
        }
        if let Some(d) = detection {
            ctx = ctx.with_detection(d);
        }

        let mut yaml = generator
            .generate_risk(&ctx)
            .map_err(|e| miette::miette!("Template error at row {}: {}", row_num, e))?;

        // Replace description (template uses multi-line format with comments)
        if !description.is_empty() {
            yaml = yaml.replace(
                "description: |\n  # Describe the risk scenario here\n  # What could go wrong? Under what conditions?",
                &format!("description: |\n  {}", description.replace('\n', "\n  ")),
            );
        }

        // Add FMEA fields if provided (template uses multi-line format with comments)
        if let Some(fm) = failure_mode {
            if !fm.is_empty() {
                yaml = yaml.replace(
                    "failure_mode: |\n  # How does this failure manifest?",
                    &format!("failure_mode: \"{}\"", fm.replace('"', "\\\"")),
                );
            }
        }
        if let Some(c) = cause {
            if !c.is_empty() {
                yaml = yaml.replace(
                    "cause: |\n  # What is the root cause or mechanism?",
                    &format!("cause: \"{}\"", c.replace('"', "\\\"")),
                );
            }
        }
        if let Some(e) = effect {
            if !e.is_empty() {
                yaml = yaml.replace(
                    "effect: |\n  # What is the impact or consequence?",
                    &format!("effect: \"{}\"", e.replace('"', "\\\"")),
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

        // Determine subdirectory based on type
        let subdir = match risk_type.as_str() {
            "process" => "process",
            _ => "design",
        };
        let type_dir = output_dir.join(subdir);

        if args.dry_run {
            println!(
                "{} Row {}: Would create {} - {}",
                style("○").dim(),
                row_num,
                style(format!("RISK-{}", &id.to_string()[5..13])).cyan(),
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import components from CSV
fn import_components(
    project: &Project,
    file_path: &PathBuf,
    args: &ImportArgs,
) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import suppliers from CSV
fn import_suppliers(
    project: &Project,
    file_path: &PathBuf,
    args: &ImportArgs,
) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import tests from CSV
fn import_tests(project: &Project, file_path: &PathBuf, args: &ImportArgs) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import processes from CSV
fn import_processes(
    project: &Project,
    file_path: &PathBuf,
    args: &ImportArgs,
) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import controls from CSV
fn import_controls(
    project: &Project,
    file_path: &PathBuf,
    args: &ImportArgs,
) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import NCRs from CSV
fn import_ncrs(project: &Project, file_path: &PathBuf, args: &ImportArgs) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import CAPAs from CSV
fn import_capas(project: &Project, file_path: &PathBuf, args: &ImportArgs) -> Result<ImportStats> {
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Import quotes from CSV
fn import_quotes(project: &Project, file_path: &PathBuf, args: &ImportArgs) -> Result<ImportStats> {
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

        let supplier = get_field(&record, &header_map, "supplier").unwrap_or_default();
        let component = get_field(&record, &header_map, "component").unwrap_or_default();
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
        let _ = short_ids.save(project);
    }

    Ok(stats)
}

/// Build a map of header names to column indices
fn build_header_map(headers: &csv::StringRecord) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.to_lowercase().trim().to_string(), i))
        .collect()
}

/// Get a field value from a CSV record
fn get_field(
    record: &csv::StringRecord,
    header_map: &HashMap<String, usize>,
    field: &str,
) -> Option<String> {
    header_map
        .get(field)
        .and_then(|&idx| record.get(idx))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Truncate a string for display
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
