//! Integration tests for suspect link change-impact analysis
//!
//! These tests cover the user-facing change-impact workflow:
//! - Manual marking and clearing
//! - `--all-for` bulk clear
//! - Reciprocal symmetric marking
//! - List and review commands
//! - Validation that mark/clear preserves link titles

mod common;

use common::{
    create_test_component, create_test_protocol, create_test_requirement, create_test_risk,
    setup_test_project, tdt,
};
use std::fs;
use std::path::PathBuf;

/// Find an entity file in the project by ID prefix.
fn find_yaml_file(tmp: &tempfile::TempDir, id_prefix: &str) -> PathBuf {
    for entry in walkdir::WalkDir::new(tmp.path())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.to_string_lossy().ends_with(".tdt.yaml") {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.starts_with(id_prefix) {
                    return p.to_path_buf();
                }
            }
        }
    }
    panic!("No file found starting with {}", id_prefix);
}

fn read_file(p: &PathBuf) -> String {
    fs::read_to_string(p).unwrap()
}

// ============================================================================
// Manual mark / clear
// ============================================================================

#[test]
fn test_suspect_mark_then_clear() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec", "input");
    create_test_protocol(&tmp, "Verify", "verification");

    // Add a link first
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();

    // Mark suspect
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();

    // Verify it appears in list
    let list_out = tdt()
        .current_dir(tmp.path())
        .args(["link", "suspect", "list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&list_out.stdout);
    assert!(
        stdout.contains("suspect"),
        "Expected suspect link in list: {}",
        stdout
    );

    // Clear it
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "clear",
            "REQ@1",
            "TEST@1",
            "-t",
            "verified_by",
        ])
        .assert()
        .success();

    // Verify it's gone
    let list_out2 = tdt()
        .current_dir(tmp.path())
        .args(["link", "suspect", "list"])
        .output()
        .unwrap();
    let stdout2 = String::from_utf8_lossy(&list_out2.stdout);
    assert!(
        stdout2.contains("No suspect"),
        "Expected no suspect links after clear: {}",
        stdout2
    );
}

// ============================================================================
// Mark/clear preserves link title
// ============================================================================

#[test]
fn test_mark_suspect_preserves_link_title() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec", "input");
    create_test_protocol(&tmp, "Verify Pressure", "verification");

    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();

    // Stamp titles via validate --fix so the link entry has {id, title}
    tdt()
        .current_dir(tmp.path())
        .args(["validate", "--fix"])
        .assert()
        .success();

    let req_file = find_yaml_file(&tmp, "REQ-");
    let before = read_file(&req_file);
    assert!(
        before.contains("title: Verify Pressure"),
        "Title not stamped: {}",
        before
    );

    // Mark suspect
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();

    let after = read_file(&req_file);
    assert!(
        after.contains("title: Verify Pressure"),
        "Title was lost during mark suspect: {}",
        after
    );
    assert!(
        after.contains("suspect: true"),
        "Suspect flag not added: {}",
        after
    );
}

#[test]
fn test_clear_suspect_preserves_link_title() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec", "input");
    create_test_protocol(&tmp, "Verify Pressure", "verification");

    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["validate", "--fix"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "clear",
            "REQ@1",
            "TEST@1",
            "-t",
            "verified_by",
            "--verified-revision",
            "3",
        ])
        .assert()
        .success();

    let req_file = find_yaml_file(&tmp, "REQ-");
    let after = read_file(&req_file);
    assert!(
        after.contains("title: Verify Pressure"),
        "Title was lost during clear: {}",
        after
    );
    assert!(
        after.contains("verified_revision: 3"),
        "verified_revision not set: {}",
        after
    );
    assert!(
        !after.contains("suspect: true"),
        "Suspect flag not cleared: {}",
        after
    );
}

// ============================================================================
// Reciprocal mark / clear
// ============================================================================

#[test]
fn test_mark_suspect_marks_reciprocal() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec", "input");
    create_test_protocol(&tmp, "Verify", "verification");

    // link add will add both forward (REQ.verified_by → TEST) and reciprocal (TEST.verifies → REQ)
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();

    // Mark suspect from REQ side
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();

    let req_file = find_yaml_file(&tmp, "REQ-");
    let test_file = find_yaml_file(&tmp, "TEST-");

    assert!(
        read_file(&req_file).contains("suspect: true"),
        "REQ side should be suspect"
    );
    assert!(
        read_file(&test_file).contains("suspect: true"),
        "TEST side reciprocal should also be suspect"
    );
}

#[test]
fn test_clear_suspect_clears_reciprocal() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec", "input");
    create_test_protocol(&tmp, "Verify", "verification");

    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "clear",
            "REQ@1",
            "TEST@1",
            "-t",
            "verified_by",
        ])
        .assert()
        .success();

    let req_file = find_yaml_file(&tmp, "REQ-");
    let test_file = find_yaml_file(&tmp, "TEST-");

    assert!(
        !read_file(&req_file).contains("suspect: true"),
        "REQ side should be cleared"
    );
    assert!(
        !read_file(&test_file).contains("suspect: true"),
        "TEST side reciprocal should also be cleared"
    );
}

// ============================================================================
// Single-value link suspect (RISK.component)
// ============================================================================

#[test]
fn test_mark_clear_single_value_link() {
    let tmp = setup_test_project();
    create_test_risk(&tmp, "Failure", "design");
    create_test_component(&tmp, "P-001", "Widget");

    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "RISK@1", "CMP@1"])
        .assert()
        .success();

    // Mark suspect on the single-value `component` field
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "RISK@1",
            "CMP@1",
            "-t",
            "component",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();

    let risk_file = find_yaml_file(&tmp, "RISK-");
    let contents = read_file(&risk_file);
    assert!(
        contents.contains("suspect: true"),
        "Single-value link should be marked: {}",
        contents
    );

    // Clear it
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "clear",
            "RISK@1",
            "CMP@1",
            "-t",
            "component",
        ])
        .assert()
        .success();

    let after = read_file(&risk_file);
    assert!(
        !after.contains("suspect: true"),
        "Single-value link should be cleared: {}",
        after
    );
}

// ============================================================================
// Bulk clear --all-for
// ============================================================================

#[test]
fn test_bulk_clear_all_for() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec1", "input");
    create_test_requirement(&tmp, "Spec2", "input");
    create_test_protocol(&tmp, "Verify", "verification");

    // Both REQs link to the same TEST
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@2", "TEST@1"])
        .assert()
        .success();

    // Mark both REQ→TEST suspect
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@2",
            "TEST@1",
            "-r",
            "manually_marked",
        ])
        .assert()
        .success();

    // Bulk clear all suspects pointing at TEST@1
    tdt()
        .current_dir(tmp.path())
        .args(["link", "suspect", "clear", "--all-for", "TEST@1"])
        .assert()
        .success();

    // Verify no suspects remain
    let list_out = tdt()
        .current_dir(tmp.path())
        .args(["link", "suspect", "list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&list_out.stdout);
    assert!(
        stdout.contains("No suspect"),
        "Expected no suspect links after bulk clear: {}",
        stdout
    );
}

// ============================================================================
// Mark-dependents on manual edit: simulated by directly editing the file
// ============================================================================
//
// The auto-mark-on-edit feature triggers from `run_edit_generic` which opens
// $EDITOR. We can't drive an editor in tests, so we verify the underlying
// `mark_dependents_suspect` function's behavior via the CLI `link suspect
// mark` command (which shares the same core logic).

#[test]
fn test_mark_dependents_chain() {
    let tmp = setup_test_project();
    // Two requirements pointing at the same test protocol
    create_test_requirement(&tmp, "Spec1", "input");
    create_test_requirement(&tmp, "Spec2", "input");
    create_test_protocol(&tmp, "Verify", "verification");

    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@2", "TEST@1"])
        .assert()
        .success();

    // Mark all dependents of TEST (i.e., the REQs) suspect. We can't drive
    // run_edit_generic in a test, so we mark each manually and verify the
    // list shows both.
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "content_modified",
        ])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@2",
            "TEST@1",
            "-r",
            "content_modified",
        ])
        .assert()
        .success();

    let list_out = tdt()
        .current_dir(tmp.path())
        .args(["link", "suspect", "list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&list_out.stdout);
    // Should show suspect links from both REQs
    assert!(
        stdout.matches("verified_by").count() >= 2
            || stdout.matches("verifies").count() >= 2
            || stdout.matches("suspect").count() >= 2,
        "Expected multiple suspect links in list: {}",
        stdout
    );
}

// ============================================================================
// Validate --fix preserves suspect metadata
// ============================================================================

#[test]
fn test_validate_fix_preserves_suspect_metadata() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Spec", "input");
    create_test_protocol(&tmp, "Verify Pressure", "verification");

    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "REQ@1", "TEST@1"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args([
            "link",
            "suspect",
            "mark",
            "REQ@1",
            "TEST@1",
            "-r",
            "revision_changed",
        ])
        .assert()
        .success();

    // Run validate --fix; this stamps titles and shouldn't drop suspect metadata
    tdt()
        .current_dir(tmp.path())
        .args(["validate", "--fix"])
        .assert()
        .success();

    let req_file = find_yaml_file(&tmp, "REQ-");
    let contents = read_file(&req_file);
    assert!(
        contents.contains("suspect: true"),
        "validate --fix dropped suspect: {}",
        contents
    );
    assert!(
        contents.contains("revision_changed"),
        "validate --fix dropped suspect_reason: {}",
        contents
    );
}
