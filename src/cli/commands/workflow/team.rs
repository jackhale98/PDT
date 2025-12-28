//! Team command - Team roster management

use clap::{Args, Subcommand};
use miette::{bail, miette, IntoDiagnostic, Result};
use std::path::PathBuf;

use crate::cli::args::GlobalOpts;
use crate::core::team::{Role, SigningFormat, TeamMember, TeamRoster};
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
    /// Configure commit and tag signing (GPG, SSH, or gitsign)
    SetupSigning(SetupSigningArgs),
    /// Export your public signing key to the team keyring (.tdt/keys/)
    AddKey(AddKeyArgs),
    /// Import GPG public keys from the team keyring into your local keyring
    ImportKeys(ImportKeysArgs),
    /// Regenerate SSH allowed_signers file from team keys
    SyncKeys(SyncKeysArgs),
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

    /// Signing format used by this member (gpg, ssh, or gitsign)
    #[arg(long, short = 's', value_enum)]
    pub signing_format: Option<SigningFormat>,
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

/// Export your public signing key to the team keyring
#[derive(Debug, Args)]
pub struct AddKeyArgs {
    /// Signing format (auto-detected from git config if not specified)
    #[arg(long, short = 'm', value_enum)]
    pub method: Option<SigningFormat>,

    /// Path to public key file (auto-detected if not specified)
    #[arg(long, short = 'k')]
    pub key: Option<PathBuf>,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Import GPG public keys from team keyring
#[derive(Debug, Args)]
pub struct ImportKeysArgs {
    /// Import only a specific user's key
    #[arg(long, short = 'u')]
    pub user: Option<String>,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Regenerate SSH allowed_signers file
#[derive(Debug, Args)]
pub struct SyncKeysArgs {
    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Configure commit and tag signing
#[derive(Debug, Args)]
pub struct SetupSigningArgs {
    /// Signing method to use (gpg, ssh, or gitsign)
    #[arg(long, short = 'm', value_enum, default_value = "gpg")]
    pub method: SigningFormat,

    /// Key ID or path (GPG key ID, or SSH key path like ~/.ssh/id_ed25519.pub)
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
            TeamCommands::AddKey(args) => args.run(global),
            TeamCommands::ImportKeys(args) => args.run(global),
            TeamCommands::SyncKeys(args) => args.run(global),
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
            signing_format: self.signing_format,
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
        if let Some(ref format) = self.signing_format {
            println!("Signing: {}", format);
        }

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

        // Dispatch to method-specific handler
        match self.method {
            SigningFormat::Gpg => self.setup_gpg(&git),
            SigningFormat::Ssh => self.setup_ssh(&git),
            SigningFormat::Gitsign => self.setup_gitsign(&git),
        }
    }

    fn setup_gpg(&self, git: &Git) -> Result<()> {
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
                 3. Provide the key ID:  tdt team setup-signing --format gpg --key-id YOUR_KEY_ID\n\n\
                 For detailed instructions:\n\
                 https://docs.github.com/en/authentication/managing-commit-signature-verification"
            );
        };

        let scope = if self.local { "--local" } else { "--global" };
        let scope_desc = if self.local { "repository" } else { "global" };

        println!("GPG Signing Configuration");
        println!("=========================\n");
        println!("This will configure git to automatically sign all commits and tags using GPG.");
        println!();
        println!("  Format:        GPG (traditional)");
        println!("  Key ID:        {}", key_id);
        println!("  Scope:         {} ({})", scope_desc, scope);
        println!();
        println!("Commands to run:");
        println!("  git config {} gpg.format openpgp", scope);
        println!("  git config {} user.signingkey {}", scope, key_id);
        println!("  git config {} commit.gpgsign true", scope);
        println!("  git config {} tag.gpgSign true", scope);
        println!();

        if !self.confirm()? {
            return Ok(());
        }

        let args_base = self.config_args();

        // Set format to openpgp (explicit, in case it was changed)
        self.run_config(git, &args_base, "gpg.format", "openpgp")?;

        // Set signing key
        self.run_config(git, &args_base, "user.signingkey", &key_id)?;

        // Enable commit signing
        self.run_config(git, &args_base, "commit.gpgsign", "true")?;

        // Enable tag signing
        self.run_config(git, &args_base, "tag.gpgSign", "true")?;

        println!("\nGPG signing configured successfully!");
        println!("All commits and tags will now be automatically signed with GPG.");
        self.print_verify_command();

        Ok(())
    }

    fn setup_ssh(&self, git: &Git) -> Result<()> {
        // Find SSH key
        let key_path = if let Some(ref k) = self.key_id {
            k.clone()
        } else {
            // Try to find existing SSH keys
            let home = std::env::var("HOME").unwrap_or_default();
            let candidates = [
                format!("{}/.ssh/id_ed25519.pub", home),
                format!("{}/.ssh/id_rsa.pub", home),
                format!("{}/.ssh/id_ecdsa.pub", home),
            ];

            candidates
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .cloned()
                .ok_or_else(|| {
                    miette!(
                        "No SSH key found.\n\n\
                         To set up SSH signing:\n\
                         1. Generate an SSH key:  ssh-keygen -t ed25519 -C \"your_email@example.com\"\n\
                         2. Provide the key path: tdt team setup-signing --format ssh --key-id ~/.ssh/id_ed25519.pub\n\n\
                         For detailed instructions:\n\
                         https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification#ssh-commit-signature-verification"
                    )
                })?
        };

        // Verify key exists
        if !std::path::Path::new(&key_path).exists() {
            bail!("SSH key not found at: {}", key_path);
        }

        let scope = if self.local { "--local" } else { "--global" };
        let scope_desc = if self.local { "repository" } else { "global" };

        println!("SSH Signing Configuration");
        println!("=========================\n");
        println!("This will configure git to automatically sign all commits and tags using SSH.");
        println!("Requires git 2.34 or later.");
        println!();
        println!("  Format:        SSH (modern, simpler)");
        println!("  Key:           {}", key_path);
        println!("  Scope:         {} ({})", scope_desc, scope);
        println!();
        println!("Commands to run:");
        println!("  git config {} gpg.format ssh", scope);
        println!("  git config {} user.signingkey {}", scope, key_path);
        println!("  git config {} commit.gpgsign true", scope);
        println!("  git config {} tag.gpgSign true", scope);
        println!();

        if !self.confirm()? {
            return Ok(());
        }

        let args_base = self.config_args();

        // Set format to ssh
        self.run_config(git, &args_base, "gpg.format", "ssh")?;

        // Set signing key
        self.run_config(git, &args_base, "user.signingkey", &key_path)?;

        // Enable commit signing
        self.run_config(git, &args_base, "commit.gpgsign", "true")?;

        // Enable tag signing
        self.run_config(git, &args_base, "tag.gpgSign", "true")?;

        println!("\nSSH signing configured successfully!");
        println!("All commits and tags will now be automatically signed with SSH.");
        println!();
        println!("Note: Add your SSH public key to GitHub/GitLab for verification badges.");
        self.print_verify_command();

        Ok(())
    }

    fn setup_gitsign(&self, git: &Git) -> Result<()> {
        // Check if gitsign is installed
        let gitsign_check = std::process::Command::new("gitsign")
            .arg("--version")
            .output();

        if gitsign_check.is_err() || !gitsign_check.unwrap().status.success() {
            bail!(
                "gitsign is not installed.\n\n\
                 To install gitsign:\n\
                 \n\
                 macOS:   brew install sigstore/tap/gitsign\n\
                 Linux:   See https://github.com/sigstore/gitsign#installation\n\
                 Windows: scoop bucket add sigstore https://github.com/sigstore/scoop-bucket\n\
                          scoop install gitsign\n\
                 \n\
                 For detailed instructions:\n\
                 https://github.com/sigstore/gitsign"
            );
        }

        let scope = if self.local { "--local" } else { "--global" };
        let scope_desc = if self.local { "repository" } else { "global" };

        println!("Sigstore gitsign Configuration");
        println!("===============================\n");
        println!("This will configure git to use Sigstore's keyless signing.");
        println!("Commits are signed using your OIDC identity (GitHub, Google, Microsoft).");
        println!("Signatures are recorded in the Rekor transparency log for auditability.");
        println!();
        println!("  Format:        gitsign (keyless, OIDC-based)");
        println!("  Identity:      Your GitHub/Google/Microsoft account");
        println!("  Transparency:  Recorded in Rekor public log");
        println!("  Scope:         {} ({})", scope_desc, scope);
        println!();
        println!("Commands to run:");
        println!("  git config {} gpg.format x509", scope);
        println!("  git config {} gpg.x509.program gitsign", scope);
        println!("  git config {} commit.gpgsign true", scope);
        println!("  git config {} tag.gpgSign true", scope);
        println!();

        if !self.confirm()? {
            return Ok(());
        }

        let args_base = self.config_args();

        // Set format to x509 (used by gitsign)
        self.run_config(git, &args_base, "gpg.format", "x509")?;

        // Set gitsign as the x509 program
        self.run_config(git, &args_base, "gpg.x509.program", "gitsign")?;

        // Enable commit signing
        self.run_config(git, &args_base, "commit.gpgsign", "true")?;

        // Enable tag signing
        self.run_config(git, &args_base, "tag.gpgSign", "true")?;

        println!("\ngitsign configured successfully!");
        println!("All commits and tags will now be signed using Sigstore.");
        println!();
        println!("When you commit, a browser will open for OIDC authentication.");
        println!("Your signature will be recorded in the Rekor transparency log.");
        println!();
        println!("Verify signatures with: gitsign verify --certificate-identity=YOUR_EMAIL");
        self.print_verify_command();

        Ok(())
    }

    fn config_args(&self) -> Vec<&str> {
        if self.local {
            vec!["config", "--local"]
        } else {
            vec!["config", "--global"]
        }
    }

    fn run_config(&self, git: &Git, args_base: &[&str], key: &str, value: &str) -> Result<()> {
        let mut args: Vec<&str> = args_base.to_vec();
        args.extend([key, value]);
        git.run_checked(&args)
            .map_err(|e| miette!("Failed to set {}: {}", key, e))?;
        println!("  ✓ Set {} = {}", key, value);
        Ok(())
    }

    fn confirm(&self) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        print!("Proceed? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(false);
        }
        Ok(true)
    }

    fn print_verify_command(&self) {
        println!();
        println!(
            "To verify, run: git config --{} --get-regexp '.*sign'",
            if self.local { "local" } else { "global" }
        );
    }

    fn show_status(&self, git: &Git) -> Result<()> {
        println!("Signing Configuration Status");
        println!("============================\n");

        // Check gpg.format
        let format = git
            .run_checked(&["config", "gpg.format"])
            .map(|o| o.stdout)
            .unwrap_or_default();

        let format_display = match format.as_str() {
            "ssh" => "SSH",
            "x509" => "x509 (gitsign)",
            "openpgp" | "" => "GPG (default)",
            other => other,
        };
        println!("  gpg.format:        {}", format_display);

        // Check signing key
        let key = git.signing_key();
        if let Some(ref k) = key {
            println!("  user.signingkey:   {} ✓", k);
        } else {
            println!("  user.signingkey:   (not configured)");
        }

        // Check x509 program (for gitsign)
        if format == "x509" {
            let program = git
                .run_checked(&["config", "gpg.x509.program"])
                .map(|o| o.stdout)
                .unwrap_or_default();
            if !program.is_empty() {
                println!("  gpg.x509.program:  {}", program);
            }
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

impl AddKeyArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let git = Git::new(project.root());

        if !git.is_repo() {
            bail!("Not a git repository.");
        }

        // Get current user from roster
        let roster = TeamRoster::load(&project);
        let current_user = roster.as_ref().and_then(|r| r.current_user());

        let username = current_user
            .map(|u| u.username.clone())
            .or_else(|| {
                git.run_checked(&["config", "user.name"])
                    .ok()
                    .map(|o| o.stdout)
            })
            .ok_or_else(|| {
                miette!(
                    "Cannot determine username. Configure git user.name or add yourself to the team roster."
                )
            })?;

        // Determine signing format
        let format = self.method.or_else(|| {
            // Try to detect from git config
            git.run_checked(&["config", "gpg.format"])
                .ok()
                .and_then(|o| match o.stdout.as_str() {
                    "ssh" => Some(SigningFormat::Ssh),
                    "x509" => Some(SigningFormat::Gitsign),
                    _ => Some(SigningFormat::Gpg),
                })
        }).unwrap_or(SigningFormat::Gpg);

        // Gitsign doesn't need a key file stored
        if format == SigningFormat::Gitsign {
            println!("gitsign uses keyless OIDC-based signing - no key file needed.");
            println!("Signatures are verified via the Rekor transparency log using your email.");
            return Ok(());
        }

        // Get the key content
        let (key_content, key_display) = match format {
            SigningFormat::Gpg => self.get_gpg_public_key(&git)?,
            SigningFormat::Ssh => self.get_ssh_public_key(&git)?,
            SigningFormat::Gitsign => unreachable!(),
        };

        // Determine target path
        let keys_dir = project.tdt_dir().join("keys");
        let format_dir = keys_dir.join(format.to_string());
        let extension = match format {
            SigningFormat::Gpg => "asc",
            SigningFormat::Ssh => "pub",
            SigningFormat::Gitsign => unreachable!(),
        };
        let key_file = format_dir.join(format!("{}.{}", username, extension));

        println!("Export Public Key to Team Keyring");
        println!("==================================\n");
        println!("  Username:    {}", username);
        println!("  Format:      {}", format);
        println!("  Key:         {}", key_display);
        println!("  Destination: {}", key_file.display());
        println!();

        if !self.confirm()? {
            return Ok(());
        }

        // Create directory and write key
        std::fs::create_dir_all(&format_dir).into_diagnostic()?;
        std::fs::write(&key_file, &key_content).into_diagnostic()?;

        println!("✓ Exported public key to {}", key_file.display());
        println!();
        println!("Next steps:");
        println!("  1. Commit the key: git add {} && git commit -m \"Add {} public key\"",
                 key_file.display(), username);
        if format == SigningFormat::Ssh {
            println!("  2. Run 'tdt team sync-keys' to update allowed_signers file");
        }
        println!();
        println!("Teammates can now verify your signatures.");

        // Update team roster if user exists
        if let Some(mut roster) = roster {
            if let Some(member) = roster.members.iter_mut().find(|m| m.username == username) {
                if member.signing_format.is_none() {
                    member.signing_format = Some(format);
                    roster.save(&project).into_diagnostic()?;
                    println!("✓ Updated signing_format in team roster");
                }
            }
        }

        Ok(())
    }

    fn get_gpg_public_key(&self, git: &Git) -> Result<(String, String)> {
        let key_id = if let Some(ref path) = self.key {
            // If a file path is provided, read it directly
            if path.exists() {
                let content = std::fs::read_to_string(path).into_diagnostic()?;
                let display = path.display().to_string();
                return Ok((content, display));
            }
            // Otherwise treat as key ID
            path.to_string_lossy().to_string()
        } else {
            git.signing_key().ok_or_else(|| {
                miette!(
                    "No GPG signing key configured.\n\
                     Provide a key ID with: tdt team add-key --method gpg --key YOUR_KEY_ID\n\
                     Or configure git: git config user.signingkey YOUR_KEY_ID"
                )
            })?
        };

        // Export the public key
        let output = std::process::Command::new("gpg")
            .args(["--armor", "--export", &key_id])
            .output()
            .into_diagnostic()?;

        if !output.status.success() {
            bail!("Failed to export GPG key: {}", String::from_utf8_lossy(&output.stderr));
        }

        let content = String::from_utf8_lossy(&output.stdout).to_string();
        if content.is_empty() {
            bail!("GPG key '{}' not found or has no public key.", key_id);
        }

        Ok((content, key_id))
    }

    fn get_ssh_public_key(&self, git: &Git) -> Result<(String, String)> {
        let key_path = if let Some(ref path) = self.key {
            path.clone()
        } else {
            // Try git config first
            if let Some(configured_key) = git.signing_key() {
                PathBuf::from(configured_key)
            } else {
                // Auto-detect from common locations
                let home = std::env::var("HOME").unwrap_or_default();
                let candidates = [
                    format!("{}/.ssh/id_ed25519.pub", home),
                    format!("{}/.ssh/id_rsa.pub", home),
                    format!("{}/.ssh/id_ecdsa.pub", home),
                ];

                candidates
                    .iter()
                    .find(|p| std::path::Path::new(p).exists())
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        miette!(
                            "No SSH public key found.\n\
                             Provide a key with: tdt team add-key --method ssh --key ~/.ssh/id_ed25519.pub"
                        )
                    })?
            }
        };

        // Ensure it's a public key
        let key_path = if !key_path.to_string_lossy().ends_with(".pub") {
            PathBuf::from(format!("{}.pub", key_path.display()))
        } else {
            key_path
        };

        if !key_path.exists() {
            bail!("SSH public key not found at: {}", key_path.display());
        }

        let content = std::fs::read_to_string(&key_path).into_diagnostic()?;
        let display = key_path.display().to_string();

        Ok((content, display))
    }

    fn confirm(&self) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        print!("Export key to team keyring? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(false);
        }
        Ok(true)
    }
}

impl ImportKeysArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;

        let gpg_dir = project.tdt_dir().join("keys").join("gpg");
        if !gpg_dir.exists() {
            println!("No GPG keys found in .tdt/keys/gpg/");
            println!("Team members can add keys with: tdt team add-key --method gpg");
            return Ok(());
        }

        // Find all .asc files
        let keys: Vec<_> = std::fs::read_dir(&gpg_dir)
            .into_diagnostic()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "asc")
                    .unwrap_or(false)
            })
            .filter(|e| {
                // If --user specified, filter to just that user
                if let Some(ref user) = self.user {
                    e.path()
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string() == *user)
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .collect();

        if keys.is_empty() {
            if let Some(ref user) = self.user {
                println!("No GPG key found for user '{}'", user);
            } else {
                println!("No GPG keys found in .tdt/keys/gpg/");
            }
            return Ok(());
        }

        println!("Import GPG Keys from Team Keyring");
        println!("==================================\n");
        println!("Found {} GPG key(s) to import:\n", keys.len());

        for key in &keys {
            let username = key
                .path()
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            println!("  - {} ({})", username, key.path().display());
        }
        println!();

        if !self.confirm()? {
            return Ok(());
        }

        let mut imported = 0;
        let mut failed = 0;

        for key in keys {
            let path = key.path();
            let username = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let output = std::process::Command::new("gpg")
                .args(["--import", &path.to_string_lossy()])
                .output();

            match output {
                Ok(o) if o.status.success() => {
                    println!("✓ Imported key for {}", username);
                    imported += 1;
                }
                Ok(o) => {
                    // GPG outputs to stderr even on success sometimes
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if stderr.contains("not changed") {
                        println!("  {} (already imported)", username);
                        imported += 1;
                    } else {
                        eprintln!("✗ Failed to import key for {}: {}", username, stderr);
                        failed += 1;
                    }
                }
                Err(e) => {
                    eprintln!("✗ Failed to import key for {}: {}", username, e);
                    failed += 1;
                }
            }
        }

        println!();
        if imported > 0 {
            println!("Imported {} key(s). You can now verify signatures from these users.", imported);
        }
        if failed > 0 {
            println!("{} key(s) failed to import.", failed);
        }

        Ok(())
    }

    fn confirm(&self) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        print!("Import these keys into your GPG keyring? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(false);
        }
        Ok(true)
    }
}

impl SyncKeysArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;

        let ssh_dir = project.tdt_dir().join("keys").join("ssh");
        let allowed_signers_path = project.tdt_dir().join("keys").join("allowed_signers");

        if !ssh_dir.exists() {
            println!("No SSH keys found in .tdt/keys/ssh/");
            println!("Team members can add keys with: tdt team add-key --method ssh");
            return Ok(());
        }

        // Load team roster to get email addresses
        let roster = TeamRoster::load(&project);

        // Find all .pub files
        let keys: Vec<_> = std::fs::read_dir(&ssh_dir)
            .into_diagnostic()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "pub")
                    .unwrap_or(false)
            })
            .collect();

        if keys.is_empty() {
            println!("No SSH keys found in .tdt/keys/ssh/");
            return Ok(());
        }

        println!("Sync SSH Allowed Signers");
        println!("========================\n");
        println!("This will regenerate the allowed_signers file for SSH signature verification.\n");
        println!("Found {} SSH key(s):\n", keys.len());

        // Build allowed_signers content
        let mut lines = vec![
            "# SSH allowed signers for signature verification".to_string(),
            "# Auto-generated by 'tdt team sync-keys'".to_string(),
            "# See: git config gpg.ssh.allowedSignersFile".to_string(),
            "".to_string(),
        ];

        for key in &keys {
            let path = key.path();
            let username = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            // Get email from roster or construct one
            let email = roster
                .as_ref()
                .and_then(|r| r.find_member(&username))
                .map(|m| m.email.clone())
                .unwrap_or_else(|| format!("{}@unknown", username));

            // Read key content
            let key_content = std::fs::read_to_string(&path)
                .into_diagnostic()?
                .trim()
                .to_string();

            println!("  - {} <{}>", username, email);

            lines.push(format!("{} {}", email, key_content));
        }

        println!();
        println!("Output: {}", allowed_signers_path.display());
        println!();

        if !self.confirm()? {
            return Ok(());
        }

        // Write allowed_signers file
        let content = lines.join("\n") + "\n";
        std::fs::write(&allowed_signers_path, content).into_diagnostic()?;

        println!("✓ Generated {}", allowed_signers_path.display());
        println!();
        println!("To enable SSH signature verification, run:");
        println!("  git config gpg.ssh.allowedSignersFile {}", allowed_signers_path.display());
        println!();
        println!("Or for global configuration:");
        println!("  git config --global gpg.ssh.allowedSignersFile {}", allowed_signers_path.display());

        Ok(())
    }

    fn confirm(&self) -> Result<bool> {
        if self.yes {
            return Ok(true);
        }

        print!("Regenerate allowed_signers file? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(false);
        }
        Ok(true)
    }
}
