//! `pdt req` command - Requirement management

use clap::{Subcommand, ValueEnum};
use console::style;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::core::entity::Priority;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::Config;
use crate::entities::requirement::{Requirement, RequirementType};
use crate::schema::template::{TemplateContext, TemplateGenerator};

#[derive(Subcommand, Debug)]
pub enum ReqCommands {
    /// List requirements with filtering
    List(ListArgs),

    /// Create a new requirement
    New(NewArgs),

    /// Show a requirement's details
    Show(ShowArgs),

    /// Edit a requirement in your editor
    Edit(EditArgs),
}

/// Requirement type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReqTypeFilter {
    Input,
    Output,
    All,
}

/// Status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Draft,
    Review,
    Approved,
    Obsolete,
    /// All active (not obsolete)
    Active,
    /// All statuses
    All,
}

/// Priority filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PriorityFilter {
    Low,
    Medium,
    High,
    Critical,
    /// High and critical only
    Urgent,
    /// All priorities
    All,
}

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Type,
    Title,
    Status,
    Priority,
    Category,
    Author,
    Created,
    Tags,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Type => write!(f, "type"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Priority => write!(f, "priority"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
            ListColumn::Tags => write!(f, "tags"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    // ========== FILTERING OPTIONS ==========
    // These let users filter without needing awk/grep

    /// Filter by type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: ReqTypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by priority
    #[arg(long, short = 'p', default_value = "all")]
    pub priority: PriorityFilter,

    /// Filter by category (exact match)
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tag (requirements with this tag)
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Search in title and text (case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,

    // ========== COMMON FILTER SHORTCUTS ==========
    // Pre-built filters for common queries

    /// Show only unlinked requirements (no satisfied_by or verified_by)
    #[arg(long)]
    pub orphans: bool,

    /// Show requirements needing review (status=draft or review)
    #[arg(long)]
    pub needs_review: bool,

    /// Show requirements created in the last N days
    #[arg(long, value_name = "DAYS")]
    pub recent: Option<u32>,

    // ========== OUTPUT CONTROL ==========

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
        ListColumn::Type,
        ListColumn::Title,
        ListColumn::Status,
        ListColumn::Priority
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
    #[arg(long, default_value = "created")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Limit output to N items
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show count only, not the items
    #[arg(long)]
    pub count: bool,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Requirement type (input/output)
    #[arg(long, short = 't', default_value = "input")]
    pub r#type: String,

    /// Title (if not provided, uses placeholder)
    #[arg(long)]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Priority (low/medium/high/critical)
    #[arg(long, short = 'p', default_value = "medium")]
    pub priority: String,

    /// Tags (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Use interactive wizard to fill in fields
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Don't open in editor after creation
    #[arg(long)]
    pub no_edit: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Requirement ID or fuzzy search term
    pub id: String,

    /// Show linked entities too
    #[arg(long)]
    pub with_links: bool,

    /// Show revision history (from git)
    #[arg(long)]
    pub history: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Requirement ID or fuzzy search term
    pub id: String,
}

pub fn run(cmd: ReqCommands) -> Result<()> {
    match cmd {
        ReqCommands::List(args) => run_list(args),
        ReqCommands::New(args) => run_new(args),
        ReqCommands::Show(args) => run_show(args),
        ReqCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(_args: ListArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Collect all requirement files
    let mut reqs: Vec<Requirement> = Vec::new();

    for path in project.iter_entity_files(EntityPrefix::Req) {
        match crate::yaml::parse_yaml_file::<Requirement>(&path) {
            Ok(req) => reqs.push(req),
            Err(e) => {
                eprintln!(
                    "{} Failed to parse {}: {}",
                    style("!").yellow(),
                    path.display(),
                    e
                );
            }
        }
    }

    // Also check outputs directory
    let outputs_dir = project.root().join("requirements/outputs");
    if outputs_dir.exists() {
        for entry in walkdir::WalkDir::new(&outputs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                Ok(req) => reqs.push(req),
                Err(e) => {
                    eprintln!(
                        "{} Failed to parse {}: {}",
                        style("!").yellow(),
                        entry.path().display(),
                        e
                    );
                }
            }
        }
    }

    if reqs.is_empty() {
        println!("No requirements found.");
        println!();
        println!(
            "Create one with: {}",
            style("pdt req new").yellow()
        );
        return Ok(());
    }

    // Sort by created date (default)
    reqs.sort_by(|a, b| a.created.cmp(&b.created));

    // Print header - column widths: ID(16), TYPE(8), TITLE(40), STATUS(10), PRIORITY(10)
    println!(
        "{:<16} {:<8} {:<40} {:<10} {:<10}",
        style("ID").bold(),
        style("TYPE").bold(),
        style("TITLE").bold(),
        style("STATUS").bold(),
        style("PRIORITY").bold()
    );
    println!("{}", "-".repeat(86));

    // Print requirements
    for req in &reqs {
        // Truncate ID for display (REQ-01KC32WB... = 15 chars)
        let id_display = format_short_id(&req.id);
        let title_truncated = truncate_str(&req.title, 38);

        println!(
            "{:<16} {:<8} {:<40} {:<10} {:<10}",
            id_display,
            req.req_type,
            title_truncated,
            req.status,
            req.priority
        );
    }

    println!();
    println!(
        "{} requirement(s) found",
        style(reqs.len()).cyan()
    );

    Ok(())
}

fn run_new(args: NewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let theme = ColorfulTheme::default();

    // Determine values - either from args or interactive wizard
    let (req_type, title, priority, category) = if args.interactive {
        // Interactive wizard mode
        let types = &["input  - Design input (customer need, regulation)",
                      "output - Design output (specification)"];
        let type_selection = Select::with_theme(&theme)
            .with_prompt("Requirement type")
            .items(types)
            .default(0)
            .interact()
            .into_diagnostic()?;
        let req_type = if type_selection == 0 {
            RequirementType::Input
        } else {
            RequirementType::Output
        };

        let title: String = Input::with_theme(&theme)
            .with_prompt("Title")
            .interact_text()
            .into_diagnostic()?;

        let priorities = &["low", "medium", "high", "critical"];
        let priority_selection = Select::with_theme(&theme)
            .with_prompt("Priority")
            .items(priorities)
            .default(1)
            .interact()
            .into_diagnostic()?;
        let priority = match priority_selection {
            0 => Priority::Low,
            1 => Priority::Medium,
            2 => Priority::High,
            3 => Priority::Critical,
            _ => Priority::Medium,
        };

        let category: String = Input::with_theme(&theme)
            .with_prompt("Category (optional)")
            .allow_empty(true)
            .interact_text()
            .into_diagnostic()?;

        (req_type, title, priority, category)
    } else {
        // Default mode - use args with defaults
        let req_type = match args.r#type.to_lowercase().as_str() {
            "input" => RequirementType::Input,
            "output" => RequirementType::Output,
            t => {
                return Err(miette::miette!(
                    "Invalid requirement type: '{}'. Use 'input' or 'output'",
                    t
                ))
            }
        };

        let title = args.title.unwrap_or_else(|| "New Requirement".to_string());

        let priority = match args.priority.to_lowercase().as_str() {
            "low" => Priority::Low,
            "medium" => Priority::Medium,
            "high" => Priority::High,
            "critical" => Priority::Critical,
            p => {
                return Err(miette::miette!(
                    "Invalid priority: '{}'. Use low/medium/high/critical",
                    p
                ))
            }
        };

        let category = args.category.unwrap_or_default();

        (req_type, title, priority, category)
    };

    // Generate entity ID and create from template
    let id = EntityId::new(EntityPrefix::Req);
    let author = config.author();

    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), author)
        .with_title(&title)
        .with_req_type(req_type.to_string())
        .with_priority(priority.to_string())
        .with_category(&category);

    let yaml_content = generator
        .generate_requirement(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Determine output directory based on type
    let output_dir = project.requirement_directory(&req_type.to_string());
    let file_path = output_dir.join(format!("{}.pdt.yaml", id));

    // Write file
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    println!(
        "{} Created requirement {}",
        style("✓").green(),
        style(format_short_id(&id)).cyan()
    );
    println!("   {}", style(file_path.display()).dim());

    // Open in editor if requested (or by default unless --no-edit)
    if args.edit || (!args.no_edit && !args.interactive) {
        let editor = config.editor();
        println!();
        println!("Opening in {}...", style(&editor).yellow());

        std::process::Command::new(&editor)
            .arg(&file_path)
            .status()
            .into_diagnostic()?;
    }

    Ok(())
}

fn run_show(args: ShowArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find the requirement by ID prefix match
    let req = find_requirement(&project, &args.id)?;

    // Display the requirement
    println!("{}", style("─".repeat(60)).dim());
    println!(
        "{}: {}",
        style("ID").bold(),
        style(&req.id.to_string()).cyan()
    );
    println!(
        "{}: {}",
        style("Type").bold(),
        req.req_type
    );
    println!(
        "{}: {}",
        style("Title").bold(),
        style(&req.title).yellow()
    );
    println!(
        "{}: {}",
        style("Status").bold(),
        req.status
    );
    println!(
        "{}: {}",
        style("Priority").bold(),
        req.priority
    );
    if let Some(ref cat) = req.category {
        if !cat.is_empty() {
            println!("{}: {}", style("Category").bold(), cat);
        }
    }
    if !req.tags.is_empty() {
        println!(
            "{}: {}",
            style("Tags").bold(),
            req.tags.join(", ")
        );
    }
    println!("{}", style("─".repeat(60)).dim());
    println!();
    println!("{}", &req.text);
    println!();

    if let Some(ref rationale) = req.rationale {
        if !rationale.is_empty() {
            println!("{}", style("Rationale:").bold());
            println!("{}", rationale);
            println!();
        }
    }

    if !req.acceptance_criteria.is_empty()
        && !req.acceptance_criteria.iter().all(|c| c.is_empty())
    {
        println!("{}", style("Acceptance Criteria:").bold());
        for criterion in &req.acceptance_criteria {
            if !criterion.is_empty() {
                println!("  • {}", criterion);
            }
        }
        println!();
    }

    println!("{}", style("─".repeat(60)).dim());
    println!(
        "{}: {} | {}: {} | {}: {}",
        style("Author").dim(),
        req.author,
        style("Created").dim(),
        req.created.format("%Y-%m-%d %H:%M"),
        style("Revision").dim(),
        req.revision
    );

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Find the requirement by ID prefix match
    let req = find_requirement(&project, &args.id)?;

    // Get the file path
    let req_type = match req.req_type {
        RequirementType::Input => "inputs",
        RequirementType::Output => "outputs",
    };
    let file_path = project
        .root()
        .join(format!("requirements/{}/{}.pdt.yaml", req_type, req.id));

    if !file_path.exists() {
        return Err(miette::miette!(
            "File not found: {}",
            file_path.display()
        ));
    }

    let editor = config.editor();
    println!(
        "Opening {} in {}...",
        style(format_short_id(&req.id)).cyan(),
        style(&editor).yellow()
    );

    std::process::Command::new(&editor)
        .arg(&file_path)
        .status()
        .into_diagnostic()?;

    Ok(())
}

/// Find a requirement by ID prefix match
fn find_requirement(project: &Project, id_query: &str) -> Result<Requirement> {
    let mut matches: Vec<(Requirement, std::path::PathBuf)> = Vec::new();

    // Search both inputs and outputs
    for subdir in &["inputs", "outputs"] {
        let dir = project.root().join(format!("requirements/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                // Check if ID matches (prefix or full)
                let id_str = req.id.to_string();
                if id_str.starts_with(id_query) || id_str == id_query {
                    matches.push((req, entry.path().to_path_buf()));
                }
                // Also check title for fuzzy match
                else if req.title.to_lowercase().contains(&id_query.to_lowercase()) {
                    matches.push((req, entry.path().to_path_buf()));
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!(
            "No requirement found matching '{}'",
            id_query
        )),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!(
                "{} Multiple matches found:",
                style("!").yellow()
            );
            for (req, _path) in &matches {
                println!(
                    "  {} - {}",
                    format_short_id(&req.id),
                    req.title
                );
            }
            Err(miette::miette!(
                "Ambiguous query '{}'. Please be more specific.",
                id_query
            ))
        }
    }
}

/// Format an entity ID for short display (prefix + first 8 chars of ULID)
fn format_short_id(id: &EntityId) -> String {
    let full = id.to_string();
    if full.len() > 12 {
        format!("{}...", &full[..12])
    } else {
        full
    }
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
