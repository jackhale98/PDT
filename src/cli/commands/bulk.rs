//! `tdt bulk` command - Bulk operations on entities

use clap::Subcommand;
use console::style;
use miette::Result;
use std::path::PathBuf;

use crate::cli::helpers::read_ids_from_stdin;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(Subcommand, Debug)]
pub enum BulkCommands {
    /// Set status on multiple entities
    SetStatus(SetStatusArgs),

    /// Add tag to multiple entities
    AddTag(AddTagArgs),

    /// Remove tag from multiple entities
    RemoveTag(RemoveTagArgs),

    /// Set author on multiple entities
    SetAuthor(SetAuthorArgs),
}

#[derive(clap::Args, Debug)]
pub struct SetStatusArgs {
    /// New status value (draft, review, approved, obsolete)
    pub status: String,

    /// Entity IDs or short IDs to update (also reads from stdin if piped)
    pub entities: Vec<String>,

    /// Select all entities of this type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Select entities with this tag
    #[arg(long)]
    pub tag: Option<String>,

    /// Show what would change without making changes
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::Args, Debug)]
pub struct AddTagArgs {
    /// Tag to add
    pub tag: String,

    /// Entity IDs or short IDs to update (also reads from stdin if piped)
    pub entities: Vec<String>,

    /// Select all entities of this type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Select entities with this status
    #[arg(long)]
    pub status: Option<String>,

    /// Show what would change without making changes
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::Args, Debug)]
pub struct RemoveTagArgs {
    /// Tag to remove
    pub tag: String,

    /// Entity IDs or short IDs to update (also reads from stdin if piped)
    pub entities: Vec<String>,

    /// Select all entities of this type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Remove tag from all entities that have it
    #[arg(long)]
    pub all: bool,

    /// Show what would change without making changes
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::Args, Debug)]
pub struct SetAuthorArgs {
    /// New author value
    pub author: String,

    /// Entity IDs or short IDs to update (also reads from stdin if piped)
    pub entities: Vec<String>,

    /// Select all entities of this type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Select entities with this current author
    #[arg(long)]
    pub current_author: Option<String>,

    /// Show what would change without making changes
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(cmd: BulkCommands) -> Result<()> {
    match cmd {
        BulkCommands::SetStatus(args) => run_set_status(args),
        BulkCommands::AddTag(args) => run_add_tag(args),
        BulkCommands::RemoveTag(args) => run_remove_tag(args),
        BulkCommands::SetAuthor(args) => run_set_author(args),
    }
}

fn run_set_status(mut args: SetStatusArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Read from stdin if available (Unix pipeline support)
    if let Some(stdin_ids) = read_ids_from_stdin() {
        args.entities.extend(stdin_ids);
    }

    // Validate we have some way to select entities
    if args.entities.is_empty() && args.entity_type.is_none() && args.tag.is_none() {
        return Err(miette::miette!(
            "No entities specified. Provide IDs as arguments, pipe them via stdin, or use --entity-type/--tag filters."
        ));
    }

    // Validate status value
    let valid_statuses = ["draft", "review", "approved", "obsolete"];
    if !valid_statuses.contains(&args.status.to_lowercase().as_str()) {
        return Err(miette::miette!(
            "Invalid status '{}'. Valid values: {}",
            args.status,
            valid_statuses.join(", ")
        ));
    }

    // Collect entity files to update
    let entity_files = collect_entity_files(
        &project,
        &short_ids,
        &args.entities,
        args.entity_type.as_deref(),
        args.tag.as_deref(),
        None,
    )?;

    if entity_files.is_empty() {
        println!("{}", style("No matching entities found.").yellow());
        return Ok(());
    }

    println!(
        "{} {} to status '{}'",
        if args.dry_run {
            style("Would update").yellow()
        } else {
            style("Updating").green()
        },
        style(entity_files.len()).cyan(),
        style(&args.status).cyan()
    );

    if args.dry_run {
        println!("\n{}", style("Entities that would be updated:").bold());
    }

    let mut updated = 0;
    let mut errors = 0;

    for (file_path, entity_id) in &entity_files {
        let display_id = short_ids
            .get_short_id(entity_id)
            .unwrap_or_else(|| entity_id.clone());

        if args.dry_run {
            println!("  {} {}", style("*").dim(), style(&display_id).cyan());
            continue;
        }

        match update_yaml_field(file_path, "status", &args.status.to_lowercase()) {
            Ok(_) => {
                println!(
                    "  {} {}",
                    style("Updated:").green(),
                    style(&display_id).cyan()
                );
                updated += 1;
            }
            Err(e) => {
                println!(
                    "  {} {} - {}",
                    style("Error:").red(),
                    style(&display_id).cyan(),
                    e
                );
                errors += 1;
            }
        }
    }

    if !args.dry_run {
        println!(
            "\n{} updated, {} errors",
            style(updated).green(),
            style(errors).red()
        );
    }

    Ok(())
}

fn run_add_tag(mut args: AddTagArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Read from stdin if available (Unix pipeline support)
    if let Some(stdin_ids) = read_ids_from_stdin() {
        args.entities.extend(stdin_ids);
    }

    // Validate we have some way to select entities
    if args.entities.is_empty() && args.entity_type.is_none() && args.status.is_none() {
        return Err(miette::miette!(
            "No entities specified. Provide IDs as arguments, pipe them via stdin, or use --entity-type/--status filters."
        ));
    }

    // Collect entity files to update
    let entity_files = collect_entity_files(
        &project,
        &short_ids,
        &args.entities,
        args.entity_type.as_deref(),
        None,
        args.status.as_deref(),
    )?;

    if entity_files.is_empty() {
        println!("{}", style("No matching entities found.").yellow());
        return Ok(());
    }

    println!(
        "{} tag '{}' to {} entities",
        if args.dry_run {
            style("Would add").yellow()
        } else {
            style("Adding").green()
        },
        style(&args.tag).cyan(),
        style(entity_files.len()).cyan()
    );

    if args.dry_run {
        println!("\n{}", style("Entities that would be updated:").bold());
    }

    let mut updated = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for (file_path, entity_id) in &entity_files {
        let display_id = short_ids
            .get_short_id(entity_id)
            .unwrap_or_else(|| entity_id.clone());

        if args.dry_run {
            println!("  {} {}", style("*").dim(), style(&display_id).cyan());
            continue;
        }

        match add_tag_to_file(file_path, &args.tag) {
            Ok(true) => {
                println!(
                    "  {} {}",
                    style("Updated:").green(),
                    style(&display_id).cyan()
                );
                updated += 1;
            }
            Ok(false) => {
                println!(
                    "  {} {} (already has tag)",
                    style("Skipped:").dim(),
                    style(&display_id).cyan()
                );
                skipped += 1;
            }
            Err(e) => {
                println!(
                    "  {} {} - {}",
                    style("Error:").red(),
                    style(&display_id).cyan(),
                    e
                );
                errors += 1;
            }
        }
    }

    if !args.dry_run {
        println!(
            "\n{} updated, {} skipped, {} errors",
            style(updated).green(),
            style(skipped).dim(),
            style(errors).red()
        );
    }

    Ok(())
}

fn run_remove_tag(mut args: RemoveTagArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Read from stdin if available (Unix pipeline support)
    if let Some(stdin_ids) = read_ids_from_stdin() {
        args.entities.extend(stdin_ids);
    }

    // Validate we have some way to select entities
    if args.entities.is_empty() && args.entity_type.is_none() && !args.all {
        return Err(miette::miette!(
            "No entities specified. Provide IDs as arguments, pipe them via stdin, or use --entity-type/--all filters."
        ));
    }

    // Collect entity files
    let entity_files = if args.all {
        // Find all entities with the tag
        collect_entities_with_tag(&project, &args.tag, args.entity_type.as_deref())?
    } else {
        collect_entity_files(
            &project,
            &short_ids,
            &args.entities,
            args.entity_type.as_deref(),
            Some(&args.tag),
            None,
        )?
    };

    if entity_files.is_empty() {
        println!("{}", style("No matching entities found.").yellow());
        return Ok(());
    }

    println!(
        "{} tag '{}' from {} entities",
        if args.dry_run {
            style("Would remove").yellow()
        } else {
            style("Removing").green()
        },
        style(&args.tag).cyan(),
        style(entity_files.len()).cyan()
    );

    if args.dry_run {
        println!("\n{}", style("Entities that would be updated:").bold());
    }

    let mut updated = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for (file_path, entity_id) in &entity_files {
        let display_id = short_ids
            .get_short_id(entity_id)
            .unwrap_or_else(|| entity_id.clone());

        if args.dry_run {
            println!("  {} {}", style("*").dim(), style(&display_id).cyan());
            continue;
        }

        match remove_tag_from_file(file_path, &args.tag) {
            Ok(true) => {
                println!(
                    "  {} {}",
                    style("Updated:").green(),
                    style(&display_id).cyan()
                );
                updated += 1;
            }
            Ok(false) => {
                println!(
                    "  {} {} (tag not present)",
                    style("Skipped:").dim(),
                    style(&display_id).cyan()
                );
                skipped += 1;
            }
            Err(e) => {
                println!(
                    "  {} {} - {}",
                    style("Error:").red(),
                    style(&display_id).cyan(),
                    e
                );
                errors += 1;
            }
        }
    }

    if !args.dry_run {
        println!(
            "\n{} updated, {} skipped, {} errors",
            style(updated).green(),
            style(skipped).dim(),
            style(errors).red()
        );
    }

    Ok(())
}

fn run_set_author(mut args: SetAuthorArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Read from stdin if available (Unix pipeline support)
    if let Some(stdin_ids) = read_ids_from_stdin() {
        args.entities.extend(stdin_ids);
    }

    // Validate we have some way to select entities
    if args.entities.is_empty() && args.entity_type.is_none() && args.current_author.is_none() {
        return Err(miette::miette!(
            "No entities specified. Provide IDs as arguments, pipe them via stdin, or use --entity-type/--current-author filters."
        ));
    }

    // Collect entity files
    let entity_files = if let Some(ref current) = args.current_author {
        collect_entities_with_author(&project, current, args.entity_type.as_deref())?
    } else {
        collect_entity_files(
            &project,
            &short_ids,
            &args.entities,
            args.entity_type.as_deref(),
            None,
            None,
        )?
    };

    if entity_files.is_empty() {
        println!("{}", style("No matching entities found.").yellow());
        return Ok(());
    }

    println!(
        "{} author to '{}' on {} entities",
        if args.dry_run {
            style("Would set").yellow()
        } else {
            style("Setting").green()
        },
        style(&args.author).cyan(),
        style(entity_files.len()).cyan()
    );

    if args.dry_run {
        println!("\n{}", style("Entities that would be updated:").bold());
    }

    let mut updated = 0;
    let mut errors = 0;

    for (file_path, entity_id) in &entity_files {
        let display_id = short_ids
            .get_short_id(entity_id)
            .unwrap_or_else(|| entity_id.clone());

        if args.dry_run {
            println!("  {} {}", style("*").dim(), style(&display_id).cyan());
            continue;
        }

        match update_yaml_field(file_path, "author", &args.author) {
            Ok(_) => {
                println!(
                    "  {} {}",
                    style("Updated:").green(),
                    style(&display_id).cyan()
                );
                updated += 1;
            }
            Err(e) => {
                println!(
                    "  {} {} - {}",
                    style("Error:").red(),
                    style(&display_id).cyan(),
                    e
                );
                errors += 1;
            }
        }
    }

    if !args.dry_run {
        println!(
            "\n{} updated, {} errors",
            style(updated).green(),
            style(errors).red()
        );
    }

    Ok(())
}

// Helper functions

fn collect_entity_files(
    project: &Project,
    short_ids: &ShortIdIndex,
    explicit_ids: &[String],
    entity_type: Option<&str>,
    filter_tag: Option<&str>,
    filter_status: Option<&str>,
) -> Result<Vec<(PathBuf, String)>> {
    let mut results = Vec::new();

    if !explicit_ids.is_empty() {
        // Resolve explicit IDs
        for id in explicit_ids {
            let resolved = short_ids.resolve(id).unwrap_or_else(|| id.clone());
            if let Some(file) = find_entity_file(project, &resolved) {
                results.push((file, resolved));
            } else {
                println!(
                    "{} Could not find entity: {}",
                    style("Warning:").yellow(),
                    id
                );
            }
        }
    } else if let Some(etype) = entity_type {
        // Collect all entities of type
        let dirs = entity_type_dirs(etype);
        for dir_path in dirs {
            let dir = project.root().join(dir_path);
            if !dir.exists() {
                continue;
            }

            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Some(id) = extract_id_from_content(&content) {
                        // Apply filters
                        if let Some(tag) = filter_tag {
                            if !content.contains(&format!("- {}", tag))
                                && !content.contains(&format!("\"{}\"", tag))
                            {
                                continue;
                            }
                        }
                        if let Some(status) = filter_status {
                            if !content.contains(&format!("status: {}", status)) {
                                continue;
                            }
                        }
                        results.push((entry.path().to_path_buf(), id));
                    }
                }
            }
        }
    }

    Ok(results)
}

fn collect_entities_with_tag(
    project: &Project,
    tag: &str,
    entity_type: Option<&str>,
) -> Result<Vec<(PathBuf, String)>> {
    let mut results = Vec::new();

    let dirs: Vec<&str> = if let Some(etype) = entity_type {
        entity_type_dirs(etype)
    } else {
        vec![
            "requirements",
            "risks",
            "verification",
            "validation",
            "bom",
            "procurement",
            "manufacturing",
            "tolerances",
        ]
    };

    for dir_path in dirs {
        let dir = project.root().join(dir_path);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                // Check if file has the tag
                if content.contains(&format!("- {}", tag))
                    || content.contains(&format!("\"{}\"", tag))
                {
                    if let Some(id) = extract_id_from_content(&content) {
                        results.push((entry.path().to_path_buf(), id));
                    }
                }
            }
        }
    }

    Ok(results)
}

fn collect_entities_with_author(
    project: &Project,
    author: &str,
    entity_type: Option<&str>,
) -> Result<Vec<(PathBuf, String)>> {
    let mut results = Vec::new();

    let dirs: Vec<&str> = if let Some(etype) = entity_type {
        entity_type_dirs(etype)
    } else {
        vec![
            "requirements",
            "risks",
            "verification",
            "validation",
            "bom",
            "procurement",
            "manufacturing",
            "tolerances",
        ]
    };

    for dir_path in dirs {
        let dir = project.root().join(dir_path);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains(&format!("author: {}", author))
                    || content.contains(&format!("author: \"{}\"", author))
                {
                    if let Some(id) = extract_id_from_content(&content) {
                        results.push((entry.path().to_path_buf(), id));
                    }
                }
            }
        }
    }

    Ok(results)
}

fn entity_type_dirs(entity_type: &str) -> Vec<&'static str> {
    match entity_type.to_lowercase().as_str() {
        "req" | "requirement" => vec!["requirements/inputs", "requirements/outputs"],
        "risk" => vec!["risks/design", "risks/process"],
        "test" => vec!["verification/protocols", "validation/protocols"],
        "rslt" | "result" => vec!["verification/results", "validation/results"],
        "cmp" | "component" => vec!["bom/components"],
        "asm" | "assembly" => vec!["bom/assemblies"],
        "sup" | "supplier" => vec!["procurement/suppliers"],
        "quote" => vec!["procurement/quotes"],
        "proc" | "process" => vec!["manufacturing/processes"],
        "ctrl" | "control" => vec!["manufacturing/controls"],
        "work" => vec!["manufacturing/work_instructions"],
        "ncr" => vec!["manufacturing/ncrs"],
        "capa" => vec!["manufacturing/capas"],
        "feat" | "feature" => vec!["tolerances/features"],
        "mate" => vec!["tolerances/mates"],
        "tol" | "stackup" => vec!["tolerances/stackups"],
        _ => vec![],
    }
}

fn find_entity_file(project: &Project, id: &str) -> Option<PathBuf> {
    // Determine entity type from ID prefix
    let prefix = id.split('-').next()?;

    let dirs = match prefix {
        "REQ" => vec!["requirements/inputs", "requirements/outputs"],
        "RISK" => vec!["risks/design", "risks/process"],
        "TEST" => vec!["verification/protocols", "validation/protocols"],
        "RSLT" => vec!["verification/results", "validation/results"],
        "CMP" => vec!["bom/components"],
        "ASM" => vec!["bom/assemblies"],
        "SUP" => vec!["procurement/suppliers"],
        "QUOTE" => vec!["procurement/quotes"],
        "PROC" => vec!["manufacturing/processes"],
        "CTRL" => vec!["manufacturing/controls"],
        "WORK" => vec!["manufacturing/work_instructions"],
        "NCR" => vec!["manufacturing/ncrs"],
        "CAPA" => vec!["manufacturing/capas"],
        "FEAT" => vec!["tolerances/features"],
        "MATE" => vec!["tolerances/mates"],
        "TOL" => vec!["tolerances/stackups"],
        _ => return None,
    };

    for dir_path in dirs {
        let dir = project.root().join(dir_path);
        if !dir.exists() {
            continue;
        }

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
                    return Some(entry.path().to_path_buf());
                }
            }
        }
    }

    None
}

fn extract_id_from_content(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.starts_with("id:") {
            let id = line.trim_start_matches("id:").trim();
            return Some(id.trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn update_yaml_field(file_path: &PathBuf, field: &str, value: &str) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| miette::miette!("Failed to read file: {}", e))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut found = false;

    for line in &mut lines {
        if line.starts_with(&format!("{}:", field)) {
            *line = format!("{}: {}", field, value);
            found = true;
            break;
        }
    }

    if !found {
        return Err(miette::miette!("Field '{}' not found in file", field));
    }

    let new_content = lines.join("\n") + "\n";
    std::fs::write(file_path, new_content)
        .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

    Ok(())
}

fn add_tag_to_file(file_path: &PathBuf, tag: &str) -> Result<bool> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| miette::miette!("Failed to read file: {}", e))?;

    // Check if tag already exists
    if content.contains(&format!("- {}", tag)) || content.contains(&format!("\"{}\"", tag)) {
        return Ok(false);
    }

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut in_tags = false;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("tags:") {
            if line.trim() == "tags: []" {
                // Empty tags array - replace
                lines[i] = "tags:".to_string();
                lines.insert(i + 1, format!("  - {}", tag));
                let new_content = lines.join("\n") + "\n";
                std::fs::write(file_path, new_content)
                    .map_err(|e| miette::miette!("Failed to write file: {}", e))?;
                return Ok(true);
            }
            in_tags = true;
        } else if in_tags {
            if line.starts_with("  - ") {
                // Continue in tags list
            } else if !line.starts_with("  ") && !line.is_empty() {
                // End of tags section - insert before this line
                lines.insert(i, format!("  - {}", tag));
                let new_content = lines.join("\n") + "\n";
                std::fs::write(file_path, new_content)
                    .map_err(|e| miette::miette!("Failed to write file: {}", e))?;
                return Ok(true);
            }
        }
    }

    // If tags section exists but we reached end of file
    if in_tags {
        lines.push(format!("  - {}", tag));
        let new_content = lines.join("\n") + "\n";
        std::fs::write(file_path, new_content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;
        return Ok(true);
    }

    // No tags section - add one
    // Find a good place to insert (after title or description)
    let insert_idx = lines
        .iter()
        .position(|l| l.starts_with("description:"))
        .or_else(|| lines.iter().position(|l| l.starts_with("title:")))
        .map(|i| i + 1)
        .unwrap_or(lines.len());

    lines.insert(insert_idx, "tags:".to_string());
    lines.insert(insert_idx + 1, format!("  - {}", tag));

    let new_content = lines.join("\n") + "\n";
    std::fs::write(file_path, new_content)
        .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

    Ok(true)
}

fn remove_tag_from_file(file_path: &PathBuf, tag: &str) -> Result<bool> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| miette::miette!("Failed to read file: {}", e))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let original_len = lines.len();

    // Find and remove the tag line
    lines.retain(|line| {
        let trimmed = line.trim();
        !(trimmed == format!("- {}", tag)
            || trimmed == format!("- \"{}\"", tag)
            || trimmed == format!("- '{}'", tag))
    });

    if lines.len() == original_len {
        return Ok(false);
    }

    let new_content = lines.join("\n") + "\n";
    std::fs::write(file_path, new_content)
        .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

    Ok(true)
}
