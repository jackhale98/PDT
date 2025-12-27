//! Team command - Team roster management

use clap::{Args, Subcommand};
use miette::{bail, miette, IntoDiagnostic, Result};

use crate::cli::args::GlobalOpts;
use crate::core::team::{Role, TeamMember, TeamRoster};
use crate::core::Project;

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

impl TeamCommands {
    pub fn run(&self, global: &GlobalOpts) -> Result<()> {
        match self {
            TeamCommands::List(args) => args.run(global),
            TeamCommands::Whoami => run_whoami(global),
            TeamCommands::Init(args) => args.run(global),
            TeamCommands::Add(args) => args.run(global),
            TeamCommands::Remove(args) => args.run(global),
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

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
