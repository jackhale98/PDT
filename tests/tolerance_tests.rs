//! Tolerance analysis tests - Features, Mates, Tolerance Stackups

mod common;

use common::{create_test_component, create_test_feature, setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;

// ============================================================================
// Feature Command Tests
// ============================================================================

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
// Backward Compatibility Tests for 3D Fields
// ============================================================================

#[test]
fn test_backward_compatibility_feat_without_3d_fields() {
    // Test that feature files without 3D geometry fields still parse correctly
    let tmp = setup_test_project();

    // Create a component first
    create_test_component(&tmp, "PN-COMPAT", "Compatibility Test Component");
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Create a feature (no 3D fields)
    create_test_feature(&tmp, "CMP@1", "internal", "Legacy Feature");

    // List should work
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Legacy Feature"));

    // Show should work
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "show", "FEAT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Legacy Feature"));
}

#[test]
fn test_backward_compatibility_tol_without_3d_fields() {
    // Test that stackup files without 3D analysis fields still parse correctly
    let tmp = setup_test_project();

    // Create a stackup (no 3D fields)
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "new", "--title", "Legacy Stackup", "--no-edit"])
        .assert()
        .success();

    // List should work
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Legacy Stackup"));

    // Show should work
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "show", "TOL@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Legacy Stackup"));
}

// ============================================================================
// 3D Tolerance Analysis Tests
// ============================================================================

#[test]
fn test_tol_analyze_accepts_3d_flag() {
    // Test that the --3d flag is accepted by tol analyze (even if not yet wired up)
    let tmp = setup_test_project();

    // Create a stackup with a contributor
    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "3D Test Stackup",
            "--target-name",
            "Gap",
            "--target-nominal",
            "5.0",
            "--target-upper",
            "6.0",
            "--target-lower",
            "4.0",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .output()
        .unwrap();

    // Add a manual contributor (via YAML editing)
    let tol_dir = tmp.path().join("tolerances/stackups");
    let entries: Vec<_> = fs::read_dir(&tol_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();

    assert_eq!(entries.len(), 1, "Should have exactly one stackup file");
    let stackup_path = entries[0].path();
    let content = fs::read_to_string(&stackup_path).unwrap();

    // Add a contributor to the stackup - replace the commented contributors section
    // Find the "contributors:" line and replace from there to "analysis_results:"
    let updated_content = if let Some(start) = content.find("contributors:") {
        if let Some(end) = content.find("analysis_results:") {
            let mut new_content = String::from(&content[..start]);
            new_content.push_str("contributors:\n  - name: Test Dimension\n    nominal: 5.0\n    plus_tol: 0.1\n    minus_tol: 0.1\n\n");
            new_content.push_str(&content[end..]);
            new_content
        } else {
            content.clone()
        }
    } else {
        content.clone()
    };
    fs::write(&stackup_path, &updated_content).unwrap();

    // The --3d flag should be accepted (command syntax check)
    // Note: Full 3D analysis requires features with geometry_3d defined
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "analyze", "TOL@1", "--3d"])
        .assert()
        .success();
}

#[test]
fn test_tol_analyze_accepts_visualize_flag() {
    // Test that the --visualize flag is accepted
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "Viz Test",
            "--target-nominal",
            "10.0",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .output()
        .unwrap();

    // Add a contributor
    let tol_dir = tmp.path().join("tolerances/stackups");
    let entries: Vec<_> = fs::read_dir(&tol_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    let stackup_path = entries[0].path();
    let content = fs::read_to_string(&stackup_path).unwrap();

    // Add a contributor - replace the commented contributors section
    let updated_content = if let Some(start) = content.find("contributors:") {
        if let Some(end) = content.find("analysis_results:") {
            let mut new_content = String::from(&content[..start]);
            new_content.push_str("contributors:\n  - name: Dim A\n    nominal: 10.0\n    plus_tol: 0.05\n    minus_tol: 0.05\n\n");
            new_content.push_str(&content[end..]);
            new_content
        } else {
            content.clone()
        }
    } else {
        content.clone()
    };
    fs::write(&stackup_path, &updated_content).unwrap();

    // The --visualize flag should be accepted
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "analyze", "TOL@1", "--visualize"])
        .assert()
        .success();
}

// ============================================================================
// Feature 3D Geometry Tests
// ============================================================================

#[test]
fn test_feat_with_3d_geometry_yaml() {
    // Test that features with 3D geometry fields can be created and parsed
    let tmp = setup_test_project();

    // Create a component
    create_test_component(&tmp, "PN-3D", "3D Test Component");
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Create a basic feature
    create_test_feature(&tmp, "CMP@1", "internal", "3D Mounting Hole");

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    // Add 3D geometry via YAML editing
    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();

    assert_eq!(entries.len(), 1, "Should have exactly one feature file");
    let feat_path = entries[0].path();
    let content = fs::read_to_string(&feat_path).unwrap();

    // Append 3D geometry fields
    let updated_content = format!(
        r#"{}
geometry_class: cylinder
datum_label: A
geometry_3d:
  origin: [0.0, 0.0, 0.0]
  axis: [0.0, 0.0, 1.0]
  length: 25.0
"#,
        content.trim()
    );
    fs::write(&feat_path, updated_content).unwrap();

    // Verify the feature still parses correctly
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "show", "FEAT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3D Mounting Hole"));

    // Validate should pass
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();
}

#[test]
fn test_tol_with_3d_config_yaml() {
    // Test that stackups with 3D analysis configuration can be created and parsed
    let tmp = setup_test_project();

    // Create a stackup
    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "3D Analysis Stackup",
            "--target-nominal",
            "15.0",
            "--target-upper",
            "16.0",
            "--target-lower",
            "14.0",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .output()
        .unwrap();

    // Add 3D configuration via YAML editing
    let tol_dir = tmp.path().join("tolerances/stackups");
    let entries: Vec<_> = fs::read_dir(&tol_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();

    let stackup_path = entries[0].path();
    let content = fs::read_to_string(&stackup_path).unwrap();

    // Fix contributors section and add 3D configuration
    let updated_content = if let Some(start) = content.find("contributors:") {
        if let Some(end) = content.find("analysis_results:") {
            let mut new_content = String::from(&content[..start]);
            // Add contributor and 3D config
            new_content.push_str("contributors:\n  - name: Test Dim\n    nominal: 15.0\n    plus_tol: 0.1\n    minus_tol: 0.1\n\n");
            new_content.push_str("functional_direction: [1.0, 0.0, 0.0]\n\n");
            new_content.push_str("analysis_3d:\n  enabled: true\n  method: jacobian_torsor\n  monte_carlo_iterations: 10000\n\n");
            new_content.push_str(&content[end..]);
            new_content
        } else {
            content.clone()
        }
    } else {
        content.clone()
    };
    fs::write(&stackup_path, &updated_content).unwrap();

    // Verify the stackup still parses correctly
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "show", "TOL@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3D Analysis Stackup"));

    // Validate should pass
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();
}

// ============================================================================
// Feature Compute Bounds Tests
// ============================================================================

#[test]
fn test_feat_compute_bounds_basic() {
    let tmp = setup_test_project();

    // Create a component first
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--title",
            "Housing",
            "--part-number",
            "HSG-001",
            "--no-edit",
        ])
        .assert()
        .success();

    // Assign short ID
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Create a feature with GD&T and geometry_class
    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--title",
            "Mounting Hole",
            "--feature-type",
            "internal",
            "--no-edit",
        ])
        .assert()
        .success();

    // Assign short ID
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    // Manually edit the feature to add GD&T and geometry_class
    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1);
    let feat_path = entries[0].path();

    let content = std::fs::read_to_string(&feat_path).unwrap();
    let updated = content
        + r#"
geometry_class: cylinder
geometry_3d:
  origin: [50.0, 25.0, 0.0]
  axis: [0.0, 0.0, 1.0]
  length: 15.0
gdt:
  - symbol: position
    value: 0.25
    datum_refs: ["A", "B", "C"]
    material_condition: rfs
"#;
    std::fs::write(&feat_path, updated).unwrap();

    // Now test compute-bounds
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "compute-bounds", "FEAT@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Computed torsor bounds"))
        .stdout(predicate::str::contains("u:"))
        .stdout(predicate::str::contains("-0.125000, 0.125000"));
}

#[test]
fn test_feat_compute_bounds_with_update() {
    let tmp = setup_test_project();

    // Create component and feature
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--title",
            "Bracket",
            "--part-number",
            "BRK-001",
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
            "Pin Hole",
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

    // Add GD&T manually
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
    value: 0.50
    material_condition: rfs
"#;
    std::fs::write(&feat_path, updated).unwrap();

    // Compute bounds and update file
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "compute-bounds", "FEAT@1", "--update"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated"));

    // Verify the file now has torsor_bounds
    let content = std::fs::read_to_string(&feat_path).unwrap();
    assert!(
        content.contains("torsor_bounds"),
        "File should have torsor_bounds after --update"
    );
}

#[test]
fn test_feat_set_length_from_another_feature() {
    let tmp = setup_test_project();

    // Create component
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--title",
            "Housing",
            "--part-number",
            "HSG-001",
            "--no-edit",
        ])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .output()
        .unwrap();

    // Create source feature with dimension
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

    tdt()
        .current_dir(tmp.path())
        .args(["feat", "list"])
        .output()
        .unwrap();

    // Modify source feature to have a specific "depth" dimension
    let feat_dir = tmp.path().join("tolerances/features");
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    let source_path = entries[0].path();

    let content = std::fs::read_to_string(&source_path).unwrap();
    // Replace the default "diameter" dimension with "depth"
    let content = content.replace("- name: diameter", "- name: depth");
    let content = content.replace("nominal: 10.0", "nominal: 25.5");
    std::fs::write(&source_path, &content).unwrap();

    // Create target feature
    tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            "CMP@1",
            "--title",
            "Cavity",
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

    // Set length on target feature from source feature's depth dimension
    tdt()
        .current_dir(tmp.path())
        .args(["feat", "set-length", "FEAT@2", "--from", "FEAT@1:depth"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set"))
        .stdout(predicate::str::contains("25.5"));

    // Verify target feature now has length_ref set
    let entries: Vec<_> = std::fs::read_dir(&feat_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    // Find the target feature (the second one created)
    let mut target_found = false;
    for entry in &entries {
        let content = std::fs::read_to_string(entry.path()).unwrap();
        if content.contains("Cavity") {
            assert!(content.contains("length_ref:"), "Should have length_ref");
            assert!(
                content.contains(":depth"),
                "Should reference depth dimension"
            );
            assert!(content.contains("length: 25.5"), "Should have length value");
            target_found = true;
            break;
        }
    }
    assert!(target_found, "Target feature not found");
}
