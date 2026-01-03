//! Workflow and team collaboration tests

mod common;

use common::{create_test_requirement, create_test_risk, setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to find entity file by ID prefix
fn find_entity_file(tmp: &TempDir, id: &str, prefix: &str) -> Option<std::path::PathBuf> {
    if !id.starts_with(prefix) {
        return None;
    }

    for entry in walkdir::WalkDir::new(tmp.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            if content.contains(&format!("id: {}", id))
                || content.contains(&format!("id: \"{}\"", id))
            {
                return Some(entry.path().to_path_buf());
            }
        }
    }
    None
}

// ============================================================================
// Full Workflow Test
// ============================================================================

#[test]
fn test_full_workflow() {
    let tmp = setup_test_project();

    // Create input requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Temperature Range",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Create output requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Thermal Design",
            "--type",
            "output",
            "--no-edit",
        ])
        .assert()
        .success();

    // Create risk
    tdt()
        .current_dir(tmp.path())
        .args(["risk", "new", "--title", "Overheating", "--no-edit"])
        .assert()
        .success();

    // List all
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 requirement(s)"));

    // Validate
    tdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .success();
}

// ============================================================================
// Workflow Review Tests
// ============================================================================

#[test]
fn test_workflow_review_list_empty() {
    let tmp = setup_test_project();

    // Review list on empty project should work
    tdt()
        .current_dir(tmp.path())
        .args(["review", "list"])
        .assert()
        .success();
}

#[test]
fn test_workflow_review_summary() {
    let tmp = setup_test_project();

    // Review summary should work
    tdt()
        .current_dir(tmp.path())
        .args(["review", "summary"])
        .assert()
        .success();
}

#[test]
fn test_workflow_review_list_target_flag() {
    let tmp = setup_test_project();

    // Review list with --target flag should work (falls back to local scan)
    tdt()
        .current_dir(tmp.path())
        .args(["review", "list", "--target", "main"])
        .assert()
        .success();
}

#[test]
fn test_workflow_review_list_needs_role_flag() {
    let tmp = setup_test_project();

    // Review list with --needs-role flag should work
    tdt()
        .current_dir(tmp.path())
        .args(["review", "list", "--needs-role"])
        .assert()
        .success();
}

#[test]
fn test_workflow_review_list_all_open_flag() {
    let tmp = setup_test_project();

    // Review list with --all-open flag should work
    tdt()
        .current_dir(tmp.path())
        .args(["review", "list", "--all-open"])
        .assert()
        .success();
}

#[test]
fn test_workflow_review_list_with_review_status_entity() {
    let tmp = setup_test_project();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Review Test Req",
            "-t",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Manually update the status to review in the YAML file
    let req_dir = tmp.path().join("requirements/inputs");
    let yaml_file = std::fs::read_dir(&req_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .expect("Should find a requirement file");

    let content = std::fs::read_to_string(yaml_file.path()).unwrap();
    let updated = content.replace("status: draft", "status: review");
    std::fs::write(yaml_file.path(), updated).unwrap();

    // Review list should show the entity
    tdt()
        .current_dir(tmp.path())
        .args(["review", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Review Test Req"));
}

#[test]
fn test_workflow_review_pending_approvals() {
    let tmp = setup_test_project();

    // Pending approvals subcommand should work
    tdt()
        .current_dir(tmp.path())
        .args(["review", "pending-approvals"])
        .assert()
        .success();
}

#[test]
fn test_workflow_team_list_no_roster() {
    let tmp = setup_test_project();

    // Team list should fail gracefully when no roster exists
    tdt()
        .current_dir(tmp.path())
        .args(["team", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No team roster found"));
}

#[test]
fn test_workflow_team_whoami_no_roster() {
    let tmp = setup_test_project();

    // Whoami should fail gracefully when no roster exists
    tdt()
        .current_dir(tmp.path())
        .args(["team", "whoami"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No team roster found"));
}

// ============================================================================
// Baseline Command Tests
// ============================================================================

#[test]
fn test_baseline_list_empty() {
    let tmp = setup_test_project();

    // Baseline list on project without baselines
    tdt()
        .current_dir(tmp.path())
        .args(["baseline", "list"])
        .assert()
        .success();
}

// ============================================================================
// History/Blame/Diff Command Tests
// ============================================================================

#[test]
fn test_history_command() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "History Test", "input");

    if !req_id.is_empty() {
        // History should work (though may show no commits in fresh repo)
        tdt()
            .current_dir(tmp.path())
            .args(["history", &req_id])
            .assert()
            .success();
    }
}

#[test]
fn test_diff_command() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Diff Test", "input");

    if !req_id.is_empty() {
        // Diff should work
        tdt()
            .current_dir(tmp.path())
            .args(["diff", &req_id])
            .assert()
            .success();
    }
}

// ============================================================================
// Submit Tests
// ============================================================================

#[test]
fn test_submit_reviewer_flag_shows_in_help() {
    // The --reviewer flag should be documented in help
    tdt()
        .args(["submit", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--reviewer"));
}

#[test]
fn test_submit_accepts_reviewer_flag() {
    let tmp = setup_test_project();

    // Enable workflow
    let config_path = tmp.path().join(".tdt/config.yaml");
    fs::write(
        &config_path,
        "workflow:\n  enabled: true\n  provider: none\n",
    )
    .unwrap();

    // Initialize git repo for submit to work
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Create a requirement
    let req_id = create_test_requirement(&tmp, "Reviewer Test", "input");

    if !req_id.is_empty() {
        // Submit with --reviewer flag (dry-run to avoid git push)
        tdt()
            .current_dir(tmp.path())
            .args([
                "submit",
                &req_id,
                "--reviewer",
                "jsmith,bwilson",
                "--dry-run",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("--reviewer"));
    }
}

// ============================================================================
// History/Log Tests
// ============================================================================

#[test]
fn test_history_workflow_flag_shows_in_help() {
    // The --workflow flag should be documented in help
    tdt()
        .args(["history", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--workflow"));
}

#[test]
fn test_history_workflow_shows_approvals() {
    let tmp = setup_test_project();

    // Create a requirement with an approval record
    let req_id = create_test_requirement(&tmp, "History Workflow Test", "input");

    if !req_id.is_empty() {
        // Find the file and add approval record manually
        let req_file = find_entity_file(&tmp, &req_id, "REQ-");
        if let Some(path) = req_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace(
                "status: draft",
                "status: approved\napprovals:\n  - approver: jsmith\n    role: engineering\n    timestamp: 2024-01-15T10:30:00Z\n    comment: LGTM",
            );
            fs::write(&path, updated).unwrap();

            // Run history --workflow
            tdt()
                .current_dir(tmp.path())
                .args(["history", &req_id, "--workflow"])
                .assert()
                .success()
                .stdout(predicate::str::contains("jsmith"));
        }
    }
}

#[test]
fn test_log_command_exists() {
    // The log command should be available
    tdt()
        .args(["log", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("workflow"));
}

#[test]
fn test_log_lists_workflow_events() {
    let tmp = setup_test_project();

    // Create entities with approval records
    let req_id = create_test_requirement(&tmp, "Log Test Req", "input");
    let risk_id = create_test_risk(&tmp, "Log Test Risk", "design");

    // Add approval records
    if !req_id.is_empty() {
        let req_file = find_entity_file(&tmp, &req_id, "REQ-");
        if let Some(path) = req_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace(
                "status: draft",
                "status: approved\napprovals:\n  - approver: alice\n    timestamp: 2024-01-15T10:30:00Z",
            );
            fs::write(&path, updated).unwrap();
        }
    }

    if !risk_id.is_empty() {
        let risk_file = find_entity_file(&tmp, &risk_id, "RISK-");
        if let Some(path) = risk_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace(
                "status: draft",
                "status: approved\napprovals:\n  - approver: bob\n    timestamp: 2024-01-16T14:00:00Z",
            );
            fs::write(&path, updated).unwrap();
        }
    }

    // Run tdt log
    tdt()
        .current_dir(tmp.path())
        .args(["log"])
        .assert()
        .success();
}

#[test]
fn test_log_filter_by_approver() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Filter Test", "input");

    if !req_id.is_empty() {
        let req_file = find_entity_file(&tmp, &req_id, "REQ-");
        if let Some(path) = req_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace(
                "status: draft",
                "status: approved\napprovals:\n  - approver: alice\n    timestamp: 2024-01-15T10:30:00Z",
            );
            fs::write(&path, updated).unwrap();
        }

        // Filter by approver
        tdt()
            .current_dir(tmp.path())
            .args(["log", "--approver", "alice"])
            .assert()
            .success()
            .stdout(predicate::str::contains("alice"));
    }
}

#[test]
fn test_log_filter_by_entity_type() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Type Filter Test", "input");

    if !req_id.is_empty() {
        let req_file = find_entity_file(&tmp, &req_id, "REQ-");
        if let Some(path) = req_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace(
                "status: draft",
                "status: approved\napprovals:\n  - approver: alice\n    timestamp: 2024-01-15T10:30:00Z",
            );
            fs::write(&path, updated).unwrap();
        }

        // Filter by entity type
        tdt()
            .current_dir(tmp.path())
            .args(["log", "--entity-type", "req"])
            .assert()
            .success();
    }
}

// ============================================================================
// Review Pending Tests
// ============================================================================

#[test]
fn test_review_pending_approvals_command() {
    let tmp = setup_test_project();

    // The pending-approvals subcommand should exist
    tdt()
        .current_dir(tmp.path())
        .args(["review", "pending-approvals"])
        .assert()
        .success();
}

#[test]
fn test_review_pending_approvals_shows_partial_approvals() {
    let tmp = setup_test_project();

    // Enable workflow with multi-approval requirement
    let config_path = tmp.path().join(".tdt/config.yaml");
    fs::write(
        &config_path,
        "workflow:\n  enabled: true\n  provider: none\n  approvals:\n    RISK:\n      min_approvals: 2\n",
    )
    .unwrap();

    // Create a risk with one approval (needs 2)
    let risk_id = create_test_risk(&tmp, "Pending Approval Test", "design");

    if !risk_id.is_empty() {
        let risk_file = find_entity_file(&tmp, &risk_id, "RISK-");
        if let Some(path) = risk_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace(
                "status: draft",
                "status: review\napprovals:\n  - approver: alice\n    role: engineering\n    timestamp: 2024-01-15T10:30:00Z",
            );
            fs::write(&path, updated).unwrap();
        }

        // Should show the entity needs more approvals
        tdt()
            .current_dir(tmp.path())
            .args(["review", "pending-approvals"])
            .assert()
            .success()
            .stdout(predicate::str::contains("1/2").or(predicate::str::contains("RISK")));
    }
}

// ============================================================================
// Approve Tests
// ============================================================================

#[test]
fn test_approve_creates_git_tag() {
    let tmp = setup_test_project();

    // Enable workflow
    let config_path = tmp.path().join(".tdt/config.yaml");
    fs::write(
        &config_path,
        "workflow:\n  enabled: true\n  provider: none\n  auto_commit: true\n",
    )
    .unwrap();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Create a requirement and set it to review status
    let req_id = create_test_requirement(&tmp, "Tag Test", "input");

    if !req_id.is_empty() {
        let req_file = find_entity_file(&tmp, &req_id, "REQ-");
        if let Some(path) = req_file {
            let content = fs::read_to_string(&path).unwrap();
            let updated = content.replace("status: draft", "status: review");
            fs::write(&path, updated).unwrap();

            // Stage the change
            std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(tmp.path())
                .output()
                .unwrap();
            std::process::Command::new("git")
                .args(["commit", "-m", "Set to review"])
                .current_dir(tmp.path())
                .output()
                .unwrap();

            // Approve the entity
            tdt()
                .current_dir(tmp.path())
                .args(["approve", &req_id, "-y"])
                .assert()
                .success();

            // Check that a tag was created
            let output = std::process::Command::new("git")
                .args(["tag", "-l", "approve/*"])
                .current_dir(tmp.path())
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("approve/"),
                "Expected approval tag, got: {}",
                stdout
            );
        }
    }
}

// ============================================================================
// GPG Signing Tests
// ============================================================================

#[test]
fn test_team_setup_signing_status() {
    let tmp = setup_test_project();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Configure git user
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Check signing status - should work even without GPG configured
    tdt()
        .current_dir(tmp.path())
        .args(["team", "setup-signing", "--status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Signing Configuration Status"))
        .stdout(predicate::str::contains("user.signingkey"));
}

#[test]
fn test_team_setup_signing_no_key() {
    let tmp = setup_test_project();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Configure git user but NOT signing key
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Try to setup signing without providing key - should error with helpful message
    tdt()
        .current_dir(tmp.path())
        .args(["team", "setup-signing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No GPG signing key configured"))
        .stderr(predicate::str::contains("gpg --full-generate-key"));
}

#[test]
fn test_release_sign_without_gpg_configured() {
    let tmp = setup_test_project();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Configure git user but NOT signing key
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Enable workflow
    let config_path = tmp.path().join(".tdt/config.yaml");
    fs::write(
        &config_path,
        "workflow:\n  enabled: true\n  provider: none\n",
    )
    .unwrap();

    // Create and setup a requirement for release
    let req_id = create_test_requirement(&tmp, "Test Req", "input");

    // Stage and commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Find and modify the file to approved status
    if let Some(path) = find_entity_file(&tmp, &req_id, "REQ-") {
        let content = fs::read_to_string(&path).unwrap();
        let updated = content.replace("status: draft", "status: approved");
        fs::write(&path, updated).unwrap();

        // Stage the change
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Set to approved"])
            .current_dir(tmp.path())
            .output()
            .unwrap();

        // Try to release with --sign without GPG configured
        tdt()
            .current_dir(tmp.path())
            .args(["release", &req_id, "-y", "--sign"])
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "GPG signing requested but not configured",
            ))
            .stderr(predicate::str::contains("git config"));
    }
}

#[test]
fn test_team_setup_signing_ssh_no_key() {
    let tmp = setup_test_project();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Configure git user
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Try SSH signing without an SSH key - should provide helpful error
    // Note: This test may pass if the user has SSH keys, so we just check
    // that the command runs and mentions SSH
    let result = tdt()
        .current_dir(tmp.path())
        .args(["team", "setup-signing", "--method", "ssh"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&result.stderr);
    let stdout = String::from_utf8_lossy(&result.stdout);

    // Either SSH key found (mentions SSH in stdout) or not found (mentions ssh-keygen in stderr)
    assert!(
        stdout.contains("SSH") || stderr.contains("ssh-keygen") || stderr.contains("SSH"),
        "Expected SSH-related output, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_team_setup_signing_gitsign_not_installed() {
    let tmp = setup_test_project();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Configure git user
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    // Try gitsign - if not installed, should provide helpful error
    let result = tdt()
        .current_dir(tmp.path())
        .args(["team", "setup-signing", "--method", "gitsign"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&result.stderr);
    let stdout = String::from_utf8_lossy(&result.stdout);

    // Either gitsign is installed (mentions Sigstore in stdout) or not (mentions installation in stderr)
    assert!(
        stdout.contains("Sigstore")
            || stdout.contains("gitsign")
            || stderr.contains("gitsign is not installed")
            || stderr.contains("brew install"),
        "Expected gitsign-related output, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

// ============================================================================
// Key Management Tests
// ============================================================================

#[test]
fn test_team_add_key_gpg_no_key_configured() {
    let tmp = setup_test_project();

    // Initialize git with no signing key
    std::process::Command::new("git")
        .current_dir(tmp.path())
        .args(["init"])
        .output()
        .unwrap();

    std::process::Command::new("git")
        .current_dir(tmp.path())
        .args(["config", "user.name", "Test User"])
        .output()
        .unwrap();

    std::process::Command::new("git")
        .current_dir(tmp.path())
        .args(["config", "user.email", "test@example.com"])
        .output()
        .unwrap();

    // GPG add-key without a configured signing key should fail
    tdt()
        .current_dir(tmp.path())
        .args(["team", "add-key", "--method", "gpg", "-y"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No GPG signing key configured"));
}

#[test]
fn test_team_add_key_gitsign_no_key_needed() {
    let tmp = setup_test_project();

    // Initialize git
    std::process::Command::new("git")
        .current_dir(tmp.path())
        .args(["init"])
        .output()
        .unwrap();

    std::process::Command::new("git")
        .current_dir(tmp.path())
        .args(["config", "user.name", "Test User"])
        .output()
        .unwrap();

    std::process::Command::new("git")
        .current_dir(tmp.path())
        .args(["config", "user.email", "test@example.com"])
        .output()
        .unwrap();

    // Gitsign doesn't need a key file
    tdt()
        .current_dir(tmp.path())
        .args(["team", "add-key", "--method", "gitsign", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("keyless OIDC-based signing"));
}

#[test]
fn test_team_import_keys_no_keys() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["team", "import-keys", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No GPG keys found"));
}

#[test]
fn test_team_sync_keys_no_keys() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["team", "sync-keys", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No SSH keys found"));
}

#[test]
fn test_team_sync_keys_generates_allowed_signers() {
    let tmp = setup_test_project();

    // Create .tdt/keys/ssh directory and add a test key
    let ssh_dir = tmp.path().join(".tdt").join("keys").join("ssh");
    fs::create_dir_all(&ssh_dir).unwrap();
    fs::write(
        ssh_dir.join("testuser.pub"),
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITestKey test@example.com\n",
    )
    .unwrap();

    // Create team roster with testuser
    let team_yaml = tmp.path().join(".tdt").join("team.yaml");
    fs::write(
        &team_yaml,
        "version: 1\nmembers:\n  - name: Test User\n    email: test@example.com\n    username: testuser\n    roles: [engineering]\n",
    )
    .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["team", "sync-keys", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated"));

    // Check that allowed_signers was created
    let allowed_signers = tmp.path().join(".tdt").join("keys").join("allowed_signers");
    assert!(allowed_signers.exists());

    let content = fs::read_to_string(&allowed_signers).unwrap();
    assert!(content.contains("test@example.com"));
    assert!(content.contains("ssh-ed25519"));
}

#[test]
fn test_team_add_with_signing_format() {
    let tmp = setup_test_project();

    // Initialize team roster
    tdt()
        .current_dir(tmp.path())
        .args(["team", "init"])
        .assert()
        .success();

    // Add member with signing format
    tdt()
        .current_dir(tmp.path())
        .args([
            "team",
            "add",
            "--name",
            "Jane Smith",
            "--email",
            "jane@example.com",
            "--username",
            "jsmith",
            "--roles",
            "engineering",
            "--signing-format",
            "ssh",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Signing: ssh"));

    // Verify it's in the team.yaml
    let team_yaml = tmp.path().join(".tdt").join("team.yaml");
    let content = fs::read_to_string(&team_yaml).unwrap();
    assert!(content.contains("signing_format: ssh"));
}
