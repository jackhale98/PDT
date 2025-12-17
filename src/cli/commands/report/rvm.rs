//! Requirements Verification Matrix (RVM) report

use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tabled::{builder::Builder, settings::Style};

use crate::cli::helpers::truncate_str;
use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::result::Verdict;
use crate::entities::test::Test;

use super::{load_all_requirements, load_all_results, load_all_tests, write_output};

#[derive(clap::Args, Debug)]
pub struct RvmArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Show only unverified requirements
    #[arg(long)]
    pub unverified_only: bool,
}

pub fn run(args: RvmArgs, _global: &GlobalOpts) -> Result<()> {
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
    let mut latest_results: HashMap<String, &crate::entities::result::Result> = HashMap::new();
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

    // Generate report
    let mut output = String::new();
    output.push_str("# Requirements Verification Matrix (RVM)\n\n");

    // Build table with tabled
    let mut builder = Builder::default();
    builder.push_record(["REQ ID", "REQ Title", "Test ID", "Test Title", "Result", "Verdict"]);

    for row in &rows {
        builder.push_record([
            row.req_short.clone(),
            truncate_str(&row.req_title, 25),
            row.test_short.clone(),
            truncate_str(&row.test_title, 25),
            row.result_id.clone(),
            row.verdict.clone(),
        ]);
    }
    output.push_str(&builder.build().with(Style::markdown()).to_string());

    // Summary
    output.push_str("\n## Summary\n\n");
    let total = requirements.len();
    let coverage = if total > 0 {
        (verified_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let mut summary = Builder::default();
    summary.push_record(["Metric", "Count", "Percentage"]);
    summary.push_record(["Total Requirements", &total.to_string(), "-"]);
    summary.push_record([
        "Verified (all tests pass)",
        &verified_count.to_string(),
        &format!("{:.1}%", coverage),
    ]);
    summary.push_record(["Partial (some tests fail)", &partial_count.to_string(), "-"]);
    summary.push_record(["Unverified", &unverified_count.to_string(), "-"]);
    summary.push_record(["Tests Passed", &passed_count.to_string(), "-"]);
    summary.push_record(["Tests Failed", &failed_count.to_string(), "-"]);
    output.push_str(&summary.build().with(Style::markdown()).to_string());

    // Coverage by Priority
    #[derive(Default)]
    struct PriorityStats {
        total: usize,
        verified: usize,
    }
    let mut priority_stats: HashMap<String, PriorityStats> = HashMap::new();

    // Coverage by Category
    let mut category_stats: HashMap<String, PriorityStats> = HashMap::new();

    // Calculate stats
    for req in &requirements {
        let req_id_str = req.id.to_string();

        // Check verification status for this requirement
        let mut all_test_ids: std::collections::HashSet<String> = req
            .links
            .verified_by
            .iter()
            .map(|id| id.to_string())
            .collect();
        if let Some(reverse_tests) = tests_verifying_req.get(&req_id_str) {
            for test_id in reverse_tests {
                all_test_ids.insert(test_id.clone());
            }
        }

        let is_verified = if all_test_ids.is_empty() {
            false
        } else {
            all_test_ids.iter().all(|test_id| {
                latest_results
                    .get(test_id)
                    .is_some_and(|r| r.verdict == Verdict::Pass || r.verdict == Verdict::NotApplicable)
            })
        };

        // Update priority stats
        let priority_key = req.priority.to_string();
        let priority_entry = priority_stats.entry(priority_key).or_default();
        priority_entry.total += 1;
        if is_verified {
            priority_entry.verified += 1;
        }

        // Update category stats
        let category_key = req.category.as_deref().unwrap_or("uncategorized").to_string();
        let category_entry = category_stats.entry(category_key).or_default();
        category_entry.total += 1;
        if is_verified {
            category_entry.verified += 1;
        }
    }

    // Coverage by Priority
    if !priority_stats.is_empty() {
        output.push_str("\n## Coverage by Priority\n\n");
        let mut priority_table = Builder::default();
        priority_table.push_record(["Priority", "Total", "Verified", "Coverage"]);

        // Sort by priority order
        let priority_order = ["critical", "high", "medium", "low"];
        for priority in priority_order {
            if let Some(stats) = priority_stats.get(priority) {
                let coverage_pct = if stats.total > 0 {
                    format!("{:.1}%", (stats.verified as f64 / stats.total as f64) * 100.0)
                } else {
                    "-".to_string()
                };
                priority_table.push_record([
                    priority.to_string(),
                    stats.total.to_string(),
                    stats.verified.to_string(),
                    coverage_pct,
                ]);
            }
        }
        output.push_str(&priority_table.build().with(Style::markdown()).to_string());
    }

    // Coverage by Category
    if !category_stats.is_empty() && category_stats.len() > 1 {
        output.push_str("\n## Coverage by Category\n\n");
        let mut category_table = Builder::default();
        category_table.push_record(["Category", "Total", "Verified", "Coverage"]);

        // Sort by total count descending
        let mut cats: Vec<_> = category_stats.iter().collect();
        cats.sort_by(|a, b| b.1.total.cmp(&a.1.total));

        for (cat, stats) in cats {
            let coverage_pct = if stats.total > 0 {
                format!("{:.1}%", (stats.verified as f64 / stats.total as f64) * 100.0)
            } else {
                "-".to_string()
            };
            category_table.push_record([
                cat.clone(),
                stats.total.to_string(),
                stats.verified.to_string(),
                coverage_pct,
            ]);
        }
        output.push_str(&category_table.build().with(Style::markdown()).to_string());
    }

    // Output
    write_output(&output, args.output)?;
    Ok(())
}
