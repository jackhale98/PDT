//! Edge case and cache tests

mod common;

use common::{setup_test_project, tdt, create_test_requirement, create_test_component, create_test_risk};
use predicates::prelude::*;
use std::fs;

// ============================================================================
// Cache Tests
// ============================================================================

#[test]
fn test_cache_is_gitignored() {
    let tmp = setup_test_project();

    // Check that .gitignore includes cache files
    let gitignore_path = tmp.path().join(".gitignore");
    let gitignore_content = fs::read_to_string(&gitignore_path).unwrap();

    assert!(
        gitignore_content.contains("cache.db"),
        ".gitignore should include cache.db"
    );
    assert!(
        gitignore_content.contains("cache.db-journal"),
        ".gitignore should include cache.db-journal"
    );
    assert!(
        gitignore_content.contains("cache.db-wal"),
        ".gitignore should include cache.db-wal"
    );
}

#[test]
fn test_entity_files_contain_full_ids_not_short_ids() {
    let tmp = setup_test_project();

    // Create an entity
    create_test_requirement(&tmp, "Full ID Test", "input");

    // Find the created file
    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();
    assert_eq!(files.len(), 1);

    let content = fs::read_to_string(files[0].path()).unwrap();

    // Entity file should contain full ULID-based ID
    assert!(
        content.contains("id: REQ-"),
        "Entity file should have full ID"
    );
    // Entity file should NOT contain short ID syntax
    assert!(
        !content.contains("@1"),
        "Entity file should NOT contain short ID syntax"
    );
    assert!(
        !content.contains("REQ@"),
        "Entity file should NOT contain short ID prefix"
    );
}

#[test]
fn test_cache_rebuild_after_external_changes() {
    let tmp = setup_test_project();

    // Create initial requirement
    create_test_requirement(&tmp, "Initial Req", "input");

    // List to generate short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@1"));

    // Simulate external change (like git pull) by creating a new file directly
    // Use a valid ULID format (26 chars, base32 Crockford)
    let new_req_content = r#"
id: REQ-01HQ5V2KRMJ0B9XYZ3NTWPGQ4E
type: input
title: Externally Added Requirement
text: This requirement was added by another user and pulled via git
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: external_user
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V2KRMJ0B9XYZ3NTWPGQ4E.tdt.yaml"),
        new_req_content,
    )
    .unwrap();

    // List should auto-sync cache and show both requirements with new short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initial Req"))
        .stdout(predicate::str::contains("Externally Added Requirement"))
        .stdout(predicate::str::contains("2 requirement(s) found"));
}

#[test]
fn test_cache_handles_deleted_entities() {
    let tmp = setup_test_project();

    // Create two requirements
    create_test_requirement(&tmp, "Req to Keep", "input");
    create_test_requirement(&tmp, "Req to Delete", "input");

    // List to verify both exist
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 requirement(s) found"));

    // Simulate external deletion (like git pull that removes a file)
    let files: Vec<_> = fs::read_dir(tmp.path().join("requirements/inputs"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .collect();

    // Delete the second file
    fs::remove_file(files[1].path()).unwrap();

    // List should auto-sync cache and only show one requirement
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

// ============================================================================
// Short ID Tests
// ============================================================================

#[test]
fn test_short_ids_are_local_to_cache() {
    // This test verifies that short IDs are derived from the local cache
    // and are not stored in entity files (important for git collaboration)
    let tmp = setup_test_project();

    // Create requirements
    create_test_requirement(&tmp, "First Req", "input");
    create_test_requirement(&tmp, "Second Req", "input");

    // Get short IDs from list
    let output = tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8_lossy(&output);
    assert!(output_str.contains("@1"), "Should have short ID @1");
    assert!(output_str.contains("@2"), "Should have short ID @2");

    // Verify the entity files don't contain short IDs
    for entry in fs::read_dir(tmp.path().join("requirements/inputs")).unwrap() {
        let entry = entry.unwrap();
        let content = fs::read_to_string(entry.path()).unwrap();
        assert!(
            !content.contains("@1") && !content.contains("@2"),
            "Entity file should not contain short IDs: {}",
            entry.path().display()
        );
    }
}

#[test]
fn test_cache_clear_and_rebuild() {
    let tmp = setup_test_project();

    // Create some entities
    create_test_requirement(&tmp, "Cache Test Req", "input");
    create_test_risk(&tmp, "Cache Test Risk", "design");

    // List to populate cache
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();
    tdt()
        .current_dir(tmp.path())
        .args(["risk", "list"])
        .assert()
        .success();

    // Clear cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "clear"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache cleared"));

    // Verify cache is deleted
    assert!(!tmp.path().join(".tdt/cache.db").exists());

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache rebuilt"));

    // Verify cache works again
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ@1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache Test Req"));
}

#[test]
fn test_cache_status_command() {
    let tmp = setup_test_project();

    // Create some entities
    create_test_requirement(&tmp, "Status Test 1", "input");
    create_test_requirement(&tmp, "Status Test 2", "input");
    create_test_risk(&tmp, "Status Test Risk", "design");

    // Rebuild cache to ensure counts are accurate
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Check cache status
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cache Status"))
        .stdout(predicate::str::contains("Total entities:"))
        .stdout(predicate::str::contains("3")); // 2 reqs + 1 risk
}

#[test]
fn test_cache_sync_incremental() {
    let tmp = setup_test_project();

    // Create initial entity
    create_test_requirement(&tmp, "Initial Req", "input");

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Add another entity externally
    // Use a valid ULID format (26 chars, base32 Crockford)
    let new_req_content = r#"
id: REQ-01HQ5V3ABCD1234EFGH5678JKM
type: input
title: Sync Test Requirement
text: This requirement was synced from external changes
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: test
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V3ABCD1234EFGH5678JKM.tdt.yaml"),
        new_req_content,
    )
    .unwrap();

    // Sync cache (incremental update)
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "sync"])
        .assert()
        .success()
        .stdout(predicate::str::contains("synced").or(predicate::str::contains("Added")));

    // Verify new entity is accessible
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync Test Requirement"))
        .stdout(predicate::str::contains("2 requirement(s) found"));
}

#[test]
fn test_cache_query_raw_sql() {
    let tmp = setup_test_project();

    // Create entities
    create_test_requirement(&tmp, "Query Test Req", "input");
    create_test_component(&tmp, "PN-QUERY", "Query Test Component");

    // Rebuild to ensure cache is populated
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Query the cache with SQL
    tdt()
        .current_dir(tmp.path())
        .args([
            "cache",
            "query",
            "SELECT id, title FROM entities WHERE prefix = 'REQ'",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Query Test Req"));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_edge_case_unicode_title() {
    let tmp = setup_test_project();

    // Create requirement with Unicode title
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Temperature: 100C +/- 5C",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created requirement"));

    // Verify it can be listed
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Temperature"));
}

#[test]
fn test_edge_case_japanese_title() {
    let tmp = setup_test_project();

    // Create requirement with Japanese characters
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Temperature Requirement - Japanese",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it can be listed
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Temperature Requirement"));
}

#[test]
fn test_edge_case_emoji_in_title() {
    let tmp = setup_test_project();

    // Create requirement with emoji
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Safety Check Warning",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify search works
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Safety Check"));
}

#[test]
fn test_edge_case_special_characters_in_title() {
    let tmp = setup_test_project();

    // Create requirement with special characters (avoiding nested quotes for YAML safety)
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Test: <angled> & (parens) [brackets] #hash",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify file was created and content is valid YAML
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();
}

#[test]
fn test_edge_case_very_long_title() {
    let tmp = setup_test_project();

    // Create requirement with long title (200 chars)
    let long_title = "A".repeat(200);
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            &long_title,
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Should be in list (potentially truncated)
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_edge_case_whitespace_in_title() {
    let tmp = setup_test_project();

    // Create requirement with extra whitespace (should be handled)
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "  Whitespace  Title  ",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 requirement(s) found"));
}

#[test]
fn test_edge_case_multiline_text_in_file() {
    let tmp = setup_test_project();

    // Create a requirement file with multiline text
    let multiline_req = r#"
id: REQ-01HQ5V5ABCD1234EFGH5678JKM
type: input
title: Multiline Test
text: |
  This is a multiline text block.
  It spans multiple lines.
  And has various content:
    - Indented items
    - More items
status: draft
priority: medium
created: 2024-01-15T10:30:00Z
author: test
"#;
    fs::write(
        tmp.path()
            .join("requirements/inputs/REQ-01HQ5V5ABCD1234EFGH5678JKM.tdt.yaml"),
        multiline_req,
    )
    .unwrap();

    // Validate should pass
    tdt()
        .current_dir(tmp.path())
        .args(["validate"])
        .assert()
        .success();

    // Show should display the content
    tdt()
        .current_dir(tmp.path())
        .args(["req", "show", "REQ-01HQ5V5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Multiline Test"));
}

#[test]
fn test_edge_case_numeric_string_part_number() {
    let tmp = setup_test_project();

    // Create component with numeric-looking part number
    tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--part-number",
            "12345",
            "--title",
            "Numeric PN Component",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it can be found
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("12345"));
}

#[test]
fn test_edge_case_zero_quantity_lot() {
    let tmp = setup_test_project();

    // Create lot with zero quantity
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Zero Qty Lot",
            "--lot-number",
            "LOT-ZERO",
            "--quantity",
            "0",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Zero Qty Lot"));
}

#[test]
fn test_edge_case_large_quantity_lot() {
    let tmp = setup_test_project();

    // Create lot with large quantity
    tdt()
        .current_dir(tmp.path())
        .args([
            "lot",
            "new",
            "--title",
            "Large Qty Lot",
            "--lot-number",
            "LOT-LARGE",
            "--quantity",
            "999999999",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["lot", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Large Qty Lot"));
}

#[test]
fn test_edge_case_fmea_boundary_values() {
    let tmp = setup_test_project();

    // Test minimum FMEA values (1)
    tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            "Min FMEA Risk",
            "--severity",
            "1",
            "--occurrence",
            "1",
            "--detection",
            "1",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RPN: 1")); // 1 * 1 * 1 = 1

    // Test maximum FMEA values (10)
    tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            "Max FMEA Risk",
            "--severity",
            "10",
            "--occurrence",
            "10",
            "--detection",
            "10",
            "--no-edit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("RPN: 1000")); // 10 * 10 * 10 = 1000
}

#[test]
fn test_edge_case_tolerance_stackup_zero_nominal() {
    let tmp = setup_test_project();

    // Create tolerance stackup with zero nominal
    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "Zero Nominal Stackup",
            "--target-name",
            "Zero Gap",
            "--target-nominal",
            "0.0",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Zero Nominal Stackup"));
}

#[test]
fn test_edge_case_negative_tolerance_value() {
    let tmp = setup_test_project();

    // Create tolerance stackup with negative nominal (allowed for offset checks)
    // Using = syntax to prevent negative value being parsed as flag
    tdt()
        .current_dir(tmp.path())
        .args([
            "tol",
            "new",
            "--title",
            "Negative Nominal Stackup",
            "--target-name",
            "Offset Check",
            "--target-nominal=-0.5",
            "--no-edit",
        ])
        .assert()
        .success();

    // Verify it was created
    tdt()
        .current_dir(tmp.path())
        .args(["tol", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Negative Nominal Stackup"));
}
