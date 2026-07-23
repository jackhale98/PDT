//! Git abstraction layer: shell `git` on desktop, gix on mobile
//!
//! Desktop/CLI builds shell out to the `git` binary for everything and never
//! compile `gix` (129 fewer crates in the build). Mobile targets (iOS/Android)
//! have no `git` binary, so local operations (status, stage, commit, log,
//! branch/tag info) are compiled from the pure-Rust `gix` implementations
//! instead; operations that inherently need the CLI (network, GPG signing,
//! stash, merge, checkout) return `GitError::ShellNotAvailable` there.
//!
//! Both implementations expose an identical API on `Git`; the `gix-vc` cargo
//! feature compiles the gix path on desktop for CI compile-checking only.

// gix-backed local operations: compiled only where no `git` binary exists
// (mobile) or when explicitly requested for compile-checking (`gix-vc`).
#[cfg(any(target_os = "ios", target_os = "android", feature = "gix-vc"))]
mod commit;
#[cfg(any(target_os = "ios", target_os = "android", feature = "gix-vc"))]
mod index;
#[cfg(any(target_os = "ios", target_os = "android", feature = "gix-vc"))]
mod repo;
mod shell;
// Shell-backed local operations: the only implementation on desktop/CLI.
#[cfg(not(any(target_os = "ios", target_os = "android", feature = "gix-vc")))]
mod shell_local;

use std::path::{Path, PathBuf};
use thiserror::Error;

// ============================================================================
// Public types
// ============================================================================

/// Git operations abstraction backed by gitoxide
pub struct Git {
    repo_root: PathBuf,
}

/// Result of a git shell command execution
#[derive(Debug)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

/// Entry from git log
#[derive(Debug, Clone)]
pub struct CommitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub is_signed: bool,
}

/// Branch info from listing
#[derive(Debug, Clone)]
pub struct BranchInfoEntry {
    pub name: String,
    pub commit: String,
    pub message: String,
}

/// Tag detail info
#[derive(Debug, Clone)]
pub struct TagInfoEntry {
    pub tagger: Option<String>,
    pub date: Option<String>,
    pub message: Option<String>,
    pub commit: Option<String>,
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

    #[error("Git CLI not available (expected on mobile)")]
    ShellNotAvailable,

    #[error("Git not installed or not in PATH")]
    GitNotFound,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Convert any gix error to GitError::CommandFailed
#[cfg(any(target_os = "ios", target_os = "android", feature = "gix-vc"))]
fn gix_err(e: impl std::fmt::Display) -> GitError {
    GitError::CommandFailed {
        message: e.to_string(),
    }
}

/// Parse NUL-delimited log output from `git log --format` (fields separated
/// with %x00 so `|` in commit subjects or author names can't break parsing)
fn parse_log_output(stdout: &str) -> Vec<CommitLogEntry> {
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\0').collect();
            if parts.len() >= 7 {
                Some(CommitLogEntry {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    message: parts[2].to_string(),
                    author: parts[3].to_string(),
                    author_email: parts[4].to_string(),
                    date: parts[5].to_string(),
                    is_signed: parts[6] == "G" || parts[6] == "U",
                })
            } else {
                None
            }
        })
        .collect()
}

/// Simple glob pattern matching (supports `*` and `?`)
#[cfg(any(
    any(target_os = "ios", target_os = "android", feature = "gix-vc"),
    test
))]
fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_inner(pattern.as_bytes(), text.as_bytes())
}

#[cfg(any(
    any(target_os = "ios", target_os = "android", feature = "gix-vc"),
    test
))]
fn glob_match_inner(pattern: &[u8], text: &[u8]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX;
    let mut star_ti = 0;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

/// Format a gix time as ISO 8601
#[cfg(any(target_os = "ios", target_os = "android", feature = "gix-vc"))]
fn format_gix_time(time: gix::date::Time) -> String {
    let secs = time.seconds;
    // gix-date's offset is already in seconds (OffsetInSeconds).
    let offset_secs = time.offset;
    if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
        if let Some(tz) = chrono::FixedOffset::east_opt(offset_secs) {
            return dt.with_timezone(&tz).to_rfc3339();
        }
        return dt.to_rfc3339();
    }
    format!("{}", secs)
}

/// Check if file metadata indicates executable permission
#[cfg(all(
    unix,
    any(target_os = "ios", target_os = "android", feature = "gix-vc")
))]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(all(
    not(unix),
    any(target_os = "ios", target_os = "android", feature = "gix-vc")
))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

// ============================================================================
// Constructor + utility
// ============================================================================

impl Git {
    /// Create a new Git instance for the repository at the given path
    pub fn new(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
        }
    }

    /// Open the repository with gix
    #[cfg(any(target_os = "ios", target_os = "android", feature = "gix-vc"))]
    fn open_repo(&self) -> Result<gix::Repository, GitError> {
        gix::open(&self.repo_root).map_err(|_| GitError::NotARepo)
    }

    /// Format a command for display (dry-run output)
    pub fn format_command(args: &[&str]) -> String {
        format!("git {}", args.join(" "))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
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
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();
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

    #[test]
    fn test_recent_commits() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();

        std::fs::write(tmp.path().join("test.txt"), "updated content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Second commit").unwrap();

        let commits = git.recent_commits(10).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "Second commit");
        assert_eq!(commits[1].message, "Initial commit");
    }

    #[test]
    fn test_list_local_branches() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();

        git.create_branch("feature-1").unwrap();
        git.create_branch("feature-2").unwrap();

        let branches = git.list_local_branches().unwrap();
        assert!(branches.len() >= 3);
    }

    #[test]
    fn test_head_sha() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();

        let sha = git.head_sha().unwrap();
        assert_eq!(sha.len(), 40);
        let short = git.head_sha_short().unwrap();
        assert_eq!(short.len(), 7);
    }

    #[test]
    fn test_list_tags() {
        let (tmp, git) = init_test_repo();
        std::fs::write(tmp.path().join("test.txt"), "content").unwrap();
        git.stage_file(&tmp.path().join("test.txt")).unwrap();
        git.commit("Initial commit").unwrap();

        git.create_tag("v1.0", Some("Release 1.0")).unwrap();
        git.create_tag("v2.0", Some("Release 2.0")).unwrap();

        let tags = git.list_tags(None).unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"v1.0".to_string()));

        let filtered = git.list_tags(Some("v1*")).unwrap();
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("v1*", "v1.0"));
        assert!(glob_match("v1*", "v1.0.0"));
        assert!(!glob_match("v1*", "v2.0"));
        assert!(glob_match("release/*", "release/1.0"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("test?", "test1"));
        assert!(!glob_match("test?", "test12"));
    }
}
