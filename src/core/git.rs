//! Git command abstraction layer
//!
//! Provides a safe interface for git operations without exposing shell commands directly.
//! All user input is properly escaped via std::process::Command args.

use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

/// Git operations abstraction
pub struct Git {
    repo_root: PathBuf,
}

/// Result of a git command execution
#[derive(Debug)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

/// Errors that can occur during git operations
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Not a git repository")]
    NotARepo,

    #[error("Git command failed: {message}")]
    CommandFailed { message: String },

    #[error("On protected branch: {branch}")]
    ProtectedBranch { branch: String },

    #[error("Uncommitted changes in working directory")]
    UncommittedChanges,

    #[error("Branch already exists: {branch}")]
    BranchExists { branch: String },

    #[error("Branch not found: {branch}")]
    BranchNotFound { branch: String },

    #[error("Git not installed or not in PATH")]
    GitNotFound,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl Git {
    /// Create a new Git instance for the repository at the given path
    pub fn new(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
        }
    }

    /// Execute a git command and return the output
    fn run(&self, args: &[&str]) -> Result<GitOutput, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    GitError::GitNotFound
                } else {
                    GitError::IoError(e)
                }
            })?;

        Ok(GitOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            code: output.status.code(),
        })
    }

    /// Check if we're in a git repository
    pub fn is_repo(&self) -> bool {
        self.run(&["rev-parse", "--git-dir"])
            .map(|o| o.success)
            .unwrap_or(false)
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, GitError> {
        let output = self.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Check if on main or master branch
    pub fn is_main_branch(&self) -> bool {
        self.current_branch()
            .map(|b| b == "main" || b == "master")
            .unwrap_or(false)
    }

    /// Check if working directory is clean (no uncommitted changes)
    pub fn is_clean(&self) -> bool {
        self.run(&["status", "--porcelain"])
            .map(|o| o.success && o.stdout.is_empty())
            .unwrap_or(false)
    }

    /// Get list of uncommitted changes
    pub fn uncommitted_files(&self) -> Result<Vec<String>, GitError> {
        let output = self.run(&["status", "--porcelain"])?;
        if output.success {
            Ok(output
                .stdout
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Get git user.name
    pub fn user_name(&self) -> Result<String, GitError> {
        let output = self.run(&["config", "user.name"])?;
        if output.success && !output.stdout.is_empty() {
            Ok(output.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: "git user.name not configured".to_string(),
            })
        }
    }

    /// Get git user.email
    pub fn user_email(&self) -> Result<String, GitError> {
        let output = self.run(&["config", "user.email"])?;
        if output.success && !output.stdout.is_empty() {
            Ok(output.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: "git user.email not configured".to_string(),
            })
        }
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, name: &str) -> bool {
        self.run(&["rev-parse", "--verify", &format!("refs/heads/{}", name)])
            .map(|o| o.success)
            .unwrap_or(false)
    }

    /// Check if a remote branch exists
    pub fn remote_branch_exists(&self, remote: &str, branch: &str) -> bool {
        self.run(&[
            "rev-parse",
            "--verify",
            &format!("refs/remotes/{}/{}", remote, branch),
        ])
        .map(|o| o.success)
        .unwrap_or(false)
    }

    /// Create a new branch
    pub fn create_branch(&self, name: &str) -> Result<(), GitError> {
        if self.branch_exists(name) {
            return Err(GitError::BranchExists {
                branch: name.to_string(),
            });
        }

        let output = self.run(&["branch", name])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Create and checkout a new branch
    pub fn create_and_checkout_branch(&self, name: &str) -> Result<(), GitError> {
        if self.branch_exists(name) {
            return Err(GitError::BranchExists {
                branch: name.to_string(),
            });
        }

        let output = self.run(&["checkout", "-b", name])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Checkout an existing branch
    pub fn checkout_branch(&self, name: &str) -> Result<(), GitError> {
        if !self.branch_exists(name) {
            return Err(GitError::BranchNotFound {
                branch: name.to_string(),
            });
        }

        let output = self.run(&["checkout", name])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Stage a file for commit
    pub fn stage_file(&self, path: &Path) -> Result<(), GitError> {
        let path_str = path.to_string_lossy();
        let output = self.run(&["add", &path_str])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Stage multiple files for commit
    pub fn stage_files(&self, paths: &[&Path]) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }

        let mut args = vec!["add"];
        let path_strings: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
        for path in &path_strings {
            args.push(path);
        }

        let output = self.run(&args)?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Commit staged changes
    pub fn commit(&self, message: &str) -> Result<String, GitError> {
        let output = self.run(&["commit", "-m", message])?;
        if output.success {
            // Get the commit hash
            let hash_output = self.run(&["rev-parse", "HEAD"])?;
            Ok(hash_output.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Push branch to remote
    pub fn push(&self, branch: &str, set_upstream: bool) -> Result<(), GitError> {
        let args = if set_upstream {
            vec!["push", "-u", "origin", branch]
        } else {
            vec!["push", "origin", branch]
        };

        let output = self.run(&args)?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Pull latest changes from remote
    pub fn pull(&self) -> Result<(), GitError> {
        let output = self.run(&["pull"])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Fetch from remote
    pub fn fetch(&self) -> Result<(), GitError> {
        let output = self.run(&["fetch"])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Get the default remote name (usually "origin")
    pub fn default_remote(&self) -> Result<String, GitError> {
        let output = self.run(&["remote"])?;
        if output.success && !output.stdout.is_empty() {
            // Return first remote (usually "origin")
            Ok(output
                .stdout
                .lines()
                .next()
                .unwrap_or("origin")
                .to_string())
        } else {
            Ok("origin".to_string())
        }
    }

    /// Get the remote URL
    pub fn remote_url(&self, remote: &str) -> Result<String, GitError> {
        let output = self.run(&["remote", "get-url", remote])?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Get the base branch (main or master)
    pub fn base_branch(&self) -> String {
        // Check if main exists
        if self.branch_exists("main") {
            return "main".to_string();
        }
        // Fall back to master
        if self.branch_exists("master") {
            return "master".to_string();
        }
        // Default to main
        "main".to_string()
    }

    /// Format a command for display (dry-run output)
    pub fn format_command(args: &[&str]) -> String {
        format!("git {}", args.join(" "))
    }

    /// Commit staged changes with GPG signing
    pub fn commit_signed(&self, message: &str) -> Result<String, GitError> {
        let output = self.run(&["commit", "-S", "-m", message])?;
        if output.success {
            // Get the commit hash
            let hash_output = self.run(&["rev-parse", "HEAD"])?;
            Ok(hash_output.stdout)
        } else {
            // If signing fails, provide a helpful message
            if output.stderr.contains("gpg") || output.stderr.contains("signing") {
                return Err(GitError::CommandFailed {
                    message: format!(
                        "Failed to sign commit. Configure GPG signing with:\n\
                         git config --global user.signingkey <YOUR_KEY_ID>\n\
                         Original error: {}",
                        output.stderr
                    ),
                });
            }
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Verify the signature of a commit
    /// Returns Ok(Some(signer)) if valid, Ok(None) if no signature, Err if invalid
    pub fn verify_commit_signature(&self, commit: &str) -> Result<Option<String>, GitError> {
        let output = self.run(&["verify-commit", "--raw", commit])?;

        // Check status: "git verify-commit" returns 0 for valid, 1 for invalid/missing
        if output.success {
            // Parse the signer from gpg output (typically in stderr)
            // Look for "Good signature from" line
            for line in output.stderr.lines() {
                if line.contains("Good signature from") {
                    // Extract signer info
                    if let Some(start) = line.find('"') {
                        if let Some(end) = line.rfind('"') {
                            return Ok(Some(line[start + 1..end].to_string()));
                        }
                    }
                    return Ok(Some(line.to_string()));
                }
            }
            // Signature verified but couldn't parse signer
            Ok(Some("verified".to_string()))
        } else {
            // Check if it's just missing (no signature) or actually invalid
            if output.stderr.contains("no signature found")
                || output.stderr.contains("unsigned commit")
            {
                Ok(None)
            } else {
                // Signature exists but is invalid
                Err(GitError::CommandFailed {
                    message: format!("Invalid signature: {}", output.stderr),
                })
            }
        }
    }

    /// Check if GPG signing is configured
    pub fn signing_configured(&self) -> bool {
        self.run(&["config", "user.signingkey"])
            .map(|o| o.success && !o.stdout.is_empty())
            .unwrap_or(false)
    }

    /// Get the GPG signing key ID
    pub fn signing_key(&self) -> Option<String> {
        self.run(&["config", "user.signingkey"])
            .ok()
            .filter(|o| o.success && !o.stdout.is_empty())
            .map(|o| o.stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn init_test_repo() -> (tempfile::TempDir, Git) {
        let tmp = tempdir().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        let git = Git::new(tmp.path());
        (tmp, git)
    }

    #[test]
    fn test_is_repo() {
        let (tmp, git) = init_test_repo();
        assert!(git.is_repo());
        drop(tmp);
    }

    #[test]
    fn test_not_a_repo() {
        let tmp = tempdir().unwrap();
        let git = Git::new(tmp.path());
        assert!(!git.is_repo());
    }

    #[test]
    fn test_current_branch() {
        let (tmp, git) = init_test_repo();
        // Need at least one commit to have a HEAD
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();
        // New repos might be on main or master depending on git config
        let branch = git.current_branch().unwrap();
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_is_clean_empty_repo() {
        let (_tmp, git) = init_test_repo();
        assert!(git.is_clean());
    }

    #[test]
    fn test_is_clean_with_changes() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        assert!(!git.is_clean());
    }

    #[test]
    fn test_uncommitted_files() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        let files = git.uncommitted_files().unwrap();
        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.contains("test.txt")));
    }

    #[test]
    fn test_user_name() {
        let (_tmp, git) = init_test_repo();
        let name = git.user_name().unwrap();
        assert_eq!(name, "Test User");
    }

    #[test]
    fn test_branch_exists() {
        let (tmp, git) = init_test_repo();
        // Need at least one commit to have a HEAD
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();
        let current = git.current_branch().unwrap();
        assert!(git.branch_exists(&current));
        assert!(!git.branch_exists("nonexistent-branch"));
    }

    #[test]
    fn test_create_branch() {
        let (tmp, git) = init_test_repo();
        // Need at least one commit to create branches
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();

        git.create_branch("feature-test").unwrap();
        assert!(git.branch_exists("feature-test"));
    }

    #[test]
    fn test_create_branch_already_exists() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();

        git.create_branch("feature-test").unwrap();
        let result = git.create_branch("feature-test");
        assert!(matches!(result, Err(GitError::BranchExists { .. })));
    }

    #[test]
    fn test_stage_and_commit() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();

        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        let hash = git.commit("Test commit").unwrap();
        assert!(!hash.is_empty());
        assert!(git.is_clean());
    }

    #[test]
    fn test_format_command() {
        let formatted = Git::format_command(&["checkout", "-b", "feature/test"]);
        assert_eq!(formatted, "git checkout -b feature/test");
    }
}
