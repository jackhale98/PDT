//! `tdt risk` command - Risk/FMEA management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::risk::{Risk, RiskLevel, RiskType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum RiskCommands {
    /// List risks with filtering
    List(ListArgs),

    /// Create a new risk
    New(NewArgs),

    /// Show a risk's details
    Show(ShowArgs),

    /// Edit a risk in your editor
    Edit(EditArgs),

    /// Show risk statistics summary
    Summary(SummaryArgs),
}

/// Risk type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RiskTypeFilter {
    Design,
    Process,
    All,
}

/// Risk level filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RiskLevelFilter {
    Low,
    Medium,
    High,
    Critical,
    /// High and critical only
    Urgent,
    /// All levels
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

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Type,
    Title,
    Status,
    RiskLevel,
    Severity,
    Occurrence,
    Detection,
    Rpn,
    Category,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Type => write!(f, "type"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::RiskLevel => write!(f, "risk-level"),
            ListColumn::Severity => write!(f, "severity"),
            ListColumn::Occurrence => write!(f, "occurrence"),
            ListColumn::Detection => write!(f, "detection"),
            ListColumn::Rpn => write!(f, "rpn"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: RiskTypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by risk level
    #[arg(long, short = 'l', default_value = "all")]
    pub level: RiskLevelFilter,

    /// Filter by category (case-insensitive)
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tag (case-insensitive)
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by minimum RPN
    #[arg(long)]
    pub min_rpn: Option<u16>,

    /// Filter by maximum RPN
    #[arg(long)]
    pub max_rpn: Option<u16>,

    /// Show risks above this RPN threshold (alias for --min-rpn)
    #[arg(long, value_name = "N")]
    pub above_rpn: Option<u16>,

    /// Filter by author (substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Search in title and description (case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,

    /// Show only risks without mitigations
    #[arg(long)]
    pub unmitigated: bool,

    /// Show risks with incomplete mitigations (not all verified/completed)
    #[arg(long)]
    pub open_mitigations: bool,

    /// Show only critical risks (shortcut for --level critical)
    #[arg(long)]
    pub critical: bool,

    /// Show risks created in last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
        ListColumn::Type,
        ListColumn::Title,
        ListColumn::Status,
        ListColumn::RiskLevel,
        ListColumn::Rpn
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field (default: created)
    #[arg(long, default_value = "created")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Sort by RPN (highest first) - shorthand for --sort rpn --reverse
    #[arg(long)]
    pub by_rpn: bool,

    /// Limit output to N items
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show count only, not the items
    #[arg(long)]
    pub count: bool,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Risk type (design/process)
    #[arg(long, short = 't', default_value = "design")]
    pub r#type: String,

    /// Title (if not provided, uses placeholder)
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Initial severity rating (1-10)
    #[arg(long, short = 'S')]
    pub severity: Option<u8>,

    /// Initial occurrence rating (1-10)
    #[arg(long, short = 'O')]
    pub occurrence: Option<u8>,

    /// Initial detection rating (1-10)
    #[arg(long, short = 'D')]
    pub detection: Option<u8>,

    /// Use interactive wizard to fill in fields
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Don't open in editor after creation
    #[arg(long, short = 'n')]
    pub no_edit: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Risk ID or fuzzy search term
    pub id: String,

    /// Show linked entities too
    #[arg(long)]
    pub with_links: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Risk ID or fuzzy search term
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct SummaryArgs {
    /// Show top N risks by RPN (default: 5)
    #[arg(long, short = 'n', default_value = "5")]
    pub top: usize,

    /// Include detailed breakdown by category
    #[arg(long)]
    pub detailed: bool,
}

pub fn run(cmd: RiskCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        RiskCommands::List(args) => run_list(args, global),
        RiskCommands::New(args) => run_new(args),
        RiskCommands::Show(args) => run_show(args, global),
        RiskCommands::Edit(args) => run_edit(args),
        RiskCommands::Summary(args) => run_summary(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Collect all risk files
    let mut risks: Vec<Risk> = Vec::new();

    // Check design risks
    let design_dir = project.root().join("risks/design");
    if design_dir.exists() {
        for entry in walkdir::WalkDir::new(&design_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                Ok(risk) => risks.push(risk),
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

    // Check process risks
    let process_dir = project.root().join("risks/process");
    if process_dir.exists() {
        for entry in walkdir::WalkDir::new(&process_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                Ok(risk) => risks.push(risk),
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

    // Apply filters
    risks.retain(|r| {
        // Type filter
        let type_match = match args.r#type {
            RiskTypeFilter::Design => r.risk_type == RiskType::Design,
            RiskTypeFilter::Process => r.risk_type == RiskType::Process,
            RiskTypeFilter::All => true,
        };

        // Status filter
        let status_match = match args.status {
            StatusFilter::Draft => r.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => r.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => r.status == crate::core::entity::Status::Approved,
            StatusFilter::Obsolete => r.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => r.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        };

        // Level filter
        let level_match = match args.level {
            RiskLevelFilter::All => true,
            RiskLevelFilter::Urgent => matches!(r.risk_level, Some(RiskLevel::High) | Some(RiskLevel::Critical)),
            RiskLevelFilter::Low => r.risk_level == Some(RiskLevel::Low),
            RiskLevelFilter::Medium => r.risk_level == Some(RiskLevel::Medium),
            RiskLevelFilter::High => r.risk_level == Some(RiskLevel::High),
            RiskLevelFilter::Critical => r.risk_level == Some(RiskLevel::Critical),
        };

        // RPN filters (above_rpn is an alias for min_rpn)
        let effective_min_rpn = args.above_rpn.or(args.min_rpn);
        let min_rpn_match = effective_min_rpn.map_or(true, |min| r.rpn.unwrap_or(0) >= min);
        let max_rpn_match = args.max_rpn.map_or(true, |max| r.rpn.unwrap_or(0) <= max);

        // Category filter (case-insensitive)
        let category_match = args.category.as_ref().map_or(true, |cat| {
            r.category.as_ref().map_or(false, |c| c.to_lowercase() == cat.to_lowercase())
        });

        // Tag filter (case-insensitive)
        let tag_match = args.tag.as_ref().map_or(true, |tag| {
            r.tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase())
        });

        // Author filter
        let author_match = args.author.as_ref().map_or(true, |author| {
            r.author.to_lowercase().contains(&author.to_lowercase())
        });

        // Search filter
        let search_match = args.search.as_ref().map_or(true, |search| {
            let search_lower = search.to_lowercase();
            r.title.to_lowercase().contains(&search_lower)
                || r.description.to_lowercase().contains(&search_lower)
        });

        // Unmitigated filter
        let unmitigated_match = !args.unmitigated || r.mitigations.is_empty();

        // Open mitigations filter (has mitigations but not all completed/verified)
        let open_mitigations_match = if args.open_mitigations {
            use crate::entities::risk::MitigationStatus;
            !r.mitigations.is_empty() && r.mitigations.iter().any(|m| {
                match m.status {
                    Some(MitigationStatus::Completed) | Some(MitigationStatus::Verified) => false,
                    _ => true, // Proposed, InProgress, or None
                }
            })
        } else {
            true
        };

        // Critical shortcut filter
        let critical_match = !args.critical || r.risk_level == Some(RiskLevel::Critical);

        // Recent filter (created in last N days)
        let recent_match = args.recent.map_or(true, |days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            r.created >= cutoff
        });

        type_match && status_match && level_match && min_rpn_match && max_rpn_match
            && category_match && tag_match && author_match && search_match
            && unmitigated_match && open_mitigations_match && critical_match && recent_match
    });

    if risks.is_empty() {
        match global.format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No risks found.");
                println!();
                println!("Create one with: {}", style("tdt risk new").yellow());
            }
        }
        return Ok(());
    }

    // Sort by specified column (or RPN if --by-rpn is used)
    if args.by_rpn {
        risks.sort_by(|a, b| b.rpn.unwrap_or(0).cmp(&a.rpn.unwrap_or(0)));
    } else {
        match args.sort {
            ListColumn::Id => risks.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
            ListColumn::Type => risks.sort_by(|a, b| a.risk_type.to_string().cmp(&b.risk_type.to_string())),
            ListColumn::Title => risks.sort_by(|a, b| a.title.cmp(&b.title)),
            ListColumn::Status => risks.sort_by(|a, b| a.status.to_string().cmp(&b.status.to_string())),
            ListColumn::RiskLevel => {
                let level_order = |l: &Option<RiskLevel>| match l {
                    Some(RiskLevel::Critical) => 0,
                    Some(RiskLevel::High) => 1,
                    Some(RiskLevel::Medium) => 2,
                    Some(RiskLevel::Low) => 3,
                    None => 4,
                };
                risks.sort_by(|a, b| level_order(&a.risk_level).cmp(&level_order(&b.risk_level)));
            }
            ListColumn::Severity => risks.sort_by(|a, b| b.severity.unwrap_or(0).cmp(&a.severity.unwrap_or(0))),
            ListColumn::Occurrence => risks.sort_by(|a, b| b.occurrence.unwrap_or(0).cmp(&a.occurrence.unwrap_or(0))),
            ListColumn::Detection => risks.sort_by(|a, b| b.detection.unwrap_or(0).cmp(&a.detection.unwrap_or(0))),
            ListColumn::Rpn => risks.sort_by(|a, b| b.rpn.unwrap_or(0).cmp(&a.rpn.unwrap_or(0))),
            ListColumn::Category => risks.sort_by(|a, b| {
                a.category.as_deref().unwrap_or("").cmp(b.category.as_deref().unwrap_or(""))
            }),
            ListColumn::Author => risks.sort_by(|a, b| a.author.cmp(&b.author)),
            ListColumn::Created => risks.sort_by(|a, b| a.created.cmp(&b.created)),
        }
    }

    // Reverse if requested (unless by_rpn which is already reversed)
    if args.reverse && !args.by_rpn {
        risks.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        risks.truncate(limit);
    }

    // Just count?
    if args.count {
        println!("{}", risks.len());
        return Ok(());
    }

    // Update short ID index with current risks (preserves other entity types)
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(risks.iter().map(|r| r.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&risks).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&risks).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,type,title,status,risk_level,severity,occurrence,detection,rpn");
            for risk in &risks {
                let short_id = short_ids.get_short_id(&risk.id.to_string()).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{},{},{}",
                    short_id,
                    risk.id,
                    risk.risk_type,
                    escape_csv(&risk.title),
                    risk.status,
                    risk.risk_level.map_or("".to_string(), |l| l.to_string()),
                    risk.severity.map_or("".to_string(), |s| s.to_string()),
                    risk.occurrence.map_or("".to_string(), |o| o.to_string()),
                    risk.detection.map_or("".to_string(), |d| d.to_string()),
                    risk.rpn.map_or("".to_string(), |r| r.to_string())
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => format!("{:<17}", style("ID").bold()),
                    ListColumn::Type => format!("{:<9}", style("TYPE").bold()),
                    ListColumn::Title => format!("{:<28}", style("TITLE").bold()),
                    ListColumn::Status => format!("{:<10}", style("STATUS").bold()),
                    ListColumn::RiskLevel => format!("{:<8}", style("LEVEL").bold()),
                    ListColumn::Severity => format!("{:<4}", style("SEV").bold()),
                    ListColumn::Occurrence => format!("{:<4}", style("OCC").bold()),
                    ListColumn::Detection => format!("{:<4}", style("DET").bold()),
                    ListColumn::Rpn => format!("{:<5}", style("RPN").bold()),
                    ListColumn::Category => format!("{:<14}", style("CATEGORY").bold()),
                    ListColumn::Author => format!("{:<14}", style("AUTHOR").bold()),
                    ListColumn::Created => format!("{:<12}", style("CREATED").bold()),
                };
                header_parts.push(header);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(90));

            for risk in &risks {
                let short_id = short_ids.get_short_id(&risk.id.to_string()).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<17}", format_short_id(&risk.id)),
                        ListColumn::Type => format!("{:<9}", risk.risk_type),
                        ListColumn::Title => format!("{:<28}", truncate_str(&risk.title, 26)),
                        ListColumn::Status => format!("{:<10}", risk.status),
                        ListColumn::RiskLevel => format!("{:<8}", risk.risk_level.map_or("-".to_string(), |l| l.to_string())),
                        ListColumn::Severity => format!("{:<4}", risk.severity.map_or("-".to_string(), |s| s.to_string())),
                        ListColumn::Occurrence => format!("{:<4}", risk.occurrence.map_or("-".to_string(), |o| o.to_string())),
                        ListColumn::Detection => format!("{:<4}", risk.detection.map_or("-".to_string(), |d| d.to_string())),
                        ListColumn::Rpn => {
                            let rpn_str = risk.rpn.map_or("-".to_string(), |r| r.to_string());
                            let colored = match risk.rpn {
                                Some(r) if r > 400 => style(&rpn_str).red().to_string(),
                                Some(r) if r > 150 => style(&rpn_str).yellow().to_string(),
                                _ => rpn_str.clone(),
                            };
                            format!("{:<5}", colored)
                        }
                        ListColumn::Category => format!("{:<14}", truncate_str(risk.category.as_deref().unwrap_or(""), 12)),
                        ListColumn::Author => format!("{:<14}", truncate_str(&risk.author, 12)),
                        ListColumn::Created => format!("{:<12}", risk.created.format("%Y-%m-%d")),
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} risk(s) found. Use {} to reference by short ID.",
                style(risks.len()).cyan(),
                style("RISK@N").cyan()
            );
        }
        OutputFormat::Id => {
            for risk in &risks {
                println!("{}", risk.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Type | Title | Status | Level | RPN |");
            println!("|---|---|---|---|---|---|---|");
            for risk in &risks {
                let short_id = short_ids.get_short_id(&risk.id.to_string()).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&risk.id),
                    risk.risk_type,
                    risk.title,
                    risk.status,
                    risk.risk_level.map_or("-".to_string(), |l| l.to_string()),
                    risk.rpn.map_or("-".to_string(), |r| r.to_string())
                );
            }
        }
        OutputFormat::Auto => unreachable!(),
    }

    Ok(())
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn run_new(args: NewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (risk_type, title, category, severity, occurrence, detection) = if args.interactive {
        // Use the schema-driven wizard
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Risk)?;

        let risk_type = result
            .get_string("type")
            .map(|s| match s {
                "process" => RiskType::Process,
                _ => RiskType::Design,
            })
            .unwrap_or(RiskType::Design);

        let title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Risk".to_string());

        let category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_default();

        let severity = result
            .get_string("severity")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let occurrence = result
            .get_string("occurrence")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let detection = result
            .get_string("detection")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        (risk_type, title, category, severity, occurrence, detection)
    } else {
        // Default mode - use args with defaults
        let risk_type = match args.r#type.to_lowercase().as_str() {
            "design" => RiskType::Design,
            "process" => RiskType::Process,
            t => {
                return Err(miette::miette!(
                    "Invalid risk type: '{}'. Use 'design' or 'process'",
                    t
                ))
            }
        };

        let title = args.title.unwrap_or_else(|| "New Risk".to_string());
        let category = args.category.unwrap_or_default();
        let severity = args.severity.unwrap_or(5);
        let occurrence = args.occurrence.unwrap_or(5);
        let detection = args.detection.unwrap_or(5);

        (risk_type, title, category, severity, occurrence, detection)
    };

    // Calculate RPN and determine risk level
    let rpn = severity as u16 * occurrence as u16 * detection as u16;
    let risk_level = match rpn {
        0..=50 => "low",
        51..=150 => "medium",
        151..=400 => "high",
        _ => "critical",
    };

    // Generate entity ID and create from template
    let id = EntityId::new(EntityPrefix::Risk);
    let author = config.author();

    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), author)
        .with_title(&title)
        .with_risk_type(risk_type.to_string())
        .with_category(&category)
        .with_severity(severity)
        .with_occurrence(occurrence)
        .with_detection(detection)
        .with_risk_level(risk_level);

    let yaml_content = generator
        .generate_risk(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Determine output directory based on type
    let output_dir = project.risk_directory(&risk_type.to_string());

    // Ensure directory exists
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.tdt.yaml", id));

    // Write file
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
    let _ = short_ids.save(&project);

    println!(
        "{} Created risk {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!("   RPN: {} ({})", style(rpn).yellow(), risk_level);

    // Open in editor if requested (or by default unless --no-edit)
    if args.edit || (!args.no_edit && !args.interactive) {
        println!();
        println!("Opening in {}...", style(config.editor()).yellow());

        config.run_editor(&file_path).into_diagnostic()?;
    }

    Ok(())
}

fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find the risk by ID prefix match
    let risk = find_risk(&project, &args.id)?;

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Yaml,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&risk).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&risk).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id => {
            println!("{}", risk.id);
        }
        _ => {
            // Human-readable format
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&risk.id.to_string()).cyan()
            );
            println!("{}: {}", style("Type").bold(), risk.risk_type);
            println!(
                "{}: {}",
                style("Title").bold(),
                style(&risk.title).yellow()
            );
            println!("{}: {}", style("Status").bold(), risk.status);
            if let Some(level) = &risk.risk_level {
                let level_styled = match level {
                    RiskLevel::Critical => style(level.to_string()).red().bold(),
                    RiskLevel::High => style(level.to_string()).red(),
                    RiskLevel::Medium => style(level.to_string()).yellow(),
                    RiskLevel::Low => style(level.to_string()).green(),
                };
                println!("{}: {}", style("Risk Level").bold(), level_styled);
            }
            if let Some(ref cat) = risk.category {
                if !cat.is_empty() {
                    println!("{}: {}", style("Category").bold(), cat);
                }
            }
            println!("{}", style("─".repeat(60)).dim());

            // Description
            println!();
            println!("{}", style("Description:").bold());
            println!("{}", &risk.description);

            // FMEA details
            if risk.failure_mode.is_some() || risk.cause.is_some() || risk.effect.is_some() {
                println!();
                println!("{}", style("FMEA Analysis:").bold());
                if let Some(ref fm) = risk.failure_mode {
                    if !fm.is_empty() {
                        println!("  {}: {}", style("Failure Mode").dim(), fm.trim());
                    }
                }
                if let Some(ref cause) = risk.cause {
                    if !cause.is_empty() {
                        println!("  {}: {}", style("Cause").dim(), cause.trim());
                    }
                }
                if let Some(ref effect) = risk.effect {
                    if !effect.is_empty() {
                        println!("  {}: {}", style("Effect").dim(), effect.trim());
                    }
                }
            }

            // Risk ratings
            if risk.severity.is_some() || risk.occurrence.is_some() || risk.detection.is_some() {
                println!();
                println!("{}", style("Risk Assessment:").bold());
                if let Some(s) = risk.severity {
                    println!("  {}: {}/10", style("Severity").dim(), s);
                }
                if let Some(o) = risk.occurrence {
                    println!("  {}: {}/10", style("Occurrence").dim(), o);
                }
                if let Some(d) = risk.detection {
                    println!("  {}: {}/10", style("Detection").dim(), d);
                }
                if let Some(rpn) = risk.rpn {
                    let rpn_styled = match rpn {
                        r if r > 400 => style(r.to_string()).red().bold(),
                        r if r > 150 => style(r.to_string()).yellow(),
                        r => style(r.to_string()).green(),
                    };
                    println!("  {}: {}", style("RPN").bold(), rpn_styled);
                }
            }

            // Mitigations
            if !risk.mitigations.is_empty() {
                println!();
                println!("{}", style("Mitigations:").bold());
                for (i, m) in risk.mitigations.iter().enumerate() {
                    if !m.action.is_empty() {
                        let status_str = m.status
                            .map(|s| format!(" [{}]", s))
                            .unwrap_or_default();
                        println!("  {}. {}{}", i + 1, m.action, style(status_str).dim());
                    }
                }
            }

            println!();
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                risk.author,
                style("Created").dim(),
                risk.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                risk.revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Find the risk by ID prefix match
    let risk = find_risk(&project, &args.id)?;

    // Get the file path
    let risk_type = match risk.risk_type {
        RiskType::Design => "design",
        RiskType::Process => "process",
    };
    let file_path = project
        .root()
        .join(format!("risks/{}/{}.tdt.yaml", risk_type, risk.id));

    if !file_path.exists() {
        return Err(miette::miette!(
            "File not found: {}",
            file_path.display()
        ));
    }

    println!(
        "Opening {} in {}...",
        style(format_short_id(&risk.id)).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&file_path).into_diagnostic()?;

    Ok(())
}

/// Find a risk by ID prefix match or short ID (@N)
fn find_risk(project: &Project, id_query: &str) -> Result<Risk> {
    // First, try to resolve short ID (@N) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids.resolve(id_query).unwrap_or_else(|| id_query.to_string());

    let mut matches: Vec<(Risk, std::path::PathBuf)> = Vec::new();

    // Search both design and process directories
    for subdir in &["design", "process"] {
        let dir = project.root().join(format!("risks/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                // Check if ID matches (prefix or full)
                let id_str = risk.id.to_string();
                if id_str.starts_with(&resolved_query) || id_str == resolved_query {
                    matches.push((risk, entry.path().to_path_buf()));
                }
                // Also check title for fuzzy match (only if not a short ID lookup)
                else if !id_query.starts_with('@') && !id_query.chars().all(|c| c.is_ascii_digit()) {
                    if risk.title.to_lowercase().contains(&resolved_query.to_lowercase()) {
                        matches.push((risk, entry.path().to_path_buf()));
                    }
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!(
            "No risk found matching '{}'",
            id_query
        )),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!(
                "{} Multiple matches found:",
                style("!").yellow()
            );
            for (risk, _path) in &matches {
                println!(
                    "  {} - {}",
                    format_short_id(&risk.id),
                    risk.title
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
    if full.len() > 13 {
        format!("{}...", &full[..13])
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

fn run_summary(args: SummaryArgs, global: &GlobalOpts) -> Result<()> {
    use crate::entities::risk::MitigationStatus;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Collect all risks
    let mut risks: Vec<Risk> = Vec::new();

    for subdir in &["risks/design", "risks/process"] {
        let dir = project.root().join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                risks.push(risk);
            }
        }
    }

    if risks.is_empty() {
        println!("{}", style("No risks found in project.").yellow());
        return Ok(());
    }

    // Calculate metrics
    let total = risks.len();

    // Count by level (using effective level - either explicit or calculated from RPN)
    let mut by_level: std::collections::HashMap<RiskLevel, usize> = std::collections::HashMap::new();
    for risk in &risks {
        let level = risk.risk_level.or_else(|| risk.determine_risk_level()).unwrap_or(RiskLevel::Medium);
        *by_level.entry(level).or_insert(0) += 1;
    }

    // Count by type
    let mut by_type: std::collections::HashMap<RiskType, usize> = std::collections::HashMap::new();
    for risk in &risks {
        *by_type.entry(risk.risk_type).or_insert(0) += 1;
    }

    // Calculate RPN statistics (only for risks that have RPN values)
    let rpns: Vec<u16> = risks.iter()
        .filter_map(|r| r.calculate_rpn())
        .collect();

    let (avg_rpn, max_rpn, min_rpn) = if rpns.is_empty() {
        (0.0, 0u16, 0u16)
    } else {
        let avg = rpns.iter().map(|&r| r as f64).sum::<f64>() / rpns.len() as f64;
        let max = *rpns.iter().max().unwrap_or(&0);
        let min = *rpns.iter().min().unwrap_or(&0);
        (avg, max, min)
    };

    // Count unmitigated
    let unmitigated = risks.iter()
        .filter(|r| r.mitigations.is_empty())
        .count();

    // Count with open mitigations (not all verified)
    let open_mitigations = risks.iter()
        .filter(|r| !r.mitigations.is_empty() &&
            r.mitigations.iter().any(|m| m.status != Some(MitigationStatus::Verified)))
        .count();

    // Sort by RPN for top N (risks without RPN go last)
    let mut sorted_risks: Vec<&Risk> = risks.iter().collect();
    sorted_risks.sort_by(|a, b| {
        let rpn_a = a.calculate_rpn().unwrap_or(0);
        let rpn_b = b.calculate_rpn().unwrap_or(0);
        rpn_b.cmp(&rpn_a)
    });

    // Output based on format
    match global.format {
        OutputFormat::Json => {
            let summary = serde_json::json!({
                "total": total,
                "by_level": {
                    "critical": by_level.get(&RiskLevel::Critical).unwrap_or(&0),
                    "high": by_level.get(&RiskLevel::High).unwrap_or(&0),
                    "medium": by_level.get(&RiskLevel::Medium).unwrap_or(&0),
                    "low": by_level.get(&RiskLevel::Low).unwrap_or(&0),
                },
                "by_type": {
                    "design": by_type.get(&RiskType::Design).unwrap_or(&0),
                    "process": by_type.get(&RiskType::Process).unwrap_or(&0),
                },
                "rpn": {
                    "average": avg_rpn,
                    "max": max_rpn,
                    "min": min_rpn,
                    "risks_with_rpn": rpns.len(),
                },
                "unmitigated": unmitigated,
                "open_mitigations": open_mitigations,
                "top_risks": sorted_risks.iter().take(args.top).map(|r| {
                    let level = r.risk_level.or_else(|| r.determine_risk_level());
                    serde_json::json!({
                        "id": r.id.to_string(),
                        "title": r.title,
                        "rpn": r.calculate_rpn(),
                        "level": level.map(|l| format!("{:?}", l).to_lowercase()),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
        }
        _ => {
            // Human-readable output
            println!("{}", style("Risk Summary").bold().underlined());
            println!();

            // Overview section
            println!("{:<20} {}", style("Total Risks:").bold(), total);
            if !rpns.is_empty() {
                println!("{:<20} {:.1}", style("Average RPN:").bold(), avg_rpn);
                println!("{:<20} {} (max: {})", style("RPN Range:").bold(), min_rpn, max_rpn);
            }
            println!();

            // By level
            println!("{}", style("By Risk Level:").bold());
            let critical = *by_level.get(&RiskLevel::Critical).unwrap_or(&0);
            let high = *by_level.get(&RiskLevel::High).unwrap_or(&0);
            let medium = *by_level.get(&RiskLevel::Medium).unwrap_or(&0);
            let low = *by_level.get(&RiskLevel::Low).unwrap_or(&0);

            if critical > 0 {
                println!("  {} {}", style("Critical:").red().bold(), critical);
            }
            if high > 0 {
                println!("  {} {}", style("High:").yellow().bold(), high);
            }
            println!("  {:<12} {}", style("Medium:").dim(), medium);
            println!("  {:<12} {}", style("Low:").dim(), low);
            println!();

            // By type
            println!("{}", style("By Risk Type:").bold());
            println!("  {:<12} {}", "Design:", by_type.get(&RiskType::Design).unwrap_or(&0));
            println!("  {:<12} {}", "Process:", by_type.get(&RiskType::Process).unwrap_or(&0));
            println!();

            // Mitigation status
            println!("{}", style("Mitigation Status:").bold());
            if unmitigated > 0 {
                println!("  {} {}", style("Unmitigated:").red(), unmitigated);
            } else {
                println!("  {} {}", style("Unmitigated:").green(), "0");
            }
            if open_mitigations > 0 {
                println!("  {} {}", style("Open (unverified):").yellow(), open_mitigations);
            }
            let fully_mitigated = total.saturating_sub(unmitigated).saturating_sub(open_mitigations);
            println!("  {:<17} {}", "Fully mitigated:", fully_mitigated);
            println!();

            // Top N risks
            println!("{} {}", style("Top").bold(), style(format!("{} Risks by RPN:", args.top)).bold());
            println!("{}", "-".repeat(60));
            println!("{:<10} {:<6} {:<10} {}", style("ID").bold(), style("RPN").bold(), style("LEVEL").bold(), style("TITLE").bold());

            for risk in sorted_risks.iter().take(args.top) {
                let id_short = short_ids.get_short_id(&risk.id.to_string())
                    .unwrap_or_else(|| truncate_str(&risk.id.to_string(), 8));
                let rpn = risk.calculate_rpn().map(|r| r.to_string()).unwrap_or_else(|| "-".to_string());
                let level = risk.risk_level.or_else(|| risk.determine_risk_level()).unwrap_or(RiskLevel::Medium);
                let level_str = format!("{:?}", level).to_lowercase();
                let level_styled = match level {
                    RiskLevel::Critical => style(level_str).red().bold().to_string(),
                    RiskLevel::High => style(level_str).yellow().to_string(),
                    RiskLevel::Medium => style(level_str).dim().to_string(),
                    RiskLevel::Low => style(level_str).dim().to_string(),
                };
                println!("{:<10} {:<6} {:<10} {}",
                    style(id_short).cyan(),
                    rpn,
                    level_styled,
                    truncate_str(&risk.title, 35));
            }

            // Detailed breakdown by category
            if args.detailed {
                println!();
                println!("{}", style("By Category:").bold());

                let mut by_category: std::collections::HashMap<String, Vec<&Risk>> = std::collections::HashMap::new();
                for risk in &risks {
                    let cat = risk.category.clone().unwrap_or_else(|| "Uncategorized".to_string());
                    by_category.entry(cat).or_default().push(risk);
                }

                let mut categories: Vec<_> = by_category.keys().collect();
                categories.sort();

                for cat in categories {
                    let cat_risks = by_category.get(cat).unwrap();
                    let cat_rpns: Vec<u16> = cat_risks.iter()
                        .filter_map(|r| r.calculate_rpn())
                        .collect();
                    let cat_avg_rpn = if cat_rpns.is_empty() {
                        "-".to_string()
                    } else {
                        format!("{:.0}", cat_rpns.iter().map(|&r| r as f64).sum::<f64>() / cat_rpns.len() as f64)
                    };
                    println!("  {} ({} risks, avg RPN: {})", style(cat).cyan(), cat_risks.len(), cat_avg_rpn);
                }
            }
        }
    }

    Ok(())
}
