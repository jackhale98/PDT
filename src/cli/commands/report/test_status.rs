//! Test Status report

use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tabled::{builder::Builder, settings::Style};

use crate::cli::helpers::{format_date_local, truncate_str};
use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::result::{Result as TestResult, Verdict};
use crate::entities::test::{Test, TestType};

use super::{load_all_results, load_all_tests, write_output};

#[derive(clap::Args, Debug)]
pub struct TestStatusArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

pub fn run(args: TestStatusArgs, _global: &GlobalOpts) -> Result<()> {
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

    // Overall stats
    let mut executed = 0;
    let mut pending = 0;
    let mut passed = 0;
    let mut failed = 0;
    let mut conditional = 0;
    let mut recent_failures: Vec<(&Test, &TestResult)> = Vec::new();

    // Breakdown by type
    #[derive(Default)]
    struct TypeStats {
        total: usize,
        executed: usize,
        passed: usize,
        failed: usize,
    }
    let mut verification_stats = TypeStats::default();
    let mut validation_stats = TypeStats::default();

    // Breakdown by level
    let mut level_stats: HashMap<String, TypeStats> = HashMap::new();

    // Breakdown by category
    let mut category_stats: HashMap<String, TypeStats> = HashMap::new();

    for test in &tests {
        let test_id = test.id.to_string();
        let test_type = test.test_type;
        let test_level = test.test_level;
        let test_category = test.category.as_deref().unwrap_or("uncategorized");

        // Get result status
        let (is_executed, is_passed, is_failed) = if let Some(result) = latest_results.get(&test_id)
        {
            executed += 1;
            let (p, f) = match result.verdict {
                Verdict::Pass => {
                    passed += 1;
                    (true, false)
                }
                Verdict::Fail => {
                    failed += 1;
                    recent_failures.push((test, result));
                    (false, true)
                }
                Verdict::Conditional => {
                    conditional += 1;
                    (false, false)
                }
                Verdict::Incomplete | Verdict::NotApplicable => (false, false),
            };
            (true, p, f)
        } else {
            pending += 1;
            (false, false, false)
        };

        // Update type stats
        let type_stats = match test_type {
            TestType::Verification => &mut verification_stats,
            TestType::Validation => &mut validation_stats,
        };
        type_stats.total += 1;
        if is_executed {
            type_stats.executed += 1;
        }
        if is_passed {
            type_stats.passed += 1;
        }
        if is_failed {
            type_stats.failed += 1;
        }

        // Update level stats
        if let Some(level) = test_level {
            let level_key = level.to_string();
            let level_entry = level_stats.entry(level_key).or_default();
            level_entry.total += 1;
            if is_executed {
                level_entry.executed += 1;
            }
            if is_passed {
                level_entry.passed += 1;
            }
            if is_failed {
                level_entry.failed += 1;
            }
        }

        // Update category stats
        let cat_entry = category_stats
            .entry(test_category.to_string())
            .or_default();
        cat_entry.total += 1;
        if is_executed {
            cat_entry.executed += 1;
        }
        if is_passed {
            cat_entry.passed += 1;
        }
        if is_failed {
            cat_entry.failed += 1;
        }
    }

    // Sort failures by date (most recent first)
    recent_failures.sort_by(|a, b| b.1.executed_date.cmp(&a.1.executed_date));
    recent_failures.truncate(10);

    // Generate report
    let mut output = String::new();
    output.push_str("# Test Execution Status Report\n\n");

    // Overall Summary
    output.push_str("## Summary\n\n");
    let mut summary = Builder::default();
    summary.push_record(["Metric", "Count"]);
    summary.push_record(["Total Protocols", &tests.len().to_string()]);
    summary.push_record(["Executed", &executed.to_string()]);
    summary.push_record(["Pending", &pending.to_string()]);
    summary.push_record(["Passed", &passed.to_string()]);
    summary.push_record(["Failed", &failed.to_string()]);
    summary.push_record(["Conditional", &conditional.to_string()]);

    if executed > 0 {
        let pass_rate = (passed as f64 / executed as f64) * 100.0;
        summary.push_record(["Pass Rate", &format!("{:.1}%", pass_rate)]);
    }
    output.push_str(&summary.build().with(Style::markdown()).to_string());

    // Breakdown by Test Type
    output.push_str("\n## By Test Type\n\n");
    let mut type_table = Builder::default();
    type_table.push_record(["Type", "Total", "Executed", "Pass", "Fail", "Pass Rate"]);

    let calc_pass_rate = |stats: &TypeStats| -> String {
        if stats.executed > 0 {
            format!("{:.1}%", (stats.passed as f64 / stats.executed as f64) * 100.0)
        } else {
            "-".to_string()
        }
    };

    type_table.push_record([
        "Verification".to_string(),
        verification_stats.total.to_string(),
        verification_stats.executed.to_string(),
        verification_stats.passed.to_string(),
        verification_stats.failed.to_string(),
        calc_pass_rate(&verification_stats),
    ]);
    type_table.push_record([
        "Validation".to_string(),
        validation_stats.total.to_string(),
        validation_stats.executed.to_string(),
        validation_stats.passed.to_string(),
        validation_stats.failed.to_string(),
        calc_pass_rate(&validation_stats),
    ]);
    output.push_str(&type_table.build().with(Style::markdown()).to_string());

    // Breakdown by Level
    if !level_stats.is_empty() {
        output.push_str("\n## By Test Level\n\n");
        let mut level_table = Builder::default();
        level_table.push_record(["Level", "Total", "Executed", "Pass", "Fail", "Pass Rate"]);

        // Sort levels in order: unit, integration, system, acceptance
        let level_order = ["unit", "integration", "system", "acceptance"];
        for level in level_order {
            if let Some(stats) = level_stats.get(level) {
                level_table.push_record([
                    level.to_string(),
                    stats.total.to_string(),
                    stats.executed.to_string(),
                    stats.passed.to_string(),
                    stats.failed.to_string(),
                    calc_pass_rate(stats),
                ]);
            }
        }
        output.push_str(&level_table.build().with(Style::markdown()).to_string());
    }

    // Breakdown by Category
    if !category_stats.is_empty() && category_stats.len() > 1 {
        output.push_str("\n## By Category\n\n");
        let mut cat_table = Builder::default();
        cat_table.push_record(["Category", "Total", "Executed", "Pass", "Fail", "Pass Rate"]);

        // Sort categories by total count descending
        let mut cats: Vec<_> = category_stats.iter().collect();
        cats.sort_by(|a, b| b.1.total.cmp(&a.1.total));

        for (cat, stats) in cats {
            cat_table.push_record([
                cat.clone(),
                stats.total.to_string(),
                stats.executed.to_string(),
                stats.passed.to_string(),
                stats.failed.to_string(),
                calc_pass_rate(stats),
            ]);
        }
        output.push_str(&cat_table.build().with(Style::markdown()).to_string());
    }

    // Recent Failures
    if !recent_failures.is_empty() {
        output.push_str("\n## Recent Failures\n\n");
        let mut failures = Builder::default();
        failures.push_record(["Test ID", "Title", "Type", "Level", "Execution Date"]);
        for (test, result) in &recent_failures {
            let test_short = short_ids
                .get_short_id(&test.id.to_string())
                .unwrap_or_else(|| test.id.to_string());
            failures.push_record([
                test_short,
                truncate_str(&test.title, 30).to_string(),
                test.test_type.to_string(),
                test.test_level.map(|l| l.to_string()).unwrap_or_else(|| "-".to_string()),
                format_date_local(&result.executed_date),
            ]);
        }
        output.push_str(&failures.build().with(Style::markdown()).to_string());
    }

    write_output(&output, args.output)?;
    Ok(())
}
