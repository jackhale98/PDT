//! `tdt status` command - Project status dashboard

use console::style;
use miette::Result;
use std::collections::HashMap;

use std::collections::HashSet;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::entity::Status;
use crate::core::project::Project;
use crate::entities::capa::Capa;
use crate::entities::mate::Mate;
use crate::entities::ncr::Ncr;
use crate::entities::result::{Result as TestResult, Verdict};
use crate::entities::risk::{Risk, RiskLevel};
use crate::entities::stackup::{AnalysisResult, Stackup};
use crate::entities::test::Test;

#[derive(clap::Args, Debug)]
pub struct StatusArgs {
    /// Show only specific section (requirements, risks, tests, quality, bom)
    #[arg(long)]
    pub section: Option<String>,

    /// Show detailed breakdown
    #[arg(long)]
    pub detailed: bool,
}

pub fn run(_args: StatusArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Collect metrics
    let req_metrics = collect_requirement_metrics(&project);
    let risk_metrics = collect_risk_metrics(&project);
    let test_metrics = collect_test_metrics(&project);
    let quality_metrics = collect_quality_metrics(&project);
    let bom_metrics = collect_bom_metrics(&project);
    let tol_metrics = collect_tolerance_metrics(&project);

    match global.output {
        OutputFormat::Json => {
            let status = serde_json::json!({
                "requirements": req_metrics,
                "risks": risk_metrics,
                "tests": test_metrics,
                "quality": quality_metrics,
                "bom": bom_metrics,
                "tolerances": tol_metrics,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&status).unwrap_or_default()
            );
        }
        _ => {
            // Human-readable dashboard
            let width = 68;

            println!("{}", style("Tessera Project Status").bold().underlined());
            println!("{}", "═".repeat(width));
            println!();

            // Requirements and Risks side by side
            print_two_columns(
                "REQUIREMENTS",
                &format_requirement_metrics(&req_metrics),
                "RISKS",
                &format_risk_metrics(&risk_metrics),
                width,
            );

            println!();

            // Tests and Quality side by side
            print_two_columns(
                "TESTS",
                &format_test_metrics(&test_metrics),
                "QUALITY",
                &format_quality_metrics(&quality_metrics),
                width,
            );

            println!();

            // BOM and Tolerances side by side
            print_two_columns(
                "BILL OF MATERIALS",
                &format_bom_metrics(&bom_metrics),
                "TOLERANCE ANALYSIS",
                &format_tolerance_metrics(&tol_metrics),
                width,
            );

            println!();
            println!("{}", "═".repeat(width));

            // Overall health indicator
            let health = calculate_health(
                &req_metrics,
                &risk_metrics,
                &test_metrics,
                &quality_metrics,
                &tol_metrics,
            );
            let health_style = match health.as_str() {
                "Healthy" => style(health.clone()).green().bold(),
                "Warning" => style(health.clone()).yellow().bold(),
                "Critical" => style(health.clone()).red().bold(),
                _ => style(health.clone()).dim(),
            };
            println!("Project Health: {}", health_style);
        }
    }

    Ok(())
}

#[derive(serde::Serialize, Default)]
struct RequirementMetrics {
    total: usize,
    by_status: HashMap<String, usize>,
    by_type: HashMap<String, usize>,
    verified: usize,
    unverified: usize,
    coverage_pct: f64,
}

#[derive(serde::Serialize, Default)]
struct RiskMetrics {
    total: usize,
    by_level: HashMap<String, usize>,
    avg_rpn: f64,
    max_rpn: u16,
    unmitigated: usize,
}

#[derive(serde::Serialize, Default)]
struct TestMetrics {
    protocols: usize,
    executed: usize,
    pending: usize,
    pass_count: usize,
    fail_count: usize,
    pass_rate: f64,
}

#[derive(serde::Serialize, Default)]
struct QualityMetrics {
    open_ncrs: usize,
    open_capas: usize,
    overdue: usize,
    ncr_by_severity: HashMap<String, usize>,
}

#[derive(serde::Serialize, Default)]
struct BomMetrics {
    components: usize,
    assemblies: usize,
    make_parts: usize,
    buy_parts: usize,
    single_source: usize,
    with_quotes: usize,
}

#[derive(serde::Serialize, Default)]
struct ToleranceMetrics {
    /// Total features defined
    features: usize,
    /// Total mates defined
    mates: usize,
    /// Mates with calculated fit analysis
    mates_with_analysis: usize,
    /// Total stackups defined
    stackups: usize,
    /// Stackups with analysis results
    stackups_with_analysis: usize,
    /// Stackups that pass worst-case
    stackups_pass: usize,
    /// Stackups that are marginal
    stackups_marginal: usize,
    /// Stackups that fail worst-case
    stackups_fail: usize,
    /// Contributors linked to features
    contributors_linked: usize,
    /// Contributors not linked to features
    contributors_unlinked: usize,
}

fn collect_requirement_metrics(project: &Project) -> RequirementMetrics {
    let mut metrics = RequirementMetrics::default();

    // First, load all tests and build set of requirement IDs that are verified by tests
    let mut verified_by_tests: HashSet<String> = HashSet::new();
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
                    for req_id in &test.links.verifies {
                        verified_by_tests.insert(req_id.to_string());
                    }
                }
            }
        }
    }

    // Now collect requirement metrics
    for subdir in &["requirements/inputs", "requirements/outputs"] {
        let dir = project.root().join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<crate::entities::requirement::Requirement>(
                entry.path(),
            ) {
                metrics.total += 1;

                let status_str = format!("{:?}", req.status).to_lowercase();
                *metrics.by_status.entry(status_str).or_insert(0) += 1;

                let type_str = format!("{:?}", req.req_type).to_lowercase();
                *metrics.by_type.entry(type_str).or_insert(0) += 1;

                // Check both: req.links.verified_by AND tests that verify this req
                let has_verification = !req.links.verified_by.is_empty()
                    || verified_by_tests.contains(&req.id.to_string());
                if has_verification {
                    metrics.verified += 1;
                } else {
                    metrics.unverified += 1;
                }
            }
        }
    }

    if metrics.total > 0 {
        metrics.coverage_pct = (metrics.verified as f64 / metrics.total as f64) * 100.0;
    }

    metrics
}

fn collect_risk_metrics(project: &Project) -> RiskMetrics {
    let mut metrics = RiskMetrics::default();
    let mut rpns: Vec<u16> = Vec::new();

    for subdir in &["risks/design", "risks/process"] {
        let dir = project.root().join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                metrics.total += 1;

                let level = risk
                    .risk_level
                    .or_else(|| risk.determine_risk_level())
                    .unwrap_or(RiskLevel::Medium);
                let level_str = format!("{:?}", level).to_lowercase();
                *metrics.by_level.entry(level_str).or_insert(0) += 1;

                if let Some(rpn) = risk.calculate_rpn() {
                    rpns.push(rpn);
                }

                if risk.mitigations.is_empty() {
                    metrics.unmitigated += 1;
                }
            }
        }
    }

    if !rpns.is_empty() {
        metrics.avg_rpn = rpns.iter().map(|&r| r as f64).sum::<f64>() / rpns.len() as f64;
        metrics.max_rpn = *rpns.iter().max().unwrap_or(&0);
    }

    metrics
}

fn collect_test_metrics(project: &Project) -> TestMetrics {
    let mut metrics = TestMetrics::default();
    let mut test_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut executed_test_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Count protocols
    for subdir in &["verification/protocols", "validation/protocols"] {
        let dir = project.root().join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(test) =
                crate::yaml::parse_yaml_file::<crate::entities::test::Test>(entry.path())
            {
                metrics.protocols += 1;
                test_ids.insert(test.id.to_string());
            }
        }
    }

    // Count results
    for subdir in &["verification/results", "validation/results"] {
        let dir = project.root().join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(result) = crate::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                executed_test_ids.insert(result.test_id.to_string());

                match result.verdict {
                    Verdict::Pass => metrics.pass_count += 1,
                    Verdict::Fail => metrics.fail_count += 1,
                    Verdict::Conditional => metrics.pass_count += 1,
                    _ => {}
                }
            }
        }
    }

    metrics.executed = executed_test_ids.len();
    metrics.pending = test_ids.difference(&executed_test_ids).count();

    let total_judged = metrics.pass_count + metrics.fail_count;
    if total_judged > 0 {
        metrics.pass_rate = (metrics.pass_count as f64 / total_judged as f64) * 100.0;
    }

    metrics
}

fn collect_quality_metrics(project: &Project) -> QualityMetrics {
    let mut metrics = QualityMetrics::default();
    let today = chrono::Utc::now().date_naive();

    // Count NCRs
    let ncr_dir = project.root().join("manufacturing/ncrs");
    if ncr_dir.exists() {
        for entry in walkdir::WalkDir::new(&ncr_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(ncr) = crate::yaml::parse_yaml_file::<Ncr>(entry.path()) {
                if ncr.status != Status::Obsolete && ncr.disposition.is_none() {
                    metrics.open_ncrs += 1;

                    let sev = format!("{:?}", ncr.severity).to_lowercase();
                    *metrics.ncr_by_severity.entry(sev).or_insert(0) += 1;
                }
            }
        }
    }

    // Count CAPAs
    let capa_dir = project.root().join("manufacturing/capas");
    if capa_dir.exists() {
        for entry in walkdir::WalkDir::new(&capa_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(capa) = crate::yaml::parse_yaml_file::<Capa>(entry.path()) {
                if capa.capa_status != crate::entities::capa::CapaStatus::Closed {
                    metrics.open_capas += 1;

                    if let Some(ref timeline) = capa.timeline {
                        if let Some(target) = timeline.target_date {
                            if target < today {
                                metrics.overdue += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    metrics
}

fn collect_bom_metrics(project: &Project) -> BomMetrics {
    let mut metrics = BomMetrics::default();
    let mut component_suppliers: HashMap<String, Vec<String>> = HashMap::new();

    // Count components
    let cmp_dir = project.root().join("bom/components");
    if cmp_dir.exists() {
        for entry in walkdir::WalkDir::new(&cmp_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(cmp) =
                crate::yaml::parse_yaml_file::<crate::entities::component::Component>(entry.path())
            {
                metrics.components += 1;

                match cmp.make_buy {
                    crate::entities::component::MakeBuy::Make => metrics.make_parts += 1,
                    crate::entities::component::MakeBuy::Buy => metrics.buy_parts += 1,
                }

                component_suppliers.insert(cmp.id.to_string(), Vec::new());
            }
        }
    }

    // Count assemblies
    let asm_dir = project.root().join("bom/assemblies");
    if asm_dir.exists() {
        for entry in walkdir::WalkDir::new(&asm_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(_asm) =
                crate::yaml::parse_yaml_file::<crate::entities::assembly::Assembly>(entry.path())
            {
                metrics.assemblies += 1;
            }
        }
    }

    // Check quotes for supplier diversity
    let quote_dir = project.root().join("bom/quotes");
    if quote_dir.exists() {
        for entry in walkdir::WalkDir::new(&quote_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(quote) =
                crate::yaml::parse_yaml_file::<crate::entities::quote::Quote>(entry.path())
            {
                if let Some(ref cmp_id) = quote.component {
                    if let Some(suppliers) = component_suppliers.get_mut(cmp_id) {
                        if !suppliers.contains(&quote.supplier) {
                            suppliers.push(quote.supplier.clone());
                        }
                    }
                    metrics.with_quotes += 1;
                }
            }
        }
    }

    // Count single-source components
    for suppliers in component_suppliers.values() {
        if suppliers.len() == 1 {
            metrics.single_source += 1;
        }
    }

    metrics
}

fn collect_tolerance_metrics(project: &Project) -> ToleranceMetrics {
    let mut metrics = ToleranceMetrics::default();

    // Count features
    let feat_dir = project.root().join("tolerances/features");
    if feat_dir.exists() {
        for entry in walkdir::WalkDir::new(&feat_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if crate::yaml::parse_yaml_file::<crate::entities::feature::Feature>(entry.path())
                .is_ok()
            {
                metrics.features += 1;
            }
        }
    }

    // Count mates
    let mate_dir = project.root().join("tolerances/mates");
    if mate_dir.exists() {
        for entry in walkdir::WalkDir::new(&mate_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(mate) = crate::yaml::parse_yaml_file::<Mate>(entry.path()) {
                metrics.mates += 1;
                if mate.fit_analysis.is_some() {
                    metrics.mates_with_analysis += 1;
                }
            }
        }
    }

    // Count stackups and analyze results
    let stackup_dir = project.root().join("tolerances/stackups");
    if stackup_dir.exists() {
        for entry in walkdir::WalkDir::new(&stackup_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(stackup) = crate::yaml::parse_yaml_file::<Stackup>(entry.path()) {
                metrics.stackups += 1;

                // Count linked vs unlinked contributors
                for contrib in &stackup.contributors {
                    if contrib.feature.is_some() {
                        metrics.contributors_linked += 1;
                    } else {
                        metrics.contributors_unlinked += 1;
                    }
                }

                // Check worst-case analysis result
                if let Some(ref results) = stackup.analysis_results.worst_case {
                    metrics.stackups_with_analysis += 1;
                    match results.result {
                        AnalysisResult::Pass => metrics.stackups_pass += 1,
                        AnalysisResult::Marginal => metrics.stackups_marginal += 1,
                        AnalysisResult::Fail => metrics.stackups_fail += 1,
                    }
                }
            }
        }
    }

    metrics
}

fn format_requirement_metrics(m: &RequirementMetrics) -> Vec<String> {
    vec![
        format!("Total:      {}", m.total),
        format!("Verified:   {} ({:.0}%)", m.verified, m.coverage_pct),
        format!("Unverified: {}", m.unverified),
        format!("Draft:      {}", m.by_status.get("draft").unwrap_or(&0)),
        format!("Approved:   {}", m.by_status.get("approved").unwrap_or(&0)),
    ]
}

fn format_risk_metrics(m: &RiskMetrics) -> Vec<String> {
    let mut lines = vec![format!("Total:      {}", m.total)];

    let critical = *m.by_level.get("critical").unwrap_or(&0);
    let high = *m.by_level.get("high").unwrap_or(&0);

    if critical > 0 {
        lines.push(format!("Critical:   {} {}", critical, style("⚠").red()));
    }
    if high > 0 {
        lines.push(format!("High:       {}", high));
    }
    lines.push(format!(
        "Medium:     {}",
        m.by_level.get("medium").unwrap_or(&0)
    ));
    lines.push(format!("Avg RPN:    {:.0}", m.avg_rpn));

    lines
}

fn format_test_metrics(m: &TestMetrics) -> Vec<String> {
    vec![
        format!("Protocols:  {}", m.protocols),
        format!("Executed:   {}", m.executed),
        format!("Pending:    {}", m.pending),
        format!("Pass Rate:  {:.0}%", m.pass_rate),
        format!("Failures:   {}", m.fail_count),
    ]
}

fn format_quality_metrics(m: &QualityMetrics) -> Vec<String> {
    let mut lines = vec![
        format!("Open NCRs:  {}", m.open_ncrs),
        format!("Open CAPAs: {}", m.open_capas),
    ];

    if m.overdue > 0 {
        lines.push(format!("Overdue:    {} {}", m.overdue, style("⚠").red()));
    }

    lines
}

fn format_bom_metrics(m: &BomMetrics) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Components: {}  (Make: {}, Buy: {})",
            m.components, m.make_parts, m.buy_parts
        ),
        format!("Assemblies: {}", m.assemblies),
        format!("With Quotes: {}", m.with_quotes),
    ];

    if m.single_source > 0 {
        lines.push(format!(
            "Single-source: {} {}",
            m.single_source,
            style("⚠").yellow()
        ));
    }

    lines
}

fn format_tolerance_metrics(m: &ToleranceMetrics) -> Vec<String> {
    let mut lines = Vec::new();

    // Features
    lines.push(format!("Features:   {}", m.features));

    // Mates
    if m.mates > 0 {
        let mates_unanalyzed = m.mates - m.mates_with_analysis;
        if mates_unanalyzed > 0 {
            lines.push(format!(
                "Mates:      {} ({} unanalyzed)",
                m.mates, mates_unanalyzed
            ));
        } else {
            lines.push(format!("Mates:      {}", m.mates));
        }
    } else {
        lines.push(format!("Mates:      {}", m.mates));
    }

    // Stackups with pass/fail breakdown
    if m.stackups > 0 {
        lines.push(format!("Stackups:   {}", m.stackups));

        if m.stackups_with_analysis > 0 {
            if m.stackups_fail > 0 {
                lines.push(format!(
                    "  Failing:  {} {}",
                    m.stackups_fail,
                    style("⚠").red()
                ));
            }
            if m.stackups_marginal > 0 {
                lines.push(format!(
                    "  Marginal: {} {}",
                    m.stackups_marginal,
                    style("⚠").yellow()
                ));
            }
            lines.push(format!("  Passing:  {}", m.stackups_pass));
        }

        let unanalyzed = m.stackups - m.stackups_with_analysis;
        if unanalyzed > 0 {
            lines.push(format!("  Unanalyzed: {}", unanalyzed));
        }
    } else {
        lines.push(format!("Stackups:   {}", m.stackups));
    }

    // Contributors linkage
    let total_contributors = m.contributors_linked + m.contributors_unlinked;
    if total_contributors > 0 {
        let linked_pct = (m.contributors_linked as f64 / total_contributors as f64) * 100.0;
        if m.contributors_unlinked > 0 {
            lines.push(format!(
                "Contributors: {} linked ({:.0}%)",
                m.contributors_linked, linked_pct
            ));
        }
    }

    lines
}

fn print_two_columns(
    title1: &str,
    lines1: &[String],
    title2: &str,
    lines2: &[String],
    _width: usize,
) {
    let col_width = 34;

    println!(
        "{:<col_width$} {}",
        style(title1).bold(),
        style(title2).bold()
    );
    println!("{:-<col_width$} {:-<col_width$}", "", "");

    let max_lines = lines1.len().max(lines2.len());

    for i in 0..max_lines {
        let l1 = lines1.get(i).map(|s| s.as_str()).unwrap_or("");
        let l2 = lines2.get(i).map(|s| s.as_str()).unwrap_or("");
        println!("  {:<32} {}", l1, l2);
    }
}

fn calculate_health(
    req: &RequirementMetrics,
    risk: &RiskMetrics,
    test: &TestMetrics,
    quality: &QualityMetrics,
    tol: &ToleranceMetrics,
) -> String {
    let mut score = 100i32;

    // Requirements coverage
    if req.coverage_pct < 50.0 {
        score -= 20;
    } else if req.coverage_pct < 80.0 {
        score -= 10;
    }

    // Critical risks
    let critical = *risk.by_level.get("critical").unwrap_or(&0);
    if critical > 0 {
        score -= 15 * critical as i32;
    }

    // Unmitigated risks
    if risk.unmitigated > 5 {
        score -= 10;
    }

    // Test failures
    if test.fail_count > 0 {
        score -= 5 * test.fail_count as i32;
    }

    // Open quality issues
    if quality.open_ncrs > 5 || quality.open_capas > 3 {
        score -= 15;
    }

    // Overdue items
    if quality.overdue > 0 {
        score -= 10 * quality.overdue as i32;
    }

    // Tolerance analysis failures
    if tol.stackups_fail > 0 {
        score -= 15 * tol.stackups_fail as i32;
    }

    // Marginal stackups
    if tol.stackups_marginal > 0 {
        score -= 5 * tol.stackups_marginal as i32;
    }

    match score {
        80..=100 => "Healthy".to_string(),
        50..=79 => "Warning".to_string(),
        _ => "Critical".to_string(),
    }
}
