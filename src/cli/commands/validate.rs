//! `tdt validate` command - Validate project files against schemas

use console::style;
use miette::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::core::cache::EntityCache;
use crate::core::project::Project;
use crate::core::EntityPrefix;
use crate::entities::feature::Feature;
use crate::entities::mate::{FitAnalysis, Mate};
use crate::entities::risk::Risk;
use crate::entities::stackup::Stackup;
use crate::schema::registry::SchemaRegistry;
use crate::schema::validator::Validator;

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
}

/// Loader for full Feature entities needed for validation calculations
/// This provides O(1) lookups for features needed in mate/stackup validation.
/// Note: This is separate from the core EntityCache which stores minimal metadata.
/// Validation needs full Feature data for FitAnalysis calculations.
struct FeatureLoader {
    features: HashMap<String, Feature>,
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
    path: &PathBuf,
    content: Option<&str>,
    cache: &Option<EntityCache>,
) -> String {
    // Try to get short ID alias
    let alias = content
        .and_then(|c| extract_entity_id(c))
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
                        "{} {} - {}",
                        style("?").yellow(),
                        path.display(),
                        "unknown entity type (skipped)"
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
                    EntityPrefix::Tol => {
                        check_stackup_values(&content, path, args.fix, &mut stats, &feature_loader)?
                    }
                    _ => vec![],
                };

                if calc_issues.is_empty() {
                    stats.files_passed += 1;
                    if !args.summary {
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
                        if !args.summary {
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
                mate.fit_analysis = Some(expected_analysis);
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
    } else {
        if fix {
            mate.fit_analysis = Some(expected_analysis);
            needs_fix = true;
        } else {
            issues.push("fit_analysis not calculated".to_string());
        }
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

    // Check each contributor that has a feature reference
    for contributor in stackup.contributors.iter_mut() {
        let feature_id = match &contributor.feature {
            Some(f) => f.id.to_string(),
            None => continue,
        };

        // O(1) lookup from cache instead of O(n) directory scan
        if let Some(feature) = features.get_feature(&feature_id) {
            // Check dimensional sync
            if contributor.is_out_of_sync(&feature) {
                if fix {
                    contributor.sync_from_feature(&feature);
                    any_synced = true;
                } else {
                    if let Some(dim) = feature.primary_dimension() {
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
                                "Contributor '{}' has stale cached component_id '{}' (feature belongs to '{}')",
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

    // Write back if we synced any contributors
    if fix && any_synced {
        let updated_content = serde_yml::to_string(&stackup)
            .map_err(|e| miette::miette!("Failed to serialize YAML: {}", e))?;
        fs::write(path, updated_content)
            .map_err(|e| miette::miette!("Failed to write file: {}", e))?;

        stats.files_fixed += 1;
    }

    Ok(issues)
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
