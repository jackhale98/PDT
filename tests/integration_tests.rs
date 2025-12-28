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
    tdt().current_dir(tmp.path()).arg("init").assert().success();
    tmp
}

/// Helper to create a test requirement
fn create_test_requirement(tmp: &TempDir, title: &str, req_type: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            title,
            "--type",
            req_type,
            "--no-edit",
        ])
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
        .args([
            "risk",
            "new",
            "--title",
            title,
            "--type",
            risk_type,
            "--no-edit",
        ])
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
        .args([
            "req",
            "new",
            "--title",
            "Test Requirement",
            "--type",
            "input",
            "--no-edit",
        ])
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
        .args([
            "req",
            "new",
            "--title",
            "Output Spec",
            "--type",
            "output",
            "--no-edit",
        ])
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
        .args(["req", "list", "--output", "json"])
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
        .args(["req", "list", "--output", "csv"])
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
    assert!(
        count_str.trim() == "2",
        "Expected count '2', got '{}'",
        count_str.trim()
    );
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
    let apple_pos = output_str
        .find("Apple Requirement")
        .expect("Apple Requirement not found");
    let zebra_pos = output_str
        .find("Zebra Requirement")
        .expect("Zebra Requirement not found");
    assert!(
        apple_pos < zebra_pos,
        "Apple should come before Zebra when sorted by title"
    );
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
    let apple_pos = output_str
        .find("Apple Requirement")
        .expect("Apple Requirement not found");
    let zebra_pos = output_str
        .find("Zebra Requirement")
        .expect("Zebra Requirement not found");
    assert!(
        zebra_pos < apple_pos,
        "Zebra should come before Apple when sorted by title reversed"
    );
}

#[test]
fn test_req_new_with_level() {
    let tmp = setup_test_project();

    // Create a stakeholder-level requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Stakeholder Need",
            "--type",
            "input",
            "--level",
            "stakeholder",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Stakeholder Need"));

    // Verify the file contains the correct level
    let files: Vec<_> = std::fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("level: stakeholder"));
}

#[test]
fn test_req_list_filter_by_level() {
    let tmp = setup_test_project();

    // Create requirements at different V-model levels
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Stakeholder Need",
            "--type",
            "input",
            "--level",
            "stakeholder",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "System Requirement",
            "--type",
            "input",
            "--level",
            "system",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Component Spec",
            "--type",
            "output",
            "--level",
            "component",
            "--no-edit",
        ])
        .assert()
        .success();

    // Filter by stakeholder level - should only show the stakeholder need
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--level", "stakeholder"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Stakeholder Need"))
        .stdout(predicate::str::contains("System Requirement").not())
        .stdout(predicate::str::contains("Component Spec").not());

    // Filter by component level - should only show the component spec
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--level", "component"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Component Spec"))
        .stdout(predicate::str::contains("Stakeholder Need").not());

    // Filter by all - should show all requirements
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--level", "all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 requirement(s)"));
}

#[test]
fn test_req_level_defaults_to_system() {
    let tmp = setup_test_project();

    // Create a requirement without specifying level
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Default Level Req",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify the file contains the default system level
    let files: Vec<_> = std::fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("level: system"));
}

// ============================================================================
// Risk Command Tests
// ============================================================================

#[test]
fn test_risk_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            "Test Risk",
            "--type",
            "design",
            "--no-edit",
        ])
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
            "risk",
            "new",
            "--title",
            "FMEA Risk",
            "--severity",
            "8",
            "--occurrence",
            "4",
            "--detection",
            "3",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RPN: 96")); // 8 * 4 * 3 = 96
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
    let invalid_path = tmp
        .path()
        .join("requirements/inputs/REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD.tdt.yaml");
    fs::write(
        &invalid_path,
        r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
title: "Test"
text: "Test text"
status: invalid_status
priority: medium
created: 2024-01-01T00:00:00Z
author: test
"#,
    )
    .unwrap();

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
// Cross-Command Integration Tests
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
            "cmp",
            "new",
            "--part-number",
            part_number,
            "--title",
            title,
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
        .args([
            "cmp",
            "new",
            "--part-number",
            "PN-001",
            "--title",
            "Test Component",
            "--no-edit",
        ])
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
            "cmp",
            "new",
            "--part-number",
            "PN-MAKE-001",
            "--title",
            "In-house Part",
            "--make-buy",
            "make",
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
        .args([
            "cmp",
            "new",
            "--part-number",
            "PN-MAKE",
            "--title",
            "Made Part",
            "--make-buy",
            "make",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--part-number",
            "PN-BUY",
            "--title",
            "Bought Part",
            "--make-buy",
            "buy",
            "--no-edit",
        ])
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
        .args(["cmp", "list", "--output", "json"])
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
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();
    tdt()
        .current_dir(tmp.path())
        .args(["sup", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "quote",
            "new",
            "--component",
            "CMP@1",
            "--supplier",
            "SUP@1",
            "--title",
            "Test Quote",
            "--price",
            "10.50",
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

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();
    tdt()
        .current_dir(tmp.path())
        .args(["sup", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "quote",
            "new",
            "--component",
            "CMP@1",
            "--supplier",
            "SUP@1",
            "--price",
            "25.00",
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
fn create_test_feature(
    tmp: &TempDir,
    component_short_id: &str,
    feature_type: &str,
    title: &str,
) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            component_short_id,
            "--feature-type",
            feature_type,
            "--title",
            title,
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
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--feature-type",
            "internal",
            "--title",
            "Mounting Hole",
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
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

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
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();
    create_test_feature(&tmp, "CMP@1", "internal", "Test Slot");
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

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
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    create_test_feature(&tmp, "CMP@1", "internal", "Mounting Hole");
    create_test_feature(&tmp, "CMP@2", "external", "Mounting Pin");
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "mate",
            "new",
            "--feature-a",
            "FEAT@1",
            "--feature-b",
            "FEAT@2",
            "--title",
            "Pin-Hole Mate",
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
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    create_test_feature(&tmp, "CMP@1", "internal", "Hole A");
    create_test_feature(&tmp, "CMP@2", "external", "Pin A");
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "mate",
            "new",
            "--feature-a",
            "FEAT@1",
            "--feature-b",
            "FEAT@2",
            "--title",
            "Test Mate",
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
            "tol",
            "new",
            "--title",
            "Gap Analysis",
            "--target-name",
            "Air Gap",
            "--target-nominal",
            "2.0",
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

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .output()
        .unwrap();

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
            "test",
            "new",
            "--title",
            title,
            "--type",
            test_type,
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
            "test",
            "new",
            "--title",
            "Temperature Test",
            "--type",
            "verification",
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
            "test",
            "new",
            "--title",
            "User Acceptance Test",
            "--type",
            "validation",
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

    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .output()
        .unwrap();

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
    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "rslt",
            "new",
            "--test",
            "TEST@1",
            "--verdict",
            "pass",
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
    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "rslt",
            "new",
            "--test",
            "TEST@1",
            "--verdict",
            "pass",
            "--no-edit",
        ])
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
            "proc",
            "new",
            "--title",
            "CNC Milling",
            "--type",
            "machining",
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
        .stdout(predicate::str::contains("2 process(s) found"));
}

#[test]
fn test_proc_show_by_short_id() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Show Process", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

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
            "ctrl",
            "new",
            "--title",
            "Diameter Check",
            "--type",
            "inspection",
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

    tdt()
        .current_dir(tmp.path())
        .args(["ctrl", "list"])
        .output()
        .unwrap();

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
            "ncr",
            "new",
            "--title",
            "Dimension Out of Spec",
            "--type",
            "internal",
            "--severity",
            "minor",
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

    tdt()
        .current_dir(tmp.path())
        .args(["ncr", "list"])
        .output()
        .unwrap();

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
            "capa",
            "new",
            "--title",
            "Improve Inspection Process",
            "--type",
            "corrective",
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

    tdt()
        .current_dir(tmp.path())
        .args(["capa", "list"])
        .output()
        .unwrap();

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
            "work",
            "new",
            "--title",
            "Lathe Setup Procedure",
            "--doc-number",
            "WI-001",
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

    tdt()
        .current_dir(tmp.path())
        .args(["work", "list"])
        .output()
        .unwrap();

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
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Main Assembly",
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
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Assembly One",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-002",
            "--title",
            "Assembly Two",
            "--no-edit",
        ])
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
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-SHOW",
            "--title",
            "Show Assembly",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

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
        .args(["--output", "json", "req", "list"])
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
        .args(["--output", "yaml", "req", "list"])
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
        .args(["--output", "id", "req", "list"])
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

// ============================================================================
// Cache and Git Collaboration Tests
// ============================================================================
// These tests verify that the SQLite cache works correctly for decentralized
// git collaboration where each user has their own local cache.

#[test]
fn test_cache_is_gitignored() {
    let tmp = setup_test_project();

    // Check that .gitignore includes cache files
    let gitignore_path = tmp.path().join(".gitignore");
    let gitignore_content = fs::read_to_string(&gitignore_path).unwrap();

    assert!(
        gitignore_content.contains("cache.db"),
        ".gitignore should include cache.db"
    );
    assert!(
        gitignore_content.contains("cache.db-journal"),
        ".gitignore should include cache.db-journal"
    );
    assert!(
        gitignore_content.contains("cache.db-wal"),
        ".gitignore should include cache.db-wal"
    );
}

#[test]
fn test_entity_files_contain_full_ids_not_short_ids() {
    let tmp = setup_test_project();

    // Create an entity
    create_test_requirement(&tmp, "Full ID Test", "input");

    // Find the created file
    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();

    // Entity file should contain full ULID-based ID
    assert!(
        content.contains("id: REQ-"),
        "Entity file should have full ID"
    );
    // Entity file should NOT contain short ID syntax
    assert!(
        !content.contains("@1"),
        "Entity file should NOT contain short ID syntax"
    );
    assert!(
        !content.contains("REQ@"),
        "Entity file should NOT contain short ID prefix"
    );
}

#[test]
fn test_cache_rebuild_after_external_changes() {
    let tmp = setup_test_project();

    // Create initial requirement
    create_test_requirement(&tmp, "Initial Req", "input");

    // List to generate short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@1"));

    // Simulate external change (like git pull) by creating a new file directly
    // Use a valid ULID format (26 chars, base32 Crockford)
    let new_req_content = r#"
id: REQ-01HQ5V2KRMJ0B9XYZ3NTWPGQ4E
type: input
title: Externally Added Requirement
text: This requirement was added by another user and pulled via git
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: external_user
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V2KRMJ0B9XYZ3NTWPGQ4E.tdt.yaml"),
        new_req_content,
    )
    .unwrap();

    // List should auto-sync cache and show both requirements with new short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initial Req"))
        .stdout(predicate::str::contains("Externally Added Requirement"))
        .stdout(predicate::str::contains("2 requirement(s) found"));
}

#[test]
fn test_cache_handles_deleted_entities() {
    let tmp = setup_test_project();

    // Create two requirements
    create_test_requirement(&tmp, "Req to Keep", "input");
    create_test_requirement(&tmp, "Req to Delete", "input");

    // List to verify both exist
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 requirement(s) found"));

    // Simulate external deletion (like git pull that removes a file)
    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();

    // Delete the second file
    fs::remove_file(files[1].path()).unwrap();

    // List should auto-sync cache and only show one requirement
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_short_ids_are_local_to_cache() {
    // This test verifies that short IDs are derived from the local cache
    // and are not stored in entity files (important for git collaboration)
    let tmp = setup_test_project();

    // Create requirements
    create_test_requirement(&tmp, "First Req", "input");
    create_test_requirement(&tmp, "Second Req", "input");

    // Get short IDs from list
    let output = tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);
    assert!(output_str.contains("@1"), "Should have short ID @1");
    assert!(output_str.contains("@2"), "Should have short ID @2");

    // Verify the entity files don't contain short IDs
    for entry in fs::read_dir(tmp.path().join("requirements/inputs")).unwrap() {
        let entry = entry.unwrap();
        let content = fs::read_to_string(entry.path()).unwrap();
        assert!(
            !content.contains("@1") && !content.contains("@2"),
            "Entity file should not contain short IDs: {}",
            entry.path().display()
        );
    }
}

#[test]
fn test_cache_clear_and_rebuild() {
    let tmp = setup_test_project();

    // Create some entities
    create_test_requirement(&tmp, "Cache Test Req", "input");
    create_test_risk(&tmp, "Cache Test Risk", "design");

    // List to populate cache
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["risk", "list"])
        .assert()
        .success();

    // Clear cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "clear"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache cleared"));

    // Verify cache is deleted
    assert!(!tmp.path().join(".tdt/cache.db").exists());

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache rebuilt"));

    // Verify cache works again
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache Test Req"));
}

#[test]
fn test_cache_status_command() {
    let tmp = setup_test_project();

    // Create some entities
    create_test_requirement(&tmp, "Status Test 1", "input");
    create_test_requirement(&tmp, "Status Test 2", "input");
    create_test_risk(&tmp, "Status Test Risk", "design");

    // Rebuild cache to ensure counts are accurate
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Check cache status
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache Status"))
        .stdout(predicate::str::contains("Total entities:"))
        .stdout(predicate::str::contains("3")); // 2 reqs + 1 risk
}

#[test]
fn test_cache_sync_incremental() {
    let tmp = setup_test_project();

    // Create initial entity
    create_test_requirement(&tmp, "Initial Req", "input");

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Add another entity externally
    // Use a valid ULID format (26 chars, base32 Crockford)
    let new_req_content = r#"
id: REQ-01HQ5V3ABCD1234EFGH5678JKM
type: input
title: Sync Test Requirement
text: This requirement was synced from external changes
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: test
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V3ABCD1234EFGH5678JKM.tdt.yaml"),
        new_req_content,
    )
    .unwrap();

    // Sync cache (incremental update)
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "sync"])
        .assert()
        .success()
        .stdout(predicate::str::contains("synced").or(predicate::str::contains("Added")));

    // Verify new entity is accessible
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync Test Requirement"))
        .stdout(predicate::str::contains("2 requirement(s) found"));
}

#[test]
fn test_cache_query_raw_sql() {
    let tmp = setup_test_project();

    // Create entities
    create_test_requirement(&tmp, "Query Test Req", "input");
    create_test_component(&tmp, "PN-QUERY", "Query Test Component");

    // Rebuild to ensure cache is populated
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Query the cache with SQL
    tdt()
        .current_dir(tmp.path())
        .args([
            "cache",
            "query",
            "SELECT id, title FROM entities WHERE prefix = 'REQ'",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Query Test Req"));
}

// ============================================================================
// DSM Tests
// ============================================================================

#[test]
fn test_dsm_help() {
    tdt()
        .args(["dsm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

#[test]
fn test_dsm_no_components() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .arg("dsm")
        .assert()
        .success()
        .stdout(predicate::str::contains("No components found"));
}

#[test]
fn test_dsm_with_components() {
    let tmp = setup_test_project();

    // Create two components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Bracket", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM - should show 2 components
    tdt()
        .current_dir(tmp.path())
        .arg("dsm")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 components"));
}

#[test]
fn test_dsm_csv_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with CSV output
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-o", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Component,CMP@1"));
}

#[test]
fn test_dsm_json_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with JSON output
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-o", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"components\""))
        .stdout(predicate::str::contains("\"relationships\""));
}

#[test]
fn test_dsm_clustering() {
    let tmp = setup_test_project();

    // Create two components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Bracket", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with clustering
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-c"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Clustered"))
        .stdout(predicate::str::contains("Cluster"));
}

#[test]
fn test_dsm_rel_type_filter() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with mate filter only
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-t", "mate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

#[test]
fn test_dsm_weighted_flag() {
    let tmp = setup_test_project();

    // Create two components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Shaft", "--no-edit"])
        .assert()
        .success();

    // Create a process that produces both components (creates relationship)
    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "Assembly Process",
            "--type",
            "assembly",
            "--op-number",
            "OP-010",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with weighted flag - should show numeric values
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "--weighted"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

#[test]
fn test_dsm_metrics_flag() {
    let tmp = setup_test_project();

    // Create components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Shaft", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with metrics flag - should show coupling statistics
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "--metrics"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Coupling Metrics"))
        .stdout(predicate::str::contains("Fan-in"))
        .stdout(predicate::str::contains("Fan-out"));
}

#[test]
fn test_dsm_cycles_flag() {
    let tmp = setup_test_project();

    // Create components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Shaft", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with cycles flag
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "--cycles"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

// ============================================================================
// DMM (Domain Mapping Matrix) Tests
// ============================================================================

#[test]
fn test_dmm_help() {
    tdt()
        .args(["dmm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Domain Mapping Matrix"));
}

#[test]
fn test_dmm_cmp_req() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--type",
            "input",
            "-T",
            "Force Requirement",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM for components vs requirements
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "req"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Domain Mapping Matrix"));
}

#[test]
fn test_dmm_cmp_proc() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a process
    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "Machining",
            "--type",
            "machining",
            "--op-number",
            "OP-010",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM for components vs processes
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "proc"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Domain Mapping Matrix"));
}

#[test]
fn test_dmm_csv_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--type",
            "input",
            "-T",
            "Test Req",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM with CSV output
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "req", "-o", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains(",REQ@1"));
}

#[test]
fn test_dmm_json_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--type",
            "input",
            "-T",
            "Test Req",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM with JSON output
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "req", "-o", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"row_entities\""))
        .stdout(predicate::str::contains("\"col_entities\""));
}

// ============================================================================
// Config Command Tests
// ============================================================================

#[test]
fn test_config_show() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Effective Configuration"));
}

#[test]
fn test_config_path() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["config", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Project:"));
}

#[test]
fn test_config_keys() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["config", "keys"])
        .assert()
        .success()
        .stdout(predicate::str::contains("author"))
        .stdout(predicate::str::contains("editor"));
}

#[test]
fn test_config_set_and_show() {
    let tmp = setup_test_project();

    // Set a config value
    tdt()
        .current_dir(tmp.path())
        .args(["config", "set", "author", "Test Author"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set author"));

    // Show just that key
    tdt()
        .current_dir(tmp.path())
        .args(["config", "show", "author"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Author"));
}

#[test]
fn test_config_unset() {
    let tmp = setup_test_project();

    // Set then unset
    tdt()
        .current_dir(tmp.path())
        .args(["config", "set", "author", "Test"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["config", "unset", "author"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed author"));
}

// ============================================================================
// Search Command Tests
// ============================================================================

#[test]
fn test_search_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["search", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No results found"));
}

#[test]
fn test_search_finds_entities() {
    let tmp = setup_test_project();

    // Create some entities
    create_test_requirement(&tmp, "Temperature Range", "input");
    create_test_requirement(&tmp, "Power Consumption", "output");
    create_test_risk(&tmp, "Battery Overheating", "design");

    // Rebuild cache to index the entities
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Search for "Temperature"
    tdt()
        .current_dir(tmp.path())
        .args(["search", "temperature"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Temperature Range"))
        .stdout(predicate::str::contains("1 results"));
}

#[test]
fn test_search_with_type_filter() {
    let tmp = setup_test_project();

    // Create entities of different types with similar names
    create_test_requirement(&tmp, "Safety Requirement", "input");
    create_test_risk(&tmp, "Safety Risk", "design");

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Search with type filter
    tdt()
        .current_dir(tmp.path())
        .args(["search", "safety", "-t", "req"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Safety Requirement"))
        .stdout(predicate::str::contains("1 results"));
}

#[test]
fn test_search_count_only() {
    let tmp = setup_test_project();

    create_test_requirement(&tmp, "Test One", "input");
    create_test_requirement(&tmp, "Test Two", "input");

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Search with count only
    tdt()
        .current_dir(tmp.path())
        .args(["search", "test", "--count"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^2\n$").unwrap());
}

// ============================================================================
// Delete Command Tests
// ============================================================================

#[test]
fn test_req_delete() {
    let tmp = setup_test_project();

    // Create a requirement
    create_test_requirement(&tmp, "Delete Me", "input");

    // List to get short ID
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Delete using short ID
    tdt()
        .current_dir(tmp.path())
        .args(["req", "delete", "REQ@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted"));

    // Verify it's gone
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No requirements found"));
}

#[test]
fn test_req_delete_with_links_blocked() {
    let tmp = setup_test_project();

    // Create a requirement and a test that references it
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Linked Req",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "test",
            "new",
            "--title",
            "Test Protocol",
            "--type",
            "verification",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to get short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .assert()
        .success();

    // Link the test to the requirement
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "TEST@1", "REQ@1", "-t", "verifies"])
        .assert()
        .success();

    // Rebuild cache to pick up the link
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Try to delete - should fail
    tdt()
        .current_dir(tmp.path())
        .args(["req", "delete", "REQ@1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("referenced by"));
}

#[test]
fn test_req_delete_force() {
    let tmp = setup_test_project();

    // Create linked entities
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Linked Req",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "test",
            "new",
            "--title",
            "Test Protocol",
            "--type",
            "verification",
            "--no-edit",
        ])
        .assert()
        .success();

    // List and link
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["test", "list"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "TEST@1", "REQ@1", "-t", "verifies"])
        .assert()
        .success();

    // Rebuild cache to pick up the link
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Force delete should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["req", "delete", "REQ@1", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted"));
}

// ============================================================================
// Archive Command Tests
// ============================================================================

#[test]
fn test_req_archive() {
    let tmp = setup_test_project();

    // Create a requirement
    create_test_requirement(&tmp, "Archive Me", "input");

    // List to get short ID
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Archive using short ID
    tdt()
        .current_dir(tmp.path())
        .args(["req", "archive", "REQ@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Archived"));

    // Verify it's gone from listing
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No requirements found"));

    // Verify the archive directory exists
    assert!(tmp.path().join(".tdt/archive/requirements/inputs").exists());
}

// ============================================================================
// Status Command Tests
// ============================================================================

#[test]
fn test_status_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Project"));
}

#[test]
fn test_status_shows_counts() {
    let tmp = setup_test_project();

    // Create some entities
    create_test_requirement(&tmp, "Test Req", "input");

    tdt()
        .current_dir(tmp.path())
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("REQ"));
}

// ============================================================================
// Trace Command Tests
// ============================================================================

#[test]
fn test_trace_help() {
    tdt()
        .args(["trace", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("trace"));
}

#[test]
fn test_trace_matrix_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "matrix"])
        .assert()
        .success();
}

#[test]
fn test_trace_orphans_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "orphans"])
        .assert()
        .success();
}

#[test]
fn test_trace_from_requirement() {
    let tmp = setup_test_project();
    let req_id = create_test_requirement(&tmp, "Test Req", "input");

    tdt()
        .current_dir(tmp.path())
        .args(["trace", "from", &req_id])
        .assert()
        .success();
}

#[test]
fn test_trace_to_requirement() {
    let tmp = setup_test_project();
    let req_id = create_test_requirement(&tmp, "Test Req", "input");

    tdt()
        .current_dir(tmp.path())
        .args(["trace", "to", &req_id])
        .assert()
        .success();
}

// ============================================================================
// Lot Command Tests
// ============================================================================

#[test]
fn test_lot_list_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No lots found"));
}

#[test]
fn test_lot_new_creates_file() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Test Lot",
            "--lot-number",
            "LOT-001",
            "--quantity",
            "100",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created lot"));

    // Verify file was created
    let lot_dir = tmp.path().join("manufacturing/lots");
    assert!(lot_dir.exists());
    let files: Vec<_> = fs::read_dir(&lot_dir).unwrap().collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_lot_list_shows_lots() {
    let tmp = setup_test_project();

    // Create a lot
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Test Lot",
            "--lot-number",
            "LOT-001",
            "--quantity",
            "100",
            "--no-edit",
        ])
        .assert()
        .success();

    // List should show it
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Lot"));
}

// ============================================================================
// Dev (Deviation) Command Tests
// ============================================================================

#[test]
fn test_dev_list_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["dev", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No deviations found"));
}

#[test]
fn test_dev_new_creates_file() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args([
            "dev",
            "new",
            "--title",
            "Test Deviation",
            "--dev-type",
            "temporary",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created deviation"));

    // Verify file was created
    let dev_dir = tmp.path().join("manufacturing/deviations");
    assert!(dev_dir.exists());
    let files: Vec<_> = fs::read_dir(&dev_dir).unwrap().collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_dev_list_shows_deviations() {
    let tmp = setup_test_project();

    // Create a deviation
    tdt()
        .current_dir(tmp.path())
        .args([
            "dev",
            "new",
            "--title",
            "Test Deviation",
            "--dev-type",
            "temporary",
            "--no-edit",
        ])
        .assert()
        .success();

    // List should show it
    tdt()
        .current_dir(tmp.path())
        .args(["dev", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Deviation"));
}

// ============================================================================
// Schema Command Tests
// ============================================================================

#[test]
fn test_schema_list() {
    tdt()
        .args(["schema", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("req"));
}

#[test]
fn test_schema_show_req() {
    tdt()
        .args(["schema", "show", "req"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

#[test]
fn test_schema_show_raw_json() {
    tdt()
        .args(["schema", "show", "req", "--raw"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"$schema\""))
        .stdout(predicate::str::contains("\"properties\""));
}

// ============================================================================
// Report Command Tests
// ============================================================================

#[test]
fn test_report_rvm_empty_project() {
    let tmp = setup_test_project();

    // RVM on empty project should succeed with empty output
    tdt()
        .current_dir(tmp.path())
        .args(["report", "rvm"])
        .assert()
        .success();
}

#[test]
fn test_report_rvm_with_requirements() {
    let tmp = setup_test_project();

    // Create a requirement
    create_test_requirement(&tmp, "Test Requirement", "input");

    // RVM should show the requirement
    tdt()
        .current_dir(tmp.path())
        .args(["report", "rvm"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Requirement"));
}

#[test]
fn test_report_rvm_with_linked_test() {
    let tmp = setup_test_project();

    // Create a requirement
    let req_id = create_test_requirement(&tmp, "Linked Requirement", "input");

    // Create a test
    let test_id = create_test_protocol(&tmp, "Verification Test", "verification");

    // Link them
    if !req_id.is_empty() && !test_id.is_empty() {
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success();

        // RVM should show the linked test
        tdt()
            .current_dir(tmp.path())
            .args(["report", "rvm"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Linked Requirement"));
    }
}

#[test]
fn test_report_fmea_empty_project() {
    let tmp = setup_test_project();

    // FMEA on empty project should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["report", "fmea"])
        .assert()
        .success();
}

#[test]
fn test_report_fmea_with_risks() {
    let tmp = setup_test_project();

    // Create a risk
    create_test_risk(&tmp, "Test Risk", "design");

    // FMEA should show the risk (check for total count and risk ID)
    tdt()
        .current_dir(tmp.path())
        .args(["report", "fmea"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total Risks"))
        .stdout(predicate::str::contains("RISK@1").or(predicate::str::contains("RISK-")));
}

#[test]
fn test_report_bom_with_assembly() {
    let tmp = setup_test_project();

    // Create an assembly
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-BOM-001",
            "--title",
            "BOM Test Assembly",
            "--no-edit",
            "--output",
            "id",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let asm_id = stdout.trim();

    if !asm_id.is_empty() && asm_id.starts_with("ASM-") {
        // BOM report for assembly should succeed
        tdt()
            .current_dir(tmp.path())
            .args(["report", "bom", asm_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("BOM Test Assembly"));
    }
}

#[test]
fn test_report_bom_with_components_in_assembly() {
    let tmp = setup_test_project();

    // Create a component
    create_test_component(&tmp, "PART-BOM-001", "BOM Component");

    // Create an assembly
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-BOM-002",
            "--title",
            "Assembly With Parts",
            "--no-edit",
            "--output",
            "id",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let asm_id = stdout.trim();

    if !asm_id.is_empty() && asm_id.starts_with("ASM-") {
        // BOM report should work (even if empty of components)
        tdt()
            .current_dir(tmp.path())
            .args(["report", "bom", asm_id])
            .assert()
            .success();
    }
}

#[test]
fn test_report_test_status_empty() {
    let tmp = setup_test_project();

    // Test status on empty project should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["report", "test-status"])
        .assert()
        .success();
}

#[test]
fn test_report_open_issues_empty() {
    let tmp = setup_test_project();

    // Open issues on empty project should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["report", "open-issues"])
        .assert()
        .success();
}

// ============================================================================
// Link Management Tests
// ============================================================================

#[test]
fn test_link_add_verified_by() {
    let tmp = setup_test_project();

    // Create a requirement and a test
    let req_id = create_test_requirement(&tmp, "Req for Link", "input");
    let test_id = create_test_protocol(&tmp, "Test for Link", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("Added link"));

        // Verify link exists
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("verified_by"));
    }
}

#[test]
fn test_link_add_mitigated_by() {
    let tmp = setup_test_project();

    // Create a risk and a component
    let risk_id = create_test_risk(&tmp, "Risk for Link", "design");
    let cmp_id = create_test_component(&tmp, "PART-002", "Component for Link");

    if !risk_id.is_empty() && !cmp_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &risk_id, "--type", "mitigated_by", &cmp_id])
            .assert()
            .success();

        // Verify link exists
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &risk_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("mitigated_by"));
    }
}

#[test]
fn test_link_remove() {
    let tmp = setup_test_project();

    // Create a requirement and a test
    let req_id = create_test_requirement(&tmp, "Req for Remove", "input");
    let test_id = create_test_protocol(&tmp, "Test for Remove", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success();

        // Remove link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "remove", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("Removed link"));

        // Verify link is gone
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req_id])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("No links")
                    .or(predicate::str::contains("verified_by").not()),
            );
    }
}

#[test]
fn test_link_show_no_links() {
    let tmp = setup_test_project();

    // Create a requirement with no links
    let req_id = create_test_requirement(&tmp, "No Links Req", "input");

    if !req_id.is_empty() {
        // Show should indicate no links
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req_id])
            .assert()
            .success();
    }
}

#[test]
fn test_link_bidirectional() {
    let tmp = setup_test_project();

    // Create two requirements
    let req1_id = create_test_requirement(&tmp, "Parent Req", "input");
    let req2_id = create_test_requirement(&tmp, "Child Req", "output");

    if !req1_id.is_empty() && !req2_id.is_empty() {
        // Add derives_from link (child derives from parent)
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req2_id, "--type", "derives_from", &req1_id])
            .assert()
            .success();

        // Check child has derives_from
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req2_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("derives_from"));
    }
}

// ============================================================================
// Trace Command Tests (Additional)
// ============================================================================

#[test]
fn test_trace_from_with_linked_entities() {
    let tmp = setup_test_project();

    // Create linked entities
    let req_id = create_test_requirement(&tmp, "Trace Source", "input");
    let test_id = create_test_protocol(&tmp, "Trace Target", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success();

        // Trace from requirement
        tdt()
            .current_dir(tmp.path())
            .args(["trace", "from", &req_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("Trace Source"));
    }
}

#[test]
fn test_trace_to_unlinked_entity() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Trace To Target", "input");

    if !req_id.is_empty() {
        // Trace to requirement (shows what points to it)
        tdt()
            .current_dir(tmp.path())
            .args(["trace", "to", &req_id])
            .assert()
            .success();
    }
}

#[test]
fn test_trace_orphans_with_requirements() {
    let tmp = setup_test_project();

    // Create some requirements (they will be orphaned since not linked)
    create_test_requirement(&tmp, "Orphan Req 1", "input");
    create_test_requirement(&tmp, "Orphan Req 2", "input");

    // Check orphans - unlinked requirements should appear
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "orphans"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Orphan Req 1"))
        .stdout(predicate::str::contains("Orphan Req 2"));
}

// ============================================================================
// Workflow Command Tests
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
// Where-Used Command Tests
// ============================================================================

#[test]
fn test_where_used_component() {
    let tmp = setup_test_project();

    let cmp_id = create_test_component(&tmp, "PART-003", "Where Used Test");

    if !cmp_id.is_empty() {
        // Where-used on component
        tdt()
            .current_dir(tmp.path())
            .args(["where-used", &cmp_id])
            .assert()
            .success();
    }
}

#[test]
fn test_where_used_no_references() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Orphan Req", "input");

    if !req_id.is_empty() {
        // Where-used on unreferenced entity
        tdt()
            .current_dir(tmp.path())
            .args(["where-used", &req_id])
            .assert()
            .success();
    }
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
// Error Handling Tests
// ============================================================================

#[test]
fn test_error_invalid_entity_id_format() {
    let tmp = setup_test_project();

    // Invalid ID format should fail
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "INVALID-ID"])
        .assert()
        .failure();
}

#[test]
fn test_error_nonexistent_short_id() {
    let tmp = setup_test_project();

    // Non-existent short ID should fail
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ@999"])
        .assert()
        .failure();
}

#[test]
fn test_error_invalid_format_option() {
    let tmp = setup_test_project();

    // Invalid format option should fail
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list", "--output", "invalid_format"])
        .assert()
        .failure();
}

#[test]
fn test_error_missing_required_argument_cmp() {
    let tmp = setup_test_project();

    // Component requires part-number in non-interactive mode
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "--title", "Missing PN", "--no-edit"])
        .assert()
        .failure();
}

#[test]
fn test_error_invalid_risk_severity_value() {
    let tmp = setup_test_project();

    // FMEA ratings must be positive integers - negative should fail
    tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            "Bad Severity",
            "--severity=-1",
            "--no-edit",
        ])
        .assert()
        .failure();
}

#[test]
fn test_error_invalid_link_type() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Link Test Req", "input");
    let test_id = create_test_protocol(&tmp, "Link Test Protocol", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Invalid link type should fail
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "invalid_type", &test_id])
            .assert()
            .failure();
    }
}

#[test]
fn test_error_link_entity_not_found() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Link Source Req", "input");

    if !req_id.is_empty() {
        // Link to non-existent entity should fail
        tdt()
            .current_dir(tmp.path())
            .args([
                "link",
                "add",
                &req_id,
                "--type",
                "verified_by",
                "TEST-NONEXISTENT",
            ])
            .assert()
            .failure();
    }
}

#[test]
fn test_error_validate_broken_link() {
    let tmp = setup_test_project();

    // Create a requirement file with a broken link
    let broken_req_content = r#"
id: REQ-01HQ5V4ABCD1234EFGH5678JKL
type: input
title: Broken Link Requirement
text: This requirement has a broken link
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: test
links:
  verified_by:
    - TEST-NONEXISTENT
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V4ABCD1234EFGH5678JKL.tdt.yaml"),
        broken_req_content,
    )
    .unwrap();

    // Validate should report broken links
    tdt()
        .current_dir(tmp.path())
        .args(["link", "check"])
        .assert()
        .failure();
}

#[test]
fn test_error_trace_from_nonexistent_entity() {
    let tmp = setup_test_project();

    // Trace from non-existent entity should fail
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "from", "REQ-NONEXISTENT"])
        .assert()
        .failure();
}

#[test]
fn test_error_duplicate_lot_number() {
    let tmp = setup_test_project();

    // Create first lot
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "First Lot",
            "--lot-number",
            "LOT-DUP",
            "--quantity",
            "100",
            "--no-edit",
        ])
        .assert()
        .success();

    // Second lot with same number should still succeed (we allow duplicates
    // as different entity files)
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Second Lot",
            "--lot-number",
            "LOT-DUP",
            "--quantity",
            "50",
            "--no-edit",
        ])
        .assert()
        .success();
}

// ============================================================================
// Edge Case Tests - Unicode and Special Characters
// ============================================================================

#[test]
fn test_edge_case_unicode_title() {
    let tmp = setup_test_project();

    // Create requirement with Unicode title
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Température: 100°C ± 5°C",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created requirement"));

    // Verify it can be listed
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Température"));
}

#[test]
fn test_edge_case_japanese_title() {
    let tmp = setup_test_project();

    // Create requirement with Japanese characters
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "温度要件 - Temperature Requirement",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it can be listed
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("温度要件"));
}

#[test]
fn test_edge_case_emoji_in_title() {
    let tmp = setup_test_project();

    // Create requirement with emoji
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Safety Check ⚠️ Warning",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify search works
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Safety Check"));
}

#[test]
fn test_edge_case_special_characters_in_title() {
    let tmp = setup_test_project();

    // Create requirement with special characters (avoiding nested quotes for YAML safety)
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Test: <angled> & (parens) [brackets] #hash",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify file was created and content is valid YAML
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();
}

#[test]
fn test_edge_case_very_long_title() {
    let tmp = setup_test_project();

    // Create requirement with long title (200 chars)
    let long_title = "A".repeat(200);
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            &long_title,
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Should be in list (potentially truncated)
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_edge_case_whitespace_in_title() {
    let tmp = setup_test_project();

    // Create requirement with extra whitespace (should be handled)
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "  Whitespace  Title  ",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_edge_case_multiline_text_in_file() {
    let tmp = setup_test_project();

    // Create a requirement file with multiline text
    let multiline_req = r#"
id: REQ-01HQ5V5ABCD1234EFGH5678JKM
type: input
title: Multiline Test
text: |
  This is a multiline text block.
  It spans multiple lines.
  And has various content:
    - Indented items
    - More items
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: test
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V5ABCD1234EFGH5678JKM.tdt.yaml"),
        multiline_req,
    )
    .unwrap();

    // Validate should pass
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();

    // Show should display the content
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ-01HQ5V5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Multiline Test"));
}

#[test]
fn test_edge_case_numeric_string_part_number() {
    let tmp = setup_test_project();

    // Create component with numeric-looking part number
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--part-number",
            "12345",
            "--title",
            "Numeric PN Component",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it can be found
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("12345"));
}

#[test]
fn test_edge_case_zero_quantity_lot() {
    let tmp = setup_test_project();

    // Create lot with zero quantity
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Zero Qty Lot",
            "--lot-number",
            "LOT-ZERO",
            "--quantity",
            "0",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Zero Qty Lot"));
}

#[test]
fn test_edge_case_large_quantity_lot() {
    let tmp = setup_test_project();

    // Create lot with large quantity
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Large Qty Lot",
            "--lot-number",
            "LOT-LARGE",
            "--quantity",
            "999999999",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Large Qty Lot"));
}

#[test]
fn test_edge_case_fmea_boundary_values() {
    let tmp = setup_test_project();

    // Test minimum FMEA values (1)
    tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            "Min FMEA Risk",
            "--severity",
            "1",
            "--occurrence",
            "1",
            "--detection",
            "1",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RPN: 1")); // 1 * 1 * 1 = 1

    // Test maximum FMEA values (10)
    tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            "Max FMEA Risk",
            "--severity",
            "10",
            "--occurrence",
            "10",
            "--detection",
            "10",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RPN: 1000")); // 10 * 10 * 10 = 1000
}

#[test]
fn test_edge_case_tolerance_stackup_zero_nominal() {
    let tmp = setup_test_project();

    // Create tolerance stackup with zero nominal
    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "Zero Nominal Stackup",
            "--target-name",
            "Zero Gap",
            "--target-nominal",
            "0.0",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Zero Nominal Stackup"));
}

#[test]
fn test_edge_case_negative_tolerance_value() {
    let tmp = setup_test_project();

    // Create tolerance stackup with negative nominal (allowed for offset checks)
    // Using = syntax to prevent negative value being parsed as flag
    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "Negative Nominal Stackup",
            "--target-name",
            "Offset Check",
            "--target-nominal=-0.5",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Negative Nominal Stackup"));
}

// ============================================================================
// Workflow Enhancement Tests
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

// ============================================================================
// Hazard Tests
// ============================================================================

#[test]
fn test_haz_list_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No hazards found"));
}

#[test]
fn test_haz_new_creates_file() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "High Voltage Exposure",
            "--category",
            "electrical",
            "--description",
            "300V DC exposed terminals",
            "--severity",
            "severe",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created hazard"));

    let files: Vec<_> = fs::read_dir(tmp.path().join("risks/hazards"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("High Voltage Exposure"));
    assert!(content.contains("electrical"));
    assert!(content.contains("severe"));
}

#[test]
fn test_haz_list_shows_hazards() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "Crush Hazard",
            "--category",
            "mechanical",
            "--description",
            "Moving parts can crush fingers",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Crush Hazard"))
        .stdout(predicate::str::contains("mechanical"));
}

#[test]
fn test_haz_show_by_short_id() {
    let tmp = setup_test_project();

    // Create a hazard
    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "Thermal Burns",
            "--category",
            "thermal",
            "--description",
            "Hot surfaces above 70C",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to assign short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list"])
        .assert()
        .success();

    // Show by short ID
    tdt()
        .current_dir(tmp.path())
        .args(["haz", "show", "HAZ@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Thermal Burns"))
        .stdout(predicate::str::contains("thermal"));
}

#[test]
fn test_haz_list_filter_by_category() {
    let tmp = setup_test_project();

    // Create electrical hazard
    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "Shock Hazard",
            "--category",
            "electrical",
            "--description",
            "Electrical shock risk",
            "--no-edit",
        ])
        .assert()
        .success();

    // Create mechanical hazard
    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "Pinch Point",
            "--category",
            "mechanical",
            "--description",
            "Finger pinch risk",
            "--no-edit",
        ])
        .assert()
        .success();

    // Filter by electrical
    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list", "--category", "electrical"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Shock Hazard"))
        .stdout(predicate::str::contains("Pinch Point").not());

    // Filter by mechanical
    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list", "--category", "mechanical"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pinch Point"))
        .stdout(predicate::str::contains("Shock Hazard").not());
}

#[test]
fn test_haz_list_no_risks_filter() {
    let tmp = setup_test_project();

    // Create hazard without linked risks
    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "Unlinked Hazard",
            "--category",
            "chemical",
            "--description",
            "Chemical exposure risk",
            "--no-edit",
        ])
        .assert()
        .success();

    // List with --no-risks should show the hazard
    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list", "--no-risks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Unlinked Hazard"));
}

#[test]
fn test_haz_json_output() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            "Test Hazard",
            "--category",
            "software",
            "--description",
            "Software malfunction risk",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["haz", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"Test Hazard\""))
        .stdout(predicate::str::contains("\"category\": \"software\""));
}

// ============================================================================
// Schema Tests
// ============================================================================

#[test]
fn test_schema_list_includes_haz() {
    tdt()
        .args(["schema", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("haz"))
        .stdout(predicate::str::contains("Hazard"));
}

#[test]
fn test_schema_show_haz() {
    tdt()
        .args(["schema", "show", "haz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hazard"))
        .stdout(predicate::str::contains("category"))
        .stdout(predicate::str::contains("severity"))
        .stdout(predicate::str::contains("electrical, mechanical, thermal"));
}

// ============================================================================
// Trace DOT Output Tests
// ============================================================================

#[test]
fn test_trace_from_dot_output() {
    let tmp = setup_test_project();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Test Requirement",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to get short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Test DOT output
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "from", "REQ@1", "--output", "dot"])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph trace_from"))
        .stdout(predicate::str::contains("rankdir=TB"))
        .stdout(predicate::str::contains("node [shape=box]"));
}

#[test]
fn test_trace_to_dot_output() {
    let tmp = setup_test_project();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Test Requirement",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to get short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Test DOT output
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "to", "REQ@1", "--output", "dot"])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph trace_to"))
        .stdout(predicate::str::contains("rankdir=TB"))
        .stdout(predicate::str::contains("fillcolor=lightblue"));
}
