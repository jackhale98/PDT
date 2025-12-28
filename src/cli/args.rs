//! CLI argument definitions using clap derive

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::cli::commands::{
    asm::AsmCommands,
    baseline::BaselineCommands,
    blame::BlameArgs,
    bulk::BulkCommands,
    cache::CacheCommands,
    capa::CapaCommands,
    cmp::CmpCommands,
    completions::CompletionsArgs,
    config::ConfigCommands,
    ctrl::CtrlCommands,
    dev::DevCommands,
    diff::DiffArgs,
    dmm::DmmArgs,
    dsm::DsmArgs,
    feat::FeatCommands,
    haz::HazCommands,
    history::HistoryArgs,
    import::ImportArgs,
    init::InitArgs,
    link::LinkCommands,
    log::LogArgs,
    lot::LotCommands,
    mate::MateCommands,
    ncr::NcrCommands,
    proc::ProcCommands,
    quote::QuoteCommands,
    recent::RecentArgs,
    report::ReportCommands,
    req::ReqCommands,
    risk::RiskCommands,
    rslt::RsltCommands,
    schema::SchemaCommands,
    search::SearchArgs,
    status::StatusArgs,
    sup::SupCommands,
    tags::TagsCommands,
    test::TestCommands,
    tol::TolCommands,
    trace::TraceCommands,
    validate::ValidateArgs,
    where_used::WhereUsedArgs,
    work::WorkCommands,
    workflow::{ApproveArgs, RejectArgs, ReleaseArgs, ReviewCommands, SubmitArgs, TeamCommands},
};

/// Custom help template with grouped commands
const HELP_TEMPLATE: &str = "\
{before-help}{name} {version}
{about-with-newline}
{usage-heading} {usage}

PROJECT:
  init        Initialize a new TDT project
  status      Show project status dashboard
  validate    Validate project files against schemas

REQUIREMENTS & RISKS:
  req         Requirement management (new, list, show, edit)
  haz         Hazard management (new, list, show, edit)
  risk        Risk/FMEA management (new, list, show, edit, summary)

VERIFICATION & VALIDATION:
  test        Test protocol management (new, list, show, edit)
  rslt        Test result management (new, list, show, edit)

BILL OF MATERIALS:
  cmp         Component management (new, list, show, edit)
  asm         Assembly management (new, list, show, edit, cost, mass)

PROCUREMENT:
  quote       Quote management (new, list, show, edit)
  sup         Supplier management (new, list, show, edit)

MANUFACTURING:
  proc        Manufacturing process management (new, list, show, edit)
  ctrl        Control plan item management (new, list, show, edit)
  work        Work instruction management (new, list, show, edit)
  lot         Production lot/batch management (new, list, show, step, complete)
  dev         Process deviation management (new, list, show, approve, expire)

QUALITY:
  ncr         Non-conformance report management (new, list, show, edit)
  capa        Corrective/preventive action management (new, list, show, edit)

TOLERANCE ANALYSIS:
  feat        Feature management - dimensional features on components
  mate        Mate management - 1:1 feature contacts with fit calculation
  tol         Tolerance stackup analysis (worst-case, RSS, Monte Carlo)

TRACEABILITY & REPORTS:
  link        Manage links between entities (add, remove, show)
  trace       Traceability queries (from, to, coverage)
  dsm         Design Structure Matrix for component interactions
  dmm         Domain Mapping Matrix for cross-entity analysis
  where-used  Find where an entity is used/referenced
  report      Generate engineering reports (rvm, fmea, bom, etc.)

VERSION CONTROL:
  history     View git history for an entity
  blame       View git blame for an entity
  diff        View git diff for an entity
  baseline    Baseline management (create, compare, list, changed)

WORKFLOW (opt-in):
  submit      Submit entities for review (creates PR)
  approve     Approve entities under review
  reject      Reject entities back to draft
  release     Release approved entities
  review      View pending reviews (list, summary)
  team        Team roster management (list, whoami, init, add, remove)

UTILITIES:
  import      Import entities from CSV files
  bulk        Bulk operations on multiple entities
  cache       Entity cache management (rebuild, sync, status, query)
  config      View and modify TDT configuration (show, set, unset)
  search      Search across all entity types
  recent      Show recently modified entities
  tags        View and manage entity tags (list, show)
  schema      View entity schemas (list, show) - for AI agent ergonomics
  completions Generate shell completion scripts (bash, zsh, fish, powershell)
  help        Print this message or the help of the given subcommand(s)

OPTIONS:
{options}
{after-help}";

#[derive(Parser)]
#[command(name = "tdt")]
#[command(author, version, about = "Tessera Design Toolkit")]
#[command(
    long_about = "A Unix-style toolkit for managing engineering artifacts as plain text files under git version control."
)]
#[command(propagate_version = true)]
#[command(help_template = HELP_TEMPLATE)]
#[command(subcommand_required = true)]
#[command(disable_help_subcommand = false)]
#[command(infer_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub global: GlobalOpts,
}

#[derive(clap::Args, Clone, Debug)]
pub struct GlobalOpts {
    /// Output format
    #[arg(long, short = 'f', global = true, default_value = "auto")]
    pub format: OutputFormat,

    /// Suppress non-essential output
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Enable verbose output
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// Project root (default: auto-detect by finding .tdt/)
    #[arg(long, global = true)]
    pub project: Option<PathBuf>,
}

/// Subcommands grouped logically by function area
#[derive(Subcommand)]
pub enum Commands {
    // ─────────────────────────────────────────────────────────────────────
    // PROJECT MANAGEMENT
    // ─────────────────────────────────────────────────────────────────────
    /// Initialize a new TDT project
    Init(InitArgs),

    /// Show project status dashboard
    Status(StatusArgs),

    /// Validate project files against schemas
    Validate(ValidateArgs),

    // ─────────────────────────────────────────────────────────────────────
    // REQUIREMENTS & RISKS
    // ─────────────────────────────────────────────────────────────────────
    /// Requirement management (new, list, show, edit)
    #[command(subcommand)]
    Req(ReqCommands),

    /// Hazard management (new, list, show, edit)
    #[command(subcommand)]
    Haz(HazCommands),

    /// Risk/FMEA management (new, list, show, edit, summary)
    #[command(subcommand)]
    Risk(RiskCommands),

    // ─────────────────────────────────────────────────────────────────────
    // VERIFICATION & VALIDATION
    // ─────────────────────────────────────────────────────────────────────
    /// Test protocol management (new, list, show, edit)
    #[command(subcommand)]
    Test(TestCommands),

    /// Test result management (new, list, show, edit)
    #[command(subcommand)]
    Rslt(RsltCommands),

    // ─────────────────────────────────────────────────────────────────────
    // BILL OF MATERIALS
    // ─────────────────────────────────────────────────────────────────────
    /// Component management (new, list, show, edit)
    #[command(subcommand)]
    Cmp(CmpCommands),

    /// Assembly management (new, list, show, edit, cost, mass)
    #[command(subcommand)]
    Asm(AsmCommands),

    // ─────────────────────────────────────────────────────────────────────
    // PROCUREMENT
    // ─────────────────────────────────────────────────────────────────────
    /// Quote management (new, list, show, edit)
    #[command(subcommand)]
    Quote(QuoteCommands),

    /// Supplier management (new, list, show, edit)
    #[command(subcommand)]
    Sup(SupCommands),

    // ─────────────────────────────────────────────────────────────────────
    // MANUFACTURING
    // ─────────────────────────────────────────────────────────────────────
    /// Manufacturing process management (new, list, show, edit)
    #[command(subcommand)]
    Proc(ProcCommands),

    /// Control plan item management (new, list, show, edit)
    #[command(subcommand)]
    Ctrl(CtrlCommands),

    /// Work instruction management (new, list, show, edit)
    #[command(subcommand)]
    Work(WorkCommands),

    /// Production lot/batch management (new, list, show, step, complete)
    #[command(subcommand)]
    Lot(LotCommands),

    /// Process deviation management (new, list, show, approve, expire)
    #[command(subcommand)]
    Dev(DevCommands),

    // ─────────────────────────────────────────────────────────────────────
    // QUALITY
    // ─────────────────────────────────────────────────────────────────────
    /// Non-conformance report management (new, list, show, edit)
    #[command(subcommand)]
    Ncr(NcrCommands),

    /// Corrective/preventive action management (new, list, show, edit)
    #[command(subcommand)]
    Capa(CapaCommands),

    // ─────────────────────────────────────────────────────────────────────
    // TOLERANCE ANALYSIS
    // ─────────────────────────────────────────────────────────────────────
    /// Feature management - dimensional features on components
    #[command(subcommand)]
    Feat(FeatCommands),

    /// Mate management - 1:1 feature contacts with fit calculation
    #[command(subcommand)]
    Mate(MateCommands),

    /// Tolerance stackup analysis (worst-case, RSS, Monte Carlo)
    #[command(subcommand)]
    Tol(TolCommands),

    // ─────────────────────────────────────────────────────────────────────
    // TRACEABILITY & REPORTS
    // ─────────────────────────────────────────────────────────────────────
    /// Manage links between entities (add, remove, show)
    #[command(subcommand)]
    Link(LinkCommands),

    /// View workflow activity log (approvals, releases) across all entities
    Log(LogArgs),

    /// Traceability queries (from, to, coverage)
    #[command(subcommand)]
    Trace(TraceCommands),

    /// Design Structure Matrix for component interactions
    Dsm(DsmArgs),

    /// Domain Mapping Matrix for cross-entity analysis
    Dmm(DmmArgs),

    /// Find where an entity is used/referenced
    WhereUsed(WhereUsedArgs),

    /// Generate engineering reports (rvm, fmea, bom, etc.)
    #[command(subcommand)]
    Report(ReportCommands),

    // ─────────────────────────────────────────────────────────────────────
    // VERSION CONTROL
    // ─────────────────────────────────────────────────────────────────────
    /// View git history for an entity
    History(HistoryArgs),

    /// View git blame for an entity
    Blame(BlameArgs),

    /// View git diff for an entity
    Diff(DiffArgs),

    /// Baseline management (create, compare, list, changed)
    #[command(subcommand)]
    Baseline(BaselineCommands),

    // ─────────────────────────────────────────────────────────────────────
    // WORKFLOW (opt-in)
    // ─────────────────────────────────────────────────────────────────────
    /// Submit entities for review (creates PR)
    Submit(SubmitArgs),

    /// Approve entities under review
    Approve(ApproveArgs),

    /// Reject entities back to draft
    Reject(RejectArgs),

    /// Release approved entities
    Release(ReleaseArgs),

    /// View pending reviews (list, summary)
    #[command(subcommand)]
    Review(ReviewCommands),

    /// Team roster management (list, whoami, init, add, remove)
    #[command(subcommand)]
    Team(TeamCommands),

    // ─────────────────────────────────────────────────────────────────────
    // UTILITIES
    // ─────────────────────────────────────────────────────────────────────
    /// Import entities from CSV files
    Import(ImportArgs),

    /// Bulk operations on multiple entities
    #[command(subcommand)]
    Bulk(BulkCommands),

    /// Entity cache management (rebuild, sync, status, query)
    #[command(subcommand)]
    Cache(CacheCommands),

    /// View and modify TDT configuration (show, set, unset)
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Search across all entity types
    Search(SearchArgs),

    /// Show recently modified entities
    Recent(RecentArgs),

    /// View and manage entity tags (list, show)
    #[command(subcommand)]
    Tags(TagsCommands),

    /// View entity schemas (list, show) - for AI agent ergonomics
    #[command(subcommand)]
    Schema(SchemaCommands),

    /// Generate shell completion scripts
    Completions(CompletionsArgs),
}

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Automatically detect based on context (yaml for show, tsv for list)
    #[default]
    Auto,
    /// YAML format (full fidelity)
    Yaml,
    /// Tab-separated values (for piping)
    Tsv,
    /// JSON format (for programming)
    Json,
    /// CSV format (for spreadsheets)
    Csv,
    /// Markdown tables
    Md,
    /// Just IDs, one per line (full ULIDs for git collaboration)
    Id,
    /// Short IDs only (e.g., REQ@1) for piping between tdt commands
    #[value(name = "short-id")]
    ShortId,
    /// File path only (for new commands - enables easy editing after creation)
    Path,
}
