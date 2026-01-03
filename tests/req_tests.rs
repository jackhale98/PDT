//! Requirement entity tests

mod common;

use common::{setup_test_project, tdt, create_test_requirement};
use predicates::prelude::*;
use std::fs;

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
// Requirement Delete Tests
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
// Requirement Archive Tests
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
