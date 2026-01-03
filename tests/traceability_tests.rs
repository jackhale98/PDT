//! Traceability tests - Links, Traces, DSM, Reports

mod common;

use common::{
    create_test_component, create_test_protocol, create_test_requirement, create_test_risk,
    setup_test_project, tdt,
};
use predicates::prelude::*;

// ============================================================================
// Link Command Tests
// ============================================================================

#[test]
fn test_link_check_empty_project() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .args(["link", "check"])
        .assert()
        .success();
}

#[test]
fn test_link_add_verified_by() {
    let tmp = setup_test_project();

    // Create a requirement and a test
    let req_id = create_test_requirement(&tmp, "Req for Link", "input");
    let test_id = create_test_protocol(&tmp, "Test for Link", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("Added link"));

        // Verify link exists
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("verified_by"));
    }
}

#[test]
fn test_link_add_mitigated_by() {
    let tmp = setup_test_project();

    // Create a risk and a component
    let risk_id = create_test_risk(&tmp, "Risk for Link", "design");
    let cmp_id = create_test_component(&tmp, "PART-002", "Component for Link");

    if !risk_id.is_empty() && !cmp_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &risk_id, "--type", "mitigated_by", &cmp_id])
            .assert()
            .success();

        // Verify link exists
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &risk_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("mitigated_by"));
    }
}

#[test]
fn test_link_remove() {
    let tmp = setup_test_project();

    // Create a requirement and a test
    let req_id = create_test_requirement(&tmp, "Req for Remove", "input");
    let test_id = create_test_protocol(&tmp, "Test for Remove", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success();

        // Remove link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "remove", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("Removed link"));

        // Verify link is gone
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req_id])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("No links")
                    .or(predicate::str::contains("verified_by").not()),
            );
    }
}

#[test]
fn test_link_show_no_links() {
    let tmp = setup_test_project();

    // Create a requirement with no links
    let req_id = create_test_requirement(&tmp, "No Links Req", "input");

    if !req_id.is_empty() {
        // Show should indicate no links
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req_id])
            .assert()
            .success();
    }
}

#[test]
fn test_link_bidirectional() {
    let tmp = setup_test_project();

    // Create two requirements
    let req1_id = create_test_requirement(&tmp, "Parent Req", "input");
    let req2_id = create_test_requirement(&tmp, "Child Req", "output");

    if !req1_id.is_empty() && !req2_id.is_empty() {
        // Add derives_from link (child derives from parent)
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req2_id, "--type", "derives_from", &req1_id])
            .assert()
            .success();

        // Check child has derives_from
        tdt()
            .current_dir(tmp.path())
            .args(["link", "show", &req2_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("derives_from"));
    }
}

// ============================================================================
// Trace Command Tests
// ============================================================================

#[test]
fn test_trace_help() {
    tdt()
        .args(["trace", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("trace"));
}

#[test]
fn test_trace_matrix_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "matrix"])
        .assert()
        .success();
}

#[test]
fn test_trace_orphans_empty_project() {
    let tmp = setup_test_project();
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "orphans"])
        .assert()
        .success();
}

#[test]
fn test_trace_from_requirement() {
    let tmp = setup_test_project();
    let req_id = create_test_requirement(&tmp, "Test Req", "input");

    tdt()
        .current_dir(tmp.path())
        .args(["trace", "from", &req_id])
        .assert()
        .success();
}

#[test]
fn test_trace_to_requirement() {
    let tmp = setup_test_project();
    let req_id = create_test_requirement(&tmp, "Test Req", "input");

    tdt()
        .current_dir(tmp.path())
        .args(["trace", "to", &req_id])
        .assert()
        .success();
}

#[test]
fn test_trace_from_with_linked_entities() {
    let tmp = setup_test_project();

    // Create linked entities
    let req_id = create_test_requirement(&tmp, "Trace Source", "input");
    let test_id = create_test_protocol(&tmp, "Trace Target", "verification");

    if !req_id.is_empty() && !test_id.is_empty() {
        // Add link
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success();

        // Trace from requirement
        tdt()
            .current_dir(tmp.path())
            .args(["trace", "from", &req_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("Trace Source"));
    }
}

#[test]
fn test_trace_to_unlinked_entity() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Trace To Target", "input");

    if !req_id.is_empty() {
        // Trace to requirement (shows what points to it)
        tdt()
            .current_dir(tmp.path())
            .args(["trace", "to", &req_id])
            .assert()
            .success();
    }
}

#[test]
fn test_trace_orphans_with_requirements() {
    let tmp = setup_test_project();

    // Create some requirements (they will be orphaned since not linked)
    create_test_requirement(&tmp, "Orphan Req 1", "input");
    create_test_requirement(&tmp, "Orphan Req 2", "input");

    // Check orphans - unlinked requirements should appear
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "orphans"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Orphan Req 1"))
        .stdout(predicate::str::contains("Orphan Req 2"));
}

// ============================================================================
// Trace DOT Output Tests
// ============================================================================

#[test]
fn test_trace_from_dot_output() {
    let tmp = setup_test_project();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Test Requirement",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to get short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Test DOT output
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "from", "REQ@1", "--output", "dot"])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph trace_from"))
        .stdout(predicate::str::contains("rankdir=TB"))
        .stdout(predicate::str::contains("node [shape=box]"));
}

#[test]
fn test_trace_to_dot_output() {
    let tmp = setup_test_project();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            "Test Requirement",
            "--type",
            "input",
            "--no-edit",
        ])
        .assert()
        .success();

    // List to get short IDs
    tdt()
        .current_dir(tmp.path())
        .args(["req", "list"])
        .assert()
        .success();

    // Test DOT output
    tdt()
        .current_dir(tmp.path())
        .args(["trace", "to", "REQ@1", "--output", "dot"])
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph trace_to"))
        .stdout(predicate::str::contains("rankdir=TB"))
        .stdout(predicate::str::contains("fillcolor=lightblue"));
}

// ============================================================================
// Where-Used Command Tests
// ============================================================================

#[test]
fn test_where_used_component() {
    let tmp = setup_test_project();

    let cmp_id = create_test_component(&tmp, "PART-003", "Where Used Test");

    if !cmp_id.is_empty() {
        // Where-used on component
        tdt()
            .current_dir(tmp.path())
            .args(["where-used", &cmp_id])
            .assert()
            .success();
    }
}

#[test]
fn test_where_used_no_references() {
    let tmp = setup_test_project();

    let req_id = create_test_requirement(&tmp, "Orphan Req", "input");

    if !req_id.is_empty() {
        // Where-used on unreferenced entity
        tdt()
            .current_dir(tmp.path())
            .args(["where-used", &req_id])
            .assert()
            .success();
    }
}

// ============================================================================
// DSM (Design Structure Matrix) Tests
// ============================================================================

#[test]
fn test_dsm_help() {
    tdt()
        .args(["dsm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

#[test]
fn test_dsm_no_components() {
    let tmp = setup_test_project();

    tdt()
        .current_dir(tmp.path())
        .arg("dsm")
        .assert()
        .success()
        .stdout(predicate::str::contains("No components found"));
}

#[test]
fn test_dsm_with_components() {
    let tmp = setup_test_project();

    // Create two components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Bracket", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM - should show 2 components
    tdt()
        .current_dir(tmp.path())
        .arg("dsm")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 components"));
}

#[test]
fn test_dsm_csv_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with CSV output
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-o", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Component,CMP@1"));
}

#[test]
fn test_dsm_json_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with JSON output
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-o", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"components\""))
        .stdout(predicate::str::contains("\"relationships\""));
}

#[test]
fn test_dsm_clustering() {
    let tmp = setup_test_project();

    // Create two components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Bracket", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with clustering
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-c"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Clustered"))
        .stdout(predicate::str::contains("Cluster"));
}

#[test]
fn test_dsm_rel_type_filter() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with mate filter only
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "-t", "mate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

#[test]
fn test_dsm_weighted_flag() {
    let tmp = setup_test_project();

    // Create two components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Shaft", "--no-edit"])
        .assert()
        .success();

    // Create a process that produces both components (creates relationship)
    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "Assembly Process",
            "--type",
            "assembly",
            "--op-number",
            "OP-010",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with weighted flag - should show numeric values
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "--weighted"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

#[test]
fn test_dsm_metrics_flag() {
    let tmp = setup_test_project();

    // Create components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Shaft", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with metrics flag - should show coupling statistics
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "--metrics"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Coupling Metrics"))
        .stdout(predicate::str::contains("Fan-in"))
        .stdout(predicate::str::contains("Fan-out"));
}

#[test]
fn test_dsm_cycles_flag() {
    let tmp = setup_test_project();

    // Create components
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-002", "-t", "Shaft", "--no-edit"])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DSM with cycles flag
    tdt()
        .current_dir(tmp.path())
        .args(["dsm", "--cycles"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Design Structure Matrix"));
}

// ============================================================================
// DMM (Domain Mapping Matrix) Tests
// ============================================================================

#[test]
fn test_dmm_help() {
    tdt()
        .args(["dmm", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Domain Mapping Matrix"));
}

#[test]
fn test_dmm_cmp_req() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--type",
            "input",
            "-T",
            "Force Requirement",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM for components vs requirements
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "req"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Domain Mapping Matrix"));
}

#[test]
fn test_dmm_cmp_proc() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a process
    tdt()
        .current_dir(tmp.path())
        .args([
            "proc",
            "new",
            "--title",
            "Machining",
            "--type",
            "machining",
            "--op-number",
            "OP-010",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM for components vs processes
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "proc"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Domain Mapping Matrix"));
}

#[test]
fn test_dmm_csv_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--type",
            "input",
            "-T",
            "Test Req",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM with CSV output
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "req", "-o", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains(",REQ@1"));
}

#[test]
fn test_dmm_json_output() {
    let tmp = setup_test_project();

    // Create a component
    tdt()
        .current_dir(tmp.path())
        .args(["cmp", "new", "-p", "PN-001", "-t", "Housing", "--no-edit"])
        .assert()
        .success();

    // Create a requirement
    tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--type",
            "input",
            "-T",
            "Test Req",
            "--no-edit",
        ])
        .assert()
        .success();

    // Rebuild cache
    tdt()
        .current_dir(tmp.path())
        .args(["cache", "rebuild"])
        .assert()
        .success();

    // Run DMM with JSON output
    tdt()
        .current_dir(tmp.path())
        .args(["dmm", "cmp", "req", "-o", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"row_entities\""))
        .stdout(predicate::str::contains("\"col_entities\""));
}

// ============================================================================
// Report Command Tests
// ============================================================================

#[test]
fn test_report_rvm_empty_project() {
    let tmp = setup_test_project();

    // RVM on empty project should succeed with empty output
    tdt()
        .current_dir(tmp.path())
        .args(["report", "rvm"])
        .assert()
        .success();
}

#[test]
fn test_report_rvm_with_requirements() {
    let tmp = setup_test_project();

    // Create a requirement
    create_test_requirement(&tmp, "Test Requirement", "input");

    // RVM should show the requirement
    tdt()
        .current_dir(tmp.path())
        .args(["report", "rvm"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Requirement"));
}

#[test]
fn test_report_rvm_with_linked_test() {
    let tmp = setup_test_project();

    // Create a requirement
    let req_id = create_test_requirement(&tmp, "Linked Requirement", "input");

    // Create a test
    let test_id = create_test_protocol(&tmp, "Verification Test", "verification");

    // Link them
    if !req_id.is_empty() && !test_id.is_empty() {
        tdt()
            .current_dir(tmp.path())
            .args(["link", "add", &req_id, "--type", "verified_by", &test_id])
            .assert()
            .success();

        // RVM should show the linked test
        tdt()
            .current_dir(tmp.path())
            .args(["report", "rvm"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Linked Requirement"));
    }
}

#[test]
fn test_report_fmea_empty_project() {
    let tmp = setup_test_project();

    // FMEA on empty project should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["report", "fmea"])
        .assert()
        .success();
}

#[test]
fn test_report_fmea_with_risks() {
    let tmp = setup_test_project();

    // Create a risk
    create_test_risk(&tmp, "Test Risk", "design");

    // FMEA should show the risk (check for total count and risk ID)
    tdt()
        .current_dir(tmp.path())
        .args(["report", "fmea"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Total Risks"))
        .stdout(predicate::str::contains("RISK@1").or(predicate::str::contains("RISK-")));
}

#[test]
fn test_report_bom_with_assembly() {
    let tmp = setup_test_project();

    // Create an assembly
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-BOM-001",
            "--title",
            "BOM Test Assembly",
            "--no-edit",
            "--output",
            "id",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let asm_id = stdout.trim();

    if !asm_id.is_empty() && asm_id.starts_with("ASM-") {
        // BOM report for assembly should succeed
        tdt()
            .current_dir(tmp.path())
            .args(["report", "bom", asm_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("BOM Test Assembly"));
    }
}

#[test]
fn test_report_bom_with_components_in_assembly() {
    let tmp = setup_test_project();

    // Create a component
    create_test_component(&tmp, "PART-BOM-001", "BOM Component");

    // Create an assembly
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            "ASM-BOM-002",
            "--title",
            "Assembly With Parts",
            "--no-edit",
            "--output",
            "id",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let asm_id = stdout.trim();

    if !asm_id.is_empty() && asm_id.starts_with("ASM-") {
        // BOM report should work (even if empty of components)
        tdt()
            .current_dir(tmp.path())
            .args(["report", "bom", asm_id])
            .assert()
            .success();
    }
}

#[test]
fn test_report_test_status_empty() {
    let tmp = setup_test_project();

    // Test status on empty project should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["report", "test-status"])
        .assert()
        .success();
}

#[test]
fn test_report_open_issues_empty() {
    let tmp = setup_test_project();

    // Open issues on empty project should succeed
    tdt()
        .current_dir(tmp.path())
        .args(["report", "open-issues"])
        .assert()
        .success();
}

// ============================================================================
// Schema Command Tests
// ============================================================================

#[test]
fn test_schema_list() {
    tdt()
        .args(["schema", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("req"));
}

#[test]
fn test_schema_show_req() {
    tdt()
        .args(["schema", "show", "req"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

#[test]
fn test_schema_show_raw_json() {
    tdt()
        .args(["schema", "show", "req", "--raw"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"$schema\""))
        .stdout(predicate::str::contains("\"properties\""));
}

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
