//! `pdt init` command - Initialize a new PDT project

use console::style;
use miette::{IntoDiagnostic, Result};
use std::path::Path;

use crate::core::project::{Project, ProjectError};

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Directory to initialize (default: current directory)
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,

    /// Also initialize a git repository
    #[arg(long)]
    pub git: bool,

    /// Force initialization even if .pdt/ already exists
    #[arg(long)]
    pub force: bool,
}

pub fn run(args: InitArgs) -> Result<()> {
    let path = if args.path.as_os_str() == "." {
        std::env::current_dir().into_diagnostic()?
    } else {
        args.path.clone()
    };

    // Create directory if it doesn't exist
    if !path.exists() {
        std::fs::create_dir_all(&path).into_diagnostic()?;
        println!(
            "{} Created directory {}",
            style("✓").green(),
            style(path.display()).cyan()
        );
    }

    // Initialize git if requested
    if args.git {
        init_git(&path)?;
    }

    // Initialize PDT project
    let project = if args.force {
        Project::init_force(&path)
    } else {
        Project::init(&path)
    };

    match project {
        Ok(project) => {
            println!(
                "{} Initialized PDT project at {}",
                style("✓").green(),
                style(project.root().display()).cyan()
            );
            println!();
            println!("Created project structure:");
            print_structure(project.root());
            println!();
            println!("Next steps:");
            println!(
                "  {} Create your first requirement",
                style("pdt req new").yellow()
            );
            println!(
                "  {} List all requirements",
                style("pdt req list").yellow()
            );
            println!(
                "  {} Validate project files",
                style("pdt validate").yellow()
            );
            Ok(())
        }
        Err(ProjectError::AlreadyExists(path)) => {
            println!(
                "{} PDT project already exists at {}",
                style("!").yellow(),
                style(path.display()).cyan()
            );
            println!();
            println!(
                "Use {} to reinitialize",
                style("pdt init --force").yellow()
            );
            Ok(())
        }
        Err(e) => Err(miette::miette!("{}", e)),
    }
}

fn init_git(path: &Path) -> Result<()> {
    let git_dir = path.join(".git");
    if git_dir.exists() {
        println!(
            "{} Git repository already exists",
            style("✓").green()
        );
        return Ok(());
    }

    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(path)
        .output()
        .into_diagnostic()?;

    if output.status.success() {
        println!(
            "{} Initialized git repository",
            style("✓").green()
        );

        // Create .gitignore
        let gitignore_path = path.join(".gitignore");
        if !gitignore_path.exists() {
            std::fs::write(
                &gitignore_path,
                "# PDT generated files\n/docs/generated/\n\n# Editor backups\n*.swp\n*~\n",
            )
            .into_diagnostic()?;
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(miette::miette!("Failed to initialize git: {}", stderr))
    }
}

fn print_structure(root: &Path) {
    let dirs = [
        ".pdt/",
        ".pdt/config.yaml",
        "requirements/inputs/",
        "requirements/outputs/",
        "risks/design/",
        "risks/process/",
        "bom/assemblies/",
        "bom/components/",
        "bom/quotes/",
        "tolerances/features/",
        "tolerances/mates/",
        "tolerances/stackups/",
        "verification/protocols/",
        "verification/results/",
        "validation/protocols/",
        "validation/results/",
        "manufacturing/processes/",
        "manufacturing/controls/",
    ];

    for dir in dirs {
        let full_path = root.join(dir);
        if full_path.exists() {
            let prefix = if dir.ends_with('/') { "📁" } else { "📄" };
            println!("  {} {}", prefix, style(dir).dim());
        }
    }
}
