//! `tdt recent` command - Show recently modified entities
//!
//! Lists entities by file modification time across all types.

use clap::ValueEnum;
use console::style;
use miette::Result;

use crate::cli::helpers::truncate_str;
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct RecentArgs {
    /// Filter by entity type(s)
    #[arg(long, short = 't', value_delimiter = ',')]
    pub entity_type: Option<Vec<EntityTypeFilter>>,

    /// Limit number of results
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,

    /// Show count only
    #[arg(long)]
    pub count: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum EntityTypeFilter {
    Req,
    Risk,
    Test,
    Rslt,
    Cmp,
    Asm,
    Feat,
    Mate,
    Tol,
    Proc,
    Ctrl,
    Work,
    Lot,
    Dev,
    Ncr,
    Capa,
    Quote,
    Sup,
}

impl EntityTypeFilter {
    fn as_prefix(&self) -> &'static str {
        match self {
            EntityTypeFilter::Req => "REQ",
            EntityTypeFilter::Risk => "RISK",
            EntityTypeFilter::Test => "TEST",
            EntityTypeFilter::Rslt => "RSLT",
            EntityTypeFilter::Cmp => "CMP",
            EntityTypeFilter::Asm => "ASM",
            EntityTypeFilter::Feat => "FEAT",
            EntityTypeFilter::Mate => "MATE",
            EntityTypeFilter::Tol => "TOL",
            EntityTypeFilter::Proc => "PROC",
            EntityTypeFilter::Ctrl => "CTRL",
            EntityTypeFilter::Work => "WORK",
            EntityTypeFilter::Lot => "LOT",
            EntityTypeFilter::Dev => "DEV",
            EntityTypeFilter::Ncr => "NCR",
            EntityTypeFilter::Capa => "CAPA",
            EntityTypeFilter::Quote => "QUOT",
            EntityTypeFilter::Sup => "SUP",
        }
    }
}

/// Run the recent command
pub fn run(args: RecentArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Open cache
    let cache = EntityCache::open(&project)?;

    // Get type prefixes filter
    let type_prefixes: Option<Vec<&str>> = args
        .entity_type
        .as_ref()
        .map(|types| types.iter().map(|t| t.as_prefix()).collect());

    // Query recent entities
    let results = cache.list_recent(type_prefixes.as_deref(), args.limit);

    // Count only
    if args.count {
        println!("{}", results.len());
        return Ok(());
    }

    // No results
    if results.is_empty() {
        println!("No recent activity found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(results.iter().map(|r| r.id.clone()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json_results: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "entity_type": r.prefix,
                        "title": r.title,
                        "status": r.status,
                        "author": r.author,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_results).unwrap());
        }
        OutputFormat::Csv => {
            println!("short_id,id,type,title,status,author");
            for result in &results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{}",
                    short_id,
                    result.id,
                    result.prefix,
                    crate::cli::helpers::escape_csv(&result.title),
                    result.status,
                    result.author
                );
            }
        }
        OutputFormat::Tsv | OutputFormat::Auto | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            println!(
                "{} recently modified entities:",
                style(results.len()).cyan()
            );
            println!();

            // Header
            println!(
                "{:<10} {:<17} {:<6} {:<35} {:<10}",
                style("SHORT").bold().dim(),
                style("ID").bold(),
                style("TYPE").bold(),
                style("TITLE").bold(),
                style("STATUS").bold()
            );
            println!("{}", "-".repeat(85));

            for result in &results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                let type_styled = match result.prefix.as_str() {
                    "REQ" => style(&result.prefix).blue(),
                    "RISK" => style(&result.prefix).red(),
                    "TEST" | "RSLT" => style(&result.prefix).green(),
                    "CMP" | "ASM" => style(&result.prefix).yellow(),
                    "NCR" | "CAPA" => style(&result.prefix).magenta(),
                    "LOT" | "DEV" => style(&result.prefix).cyan(),
                    _ => style(&result.prefix).white(),
                };

                println!(
                    "{:<10} {:<17} {:<6} {:<35} {:<10}",
                    style(&short_id).cyan(),
                    truncate_str(&result.id, 15),
                    type_styled,
                    truncate_str(&result.title, 33),
                    result.status
                );
            }

            println!();
            println!(
                "Use {} to show entity details.",
                style("<TYPE> show <ID>").cyan()
            );
        }
        OutputFormat::Id => {
            for result in &results {
                println!("{}", result.id);
            }
        }
        OutputFormat::ShortId => {
            for result in &results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                println!("{}", short_id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Type | Title | Status |");
            println!("|---|---|---|---|---|");
            for result in &results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&result.id, 15),
                    result.prefix,
                    result.title,
                    result.status
                );
            }
        }
        OutputFormat::Yaml | OutputFormat::Path => {
            let yaml_results: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "entity_type": r.prefix,
                        "title": r.title,
                        "status": r.status,
                        "author": r.author,
                    })
                })
                .collect();
            println!("{}", serde_yml::to_string(&yaml_results).unwrap());
        }
    }

    Ok(())
}
