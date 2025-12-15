//! `tdt baseline` command - Baseline management (git tags with validation)

use clap::Subcommand;
use console::style;
use miette::Result;
use std::process::Command;

use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(Subcommand, Debug)]
pub enum BaselineCommands {
    /// Create a new baseline (validates then creates git tag)
    Create(CreateArgs),

    /// Compare two baselines
    Compare(CompareArgs),

    /// List entities changed since a baseline
    Changed(ChangedArgs),

    /// List all baselines (TDT-prefixed git tags)
    List(ListArgs),
}

#[derive(clap::Args, Debug)]
pub struct CreateArgs {
    /// Baseline name (will be prefixed with 'tdt-' if not already)
    pub name: String,

    /// Message for the baseline tag
    #[arg(long, short = 'm')]
    pub message: Option<String>,

    /// Skip validation before creating baseline
    #[arg(long)]
    pub skip_validation: bool,

    /// Force creation even if validation fails (not recommended)
    #[arg(long)]
    pub force: bool,
}

#[derive(clap::Args, Debug)]
pub struct CompareArgs {
    /// First baseline (older)
    pub baseline1: String,

    /// Second baseline (newer, defaults to HEAD)
    pub baseline2: Option<String>,

    /// Show only entity IDs, not filenames
    #[arg(long)]
    pub ids_only: bool,

    /// Show full diff for modified files
    #[arg(long, short = 'd')]
    pub diff: bool,
}

#[derive(clap::Args, Debug)]
pub struct ChangedArgs {
    /// Baseline to compare against
    pub since: String,

    /// Show only specific entity types (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Show only entity IDs, not filenames
    #[arg(long)]
    pub ids_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Show all git tags, not just TDT baselines
    #[arg(long)]
    pub all: bool,

    /// Show tag messages
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

pub fn run(cmd: BaselineCommands) -> Result<()> {
    match cmd {
        BaselineCommands::Create(args) => run_create(args),
        BaselineCommands::Compare(args) => run_compare(args),
        BaselineCommands::Changed(args) => run_changed(args),
        BaselineCommands::List(args) => run_list(args),
    }
}

fn run_create(args: CreateArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Normalize tag name
    let tag_name = if args.name.starts_with("tdt-") {
        args.name.clone()
    } else {
        format!("tdt-{}", args.name)
    };

    // Check for uncommitted changes
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git status: {}", e))?;

    let has_uncommitted = !status_output.stdout.is_empty();
    if has_uncommitted {
        println!(
            "{}",
            style("Warning: There are uncommitted changes.").yellow()
        );
        if !args.force {
            return Err(miette::miette!(
                "Commit or stash changes before creating a baseline. Use --force to override."
            ));
        }
    }

    // Run validation unless skipped
    if !args.skip_validation {
        println!("{}", style("Running validation...").dim());
        let validate_result =
            crate::cli::commands::validate::run(crate::cli::commands::validate::ValidateArgs {
                paths: vec![],
                entity_type: None,
                strict: false,
                staged: false,
                fail_fast: false,
                summary: false,
                fix: false,
            });

        if let Err(e) = validate_result {
            println!("{} {}", style("Validation failed:").red().bold(), e);
            if !args.force {
                return Err(miette::miette!("Fix validation errors before creating baseline. Use --force to override (not recommended)."));
            }
            println!(
                "{}",
                style("Proceeding anyway due to --force flag.").yellow()
            );
        } else {
            println!("{}", style("Validation passed.").green());
        }
    }

    // Create annotated tag
    let message = args
        .message
        .unwrap_or_else(|| format!("TDT Baseline: {}", tag_name));

    let tag_output = Command::new("git")
        .args(["tag", "-a", &tag_name, "-m", &message])
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to create git tag: {}", e))?;

    if !tag_output.status.success() {
        let stderr = String::from_utf8_lossy(&tag_output.stderr);
        return Err(miette::miette!("Failed to create tag: {}", stderr));
    }

    println!(
        "\n{} {}",
        style("Created baseline:").green().bold(),
        style(&tag_name).cyan()
    );
    println!("{}", style("Push with: git push origin --tags").dim());

    Ok(())
}

fn run_compare(args: CompareArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Normalize baseline names
    let baseline1 = normalize_baseline_name(&args.baseline1);
    let baseline2 = args
        .baseline2
        .map(|b| normalize_baseline_name(&b))
        .unwrap_or_else(|| "HEAD".to_string());

    println!(
        "{} {} .. {}\n",
        style("Comparing:").bold(),
        style(&baseline1).cyan(),
        style(&baseline2).cyan()
    );

    // Get changed files
    let output = Command::new("git")
        .args([
            "diff",
            "--name-status",
            &format!("{}..{}", baseline1, baseline2),
            "--",
            "*.tdt.yaml",
        ])
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git diff: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(miette::miette!("Git error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("{}", style("No entity changes between baselines.").green());
        return Ok(());
    }

    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    let mut modified_files: Vec<String> = Vec::new();

    println!(
        "{:<8} {:<12} {}",
        style("STATUS").bold(),
        style("ID").bold(),
        style("FILE").bold()
    );
    println!("{}", "-".repeat(70));

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let status = parts[0];
            let file = parts[1];

            let (_status_str, status_style) = match status {
                "A" => {
                    added += 1;
                    ("Added", style("Added").green())
                }
                "M" => {
                    modified += 1;
                    modified_files.push(file.to_string());
                    ("Modified", style("Modified").yellow())
                }
                "D" => {
                    deleted += 1;
                    ("Deleted", style("Deleted").red())
                }
                _ => ("Changed", style("Changed").dim()),
            };

            if args.ids_only {
                // Try to extract entity ID from file content
                if let Some(id) = extract_entity_id(&project, file) {
                    let short = short_ids.get_short_id(&id).unwrap_or(id);
                    println!("{}", short);
                }
            } else {
                let id = extract_entity_id(&project, file)
                    .and_then(|id| short_ids.get_short_id(&id).or(Some(id)))
                    .unwrap_or_else(|| "-".to_string());

                println!("{:<8} {:<12} {}", status_style, style(&id).cyan(), file);
            }
        }
    }

    if !args.ids_only {
        println!(
            "\n{} {} added, {} modified, {} deleted",
            style("Summary:").bold(),
            style(added).green(),
            style(modified).yellow(),
            style(deleted).red()
        );
    }

    // Show diffs for modified files if requested
    if args.diff && !modified_files.is_empty() {
        println!("\n{}", style("═".repeat(70)).dim());
        println!("{}\n", style("Diffs for Modified Files:").bold());

        for file in &modified_files {
            let id = extract_entity_id(&project, file)
                .and_then(|id| short_ids.get_short_id(&id).or(Some(id)))
                .unwrap_or_else(|| "-".to_string());

            println!(
                "{} {} ({})",
                style("───").dim(),
                style(&id).cyan().bold(),
                style(file).dim()
            );

            let diff_output = Command::new("git")
                .args(["diff", &format!("{}..{}", baseline1, baseline2), "--", file])
                .current_dir(project.root())
                .output();

            if let Ok(output) = diff_output {
                let diff_text = String::from_utf8_lossy(&output.stdout);
                // Skip the header lines and show just the content diff
                let mut in_content = false;
                for line in diff_text.lines() {
                    if line.starts_with("@@") {
                        in_content = true;
                        println!("{}", style(line).dim());
                    } else if in_content {
                        if line.starts_with('+') && !line.starts_with("+++") {
                            println!("{}", style(line).green());
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            println!("{}", style(line).red());
                        } else {
                            println!("{}", line);
                        }
                    }
                }
            }
            println!();
        }
    }

    Ok(())
}

fn run_changed(args: ChangedArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    let baseline = normalize_baseline_name(&args.since);

    println!(
        "{} {}\n",
        style("Changed since:").bold(),
        style(&baseline).cyan()
    );

    // Build glob pattern based on entity type filter
    let glob_pattern = if let Some(ref entity_type) = args.entity_type {
        let prefix = entity_type_to_prefix(entity_type);
        format!("**/*{}*.tdt.yaml", prefix.to_lowercase())
    } else {
        "*.tdt.yaml".to_string()
    };

    // Get changed files
    let output = Command::new("git")
        .args([
            "diff",
            "--name-only",
            &format!("{}..HEAD", baseline),
            "--",
            &glob_pattern,
        ])
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git diff: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(miette::miette!("Git error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("{}", style("No changes since baseline.").green());
        return Ok(());
    }

    let mut count = 0;
    for line in stdout.lines() {
        let file = line.trim();
        if file.is_empty() {
            continue;
        }

        count += 1;

        if args.ids_only {
            if let Some(id) = extract_entity_id(&project, file) {
                let short = short_ids.get_short_id(&id).unwrap_or(id);
                println!("{}", short);
            }
        } else {
            let id = extract_entity_id(&project, file)
                .and_then(|id| short_ids.get_short_id(&id).or(Some(id)))
                .unwrap_or_else(|| "-".to_string());

            println!("{:<12} {}", style(&id).cyan(), file);
        }
    }

    if !args.ids_only {
        println!("\n{} entities changed.", style(count).cyan());
    }

    Ok(())
}

fn run_list(args: ListArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    println!("{}\n", style("TDT Baselines:").bold());

    // List tags
    let mut git_args = vec!["tag", "-l"];

    if !args.all {
        git_args.push("tdt-*");
    }

    if args.verbose {
        git_args.push("-n1"); // Show first line of annotation
    }

    let output = Command::new("git")
        .args(&git_args)
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git tag: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(miette::miette!("Git error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        if args.all {
            println!("{}", style("No git tags found.").yellow());
        } else {
            println!("{}", style("No TDT baselines found.").yellow());
            println!(
                "{}",
                style("Create one with: tdt baseline create <name>").dim()
            );
        }
        return Ok(());
    }

    // Get tag details with dates
    for tag in stdout.lines() {
        if tag.trim().is_empty() {
            continue;
        }

        // Get tag date
        let date_output = Command::new("git")
            .args(["log", "-1", "--format=%ci", tag])
            .current_dir(project.root())
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let date_short = date_output.split(' ').next().unwrap_or("");

        if args.verbose {
            // Get tag message
            let msg_output = Command::new("git")
                .args(["tag", "-l", "-n1", tag])
                .current_dir(project.root())
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();

            let msg = msg_output
                .split_once(' ')
                .map(|(_, m)| m.trim())
                .unwrap_or("");

            println!("{:<20} {:<12} {}", style(tag).cyan(), date_short, msg);
        } else {
            println!("{:<20} {}", style(tag).cyan(), date_short);
        }
    }

    Ok(())
}

fn normalize_baseline_name(name: &str) -> String {
    if name.starts_with("tdt-") || name == "HEAD" || name.contains("..") {
        name.to_string()
    } else {
        format!("tdt-{}", name)
    }
}

fn entity_type_to_prefix(entity_type: &str) -> &str {
    match entity_type.to_lowercase().as_str() {
        "req" | "requirement" => "REQ",
        "risk" => "RISK",
        "test" => "TEST",
        "rslt" | "result" => "RSLT",
        "cmp" | "component" => "CMP",
        "asm" | "assembly" => "ASM",
        "sup" | "supplier" => "SUP",
        "quote" => "QUOTE",
        "proc" | "process" => "PROC",
        "ctrl" | "control" => "CTRL",
        "work" => "WORK",
        "ncr" => "NCR",
        "capa" => "CAPA",
        "feat" | "feature" => "FEAT",
        "mate" => "MATE",
        "tol" | "stackup" => "TOL",
        _ => "",
    }
}

fn extract_entity_id(project: &Project, file_path: &str) -> Option<String> {
    let full_path = project.root().join(file_path);
    if let Ok(content) = std::fs::read_to_string(&full_path) {
        // Look for id: field in YAML
        for line in content.lines() {
            if line.starts_with("id:") {
                let id = line.trim_start_matches("id:").trim();
                // Remove quotes if present
                let id = id.trim_matches('"').trim_matches('\'');
                return Some(id.to_string());
            }
        }
    }
    None
}
