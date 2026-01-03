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

// ============================================================================
// Assembly Routing Command Tests
// ============================================================================

#[test]
fn test_asm_routing_list_empty() {
    let tmp = setup_test_project();

    // Create an assembly
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Test Assembly",
            "--no-edit",
        ])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    // Routing list should show empty
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "list", "ASM@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No routing configured"));
}

#[test]
fn test_asm_routing_add_and_list() {
    let tmp = setup_test_project();

    // Create an assembly and some processes
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Test Assembly",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "CNC Machining",
            "--type",
            "machining",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "Inspection",
            "--type",
            "inspection",
            "--no-edit",
        ])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Add processes to routing
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "add", "ASM@1", "PROC@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added"));

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "add", "ASM@1", "PROC@2"])
        .assert()
        .success();

    // Routing list should show both processes
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "list", "ASM@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CNC Machining"))
        .stdout(predicate::str::contains("Inspection"));
}

#[test]
fn test_asm_routing_rm() {
    let tmp = setup_test_project();

    // Create an assembly and process
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Test Assembly",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "CNC Machining", "--no-edit"])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Add process to routing
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "add", "ASM@1", "PROC@1"])
        .assert()
        .success();

    // Remove by position (1-indexed)
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "rm", "ASM@1", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    // Routing should be empty now
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "list", "ASM@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No routing configured"));
}

#[test]
fn test_asm_routing_set() {
    let tmp = setup_test_project();

    // Create an assembly and processes
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Test Assembly",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Process A", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Process B", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Process C", "--no-edit"])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Set complete routing in one command
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm", "routing", "set", "ASM@1", "PROC@1", "PROC@2", "PROC@3",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set routing with 3 steps"));

    // Verify all processes are in routing
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "list", "ASM@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Process A"))
        .stdout(predicate::str::contains("Process B"))
        .stdout(predicate::str::contains("Process C"));
}

// ============================================================================
// Component Routing Command Tests
// ============================================================================

#[test]
fn test_cmp_routing_add_and_list() {
    let tmp = setup_test_project();

    // Create a component and process
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--part-number",
            "CMP-001",
            "--title",
            "Test Component",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "Turning",
            "--type",
            "machining",
            "--no-edit",
        ])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Add process to routing
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "routing", "add", "CMP@1", "PROC@1"])
        .assert()
        .success();

    // Verify it appears in routing list
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "routing", "list", "CMP@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Turning"));
}

// ============================================================================
// Lot with Routing Tests
// ============================================================================

#[test]
fn test_lot_new_from_routing() {
    let tmp = setup_test_project();

    // Create assembly and processes
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Assembled Widget",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Step 1: Machining", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Step 2: Assembly", "--no-edit"])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Set routing on assembly
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "set", "ASM@1", "PROC@1", "PROC@2"])
        .assert()
        .success();

    // Create lot from routing
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Production Lot",
            "--lot-number",
            "LOT-2024-001",
            "--product",
            "ASM@1",
            "--from-routing",
            "--quantity",
            "50",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created lot"));

    // Prime lot short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .output()
        .unwrap();

    // Show lot should display pre-populated execution steps
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "show", "LOT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Machining").or(predicate::str::contains("pending")));
}

#[test]
fn test_lot_step_complete() {
    let tmp = setup_test_project();

    // Create lot with steps
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Widget",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "Process 1", "--no-edit"])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Set routing
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "routing", "set", "ASM@1", "PROC@1"])
        .assert()
        .success();

    // Create lot from routing
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Test Lot",
            "--lot-number",
            "LOT-001",
            "--product",
            "ASM@1",
            "--from-routing",
            "--quantity",
            "10",
            "--no-edit",
        ])
        .assert()
        .success();

    // Prime lot short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .output()
        .unwrap();

    // Complete the step
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "step",
            "LOT@1",
            "--process",
            "0",
            "--status",
            "completed",
            "--operator",
            "John Smith",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated"))
        .stdout(predicate::str::contains("completed"));
}

#[test]
fn test_lot_complete() {
    let tmp = setup_test_project();

    // Create a simple lot
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Complete Test Lot",
            "--lot-number",
            "LOT-COMPLETE-001",
            "--quantity",
            "25",
            "--no-edit",
        ])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .output()
        .unwrap();

    // Complete the lot
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "complete", "LOT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("completed"));

    // Verify lot shows as completed
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "show", "LOT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("completed"));
}

// ============================================================================
// Lot Show-WI Tests
// ============================================================================

#[test]
fn test_lot_step_show_wi() {
    let tmp = setup_test_project();

    // Create process with work instruction link
    tdt()
        .current_dir(tmp.path())
        .args([
            "work",
            "new",
            "--title",
            "Setup Procedure",
            "--doc-number",
            "WI-001",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "new", "--title", "CNC Setup", "--no-edit"])
        .assert()
        .success();

    // Prime short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["work", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["proc", "list"])
        .output()
        .unwrap();

    // Link work instruction to process
    tdt()
        .current_dir(tmp.path())
        .args(["link", "add", "PROC@1", "WORK@1", "work_instructions"])
        .assert()
        .success();

    // Create assembly with routing
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-001",
            "--title",
            "Test Product",
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
        .args(["asm", "routing", "set", "ASM@1", "PROC@1"])
        .assert()
        .success();

    // Create lot from routing
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "WI Test Lot",
            "--lot-number",
            "LOT-WI-001",
            "--product",
            "ASM@1",
            "--from-routing",
            "--quantity",
            "5",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .output()
        .unwrap();

    // Show work instructions for step
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "step", "LOT@1", "--process", "0", "--show-wi"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Setup Procedure")
                .or(predicate::str::contains("WI-"))
                .or(predicate::str::contains("Work Instructions")),
        );
}
