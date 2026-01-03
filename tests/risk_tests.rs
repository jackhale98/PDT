//! Risk and Hazard entity tests

mod common;

use common::{create_test_risk, setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;

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
// Hazard Command Tests
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
// Hazard Schema Tests
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
