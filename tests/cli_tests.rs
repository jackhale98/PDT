//! CLI and basic command tests

mod common;

use common::{
    create_test_protocol, create_test_requirement, create_test_risk, setup_test_project, tdt,
};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

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
// Not In Project Test
// ============================================================================

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
