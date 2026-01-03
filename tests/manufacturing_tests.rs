//! Manufacturing entity tests - Processes, Controls, Work Instructions, Lots, Deviations

mod common;

use common::{setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;

// ============================================================================
// Process Command Tests
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
// Deviation Command Tests
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
