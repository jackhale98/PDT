//! Team command - Team roster management

use clap::{Args, Subcommand};
use miette::{bail, miette, IntoDiagnostic, Result};

use crate::cli::args::GlobalOpts;
use crate::core::team::{Role, TeamMember, TeamRoster};
use crate::core::{Git, Project};

/// Team roster management
#[derive(Debug, Subcommand)]
pub enum TeamCommands {
    /// List team members
    List(TeamListArgs),
    /// Show current user's role
    Whoami,
    /// Initialize team roster template
    Init(TeamInitArgs),
    /// Add a team member
    Add(TeamAddArgs),
    /// Remove a team member
    Remove(TeamRemoveArgs),
    /// Configure GPG signing for commits and tags
    SetupSigning(SetupSigningArgs),
}

/// List team members
#[derive(Debug, Args)]
pub struct TeamListArgs {
    /// Filter by role
    #[arg(long, short = 'r')]
    pub role: Option<Role>,

    /// Output style (table, json)
    #[arg(long, short = 'o', default_value = "table")]
    pub output: String,
}

/// Initialize team roster
#[derive(Debug, Args)]
pub struct TeamInitArgs {
    /// Overwrite existing team.yaml
    #[arg(long)]
    pub force: bool,
}

/// Add a team member
#[derive(Debug, Args)]
pub struct TeamAddArgs {
    /// Member's full name
    #[arg(long)]
    pub name: String,

    /// Member's email
    #[arg(long)]
    pub email: String,

    /// Username (matches git user.name)
    #[arg(long)]
    pub username: String,

    /// Roles (comma-separated: engineering,quality,management,admin)
    #[arg(long, value_delimiter = ',')]
    pub roles: Vec<Role>,
}

/// Remove a team member
#[derive(Debug, Args)]
pub struct TeamRemoveArgs {
    /// Username to remove
    pub username: String,

    /// Skip confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Configure GPG signing for commits and tags
#[derive(Debug, Args)]
pub struct SetupSigningArgs {
    /// GPG key ID to use for signing (if not provided, will detect from git config)
    #[arg(long, short = 'k')]
    pub key_id: Option<String>,

    /// Configure for this repository only (not global)
    #[arg(long)]
    pub local: bool,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show current signing configuration without making changes
    #[arg(long)]
    pub status: bool,
}

impl TeamCommands {
    pub fn run(&self, global: &GlobalOpts) -> Result<()> {
        match self {
            TeamCommands::List(args) => args.run(global),
            TeamCommands::Whoami => run_whoami(global),
            TeamCommands::Init(args) => args.run(global),
            TeamCommands::Add(args) => args.run(global),
            TeamCommands::Remove(args) => args.run(global),
            TeamCommands::SetupSigning(args) => args.run(global),
        }
    }
}

impl TeamListArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;

        let Some(roster) = TeamRoster::load(&project) else {
            bail!("No team roster found. Run 'tdt team init' to create one.");
        };

        let members: Vec<&TeamMember> = if let Some(ref role) = self.role {
            roster.members_with_role(*role).collect()
        } else {
            roster.active_members().collect()
        };

        if members.is_empty() {
            println!("No team members found.");
            return Ok(());
        }

        match self.output.as_str() {
            "json" => {
                let json = serde_json::to_string_pretty(&members).into_diagnostic()?;
                println!("{}", json);
            }
            _ => {
                println!("\nTeam Members\n");
                println!("{:<20} {:<25} {:<15} ROLES", "NAME", "EMAIL", "USERNAME");
                println!("{}", "-".repeat(75));

                for member in members {
                    let roles: Vec<String> = member.roles.iter().map(|r| r.to_string()).collect();
                    println!(
                        "{:<20} {:<25} {:<15} {}",
                        truncate(&member.name, 18),
                        truncate(&member.email, 23),
                        truncate(&member.username, 13),
                        roles.join(", ")
                    );
                }
            }
        }

        Ok(())
    }
}

fn run_whoami(_global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;

    let Some(roster) = TeamRoster::load(&project) else {
        bail!("No team roster found. Run 'tdt team init' to create one.");
    };

    let Some(user) = roster.current_user() else {
        // Try to show git user info
        if let Ok(output) = std::process::Command::new("git")
            .args(["config", "user.name"])
            .output()
        {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                bail!(
                    "You ({}) are not in the team roster.\n\
                     Add yourself with: tdt team add --name \"{}\" --email your@email.com --username {} --roles engineering",
                    name, name, name
                );
            }
        }
        bail!("You are not in the team roster and git user.name is not configured.");
    };

    println!("\nCurrent User\n");
    println!("Name:     {}", user.name);
    println!("Email:    {}", user.email);
    println!("Username: {}", user.username);
    println!(
        "Roles:    {}",
        user.roles
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Active:   {}", user.active);

    // Show what they can approve
    println!("\nAuthorization:");
    println!(
        "  Can approve: {}",
        if user.is_admin() {
            "All entities (admin)".to_string()
        } else {
            user.roles
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!(
        "  Can release: {}",
        if roster.can_release(user) {
            "Yes"
        } else {
            "No"
        }
    );

    Ok(())
}

impl TeamInitArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let team_path = project.tdt_dir().join("team.yaml");

        if team_path.exists() && !self.force {
            bail!(
                "Team roster already exists at {}\n\
                 Use --force to overwrite.",
                team_path.display()
            );
        }

        let template = TeamRoster::default_template();
        std::fs::write(&team_path, template).into_diagnostic()?;

        println!("Created team roster at {}", team_path.display());
        println!("\nEdit this file to add your team members, or use:");
        println!("  tdt team add --name \"Jane Smith\" --email jane@co.com --username jsmith --roles engineering,quality");

        Ok(())
    }
}

impl TeamAddArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let team_path = project.tdt_dir().join("team.yaml");

        let mut roster = if team_path.exists() {
            TeamRoster::load(&project).unwrap_or_default()
        } else {
            TeamRoster::default()
        };

        // Check if user already exists
        if roster.find_member(&self.username).is_some() {
            bail!(
                "User '{}' already exists in the team roster.\n\
                 Use 'tdt team remove {}' first to update.",
                self.username,
                self.username
            );
        }

        let member = TeamMember {
            name: self.name.clone(),
            email: self.email.clone(),
            username: self.username.clone(),
            roles: self.roles.clone(),
            active: true,
        };

        roster.add_member(member);
        roster.save(&project).into_diagnostic()?;

        println!("Added {} ({}) to team roster", self.name, self.username);
        println!(
            "Roles: {}",
            self.roles
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );

        Ok(())
    }
}

impl TeamRemoveArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;

        let mut roster =
            TeamRoster::load(&project).ok_or_else(|| miette!("No team roster found."))?;

        let member = roster
            .find_member(&self.username)
            .ok_or_else(|| miette!("User '{}' not found in team roster.", self.username))?;

        let name = member.name.clone();

        // Confirm if not --yes
        if !self.yes {
            print!(
                "Remove {} ({}) from team roster? [y/N] ",
                name, self.username
            );
            std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).into_diagnostic()?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        if roster.remove_member(&self.username) {
            roster.save(&project).into_diagnostic()?;
            println!("Removed {} ({}) from team roster", name, self.username);
        } else {
            bail!("Failed to remove user.");
        }

        Ok(())
    }
}

impl SetupSigningArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let git = Git::new(project.root());

        if !git.is_repo() {
            bail!("Not a git repository.");
        }

        // Status mode - just show current configuration
        if self.status {
            return self.show_status(&git);
        }

        // Check if a signing key is available
        let key_id = if let Some(ref k) = self.key_id {
            k.clone()
        } else if let Some(k) = git.signing_key() {
            k
        } else {
            bail!(
                "No GPG signing key configured.\n\n\
                 To set up GPG signing:\n\
                 1. Generate a GPG key:  gpg --full-generate-key\n\
                 2. List your keys:      gpg --list-secret-keys --keyid-format=long\n\
                 3. Provide the key ID:  tdt team setup-signing --key-id YOUR_KEY_ID\n\n\
                 For detailed instructions:\n\
                 https://docs.github.com/en/authentication/managing-commit-signature-verification"
            );
        };

        let scope = if self.local { "--local" } else { "--global" };
        let scope_desc = if self.local { "repository" } else { "global" };

        println!("GPG Signing Configuration");
        println!("========================\n");
        println!("This will configure git to automatically sign all commits and tags.");
        println!();
        println!("  Key ID:        {}", key_id);
        println!("  Scope:         {} ({})", scope_desc, scope);
        println!();
        println!("Commands to run:");
        println!("  git config {} user.signingkey {}", scope, key_id);
        println!("  git config {} commit.gpgsign true", scope);
        println!("  git config {} tag.gpgSign true", scope);
        println!();

        // Confirm if not --yes
        if !self.yes {
            print!("Proceed? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).into_diagnostic()?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        // Run the configuration commands
        let args_base: Vec<&str> = if self.local {
            vec!["config", "--local"]
        } else {
            vec!["config", "--global"]
        };

        // Set signing key
        let mut args = args_base.clone();
        args.extend(["user.signingkey", &key_id]);
        git.run_checked(&args)
            .map_err(|e| miette!("Failed to set user.signingkey: {}", e))?;
        println!("  ✓ Set user.signingkey = {}", key_id);

        // Enable commit signing
        let mut args = args_base.clone();
        args.extend(["commit.gpgsign", "true"]);
        git.run_checked(&args)
            .map_err(|e| miette!("Failed to set commit.gpgsign: {}", e))?;
        println!("  ✓ Set commit.gpgsign = true");

        // Enable tag signing
        let mut args = args_base.clone();
        args.extend(["tag.gpgSign", "true"]);
        git.run_checked(&args)
            .map_err(|e| miette!("Failed to set tag.gpgSign: {}", e))?;
        println!("  ✓ Set tag.gpgSign = true");

        println!("\nGPG signing configured successfully!");
        println!("All commits and tags will now be automatically signed.");
        println!();
        println!("To verify, run: git config --{} --get-regexp '.*sign'", if self.local { "local" } else { "global" });

        Ok(())
    }

    fn show_status(&self, git: &Git) -> Result<()> {
        println!("GPG Signing Status");
        println!("==================\n");

        // Check signing key
        let key = git.signing_key();
        if let Some(ref k) = key {
            println!("  user.signingkey:   {} ✓", k);
        } else {
            println!("  user.signingkey:   (not configured)");
        }

        // Check commit.gpgsign
        if git.commit_gpgsign_enabled() {
            println!("  commit.gpgsign:    true ✓");
        } else {
            println!("  commit.gpgsign:    false");
        }

        // Check tag.gpgSign
        if git.tag_gpgsign_enabled() {
            println!("  tag.gpgSign:       true ✓");
        } else {
            println!("  tag.gpgSign:       false");
        }

        println!();

        // Determine overall status
        if key.is_some() && git.commit_gpgsign_enabled() && git.tag_gpgsign_enabled() {
            println!("Status: Fully configured ✓");
            println!("All commits and tags will be signed automatically.");
        } else if key.is_some() {
            println!("Status: Partially configured");
            println!("Signing key is set but auto-signing is not enabled.");
            println!("Run 'tdt team setup-signing' to enable auto-signing.");
        } else {
            println!("Status: Not configured");
            println!("Run 'tdt team setup-signing --key-id YOUR_KEY_ID' to configure.");
        }

        Ok(())
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
