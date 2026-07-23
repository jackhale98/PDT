//! Shell-based implementations of local git operations (desktop/CLI builds)
//!
//! This is the desktop twin of the gix-backed `repo.rs`/`index.rs`/`commit.rs`
//! modules, which are only compiled for mobile targets (iOS/Android have no
//! `git` binary). Desktop builds use these shell implementations exclusively,
//! keeping `gix` out of the dependency graph entirely.
//!
//! The public API is identical between the two implementations — callers
//! never know which one they got.

use std::path::Path;

use super::{parse_log_output, BranchInfoEntry, CommitLogEntry, Git, GitError, TagInfoEntry};

impl Git {
    // ========================================================================
    // Repository queries
    // ========================================================================

    /// Check if we're in a git repository (the repo root itself, not a parent)
    pub fn is_repo(&self) -> bool {
        match self.run_shell(&["rev-parse", "--show-toplevel"]) {
            Ok(out) if out.success => {
                // rev-parse walks up parents; require the toplevel to be our root
                let toplevel = Path::new(&out.stdout);
                match (toplevel.canonicalize(), self.repo_root.canonicalize()) {
                    (Ok(a), Ok(b)) => a == b,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, GitError> {
        let out = self.run_checked(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if out.stdout == "HEAD" {
            // Detached HEAD — return the short hash
            let sha = self.head_sha()?;
            Ok(sha[..7].to_string())
        } else {
            Ok(out.stdout)
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
        self.uncommitted_files()
            .map(|f| f.is_empty())
            .unwrap_or(false)
    }

    /// Get list of uncommitted changes (paths only, status --porcelain style)
    pub fn uncommitted_files(&self) -> Result<Vec<String>, GitError> {
        let out = self.run_checked(&["status", "--porcelain"])?;
        Ok(out
            .stdout
            .lines()
            .filter(|l| l.len() > 3)
            .map(|l| {
                let path = &l[3..];
                // Renames are shown as "old -> new"; report the new location
                match path.split_once(" -> ") {
                    Some((_, new)) => new.to_string(),
                    None => path.to_string(),
                }
            })
            .collect())
    }

    /// Get git user.name from config
    pub fn user_name(&self) -> Result<String, GitError> {
        let out = self.run_shell(&["config", "user.name"])?;
        if out.success && !out.stdout.is_empty() {
            Ok(out.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: "git user.name not configured".to_string(),
            })
        }
    }

    /// Get git user.email from config
    pub fn user_email(&self) -> Result<String, GitError> {
        let out = self.run_shell(&["config", "user.email"])?;
        if out.success && !out.stdout.is_empty() {
            Ok(out.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: "git user.email not configured".to_string(),
            })
        }
    }

    /// Get the GPG signing key ID
    pub fn signing_key(&self) -> Option<String> {
        self.run_shell(&["config", "user.signingkey"])
            .ok()
            .filter(|o| o.success && !o.stdout.is_empty())
            .map(|o| o.stdout)
    }

    /// Check if GPG signing is configured
    pub fn signing_configured(&self) -> bool {
        self.signing_key().is_some()
    }

    /// Check if commit.gpgsign is enabled
    pub fn commit_gpgsign_enabled(&self) -> bool {
        self.config_bool("commit.gpgsign")
    }

    /// Check if tag.gpgSign is enabled
    pub fn tag_gpgsign_enabled(&self) -> bool {
        self.config_bool("tag.gpgSign")
    }

    fn config_bool(&self, key: &str) -> bool {
        self.run_shell(&["config", "--type=bool", key])
            .map(|o| o.success && o.stdout == "true")
            .unwrap_or(false)
    }

    /// Check if a local branch exists
    pub fn branch_exists(&self, name: &str) -> bool {
        self.ref_exists(&format!("refs/heads/{}", name))
    }

    /// Check if a remote branch exists
    pub fn remote_branch_exists(&self, remote: &str, branch: &str) -> bool {
        self.ref_exists(&format!("refs/remotes/{}/{}", remote, branch))
    }

    fn ref_exists(&self, full_ref: &str) -> bool {
        self.run_shell(&["show-ref", "--verify", "--quiet", full_ref])
            .map(|o| o.success)
            .unwrap_or(false)
    }

    /// Create a new branch (without checking out)
    pub fn create_branch(&self, name: &str) -> Result<(), GitError> {
        if self.branch_exists(name) {
            return Err(GitError::BranchExists {
                branch: name.to_string(),
            });
        }
        self.run_checked(&["branch", "--", name])?;
        Ok(())
    }

    /// List all local branches
    pub fn list_local_branches(&self) -> Result<Vec<BranchInfoEntry>, GitError> {
        self.list_branches_in("refs/heads")
    }

    /// List remote branches
    pub fn list_remote_branches(&self) -> Result<Vec<BranchInfoEntry>, GitError> {
        Ok(self
            .list_branches_in("refs/remotes")?
            .into_iter()
            .filter(|b| !b.name.contains("HEAD"))
            .collect())
    }

    fn list_branches_in(&self, namespace: &str) -> Result<Vec<BranchInfoEntry>, GitError> {
        let out = self.run_checked(&[
            "for-each-ref",
            "--format=%(refname:short)%00%(objectname)%00%(subject)",
            namespace,
        ])?;
        Ok(out
            .stdout
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| {
                let mut parts = l.split('\0');
                Some(BranchInfoEntry {
                    name: parts.next()?.to_string(),
                    commit: parts.next()?.to_string(),
                    message: parts.next().unwrap_or("").to_string(),
                })
            })
            .collect())
    }

    /// Get the current HEAD commit SHA (full 40-char)
    pub fn head_sha(&self) -> Result<String, GitError> {
        Ok(self.run_checked(&["rev-parse", "HEAD"])?.stdout)
    }

    /// Get the short commit SHA (first 7 characters)
    pub fn head_sha_short(&self) -> Result<String, GitError> {
        let sha = self.head_sha()?;
        Ok(sha[..7].to_string())
    }

    /// Check if a tag exists
    pub fn tag_exists(&self, name: &str) -> bool {
        self.ref_exists(&format!("refs/tags/{}", name))
    }

    /// List tags, optionally filtered by a glob pattern
    pub fn list_tags(&self, pattern: Option<&str>) -> Result<Vec<String>, GitError> {
        let out = match pattern {
            Some(pat) => self.run_checked(&["tag", "--list", "--", pat])?,
            None => self.run_checked(&["tag", "--list"])?,
        };
        let mut tags: Vec<String> = out
            .stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        tags.sort();
        Ok(tags)
    }

    /// Get tag info (tagger, date, message, commit)
    pub fn tag_info(&self, name: &str) -> Result<TagInfoEntry, GitError> {
        let out = self.run_checked(&[
            "for-each-ref",
            "--format=%(objecttype)%00%(objectname)%00%(*objectname)%00%(taggername)%00%(taggeremail)%00%(taggerdate:iso-strict)%00%(contents)",
            &format!("refs/tags/{}", name),
        ])?;
        if out.stdout.is_empty() {
            return Err(GitError::CommandFailed {
                message: format!("Tag '{}' not found", name),
            });
        }
        let mut parts = out.stdout.splitn(7, '\0');
        let objecttype = parts.next().unwrap_or("");
        let objectname = parts.next().unwrap_or("");
        let peeled = parts.next().unwrap_or("");
        let tagger_name = parts.next().unwrap_or("");
        let tagger_email = parts.next().unwrap_or("");
        let tagger_date = parts.next().unwrap_or("");
        let message = parts.next().unwrap_or("");

        if objecttype == "tag" {
            // Annotated tag: peeled = the commit it points to
            let tagger = if tagger_name.is_empty() {
                None
            } else {
                // taggeremail comes with surrounding <> already
                Some(format!("{} {}", tagger_name, tagger_email))
            };
            Ok(TagInfoEntry {
                tagger,
                date: if tagger_date.is_empty() {
                    None
                } else {
                    Some(tagger_date.to_string())
                },
                message: Some(message.to_string()),
                commit: Some(peeled.to_string()),
            })
        } else {
            // Lightweight tag pointing directly to a commit
            Ok(TagInfoEntry {
                tagger: None,
                date: None,
                message: None,
                commit: Some(objectname.to_string()),
            })
        }
    }

    /// Get the default remote name (usually "origin")
    pub fn default_remote(&self) -> Result<String, GitError> {
        let out = self.run_checked(&["remote"])?;
        let remotes: Vec<&str> = out.stdout.lines().filter(|l| !l.is_empty()).collect();
        if remotes.contains(&"origin") {
            Ok("origin".to_string())
        } else {
            Ok(remotes.first().unwrap_or(&"origin").to_string())
        }
    }

    /// Get the URL of a remote
    pub fn remote_url(&self, remote: &str) -> Result<String, GitError> {
        let out = self.run_shell(&["remote", "get-url", "--", remote])?;
        if out.success && !out.stdout.is_empty() {
            Ok(out.stdout)
        } else {
            Err(GitError::CommandFailed {
                message: format!("Remote '{}' not found", remote),
            })
        }
    }

    /// Get the base branch (main or master)
    pub fn base_branch(&self) -> String {
        if self.branch_exists("main") {
            return "main".to_string();
        }
        if self.branch_exists("master") {
            return "master".to_string();
        }
        "main".to_string()
    }

    /// Resolve a reference to its commit SHA
    pub fn rev_parse(&self, reference: &str) -> Result<String, GitError> {
        Ok(self
            .run_checked(&["rev-parse", "--verify", reference])?
            .stdout)
    }

    // ========================================================================
    // Index / staging
    // ========================================================================

    /// Stage a single file for commit
    pub fn stage_file(&self, path: &Path) -> Result<(), GitError> {
        self.stage_files(&[path])
    }

    /// Stage multiple files for commit (handles deletions via -A)
    pub fn stage_files(&self, paths: &[&Path]) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args: Vec<&str> = vec!["add", "-A", "--"];
        let path_strs: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        args.extend(path_strs.iter().map(|s| s.as_str()));
        self.run_checked(&args)?;
        Ok(())
    }

    /// Get list of staged files (files in index that differ from HEAD)
    pub fn staged_files(&self) -> Result<Vec<String>, GitError> {
        let out = self.run_shell(&["diff", "--cached", "--name-only"])?;
        if out.success {
            Ok(out
                .stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect())
        } else {
            // Unborn HEAD (no commits yet): everything in the index is staged
            let out = self.run_checked(&["ls-files"])?;
            Ok(out
                .stdout
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect())
        }
    }

    /// Check if a file path is tracked (exists in the index)
    pub fn is_tracked(&self, path: &str) -> Result<bool, GitError> {
        let path = path.replace('\\', "/");
        Ok(self
            .run_shell(&["ls-files", "--error-unmatch", "--", &path])?
            .success)
    }

    /// Unstage files (reset index entries to match HEAD)
    pub fn unstage_files(&self, paths: &[&str]) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }
        let normalized: Vec<String> = paths.iter().map(|p| p.replace('\\', "/")).collect();

        let mut args: Vec<&str> = vec!["reset", "-q", "HEAD", "--"];
        args.extend(normalized.iter().map(|s| s.as_str()));
        let out = self.run_shell(&args)?;
        if out.success {
            return Ok(());
        }

        // Unborn HEAD (no commits yet): remove the entries from the index
        let mut args: Vec<&str> = vec!["rm", "--cached", "-q", "--ignore-unmatch", "--"];
        args.extend(normalized.iter().map(|s| s.as_str()));
        self.run_checked(&args)?;
        Ok(())
    }

    // ========================================================================
    // Commit / tag / log
    // ========================================================================

    /// Commit staged changes (unsigned), returning the new commit hash.
    ///
    /// Signing is explicitly disabled to match the gix implementation used on
    /// mobile; use `create_signed_commit` for signed commits.
    pub fn commit(&self, message: &str) -> Result<String, GitError> {
        self.run_checked(&["-c", "commit.gpgsign=false", "commit", "-m", message])?;
        self.head_sha()
    }

    /// Create a tag (annotated when a message is given, lightweight otherwise)
    pub fn create_tag(&self, name: &str, message: Option<&str>) -> Result<(), GitError> {
        match message {
            Some(msg) => self.run_checked(&[
                "-c",
                "tag.gpgSign=false",
                "tag",
                "-a",
                "-m",
                msg,
                "--",
                name,
            ])?,
            None => self.run_checked(&["tag", "--", name])?,
        };
        Ok(())
    }

    /// Get recent commits from HEAD
    pub fn recent_commits(&self, limit: u32) -> Result<Vec<CommitLogEntry>, GitError> {
        let limit_arg = format!("-n{}", limit);
        let out = self.run_checked(&[
            "log",
            &limit_arg,
            "--format=%H%x00%h%x00%s%x00%an%x00%ae%x00%aI%x00%G?",
        ])?;
        Ok(parse_log_output(&out.stdout))
    }
}
