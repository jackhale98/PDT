//! `tdt report` command - Generate engineering reports

use clap::Subcommand;
use console::style;
use miette::{IntoDiagnostic, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::cli::helpers::{format_date_local, truncate_str};
use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::component::Component;
use crate::entities::quote::Quote;
use crate::entities::requirement::Requirement;
use crate::entities::result::{Result as TestResult, Verdict};
use crate::entities::risk::Risk;
use crate::entities::test::Test;

#[derive(Subcommand, Debug)]
pub enum ReportCommands {
    /// Requirements Verification Matrix (RVM)
    Rvm(RvmArgs),

    /// FMEA report sorted by RPN
    Fmea(FmeaArgs),

    /// BOM (Bill of Materials) with costs
    Bom(BomArgs),

    /// Test execution status summary
    TestStatus(TestStatusArgs),

    /// All open issues (NCRs, CAPAs, failed tests)
    OpenIssues(OpenIssuesArgs),
}

#[derive(clap::Args, Debug)]
pub struct RvmArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Show only unverified requirements
    #[arg(long)]
    pub unverified_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct FmeaArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Minimum RPN to include (default: 0)
    #[arg(long, default_value = "0")]
    pub min_rpn: u16,

    /// Only show design risks
    #[arg(long)]
    pub design_only: bool,

    /// Only show process risks
    #[arg(long)]
    pub process_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct BomArgs {
    /// Assembly ID to generate BOM for
    pub assembly_id: String,

    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Include cost rollup
    #[arg(long)]
    pub with_cost: bool,

    /// Include mass rollup
    #[arg(long)]
    pub with_mass: bool,
}

#[derive(clap::Args, Debug)]
pub struct TestStatusArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
pub struct OpenIssuesArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

pub fn run(cmd: ReportCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        ReportCommands::Rvm(args) => run_rvm(args, global),
        ReportCommands::Fmea(args) => run_fmea(args, global),
        ReportCommands::Bom(args) => run_bom(args, global),
        ReportCommands::TestStatus(args) => run_test_status(args, global),
        ReportCommands::OpenIssues(args) => run_open_issues(args, global),
    }
}

/// Requirements Verification Matrix
fn run_rvm(args: RvmArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Load all requirements
    let requirements = load_all_requirements(&project);
    let tests = load_all_tests(&project);
    let results = load_all_results(&project);

    // Build test lookup by ID
    let test_map: HashMap<String, &Test> = tests.iter().map(|t| (t.id.to_string(), t)).collect();

    // Build reverse lookup: which tests verify each requirement (from test.links.verifies)
    let mut tests_verifying_req: HashMap<String, Vec<String>> = HashMap::new();
    for test in &tests {
        for req_id in &test.links.verifies {
            let req_id_str = req_id.to_string();
            tests_verifying_req
                .entry(req_id_str)
                .or_default()
                .push(test.id.to_string());
        }
    }

    // Build result lookup by test ID (latest result for each test)
    let mut latest_results: HashMap<String, &TestResult> = HashMap::new();
    for result in &results {
        let test_id = result.test_id.to_string();
        if let Some(existing) = latest_results.get(&test_id) {
            if result.executed_date > existing.executed_date {
                latest_results.insert(test_id, result);
            }
        } else {
            latest_results.insert(test_id, result);
        }
    }

    // First pass: collect all row data to determine column widths
    struct RvmRow {
        req_short: String,
        req_title: String,
        test_short: String,
        test_title: String,
        result_id: String,
        verdict: String,
        is_verified: bool, // true verification = test passed
    }
    let mut rows: Vec<RvmRow> = Vec::new();

    let mut verified_count = 0; // Has linked tests that passed
    let mut partial_count = 0; // Has linked tests but not all passed
    let mut unverified_count = 0; // No linked tests
    let mut passed_count = 0;
    let mut failed_count = 0;

    for req in &requirements {
        let req_short = short_ids
            .get_short_id(&req.id.to_string())
            .unwrap_or_else(|| req.id.to_string());
        let req_title = req.title.clone();
        let req_id_str = req.id.to_string();

        // Merge links from both directions:
        // 1. req.links.verified_by (tests listed in requirement)
        // 2. test.links.verifies (tests that point to this requirement)
        let mut all_test_ids: std::collections::HashSet<String> = req
            .links
            .verified_by
            .iter()
            .map(|id| id.to_string())
            .collect();

        // Add tests from reverse lookup (test.verifies -> this req)
        if let Some(reverse_tests) = tests_verifying_req.get(&req_id_str) {
            for test_id in reverse_tests {
                all_test_ids.insert(test_id.clone());
            }
        }

        if all_test_ids.is_empty() {
            // No linked tests - truly unverified (always show in both modes)
            rows.push(RvmRow {
                req_short: req_short.clone(),
                req_title: req_title.clone(),
                test_short: "-".to_string(),
                test_title: "(no tests linked)".to_string(),
                result_id: "-".to_string(),
                verdict: "-".to_string(),
                is_verified: false,
            });
            unverified_count += 1;
        } else {
            // Has linked tests - check if they passed
            let mut all_passed = true;
            let mut any_executed = false;

            for test_id_str in all_test_ids {
                let test_short = short_ids
                    .get_short_id(&test_id_str)
                    .unwrap_or_else(|| test_id_str.clone());

                let (test_title, result_id, verdict, test_passed) =
                    if let Some(test) = test_map.get(&test_id_str) {
                        let title = test.title.clone();
                        if let Some(result) = latest_results.get(&test_id_str) {
                            any_executed = true;
                            let result_short = short_ids
                                .get_short_id(&result.id.to_string())
                                .unwrap_or_else(|| result.id.to_string());
                            let (verdict_str, passed) = match result.verdict {
                                Verdict::Pass => {
                                    passed_count += 1;
                                    ("✓ Pass".to_string(), true)
                                }
                                Verdict::Fail => {
                                    failed_count += 1;
                                    all_passed = false;
                                    ("✗ Fail".to_string(), false)
                                }
                                Verdict::Conditional => {
                                    all_passed = false;
                                    ("⚠ Conditional".to_string(), false)
                                }
                                Verdict::Incomplete => {
                                    all_passed = false;
                                    ("… Incomplete".to_string(), false)
                                }
                                Verdict::NotApplicable => ("N/A".to_string(), true),
                            };
                            (title, result_short, verdict_str, passed)
                        } else {
                            all_passed = false;
                            (title, "-".to_string(), "(not executed)".to_string(), false)
                        }
                    } else {
                        all_passed = false;
                        (
                            "(test not found)".to_string(),
                            "-".to_string(),
                            "-".to_string(),
                            false,
                        )
                    };

                if !args.unverified_only {
                    rows.push(RvmRow {
                        req_short: req_short.clone(),
                        req_title: req_title.clone(),
                        test_short,
                        test_title,
                        result_id,
                        verdict,
                        is_verified: test_passed,
                    });
                }
            }

            // Determine requirement verification status
            if any_executed && all_passed {
                verified_count += 1;
            } else if any_executed {
                partial_count += 1;
            } else {
                unverified_count += 1;
            }
        }
    }

    // Filter for unverified_only if requested
    if args.unverified_only {
        rows.retain(|r| !r.is_verified);
    }

    // Calculate column widths (min widths for headers)
    let mut w_req_id = "REQ ID".len();
    let mut w_req_title = "REQ Title".len();
    let mut w_test_id = "Test ID".len();
    let mut w_test_title = "Test Title".len();
    let mut w_result = "Result".len();
    let mut w_verdict = "Verdict".len();

    for row in &rows {
        w_req_id = w_req_id.max(row.req_short.len());
        w_req_title = w_req_title.max(row.req_title.len().min(40)); // Cap at 40
        w_test_id = w_test_id.max(row.test_short.len());
        w_test_title = w_test_title.max(row.test_title.len().min(35)); // Cap at 35
        w_result = w_result.max(row.result_id.len());
        w_verdict = w_verdict.max(row.verdict.len());
    }

    // Generate report with dynamic column widths
    let mut output = String::new();
    output.push_str("# Requirements Verification Matrix (RVM)\n\n");

    // Header
    output.push_str(&format!(
        "| {:<w_req_id$} | {:<w_req_title$} | {:<w_test_id$} | {:<w_test_title$} | {:<w_result$} | {:<w_verdict$} |\n",
        "REQ ID", "REQ Title", "Test ID", "Test Title", "Result", "Verdict",
        w_req_id = w_req_id, w_req_title = w_req_title, w_test_id = w_test_id,
        w_test_title = w_test_title, w_result = w_result, w_verdict = w_verdict
    ));

    // Separator
    output.push_str(&format!(
        "|{:-<w1$}|{:-<w2$}|{:-<w3$}|{:-<w4$}|{:-<w5$}|{:-<w6$}|\n",
        "",
        "",
        "",
        "",
        "",
        "",
        w1 = w_req_id + 2,
        w2 = w_req_title + 2,
        w3 = w_test_id + 2,
        w4 = w_test_title + 2,
        w5 = w_result + 2,
        w6 = w_verdict + 2
    ));

    // Data rows
    for row in &rows {
        output.push_str(&format!(
            "| {:<w_req_id$} | {:<w_req_title$} | {:<w_test_id$} | {:<w_test_title$} | {:<w_result$} | {:<w_verdict$} |\n",
            row.req_short,
            truncate_str(&row.req_title, 25),
            row.test_short,
            truncate_str(&row.test_title, 25),
            row.result_id,
            row.verdict,
            w_req_id = w_req_id, w_req_title = w_req_title, w_test_id = w_test_id,
            w_test_title = w_test_title, w_result = w_result, w_verdict = w_verdict
        ));
    }

    // Summary
    output.push_str("\n## Summary\n\n");
    let total = requirements.len();
    let coverage = if total > 0 {
        (verified_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    output.push_str(&format!("- **Total Requirements:** {}\n", total));
    output.push_str(&format!(
        "- **Verified (all tests pass):** {} ({:.1}%)\n",
        verified_count, coverage
    ));
    output.push_str(&format!(
        "- **Partial (some tests fail):** {}\n",
        partial_count
    ));
    output.push_str(&format!(
        "- **Unverified (no tests or not executed):** {}\n",
        unverified_count
    ));
    output.push_str(&format!("- **Tests Passed:** {}\n", passed_count));
    output.push_str(&format!("- **Tests Failed:** {}\n", failed_count));

    // Output
    write_output(&output, args.output)?;
    Ok(())
}

/// FMEA Report
fn run_fmea(args: FmeaArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Load all risks
    let mut risks = load_all_risks(&project);

    // Filter by type
    if args.design_only {
        risks.retain(|r| r.risk_type == crate::entities::risk::RiskType::Design);
    }
    if args.process_only {
        risks.retain(|r| r.risk_type == crate::entities::risk::RiskType::Process);
    }

    // Filter by min RPN
    risks.retain(|r| r.rpn.unwrap_or(0) >= args.min_rpn);

    // Sort by RPN descending
    risks.sort_by(|a, b| b.rpn.unwrap_or(0).cmp(&a.rpn.unwrap_or(0)));

    // First pass: collect all row data
    struct FmeaRow {
        id: String,
        failure_mode: String,
        cause: String,
        effect: String,
        s: String,
        o: String,
        d: String,
        rpn: String,
        level: String,
        mitigations: String,
    }
    let mut rows: Vec<FmeaRow> = Vec::new();
    let mut total_rpn: u32 = 0;
    let mut by_level: HashMap<String, usize> = HashMap::new();

    for risk in &risks {
        let risk_short = short_ids
            .get_short_id(&risk.id.to_string())
            .unwrap_or_else(|| risk.id.to_string());
        let failure_mode =
            truncate_str(risk.failure_mode.as_deref().unwrap_or("-"), 20).to_string();
        let cause = truncate_str(risk.cause.as_deref().unwrap_or("-"), 15).to_string();
        let effect = truncate_str(risk.effect.as_deref().unwrap_or("-"), 15).to_string();
        let s = risk.severity.map_or("-".to_string(), |v| v.to_string());
        let o = risk.occurrence.map_or("-".to_string(), |v| v.to_string());
        let d = risk.detection.map_or("-".to_string(), |v| v.to_string());
        let rpn = risk.rpn.map_or("-".to_string(), |v| v.to_string());
        let level = risk.risk_level.map_or("-".to_string(), |l| l.to_string());
        let mitigations = if risk.mitigations.is_empty() {
            "None".to_string()
        } else {
            format!("{} action(s)", risk.mitigations.len())
        };

        if let Some(rpn_val) = risk.rpn {
            total_rpn += rpn_val as u32;
        }

        if let Some(ref lvl) = risk.risk_level {
            *by_level.entry(lvl.to_string()).or_insert(0) += 1;
        }

        rows.push(FmeaRow {
            id: risk_short,
            failure_mode,
            cause,
            effect,
            s,
            o,
            d,
            rpn,
            level,
            mitigations,
        });
    }

    // Calculate column widths (min widths for headers)
    let mut w_id = "ID".len();
    let mut w_fm = "Failure Mode".len();
    let mut w_cause = "Cause".len();
    let mut w_effect = "Effect".len();
    let mut w_s = "S".len();
    let mut w_o = "O".len();
    let mut w_d = "D".len();
    let mut w_rpn = "RPN".len();
    let mut w_level = "Level".len();
    let mut w_mit = "Mitigations".len();

    for row in &rows {
        w_id = w_id.max(row.id.len());
        w_fm = w_fm.max(row.failure_mode.len());
        w_cause = w_cause.max(row.cause.len());
        w_effect = w_effect.max(row.effect.len());
        w_s = w_s.max(row.s.len());
        w_o = w_o.max(row.o.len());
        w_d = w_d.max(row.d.len());
        w_rpn = w_rpn.max(row.rpn.len());
        w_level = w_level.max(row.level.len());
        w_mit = w_mit.max(row.mitigations.len());
    }

    // Generate report with dynamic column widths
    let mut output = String::new();
    output.push_str("# FMEA Report\n\n");

    // Header
    output.push_str(&format!(
        "| {:<w_id$} | {:<w_fm$} | {:<w_cause$} | {:<w_effect$} | {:<w_s$} | {:<w_o$} | {:<w_d$} | {:<w_rpn$} | {:<w_level$} | {:<w_mit$} |\n",
        "ID", "Failure Mode", "Cause", "Effect", "S", "O", "D", "RPN", "Level", "Mitigations",
        w_id = w_id, w_fm = w_fm, w_cause = w_cause, w_effect = w_effect,
        w_s = w_s, w_o = w_o, w_d = w_d, w_rpn = w_rpn, w_level = w_level, w_mit = w_mit
    ));

    // Separator
    output.push_str(&format!(
        "|{:-<w1$}|{:-<w2$}|{:-<w3$}|{:-<w4$}|{:-<w5$}|{:-<w6$}|{:-<w7$}|{:-<w8$}|{:-<w9$}|{:-<w10$}|\n",
        "", "", "", "", "", "", "", "", "", "",
        w1 = w_id + 2, w2 = w_fm + 2, w3 = w_cause + 2, w4 = w_effect + 2,
        w5 = w_s + 2, w6 = w_o + 2, w7 = w_d + 2, w8 = w_rpn + 2, w9 = w_level + 2, w10 = w_mit + 2
    ));

    // Data rows
    for row in &rows {
        output.push_str(&format!(
            "| {:<w_id$} | {:<w_fm$} | {:<w_cause$} | {:<w_effect$} | {:<w_s$} | {:<w_o$} | {:<w_d$} | {:<w_rpn$} | {:<w_level$} | {:<w_mit$} |\n",
            row.id, row.failure_mode, row.cause, row.effect,
            row.s, row.o, row.d, row.rpn, row.level, row.mitigations,
            w_id = w_id, w_fm = w_fm, w_cause = w_cause, w_effect = w_effect,
            w_s = w_s, w_o = w_o, w_d = w_d, w_rpn = w_rpn, w_level = w_level, w_mit = w_mit
        ));
    }

    // Summary
    output.push_str("\n## Summary\n\n");
    output.push_str(&format!("- **Total Risks:** {}\n", risks.len()));
    if !risks.is_empty() {
        output.push_str(&format!(
            "- **Average RPN:** {:.1}\n",
            total_rpn as f64 / risks.len() as f64
        ));
    }
    output.push_str(&format!(
        "- **Critical:** {}\n",
        by_level.get("critical").unwrap_or(&0)
    ));
    output.push_str(&format!(
        "- **High:** {}\n",
        by_level.get("high").unwrap_or(&0)
    ));
    output.push_str(&format!(
        "- **Medium:** {}\n",
        by_level.get("medium").unwrap_or(&0)
    ));
    output.push_str(&format!(
        "- **Low:** {}\n",
        by_level.get("low").unwrap_or(&0)
    ));

    let unmitigated = risks.iter().filter(|r| r.mitigations.is_empty()).count();
    output.push_str(&format!("- **Unmitigated:** {}\n", unmitigated));

    // Output
    write_output(&output, args.output)?;
    Ok(())
}

/// BOM Report
fn run_bom(args: BomArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let resolved_id = short_ids
        .resolve(&args.assembly_id)
        .unwrap_or_else(|| args.assembly_id.clone());

    // Load assembly
    let assembly = load_assembly(&project, &resolved_id)?;

    // Load all components for lookup
    let components = load_all_components(&project);
    let component_map: HashMap<String, &Component> =
        components.iter().map(|c| (c.id.to_string(), c)).collect();

    // Load all assemblies for subassembly lookup
    let assemblies = load_all_assemblies(&project);
    let assembly_map: HashMap<String, &crate::entities::assembly::Assembly> =
        assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

    // Load quotes for price lookup (used when --with-cost)
    let quotes = load_all_quotes(&project);
    let quote_map: HashMap<String, &Quote> = quotes.iter().map(|q| (q.id.to_string(), q)).collect();

    // Generate indented BOM
    let mut output = String::new();
    output.push_str(&format!("# Bill of Materials: {}\n\n", assembly.title));
    output.push_str(&format!("Assembly ID: {}\n", assembly.id));
    output.push_str(&format!("Part Number: {}\n\n", assembly.part_number));

    let mut total_cost = 0.0;
    let mut total_mass = 0.0;

    output.push_str("```\n");

    // Recursively print BOM
    fn print_bom_item(
        output: &mut String,
        component_map: &HashMap<String, &Component>,
        assembly_map: &HashMap<String, &crate::entities::assembly::Assembly>,
        quote_map: &HashMap<String, &Quote>,
        short_ids: &ShortIdIndex,
        bom: &[crate::entities::assembly::BomItem],
        indent: usize,
        total_cost: &mut f64,
        total_mass: &mut f64,
        with_cost: bool,
        with_mass: bool,
        visited: &mut std::collections::HashSet<String>,
    ) {
        let prefix = "│  ".repeat(indent);
        for (i, item) in bom.iter().enumerate() {
            let is_last = i == bom.len() - 1;
            let branch = if is_last { "└─ " } else { "├─ " };

            let item_id = item.component_id.to_string();
            let item_short = short_ids
                .get_short_id(&item_id)
                .unwrap_or_else(|| item_id.clone());

            // Check if it's a component or subassembly
            if let Some(cmp) = component_map.get(&item_id) {
                let cost_str = if with_cost {
                    // Priority 1: Use selected quote if set
                    let unit_price = if let Some(ref quote_id) = cmp.selected_quote {
                        if let Some(quote) = quote_map.get(quote_id) {
                            quote.price_for_qty(item.quantity).unwrap_or(0.0)
                        } else {
                            cmp.unit_cost.unwrap_or(0.0)
                        }
                    } else {
                        // Priority 2: Fall back to unit_cost
                        cmp.unit_cost.unwrap_or(0.0)
                    };

                    if unit_price > 0.0 {
                        let line_cost = unit_price * item.quantity as f64;
                        *total_cost += line_cost;
                        format!(" ${:.2}", line_cost)
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };

                let mass_str = if with_mass {
                    cmp.mass_kg.map_or("".to_string(), |m| {
                        let line_mass = m * item.quantity as f64;
                        *total_mass += line_mass;
                        format!(" {:.3}kg", line_mass)
                    })
                } else {
                    "".to_string()
                };

                output.push_str(&format!(
                    "{}{}{}: {} (qty: {}){}{}\n",
                    prefix, branch, item_short, cmp.title, item.quantity, cost_str, mass_str
                ));
            } else if let Some(asm) = assembly_map.get(&item_id) {
                // Subassembly - check for cycles
                if visited.contains(&item_id) {
                    output.push_str(&format!(
                        "{}{}{}: {} (qty: {}) [CYCLE DETECTED]\n",
                        prefix, branch, item_short, asm.title, item.quantity
                    ));
                } else {
                    output.push_str(&format!(
                        "{}{}{}: {} (qty: {})\n",
                        prefix, branch, item_short, asm.title, item.quantity
                    ));

                    visited.insert(item_id.clone());
                    print_bom_item(
                        output,
                        component_map,
                        assembly_map,
                        quote_map,
                        short_ids,
                        &asm.bom,
                        indent + 1,
                        total_cost,
                        total_mass,
                        with_cost,
                        with_mass,
                        visited,
                    );
                    visited.remove(&item_id);
                }
            } else {
                output.push_str(&format!(
                    "{}{}{}: (not found) (qty: {})\n",
                    prefix, branch, item_short, item.quantity
                ));
            }
        }
    }

    let mut visited = std::collections::HashSet::new();
    visited.insert(assembly.id.to_string());
    print_bom_item(
        &mut output,
        &component_map,
        &assembly_map,
        &quote_map,
        &short_ids,
        &assembly.bom,
        0,
        &mut total_cost,
        &mut total_mass,
        args.with_cost,
        args.with_mass,
        &mut visited,
    );

    output.push_str("```\n");

    // Totals
    if args.with_cost {
        output.push_str(&format!("\n**Total Cost:** ${:.2}\n", total_cost));
    }
    if args.with_mass {
        output.push_str(&format!("**Total Mass:** {:.3} kg\n", total_mass));
    }

    write_output(&output, args.output)?;
    Ok(())
}

/// Test Status Report
fn run_test_status(args: TestStatusArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    let tests = load_all_tests(&project);
    let results = load_all_results(&project);

    // Build latest result for each test
    let mut latest_results: HashMap<String, &TestResult> = HashMap::new();
    for result in &results {
        let test_id = result.test_id.to_string();
        if let Some(existing) = latest_results.get(&test_id) {
            if result.executed_date > existing.executed_date {
                latest_results.insert(test_id, result);
            }
        } else {
            latest_results.insert(test_id, result);
        }
    }

    // Categorize tests
    let mut executed = 0;
    let mut pending = 0;
    let mut passed = 0;
    let mut failed = 0;
    let mut conditional = 0;
    let mut recent_failures: Vec<(&Test, &TestResult)> = Vec::new();

    for test in &tests {
        let test_id = test.id.to_string();
        if let Some(result) = latest_results.get(&test_id) {
            executed += 1;
            match result.verdict {
                Verdict::Pass => passed += 1,
                Verdict::Fail => {
                    failed += 1;
                    recent_failures.push((test, result));
                }
                Verdict::Conditional => conditional += 1,
                Verdict::Incomplete | Verdict::NotApplicable => {}
            }
        } else {
            pending += 1;
        }
    }

    // Sort failures by date (most recent first)
    recent_failures.sort_by(|a, b| b.1.executed_date.cmp(&a.1.executed_date));
    recent_failures.truncate(10);

    // Generate report
    let mut output = String::new();
    output.push_str("# Test Execution Status Report\n\n");

    output.push_str("## Summary\n\n");
    output.push_str(&"| Metric | Count |\n".to_string());
    output.push_str(&"|--------|-------|\n".to_string());
    output.push_str(&format!("| Total Protocols | {} |\n", tests.len()));
    output.push_str(&format!("| Executed | {} |\n", executed));
    output.push_str(&format!("| Pending | {} |\n", pending));
    output.push_str(&format!("| Passed | {} |\n", passed));
    output.push_str(&format!("| Failed | {} |\n", failed));
    output.push_str(&format!("| Conditional | {} |\n", conditional));

    if executed > 0 {
        let pass_rate = (passed as f64 / executed as f64) * 100.0;
        output.push_str(&format!("| Pass Rate | {:.1}% |\n", pass_rate));
    }

    if !recent_failures.is_empty() {
        output.push_str("\n## Recent Failures\n\n");
        output.push_str("| Test ID | Title | Execution Date |\n");
        output.push_str("|---------|-------|----------------|\n");
        for (test, result) in &recent_failures {
            let test_short = short_ids
                .get_short_id(&test.id.to_string())
                .unwrap_or_else(|| test.id.to_string());
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                test_short,
                truncate_str(&test.title, 40),
                format_date_local(&result.executed_date)
            ));
        }
    }

    write_output(&output, args.output)?;
    Ok(())
}

/// Open Issues Report
fn run_open_issues(args: OpenIssuesArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Load NCRs
    let ncrs = load_all_ncrs(&project);
    let open_ncrs: Vec<_> = ncrs
        .iter()
        .filter(|n| n.ncr_status != crate::entities::ncr::NcrStatus::Closed)
        .collect();

    // Load CAPAs
    let capas = load_all_capas(&project);
    let open_capas: Vec<_> = capas
        .iter()
        .filter(|c| c.capa_status != crate::entities::capa::CapaStatus::Closed)
        .collect();

    // Load test failures
    let tests = load_all_tests(&project);
    let results = load_all_results(&project);
    let mut latest_results: HashMap<String, &TestResult> = HashMap::new();
    for result in &results {
        let test_id = result.test_id.to_string();
        if let Some(existing) = latest_results.get(&test_id) {
            if result.executed_date > existing.executed_date {
                latest_results.insert(test_id, result);
            }
        } else {
            latest_results.insert(test_id, result);
        }
    }

    let failed_tests: Vec<_> = tests
        .iter()
        .filter(|t| {
            latest_results
                .get(&t.id.to_string())
                .is_some_and(|r| r.verdict == Verdict::Fail)
        })
        .collect();

    // Generate report
    let mut output = String::new();
    output.push_str("# Open Issues Report\n\n");

    // Summary
    output.push_str("## Summary\n\n");
    output.push_str(&"| Category | Count |\n".to_string());
    output.push_str(&"|----------|-------|\n".to_string());
    output.push_str(&format!("| Open NCRs | {} |\n", open_ncrs.len()));
    output.push_str(&format!("| Open CAPAs | {} |\n", open_capas.len()));
    output.push_str(&format!("| Failed Tests | {} |\n", failed_tests.len()));

    // Open NCRs
    if !open_ncrs.is_empty() {
        output.push_str("\n## Open NCRs\n\n");
        output.push_str("| ID | Title | Severity | Status |\n");
        output.push_str("|----|-------|----------|--------|\n");
        for ncr in &open_ncrs {
            let ncr_short = short_ids
                .get_short_id(&ncr.id.to_string())
                .unwrap_or_else(|| ncr.id.to_string());
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                ncr_short,
                truncate_str(&ncr.title, 30),
                ncr.severity,
                ncr.ncr_status
            ));
        }
    }

    // Open CAPAs
    if !open_capas.is_empty() {
        output.push_str("\n## Open CAPAs\n\n");
        output.push_str("| ID | Title | Type | Status |\n");
        output.push_str("|----|-------|------|--------|\n");
        for capa in &open_capas {
            let capa_short = short_ids
                .get_short_id(&capa.id.to_string())
                .unwrap_or_else(|| capa.id.to_string());
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                capa_short,
                truncate_str(&capa.title, 30),
                capa.capa_type,
                capa.capa_status
            ));
        }
    }

    // Failed Tests
    if !failed_tests.is_empty() {
        output.push_str("\n## Failed Tests\n\n");
        output.push_str("| ID | Title | Type |\n");
        output.push_str("|----|-------|------|\n");
        for test in &failed_tests {
            let test_short = short_ids
                .get_short_id(&test.id.to_string())
                .unwrap_or_else(|| test.id.to_string());
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                test_short,
                truncate_str(&test.title, 40),
                test.test_type
            ));
        }
    }

    write_output(&output, args.output)?;
    Ok(())
}

// Helper functions

fn write_output(content: &str, output_path: Option<PathBuf>) -> Result<()> {
    if let Some(path) = output_path {
        let file = File::create(&path).into_diagnostic()?;
        let mut writer = BufWriter::new(file);
        writer.write_all(content.as_bytes()).into_diagnostic()?;
        println!(
            "{} Report written to {}",
            style("✓").green(),
            style(path.display()).cyan()
        );
    } else {
        print!("{}", content);
    }
    Ok(())
}

fn load_all_requirements(project: &Project) -> Vec<Requirement> {
    let mut reqs = Vec::new();

    for subdir in &["inputs", "outputs"] {
        let dir = project.root().join(format!("requirements/{}", subdir));
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                    reqs.push(req);
                }
            }
        }
    }

    reqs
}

fn load_all_tests(project: &Project) -> Vec<Test> {
    let mut tests = Vec::new();

    for subdir in &["verification/protocols", "validation/protocols"] {
        let dir = project.root().join(subdir);
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(entry.path()) {
                    tests.push(test);
                }
            }
        }
    }

    tests
}

fn load_all_results(project: &Project) -> Vec<TestResult> {
    let mut results = Vec::new();

    for subdir in &["verification/results", "validation/results"] {
        let dir = project.root().join(subdir);
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(result) = crate::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                    results.push(result);
                }
            }
        }
    }

    results
}

fn load_all_risks(project: &Project) -> Vec<Risk> {
    let mut risks = Vec::new();

    for subdir in &["design", "process"] {
        let dir = project.root().join(format!("risks/{}", subdir));
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                    risks.push(risk);
                }
            }
        }
    }

    risks
}

fn load_all_components(project: &Project) -> Vec<Component> {
    let mut components = Vec::new();
    let dir = project.root().join("bom/components");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(cmp) = crate::yaml::parse_yaml_file::<Component>(entry.path()) {
                components.push(cmp);
            }
        }
    }

    components
}

fn load_all_assemblies(project: &Project) -> Vec<crate::entities::assembly::Assembly> {
    let mut assemblies = Vec::new();
    let dir = project.root().join("bom/assemblies");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) =
                crate::yaml::parse_yaml_file::<crate::entities::assembly::Assembly>(entry.path())
            {
                assemblies.push(asm);
            }
        }
    }

    assemblies
}

fn load_all_quotes(project: &Project) -> Vec<Quote> {
    let mut quotes = Vec::new();

    // Check both sourcing/quotes and bom/quotes directories
    for dir_path in &["sourcing/quotes", "bom/quotes"] {
        let dir = project.root().join(dir_path);
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(quote) = crate::yaml::parse_yaml_file::<Quote>(entry.path()) {
                    quotes.push(quote);
                }
            }
        }
    }

    quotes
}

fn load_assembly(project: &Project, id: &str) -> Result<crate::entities::assembly::Assembly> {
    let dir = project.root().join("bom/assemblies");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) =
                crate::yaml::parse_yaml_file::<crate::entities::assembly::Assembly>(entry.path())
            {
                if asm.id.to_string() == id || asm.id.to_string().starts_with(id) {
                    return Ok(asm);
                }
            }
        }
    }

    Err(miette::miette!("Assembly not found: {}", id))
}

fn load_all_ncrs(project: &Project) -> Vec<crate::entities::ncr::Ncr> {
    let mut ncrs = Vec::new();
    let dir = project.root().join("manufacturing/ncrs");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(ncr) = crate::yaml::parse_yaml_file::<crate::entities::ncr::Ncr>(entry.path())
            {
                ncrs.push(ncr);
            }
        }
    }

    ncrs
}

fn load_all_capas(project: &Project) -> Vec<crate::entities::capa::Capa> {
    let mut capas = Vec::new();
    let dir = project.root().join("manufacturing/capas");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(capa) =
                crate::yaml::parse_yaml_file::<crate::entities::capa::Capa>(entry.path())
            {
                capas.push(capa);
            }
        }
    }

    capas
}
