//! `tdt import` command - Import entities from CSV or SysML files

mod asm;
mod capa;
mod cmp;
mod common;
mod ctrl;
mod feat;
mod ncr;
mod proc;
mod quote;
mod req;
mod risk;
mod rslt;
mod sup;
mod sysml;
mod test;
mod update;

use console::style;
use miette::Result;
use std::path::PathBuf;

use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;

pub use common::generate_template;

/// What the first positional argument resolved to
#[derive(Debug, Clone)]
pub(crate) enum ImportTarget {
    /// A standard CSV entity type import
    EntityType(EntityPrefix),
    /// SysML v2 format import
    Sysml,
}

#[derive(clap::Args, Debug)]
pub struct ImportArgs {
    /// Entity type or format to import (req, risk, cmp, sup, feat, rslt, asm, sysml, etc.)
    #[arg(value_parser = parse_import_target)]
    pub target: Option<ImportTarget>,

    /// File to import (CSV or SysML)
    pub file: Option<PathBuf>,

    /// Generate a CSV template for the entity type
    #[arg(long)]
    pub template: bool,

    /// Validate without creating files
    #[arg(long)]
    pub dry_run: bool,

    /// Continue importing after errors (default: stop on first error)
    #[arg(long)]
    pub skip_errors: bool,

    /// Update existing entities instead of creating new ones. Every row
    /// must have an 'id' column identifying the entity (full or short ID);
    /// non-empty cells overwrite the matching YAML fields after schema
    /// validation. Rows without an id are errors.
    #[arg(long)]
    pub update: bool,

    /// Default component ID for feature imports (used when CSV row lacks component column)
    #[arg(long)]
    pub component: Option<String>,

    /// Default supplier ID for quote imports (used when CSV row lacks supplier column)
    #[arg(long)]
    pub supplier: Option<String>,

    /// Default test ID for result imports (used when CSV row lacks test column)
    #[arg(long)]
    pub test: Option<String>,

    /// Default process ID for control imports (used when CSV row lacks process column)
    #[arg(long)]
    pub process: Option<String>,

    /// Default assembly ID for component imports (used when CSV row lacks assembly column)
    #[arg(long)]
    pub assembly: Option<String>,
}

fn parse_import_target(s: &str) -> Result<ImportTarget, String> {
    match s.to_lowercase().as_str() {
        "sysml" => Ok(ImportTarget::Sysml),
        "req" => Ok(ImportTarget::EntityType(EntityPrefix::Req)),
        "risk" => Ok(ImportTarget::EntityType(EntityPrefix::Risk)),
        "cmp" => Ok(ImportTarget::EntityType(EntityPrefix::Cmp)),
        "asm" => Ok(ImportTarget::EntityType(EntityPrefix::Asm)),
        "sup" => Ok(ImportTarget::EntityType(EntityPrefix::Sup)),
        "test" => Ok(ImportTarget::EntityType(EntityPrefix::Test)),
        "rslt" | "result" => Ok(ImportTarget::EntityType(EntityPrefix::Rslt)),
        "proc" => Ok(ImportTarget::EntityType(EntityPrefix::Proc)),
        "ctrl" => Ok(ImportTarget::EntityType(EntityPrefix::Ctrl)),
        "ncr" => Ok(ImportTarget::EntityType(EntityPrefix::Ncr)),
        "capa" => Ok(ImportTarget::EntityType(EntityPrefix::Capa)),
        "quote" | "quot" => Ok(ImportTarget::EntityType(EntityPrefix::Quot)),
        "feat" | "feature" => Ok(ImportTarget::EntityType(EntityPrefix::Feat)),
        _ => Err(format!(
            "Unsupported import target: '{}'. Supported: req, risk, cmp, asm, sup, test, rslt, proc, ctrl, ncr, capa, quote, feat, sysml",
            s
        )),
    }
}

pub fn run(args: ImportArgs) -> Result<()> {
    // Handle template generation
    if args.template {
        let target = args.target.ok_or_else(|| {
            miette::miette!(
                "Entity type required for template generation. Usage: tdt import --template req"
            )
        })?;
        match target {
            ImportTarget::EntityType(entity_type) => return generate_template(entity_type),
            ImportTarget::Sysml => {
                return Err(miette::miette!(
                    "Template generation not available for SysML format"
                ));
            }
        }
    }

    // Require both target and file for import
    let target = args
        .target
        .clone()
        .ok_or_else(|| miette::miette!("Import target required. Usage: tdt import req data.csv  OR  tdt import sysml model.sysml"))?;

    let file_path = args.file.clone().ok_or_else(|| {
        miette::miette!(
            "File required. Usage: tdt import req data.csv  OR  tdt import sysml model.sysml"
        )
    })?;

    if !file_path.exists() {
        return Err(miette::miette!("File not found: {}", file_path.display()));
    }

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Handle SysML import
    if let ImportTarget::Sysml = target {
        println!(
            "{} Importing SysML v2 from {}{}",
            style("->").blue(),
            style(file_path.display()).yellow(),
            if args.dry_run {
                style(" (dry run)").dim().to_string()
            } else {
                String::new()
            }
        );
        println!();

        let stats = sysml::import(&project, &file_path, args.dry_run, args.update)?;

        // Print summary
        println!();
        println!("{}", style("─".repeat(50)).dim());
        println!("{}", style("Import Summary").bold());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Entities parsed:  {}", style(stats.rows_processed).cyan());
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

        if args.dry_run {
            println!();
            println!(
                "{}",
                style("Dry run complete. No files were created.").yellow()
            );
        } else {
            super::utils::sync_cache(&project);
        }

        return Ok(());
    }

    // Standard CSV entity import
    let entity_type = match target {
        ImportTarget::EntityType(et) => et,
        ImportTarget::Sysml => unreachable!(),
    };

    if args.update {
        println!(
            "{} Updating {} entities from {}{}",
            style("->").blue(),
            style(entity_type.as_str()).cyan(),
            style(file_path.display()).yellow(),
            if args.dry_run {
                style(" (dry run)").dim().to_string()
            } else {
                String::new()
            }
        );
        println!();

        let stats = update::import_update(
            &project,
            entity_type,
            &file_path,
            args.dry_run,
            args.skip_errors,
        )?;

        println!();
        println!("{}", style("─".repeat(50)).dim());
        println!("{}", style("Import Summary").bold());
        println!("{}", style("─".repeat(50)).dim());
        println!("  Rows processed:   {}", style(stats.rows_processed).cyan());
        println!(
            "  Entities updated: {}",
            style(stats.entities_updated).green()
        );
        if stats.errors > 0 {
            println!("  Errors:           {}", style(stats.errors).red());
        }
        if args.dry_run {
            println!();
            println!(
                "{}",
                style("Dry run complete. No files were changed.").yellow()
            );
        } else if stats.entities_updated > 0 {
            super::utils::sync_cache(&project);
        }
        return Ok(());
    }

    println!(
        "{} Importing {} entities from {}{}",
        style("->").blue(),
        style(entity_type.as_str()).cyan(),
        style(file_path.display()).yellow(),
        if args.dry_run {
            style(" (dry run)").dim().to_string()
        } else {
            String::new()
        }
    );
    println!();

    // Convert clap args to internal args struct
    let internal_args = common::ImportArgs {
        dry_run: args.dry_run,
        skip_errors: args.skip_errors,
        component: args.component.clone(),
        supplier: args.supplier.clone(),
        test: args.test.clone(),
        process: args.process.clone(),
        assembly: args.assembly.clone(),
    };

    let stats = match entity_type {
        EntityPrefix::Req => req::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Risk => risk::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Cmp => cmp::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Asm => asm::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Sup => sup::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Test => test::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Rslt => rslt::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Proc => proc::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Ctrl => ctrl::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Ncr => ncr::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Capa => capa::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Quot => quote::import(&project, &file_path, &internal_args)?,
        EntityPrefix::Feat => feat::import(&project, &file_path, &internal_args)?,
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
    } else {
        // Sync cache after import (when not dry run)
        super::utils::sync_cache(&project);
    }

    if stats.errors > 0 && !args.skip_errors {
        return Err(miette::miette!(
            "Import completed with {} error(s)",
            stats.errors
        ));
    }

    Ok(())
}
