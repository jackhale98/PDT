//! `tdt haz` command - Hazard management for safety analysis

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::hazard::{Hazard, HazardCategory, HazardSeverity};
use crate::schema::wizard::SchemaWizard;

/// Column definitions for hazard list output
const HAZ_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 12),
    ColumnDef::new("category", "CATEGORY", 12),
    ColumnDef::new("severity", "SEVERITY", 12),
    ColumnDef::new("title", "TITLE", 35),
    ColumnDef::new("risks", "RISKS", 6),
    ColumnDef::new("ctrls", "CTRLS", 6),
    ColumnDef::new("status", "STATUS", 10),
];

/// CLI-friendly hazard category enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliHazardCategory {
    Electrical,
    Mechanical,
    Thermal,
    Chemical,
    Biological,
    Radiation,
    Ergonomic,
    Software,
    Environmental,
}

impl std::fmt::Display for CliHazardCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliHazardCategory::Electrical => write!(f, "electrical"),
            CliHazardCategory::Mechanical => write!(f, "mechanical"),
            CliHazardCategory::Thermal => write!(f, "thermal"),
            CliHazardCategory::Chemical => write!(f, "chemical"),
            CliHazardCategory::Biological => write!(f, "biological"),
            CliHazardCategory::Radiation => write!(f, "radiation"),
            CliHazardCategory::Ergonomic => write!(f, "ergonomic"),
            CliHazardCategory::Software => write!(f, "software"),
            CliHazardCategory::Environmental => write!(f, "environmental"),
        }
    }
}

impl From<CliHazardCategory> for HazardCategory {
    fn from(cli: CliHazardCategory) -> Self {
        match cli {
            CliHazardCategory::Electrical => HazardCategory::Electrical,
            CliHazardCategory::Mechanical => HazardCategory::Mechanical,
            CliHazardCategory::Thermal => HazardCategory::Thermal,
            CliHazardCategory::Chemical => HazardCategory::Chemical,
            CliHazardCategory::Biological => HazardCategory::Biological,
            CliHazardCategory::Radiation => HazardCategory::Radiation,
            CliHazardCategory::Ergonomic => HazardCategory::Ergonomic,
            CliHazardCategory::Software => HazardCategory::Software,
            CliHazardCategory::Environmental => HazardCategory::Environmental,
        }
    }
}

/// CLI-friendly hazard severity enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliHazardSeverity {
    Negligible,
    Minor,
    Serious,
    Severe,
    Catastrophic,
}

impl std::fmt::Display for CliHazardSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliHazardSeverity::Negligible => write!(f, "negligible"),
            CliHazardSeverity::Minor => write!(f, "minor"),
            CliHazardSeverity::Serious => write!(f, "serious"),
            CliHazardSeverity::Severe => write!(f, "severe"),
            CliHazardSeverity::Catastrophic => write!(f, "catastrophic"),
        }
    }
}

impl From<CliHazardSeverity> for HazardSeverity {
    fn from(cli: CliHazardSeverity) -> Self {
        match cli {
            CliHazardSeverity::Negligible => HazardSeverity::Negligible,
            CliHazardSeverity::Minor => HazardSeverity::Minor,
            CliHazardSeverity::Serious => HazardSeverity::Serious,
            CliHazardSeverity::Severe => HazardSeverity::Severe,
            CliHazardSeverity::Catastrophic => HazardSeverity::Catastrophic,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum HazCommands {
    /// List hazards with filtering
    List(ListArgs),

    /// Create a new hazard
    New(NewArgs),

    /// Show a hazard's details
    Show(ShowArgs),

    /// Edit a hazard in your editor
    Edit(EditArgs),

    /// Delete a hazard
    Delete(DeleteArgs),

    /// Archive a hazard (soft delete)
    Archive(ArchiveArgs),
}

/// Category filter for list
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CategoryFilter {
    Electrical,
    Mechanical,
    Thermal,
    Chemical,
    Biological,
    Radiation,
    Ergonomic,
    Software,
    Environmental,
    All,
}

/// Severity filter for list
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SeverityFilter {
    Negligible,
    Minor,
    Serious,
    Severe,
    Catastrophic,
    All,
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by category
    #[arg(long, short = 'c', value_enum, default_value = "all")]
    pub category: CategoryFilter,

    /// Filter by severity
    #[arg(long, value_enum, default_value = "all")]
    pub severity: SeverityFilter,

    /// Filter by status (draft, review, approved, released)
    #[arg(long, short = 's')]
    pub status: Option<String>,

    /// Filter by tag
    #[arg(long, short = 't')]
    pub tag: Option<String>,

    /// Show only uncontrolled hazards (no controls linked)
    #[arg(long)]
    pub uncontrolled: bool,

    /// Show only hazards without any linked risks
    #[arg(long)]
    pub no_risks: bool,

    /// Maximum number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Hazard title
    #[arg(long)]
    pub title: Option<String>,

    /// Hazard category
    #[arg(long, short = 'c', value_enum)]
    pub category: Option<CliHazardCategory>,

    /// Hazard description
    #[arg(long, short = 'd')]
    pub description: Option<String>,

    /// Potential harms (comma-separated)
    #[arg(long)]
    pub harms: Option<String>,

    /// Energy level or magnitude
    #[arg(long)]
    pub energy: Option<String>,

    /// Maximum severity
    #[arg(long, value_enum)]
    pub severity: Option<CliHazardSeverity>,

    /// Tags (comma-separated)
    #[arg(long, short = 't')]
    pub tags: Option<String>,

    /// Component/assembly this hazard originates from
    #[arg(long)]
    pub source: Option<String>,

    /// Skip opening editor
    #[arg(long)]
    pub no_edit: bool,

    /// Use interactive wizard
    #[arg(long, short = 'i')]
    pub interactive: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Hazard ID or short ID (e.g., HAZ@1)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Hazard ID or short ID
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Hazard ID or short ID
    pub id: String,

    /// Skip confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(clap::Args, Debug)]
pub struct ArchiveArgs {
    /// Hazard ID or short ID
    pub id: String,

    /// Skip confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

pub fn run(cmd: HazCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        HazCommands::List(args) => run_list(args, global),
        HazCommands::New(args) => run_new(args, global),
        HazCommands::Show(args) => run_show(args, global),
        HazCommands::Edit(args) => run_edit(args, global),
        HazCommands::Delete(args) => run_delete(args, global),
        HazCommands::Archive(args) => run_archive(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let short_ids = ShortIdIndex::load(&project);

    // Load hazards from filesystem
    let hazards_dir = project.root().join("risks/hazards");
    let mut hazards: Vec<Hazard> = Vec::new();

    if hazards_dir.exists() {
        for entry in walkdir::WalkDir::new(&hazards_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(hazard) = serde_yml::from_str::<Hazard>(&content) {
                    hazards.push(hazard);
                }
            }
        }
    }

    // Apply filters
    let filtered: Vec<&Hazard> = hazards
        .iter()
        .filter(|h| match args.category {
            CategoryFilter::Electrical => h.category == HazardCategory::Electrical,
            CategoryFilter::Mechanical => h.category == HazardCategory::Mechanical,
            CategoryFilter::Thermal => h.category == HazardCategory::Thermal,
            CategoryFilter::Chemical => h.category == HazardCategory::Chemical,
            CategoryFilter::Biological => h.category == HazardCategory::Biological,
            CategoryFilter::Radiation => h.category == HazardCategory::Radiation,
            CategoryFilter::Ergonomic => h.category == HazardCategory::Ergonomic,
            CategoryFilter::Software => h.category == HazardCategory::Software,
            CategoryFilter::Environmental => h.category == HazardCategory::Environmental,
            CategoryFilter::All => true,
        })
        .filter(|h| match args.severity {
            SeverityFilter::Negligible => h.severity == HazardSeverity::Negligible,
            SeverityFilter::Minor => h.severity == HazardSeverity::Minor,
            SeverityFilter::Serious => h.severity == HazardSeverity::Serious,
            SeverityFilter::Severe => h.severity == HazardSeverity::Severe,
            SeverityFilter::Catastrophic => h.severity == HazardSeverity::Catastrophic,
            SeverityFilter::All => true,
        })
        .filter(|h| {
            if let Some(ref status_filter) = args.status {
                h.status.to_string().eq_ignore_ascii_case(status_filter)
            } else {
                true
            }
        })
        .filter(|h| {
            if let Some(ref tag_filter) = args.tag {
                h.tags.iter().any(|t| t.eq_ignore_ascii_case(tag_filter))
            } else {
                true
            }
        })
        .filter(|h| {
            if args.uncontrolled {
                h.links.controlled_by.is_empty()
            } else {
                true
            }
        })
        .filter(|h| {
            if args.no_risks {
                h.links.causes.is_empty()
            } else {
                true
            }
        })
        .take(args.limit.unwrap_or(usize::MAX))
        .collect();

    // Output format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Table,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&filtered).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::ShortId => {
            for h in &filtered {
                let short = short_ids
                    .get_short_id(&h.id.to_string())
                    .unwrap_or_else(|| h.id.to_string());
                println!("{}", short);
            }
        }
        OutputFormat::Id => {
            for h in &filtered {
                println!("{}", h.id);
            }
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&filtered).into_diagnostic()?;
            println!("{}", yaml);
        }
        _ => {
            if filtered.is_empty() {
                println!("No hazards found.");
                return Ok(());
            }

            // Default visible columns
            let visible: Vec<&str> = vec!["id", "category", "severity", "title", "risks", "ctrls", "status"];

            // Convert to TableRows
            let rows: Vec<TableRow> = filtered
                .iter()
                .map(|h| hazard_to_row(h, &short_ids))
                .collect();

            // Configure table
            let config = TableConfig::default();
            let formatter = TableFormatter::new(HAZ_COLUMNS, "hazard", "HAZ").with_config(config);
            formatter.output(rows, format, &visible);

            let uncontrolled = filtered.iter().filter(|h| h.links.controlled_by.is_empty()).count();
            if uncontrolled > 0 {
                println!(
                    "\n{} hazards, {} uncontrolled",
                    filtered.len(),
                    style(uncontrolled).yellow()
                );
            } else {
                println!("\n{} hazards", filtered.len());
            }
        }
    }

    Ok(())
}

/// Convert a Hazard to a TableRow
fn hazard_to_row(hazard: &Hazard, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(hazard.id.to_string(), short_ids)
        .cell("id", CellValue::Id(hazard.id.to_string()))
        .cell("category", CellValue::Type(hazard.category.to_string()))
        .cell("severity", CellValue::Text(hazard.severity.to_string()))
        .cell("title", CellValue::Text(hazard.title.clone()))
        .cell("risks", CellValue::Number(hazard.links.causes.len() as i64))
        .cell("ctrls", CellValue::Number(hazard.links.controlled_by.len() as i64))
        .cell("status", CellValue::Status(hazard.status))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let config = Config::load();

    // Get author
    let author = config.author();

    // Interactive wizard mode
    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Haz)?;

        let title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Hazard".to_string());

        let category_str = result
            .get_string("category")
            .unwrap_or("electrical");
        let category = category_str.parse::<HazardCategory>().unwrap_or(HazardCategory::Electrical);

        let description = result
            .get_string("description")
            .map(String::from)
            .unwrap_or_default();

        let id = EntityId::new(EntityPrefix::Haz);
        let hazard = Hazard::new(id.clone(), title, category, description, author.clone());

        // Write file
        let hazards_dir = project.root().join("risks/hazards");
        fs::create_dir_all(&hazards_dir).into_diagnostic()?;

        let yaml = serde_yml::to_string(&hazard).into_diagnostic()?;
        let filename = format!("{}.tdt.yaml", id);
        let file_path = hazards_dir.join(&filename);
        fs::write(&file_path, &yaml).into_diagnostic()?;

        match global.output {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "id": id.to_string(),
                    "title": hazard.title,
                    "path": file_path.display().to_string()
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            OutputFormat::Id => {
                println!("{}", id);
            }
            _ => {
                println!("Created hazard {} at {}", style(&id.to_string()).cyan(), file_path.display());
            }
        }
        return Ok(());
    }

    // Check required fields
    let title = args.title.ok_or_else(|| {
        miette::miette!("--title is required (or use --interactive for wizard mode)")
    })?;

    let category: HazardCategory = args
        .category
        .map(|c| c.into())
        .unwrap_or(HazardCategory::Electrical);

    let description = args.description.unwrap_or_default();

    // Generate ID
    let id = EntityId::new(EntityPrefix::Haz);

    // Build hazard
    let mut hazard = Hazard::new(id.clone(), title, category, description, author);

    // Apply optional fields
    if let Some(harms) = args.harms {
        hazard.potential_harms = harms.split(',').map(|s| s.trim().to_string()).collect();
    }

    if let Some(energy) = args.energy {
        hazard.energy_level = Some(energy);
    }

    if let Some(severity) = args.severity {
        hazard.severity = severity.into();
    }

    if let Some(tags) = args.tags {
        hazard.tags = tags.split(',').map(|s| s.trim().to_string()).collect();
    }

    if let Some(source) = args.source {
        let resolved = crate::core::shortid::parse_entity_reference(&source, &project);
        let source_id = EntityId::parse(&resolved)
            .map_err(|e| miette::miette!("Invalid source ID '{}': {}", source, e))?;
        hazard.links.originates_from.push(source_id);
    }

    // Serialize
    let yaml = serde_yml::to_string(&hazard).into_diagnostic()?;

    // Write file
    let hazards_dir = project.root().join("risks/hazards");
    fs::create_dir_all(&hazards_dir).into_diagnostic()?;

    let filename = format!("{}.tdt.yaml", id);
    let file_path = hazards_dir.join(&filename);
    fs::write(&file_path, &yaml).into_diagnostic()?;

    // Open editor if not --no-edit
    if !args.no_edit {
        config.run_editor(&file_path).into_diagnostic()?;
    }

    // Output result
    match global.output {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "id": id.to_string(),
                "title": hazard.title,
                "path": file_path.display().to_string()
            });
            println!("{}", serde_json::to_string_pretty(&output).into_diagnostic()?);
        }
        OutputFormat::Id => {
            println!("{}", id);
        }
        _ => {
            println!("Created hazard {} at {}", style(&id.to_string()).cyan(), file_path.display());
        }
    }
    Ok(())
}

fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve ID
    let full_id = short_ids
        .resolve(&args.id)
        .ok_or_else(|| miette::miette!("Cannot resolve ID: {}", args.id))?;

    // Find file
    let file_path = find_hazard_file(&project, &full_id)?;
    let content = fs::read_to_string(&file_path).into_diagnostic()?;
    let hazard: Hazard = serde_yml::from_str(&content).into_diagnostic()?;

    let format = global.output;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&hazard).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            println!("{}", content);
        }
        OutputFormat::Auto | _ => {
            let short_id = short_ids
                .get_short_id(&hazard.id.to_string())
                .unwrap_or_else(|| hazard.id.to_string());

            println!("{} {}", style(&short_id).cyan(), style(&hazard.title).bold());
            println!();
            println!("  Category:    {}", hazard.category);
            println!("  Severity:    {}", hazard.severity);
            println!("  Status:      {}", hazard.status);

            if !hazard.potential_harms.is_empty() {
                println!("  Harms:       {}", hazard.potential_harms.join(", "));
            }

            if let Some(ref energy) = hazard.energy_level {
                println!("  Energy:      {}", energy);
            }

            if let Some(ref scenario) = hazard.exposure_scenario {
                println!("  Exposure:    {}", scenario);
            }

            if !hazard.affected_populations.is_empty() {
                println!("  Affected:    {}", hazard.affected_populations.join(", "));
            }

            if !hazard.tags.is_empty() {
                println!("  Tags:        {}", hazard.tags.join(", "));
            }

            println!();
            println!("  {}", style("Description:").dim());
            for line in hazard.description.lines() {
                println!("    {}", line);
            }

            // Links
            if !hazard.links.originates_from.is_empty() {
                println!();
                println!("  {}", style("Originates from:").dim());
                for link in &hazard.links.originates_from {
                    let display = short_ids
                        .get_short_id(&link.to_string())
                        .unwrap_or_else(|| link.to_string());
                    println!("    {}", display);
                }
            }

            if !hazard.links.causes.is_empty() {
                println!();
                println!("  {}", style("Causes risks:").dim());
                for link in &hazard.links.causes {
                    let display = short_ids
                        .get_short_id(&link.to_string())
                        .unwrap_or_else(|| link.to_string());
                    println!("    {}", display);
                }
            }

            if !hazard.links.controlled_by.is_empty() {
                println!();
                println!("  {}", style("Controlled by:").dim());
                for link in &hazard.links.controlled_by {
                    let display = short_ids
                        .get_short_id(&link.to_string())
                        .unwrap_or_else(|| link.to_string());
                    println!("    {}", display);
                }
            }

            if !hazard.links.verified_by.is_empty() {
                println!();
                println!("  {}", style("Verified by:").dim());
                for link in &hazard.links.verified_by {
                    let display = short_ids
                        .get_short_id(&link.to_string())
                        .unwrap_or_else(|| link.to_string());
                    println!("    {}", display);
                }
            }

            println!();
            println!(
                "  Created: {} by {}",
                hazard.created.format("%Y-%m-%d"),
                hazard.author
            );
            println!("  Revision: {}", hazard.revision);
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let config = Config::load();
    let short_ids = ShortIdIndex::load(&project);

    let full_id = short_ids
        .resolve(&args.id)
        .ok_or_else(|| miette::miette!("Cannot resolve ID: {}", args.id))?;

    let file_path = find_hazard_file(&project, &full_id)?;
    config.run_editor(&file_path).into_diagnostic()?;

    println!("Edited: {}", file_path.display());
    Ok(())
}

fn run_delete(args: DeleteArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let short_ids = ShortIdIndex::load(&project);

    let full_id = short_ids
        .resolve(&args.id)
        .ok_or_else(|| miette::miette!("Cannot resolve ID: {}", args.id))?;

    let file_path = find_hazard_file(&project, &full_id)?;

    if !args.yes {
        print!("Delete {}? [y/N] ", file_path.display());
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    fs::remove_file(&file_path).into_diagnostic()?;
    println!("Deleted: {}", file_path.display());
    Ok(())
}

fn run_archive(args: ArchiveArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let short_ids = ShortIdIndex::load(&project);

    let full_id = short_ids
        .resolve(&args.id)
        .ok_or_else(|| miette::miette!("Cannot resolve ID: {}", args.id))?;

    let file_path = find_hazard_file(&project, &full_id)?;

    if !args.yes {
        print!("Archive {}? [y/N] ", file_path.display());
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Create archive directory
    let archive_dir = project.root().join(".archive").join("hazards");
    fs::create_dir_all(&archive_dir).into_diagnostic()?;

    let filename = file_path.file_name().unwrap();
    let archive_path = archive_dir.join(filename);

    fs::rename(&file_path, &archive_path).into_diagnostic()?;
    println!("Archived to: {}", archive_path.display());
    Ok(())
}

fn find_hazard_file(project: &Project, id: &str) -> Result<std::path::PathBuf> {
    let hazards_dir = project.root().join("risks/hazards");

    if hazards_dir.exists() {
        for entry in walkdir::WalkDir::new(&hazards_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if content.contains(&format!("id: {}", id))
                    || content.contains(&format!("id: \"{}\"", id))
                {
                    return Ok(entry.path().to_path_buf());
                }
            }
        }
    }

    Err(miette::miette!("Hazard not found: {}", id))
}
