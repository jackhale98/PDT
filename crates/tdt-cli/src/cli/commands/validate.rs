//! `tdt validate` command - Validate project files against schemas

use console::style;
use miette::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use tdt_core::core::cache::{EntityCache, EntityFilter};
use tdt_core::core::entity::Status;
use tdt_core::core::links::{
    add_explicit_link, check_link_titles, get_field_reference_rules, stamp_link_titles,
};
use tdt_core::core::loader;
use tdt_core::core::project::Project;
use tdt_core::core::suspect::get_suspect_links;
use tdt_core::core::EntityPrefix;
use tdt_core::entities::feature::Feature;
use tdt_core::entities::mate::{FitAnalysis, FitResult, Mate, MateType};
use tdt_core::entities::risk::Risk;
use tdt_core::entities::stackup::Stackup;
use tdt_core::schema::registry::SchemaRegistry;
use tdt_core::schema::validator::Validator;

#[derive(clap::Args, Debug)]
pub struct ValidateArgs {
    /// Paths to validate (default: entire project)
    #[arg()]
    pub paths: Vec<PathBuf>,

    /// Strict mode - warnings become errors
    #[arg(long)]
    pub strict: bool,

    /// Only validate git-staged files
    #[arg(long)]
    pub staged: bool,

    /// Specific entity types to validate (e.g., req, risk)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Stop validation on first error (default: validate all files)
    #[arg(long)]
    pub fail_fast: bool,

    /// Show summary only, don't show individual errors
    #[arg(long)]
    pub summary: bool,

    /// Fix calculated values (RPN, risk level) in-place
    #[arg(long)]
    pub fix: bool,

    /// Deep fix: also re-run tolerance analysis (Monte Carlo, RSS, worst-case)
    /// Requires --fix flag
    #[arg(long)]
    pub deep: bool,

    /// Monte Carlo iterations for deep analysis (default: 10000)
    #[arg(long, default_value = "10000")]
    pub iterations: u32,

    /// Show passing files (default: only show warnings and errors)
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

/// Validation statistics
#[derive(Default)]
struct ValidationStats {
    files_checked: usize,
    files_passed: usize,
    files_failed: usize,
    total_errors: usize,
    total_warnings: usize,
    files_fixed: usize,
    analysis_rerun: usize,
    suspect_links: usize,
    files_with_suspect_links: usize,
    missing_reciprocal_links: usize,
    reciprocal_links_fixed: usize,
    maturity_mismatches: usize,
    stale_link_titles: usize,
    link_titles_fixed: usize,
}

/// Loader for full Feature entities needed for validation calculations
/// This provides O(1) lookups for features needed in mate/stackup validation.
/// Note: This is separate from the core EntityCache which stores minimal metadata.
/// Validation needs full Feature data for FitAnalysis calculations.
struct FeatureLoader {
    features: HashMap<String, Feature>,
}

/// Truncate an ID string for display
fn truncate_id(id: &str) -> String {
    if id.len() > 16 {
        format!("{}...", &id[..13])
    } else {
        id.to_string()
    }
}

/// Extract entity ID from YAML content
fn extract_entity_id(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("id:") {
            let id = line.strip_prefix("id:")?.trim();
            // Handle quoted strings
            let id = id.trim_matches('"').trim_matches('\'');
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Format file path with short ID alias if available
fn format_path_with_alias(
    path: &std::path::Path,
    content: Option<&str>,
    cache: &Option<EntityCache>,
) -> String {
    // Try to get short ID alias
    let alias = content
        .and_then(extract_entity_id)
        .and_then(|id| cache.as_ref().and_then(|c| c.get_short_id(&id)));

    match alias {
        Some(short_id) => format!("{} ({})", path.display(), style(&short_id).cyan()),
        None => path.display().to_string(),
    }
}

impl FeatureLoader {
    /// Load all features once for efficient validation
    fn load(project: &Project) -> Result<Self> {
        let mut features = HashMap::new();

        // Load all features
        let feat_dir = project.root().join("tolerances/features");
        if feat_dir.exists() {
            for entry in WalkDir::new(&feat_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                if !path.to_string_lossy().ends_with(".tdt.yaml") {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                        features.insert(feat.id.to_string(), feat);
                    }
                }
            }
        }

        Ok(Self { features })
    }

    fn get_feature(&self, id: &str) -> Option<&Feature> {
        self.features.get(id)
    }
}

pub fn run(args: ValidateArgs) -> Result<()> {
    // Validate --deep requires --fix
    if args.deep && !args.fix {
        return Err(miette::miette!(
            "--deep requires --fix flag. Use: tdt validate --fix --deep"
        ));
    }

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let registry = SchemaRegistry::default();
    let validator = Validator::new(&registry);

    // Sync the core cache to ensure short IDs are up-to-date (use Option for graceful fallback)
    let cache = EntityCache::open(&project).ok();

    // Load features for mate/stackup validation (needs full Feature data)
    let feature_loader = FeatureLoader::load(&project)?;

    let mut stats = ValidationStats::default();
    let mut had_error = false;

    // Determine which files to validate
    let files_to_validate: Vec<PathBuf> = if args.staged {
        get_staged_files(&project)?
    } else if args.paths.is_empty() {
        get_all_tdt_files(&project)
    } else {
        expand_paths(&args.paths)
    };

    // Filter by entity type if specified
    let entity_filter: Option<EntityPrefix> = args
        .entity_type
        .as_ref()
        .and_then(|t| t.to_uppercase().parse().ok());

    // Check for duplicate entity IDs (same ULID in multiple files)
    let mut id_to_paths: std::collections::HashMap<String, Vec<PathBuf>> =
        std::collections::HashMap::new();
    for path in &files_to_validate {
        if !path.to_string_lossy().ends_with(".tdt.yaml") {
            continue;
        }
        // Extract ID from filename (e.g., RISK-01KCF6P2EQ...tdt.yaml -> RISK-01KCF6P2EQ...)
        if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
            if let Some(id) = fname
                .strip_suffix(".tdt.yaml")
                .or_else(|| fname.strip_suffix(".pdt.yaml"))
            {
                id_to_paths
                    .entry(id.to_string())
                    .or_default()
                    .push(path.clone());
            }
        }
    }

    let mut duplicate_count = 0;
    for (id, paths) in &id_to_paths {
        if paths.len() > 1 {
            duplicate_count += 1;
            if !args.summary {
                let relative_paths: Vec<String> = paths
                    .iter()
                    .map(|p| {
                        p.strip_prefix(project.root())
                            .unwrap_or(p)
                            .display()
                            .to_string()
                    })
                    .collect();
                eprintln!(
                    "{} Duplicate entity ID '{}' found in {} files:",
                    console::style("✗").red(),
                    id,
                    paths.len(),
                );
                for rp in &relative_paths {
                    eprintln!("    → {}", rp);
                }
                eprintln!(
                    "    {}",
                    console::style("Remove duplicate files or rename entity IDs").dim()
                );
            }
        }
    }
    if duplicate_count > 0 {
        stats.total_errors += duplicate_count;
        had_error = true;
        eprintln!();
    }

    println!(
        "{} Validating {} file(s)...\n",
        style("→").blue(),
        files_to_validate.len()
    );

    for path in &files_to_validate {
        // Skip non-.tdt.yaml files
        if !path.to_string_lossy().ends_with(".tdt.yaml") {
            continue;
        }

        // Determine entity type from path
        let prefix =
            EntityPrefix::from_filename(&path.file_name().unwrap_or_default().to_string_lossy())
                .or_else(|| EntityPrefix::from_path(path));

        // Skip if filtering by entity type and this doesn't match
        if let Some(filter) = entity_filter {
            if prefix != Some(filter) {
                continue;
            }
        }

        stats.files_checked += 1;

        // Read file content
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                if !args.summary {
                    println!("{} {} - {}", style("✗").red(), path.display(), e);
                }
                stats.files_failed += 1;
                stats.total_errors += 1;
                had_error = true;
                if args.fail_fast {
                    break;
                }
                continue;
            }
        };

        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip if we can't determine entity type
        let entity_prefix = match prefix {
            Some(p) => p,
            None => {
                if !args.summary {
                    println!(
                        "{} {} - unknown entity type (skipped)",
                        style("?").yellow(),
                        path.display()
                    );
                }
                continue;
            }
        };

        // Validate schema
        match validator.iter_errors(&content, &filename, entity_prefix) {
            Ok(_) => {
                // Schema validation passed - now check calculated values
                let calc_issues = match entity_prefix {
                    EntityPrefix::Risk => {
                        check_risk_calculations(&content, path, args.fix, &mut stats)?
                    }
                    EntityPrefix::Mate => {
                        check_mate_values(&content, path, args.fix, &mut stats, &feature_loader)?
                    }
                    EntityPrefix::Tol => check_stackup_values(
                        &content,
                        path,
                        args.fix,
                        args.deep,
                        args.iterations,
                        &mut stats,
                        &feature_loader,
                    )?,
                    EntityPrefix::Feat => check_feature_values(
                        &content,
                        path,
                        args.fix,
                        &mut stats,
                        &feature_loader,
                        cache.as_ref(),
                    )?,
                    _ => vec![],
                };

                if calc_issues.is_empty() {
                    stats.files_passed += 1;
                    if args.verbose {
                        println!(
                            "{} {}",
                            style("✓").green(),
                            format_path_with_alias(path, Some(&content), &cache)
                        );
                    }
                } else {
                    // Has calculation issues but schema is valid
                    if args.fix {
                        stats.files_passed += 1;
                        if args.verbose {
                            println!(
                                "{} {} (fixed)",
                                style("✓").green(),
                                format_path_with_alias(path, Some(&content), &cache)
                            );
                        }
                    } else {
                        stats.total_warnings += calc_issues.len();
                        if !args.summary {
                            println!(
                                "{} {} - {} calculation warning(s)",
                                style("!").yellow(),
                                format_path_with_alias(path, Some(&content), &cache),
                                calc_issues.len()
                            );
                            for issue in &calc_issues {
                                println!("    {}", style(issue).yellow());
                            }
                        }
                        if args.strict {
                            stats.files_failed += 1;
                            had_error = true;
                        } else {
                            stats.files_passed += 1;
                        }
                    }
                }
            }
            Err(e) => {
                stats.files_failed += 1;
                stats.total_errors += e.violation_count();
                had_error = true;

                if !args.summary {
                    println!(
                        "{} {} - {} error(s)",
                        style("✗").red(),
                        format_path_with_alias(path, Some(&content), &cache),
                        e.violation_count()
                    );

                    // Print detailed error using miette
                    let report = miette::Report::new(e);
                    println!("{:?}", report);
                }

                if args.fail_fast {
                    break;
                }
            }
        }

        // Check for suspect links
        if let Ok(suspect_links) = get_suspect_links(path) {
            if !suspect_links.is_empty() {
                stats.files_with_suspect_links += 1;
                stats.suspect_links += suspect_links.len();
                stats.total_warnings += suspect_links.len();

                if !args.summary {
                    println!(
                        "{} {} - {} suspect link(s)",
                        style("!").yellow(),
                        format_path_with_alias(path, Some(&content), &cache),
                        suspect_links.len()
                    );
                    for (link_type, target_id, reason) in &suspect_links {
                        println!(
                            "    {} → {} ({})",
                            style(link_type).cyan(),
                            truncate_id(target_id),
                            style(reason.to_string()).dim()
                        );
                    }
                }
            }
        }
    }

    // === Link Consistency Check ===
    // Check that field-based references (e.g., QUOT.supplier, FEAT.component) have
    // reciprocal links on the target entities.
    if !args.fail_fast || !had_error {
        let link_issues = check_link_consistency(
            &project,
            &files_to_validate,
            entity_filter,
            args.fix,
            args.summary,
            &mut stats,
        );
        if !link_issues.is_empty() && args.strict {
            had_error = true;
        }
    }

    // === Link Title Check ===
    // Check that all link entries have up-to-date entity titles.
    // Missing or stale titles are fixed with --fix.
    if !args.fail_fast || !had_error {
        if let Some(ref cache) = cache {
            check_link_title_consistency(
                cache,
                &files_to_validate,
                args.fix,
                args.summary,
                &mut stats,
            );
        }
    }

    // === Maturity Mismatch Check ===
    if !args.fail_fast || !had_error {
        if let Some(ref cache) = cache {
            let mismatches =
                check_maturity_mismatches(cache, entity_filter, args.summary, &mut stats);
            if !mismatches.is_empty() && args.strict {
                had_error = true;
            }
        }
    }

    // Print summary
    println!();
    println!("{}", style("─".repeat(60)).dim());
    println!("{}", style("Validation Summary").bold());
    println!("{}", style("─".repeat(60)).dim());
    println!("  Files checked:  {}", style(stats.files_checked).cyan());
    println!("  Files passed:   {}", style(stats.files_passed).green());
    println!("  Files failed:   {}", style(stats.files_failed).red());
    println!("  Total errors:   {}", style(stats.total_errors).red());

    if stats.total_warnings > 0 {
        println!("  Total warnings: {}", style(stats.total_warnings).yellow());
    }

    if stats.files_fixed > 0 {
        println!("  Files fixed:    {}", style(stats.files_fixed).cyan());
    }

    if stats.analysis_rerun > 0 {
        println!(
            "  Analysis rerun: {} (Monte Carlo/RSS/Worst-case)",
            style(stats.analysis_rerun).cyan()
        );
    }

    if stats.missing_reciprocal_links > 0 {
        if stats.reciprocal_links_fixed > 0 {
            println!(
                "  Missing links:  {} detected, {} fixed",
                style(stats.missing_reciprocal_links).yellow(),
                style(stats.reciprocal_links_fixed).green()
            );
        } else {
            println!(
                "  Missing links:  {} (run with --fix to repair)",
                style(stats.missing_reciprocal_links).yellow(),
            );
        }
    }

    if stats.suspect_links > 0 {
        println!(
            "  Suspect links:  {} in {} file(s)",
            style(stats.suspect_links).yellow(),
            style(stats.files_with_suspect_links).yellow()
        );
        println!("                  Run 'tdt link suspect list' to review");
    }

    if stats.maturity_mismatches > 0 {
        println!(
            "  Maturity gaps:  {} (approved/released entities with less-mature dependencies)",
            style(stats.maturity_mismatches).yellow(),
        );
    }

    if stats.stale_link_titles > 0 {
        if stats.link_titles_fixed > 0 {
            println!(
                "  Link titles:    {} missing/stale, {} fixed",
                style(stats.stale_link_titles).yellow(),
                style(stats.link_titles_fixed).green()
            );
        } else {
            println!(
                "  Link titles:    {} missing/stale (run with --fix to update)",
                style(stats.stale_link_titles).yellow(),
            );
        }
    }

    println!();

    if had_error {
        if stats.files_failed == 1 {
            Err(miette::miette!("Validation failed: 1 file has errors"))
        } else {
            Err(miette::miette!(
                "Validation failed: {} files have errors",
                stats.files_failed
            ))
        }
    } else {
        println!("{} All files passed validation!", style("✓").green().bold());
        Ok(())
    }
}

/// Get all .tdt.yaml files in the project
fn get_all_tdt_files(project: &Project) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(project.root())
        .into_iter()
        .filter_entry(|e| {
            // Skip .git and .tdt directories
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') || e.depth() == 0
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.to_string_lossy().ends_with(".tdt.yaml") {
            files.push(path.to_path_buf());
        }
    }

    files.sort();
    files
}

/// Get git-staged .tdt.yaml files
fn get_staged_files(project: &Project) -> Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .current_dir(project.root())
        .output()
        .map_err(|e| miette::miette!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(miette::miette!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".tdt.yaml"))
        .map(|line| project.root().join(line))
        .filter(|path| path.exists())
        .collect();

    Ok(files)
}

/// Expand paths - if a directory is given, find all .tdt.yaml files in it
fn expand_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for path in paths {
        if path.is_dir() {
            for entry in WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                if entry.path().to_string_lossy().ends_with(".tdt.yaml") {
                    files.push(entry.path().to_path_buf());
                }
            }
        } else if path.exists() {
            files.push(path.clone());
        }
    }

    files.sort();
    files
}

/// Check and optionally fix calculated values in RISK entities
fn check_risk_calculations(
    content: &str,
    path: &PathBuf,
    fix: bool,
    stats: &mut ValidationStats,
) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    // Parse the risk
    let risk: Risk = match serde_yml::from_str(content) {
        Ok(r) => r,
        Err(_) => return Ok(issues), // Already reported by schema validation
    };

    // Check RPN calculation
    if let Some(expected_rpn) = risk.calculate_rpn() {
        if let Some(actual_rpn) = risk.rpn {
            if actual_rpn != expected_rpn {
                issues.push(format!(
                    "RPN mismatch: stored {} but calculated {} ({}×{}×{})",
                    actual_rpn,
                    expected_rpn,
                    risk.severity.unwrap_or(0),
                    risk.occurrence.unwrap_or(0),
                    risk.detection.unwrap_or(0)
                ));
            }
        }
    }

    // Check risk level calculation
    if let Some(expected_level) = risk.determine_risk_level() {
        if let Some(actual_level) = risk.risk_level {
            if actual_level != expected_level {
                issues.push(format!(
                    "risk_level mismatch: stored '{}' but calculated '{}'",
                    actual_level, expected_level
                ));
            }
        }
    }

    // Fix if requested and there are issues
    if fix && !issues.is_empty() {
        // Re-parse as a mutable value to fix
        let mut value: serde_yml::Value = serde_yml::from_str(content)
            .map_err(|e| miette::miette!("Failed to re-parse YAML: {}", e))?;

        // Update RPN
        if let Some(expected_rpn) = risk.calculate_rpn() {
            value["rpn"] = serde_yml::Value::Number(expected_rpn.into());

            // Update risk level based on the calculated RPN
            let expected_level = match expected_rpn {
                0..=50 => "low",
                51..=150 => "medium",
                151..=400 => "high",
                _ => "critical",
            };
            value["risk_level"] = serde_yml::Value::String(expected_level.to_string());
        }

        // Write back
        let updated_content = serde_yml::to_string(&value)
            .map_err(|e| miette::miette!("Failed to serialize YAML: {}", e))?;
        fs::write(path, updated_content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

        stats.files_fixed += 1;
        issues.clear(); // Clear issues since we fixed them
    }

    Ok(issues)
}

/// Check and optionally fix calculated values in MATE entities
fn check_mate_values(
    content: &str,
    path: &PathBuf,
    fix: bool,
    stats: &mut ValidationStats,
    features: &FeatureLoader,
) -> Result<Vec<String>> {
    let mut issues = Vec::new();
    let mut needs_fix = false;

    // Parse the mate
    let mut mate: Mate = match serde_yml::from_str(content) {
        Ok(m) => m,
        Err(_) => return Ok(issues), // Already reported by schema validation
    };

    // Load linked features (O(1) lookup instead of O(n) directory scan)
    let feat_a_id = mate.feature_a.id.to_string();
    let feat_a = match features.get_feature(&feat_a_id) {
        Some(f) => f,
        None => {
            issues.push(format!("Cannot find feature_a: {}", mate.feature_a));
            return Ok(issues);
        }
    };

    // Validate cached info for feature_a
    if let Some(ref cached_name) = mate.feature_a.name {
        if cached_name != &feat_a.title {
            if fix {
                mate.feature_a.name = Some(feat_a.title.clone());
                needs_fix = true;
            } else {
                issues.push(format!(
                    "feature_a has stale cached name '{}' (feature is '{}')",
                    cached_name, feat_a.title
                ));
            }
        }
    }
    if let Some(ref cached_cmp_id) = mate.feature_a.component_id {
        if cached_cmp_id != &feat_a.component {
            if fix {
                mate.feature_a.component_id = Some(feat_a.component.clone());
                needs_fix = true;
            } else {
                issues.push(format!(
                    "feature_a has stale cached component_id '{}' (feature belongs to '{}')",
                    cached_cmp_id, feat_a.component
                ));
            }
        }
    }

    let feat_b_id = mate.feature_b.id.to_string();
    let feat_b = match features.get_feature(&feat_b_id) {
        Some(f) => f,
        None => {
            issues.push(format!("Cannot find feature_b: {}", mate.feature_b));
            return Ok(issues);
        }
    };

    // Validate cached info for feature_b
    if let Some(ref cached_name) = mate.feature_b.name {
        if cached_name != &feat_b.title {
            if fix {
                mate.feature_b.name = Some(feat_b.title.clone());
                needs_fix = true;
            } else {
                issues.push(format!(
                    "feature_b has stale cached name '{}' (feature is '{}')",
                    cached_name, feat_b.title
                ));
            }
        }
    }
    if let Some(ref cached_cmp_id) = mate.feature_b.component_id {
        if cached_cmp_id != &feat_b.component {
            if fix {
                mate.feature_b.component_id = Some(feat_b.component.clone());
                needs_fix = true;
            } else {
                issues.push(format!(
                    "feature_b has stale cached component_id '{}' (feature belongs to '{}')",
                    cached_cmp_id, feat_b.component
                ));
            }
        }
    }

    // Get primary dimensions
    let dim_a = match feat_a.primary_dimension() {
        Some(d) => d,
        None => {
            issues.push(format!("Feature {} has no dimension", mate.feature_a));
            return Ok(issues);
        }
    };

    let dim_b = match feat_b.primary_dimension() {
        Some(d) => d,
        None => {
            issues.push(format!("Feature {} has no dimension", mate.feature_b));
            return Ok(issues);
        }
    };

    // Check that features form a valid mate (one internal, one external)
    if dim_a.internal == dim_b.internal {
        if dim_a.internal {
            issues.push(
                "Both features are internal - mate requires one internal and one external"
                    .to_string(),
            );
        } else {
            issues.push(
                "Both features are external - mate requires one internal and one external"
                    .to_string(),
            );
        }
        return Ok(issues);
    }

    // Calculate expected fit analysis
    let expected_analysis = match FitAnalysis::from_dimensions(dim_a, dim_b) {
        Ok(a) => a,
        Err(e) => {
            issues.push(format!("Cannot calculate fit: {}", e));
            return Ok(issues);
        }
    };

    // Compare with stored analysis
    if let Some(actual) = &mate.fit_analysis {
        let min_diff =
            (actual.worst_case_min_clearance - expected_analysis.worst_case_min_clearance).abs();
        let max_diff =
            (actual.worst_case_max_clearance - expected_analysis.worst_case_max_clearance).abs();

        if min_diff > 1e-6 || max_diff > 1e-6 || actual.fit_result != expected_analysis.fit_result {
            if fix {
                mate.fit_analysis = Some(expected_analysis.clone());
                needs_fix = true;
            } else {
                issues.push(format!(
                    "fit_analysis mismatch: stored ({:.4} to {:.4}, {}) but calculated ({:.4} to {:.4}, {})",
                    actual.worst_case_min_clearance,
                    actual.worst_case_max_clearance,
                    actual.fit_result,
                    expected_analysis.worst_case_min_clearance,
                    expected_analysis.worst_case_max_clearance,
                    expected_analysis.fit_result
                ));
            }
        }
    } else if fix {
        mate.fit_analysis = Some(expected_analysis.clone());
        needs_fix = true;
    } else {
        issues.push("fit_analysis not calculated".to_string());
    }

    // Check if specified mate_type matches calculated fit_result
    // This is a design issue: the engineer specified one fit type but the tolerances produce another
    let actual_fit = mate
        .fit_analysis
        .as_ref()
        .map(|a| &a.fit_result)
        .unwrap_or(&expected_analysis.fit_result);

    let type_matches = matches!(
        (&mate.mate_type, actual_fit),
        (MateType::Clearance, FitResult::Clearance)
            | (MateType::Interference, FitResult::Interference)
            | (MateType::Transition, FitResult::Transition)
    );

    if !type_matches {
        issues.push(format!(
            "mate_type mismatch: specified '{}' but calculated fit is '{}' - tolerances don't achieve intended fit",
            mate.mate_type, actual_fit
        ));
    }

    // Fix if requested and there are changes to make
    if fix && needs_fix {
        let updated_content = serde_yml::to_string(&mate)
            .map_err(|e| miette::miette!("Failed to serialize YAML: {}", e))?;
        fs::write(path, updated_content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

        stats.files_fixed += 1;
    }

    Ok(issues)
}

/// Check and optionally fix contributor values in stackup entities
fn check_stackup_values(
    content: &str,
    path: &PathBuf,
    fix: bool,
    deep: bool,
    iterations: u32,
    stats: &mut ValidationStats,
    features: &FeatureLoader,
) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    // Parse the stackup
    let mut stackup: Stackup = match serde_yml::from_str(content) {
        Ok(s) => s,
        Err(_) => return Ok(issues), // Already reported by schema validation
    };

    let mut any_synced = false;
    let mut analysis_needed = false;

    // Check each contributor that has a feature reference
    for contributor in stackup.contributors.iter_mut() {
        let feature_id = match &contributor.feature {
            Some(f) => f.id.to_string(),
            None => continue,
        };

        // O(1) lookup from cache instead of O(n) directory scan
        if let Some(feature) = features.get_feature(&feature_id) {
            // Check dimensional sync
            if contributor.is_out_of_sync(feature) {
                if fix {
                    contributor.sync_from_feature(feature);
                    any_synced = true;
                    analysis_needed = true; // Dimensions changed, need re-analysis
                } else if let Some(dim) = feature.primary_dimension() {
                    issues.push(format!(
                        "Contributor '{}' out of sync with {}: stored ({:.4} +{:.4}/-{:.4}) vs feature ({:.4} +{:.4}/-{:.4})",
                        contributor.name,
                        feature_id,
                        contributor.nominal,
                        contributor.plus_tol,
                        contributor.minus_tol,
                        dim.nominal,
                        dim.plus_tol,
                        dim.minus_tol
                    ));
                }
            }

            // Check cached feature info
            if let Some(ref mut feat_ref) = contributor.feature {
                if let Some(ref cached_name) = feat_ref.name {
                    if cached_name != &feature.title {
                        if fix {
                            feat_ref.name = Some(feature.title.clone());
                            any_synced = true;
                        } else {
                            issues.push(format!(
                                "Contributor '{}' has stale cached name '{}' (feature is '{}')",
                                contributor.name, cached_name, feature.title
                            ));
                        }
                    }
                }

                if let Some(ref cached_cmp_id) = feat_ref.component_id {
                    if cached_cmp_id != &feature.component {
                        if fix {
                            feat_ref.component_id = Some(feature.component.clone());
                            any_synced = true;
                        } else {
                            issues.push(format!(
                                "Contributor '{}' has stale component_id '{}' in YAML (feature belongs to '{}') — run `tdt validate --fix` to update",
                                contributor.name, cached_cmp_id, feature.component
                            ));
                        }
                    }
                }
            }
        } else {
            issues.push(format!(
                "Contributor '{}' references unknown feature: {}",
                contributor.name, feature_id
            ));
        }
    }

    // Deep mode: re-run tolerance analysis if contributors exist
    if deep && !stackup.contributors.is_empty() {
        // Always re-run analysis in deep mode, or when dimensions changed.
        // Any analysis error (e.g., contributors went away during processing)
        // is surfaced as a validation issue rather than panicking.
        match (
            stackup.calculate_monte_carlo(iterations),
            stackup.calculate_worst_case(),
            stackup.calculate_rss(),
        ) {
            (Ok(mc), Ok(wc), Ok(rss)) => {
                stackup.analysis_results.monte_carlo = Some(mc);
                stackup.analysis_results.worst_case = Some(wc);
                stackup.analysis_results.rss = Some(rss);
                stackup.analysis_results.analyzed_at = Some(chrono::Utc::now());
                stackup.analysis_results.input_hash = Some(stackup.contributor_input_hash());
                stats.analysis_rerun += 1;
                any_synced = true; // Force write since analysis results changed
            }
            (a, b, c) => {
                let msg = a
                    .err()
                    .map(|e| e.to_string())
                    .or_else(|| b.err().map(|e| e.to_string()))
                    .or_else(|| c.err().map(|e| e.to_string()))
                    .unwrap_or_else(|| "unknown analysis error".to_string());
                issues.push(format!("analysis failed during validate --deep: {msg}"));
            }
        }
    } else if fix && analysis_needed && !stackup.contributors.is_empty() {
        // Even without --deep, if dimensions changed we should flag it
        issues.push(
            "Contributor dimensions synced - consider running 'tdt validate --fix --deep' or 'tdt tol analyze' to update analysis results"
                .to_string(),
        );
    }

    // Stale-analysis detection: if the contributor chain hash on disk no
    // longer matches the analysis's stored hash, the numbers can't be
    // trusted. Surface this as a validate issue so it shows up in CI.
    if stackup.analysis_is_fresh() == Some(false) {
        issues.push(
            "analysis is stale — contributors have changed since the last run; \
             rerun `tdt tol analyze` to refresh"
                .to_string(),
        );
    }

    // Check if any analysis shows Fail or Marginal (design issue that needs attention)
    // This is similar to mate type mismatch - the tolerances don't achieve the desired fit
    if let Some(ref wc) = stackup.analysis_results.worst_case {
        use tdt_core::entities::stackup::AnalysisResult;
        match wc.result {
            AnalysisResult::Fail => {
                issues.push(format!(
                    "worst-case analysis shows FAIL: margin = {:.4} {} (min: {:.4}, max: {:.4})",
                    wc.margin, &stackup.target.units, wc.min, wc.max
                ));
            }
            AnalysisResult::Marginal => {
                issues.push(format!(
                    "worst-case analysis shows MARGINAL: margin = {:.4} {} (min: {:.4}, max: {:.4})",
                    wc.margin,
                    &stackup.target.units,
                    wc.min,
                    wc.max
                ));
            }
            AnalysisResult::Pass => {}
        }
    }

    // RSS uses Cpk to determine pass/fail: Cpk >= 1.33 = pass, 1.0-1.33 = marginal, < 1.0 = fail
    if let Some(ref rss) = stackup.analysis_results.rss {
        if rss.cpk < 1.0 {
            issues.push(format!(
                "RSS analysis shows FAIL: Cpk = {:.2} (< 1.0) - yield: {:.2}%",
                rss.cpk, rss.yield_percent
            ));
        } else if rss.cpk < 1.33 {
            issues.push(format!(
                "RSS analysis shows MARGINAL: Cpk = {:.2} (< 1.33) - yield: {:.2}%",
                rss.cpk, rss.yield_percent
            ));
        }
    }

    // Monte Carlo uses Ppk to determine pass/fail: Ppk >= 1.33 = pass, 1.0-1.33 = marginal, < 1.0 = fail
    if let Some(ref mc) = stackup.analysis_results.monte_carlo {
        if let Some(ppk) = mc.ppk {
            if ppk < 1.0 {
                issues.push(format!(
                    "Monte Carlo analysis shows FAIL: Ppk = {:.2} (< 1.0) - yield: {:.2}%",
                    ppk, mc.yield_percent
                ));
            } else if ppk < 1.33 {
                issues.push(format!(
                    "Monte Carlo analysis shows MARGINAL: Ppk = {:.2} (< 1.33) - yield: {:.2}%",
                    ppk, mc.yield_percent
                ));
            }
        }
    }

    // Write back if we synced any contributors or re-ran analysis
    if fix && any_synced {
        let updated_content = serde_yml::to_string(&stackup)
            .map_err(|e| miette::miette!("Failed to serialize YAML: {}", e))?;
        fs::write(path, updated_content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

        stats.files_fixed += 1;
    }

    Ok(issues)
}

/// Check and optionally fix calculated values in FEAT entities
/// This includes checking if torsor_bounds are stale compared to GD&T controls
/// and if length_ref values are stale compared to referenced dimensions
fn check_feature_values(
    content: &str,
    path: &PathBuf,
    fix: bool,
    stats: &mut ValidationStats,
    feature_loader: &FeatureLoader,
    cache: Option<&EntityCache>,
) -> Result<Vec<String>> {
    use tdt_core::core::gdt_torsor::{check_stale_bounds, compute_torsor_bounds};
    use tdt_core::entities::feature::{DimensionRef, MaterialCondition};

    let mut issues = Vec::new();
    let mut needs_length_fix = false;
    let mut expected_length: Option<f64> = None;

    // Parse the feature
    let feat: Feature = match serde_yml::from_str(content) {
        Ok(f) => f,
        Err(_) => return Ok(issues), // Already reported by schema validation
    };

    // Check length_ref if present
    if let Some(ref geometry_3d) = feat.geometry_3d {
        if let Some(ref length_ref_str) = geometry_3d.length_ref {
            if let Some(dim_ref) = DimensionRef::parse(length_ref_str) {
                // Resolve feature ID (handle short IDs like FEAT@1)
                let resolved_id = if dim_ref.feature_id.contains('@') {
                    // Short ID - resolve via cache
                    cache
                        .and_then(|c| c.resolve_short_id(&dim_ref.feature_id))
                        .unwrap_or_else(|| dim_ref.feature_id.clone())
                } else {
                    dim_ref.feature_id.clone()
                };

                // Look up the target feature
                if let Some(target_feat) = feature_loader.get_feature(&resolved_id) {
                    // Get the dimension value
                    if let Some(dim_value) =
                        target_feat.get_dimension_value(&dim_ref.dimension_name)
                    {
                        expected_length = Some(dim_value);
                        // Check if cached length matches
                        match geometry_3d.length {
                            Some(cached_len) if (cached_len - dim_value).abs() > 1e-6 => {
                                issues.push(format!(
                                    "length_ref stale: cached {} but {} has {}={:.6}",
                                    cached_len, length_ref_str, dim_ref.dimension_name, dim_value
                                ));
                                needs_length_fix = true;
                            }
                            None => {
                                issues.push(format!(
                                    "length_ref set but length not cached (should be {:.6} from {})",
                                    dim_value, length_ref_str
                                ));
                                needs_length_fix = true;
                            }
                            _ => {} // Length matches, all good
                        }
                    } else {
                        issues.push(format!(
                            "length_ref references unknown dimension '{}' in {}",
                            dim_ref.dimension_name, resolved_id
                        ));
                    }
                } else {
                    issues.push(format!(
                        "length_ref references unknown feature '{}'",
                        dim_ref.feature_id
                    ));
                }
            } else {
                issues.push(format!(
                    "Invalid length_ref format '{}' - expected 'FEAT@1:dimension_name'",
                    length_ref_str
                ));
            }
        }
    }

    // Skip torsor bounds check if no GD&T controls or dimensions (nothing to compute)
    let check_torsor = !feat.gdt.is_empty() || !feat.dimensions.is_empty();

    // Skip if no geometry_class defined (can't compute bounds without knowing geometry type)
    if check_torsor && feat.geometry_class.is_none() {
        // Only warn if there ARE GD&T controls that could be used
        if !feat.gdt.is_empty() {
            issues.push(
                "Feature has GD&T controls but no geometry_class - cannot auto-compute torsor_bounds"
                    .to_string(),
            );
        }
    }

    // Informational note about unverifiable bonus-inclusive bounds (MMC/LMC).
    // Kept out of `issues` so it neither triggers --fix nor gets cleared by it.
    let mut bonus_note: Option<String> = None;

    // Compute expected torsor bounds if we have geometry_class
    let torsor_result = if check_torsor && feat.geometry_class.is_some() {
        // Note: Feature lookup not available in this context, use None
        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

        // Features with MMC/LMC material modifiers may have stored bounds that
        // legitimately include measured-size bonus tolerance. Recomputing here
        // with no actual size cannot reproduce that bonus, so a mismatch is
        // NOT evidence of staleness - skip the stale comparison and never let
        // --fix overwrite (which would strip the bonus).
        let has_material_modifier = feat.gdt.iter().any(|g| {
            matches!(
                g.material_condition,
                MaterialCondition::Mmc | MaterialCondition::Lmc
            )
        });

        if has_material_modifier {
            if check_stale_bounds(&feat.torsor_bounds, &result.bounds, 1e-6).is_some() {
                bonus_note = Some(
                    "note: torsor_bounds differ from size-independent computation, but feature has MMC/LMC modifiers so bounds may include measured-size bonus; cannot verify without --actual-size (not auto-fixed)"
                        .to_string(),
                );
            }

            // Add any warnings from computation
            for warning in &result.warnings {
                issues.push(warning.clone());
            }

            // Return None so --fix does not overwrite bonus-inclusive bounds
            None
        } else {
            // Check if stored bounds match computed bounds
            if let Some(stale_msg) = check_stale_bounds(&feat.torsor_bounds, &result.bounds, 1e-6) {
                issues.push(stale_msg);
            }

            // Add any warnings from computation
            for warning in &result.warnings {
                issues.push(warning.clone());
            }
            Some(result)
        }
    } else {
        None
    };

    // Fix if requested and there are issues
    if fix && !issues.is_empty() {
        // Re-parse as a mutable feature to fix
        let mut feat: Feature = serde_yml::from_str(content)
            .map_err(|e| miette::miette!("Failed to re-parse YAML: {}", e))?;

        // Update torsor_bounds with computed values if available
        if let Some(ref result) = torsor_result {
            feat.torsor_bounds = Some(result.bounds.clone());
        }

        // Update length from length_ref if stale
        if needs_length_fix {
            if let Some(ref mut geometry_3d) = feat.geometry_3d {
                geometry_3d.length = expected_length;
            }
        }

        // Write back
        let updated_content = serde_yml::to_string(&feat)
            .map_err(|e| miette::miette!("Failed to serialize YAML: {}", e))?;
        fs::write(path, updated_content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

        stats.files_fixed += 1;
        issues.clear(); // Clear issues since we fixed them
    }

    // Surface the informational MMC/LMC note regardless of --fix (it is
    // never auto-fixable without an actual measured size).
    if let Some(note) = bonus_note {
        issues.push(note);
    }

    Ok(issues)
}

// ============================================================================
// Maturity Mismatch Check
// ============================================================================

/// A maturity mismatch: source entity is more mature than a linked target
struct MaturityMismatch {
    source_id: String,
    source_status: Status,
    target_id: String,
    target_status: Status,
    link_type: String,
}

/// Check that entities don't depend on less-mature targets.
///
/// When an entity is promoted to `approved` or `released`, its linked targets
/// should be at least as mature. Flags cases where `source.status > target.status`,
/// excluding `Obsolete` sources (obsolete entities are a special case).
fn check_maturity_mismatches(
    cache: &EntityCache,
    entity_filter: Option<EntityPrefix>,
    summary_only: bool,
    stats: &mut ValidationStats,
) -> Vec<MaturityMismatch> {
    let mut mismatches: Vec<MaturityMismatch> = Vec::new();

    let filter = EntityFilter {
        prefix: entity_filter,
        ..Default::default()
    };
    let entities = cache.list_entities(&filter);

    for entity in &entities {
        // Only check entities above Draft and not Obsolete
        if entity.status <= Status::Draft || entity.status == Status::Obsolete {
            continue;
        }

        let links = cache.get_links_from(&entity.id);
        for link in &links {
            if let Some(target) = cache.get_entity(&link.target_id) {
                // Flag if source is more mature than target (excluding obsolete targets)
                if target.status != Status::Obsolete && entity.status > target.status {
                    mismatches.push(MaturityMismatch {
                        source_id: entity.id.clone(),
                        source_status: entity.status,
                        target_id: target.id.clone(),
                        target_status: target.status,
                        link_type: link.link_type.clone(),
                    });
                }
            }
        }
    }

    if mismatches.is_empty() {
        return mismatches;
    }

    stats.maturity_mismatches = mismatches.len();
    stats.total_warnings += mismatches.len();

    if !summary_only {
        println!();
        println!(
            "{} Maturity Mismatch: {} gap(s) found",
            style("!").yellow(),
            mismatches.len()
        );

        for mm in &mismatches {
            println!(
                "    {} {} ({}) → {} ({}) via {}",
                style("!").yellow(),
                truncate_id(&mm.source_id),
                mm.source_status,
                truncate_id(&mm.target_id),
                mm.target_status,
                mm.link_type,
            );
        }
    }

    mismatches
}

// ============================================================================
// Link Consistency Check
// ============================================================================

/// A missing reciprocal link found during consistency checking
struct MissingLink {
    /// Source entity ID (the one with the field reference)
    source_id: String,
    /// Source entity title (for writing into the link entry)
    source_title: String,
    /// The field on the source entity
    field_name: String,
    /// Target entity ID (the one missing the reciprocal)
    target_id: String,
    /// The link type that should exist on the target
    reciprocal_link_type: String,
    /// Path to the target entity file (for fixing)
    target_path: Option<PathBuf>,
}

/// Check link consistency across all entities.
///
/// For each entity type that has field-based references (e.g., QUOT.supplier,
/// FEAT.component), verify that the target entity has the expected reciprocal
/// link back to the source. Reports missing links and optionally fixes them.
fn check_link_consistency(
    project: &Project,
    files: &[PathBuf],
    entity_filter: Option<EntityPrefix>,
    fix: bool,
    summary_only: bool,
    stats: &mut ValidationStats,
) -> Vec<MissingLink> {
    let mut missing_links: Vec<MissingLink> = Vec::new();

    // Phase 1: Collect all field references that need reciprocal links
    for path in files {
        if !path.to_string_lossy().ends_with(".tdt.yaml") {
            continue;
        }

        let prefix =
            EntityPrefix::from_filename(&path.file_name().unwrap_or_default().to_string_lossy())
                .or_else(|| EntityPrefix::from_path(path));

        let source_prefix = match prefix {
            Some(p) => p,
            None => continue,
        };

        // Skip if filtering and this type doesn't have rules
        if let Some(filter) = entity_filter {
            if source_prefix != filter {
                continue;
            }
        }

        let rules = get_field_reference_rules(source_prefix);
        if rules.is_empty() {
            continue;
        }

        // Read and parse the source entity YAML
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let value: serde_yml::Value = match serde_yml::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Extract source entity ID and title
        let source_id = match value.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };
        let source_title = value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Check each field reference rule
        for rule in &rules {
            // Get the field value (direct field, not in links)
            let target_id = match value.get(rule.field_name).and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => continue,
            };

            // Determine target entity prefix
            let target_prefix = match target_id.split('-').next() {
                Some(p) => match p.parse::<EntityPrefix>() {
                    Ok(prefix) => prefix,
                    Err(_) => continue,
                },
                None => continue,
            };

            // Find the target entity file
            let target_dir = project
                .root()
                .join(Project::entity_directory(target_prefix));
            let target_path = loader::find_entity_file(&target_dir, &target_id);

            // Check if the target has the reciprocal link
            let has_reciprocal = if let Some(ref tp) = target_path {
                check_has_link(tp, rule.reciprocal_link_type, &source_id)
            } else {
                false // Can't find target file, report as missing
            };

            if !has_reciprocal && target_path.is_some() {
                missing_links.push(MissingLink {
                    source_id: source_id.clone(),
                    source_title: source_title.clone(),
                    field_name: rule.field_name.to_string(),
                    target_id: target_id.clone(),
                    reciprocal_link_type: rule.reciprocal_link_type.to_string(),
                    target_path,
                });
            }
        }
    }

    if missing_links.is_empty() {
        return missing_links;
    }

    stats.missing_reciprocal_links = missing_links.len();

    // Phase 2: Report and optionally fix
    if !summary_only {
        println!();
        println!(
            "{} Link Consistency: {} missing reciprocal link(s)",
            style("!").yellow(),
            missing_links.len()
        );
    }

    for ml in &missing_links {
        if !summary_only {
            println!(
                "    {} {}.{} → {} missing {}.links.{}",
                if fix {
                    style("→").blue().to_string()
                } else {
                    style("!").yellow().to_string()
                },
                truncate_id(&ml.source_id),
                ml.field_name,
                truncate_id(&ml.target_id),
                truncate_id(&ml.target_id),
                ml.reciprocal_link_type
            );
        }

        if fix {
            if let Some(ref target_path) = ml.target_path {
                let title = if ml.source_title.is_empty() {
                    None
                } else {
                    Some(ml.source_title.as_str())
                };
                match add_explicit_link(target_path, &ml.reciprocal_link_type, &ml.source_id, title)
                {
                    Ok(()) => {
                        stats.reciprocal_links_fixed += 1;
                    }
                    Err(e) => {
                        if !summary_only {
                            println!("      {} Failed to fix: {}", style("✗").red(), e);
                        }
                    }
                }
            }
        }
    }

    if !summary_only && !fix && !missing_links.is_empty() {
        println!(
            "    {} Run 'tdt validate --fix' or 'tdt link sync' to repair",
            style("→").dim()
        );
    }

    stats.total_warnings += missing_links.len();
    missing_links
}

/// Check if an entity file has a specific link to a given target ID.
/// Handles both `{id, title}` mappings and bare string entries.
fn check_has_link(path: &std::path::Path, link_type: &str, target_id: &str) -> bool {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let value: serde_yml::Value = match serde_yml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let links = match value.get("links") {
        Some(l) => l,
        None => return false,
    };

    match links.get(link_type) {
        Some(serde_yml::Value::Sequence(arr)) => arr
            .iter()
            .any(|v| tdt_core::core::links::extract_link_id(v) == Some(target_id)),
        Some(v) => tdt_core::core::links::extract_link_id(v) == Some(target_id),
        _ => false,
    }
}

/// Check that all link entries across project files have correct, up-to-date titles.
///
/// Missing or stale titles are reported as warnings. With `--fix`, titles are
/// updated in-place using `stamp_link_titles`.
fn check_link_title_consistency(
    cache: &EntityCache,
    files: &[PathBuf],
    fix: bool,
    summary_only: bool,
    stats: &mut ValidationStats,
) {
    // Build a title map from all cached entities
    let all_entities = cache.list_entities(&EntityFilter::default());
    let titles: HashMap<String, String> = all_entities
        .iter()
        .map(|e| (e.id.clone(), e.title.clone()))
        .collect();

    let mut total_issues = 0;
    let mut total_fixed = 0;

    for path in files {
        if !path.to_string_lossy().ends_with(".tdt.yaml") {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let issues = check_link_titles(&content, &titles);
        if issues.is_empty() {
            continue;
        }

        total_issues += issues.len();

        if !summary_only && !fix {
            for (id, current, actual) in &issues {
                let truncated = if id.len() > 16 {
                    format!("{}...", &id[..13])
                } else {
                    id.clone()
                };
                match current {
                    Some(old) => println!(
                        "  {} Link to {} has stale title \"{}\" (actual: \"{}\")",
                        style("!").yellow(),
                        truncated,
                        old,
                        actual,
                    ),
                    None => println!(
                        "  {} Link to {} missing title (should be \"{}\")",
                        style("!").yellow(),
                        truncated,
                        actual,
                    ),
                }
            }
        }

        if fix {
            match stamp_link_titles(&content, &titles) {
                Ok(updated) => {
                    if updated != content {
                        if let Ok(()) = fs::write(path, &updated) {
                            total_fixed += issues.len();
                        }
                    }
                }
                Err(e) => {
                    if !summary_only {
                        println!(
                            "  {} Failed to fix link titles in {}: {}",
                            style("✗").red(),
                            path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    if total_issues > 0 && !summary_only && !fix {
        println!();
        println!(
            "{} Link Titles: {} missing or stale title(s)",
            style("!").yellow(),
            total_issues
        );
        println!(
            "    {} Run 'tdt validate --fix' to update link titles",
            style("→").dim()
        );
    }

    stats.stale_link_titles = total_issues;
    stats.link_titles_fixed = total_fixed;
    stats.total_warnings += total_issues;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_project() -> (TempDir, Project) {
        let tmp = TempDir::new().unwrap();

        // Initialize as a TDT project
        let config_dir = tmp.path().join(".tdt");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.yaml"), "version: 1\n").unwrap();

        // Create directories
        fs::create_dir_all(tmp.path().join("requirements/inputs")).unwrap();
        fs::create_dir_all(tmp.path().join("risks")).unwrap();
        fs::create_dir_all(tmp.path().join("tolerances/features")).unwrap();
        fs::create_dir_all(tmp.path().join("tolerances/mates")).unwrap();
        fs::create_dir_all(tmp.path().join("tolerances/stackups")).unwrap();
        fs::create_dir_all(tmp.path().join("bom/components")).unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        (tmp, project)
    }

    fn write_entity(project: &Project, rel_path: &str, content: &str) {
        let full_path = project.root().join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content.trim_start()).unwrap();
    }

    // =========================================================================
    // Risk Calculation Tests
    // =========================================================================

    #[test]
    fn test_check_risk_rpn_correct() {
        // RPN = 8 * 5 * 4 = 160, risk_level should be "high" (151-400)
        let content = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Test Risk
description: A test risk
type: design
author: Test
created: 2024-01-15T10:30:00Z
severity: 8
occurrence: 5
detection: 4
rpn: 160
risk_level: high
"#;

        let mut stats = ValidationStats::default();
        let path = PathBuf::from("/tmp/test.yaml");
        let issues = check_risk_calculations(content.trim(), &path, false, &mut stats).unwrap();

        assert!(
            issues.is_empty(),
            "No issues expected for correct RPN: {:?}",
            issues
        );
    }

    #[test]
    fn test_check_risk_rpn_mismatch() {
        // RPN should be 8 * 5 * 4 = 160, but stored as 100
        let content = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Test Risk
description: A test risk
type: design
author: Test
created: 2024-01-15T10:30:00Z
severity: 8
occurrence: 5
detection: 4
rpn: 100
risk_level: medium
"#;

        let mut stats = ValidationStats::default();
        let path = PathBuf::from("/tmp/test.yaml");
        let issues = check_risk_calculations(content.trim(), &path, false, &mut stats).unwrap();

        assert!(
            issues.iter().any(|i| i.contains("RPN mismatch")),
            "Expected RPN mismatch issue: {:?}",
            issues
        );
    }

    #[test]
    fn test_check_risk_level_mismatch() {
        // RPN 160 = high (151-400), but stored as "low"
        let content = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Test Risk
description: A test risk
type: design
author: Test
created: 2024-01-15T10:30:00Z
severity: 8
occurrence: 5
detection: 4
rpn: 160
risk_level: low
"#;

        let mut stats = ValidationStats::default();
        let path = PathBuf::from("/tmp/test.yaml");
        let issues = check_risk_calculations(content.trim(), &path, false, &mut stats).unwrap();

        assert!(
            issues.iter().any(|i| i.contains("risk_level mismatch")),
            "Expected risk_level mismatch: {:?}",
            issues
        );
    }

    #[test]
    fn test_check_risk_no_rpn() {
        let content = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Test Risk
description: A test risk
type: design
author: Test
created: 2024-01-15T10:30:00Z
"#;

        let mut stats = ValidationStats::default();
        let path = PathBuf::from("/tmp/test.yaml");
        let issues = check_risk_calculations(content.trim(), &path, false, &mut stats).unwrap();

        assert!(issues.is_empty(), "No issues when RPN fields not set");
    }

    // =========================================================================
    // Feature Loader Tests
    // =========================================================================

    #[test]
    fn test_feature_loader_empty_project() {
        let (_tmp, project) = create_test_project();
        let loader = FeatureLoader::load(&project).unwrap();

        assert!(loader.get_feature("FEAT-NONEXISTENT").is_none());
    }

    #[test]
    fn test_feature_loader_finds_features() {
        let (_tmp, project) = create_test_project();

        write_entity(
            &project,
            "tolerances/features/FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTD.tdt.yaml",
            r#"
id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Bore Diameter
feature_type: internal
dimension:
  nominal: 10.0
  plus_tol: 0.05
  minus_tol: 0.05
  unit: mm
component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPT1
author: Test
created: 2024-01-15T10:30:00Z
"#,
        );

        let loader = FeatureLoader::load(&project).unwrap();

        let feat = loader.get_feature("FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTD");
        assert!(feat.is_some(), "Should find the feature");
        assert_eq!(feat.unwrap().title, "Bore Diameter");
    }

    #[test]
    fn test_feature_loader_multiple_features() {
        let (_tmp, project) = create_test_project();

        // Generate 5 unique ULID-like IDs
        let ids = [
            "01HC2JB7SMQX7RS1Y0GFKBHPT0",
            "01HC2JB7SMQX7RS1Y0GFKBHPT1",
            "01HC2JB7SMQX7RS1Y0GFKBHPT2",
            "01HC2JB7SMQX7RS1Y0GFKBHPT3",
            "01HC2JB7SMQX7RS1Y0GFKBHPT4",
        ];

        for (i, ulid) in ids.iter().enumerate() {
            write_entity(
                &project,
                &format!("tolerances/features/FEAT-{}.tdt.yaml", ulid),
                &format!(
                    r#"
id: FEAT-{}
title: Feature {}
feature_type: internal
dimension:
  nominal: 10.0
  plus_tol: 0.05
  minus_tol: 0.05
  unit: mm
component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPT0
author: Test
created: 2024-01-15T10:30:00Z
"#,
                    ulid, i
                ),
            );
        }

        let loader = FeatureLoader::load(&project).unwrap();

        for ulid in &ids {
            let feat = loader.get_feature(&format!("FEAT-{}", ulid));
            assert!(feat.is_some(), "Should find feature with ULID {}", ulid);
        }
    }

    // =========================================================================
    // Schema Validation Tests
    // =========================================================================

    #[test]
    fn test_schema_validates_valid_requirement() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let content = r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Valid Requirement
status: draft
type: input
priority: high
author: Test Author
created: 2024-01-15T10:30:00Z
text: This is a test requirement.
"#;

        let result = validator.validate(content.trim(), "test.yaml", EntityPrefix::Req);
        assert!(
            result.is_ok(),
            "Valid requirement should pass: {:?}",
            result
        );
    }

    #[test]
    fn test_schema_rejects_missing_required_field() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let content = r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#;

        let result = validator.validate(content.trim(), "test.yaml", EntityPrefix::Req);
        assert!(result.is_err(), "Missing 'title' should fail validation");
    }

    #[test]
    fn test_schema_validates_valid_risk() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let content = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Test Risk
description: A test risk description
type: design
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#;

        let result = validator.validate(content.trim(), "test.yaml", EntityPrefix::Risk);
        assert!(result.is_ok(), "Valid risk should pass: {:?}", result);
    }

    #[test]
    fn test_schema_validates_valid_component() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let content = r#"
id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
part_number: PN-001
title: Housing
make_buy: make
category: mechanical
status: draft
description: Main housing component
author: Test
created: 2024-01-15T10:30:00Z
"#;

        let result = validator.validate(content.trim(), "test.yaml", EntityPrefix::Cmp);
        assert!(result.is_ok(), "Valid component should pass: {:?}", result);
    }

    #[test]
    fn test_schema_validates_valid_feature() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let content = r#"
id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: Bore Diameter
feature_type: internal
status: draft
component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPT0
author: Test
created: 2024-01-15T10:30:00Z
dimensions:
  - name: diameter
    nominal: 10.0
    plus_tol: 0.05
    minus_tol: 0.05
"#;

        let result = validator.validate(content.trim(), "test.yaml", EntityPrefix::Feat);
        assert!(result.is_ok(), "Valid feature should pass: {:?}", result);
    }
}
