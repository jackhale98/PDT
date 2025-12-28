//! Open Issues report

use chrono::Utc;
use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tabled::{builder::Builder, settings::Style};

use crate::cli::helpers::truncate_str;
use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::capa::{ActionStatus, Capa};
use crate::entities::ncr::Ncr;
use crate::entities::result::{Result as TestResult, Verdict};

use super::{load_all_capas, load_all_ncrs, load_all_results, load_all_tests, write_output};

#[derive(clap::Args, Debug)]
pub struct OpenIssuesArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'f')]
    pub file: Option<PathBuf>,
}

pub fn run(args: OpenIssuesArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);
    let today = Utc::now().date_naive();

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

    // Calculate NCR aging and costs
    let calc_days_open =
        |ncr: &Ncr| -> Option<i64> { ncr.report_date.map(|d| (today - d).num_days()) };

    let mut total_rework_cost = 0.0;
    let mut total_scrap_cost = 0.0;
    let mut ncrs_over_30_days = 0;
    let mut ncrs_over_60_days = 0;

    for ncr in &open_ncrs {
        if let Some(days) = calc_days_open(ncr) {
            if days > 60 {
                ncrs_over_60_days += 1;
            } else if days > 30 {
                ncrs_over_30_days += 1;
            }
        }
        if let Some(ref cost) = ncr.cost_impact {
            total_rework_cost += cost.rework_cost.unwrap_or(0.0);
            total_scrap_cost += cost.scrap_cost.unwrap_or(0.0);
        }
    }

    // Find overdue CAPA actions
    let mut overdue_actions: Vec<(&Capa, &crate::entities::capa::ActionItem, i64)> = Vec::new();
    for capa in &open_capas {
        for action in &capa.actions {
            if action.status != ActionStatus::Completed && action.status != ActionStatus::Verified {
                if let Some(due) = action.due_date {
                    if due < today {
                        let days_overdue = (today - due).num_days();
                        overdue_actions.push((capa, action, days_overdue));
                    }
                }
            }
        }
    }
    overdue_actions.sort_by(|a, b| b.2.cmp(&a.2)); // Most overdue first

    // Generate report
    let mut output = String::new();
    output.push_str("# Open Issues Report\n\n");

    // Summary
    output.push_str("## Summary\n\n");
    let mut summary = Builder::default();
    summary.push_record(["Category", "Count"]);
    summary.push_record(["Open NCRs", &open_ncrs.len().to_string()]);
    summary.push_record(["Open CAPAs", &open_capas.len().to_string()]);
    summary.push_record(["Failed Tests", &failed_tests.len().to_string()]);
    summary.push_record(["Overdue CAPA Actions", &overdue_actions.len().to_string()]);
    output.push_str(&summary.build().with(Style::markdown()).to_string());

    // NCR Aging Analysis
    if !open_ncrs.is_empty() {
        output.push_str("\n## NCR Aging Analysis\n\n");
        let mut ncr_table = Builder::default();
        ncr_table.push_record(["ID", "Title", "Severity", "Days Open", "Cost Impact"]);

        // Sort by days open (oldest first)
        let mut sorted_ncrs: Vec<_> = open_ncrs.iter().collect();
        sorted_ncrs.sort_by(|a, b| {
            let days_a = calc_days_open(a).unwrap_or(0);
            let days_b = calc_days_open(b).unwrap_or(0);
            days_b.cmp(&days_a)
        });

        for ncr in &sorted_ncrs {
            let ncr_short = short_ids
                .get_short_id(&ncr.id.to_string())
                .unwrap_or_else(|| ncr.id.to_string());

            let days_open = calc_days_open(ncr)
                .map(|d| {
                    if d > 60 {
                        format!("{} (!)", d)
                    } else if d > 30 {
                        format!("{} (*)", d)
                    } else {
                        d.to_string()
                    }
                })
                .unwrap_or_else(|| "-".to_string());

            let cost = ncr
                .cost_impact
                .as_ref()
                .map(|c| {
                    let total = c.rework_cost.unwrap_or(0.0) + c.scrap_cost.unwrap_or(0.0);
                    if total > 0.0 {
                        format!("${:.0}", total)
                    } else {
                        "-".to_string()
                    }
                })
                .unwrap_or_else(|| "-".to_string());

            ncr_table.push_record([
                ncr_short,
                truncate_str(&ncr.title, 25).to_string(),
                ncr.severity.to_string(),
                days_open,
                cost,
            ]);
        }
        output.push_str(&ncr_table.build().with(Style::markdown()).to_string());

        // Aging summary
        output.push('\n');
        output.push_str(&format!(
            "*Aging: {} NCRs > 30 days, {} NCRs > 60 days*\n",
            ncrs_over_30_days, ncrs_over_60_days
        ));
        output.push_str("*Legend: (\\*) = >30 days, (!) = >60 days*\n");
    }

    // Cost Impact Summary
    if total_rework_cost > 0.0 || total_scrap_cost > 0.0 {
        output.push_str("\n## Cost Impact\n\n");
        let mut cost_table = Builder::default();
        cost_table.push_record(["Category", "Amount"]);
        cost_table.push_record(["Total Rework Cost", &format!("${:.2}", total_rework_cost)]);
        cost_table.push_record(["Total Scrap Cost", &format!("${:.2}", total_scrap_cost)]);
        cost_table.push_record([
            "**Total Impact**",
            &format!("**${:.2}**", total_rework_cost + total_scrap_cost),
        ]);
        output.push_str(&cost_table.build().with(Style::markdown()).to_string());
    }

    // Overdue CAPA Actions
    if !overdue_actions.is_empty() {
        output.push_str("\n## Overdue CAPA Actions\n\n");
        let mut action_table = Builder::default();
        action_table.push_record(["CAPA ID", "Action", "Owner", "Due Date", "Days Overdue"]);

        for (capa, action, days_overdue) in &overdue_actions {
            let capa_short = short_ids
                .get_short_id(&capa.id.to_string())
                .unwrap_or_else(|| capa.id.to_string());

            action_table.push_record([
                capa_short,
                truncate_str(&action.description, 30).to_string(),
                action.owner.as_deref().unwrap_or("-").to_string(),
                action
                    .due_date
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                days_overdue.to_string(),
            ]);
        }
        output.push_str(&action_table.build().with(Style::markdown()).to_string());
    }

    // Open CAPAs
    if !open_capas.is_empty() {
        output.push_str("\n## Open CAPAs\n\n");
        let mut capa_table = Builder::default();
        capa_table.push_record(["ID", "Title", "Type", "Status", "Open Actions"]);
        for capa in &open_capas {
            let capa_short = short_ids
                .get_short_id(&capa.id.to_string())
                .unwrap_or_else(|| capa.id.to_string());

            let open_action_count = capa
                .actions
                .iter()
                .filter(|a| {
                    a.status != ActionStatus::Completed && a.status != ActionStatus::Verified
                })
                .count();

            capa_table.push_record([
                capa_short,
                truncate_str(&capa.title, 25).to_string(),
                capa.capa_type.to_string(),
                capa.capa_status.to_string(),
                open_action_count.to_string(),
            ]);
        }
        output.push_str(&capa_table.build().with(Style::markdown()).to_string());
    }

    // Failed Tests
    if !failed_tests.is_empty() {
        output.push_str("\n## Failed Tests\n\n");
        let mut test_table = Builder::default();
        test_table.push_record(["ID", "Title", "Type"]);
        for test in &failed_tests {
            let test_short = short_ids
                .get_short_id(&test.id.to_string())
                .unwrap_or_else(|| test.id.to_string());
            test_table.push_record([
                test_short,
                truncate_str(&test.title, 40).to_string(),
                test.test_type.to_string(),
            ]);
        }
        output.push_str(&test_table.build().with(Style::markdown()).to_string());
    }

    write_output(&output, args.file)?;
    Ok(())
}
