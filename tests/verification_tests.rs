//! Verification entity tests - Test Protocols and Results

mod common;

use common::{setup_test_project, tdt, create_test_protocol};
use predicates::prelude::*;
use std::fs;

// ============================================================================
// Test Protocol Command Tests
// ============================================================================

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
