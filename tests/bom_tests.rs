//! BOM (Bill of Materials) entity tests - Components, Assemblies, Suppliers, Quotes

mod common;

use common::{create_test_component, create_test_supplier, setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;

// ============================================================================
// Component Command Tests
// ============================================================================

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
// Quote Price Tests
// ============================================================================

#[test]
fn test_quote_price_basic() {
    let tmp = setup_test_project();

    // Create component and supplier
    create_test_component(&tmp, "PN-PRICE1", "Price Test Component");
    create_test_supplier(&tmp, "Price Test Supplier");

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

    // Create quote with price breaks
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
            "Multi-Break Quote",
            "--breaks",
            "1:10.00:7,100:8.00:14,1000:6.00:21",
            "--no-edit",
        ])
        .assert()
        .success();

    // Generate quote short ID
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .output()
        .unwrap();

    // Test quote price at qty 1
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "price", "QUOT@1", "--qty", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Unit Price:"))
        .stdout(predicate::str::contains("10.00"));

    // Test quote price at qty 100 (should get $8.00 price)
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "price", "QUOT@1", "--qty", "100"])
        .assert()
        .success()
        .stdout(predicate::str::contains("8.00"));

    // Test quote price at qty 1000 (should get $6.00 price)
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "price", "QUOT@1", "--qty", "1000"])
        .assert()
        .success()
        .stdout(predicate::str::contains("6.00"));
}

#[test]
fn test_quote_price_with_all_breaks() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-BREAKS", "Price Breaks Component");
    create_test_supplier(&tmp, "Breaks Supplier");

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
            "All Breaks Quote",
            "--breaks",
            "1:20.00,50:15.00,200:10.00",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .output()
        .unwrap();

    // Test --all flag shows all price breaks
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "price", "QUOT@1", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Price Breaks:"))
        .stdout(predicate::str::contains("20.00"))
        .stdout(predicate::str::contains("15.00"))
        .stdout(predicate::str::contains("10.00"));
}

// ============================================================================
// Quote Compare with Quantity Tests
// ============================================================================

#[test]
fn test_quote_compare_with_qty() {
    let tmp = setup_test_project();

    // Create component and two suppliers
    create_test_component(&tmp, "PN-CMP-QTY", "Compare Qty Component");
    create_test_supplier(&tmp, "Supplier A");
    create_test_supplier(&tmp, "Supplier B");

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

    // Create two quotes with different price breaks
    // Quote 1: Better at low qty, worse at high qty
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
            "Quote A",
            "--breaks",
            "1:5.00,100:4.50,1000:4.00",
            "--no-edit",
        ])
        .assert()
        .success();

    // Quote 2: Worse at low qty, better at high qty
    tdt()
        .current_dir(tmp.path())
        .args([
            "quote",
            "new",
            "--component",
            "CMP@1",
            "--supplier",
            "SUP@2",
            "--title",
            "Quote B",
            "--breaks",
            "1:6.00,100:4.00,1000:3.00",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .output()
        .unwrap();

    // Compare at qty 1 - Quote A should be cheaper
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "compare", "CMP@1", "--qty", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("at qty 1"))
        .stdout(predicate::str::contains("5.00"));

    // Compare at qty 1000 - Quote B should be cheaper
    tdt()
        .current_dir(tmp.path())
        .args(["quote", "compare", "CMP@1", "--qty", "1000"])
        .assert()
        .success()
        .stdout(predicate::str::contains("at qty 1000"))
        .stdout(predicate::str::contains("3.00"));
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
// BOM Cost with NRE Tests
// ============================================================================

#[test]
fn test_asm_cost_with_nre_amortization() {
    let tmp = setup_test_project();

    // Create component, supplier, and assembly
    create_test_component(&tmp, "PN-NRE", "NRE Test Component");
    create_test_supplier(&tmp, "NRE Supplier");

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

    // Create quote with tooling cost
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
            "NRE Quote",
            "--price",
            "5.00",
            "--tooling",
            "10000",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .output()
        .unwrap();

    // Set quote on component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "set-quote", "CMP@1", "QUOT@1"])
        .assert()
        .success();

    // Create assembly with this component
    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-NRE",
            "--title",
            "NRE Assembly",
            "--bom",
            "CMP@1:2",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    // Test cost with NRE amortization
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "cost", "ASM@1", "--amortize", "1000", "--breakdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains("NRE Amortization: 1000"))
        .stdout(predicate::str::contains("Piece Cost:"))
        .stdout(predicate::str::contains("Total NRE:"))
        .stdout(predicate::str::contains("NRE per Unit:"))
        .stdout(predicate::str::contains("Effective Unit Cost:"));
}

#[test]
fn test_asm_cost_no_nre_flag() {
    let tmp = setup_test_project();

    create_test_component(&tmp, "PN-NONRE", "No NRE Component");
    create_test_supplier(&tmp, "No NRE Supplier");

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
            "No NRE Quote",
            "--price",
            "10.00",
            "--tooling",
            "5000",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["quote", "list"])
        .output()
        .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "set-quote", "CMP@1", "QUOT@1"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-NONRE",
            "--title",
            "No NRE Assembly",
            "--bom",
            "CMP@1:1",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["asm", "list"])
        .output()
        .unwrap();

    // Test cost with --no-nre should not show NRE details
    tdt()
        .current_dir(tmp.path())
        .args(["asm", "cost", "ASM@1", "--no-nre"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total Cost:"))
        // Should NOT contain NRE details when --no-nre is used
        .stdout(predicate::str::contains("Total NRE:").not());
}

// ============================================================================
// Component Supplier ID Tests
// ============================================================================

#[test]
fn test_component_supplier_id_field() {
    let tmp = setup_test_project();

    // Create a component
    create_test_component(&tmp, "PN-SUPID", "Supplier ID Test");

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Get the component file and manually add supplier with supplier_id
    let cmp_dir = tmp.path().join("bom/components");
    let entries: Vec<_> = fs::read_dir(&cmp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();

    let cmp_path = entries[0].path();
    let content = fs::read_to_string(&cmp_path).unwrap();

    // Add supplier with supplier_id field
    let updated_content = format!(
        r#"{}
suppliers:
  - supplier_id: SUP-TEST123
    name: Test Supplier
    supplier_pn: TS-001
    lead_time_days: 14
    moq: 100
    unit_cost: 5.00
"#,
        content.trim()
    );
    fs::write(&cmp_path, updated_content).unwrap();

    // Validate should pass (schema accepts supplier_id)
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();

    // Component should still parse correctly
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "show", "CMP@1", "-o", "yaml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("supplier_id: SUP-TEST123"))
        .stdout(predicate::str::contains("name: Test Supplier"));
}
