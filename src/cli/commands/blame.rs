//! `tdt blame` command - View git blame for an entity

use console::style;
use miette::Result;
use std::process::Command;

use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct BlameArgs {
    /// Entity ID or short ID to show blame for
    pub id: String,

    /// Show specific line range (start-end, e.g., "10-20")
    #[arg(long, short = 'L')]
    pub lines: Option<String>,

    /// Show commit timestamp instead of author date
    #[arg(long)]
    pub show_email: bool,

    /// Don't show the commit hash
    #[arg(long)]
    pub no_hash: bool,
}

pub fn run(args: BlameArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve the entity ID
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the entity file
    let entity_file = find_entity_file(&project, &resolved_id)?;

    // Build git blame command
    let mut git_args = vec!["blame".to_string()];

    if let Some(ref lines) = args.lines {
        git_args.push(format!("-L{}", lines));
    }

    if args.show_email {
        git_args.push("-e".to_string());
    }

    if args.no_hash {
        git_args.push("-s".to_string());
    }

    git_args.push("--".to_string());
    git_args.push(entity_file.to_string_lossy().to_string());

    // Print header
    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| resolved_id.clone());
    println!(
        "{} {}\n",
        style("Blame for:").bold(),
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
        if stderr.contains("no such path") || stderr.contains("does not exist") {
            return Err(miette::miette!("Entity file not tracked in git yet"));
        }
        return Err(miette::miette!("Git error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("{}", style("No blame information available.").yellow());
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
