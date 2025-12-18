//! `tdt diff` command - View git diff for an entity

use console::style;
use miette::Result;
use std::process::Command;

use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct DiffArgs {
    /// Entity ID or short ID to show diff for
    pub id: String,

    /// Git revision or range (e.g., HEAD~1, v1.0..v2.0, abc123)
    #[arg(value_name = "REVISION")]
    pub revision: Option<String>,

    /// Show staged changes only
    #[arg(long)]
    pub staged: bool,

    /// Show word-level diff
    #[arg(long)]
    pub word_diff: bool,

    /// Show stat summary instead of full diff
    #[arg(long)]
    pub stat: bool,

    /// Generate a patch file
    #[arg(long)]
    pub patch: bool,
}

pub fn run(args: DiffArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve the entity ID
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the entity file
    let entity_file = find_entity_file(&project, &resolved_id)?;

    // Build git diff command
    let mut git_args = vec!["diff".to_string()];

    if args.staged {
        git_args.push("--staged".to_string());
    }

    if args.word_diff {
        git_args.push("--word-diff".to_string());
    }

    if args.stat {
        git_args.push("--stat".to_string());
    }

    if args.patch {
        git_args.push("-p".to_string());
    }

    // Add revision if specified
    if let Some(ref rev) = args.revision {
        git_args.push(rev.clone());
    }

    git_args.push("--".to_string());
    git_args.push(entity_file.to_string_lossy().to_string());

    // Print header
    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| resolved_id.clone());
    let rev_desc = args.revision.as_deref().unwrap_or("working copy");
    println!(
        "{} {} ({})\n",
        style("Diff for:").bold(),
        style(&display_id).cyan(),
        rev_desc
    );

    // Execute git command
    let output = Command::new("git")
        .args(&git_args)
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(miette::miette!("Git error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("{}", style("No differences found.").green());
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
