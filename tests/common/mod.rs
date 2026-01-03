//! Shared test helpers for integration tests
//!
//! This module provides common utilities used across all test files.

#![allow(dead_code)]

use assert_cmd::cargo;
use assert_cmd::Command;
use tempfile::TempDir;

/// Helper to get a tdt command
pub fn tdt() -> Command {
    Command::new(cargo::cargo_bin!("tdt"))
}

/// Helper to create a test project in a temp directory
pub fn setup_test_project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    tdt().current_dir(tmp.path()).arg("init").assert().success();
    tmp
}

/// Helper to create a test requirement
pub fn create_test_requirement(tmp: &TempDir, title: &str, req_type: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "req",
            "new",
            "--title",
            title,
            "--type",
            req_type,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("REQ-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("REQ-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test risk
pub fn create_test_risk(tmp: &TempDir, title: &str, risk_type: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "risk",
            "new",
            "--title",
            title,
            "--type",
            risk_type,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("RISK-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("RISK-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test component
pub fn create_test_component(tmp: &TempDir, part_number: &str, title: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "cmp",
            "new",
            "--part-number",
            part_number,
            "--title",
            title,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("CMP-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("CMP-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test supplier
pub fn create_test_supplier(tmp: &TempDir, name: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args(["sup", "new", "--name", name, "--no-edit"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("SUP-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("SUP-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test feature
pub fn create_test_feature(
    tmp: &TempDir,
    component_short_id: &str,
    feature_type: &str,
    title: &str,
) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "feat",
            "new",
            "--component",
            component_short_id,
            "--feature-type",
            feature_type,
            "--title",
            title,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("FEAT-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("FEAT-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test protocol
pub fn create_test_protocol(tmp: &TempDir, title: &str, test_type: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "test",
            "new",
            "--title",
            title,
            "--type",
            test_type,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("TEST-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("TEST-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test assembly
pub fn create_test_assembly(tmp: &TempDir, part_number: &str, title: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "asm",
            "new",
            "--part-number",
            part_number,
            "--title",
            title,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("ASM-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("ASM-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}

/// Helper to create a test hazard
pub fn create_test_hazard(tmp: &TempDir, title: &str, category: &str) -> String {
    let output = tdt()
        .current_dir(tmp.path())
        .args([
            "haz",
            "new",
            "--title",
            title,
            "--category",
            category,
            "--no-edit",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.contains("HAZ-"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("HAZ-")))
        .map(|s| s.trim_end_matches("...").to_string())
        .unwrap_or_default()
}
