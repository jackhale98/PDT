//! Integration tests for PDT CLI
//!
//! These tests exercise the CLI commands end-to-end using assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to get a pdt command
fn pdt() -> Command {
    Command::cargo_bin("pdt").unwrap()
}

/// Helper to create a test project in a temp directory
fn setup_test_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    pdt()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success();
    tmp
}

/// Helper to create a test requirement
fn create_test_requirement(tmp: &TempDir, title: &str, req_type: &str) -> String {
    let output = pdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", title, "--type", req_type, "--no-edit"])
        .output()
        .unwrap();

    // Extract ID from output
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output format: "✓ Created requirement REQ-01ABC..."
    stdout
        .lines()
        .find(|l| l.contains("REQ-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("REQ-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test risk
fn create_test_risk(tmp: &TempDir, title: &str, risk_type: &str) -> String {
    let output = pdt()
        .current_dir(tmp.path())
        .args(["risk", "new", "--title", title, "--type", risk_type, "--no-edit"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("RISK-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("RISK-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

// ============================================================================
// CLI Basic Tests
// ============================================================================

#[test]
fn test_help_displays() {
    pdt()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("product development artifacts"));
}

#[test]
fn test_version_displays() {
    pdt()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("pdt"));
}

#[test]
fn test_unknown_command_fails() {
    pdt()
        .arg("unknown-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

// ============================================================================
// Init Command Tests
// ============================================================================

#[test]
fn test_init_creates_project_structure() {
    let tmp = TempDir::new().unwrap();

    pdt()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized"));

    // Verify structure
    assert!(tmp.path().join(".pdt").exists());
    assert!(tmp.path().join(".pdt/config.yaml").exists());
    assert!(tmp.path().join("requirements/inputs").is_dir());
    assert!(tmp.path().join("requirements/outputs").is_dir());
    assert!(tmp.path().join("risks/design").is_dir());
    assert!(tmp.path().join("risks/process").is_dir());
    assert!(tmp.path().join("verification/protocols").is_dir());
    assert!(tmp.path().join("verification/results").is_dir());
}

#[test]
fn test_init_fails_if_project_exists() {
    let tmp = setup_test_project();

    // Init without --force should warn but not fail (it prints to stdout)
    pdt()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn test_init_force_overwrites() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["init", "--force"])
        .assert()
        .success();
}

// ============================================================================
// Requirement Command Tests
// ============================================================================

#[test]
fn test_req_new_creates_file() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Test Requirement", "--type", "input", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created requirement"));

    // Verify file was created
    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1, "Expected exactly one requirement file");

    // Verify content
    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Test Requirement"));
    assert!(content.contains("type: input"));
}

#[test]
fn test_req_new_output_creates_in_outputs_dir() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Output Spec", "--type", "output", "--no-edit"])
        .assert()
        .success();

    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/outputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_req_list_empty_project() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No requirements found"));
}

#[test]
fn test_req_list_shows_requirements() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "First Requirement", "input");
    create_test_requirement(&tmp, "Second Requirement", "output");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First Requirement"))
        .stdout(predicate::str::contains("Second Requirement"))
        .stdout(predicate::str::contains("2 requirement(s) found"));
}

#[test]
fn test_req_list_shows_short_ids() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Test Req", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@1"));
}

#[test]
fn test_req_show_by_partial_id() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Temperature Range", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ-"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Temperature Range"));
}

#[test]
fn test_req_show_by_short_id() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Test Req", "input");

    // First list to generate short IDs
    pdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Then show by short ID
    pdt()
        .current_dir(tmp.path())
        .args(["req", "show", "@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Req"));
}

#[test]
fn test_req_show_not_found() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ-NONEXISTENT"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No requirement found"));
}

#[test]
fn test_req_list_json_format() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "JSON Test", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "-f", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("\"title\""))
        .stdout(predicate::str::contains("JSON Test"));
}

#[test]
fn test_req_list_csv_format() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "CSV Test", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "-f", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("short_id,id,type,title"))
        .stdout(predicate::str::contains("CSV Test"));
}

// ============================================================================
// Requirement Filtering Tests
// ============================================================================

#[test]
fn test_req_list_filter_by_type() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Input Req", "input");
    create_test_requirement(&tmp, "Output Req", "output");

    // Filter by input type
    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--type", "input"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Input Req"))
        .stdout(predicate::str::contains("1 requirement(s) found"));

    // Filter by output type
    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--type", "output"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Output Req"))
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_req_list_search_filter() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Temperature Range", "input");
    create_test_requirement(&tmp, "Power Supply", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--search", "temperature"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Temperature Range"))
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_req_list_limit() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Req One", "input");
    create_test_requirement(&tmp, "Req Two", "input");
    create_test_requirement(&tmp, "Req Three", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "-n", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 requirement(s) found"));
}

#[test]
fn test_req_list_count_only() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Req One", "input");
    create_test_requirement(&tmp, "Req Two", "input");

    let output = pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--count"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let count_str = String::from_utf8_lossy(&output);
    assert!(count_str.trim() == "2", "Expected count '2', got '{}'", count_str.trim());
}

#[test]
fn test_req_list_orphans_filter() {
    let tmp = setup_test_project();
    // Create requirements without any links (orphans)
    create_test_requirement(&tmp, "Orphan Req", "input");

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--orphans"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Orphan Req"));
}

#[test]
fn test_req_list_sort_by_title() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Zebra Requirement", "input");
    create_test_requirement(&tmp, "Apple Requirement", "input");

    let output = pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--sort", "title"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);
    let apple_pos = output_str.find("Apple Requirement").expect("Apple Requirement not found");
    let zebra_pos = output_str.find("Zebra Requirement").expect("Zebra Requirement not found");
    assert!(apple_pos < zebra_pos, "Apple should come before Zebra when sorted by title");
}

#[test]
fn test_req_list_sort_reverse() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Zebra Requirement", "input");
    create_test_requirement(&tmp, "Apple Requirement", "input");

    let output = pdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--sort", "title", "--reverse"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);
    let apple_pos = output_str.find("Apple Requirement").expect("Apple Requirement not found");
    let zebra_pos = output_str.find("Zebra Requirement").expect("Zebra Requirement not found");
    assert!(zebra_pos < apple_pos, "Zebra should come before Apple when sorted by title reversed");
}

// ============================================================================
// Risk Command Tests
// ============================================================================

#[test]
fn test_risk_new_creates_file() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["risk", "new", "--title", "Test Risk", "--type", "design", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created risk"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("risks/design"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_risk_new_with_fmea_ratings() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args([
            "risk", "new",
            "--title", "FMEA Risk",
            "--severity", "8",
            "--occurrence", "4",
            "--detection", "3",
            "--no-edit"
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RPN: 96"));  // 8 * 4 * 3 = 96
}

#[test]
fn test_risk_list_empty_project() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["risk", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No risks found"));
}

#[test]
fn test_risk_list_shows_risks() {
    let tmp = setup_test_project();
    create_test_risk(&tmp, "Design Risk", "design");
    create_test_risk(&tmp, "Process Risk", "process");

    pdt()
        .current_dir(tmp.path())
        .args(["risk", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Risk"))
        .stdout(predicate::str::contains("Process Risk"))
        .stdout(predicate::str::contains("2 risk(s) found"));
}

#[test]
fn test_risk_show_by_short_id() {
    let tmp = setup_test_project();
    create_test_risk(&tmp, "Thermal Risk", "design");

    // Generate short IDs
    pdt()
        .current_dir(tmp.path())
        .args(["risk", "list"])
        .assert()
        .success();

    pdt()
        .current_dir(tmp.path())
        .args(["risk", "show", "@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Thermal Risk"));
}

// ============================================================================
// Validation Command Tests
// ============================================================================

#[test]
fn test_validate_empty_project() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .success();
}

#[test]
fn test_validate_valid_requirement() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Valid Req", "input");

    pdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("passed"));
}

#[test]
fn test_validate_invalid_yaml_syntax() {
    let tmp = setup_test_project();

    // Create a file with invalid YAML
    let invalid_path = tmp.path().join("requirements/inputs/REQ-INVALID.pdt.yaml");
    fs::write(&invalid_path, "id: REQ-123\n  bad indent: true").unwrap();

    pdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .failure();
}

#[test]
fn test_validate_invalid_schema() {
    let tmp = setup_test_project();

    // Create a file with valid YAML but invalid schema
    let invalid_path = tmp.path().join("requirements/inputs/REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD.pdt.yaml");
    fs::write(&invalid_path, r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
title: "Test"
text: "Test text"
status: invalid_status
priority: medium
created: 2024-01-01T00:00:00Z
author: test
"#).unwrap();

    // Error details go to stdout in our validation output
    pdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .failure()
        .stdout(predicate::str::contains("status").or(predicate::str::contains("invalid")));
}

// ============================================================================
// Link Command Tests
// ============================================================================

#[test]
fn test_link_check_empty_project() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["link", "check"])
        .assert()
        .success();
}

// ============================================================================
// Trace Command Tests
// ============================================================================

#[test]
fn test_trace_coverage_empty_project() {
    let tmp = setup_test_project();

    pdt()
        .current_dir(tmp.path())
        .args(["trace", "coverage"])
        .assert()
        .success();
}

// ============================================================================
// Cross-Command Integration Tests
// ============================================================================

#[test]
fn test_full_workflow() {
    let tmp = setup_test_project();

    // Create input requirement
    pdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Temperature Range", "--type", "input", "--no-edit"])
        .assert()
        .success();

    // Create output requirement
    pdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Thermal Design", "--type", "output", "--no-edit"])
        .assert()
        .success();

    // Create risk
    pdt()
        .current_dir(tmp.path())
        .args(["risk", "new", "--title", "Overheating", "--no-edit"])
        .assert()
        .success();

    // List all
    pdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 requirement(s)"));

    // Validate
    pdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .success();
}

#[test]
fn test_not_in_project_fails() {
    let tmp = TempDir::new().unwrap();

    pdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a PDT project"));
}
