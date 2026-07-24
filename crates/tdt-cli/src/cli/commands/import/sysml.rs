//! `tdt import sysml` - Import entities from a SysML v2 file

use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use tdt_core::core::config::Config;
use tdt_core::core::project::Project;
use tdt_core::sysml::import::{convert_to_entities, parse_sysml};

use super::common::ImportStats;

pub fn import(
    project: &Project,
    file_path: &PathBuf,
    dry_run: bool,
    update: bool,
) -> Result<ImportStats> {
    let mut stats = ImportStats::default();

    let content = fs::read_to_string(file_path).into_diagnostic()?;
    let config = Config::load();
    let author = config.author();

    let package = parse_sysml(&content).map_err(|e| miette::miette!("{}", e))?;

    println!(
        "  Parsed package '{}': {} requirements, {} verifications, {} parts, {} satisfy relations",
        style(&package.name).cyan(),
        package.requirements.len(),
        package.verifications.len(),
        package.parts.len(),
        package.satisfy_rels.len(),
    );
    if package.skipped_count > 0 {
        println!(
            "  {} Skipped {} unrecognized SysML construct(s)",
            style("note:").dim(),
            package.skipped_count,
        );
    }
    println!();

    let import_result =
        convert_to_entities(&package, &author).map_err(|e| miette::miette!("{}", e))?;

    stats.rows_processed = import_result.entities.len();

    // Print warnings
    for warning in &import_result.warnings {
        println!("  {} {}", style("warning:").yellow(), warning);
    }

    if import_result.skipped_constructs > 0 {
        println!(
            "  {} {} unrecognized constructs skipped",
            style("note:").dim(),
            import_result.skipped_constructs,
        );
    }

    for entity in &import_result.entities {
        let output_dir = get_entity_dir(project, &entity.prefix);
        let entity_path = output_dir.join(format!("{}.tdt.yaml", entity.id));

        // A SysML round-trip is lossy (links other than satisfy/verify,
        // acceptance criteria, revision history are not represented in
        // SysML). Never overwrite an existing entity unless --update was
        // explicitly requested.
        let exists = entity_path.exists();
        if exists && !update {
            println!(
                "  {} Skipped {} - already exists (use --update to overwrite;                  SysML overwrite loses non-SysML fields)",
                style("!").yellow(),
                style(&entity.id).cyan(),
            );
            continue;
        }

        let (verb_dry, verb_done) = if exists {
            ("Would update", "Updated")
        } else {
            ("Would create", "Created")
        };

        if dry_run {
            println!(
                "  {} {} {} - {}",
                style("+").green(),
                verb_dry,
                style(&entity.id).cyan(),
                entity.title,
            );
        } else {
            if !output_dir.exists() {
                fs::create_dir_all(&output_dir).into_diagnostic()?;
            }
            fs::write(&entity_path, &entity.yaml).into_diagnostic()?;
            println!(
                "  {} {} {} - {}",
                style("+").green(),
                verb_done,
                style(&entity.id).cyan(),
                entity.title,
            );
        }
        if exists {
            stats.entities_updated += 1;
        } else {
            stats.entities_created += 1;
        }
    }

    Ok(stats)
}

/// Get the output directory for a given entity prefix.
fn get_entity_dir(project: &Project, prefix: &str) -> PathBuf {
    let root = project.root();
    match prefix {
        "REQ" => root.join("requirements/inputs"),
        "TEST" => root.join("verification/protocols"),
        "RSLT" => root.join("verification/results"),
        "CMP" => root.join("bom/components"),
        "ASM" => root.join("bom/assemblies"),
        "RISK" => root.join("risks/design"),
        _ => root.join("imports"),
    }
}
