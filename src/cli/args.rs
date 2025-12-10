//! CLI argument definitions using clap derive

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::cli::commands::{
    init::InitArgs,
    link::LinkCommands,
    req::ReqCommands,
    risk::RiskCommands,
    rslt::RsltCommands,
    test::TestCommands,
    trace::TraceCommands,
    validate::ValidateArgs,
};

#[derive(Parser)]
#[command(name = "pdt")]
#[command(author, version, about = "Plain-text Product Development Toolkit")]
#[command(long_about = "A Unix-style toolkit for managing product development artifacts as plain text files under git version control.")]
#[command(propagate_version = true)]
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

    /// Project root (default: auto-detect by finding .pdt/)
    #[arg(long, global = true)]
    pub project: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new PDT project
    Init(InitArgs),

    /// Requirement management
    #[command(subcommand)]
    Req(ReqCommands),

    /// Risk/FMEA management
    #[command(subcommand)]
    Risk(RiskCommands),

    /// Test protocol management (verification/validation)
    #[command(subcommand)]
    Test(TestCommands),

    /// Test result management
    #[command(subcommand)]
    Rslt(RsltCommands),

    /// Validate project files against schemas
    Validate(ValidateArgs),

    /// Manage links between entities
    #[command(subcommand)]
    Link(LinkCommands),

    /// Traceability queries and reports
    #[command(subcommand)]
    Trace(TraceCommands),
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
    /// Just IDs, one per line
    Id,
}
