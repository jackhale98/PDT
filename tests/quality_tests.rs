//! Quality entity tests - NCRs, CAPAs, and Validation

mod common;

use common::{create_test_component, setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;

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
    common::create_test_requirement(&tmp, "Valid Req", "input");

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
// Stale Torsor Bounds Tests
// ============================================================================

#[test]
fn test_validate_detects_stale_torsor_bounds() {
    let tmp = setup_test_project();

    // Create component and feature with GD&T
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--title",
            "Frame",
            "--part-number",
            "FRM-001",
            "--no-edit",
        ])
        .assert()
        .success();

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
            "--title",
            "Bore",
            "--feature-type",
            "internal",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    // Add GD&T and WRONG torsor_bounds manually
    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    let feat_path = entries[0].path();

    let content = std::fs::read_to_string(&feat_path).unwrap();
    let updated = content
        + r#"
geometry_class: cylinder
gdt:
  - symbol: position
    value: 0.25
    material_condition: rfs
torsor_bounds:
  u: [-0.5, 0.5]
  v: [-0.5, 0.5]
"#;
    std::fs::write(&feat_path, updated).unwrap();

    // Validate should warn about stale bounds
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("calculation warning"))
        .stdout(predicate::str::contains("differs from computed"));
}

#[test]
fn test_validate_fix_updates_torsor_bounds() {
    let tmp = setup_test_project();

    // Create component and feature with GD&T but no torsor_bounds
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--title",
            "Plate",
            "--part-number",
            "PLT-001",
            "--no-edit",
        ])
        .assert()
        .success();

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
            "--title",
            "Locator Hole",
            "--feature-type",
            "internal",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    // Add GD&T without torsor_bounds
    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    let feat_path = entries[0].path();

    let content = std::fs::read_to_string(&feat_path).unwrap();
    let updated = content
        + r#"
geometry_class: cylinder
gdt:
  - symbol: position
    value: 0.30
    material_condition: rfs
"#;
    std::fs::write(&feat_path, updated).unwrap();

    // Validate with --fix should add torsor_bounds
    tdt()
        .current_dir(tmp.path())
        .args(["validate", "--fix"])
        .assert()
        .success();

    // Verify the file now has torsor_bounds
    let content = std::fs::read_to_string(&feat_path).unwrap();
    assert!(
        content.contains("torsor_bounds"),
        "File should have torsor_bounds after validate --fix"
    );
    assert!(
        content.contains("-0.15") || content.contains("0.15"),
        "Bounds should be 0.30/2 = 0.15"
    );
}

// ============================================================================
// Feature length_ref Tests
// ============================================================================

#[test]
fn test_validate_detects_stale_length_ref() {
    let tmp = setup_test_project();

    // Create a component first
    create_test_component(&tmp, "PN-LREF", "Length Ref Test");

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Create the source feature with a dimension
    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--title",
            "Wall Thickness",
            "--feature-type",
            "external",
            "--no-edit",
        ])
        .assert()
        .success();

    // Create the referencing feature
    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--title",
            "Bore Depth",
            "--feature-type",
            "internal",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to get short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    // Manually add dimension to first feature and length_ref to second
    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    // Add dimension "depth" to first feature (Wall Thickness)
    let wall_path = entries
        .iter()
        .find(|e| {
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();
            content.contains("Wall Thickness")
        })
        .unwrap()
        .path();
    let wall_content = std::fs::read_to_string(&wall_path).unwrap();
    // Replace the default diameter dimension with our depth dimension
    let wall_updated = wall_content
        .replace("- name: diameter", "- name: depth")
        .replace("nominal: 10.0", "nominal: 25.0");
    std::fs::write(&wall_path, wall_updated).unwrap();

    // Get the ID of the first feature for the reference
    let wall_id = wall_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .replace(".tdt.yaml", "");

    // Add length_ref to second feature (Bore Depth) with stale cached value
    let bore_path = entries
        .iter()
        .find(|e| {
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();
            content.contains("Bore Depth")
        })
        .unwrap()
        .path();
    let bore_content = std::fs::read_to_string(&bore_path).unwrap();
    // Add geometry_class and geometry_3d after feature_type line
    let bore_updated = bore_content.replace(
        "feature_type: internal",
        &format!(
            r#"feature_type: internal
geometry_class: cylinder
geometry_3d:
  origin: [0, 0, 0]
  axis: [0, 0, 1]
  length: 20.0
  length_ref: "{}:depth""#,
            wall_id
        ),
    );
    std::fs::write(&bore_path, bore_updated).unwrap();

    // Validate should detect stale length (20.0 != 25.0)
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("length_ref stale"));
}

#[test]
fn test_validate_fix_updates_length_ref() {
    let tmp = setup_test_project();

    // Create a component first
    create_test_component(&tmp, "PN-LFIX", "Length Fix Test");

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Create source feature
    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--title",
            "Source Dim",
            "--feature-type",
            "external",
            "--no-edit",
        ])
        .assert()
        .success();

    // Create referencing feature
    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--title",
            "Referencing Feat",
            "--feature-type",
            "internal",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    // Add dimension to source - replace default diameter with thickness
    let source_path = entries
        .iter()
        .find(|e| {
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();
            content.contains("Source Dim")
        })
        .unwrap()
        .path();
    let source_content = std::fs::read_to_string(&source_path).unwrap();
    let source_updated = source_content
        .replace("- name: diameter", "- name: thickness")
        .replace("nominal: 10.0", "nominal: 15.5");
    std::fs::write(&source_path, source_updated).unwrap();

    let source_id = source_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .replace(".tdt.yaml", "");

    // Add stale length_ref to referencing feature
    let ref_path = entries
        .iter()
        .find(|e| {
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();
            content.contains("Referencing Feat")
        })
        .unwrap()
        .path();
    let ref_content = std::fs::read_to_string(&ref_path).unwrap();
    let ref_updated = ref_content.replace(
        "feature_type: internal",
        &format!(
            r#"feature_type: internal
geometry_class: cylinder
geometry_3d:
  origin: [0, 0, 0]
  axis: [0, 0, 1]
  length: 10.0
  length_ref: "{}:thickness""#,
            source_id
        ),
    );
    std::fs::write(&ref_path, ref_updated).unwrap();

    // Validate with --fix should update the length
    tdt()
        .current_dir(tmp.path())
        .args(["validate", "--fix"])
        .assert()
        .success();

    // Verify the file now has correct length
    let content = std::fs::read_to_string(&ref_path).unwrap();
    assert!(
        content.contains("15.5"),
        "File should have updated length (15.5) after validate --fix"
    );
}
