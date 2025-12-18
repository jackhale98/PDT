//! `tdt history` command - View git history for an entity

use console::style;
use miette::Result;
use std::process::Command;

use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct HistoryArgs {
    /// Entity ID or short ID to show history for
    pub id: String,

    /// Limit to last N commits
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show commits since date (YYYY-MM-DD)
    #[arg(long)]
    pub since: Option<String>,

    /// Show commits until date (YYYY-MM-DD)
    #[arg(long)]
    pub until: Option<String>,

    /// Show full commit messages (not just oneline)
    #[arg(long)]
    pub full: bool,

    /// Show patch/diff for each commit
    #[arg(long, short = 'p')]
    pub patch: bool,
}

pub fn run(args: HistoryArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve the entity ID
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the entity file
    let entity_file = find_entity_file(&project, &resolved_id)?;

    // Build git log command
    let mut git_args = vec!["log".to_string()];

    if !args.full {
        git_args.push("--oneline".to_string());
    }

    git_args.push("--follow".to_string());

    if let Some(n) = args.limit {
        git_args.push(format!("-{}", n));
    }

    if let Some(ref since) = args.since {
        git_args.push(format!("--since={}", since));
    }

    if let Some(ref until) = args.until {
        git_args.push(format!("--until={}", until));
    }

    if args.patch {
        git_args.push("-p".to_string());
    }

    git_args.push("--".to_string());
    git_args.push(entity_file.to_string_lossy().to_string());

    // Print header
    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| resolved_id.clone());
    println!(
        "{} {}\n",
        style("History for:").bold(),
        style(&display_id).cyan()
    );

    // Execute git command
    let output = Command::new("git")
        .args(&git_args)
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("does not have any commits yet") {
            println!("{}", style("No commits yet for this entity.").yellow());
            return Ok(());
        }
        return Err(miette::miette!("Git error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!(
            "{}",
            style("No history found (file may not be tracked yet).").yellow()
        );
    } else {
        print!("{}", stdout);
    }

    Ok(())
}

fn find_entity_file(project: &Project, id: &str) -> Result<std::path::PathBuf> {
    use crate::core::cache::EntityCache;

    // Try cache-based lookup first (O(1) via SQLite)
    if let Ok(cache) = EntityCache::open(project) {
        // Resolve short ID if needed
        let full_id = if id.contains('@') {
            cache.resolve_short_id(id)
        } else {
            None
        };

        let lookup_id = full_id.as_deref().unwrap_or(id);

        // Try exact match via cache
        if let Some(entity) = cache.get_entity(lookup_id) {
            return Ok(entity.file_path);
        }
    }

    // Fallback: filesystem search
    let search_dirs: Vec<(&str, &str)> = vec![
        ("REQ-", "requirements"),
        ("RISK-", "risks"),
        ("TEST-", "verification"),
        ("RSLT-", "verification"),
        ("CMP-", "bom/components"),
        ("ASM-", "bom/assemblies"),
        ("SUP-", "procurement/suppliers"),
        ("QUOTE-", "procurement/quotes"),
        ("PROC-", "manufacturing/processes"),
        ("CTRL-", "manufacturing/controls"),
        ("WORK-", "manufacturing/work_instructions"),
        ("NCR-", "manufacturing/ncrs"),
        ("CAPA-", "manufacturing/capas"),
        ("FEAT-", "tolerances/features"),
        ("MATE-", "tolerances/mates"),
        ("TOL-", "tolerances/stackups"),
    ];

    for (prefix, base_dir) in &search_dirs {
        if id.starts_with(prefix) {
            // Search recursively in the base directory
            let dir = project.root().join(base_dir);
            if dir.exists() {
                for entry in walkdir::WalkDir::new(&dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
                {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if content.contains(&format!("id: {}", id))
                            || content.contains(&format!("id: \"{}\"", id))
                        {
                            return Ok(entry.path().to_path_buf());
                        }
                    }
                }
            }
        }
    }

    Err(miette::miette!("Could not find entity file for ID: {}", id))
}
