//! Integration tests for TDT CLI
//!
//! These tests exercise the CLI commands end-to-end using assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper to get a tdt command
fn tdt() -> Command {
    Command::cargo_bin("tdt").unwrap()
}

/// Helper to create a test project in a temp directory
fn setup_test_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    tdt()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success();
    tmp
}

/// Helper to create a test requirement
fn create_test_requirement(tmp: &TempDir, title: &str, req_type: &str) -> String {
    let output = tdt()
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
    let output = tdt()
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
    tdt()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("engineering artifacts"));
}

#[test]
fn test_version_displays() {
    tdt()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("tdt"));
}

#[test]
fn test_unknown_command_fails() {
    tdt()
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

    tdt()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized"));

    // Verify structure
    assert!(tmp.path().join(".tdt").exists());
    assert!(tmp.path().join(".tdt/config.yaml").exists());
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
    tdt()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn test_init_force_overwrites() {
    let tmp = setup_test_project();

    tdt()
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

    tdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Test Requirement", "--type", "input", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created requirement"));

    // Verify file was created
    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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

    tdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Output Spec", "--type", "output", "--no-edit"])
        .assert()
        .success();

    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/outputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_req_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
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

    tdt()
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

    tdt()
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

    tdt()
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
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Then show by prefixed short ID (REQ@1 format)
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Req"));
}

#[test]
fn test_req_show_not_found() {
    let tmp = setup_test_project();

    tdt()
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

    tdt()
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

    tdt()
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
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--type", "input"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Input Req"))
        .stdout(predicate::str::contains("1 requirement(s) found"));

    // Filter by output type
    tdt()
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

    tdt()
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

    tdt()
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

    let output = tdt()
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

    tdt()
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

    let output = tdt()
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

    let output = tdt()
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

    tdt()
        .current_dir(tmp.path())
        .args(["risk", "new", "--title", "Test Risk", "--type", "design", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created risk"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("risks/design"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_risk_new_with_fmea_ratings() {
    let tmp = setup_test_project();

    tdt()
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

    tdt()
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

    tdt()
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
    tdt()
        .current_dir(tmp.path())
        .args(["risk", "list"])
        .assert()
        .success();

    // Show by prefixed short ID (RISK@1 format)
    tdt()
        .current_dir(tmp.path())
        .args(["risk", "show", "RISK@1"])
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

    tdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .success();
}

#[test]
fn test_validate_valid_requirement() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Valid Req", "input");

    tdt()
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
    let invalid_path = tmp.path().join("requirements/inputs/REQ-INVALID.tdt.yaml");
    fs::write(&invalid_path, "id: REQ-123\n  bad indent: true").unwrap();

    tdt()
        .current_dir(tmp.path())
        .arg("validate")
        .assert()
        .failure();
}

#[test]
fn test_validate_invalid_schema() {
    let tmp = setup_test_project();

    // Create a file with valid YAML but invalid schema
    let invalid_path = tmp.path().join("requirements/inputs/REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD.tdt.yaml");
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
    tdt()
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

    tdt()
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

    tdt()
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
    tdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Temperature Range", "--type", "input", "--no-edit"])
        .assert()
        .success();

    // Create output requirement
    tdt()
        .current_dir(tmp.path())
        .args(["req", "new", "--title", "Thermal Design", "--type", "output", "--no-edit"])
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

#[test]
fn test_not_in_project_fails() {
    let tmp = TempDir::new().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a TDT project"));
}

// ============================================================================
// Component Command Tests
// ============================================================================

/// Helper to create a test component
fn create_test_component(tmp: &TempDir, part_number: &str, title: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "cmp", "new",
            "--part-number", part_number,
            "--title", title,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("CMP-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("CMP-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

#[test]
fn test_cmp_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "--part-number", "PN-001", "--title", "Test Component", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created component"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("bom/components"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1, "Expected exactly one component file");

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("PN-001"));
    assert!(content.contains("Test Component"));
}

#[test]
fn test_cmp_new_with_make_buy() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp", "new",
            "--part-number", "PN-MAKE-001",
            "--title", "In-house Part",
            "--make-buy", "make",
            "--no-edit",
        ])
        .assert()
        .success();

    let files: Vec<_> = fs::read_dir(tmp.path().join("bom/components"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("make_buy: make"));
}

#[test]
fn test_cmp_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No components found"));
}

#[test]
fn test_cmp_list_shows_components() {
    let tmp = setup_test_project();
    create_test_component(&tmp, "PN-001", "First Component");
    create_test_component(&tmp, "PN-002", "Second Component");

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First Component"))
        .stdout(predicate::str::contains("Second Component"))
        .stdout(predicate::str::contains("2 component(s) found"));
}

#[test]
fn test_cmp_show_by_short_id() {
    let tmp = setup_test_project();
    create_test_component(&tmp, "PN-TEST", "Test Component");

    // Generate short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "show", "CMP@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Component"));
}

#[test]
fn test_cmp_list_filter_by_make_buy() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "--part-number", "PN-MAKE", "--title", "Made Part", "--make-buy", "make", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "--part-number", "PN-BUY", "--title", "Bought Part", "--make-buy", "buy", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list", "--make-buy", "make"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 component(s) found"));
}

#[test]
fn test_cmp_list_json_format() {
    let tmp = setup_test_project();
    create_test_component(&tmp, "PN-JSON", "JSON Component");

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list", "-f", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("\"part_number\""));
}

// ============================================================================
// Supplier Command Tests
// ============================================================================

/// Helper to create a test supplier
fn create_test_supplier(tmp: &TempDir, name: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args(["sup", "new", "--name", name, "--no-edit"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("SUP-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("SUP-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

#[test]
fn test_sup_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["sup", "new", "--name", "Acme Corp", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created supplier"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("bom/suppliers"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Acme Corp"));
}

#[test]
fn test_sup_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["sup", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No suppliers found"));
}

#[test]
fn test_sup_list_shows_suppliers() {
    let tmp = setup_test_project();
    create_test_supplier(&tmp, "Supplier One");
    create_test_supplier(&tmp, "Supplier Two");

    tdt()
        .current_dir(tmp.path())
        .args(["sup", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Supplier One"))
        .stdout(predicate::str::contains("Supplier Two"))
        .stdout(predicate::str::contains("2 supplier(s) found"));
}

#[test]
fn test_sup_show_by_short_id() {
    let tmp = setup_test_project();
    create_test_supplier(&tmp, "Test Supplier");

    tdt()
        .current_dir(tmp.path())
        .args(["sup", "list"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["sup", "show", "SUP@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Supplier"));
}

// ============================================================================
// Quote Command Tests
// ============================================================================

#[test]
fn test_quote_new_creates_file() {
    let tmp = setup_test_project();

    // Create prerequisite component and supplier
    create_test_component(&tmp, "PN-QUOTE", "Quoted Component");
    create_test_supplier(&tmp, "Quote Supplier");

    // Generate short IDs
    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();
    tdt().current_dir(tmp.path()).args(["sup", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "quote", "new",
            "--component", "CMP@1",
            "--supplier", "SUP@1",
            "--title", "Test Quote",
            "--price", "10.50",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created quote"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("bom/quotes"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_quote_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No quotes found"));
}

#[test]
fn test_quote_list_shows_quotes() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-Q1", "Component 1");
    create_test_supplier(&tmp, "Supplier 1");

    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();
    tdt().current_dir(tmp.path()).args(["sup", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "quote", "new",
            "--component", "CMP@1",
            "--supplier", "SUP@1",
            "--price", "25.00",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 quote(s) found"));
}

// ============================================================================
// Feature Command Tests
// ============================================================================

/// Helper to create a test feature
fn create_test_feature(tmp: &TempDir, component_short_id: &str, feature_type: &str, title: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "feat", "new",
            "--component", component_short_id,
            "--feature-type", feature_type,
            "--title", title,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("FEAT-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("FEAT-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

#[test]
fn test_feat_new_creates_file() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-FEAT", "Feature Component");
    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "feat", "new",
            "--component", "CMP@1",
            "--feature-type", "internal",
            "--title", "Mounting Hole",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created feature"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("tolerances/features"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Mounting Hole"));
    assert!(content.contains("feature_type: internal"));
}

#[test]
fn test_feat_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No features found"));
}

#[test]
fn test_feat_list_shows_features() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-F", "Feature Component");
    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();

    create_test_feature(&tmp, "CMP@1", "internal", "Hole Feature");
    create_test_feature(&tmp, "CMP@1", "external", "Pin Feature");

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hole Feature"))
        .stdout(predicate::str::contains("Pin Feature"))
        .stdout(predicate::str::contains("2 feature(s) found"));
}

#[test]
fn test_feat_show_by_short_id() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-FS", "Feature Show Component");
    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();
    create_test_feature(&tmp, "CMP@1", "internal", "Test Slot");
    tdt().current_dir(tmp.path()).args(["feat", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "show", "FEAT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Slot"));
}

// ============================================================================
// Mate Command Tests
// ============================================================================

#[test]
fn test_mate_new_creates_file() {
    let tmp = setup_test_project();

    // Create two components with features
    create_test_component(&tmp, "PN-HOLE", "Hole Component");
    create_test_component(&tmp, "PN-PIN", "Pin Component");
    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();

    create_test_feature(&tmp, "CMP@1", "internal", "Mounting Hole");
    create_test_feature(&tmp, "CMP@2", "external", "Mounting Pin");
    tdt().current_dir(tmp.path()).args(["feat", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "mate", "new",
            "--feature-a", "FEAT@1",
            "--feature-b", "FEAT@2",
            "--title", "Pin-Hole Mate",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created mate"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("tolerances/mates"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_mate_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["mate", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No mates found"));
}

#[test]
fn test_mate_list_shows_mates() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-M1", "Component 1");
    create_test_component(&tmp, "PN-M2", "Component 2");
    tdt().current_dir(tmp.path()).args(["cmp", "list"]).output().unwrap();

    create_test_feature(&tmp, "CMP@1", "internal", "Hole A");
    create_test_feature(&tmp, "CMP@2", "external", "Pin A");
    tdt().current_dir(tmp.path()).args(["feat", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "mate", "new",
            "--feature-a", "FEAT@1",
            "--feature-b", "FEAT@2",
            "--title", "Test Mate",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["mate", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Mate"))
        .stdout(predicate::str::contains("1 mate(s) found"));
}

// ============================================================================
// Tolerance Stackup Command Tests
// ============================================================================

#[test]
fn test_tol_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "tol", "new",
            "--title", "Gap Analysis",
            "--target-name", "Air Gap",
            "--target-nominal", "2.0",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created stackup"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("tolerances/stackups"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Gap Analysis"));
    assert!(content.contains("Air Gap"));
}

#[test]
fn test_tol_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No stackups found"));
}

#[test]
fn test_tol_list_shows_stackups() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "new", "--title", "Stackup One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "new", "--title", "Stackup Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Stackup One"))
        .stdout(predicate::str::contains("Stackup Two"))
        .stdout(predicate::str::contains("2 stackup(s) found"));
}

#[test]
fn test_tol_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "new", "--title", "Show Stackup", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["tol", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "show", "TOL@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show Stackup"));
}

// ============================================================================
// Test Protocol Command Tests
// ============================================================================

/// Helper to create a test protocol
fn create_test_protocol(tmp: &TempDir, title: &str, test_type: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "test", "new",
            "--title", title,
            "--type", test_type,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("TEST-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("TEST-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

#[test]
fn test_test_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "test", "new",
            "--title", "Temperature Test",
            "--type", "verification",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created test"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("verification/protocols"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Temperature Test"));
}

#[test]
fn test_test_new_validation_type() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "test", "new",
            "--title", "User Acceptance Test",
            "--type", "validation",
            "--no-edit",
        ])
        .assert()
        .success();

    let files: Vec<_> = fs::read_dir(tmp.path().join("validation/protocols"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_test_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tests found"));
}

#[test]
fn test_test_list_shows_tests() {
    let tmp = setup_test_project();
    create_test_protocol(&tmp, "Test One", "verification");
    create_test_protocol(&tmp, "Test Two", "verification");

    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test One"))
        .stdout(predicate::str::contains("Test Two"))
        .stdout(predicate::str::contains("2 test(s) found"));
}

#[test]
fn test_test_show_by_short_id() {
    let tmp = setup_test_project();
    create_test_protocol(&tmp, "Show Test", "verification");

    tdt().current_dir(tmp.path()).args(["test", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["test", "show", "TEST@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show Test"));
}

// ============================================================================
// Test Result Command Tests
// ============================================================================

#[test]
fn test_rslt_new_creates_file() {
    let tmp = setup_test_project();

    // Create prerequisite test protocol
    create_test_protocol(&tmp, "Protocol for Result", "verification");
    tdt().current_dir(tmp.path()).args(["test", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "rslt", "new",
            "--test", "TEST@1",
            "--verdict", "pass",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created result"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("verification/results"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_rslt_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["rslt", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No results found"));
}

#[test]
fn test_rslt_list_shows_results() {
    let tmp = setup_test_project();

    create_test_protocol(&tmp, "Test Protocol", "verification");
    tdt().current_dir(tmp.path()).args(["test", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["rslt", "new", "--test", "TEST@1", "--verdict", "pass", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["rslt", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 result(s) found"));
}

// ============================================================================
// Manufacturing Process Command Tests
// ============================================================================

#[test]
fn test_proc_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "proc", "new",
            "--title", "CNC Milling",
            "--type", "machining",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created process"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("manufacturing/processes"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("CNC Milling"));
}

#[test]
fn test_proc_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No processes found"));
}

#[test]
fn test_proc_list_shows_processes() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Process One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Process Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Process One"))
        .stdout(predicate::str::contains("Process Two"))
        .stdout(predicate::str::contains("2 process(es) found"));
}

#[test]
fn test_proc_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Show Process", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["proc", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "show", "PROC@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show Process"));
}

// ============================================================================
// Control Plan Command Tests
// ============================================================================

#[test]
fn test_ctrl_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "ctrl", "new",
            "--title", "Diameter Check",
            "--type", "inspection",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created control"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("manufacturing/controls"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Diameter Check"));
}

#[test]
fn test_ctrl_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No controls found"));
}

#[test]
fn test_ctrl_list_shows_controls() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "new", "--title", "Control One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "new", "--title", "Control Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Control One"))
        .stdout(predicate::str::contains("Control Two"))
        .stdout(predicate::str::contains("2 control(s) found"));
}

#[test]
fn test_ctrl_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "new", "--title", "Show Control", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["ctrl", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "show", "CTRL@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show Control"));
}

// ============================================================================
// NCR Command Tests
// ============================================================================

#[test]
fn test_ncr_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "ncr", "new",
            "--title", "Dimension Out of Spec",
            "--type", "internal",
            "--severity", "minor",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created NCR"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("manufacturing/ncrs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Dimension Out of Spec"));
}

#[test]
fn test_ncr_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No NCRs found"));
}

#[test]
fn test_ncr_list_shows_ncrs() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "new", "--title", "NCR One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "new", "--title", "NCR Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("NCR One"))
        .stdout(predicate::str::contains("NCR Two"))
        .stdout(predicate::str::contains("2 NCR(s) found"));
}

#[test]
fn test_ncr_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "new", "--title", "Show NCR", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["ncr", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "show", "NCR@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show NCR"));
}

// ============================================================================
// CAPA Command Tests
// ============================================================================

#[test]
fn test_capa_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "capa", "new",
            "--title", "Improve Inspection Process",
            "--type", "corrective",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created CAPA"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("manufacturing/capas"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Improve Inspection Process"));
}

#[test]
fn test_capa_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No CAPAs found"));
}

#[test]
fn test_capa_list_shows_capas() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "new", "--title", "CAPA One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "new", "--title", "CAPA Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CAPA One"))
        .stdout(predicate::str::contains("CAPA Two"))
        .stdout(predicate::str::contains("2 CAPA(s) found"));
}

#[test]
fn test_capa_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "new", "--title", "Show CAPA", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["capa", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "show", "CAPA@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show CAPA"));
}

// ============================================================================
// Work Instruction Command Tests
// ============================================================================

#[test]
fn test_work_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "work", "new",
            "--title", "Lathe Setup Procedure",
            "--doc-number", "WI-001",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created work instruction"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("manufacturing/work_instructions"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Lathe Setup Procedure"));
}

#[test]
fn test_work_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["work", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No work instructions found"));
}

#[test]
fn test_work_list_shows_work_instructions() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["work", "new", "--title", "Work One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["work", "new", "--title", "Work Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["work", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Work One"))
        .stdout(predicate::str::contains("Work Two"))
        .stdout(predicate::str::contains("2 work instruction(s) found"));
}

#[test]
fn test_work_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["work", "new", "--title", "Show Work", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["work", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["work", "show", "WORK@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show Work"));
}

// ============================================================================
// Assembly Command Tests
// ============================================================================

#[test]
fn test_asm_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "asm", "new",
            "--part-number", "ASM-001",
            "--title", "Main Assembly",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created assembly"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("bom/assemblies"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("Main Assembly"));
    assert!(content.contains("ASM-001"));
}

#[test]
fn test_asm_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No assemblies found"));
}

#[test]
fn test_asm_list_shows_assemblies() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "new", "--part-number", "ASM-001", "--title", "Assembly One", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "new", "--part-number", "ASM-002", "--title", "Assembly Two", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Assembly One"))
        .stdout(predicate::str::contains("Assembly Two"))
        .stdout(predicate::str::contains("2 assembl"));
}

#[test]
fn test_asm_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "new", "--part-number", "ASM-SHOW", "--title", "Show Assembly", "--no-edit"])
        .assert()
        .success();

    tdt().current_dir(tmp.path()).args(["asm", "list"]).output().unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "show", "ASM@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show Assembly"));
}

// ============================================================================
// Global Format Flag Tests
// ============================================================================

#[test]
fn test_global_format_flag_json() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Format Test", "input");

    // Test global -f flag before subcommand
    tdt()
        .current_dir(tmp.path())
        .args(["-f", "json", "req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("["))
        .stdout(predicate::str::contains("\"title\""));
}

#[test]
fn test_global_format_flag_yaml() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "YAML Test", "input");

    tdt()
        .current_dir(tmp.path())
        .args(["-f", "yaml", "req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("title:"));
}

#[test]
fn test_global_format_flag_id() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "ID Test", "input");

    let output = tdt()
        .current_dir(tmp.path())
        .args(["-f", "id", "req", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);
    assert!(output_str.trim().starts_with("REQ-"));
    // Should only have the ID, no other columns
    assert!(!output_str.contains("ID Test"));
}
