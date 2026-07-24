//! Integration tests for `tdt import --update`

mod common;

use common::{create_test_requirement, setup_test_project, tdt};
use predicates::prelude::*;
use std::fs;

fn req_file_content(tmp: &tempfile::TempDir) -> String {
    let dir = tmp.path().join("requirements/inputs");
    let entry = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        .expect("requirement file exists");
    fs::read_to_string(entry.path()).unwrap()
}

#[test]
fn test_import_update_patches_fields_and_bumps_revision() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Original title", "input");

    let before = req_file_content(&tmp);
    assert!(before.contains("Original title"));

    let csv = tmp.path().join("update.csv");
    fs::write(
        &csv,
        "id,title,priority,status,tags\nREQ@1,Updated via CSV,high,review,\"a,b\"\n",
    )
    .unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["import", "req", csv.to_str().unwrap(), "--update"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Entities updated: 1"));

    let after = req_file_content(&tmp);
    assert!(after.contains("Updated via CSV"));
    assert!(after.contains("priority: high"));
    assert!(after.contains("status: review"));
    assert!(after.contains("- a"));
    assert!(after.contains("- b"));
    // Requirements track an integer entity revision named `revision`
    assert!(
        after.contains("revision: 2"),
        "revision should be bumped: {}",
        after
    );
    // Creation metadata must be untouched
    assert!(!after.contains("Original title"));
}

#[test]
fn test_import_update_requires_id_column() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Original", "input");

    let csv = tmp.path().join("noid.csv");
    fs::write(&csv, "title\nNew title\n").unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["import", "req", csv.to_str().unwrap(), "--update"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires an 'id' column"));
}

#[test]
fn test_import_update_rejects_row_without_id() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Original", "input");

    let csv = tmp.path().join("emptyid.csv");
    fs::write(&csv, "id,title\n,New title\n").unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["import", "req", csv.to_str().unwrap(), "--update"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing 'id'"));

    // Nothing changed
    assert!(req_file_content(&tmp).contains("Original"));
}

#[test]
fn test_import_update_dry_run_changes_nothing() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Original title", "input");

    let csv = tmp.path().join("update.csv");
    fs::write(&csv, "id,title\nREQ@1,Changed title\n").unwrap();

    tdt()
        .current_dir(tmp.path())
        .args([
            "import",
            "req",
            csv.to_str().unwrap(),
            "--update",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("would update"));

    assert!(req_file_content(&tmp).contains("Original title"));
}

#[test]
fn test_import_update_rejects_wrong_entity_type_id() {
    let tmp = setup_test_project();
    create_test_requirement(&tmp, "Original", "input");

    let csv = tmp.path().join("wrong.csv");
    fs::write(&csv, "id,title\nRISK@1,New title\n").unwrap();

    tdt()
        .current_dir(tmp.path())
        .args(["import", "req", csv.to_str().unwrap(), "--update"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a REQ entity"));
}
