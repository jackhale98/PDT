//! FMEA (Failure Mode and Effects Analysis) report

use chrono::Utc;
use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tabled::{builder::Builder, settings::Style};

use crate::cli::helpers::truncate_str;
use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::risk::MitigationStatus;

use super::{load_all_risks, write_output};

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

pub fn run(args: FmeaArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);
    let today = Utc::now().date_naive();

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
    let mut total_initial_rpn: u32 = 0;
    let mut risks_with_initial: usize = 0;
    let mut by_level: HashMap<String, usize> = HashMap::new();

    // Collect risk reduction data and overdue mitigations
    struct ReductionRow {
        id: String,
        failure_mode: String,
        initial_rpn: u16,
        current_rpn: u16,
        reduction: i32,
    }
    let mut reduction_rows: Vec<ReductionRow> = Vec::new();

    struct MitigationRow {
        risk_id: String,
        action: String,
        mit_type: String,
        owner: String,
        due_date: String,
        status: String,
    }
    let mut mitigation_rows: Vec<MitigationRow> = Vec::new();

    struct OverdueRow {
        risk_id: String,
        action: String,
        owner: String,
        due_date: String,
        days_overdue: i64,
    }
    let mut overdue_rows: Vec<OverdueRow> = Vec::new();

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

        let current_rpn = risk.rpn.unwrap_or(0);
        if current_rpn > 0 {
            total_rpn += current_rpn as u32;
        }

        // Track initial risk for reduction analysis
        if let Some(ref initial) = risk.initial_risk {
            if let Some(init_rpn) = initial.rpn {
                total_initial_rpn += init_rpn as u32;
                risks_with_initial += 1;

                let reduction = init_rpn as i32 - current_rpn as i32;
                if reduction != 0 {
                    reduction_rows.push(ReductionRow {
                        id: risk_short.clone(),
                        failure_mode: truncate_str(risk.failure_mode.as_deref().unwrap_or("-"), 25).to_string(),
                        initial_rpn: init_rpn,
                        current_rpn,
                        reduction,
                    });
                }
            }
        }

        // Collect mitigation details
        for mit in &risk.mitigations {
            let status = mit.status.unwrap_or(MitigationStatus::Proposed);
            let is_complete = status == MitigationStatus::Completed || status == MitigationStatus::Verified;

            mitigation_rows.push(MitigationRow {
                risk_id: risk_short.clone(),
                action: truncate_str(&mit.action, 30).to_string(),
                mit_type: mit.mitigation_type.map(|t| format!("{:?}", t).to_lowercase()).unwrap_or_else(|| "-".to_string()),
                owner: mit.owner.as_deref().unwrap_or("-").to_string(),
                due_date: mit.due_date.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string()),
                status: status.to_string(),
            });

            // Check for overdue
            if !is_complete {
                if let Some(due) = mit.due_date {
                    if due < today {
                        let days_overdue = (today - due).num_days();
                        overdue_rows.push(OverdueRow {
                            risk_id: risk_short.clone(),
                            action: truncate_str(&mit.action, 30).to_string(),
                            owner: mit.owner.as_deref().unwrap_or("-").to_string(),
                            due_date: due.to_string(),
                            days_overdue,
                        });
                    }
                }
            }
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

    // Sort reduction rows by reduction amount (descending)
    reduction_rows.sort_by(|a, b| b.reduction.cmp(&a.reduction));

    // Sort overdue by days overdue (descending)
    overdue_rows.sort_by(|a, b| b.days_overdue.cmp(&a.days_overdue));

    // Generate report
    let mut output = String::new();
    output.push_str("# FMEA Report\n\n");

    // Summary
    output.push_str("## Summary\n\n");
    let mut summary = Builder::default();
    summary.push_record(["Metric", "Value"]);
    summary.push_record(["Total Risks", &risks.len().to_string()]);
    if !risks.is_empty() {
        summary.push_record(["Average RPN", &format!("{:.1}", total_rpn as f64 / risks.len() as f64)]);
    }
    summary.push_record(["Critical", &by_level.get("critical").unwrap_or(&0).to_string()]);
    summary.push_record(["High", &by_level.get("high").unwrap_or(&0).to_string()]);
    summary.push_record(["Medium", &by_level.get("medium").unwrap_or(&0).to_string()]);
    summary.push_record(["Low", &by_level.get("low").unwrap_or(&0).to_string()]);

    let unmitigated = risks.iter().filter(|r| r.mitigations.is_empty()).count();
    summary.push_record(["Unmitigated Risks", &unmitigated.to_string()]);
    summary.push_record(["Overdue Mitigations", &overdue_rows.len().to_string()]);
    output.push_str(&summary.build().with(Style::markdown()).to_string());

    // Risk Reduction Summary
    if !reduction_rows.is_empty() {
        output.push_str("\n## Risk Reduction Summary\n\n");
        let mut reduction_table = Builder::default();
        reduction_table.push_record(["ID", "Failure Mode", "Initial RPN", "Current RPN", "Reduction"]);

        for row in &reduction_rows {
            reduction_table.push_record([
                row.id.clone(),
                row.failure_mode.clone(),
                row.initial_rpn.to_string(),
                row.current_rpn.to_string(),
                format!("{:+}", row.reduction),
            ]);
        }
        output.push_str(&reduction_table.build().with(Style::markdown()).to_string());

        if risks_with_initial > 0 && total_initial_rpn > 0 {
            let reduction_pct = ((total_initial_rpn as f64 - total_rpn as f64) / total_initial_rpn as f64) * 100.0;
            output.push_str(&format!(
                "\n*Total RPN Reduction: {} points ({:.1}% from initial)*\n",
                total_initial_rpn as i32 - total_rpn as i32,
                reduction_pct
            ));
        }
    }

    // Overdue Mitigations
    if !overdue_rows.is_empty() {
        output.push_str("\n## Overdue Mitigations\n\n");
        let mut overdue_table = Builder::default();
        overdue_table.push_record(["Risk ID", "Mitigation", "Owner", "Due Date", "Days Overdue"]);

        for row in &overdue_rows {
            overdue_table.push_record([
                row.risk_id.clone(),
                row.action.clone(),
                row.owner.clone(),
                row.due_date.clone(),
                row.days_overdue.to_string(),
            ]);
        }
        output.push_str(&overdue_table.build().with(Style::markdown()).to_string());
    }

    // Main FMEA table
    output.push_str("\n## Risk Register\n\n");
    let mut builder = Builder::default();
    builder.push_record(["ID", "Failure Mode", "Cause", "Effect", "S", "O", "D", "RPN", "Level", "Mitigations"]);

    for row in &rows {
        builder.push_record([
            &row.id,
            &row.failure_mode,
            &row.cause,
            &row.effect,
            &row.s,
            &row.o,
            &row.d,
            &row.rpn,
            &row.level,
            &row.mitigations,
        ]);
    }
    output.push_str(&builder.build().with(Style::markdown()).to_string());

    // Mitigation Tracking
    if !mitigation_rows.is_empty() {
        output.push_str("\n## Mitigation Tracking\n\n");
        let mut mit_table = Builder::default();
        mit_table.push_record(["Risk ID", "Mitigation", "Type", "Owner", "Due Date", "Status"]);

        for row in &mitigation_rows {
            mit_table.push_record([
                row.risk_id.clone(),
                row.action.clone(),
                row.mit_type.clone(),
                row.owner.clone(),
                row.due_date.clone(),
                row.status.clone(),
            ]);
        }
        output.push_str(&mit_table.build().with(Style::markdown()).to_string());
    }

    // Output
    write_output(&output, args.output)?;
    Ok(())
}
