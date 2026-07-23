//! Shell-based git operations (fallback for network ops, GPG, stash, merge, checkout)
//!
//! These operations require the `git` CLI binary. On mobile (iOS/Android) where
//! no binary is available, they return `GitError::ShellNotAvailable`.

use std::process::Command;

use super::{parse_log_output, CommitLogEntry, Git, GitError, GitOutput};

/// Full commit metadata returned by `Git::commit_metadata`.
///
/// Tuple fields: `(full_hash, short_hash, full_message, author, author_email, date, is_signed)`.
pub type CommitMetadata = (String, String, String, String, Option<String>, String, bool);

impl Git {
    /// Execute a git CLI command and return the output
    pub(super) fn run_shell(&self, args: &[&str]) -> Result<GitOutput, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    GitError::ShellNotAvailable
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

    /// Execute a git CLI command, returning error on failure
    pub fn run_checked(&self, args: &[&str]) -> Result<GitOutput, GitError> {
        let output = self.run_shell(args)?;
        if output.success {
            Ok(output)
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr.clone(),
            })
        }
    }

    // ========================================================================
    // Checkout operations (require working tree manipulation)
    // ========================================================================

    /// Checkout an existing branch
    pub fn checkout_branch(&self, name: &str) -> Result<(), GitError> {
        if !self.branch_exists(name) {
            return Err(GitError::BranchNotFound {
                branch: name.to_string(),
            });
        }
        let output = self.run_shell(&["checkout", name])?;
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
        let output = self.run_shell(&["checkout", "-b", name])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Fetch and checkout a remote branch, creating a local tracking branch
    pub fn fetch_and_checkout_branch(&self, branch: &str) -> Result<(), GitError> {
        if self.branch_exists(branch) {
            return self.checkout_branch(branch);
        }

        self.fetch()?;

        let remote = self
            .default_remote()
            .unwrap_or_else(|_| "origin".to_string());
        if !self.remote_branch_exists(&remote, branch) {
            return Err(GitError::BranchNotFound {
                branch: format!("{}/{}", remote, branch),
            });
        }

        let output =
            self.run_shell(&["checkout", "-b", branch, &format!("{}/{}", remote, branch)])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    // ========================================================================
    // Network operations (push / pull / fetch)
    // ========================================================================

    /// Push branch to remote
    pub fn push(&self, branch: &str, set_upstream: bool) -> Result<(), GitError> {
        let args = if set_upstream {
            vec!["push", "-u", "origin", branch]
        } else {
            vec!["push", "origin", branch]
        };
        let output = self.run_shell(&args)?;
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
        let output = self.run_shell(&["pull"])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Pull with rebase
    pub fn pull_rebase(&self) -> Result<(), GitError> {
        let output = self.run_shell(&["pull", "--rebase"])?;
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
        let output = self.run_shell(&["fetch"])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    // ========================================================================
    // Stash operations
    // ========================================================================

    /// Stash uncommitted changes
    pub fn stash(&self, message: Option<&str>) -> Result<(), GitError> {
        let args = if let Some(msg) = message {
            vec!["stash", "push", "-m", msg]
        } else {
            vec!["stash", "push"]
        };
        let output = self.run_shell(&args)?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Pop the most recent stash
    pub fn stash_pop(&self) -> Result<(), GitError> {
        let output = self.run_shell(&["stash", "pop"])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    // ========================================================================
    // Merge operations
    // ========================================================================

    /// Merge a branch into the current branch
    pub fn merge_branch(&self, branch: &str, message: &str) -> Result<String, GitError> {
        if !self.branch_exists(branch) {
            return Err(GitError::BranchNotFound {
                branch: branch.to_string(),
            });
        }
        let output = self.run_shell(&["merge", "--no-ff", "-m", message, branch])?;
        if output.success {
            self.head_sha()
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Merge a branch with GPG signing
    pub fn merge_branch_signed(&self, branch: &str, message: &str) -> Result<String, GitError> {
        if !self.branch_exists(branch) {
            return Err(GitError::BranchNotFound {
                branch: branch.to_string(),
            });
        }
        let output = self.run_shell(&["merge", "--no-ff", "-S", "-m", message, branch])?;
        if output.success {
            self.head_sha()
        } else {
            if output.stderr.contains("gpg") || output.stderr.contains("signing") {
                return Err(GitError::CommandFailed {
                    message: format!(
                        "Failed to sign merge commit. Configure GPG signing with:\n\
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

    // ========================================================================
    // GPG signing operations
    // ========================================================================

    /// Commit staged changes with GPG signing
    pub fn commit_signed(&self, message: &str) -> Result<String, GitError> {
        let output = self.run_shell(&["commit", "-S", "-m", message])?;
        if output.success {
            self.head_sha()
        } else {
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
    ///
    /// Returns `Ok(Some(signer))` for a good signature, `Ok(None)` for an
    /// unsigned commit, and `Err` for a bad/unverifiable signature.
    pub fn verify_commit_signature(&self, commit: &str) -> Result<Option<String>, GitError> {
        let output = self.run_shell(&["verify-commit", "--raw", commit])?;

        if output.success {
            // With --raw, GPG emits machine-readable status lines:
            //   [GNUPG:] GOODSIG <keyid> <signer name>
            for line in output.stderr.lines() {
                if let Some(rest) = line.strip_prefix("[GNUPG:] GOODSIG ") {
                    // Everything after the key ID is the signer's name.
                    if let Some((_keyid, signer)) = rest.split_once(' ') {
                        return Ok(Some(signer.to_string()));
                    }
                    return Ok(Some(rest.to_string()));
                }
            }
            Ok(Some("verified".to_string()))
        } else if output.stderr.trim().is_empty() {
            // `git verify-commit --raw` exits non-zero with EMPTY stderr for
            // commits that simply aren't signed.
            Ok(None)
        } else if output.stderr.contains("BADSIG") || output.stderr.contains("ERRSIG") {
            Err(GitError::CommandFailed {
                message: format!("Invalid signature: {}", output.stderr),
            })
        } else if output.stderr.contains("no signature found")
            || output.stderr.contains("unsigned commit")
        {
            Ok(None)
        } else {
            Err(GitError::CommandFailed {
                message: format!("Signature verification failed: {}", output.stderr),
            })
        }
    }

    /// Create a GPG-signed tag
    pub fn create_signed_tag(&self, name: &str, message: &str) -> Result<(), GitError> {
        let output = self.run_shell(&["tag", "-s", name, "-m", message])?;
        if output.success {
            Ok(())
        } else {
            if output.stderr.contains("gpg") || output.stderr.contains("signing") {
                return Err(GitError::CommandFailed {
                    message: format!(
                        "Failed to sign tag. Configure GPG signing with:\n\
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

    /// Create an annotated tag (optionally signed)
    pub fn create_tag_with_options(
        &self,
        name: &str,
        message: &str,
        sign: bool,
    ) -> Result<(), GitError> {
        if sign {
            self.create_signed_tag(name, message)
        } else {
            self.create_tag(name, Some(message))
        }
    }

    // ========================================================================
    // Branch deletion
    // ========================================================================

    /// Delete a local branch
    pub fn delete_branch(&self, name: &str) -> Result<(), GitError> {
        if !self.branch_exists(name) {
            return Err(GitError::BranchNotFound {
                branch: name.to_string(),
            });
        }
        if self.current_branch().ok().as_deref() == Some(name) {
            return Err(GitError::CommandFailed {
                message: format!("Cannot delete the currently checked out branch: {}", name),
            });
        }
        let output = self.run_shell(&["branch", "-d", name])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Force delete a local branch
    pub fn delete_branch_force(&self, name: &str) -> Result<(), GitError> {
        if !self.branch_exists(name) {
            return Err(GitError::BranchNotFound {
                branch: name.to_string(),
            });
        }
        if self.current_branch().ok().as_deref() == Some(name) {
            return Err(GitError::CommandFailed {
                message: format!("Cannot delete the currently checked out branch: {}", name),
            });
        }
        let output = self.run_shell(&["branch", "-D", name])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    // ========================================================================
    // Log & diff operations (shell fallback for complex queries)
    // ========================================================================

    /// Get commit log for a specific file (with --follow for renames)
    pub fn file_log(&self, file_path: &str, limit: u32) -> Result<Vec<CommitLogEntry>, GitError> {
        let output = self.run_shell(&[
            "log",
            "--format=%H%x00%h%x00%s%x00%an%x00%ae%x00%aI%x00%G?",
            "--follow",
            &format!("-{}", limit),
            "--",
            file_path,
        ])?;
        if output.success {
            Ok(parse_log_output(&output.stdout))
        } else {
            if output.stderr.contains("does not have any commits yet") {
                return Ok(Vec::new());
            }
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Get the diff of a specific commit (optionally scoped to a file)
    pub fn show_commit_diff(
        &self,
        commit_hash: &str,
        file_path: Option<&str>,
    ) -> Result<String, GitError> {
        let mut args = vec!["show", "--format=", commit_hash];
        if let Some(path) = file_path {
            args.push("--");
            args.push(path);
        }
        let output = self.run_shell(&args)?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }

    /// Get changed files with status for a specific commit
    /// Returns Vec<(status_char, file_path)>
    pub fn commit_changed_files(
        &self,
        commit_hash: &str,
    ) -> Result<Vec<(String, String)>, GitError> {
        let output = self.run_shell(&[
            "diff-tree",
            "--no-commit-id",
            "-r",
            "--name-status",
            commit_hash,
        ])?;
        if !output.success {
            return Err(GitError::CommandFailed {
                message: output.stderr,
            });
        }

        Ok(output
            .stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                let status = parts.first().unwrap_or(&"M").to_string();
                let path = parts.get(1).unwrap_or(&"").to_string();
                (status, path)
            })
            .collect())
    }

    /// Get insertions/deletions stats for a commit
    pub fn commit_stats(&self, commit_hash: &str) -> Result<(u32, u32), GitError> {
        let output = self.run_shell(&[
            "diff-tree",
            "--no-commit-id",
            "--shortstat",
            "-r",
            commit_hash,
        ])?;
        if !output.success {
            // Don't mask failures (e.g. a bad hash) as "0 insertions, 0 deletions".
            return Err(GitError::CommandFailed {
                message: format!(
                    "diff-tree failed for {}: {}",
                    commit_hash,
                    output.stderr.trim()
                ),
            });
        }

        let mut insertions: u32 = 0;
        let mut deletions: u32 = 0;

        for part in output.stdout.split(',') {
            let part = part.trim();
            if part.contains("insertion") {
                if let Some(num) = part.split_whitespace().next() {
                    insertions = num.parse().unwrap_or(0);
                }
            } else if part.contains("deletion") {
                if let Some(num) = part.split_whitespace().next() {
                    deletions = num.parse().unwrap_or(0);
                }
            }
        }

        Ok((insertions, deletions))
    }

    /// Get full commit metadata (for commit details view)
    /// Returns (full_hash, short_hash, full_message, author, author_email, date, is_signed)
    pub fn commit_metadata(&self, commit_hash: &str) -> Result<CommitMetadata, GitError> {
        let output = self.run_shell(&[
            "show",
            "--format=%H%x00%h%x00%B%n---END_MSG---%x00%an%x00%ae%x00%aI%x00%G?",
            "--no-patch",
            commit_hash,
        ])?;
        if !output.success {
            return Err(GitError::CommandFailed {
                message: output.stderr,
            });
        }

        let stdout = output.stdout.trim().to_string();

        let (msg_part, meta_part) =
            stdout
                .split_once("---END_MSG---")
                .ok_or_else(|| GitError::CommandFailed {
                    message: "Failed to parse commit output".to_string(),
                })?;

        // Fields are NUL-separated (%x00) so `|` in messages or author names
        // can't shift the columns.
        let meta_parts: Vec<&str> = meta_part.split('\0').collect();
        let msg_lines: Vec<&str> = msg_part.split('\0').collect();

        let full_hash = msg_lines.first().unwrap_or(&"").to_string();
        let short_hash = msg_lines.get(1).unwrap_or(&"").to_string();
        let full_message = msg_lines
            .get(2..)
            .map(|s| s.join("\0"))
            .unwrap_or_default()
            .trim()
            .to_string();

        let author = meta_parts.get(1).unwrap_or(&"").to_string();
        let author_email = meta_parts.get(2).map(|s| s.to_string());
        let date = meta_parts.get(3).unwrap_or(&"").to_string();
        let is_signed = meta_parts.get(4).is_some_and(|s| *s == "G" || *s == "U");

        Ok((
            full_hash,
            short_hash,
            full_message,
            author,
            author_email,
            date,
            is_signed,
        ))
    }

    /// Get diff for a file (working tree vs HEAD, then staged vs HEAD)
    pub fn diff_file(&self, path: &str) -> Result<String, GitError> {
        // Try diff HEAD first (both staged and unstaged)
        let output = self.run_shell(&["diff", "HEAD", "--", path])?;
        if output.success && !output.stdout.is_empty() {
            return Ok(output.stdout);
        }

        // Try staged diff
        let output = self.run_shell(&["diff", "--cached", "--", path])?;
        if output.success && !output.stdout.is_empty() {
            return Ok(output.stdout);
        }

        Ok(String::new())
    }

    /// Restore a file from HEAD (discard working tree changes)
    pub fn restore_file(&self, path: &str) -> Result<(), GitError> {
        // `--` prevents a filename that looks like an option from being
        // misinterpreted by git.
        let output = self.run_shell(&["restore", "--", path])?;
        if output.success {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                message: output.stderr,
            })
        }
    }
}
