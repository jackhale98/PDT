//! GitHub/GitLab CLI wrapper for PR/MR operations
//!
//! Shells out to `gh` (GitHub CLI) or `glab` (GitLab CLI) for pull/merge request operations.
//! All user input is properly escaped via std::process::Command args.

use clap::ValueEnum;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

/// Git hosting provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    GitHub,
    GitLab,
    #[default]
    None, // Manual/platform-agnostic mode
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::GitHub => write!(f, "github"),
            Provider::GitLab => write!(f, "gitlab"),
            Provider::None => write!(f, "none"),
        }
    }
}

/// Pull/Merge request state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Closed,
    Merged,
    Draft,
}

impl std::fmt::Display for PrState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrState::Open => write!(f, "open"),
            PrState::Closed => write!(f, "closed"),
            PrState::Merged => write!(f, "merged"),
            PrState::Draft => write!(f, "draft"),
        }
    }
}

/// Information about a pull/merge request
#[derive(Debug, Clone)]
pub struct PrInfo {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub author: String,
    pub branch: String,
    pub state: PrState,
}

/// Errors that can occur during provider operations
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider not configured (workflow.provider is 'none')")]
    NotConfigured,

    #[error("{cli} CLI not found. Install it from {install_url}")]
    CliNotFound { cli: String, install_url: String },

    #[error("{cli} CLI not authenticated. Run '{auth_cmd}' first")]
    NotAuthenticated { cli: String, auth_cmd: String },

    #[error("Command failed: {message}")]
    CommandFailed { message: String },

    #[error("PR not found for branch: {branch}")]
    PrNotFound { branch: String },

    #[error("Failed to parse CLI output: {message}")]
    ParseError { message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Provider client for GitHub/GitLab operations
pub struct ProviderClient {
    provider: Provider,
    repo_root: PathBuf,
    /// If true, don't execute commands, just return what would be run
    dry_run: bool,
    /// If true, print commands to stderr before executing
    verbose: bool,
}

impl ProviderClient {
    /// Create a new provider client
    pub fn new(provider: Provider, repo_root: &Path) -> Self {
        Self {
            provider,
            repo_root: repo_root.to_path_buf(),
            dry_run: false,
            verbose: false,
        }
    }

    /// Set dry-run mode
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set verbose mode
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Get the CLI command name for this provider
    fn cli_name(&self) -> &'static str {
        match self.provider {
            Provider::GitHub => "gh",
            Provider::GitLab => "glab",
            Provider::None => "",
        }
    }

    /// Get the install URL for the CLI
    fn install_url(&self) -> &'static str {
        match self.provider {
            Provider::GitHub => "https://cli.github.com",
            Provider::GitLab => "https://gitlab.com/gitlab-org/cli",
            Provider::None => "",
        }
    }

    /// Get the auth command for the CLI
    fn auth_cmd(&self) -> &'static str {
        match self.provider {
            Provider::GitHub => "gh auth login",
            Provider::GitLab => "glab auth login",
            Provider::None => "",
        }
    }

    /// Check if the provider CLI is available
    pub fn is_available(&self) -> bool {
        if self.provider == Provider::None {
            return false;
        }

        Command::new(self.cli_name())
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if the CLI is authenticated
    pub fn is_authenticated(&self) -> bool {
        if self.provider == Provider::None {
            return false;
        }

        let args = match self.provider {
            Provider::GitHub => vec!["auth", "status"],
            Provider::GitLab => vec!["auth", "status"],
            Provider::None => return false,
        };

        Command::new(self.cli_name())
            .args(&args)
            .current_dir(&self.repo_root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Validate that the provider is configured and available
    fn validate(&self) -> Result<(), ProviderError> {
        if self.provider == Provider::None {
            return Err(ProviderError::NotConfigured);
        }

        if !self.is_available() {
            return Err(ProviderError::CliNotFound {
                cli: self.cli_name().to_string(),
                install_url: self.install_url().to_string(),
            });
        }

        if !self.is_authenticated() {
            return Err(ProviderError::NotAuthenticated {
                cli: self.cli_name().to_string(),
                auth_cmd: self.auth_cmd().to_string(),
            });
        }

        Ok(())
    }

    /// Execute a CLI command
    fn run(&self, args: &[&str]) -> Result<String, ProviderError> {
        let cli = self.cli_name();

        if self.verbose {
            eprintln!("  {} {}", cli, args.join(" "));
        }

        if self.dry_run {
            return Ok(String::new());
        }

        let output = Command::new(cli)
            .args(args)
            .current_dir(&self.repo_root)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(ProviderError::CommandFailed {
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }

    /// Format a command for display (dry-run output)
    pub fn format_command(&self, args: &[&str]) -> String {
        format!("{} {}", self.cli_name(), args.join(" "))
    }

    /// Create a pull/merge request
    pub fn create_pr(
        &self,
        title: &str,
        body: &str,
        base: &str,
        draft: bool,
    ) -> Result<PrInfo, ProviderError> {
        self.create_pr_with_reviewers(title, body, base, draft, &[])
    }

    /// Create a pull/merge request with specific reviewers
    pub fn create_pr_with_reviewers(
        &self,
        title: &str,
        body: &str,
        base: &str,
        draft: bool,
        reviewers: &[String],
    ) -> Result<PrInfo, ProviderError> {
        self.validate()?;

        // Build reviewer string for GitHub/GitLab
        let reviewer_str = reviewers.join(",");

        let args = match self.provider {
            Provider::GitHub => {
                let mut args = vec![
                    "pr", "create", "--title", title, "--body", body, "--base", base,
                ];
                if draft {
                    args.push("--draft");
                }
                args
            }
            Provider::GitLab => {
                let mut args = vec![
                    "mr",
                    "create",
                    "--title",
                    title,
                    "--description",
                    body,
                    "--target-branch",
                    base,
                    "--remove-source-branch",
                    "--yes",
                ];
                if draft {
                    args.push("--draft");
                }
                args
            }
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        // Convert to owned strings so we can add reviewer args
        let mut owned_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        // Add reviewer flag if reviewers specified
        if !reviewers.is_empty() {
            match self.provider {
                Provider::GitHub => {
                    owned_args.push("--reviewer".to_string());
                    owned_args.push(reviewer_str);
                }
                Provider::GitLab => {
                    owned_args.push("--reviewer".to_string());
                    owned_args.push(reviewer_str);
                }
                Provider::None => {}
            }
        }

        // Convert back to &str for run()
        let args_refs: Vec<&str> = owned_args.iter().map(|s| s.as_str()).collect();

        if self.dry_run {
            return Ok(PrInfo {
                number: 0,
                url: format!("(dry-run) {}", self.format_command(&args_refs)),
                title: title.to_string(),
                author: String::new(),
                branch: String::new(),
                state: if draft { PrState::Draft } else { PrState::Open },
            });
        }

        let output = self.run(&args_refs)?;

        // Parse the output to get PR info
        // GitHub outputs just the URL, GitLab outputs more info
        let url = output.lines().last().unwrap_or(&output).to_string();
        let number = self.extract_pr_number(&url).unwrap_or(0);

        Ok(PrInfo {
            number,
            url,
            title: title.to_string(),
            author: String::new(),
            branch: String::new(),
            state: if draft { PrState::Draft } else { PrState::Open },
        })
    }

    /// Add an approving review to a PR
    pub fn approve_pr(&self, pr_number: u64, comment: Option<&str>) -> Result<(), ProviderError> {
        self.validate()?;

        let pr_str = pr_number.to_string();
        let args = match self.provider {
            Provider::GitHub => {
                let mut args = vec!["pr", "review", &pr_str, "--approve"];
                if let Some(c) = comment {
                    args.push("--body");
                    args.push(c);
                }
                args
            }
            Provider::GitLab => {
                vec!["mr", "approve", &pr_str]
                // GitLab doesn't support comment with approve in one command
            }
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        self.run(&args)?;
        Ok(())
    }

    /// Merge a PR
    pub fn merge_pr(&self, pr_number: u64, delete_branch: bool) -> Result<(), ProviderError> {
        self.validate()?;

        let pr_str = pr_number.to_string();
        let args = match self.provider {
            Provider::GitHub => {
                let mut args = vec!["pr", "merge", &pr_str, "--merge"];
                if delete_branch {
                    args.push("--delete-branch");
                }
                args
            }
            Provider::GitLab => {
                let mut args = vec!["mr", "merge", &pr_str];
                if delete_branch {
                    args.push("--remove-source-branch");
                }
                args
            }
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        self.run(&args)?;
        Ok(())
    }

    /// Close a PR without merging
    pub fn close_pr(&self, pr_number: u64, comment: Option<&str>) -> Result<(), ProviderError> {
        self.validate()?;

        // Add comment first if provided
        if let Some(c) = comment {
            let pr_str = pr_number.to_string();
            let comment_args = match self.provider {
                Provider::GitHub => vec!["pr", "comment", &pr_str, "--body", c],
                Provider::GitLab => vec!["mr", "note", &pr_str, "--message", c],
                Provider::None => return Err(ProviderError::NotConfigured),
            };
            let _ = self.run(&comment_args); // Ignore comment errors
        }

        let pr_str = pr_number.to_string();
        let args = match self.provider {
            Provider::GitHub => vec!["pr", "close", &pr_str],
            Provider::GitLab => vec!["mr", "close", &pr_str],
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        self.run(&args)?;
        Ok(())
    }

    /// List PRs awaiting review by current user
    pub fn pending_reviews(&self) -> Result<Vec<PrInfo>, ProviderError> {
        self.validate()?;

        let args = match self.provider {
            Provider::GitHub => vec![
                "pr",
                "list",
                "--search",
                "review-requested:@me",
                "--json",
                "number,url,title,author,headRefName,state",
            ],
            Provider::GitLab => vec!["mr", "list", "--reviewer=@me", "--state=opened"],
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        let output = self.run(&args)?;

        match self.provider {
            Provider::GitHub => self.parse_github_pr_list(&output),
            Provider::GitLab => self.parse_gitlab_mr_list(&output),
            Provider::None => Ok(Vec::new()),
        }
    }

    /// List all open PRs targeting a specific branch (e.g., main)
    pub fn list_prs_targeting(&self, target_branch: &str) -> Result<Vec<PrInfo>, ProviderError> {
        self.validate()?;

        let args = match self.provider {
            Provider::GitHub => vec![
                "pr",
                "list",
                "--base",
                target_branch,
                "--state",
                "open",
                "--json",
                "number,url,title,author,headRefName,state",
            ],
            Provider::GitLab => vec![
                "mr",
                "list",
                "--target-branch",
                target_branch,
                "--state=opened",
            ],
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        let output = self.run(&args)?;

        match self.provider {
            Provider::GitHub => self.parse_github_pr_list(&output),
            Provider::GitLab => self.parse_gitlab_mr_list(&output),
            Provider::None => Ok(Vec::new()),
        }
    }

    /// List all open PRs in the repository
    pub fn list_open_prs(&self) -> Result<Vec<PrInfo>, ProviderError> {
        self.validate()?;

        let args = match self.provider {
            Provider::GitHub => vec![
                "pr",
                "list",
                "--state",
                "open",
                "--json",
                "number,url,title,author,headRefName,state",
            ],
            Provider::GitLab => vec!["mr", "list", "--state=opened"],
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        let output = self.run(&args)?;

        match self.provider {
            Provider::GitHub => self.parse_github_pr_list(&output),
            Provider::GitLab => self.parse_gitlab_mr_list(&output),
            Provider::None => Ok(Vec::new()),
        }
    }

    /// Get PR info by branch name
    pub fn get_pr_for_branch(&self, branch: &str) -> Result<Option<PrInfo>, ProviderError> {
        self.validate()?;

        let args = match self.provider {
            Provider::GitHub => vec![
                "pr",
                "list",
                "--head",
                branch,
                "--json",
                "number,url,title,author,headRefName,state",
            ],
            Provider::GitLab => vec!["mr", "list", "--source-branch", branch, "--state=opened"],
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        let output = self.run(&args)?;

        let prs = match self.provider {
            Provider::GitHub => self.parse_github_pr_list(&output)?,
            Provider::GitLab => self.parse_gitlab_mr_list(&output)?,
            Provider::None => Vec::new(),
        };

        Ok(prs.into_iter().next())
    }

    /// Get PR info by PR number
    pub fn get_pr(&self, pr_number: u64) -> Result<PrInfo, ProviderError> {
        self.validate()?;

        let pr_str = pr_number.to_string();
        let args = match self.provider {
            Provider::GitHub => vec![
                "pr",
                "view",
                &pr_str,
                "--json",
                "number,url,title,author,headRefName,state",
            ],
            Provider::GitLab => vec!["mr", "view", &pr_str],
            Provider::None => return Err(ProviderError::NotConfigured),
        };

        let output = self.run(&args)?;

        match self.provider {
            Provider::GitHub => {
                let prs = self.parse_github_pr_list(&format!("[{}]", output))?;
                prs.into_iter()
                    .next()
                    .ok_or(ProviderError::PrNotFound { branch: pr_str })
            }
            Provider::GitLab => {
                // Parse single MR view output
                self.parse_gitlab_mr_view(&output, pr_number)
            }
            Provider::None => Err(ProviderError::NotConfigured),
        }
    }

    /// Extract PR number from URL
    fn extract_pr_number(&self, url: &str) -> Option<u64> {
        // URLs like https://github.com/owner/repo/pull/123
        // or https://gitlab.com/owner/repo/-/merge_requests/123
        url.rsplit('/').next().and_then(|s| s.parse().ok())
    }

    /// Parse GitHub PR list JSON output
    fn parse_github_pr_list(&self, json: &str) -> Result<Vec<PrInfo>, ProviderError> {
        #[derive(Deserialize)]
        struct GhPr {
            number: u64,
            url: String,
            title: String,
            author: GhAuthor,
            #[serde(rename = "headRefName")]
            head_ref_name: String,
            state: String,
        }

        #[derive(Deserialize)]
        struct GhAuthor {
            login: String,
        }

        let prs: Vec<GhPr> = serde_json::from_str(json).map_err(|e| ProviderError::ParseError {
            message: e.to_string(),
        })?;

        Ok(prs
            .into_iter()
            .map(|pr| PrInfo {
                number: pr.number,
                url: pr.url,
                title: pr.title,
                author: pr.author.login,
                branch: pr.head_ref_name,
                state: match pr.state.to_lowercase().as_str() {
                    "open" => PrState::Open,
                    "closed" => PrState::Closed,
                    "merged" => PrState::Merged,
                    _ => PrState::Open,
                },
            })
            .collect())
    }

    /// Parse GitLab MR list output (not JSON by default)
    fn parse_gitlab_mr_list(&self, output: &str) -> Result<Vec<PrInfo>, ProviderError> {
        // glab mr list output format:
        // !123  Title  (branch -> target)  author
        let mut prs = Vec::new();

        for line in output.lines() {
            if let Some(pr) = self.parse_gitlab_mr_line(line) {
                prs.push(pr);
            }
        }

        Ok(prs)
    }

    /// Parse a single GitLab MR line
    fn parse_gitlab_mr_line(&self, line: &str) -> Option<PrInfo> {
        // Format: !123  Title  (branch -> target)  author
        let line = line.trim();
        if !line.starts_with('!') {
            return None;
        }

        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            return None;
        }

        let number: u64 = parts[0].trim_start_matches('!').parse().ok()?;
        let rest = parts[1].trim();

        // Extract title (everything before the parentheses)
        let title_end = rest.find('(')?;
        let title = rest[..title_end].trim().to_string();

        Some(PrInfo {
            number,
            url: String::new(), // Would need to construct from repo URL
            title,
            author: String::new(),
            branch: String::new(),
            state: PrState::Open,
        })
    }

    /// Parse GitLab MR view output
    fn parse_gitlab_mr_view(&self, _output: &str, number: u64) -> Result<PrInfo, ProviderError> {
        // Simplified parsing - would need more work for full implementation
        Ok(PrInfo {
            number,
            url: String::new(),
            title: String::new(),
            author: String::new(),
            branch: String::new(),
            state: PrState::Open,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_display() {
        assert_eq!(Provider::GitHub.to_string(), "github");
        assert_eq!(Provider::GitLab.to_string(), "gitlab");
        assert_eq!(Provider::None.to_string(), "none");
    }

    #[test]
    fn test_extract_pr_number() {
        let client = ProviderClient::new(Provider::GitHub, Path::new("."));

        assert_eq!(
            client.extract_pr_number("https://github.com/owner/repo/pull/123"),
            Some(123)
        );
        assert_eq!(
            client.extract_pr_number("https://gitlab.com/owner/repo/-/merge_requests/456"),
            Some(456)
        );
        assert_eq!(client.extract_pr_number("not-a-url"), None);
    }

    #[test]
    fn test_format_command() {
        let client = ProviderClient::new(Provider::GitHub, Path::new("."));
        assert_eq!(
            client.format_command(&["pr", "create", "--title", "Test"]),
            "gh pr create --title Test"
        );

        let client = ProviderClient::new(Provider::GitLab, Path::new("."));
        assert_eq!(
            client.format_command(&["mr", "create", "--title", "Test"]),
            "glab mr create --title Test"
        );
    }

    #[test]
    fn test_parse_github_pr_list() {
        let client = ProviderClient::new(Provider::GitHub, Path::new("."));
        let json = r#"[
            {
                "number": 42,
                "url": "https://github.com/owner/repo/pull/42",
                "title": "Test PR",
                "author": {"login": "testuser"},
                "headRefName": "feature-branch",
                "state": "OPEN"
            }
        ]"#;

        let prs = client.parse_github_pr_list(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].title, "Test PR");
        assert_eq!(prs[0].author, "testuser");
        assert_eq!(prs[0].branch, "feature-branch");
    }

    #[test]
    fn test_dry_run_mode() {
        let client = ProviderClient::new(Provider::GitHub, Path::new(".")).with_dry_run(true);
        assert!(client.dry_run);
    }
}
