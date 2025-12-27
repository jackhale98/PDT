//! `tdt tags` command - Manage and view entity tags
//!
//! Lists all tags in use and shows entities by tag.

use clap::Subcommand;
use console::style;
use miette::Result;

use crate::cli::helpers::truncate_str;
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(Subcommand, Debug)]
pub enum TagsCommands {
    /// List all tags in use across the project
    List(ListArgs),

    /// Show entities with a specific tag
    Show(ShowArgs),
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Show count only (total number of unique tags)
    #[arg(long)]
    pub count: bool,

    /// Limit number of tags to show
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Tag to search for
    pub tag: String,

    /// Limit number of results
    #[arg(long, short = 'n', default_value = "50")]
    pub limit: usize,

    /// Show count only
    #[arg(long)]
    pub count: bool,
}

/// Run the tags command
pub fn run(cmd: TagsCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        TagsCommands::List(args) => run_list(args, global),
        TagsCommands::Show(args) => run_show(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Open cache
    let cache = EntityCache::open(&project)?;

    // Get all tags with counts
    let mut tags = cache.list_all_tags();

    // Apply limit if specified
    if let Some(limit) = args.limit {
        tags.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", tags.len());
        return Ok(());
    }

    // No results
    if tags.is_empty() {
        println!("No tags found in project.");
        println!();
        println!(
            "Add tags with: {}",
            style("tdt bulk add-tag <tag> <entities...>").yellow()
        );
        return Ok(());
    }

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json_results: Vec<serde_json::Value> = tags
                .iter()
                .map(|(tag, count)| {
                    serde_json::json!({
                        "tag": tag,
                        "count": count,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_results).unwrap());
        }
        OutputFormat::Csv => {
            println!("tag,count");
            for (tag, count) in &tags {
                println!("{},{}", crate::cli::helpers::escape_csv(tag), count);
            }
        }
        OutputFormat::Tsv | OutputFormat::Auto => {
            println!("{} unique tags in project:", style(tags.len()).cyan());
            println!();

            // Header
            println!("{:<30} {:<10}", style("TAG").bold(), style("COUNT").bold());
            println!("{}", "-".repeat(42));

            for (tag, count) in &tags {
                println!("{:<30} {:<10}", style(tag).yellow(), count);
            }

            println!();
            println!(
                "Use {} to see entities with a tag.",
                style("tdt tags show <tag>").cyan()
            );
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            // Just output tag names
            for (tag, _) in &tags {
                println!("{}", tag);
            }
        }
        OutputFormat::Md => {
            println!("| Tag | Count |");
            println!("|---|---|");
            for (tag, count) in &tags {
                println!("| {} | {} |", tag, count);
            }
        }
        OutputFormat::Yaml | OutputFormat::Path => {
            let yaml_results: Vec<serde_json::Value> = tags
                .iter()
                .map(|(tag, count)| {
                    serde_json::json!({
                        "tag": tag,
                        "count": count,
                    })
                })
                .collect();
            println!("{}", serde_yml::to_string(&yaml_results).unwrap());
        }
    }

    Ok(())
}

fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Open cache
    let cache = EntityCache::open(&project)?;

    // Get entities with this tag
    let results = cache.list_by_tag(&args.tag, Some(args.limit));

    // Count only
    if args.count {
        println!("{}", results.len());
        return Ok(());
    }

    // No results
    if results.is_empty() {
        println!(
            "No entities found with tag '{}'.",
            style(&args.tag).yellow()
        );
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(results.iter().map(|r| r.id.clone()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.format {
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
                        "tags": r.tags,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_results).unwrap());
        }
        OutputFormat::Csv => {
            println!("short_id,id,type,title,status");
            for result in &results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{}",
                    short_id,
                    result.id,
                    result.prefix,
                    crate::cli::helpers::escape_csv(&result.title),
                    result.status
                );
            }
        }
        OutputFormat::Tsv | OutputFormat::Auto => {
            println!(
                "{} entities with tag '{}':",
                style(results.len()).cyan(),
                style(&args.tag).yellow()
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
                        "tags": r.tags,
                    })
                })
                .collect();
            println!("{}", serde_yml::to_string(&yaml_results).unwrap());
        }
    }

    Ok(())
}
