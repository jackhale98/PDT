//! `tdt search` command - Search across all entity types
//!
//! Provides unified search functionality across all entity types.

use clap::ValueEnum;
use console::style;
use miette::Result;

use crate::cli::helpers::truncate_str;
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct SearchArgs {
    /// Search term (searches in title, description, and notes)
    pub query: String,

    /// Filter by entity type(s)
    #[arg(long, short = 't', value_delimiter = ',')]
    pub entity_type: Option<Vec<EntityTypeFilter>>,

    /// Filter by status
    #[arg(long, short = 's')]
    pub status: Option<String>,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Filter by tag
    #[arg(long)]
    pub tag: Option<String>,

    /// Limit number of results
    #[arg(long, short = 'n', default_value = "50")]
    pub limit: usize,

    /// Show only count
    #[arg(long)]
    pub count: bool,

    /// Case-sensitive search
    #[arg(long, short = 'c')]
    pub case_sensitive: bool,
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
            EntityTypeFilter::Ncr => "NCR",
            EntityTypeFilter::Capa => "CAPA",
            EntityTypeFilter::Quote => "QUOT",
            EntityTypeFilter::Sup => "SUP",
        }
    }
}

/// Run the search command
pub fn run(args: SearchArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Open cache
    let cache = EntityCache::open(&project)?;

    // Get all entity types to search
    let type_prefixes: Option<Vec<&str>> = args
        .entity_type
        .as_ref()
        .map(|types| types.iter().map(|t| t.as_prefix()).collect());

    // Perform search using cache's search_all function
    let results = cache.search_all(
        &args.query,
        type_prefixes.as_deref(),
        args.status.as_deref(),
        args.author.as_deref(),
        args.tag.as_deref(),
        args.case_sensitive,
        args.limit,
    );

    // Count only
    if args.count {
        println!("{}", results.len());
        return Ok(());
    }

    // No results
    if results.is_empty() {
        println!("No results found for '{}'.", style(&args.query).yellow());
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
                        "entity_type": r.entity_type,
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
                    result.entity_type,
                    crate::cli::helpers::escape_csv(&result.title),
                    result.status,
                    result.author
                );
            }
        }
        OutputFormat::Tsv | OutputFormat::Auto | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            println!(
                "{} results for '{}':",
                style(results.len()).cyan(),
                style(&args.query).yellow()
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
                let type_styled = match result.entity_type.as_str() {
                    "REQ" => style(&result.entity_type).blue(),
                    "RISK" => style(&result.entity_type).red(),
                    "TEST" | "RSLT" => style(&result.entity_type).green(),
                    "CMP" | "ASM" => style(&result.entity_type).yellow(),
                    "NCR" | "CAPA" => style(&result.entity_type).magenta(),
                    _ => style(&result.entity_type).white(),
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
                style("<TYPE>@N show").cyan()
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
                    result.entity_type,
                    result.title,
                    result.status
                );
            }
        }
        OutputFormat::Yaml | OutputFormat::Path => {
            // For YAML, output as list
            let yaml_results: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "entity_type": r.entity_type,
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
