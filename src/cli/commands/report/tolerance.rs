//! Tolerance Analysis report

use miette::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::helpers::truncate_str;
use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::assembly::Assembly;
use crate::entities::component::Component;
use crate::entities::feature::Feature;
use crate::entities::mate::Mate;
use crate::entities::stackup::{AnalysisResult, Stackup};
use tabled::{builder::Builder, settings::Style};

use super::{
    load_all_assemblies, load_all_components, load_all_features, load_all_mates, load_all_stackups,
    write_output,
};

#[derive(clap::Args, Debug)]
pub struct ToleranceArgs {
    /// Output to file instead of stdout
    #[arg(long, short = 'f')]
    pub file: Option<PathBuf>,

    /// Filter to specific assembly (shows components in assembly + external contributors)
    #[arg(long, short = 'a')]
    pub assembly: Option<String>,

    /// Filter to specific component
    #[arg(long, short = 'c')]
    pub component: Option<String>,

    /// Only show stackups with issues (marginal or fail)
    #[arg(long)]
    pub issues_only: bool,
}

pub fn run(args: ToleranceArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Load all entities
    let components = load_all_components(&project);
    let features = load_all_features(&project);
    let mates = load_all_mates(&project);
    let mut stackups = load_all_stackups(&project);
    let assemblies = load_all_assemblies(&project);

    // Build lookup maps
    let component_map: HashMap<String, &Component> =
        components.iter().map(|c| (c.id.to_string(), c)).collect();
    let _feature_map: HashMap<String, &Feature> =
        features.iter().map(|f| (f.id.to_string(), f)).collect();

    // Build feature -> component lookup
    let feature_to_component: HashMap<String, String> = features
        .iter()
        .map(|f| (f.id.to_string(), f.component.clone()))
        .collect();

    // Determine which components are in scope
    let mut in_scope_components: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut assembly_title: Option<String> = None;

    if let Some(ref asm_id) = args.assembly {
        // Resolve assembly ID
        let resolved_asm = short_ids.resolve(asm_id).unwrap_or_else(|| asm_id.clone());

        // Find the assembly
        if let Some(asm) = assemblies.iter().find(|a| a.id.to_string() == resolved_asm) {
            assembly_title = Some(format!(
                "{} ({})",
                asm.title,
                short_ids
                    .get_short_id(&asm.id.to_string())
                    .unwrap_or_else(|| asm.id.to_string())
            ));

            // Collect all component IDs from BOM (recursively)
            fn collect_bom_components(
                bom: &[crate::entities::assembly::BomItem],
                assembly_map: &HashMap<String, &Assembly>,
                result: &mut std::collections::HashSet<String>,
                visited: &mut std::collections::HashSet<String>,
            ) {
                for item in bom {
                    let id = item.component_id.to_string();
                    if result.insert(id.clone()) {
                        // If it's a sub-assembly, recurse
                        if let Some(sub_asm) = assembly_map.get(&id) {
                            if visited.insert(id.clone()) {
                                collect_bom_components(&sub_asm.bom, assembly_map, result, visited);
                            }
                        }
                    }
                }
            }

            let assembly_map: HashMap<String, &Assembly> =
                assemblies.iter().map(|a| (a.id.to_string(), a)).collect();
            let mut visited = std::collections::HashSet::new();
            collect_bom_components(
                &asm.bom,
                &assembly_map,
                &mut in_scope_components,
                &mut visited,
            );
        } else {
            return Err(miette::miette!("Assembly not found: {}", asm_id));
        }
    } else if let Some(ref cmp_id) = args.component {
        // Single component filter
        let resolved_cmp = short_ids.resolve(cmp_id).unwrap_or_else(|| cmp_id.clone());
        in_scope_components.insert(resolved_cmp);
    } else {
        // All components
        for c in &components {
            in_scope_components.insert(c.id.to_string());
        }
    }

    // Filter stackups if --issues-only
    if args.issues_only {
        stackups.retain(|s| {
            if let Some(ref wc) = s.analysis_results.worst_case {
                wc.result != AnalysisResult::Pass
            } else {
                false
            }
        });
    }

    // Find stackups that involve in-scope components
    let mut relevant_stackups: Vec<&Stackup> = Vec::new();
    let mut external_components: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for stackup in &stackups {
        let mut involves_in_scope = false;
        for contrib in &stackup.contributors {
            if let Some(ref feat_ref) = contrib.feature {
                if let Some(cmp_id) = feature_to_component.get(&feat_ref.id.to_string()) {
                    if in_scope_components.contains(cmp_id) {
                        involves_in_scope = true;
                    } else if args.assembly.is_some() {
                        // Track external components
                        external_components.insert(cmp_id.clone());
                    }
                }
            }
        }
        if involves_in_scope {
            relevant_stackups.push(stackup);
            // Also track all external components in this stackup
            for contrib in &stackup.contributors {
                if let Some(ref feat_ref) = contrib.feature {
                    if let Some(cmp_id) = feature_to_component.get(&feat_ref.id.to_string()) {
                        if !in_scope_components.contains(cmp_id) {
                            external_components.insert(cmp_id.clone());
                        }
                    }
                }
            }
        }
    }

    // Build component -> features map
    let mut component_features: HashMap<String, Vec<&Feature>> = HashMap::new();
    for feat in &features {
        component_features
            .entry(feat.component.clone())
            .or_default()
            .push(feat);
    }

    // Build component -> mates map (where either feature belongs to component)
    let mut component_mates: HashMap<String, Vec<&Mate>> = HashMap::new();
    for mate in &mates {
        // Check feature_a
        if let Some(cmp_id) = feature_to_component.get(&mate.feature_a.id.to_string()) {
            component_mates
                .entry(cmp_id.clone())
                .or_default()
                .push(mate);
        }
        // Check feature_b (may add same mate to different component)
        if let Some(cmp_id) = feature_to_component.get(&mate.feature_b.id.to_string()) {
            let entry = component_mates.entry(cmp_id.clone()).or_default();
            // Avoid duplicates if both features are on the same component
            if !entry.iter().any(|m| m.id == mate.id) {
                entry.push(mate);
            }
        }
    }

    // Build component -> stackup contributions map
    let mut component_stackup_contribs: HashMap<String, Vec<(&Stackup, Vec<usize>)>> =
        HashMap::new();
    for stackup in &relevant_stackups {
        for (idx, contrib) in stackup.contributors.iter().enumerate() {
            if let Some(ref feat_ref) = contrib.feature {
                if let Some(cmp_id) = feature_to_component.get(&feat_ref.id.to_string()) {
                    let entry = component_stackup_contribs
                        .entry(cmp_id.clone())
                        .or_default();
                    if let Some((_, indices)) = entry.iter_mut().find(|(s, _)| s.id == stackup.id) {
                        indices.push(idx);
                    } else {
                        entry.push((stackup, vec![idx]));
                    }
                }
            }
        }
    }

    // Generate report
    let mut output = String::new();
    output.push_str("# Tolerance Analysis Report\n\n");

    // Scope header
    if let Some(ref title) = assembly_title {
        output.push_str(&format!("**Scope:** Assembly {}\n\n", title));
    } else if let Some(ref cmp_id) = args.component {
        let resolved = short_ids.resolve(cmp_id).unwrap_or_else(|| cmp_id.clone());
        if let Some(cmp) = component_map.get(&resolved) {
            let short = short_ids
                .get_short_id(&resolved)
                .unwrap_or_else(|| resolved.clone());
            output.push_str(&format!(
                "**Scope:** Component {} {} ({})\n\n",
                short,
                cmp.title,
                if cmp.part_number.is_empty() {
                    "-"
                } else {
                    &cmp.part_number
                }
            ));
        }
    }

    output.push_str("---\n\n");
    output.push_str("## Components\n\n");

    // Track summary stats (prefixed with _ as currently unused but may be useful later)
    let mut _total_features = 0;
    let mut _total_mates = 0;
    let mut _total_stackup_refs = 0;
    let mut _components_written = 0;

    // Sort components by part number
    let mut sorted_components: Vec<_> = in_scope_components.iter().collect();
    sorted_components.sort_by(|a, b| {
        let pn_a = component_map.get(*a).map(|c| &c.part_number);
        let pn_b = component_map.get(*b).map(|c| &c.part_number);
        pn_a.cmp(&pn_b)
    });

    for cmp_id in sorted_components {
        let cmp = match component_map.get(cmp_id) {
            Some(c) => c,
            None => continue,
        };

        let cmp_short = short_ids
            .get_short_id(cmp_id)
            .unwrap_or_else(|| cmp_id.clone());
        let part_number = if cmp.part_number.is_empty() {
            "-"
        } else {
            &cmp.part_number
        };

        output.push_str(&format!(
            "### {}: {} (PN: {})\n\n",
            cmp_short, cmp.title, part_number
        ));

        // Features section
        let feats = component_features.get(cmp_id);
        if let Some(feats) = feats {
            if !feats.is_empty() {
                output.push_str("#### Features\n\n");
                let mut builder = Builder::default();
                builder.push_record([
                    "ID",
                    "Title",
                    "Type",
                    "Dimension",
                    "Nominal",
                    "+Tol",
                    "-Tol",
                    "MMC",
                    "LMC",
                ]);

                for feat in feats {
                    let feat_short = short_ids
                        .get_short_id(&feat.id.to_string())
                        .unwrap_or_else(|| feat.id.to_string());
                    let feat_type = format!("{}", feat.feature_type);

                    if feat.dimensions.is_empty() {
                        builder.push_record([
                            feat_short,
                            truncate_str(&feat.title, 25),
                            feat_type,
                            "-".to_string(),
                            "-".to_string(),
                            "-".to_string(),
                            "-".to_string(),
                            "-".to_string(),
                            "-".to_string(),
                        ]);
                    } else {
                        for (i, dim) in feat.dimensions.iter().enumerate() {
                            builder.push_record([
                                if i == 0 {
                                    feat_short.clone()
                                } else {
                                    "".to_string()
                                },
                                if i == 0 {
                                    truncate_str(&feat.title, 25)
                                } else {
                                    "".to_string()
                                },
                                if i == 0 {
                                    feat_type.clone()
                                } else {
                                    "".to_string()
                                },
                                dim.name.clone(),
                                format!("{:.3}", dim.nominal),
                                format!("{:.3}", dim.plus_tol),
                                format!("{:.3}", dim.minus_tol),
                                format!("{:.3}", dim.mmc()),
                                format!("{:.3}", dim.lmc()),
                            ]);
                        }
                    }
                    _total_features += 1;
                }
                output.push_str(&builder.build().with(Style::markdown()).to_string());
                output.push('\n');
            }
        }

        // Mates section
        let mates_for_cmp = component_mates.get(cmp_id);
        if let Some(mates_list) = mates_for_cmp {
            if !mates_list.is_empty() {
                output.push_str("#### Mates\n\n");
                let mut builder = Builder::default();
                builder.push_record([
                    "ID",
                    "Title",
                    "This Feature",
                    "Mating Feature",
                    "Mating Component",
                    "Fit",
                    "Min Clear",
                    "Max Clear",
                ]);

                for mate in mates_list {
                    let mate_short = short_ids
                        .get_short_id(&mate.id.to_string())
                        .unwrap_or_else(|| mate.id.to_string());

                    // Determine which feature is "this" vs "mating"
                    let feat_a_cmp = feature_to_component.get(&mate.feature_a.id.to_string());
                    let feat_b_cmp = feature_to_component.get(&mate.feature_b.id.to_string());

                    let (this_feat, other_feat, other_cmp_id) =
                        if feat_a_cmp.map(|c| c == cmp_id).unwrap_or(false) {
                            (&mate.feature_a, &mate.feature_b, feat_b_cmp)
                        } else {
                            (&mate.feature_b, &mate.feature_a, feat_a_cmp)
                        };

                    let this_id_str = this_feat.id.to_string();
                    let this_name = this_feat.name.as_deref().unwrap_or(&this_id_str);
                    let other_id_str = other_feat.id.to_string();
                    let other_name = other_feat.name.as_deref().unwrap_or(&other_id_str);

                    let other_cmp_display = if let Some(other_id) = other_cmp_id {
                        if let Some(other_cmp) = component_map.get(other_id) {
                            format!(
                                "{} ({})",
                                other_cmp.title,
                                if other_cmp.part_number.is_empty() {
                                    "-"
                                } else {
                                    &other_cmp.part_number
                                }
                            )
                        } else {
                            other_id.clone()
                        }
                    } else {
                        "-".to_string()
                    };

                    let (min_clear, max_clear, fit_str) =
                        if let Some(ref analysis) = mate.fit_analysis {
                            (
                                format!("{:+.4}", analysis.worst_case_min_clearance),
                                format!("{:+.4}", analysis.worst_case_max_clearance),
                                format!("{}", analysis.fit_result),
                            )
                        } else {
                            ("-".to_string(), "-".to_string(), "-".to_string())
                        };

                    builder.push_record([
                        mate_short,
                        truncate_str(&mate.title, 20),
                        truncate_str(this_name, 15),
                        truncate_str(other_name, 15),
                        truncate_str(&other_cmp_display, 25),
                        fit_str,
                        min_clear,
                        max_clear,
                    ]);
                    _total_mates += 1;
                }
                output.push_str(&builder.build().with(Style::markdown()).to_string());
                output.push('\n');
            }
        }

        // Stackup contributions section
        let stackup_contribs = component_stackup_contribs.get(cmp_id);
        if let Some(contribs) = stackup_contribs {
            if !contribs.is_empty() {
                output.push_str("#### Stackup Contributions\n\n");
                let mut builder = Builder::default();
                builder.push_record(["Stackup", "Title", "Contributor", "Direction"]);

                for (stackup, indices) in contribs {
                    let stack_short = short_ids
                        .get_short_id(&stackup.id.to_string())
                        .unwrap_or_else(|| stackup.id.to_string());

                    for (i, &idx) in indices.iter().enumerate() {
                        let contrib = &stackup.contributors[idx];
                        let dir = match contrib.direction {
                            crate::entities::stackup::Direction::Positive => "+",
                            crate::entities::stackup::Direction::Negative => "-",
                        };

                        builder.push_record([
                            if i == 0 {
                                stack_short.clone()
                            } else {
                                "".to_string()
                            },
                            if i == 0 {
                                truncate_str(&stackup.title, 20)
                            } else {
                                "".to_string()
                            },
                            format!(
                                "{}: {:.3} ±{:.3}/{:.3}",
                                contrib.name, contrib.nominal, contrib.plus_tol, contrib.minus_tol
                            ),
                            dir.to_string(),
                        ]);
                        _total_stackup_refs += 1;
                    }
                }
                output.push_str(&builder.build().with(Style::markdown()).to_string());
                output.push('\n');
            }
        }

        output.push_str("---\n\n");
        _components_written += 1;
    }

    // External contributors section (only if assembly scope)
    if args.assembly.is_some() && !external_components.is_empty() {
        output.push_str("## External Contributors\n\n");
        output.push_str("*Components not in assembly but contributing to stackups involving assembly components.*\n\n");

        for ext_cmp_id in &external_components {
            let cmp = match component_map.get(ext_cmp_id) {
                Some(c) => c,
                None => continue,
            };

            let cmp_short = short_ids
                .get_short_id(ext_cmp_id)
                .unwrap_or_else(|| ext_cmp_id.clone());
            let part_number = if cmp.part_number.is_empty() {
                "-"
            } else {
                &cmp.part_number
            };

            output.push_str(&format!(
                "### {}: {} (PN: {})\n\n",
                cmp_short, cmp.title, part_number
            ));

            // Show stackup contributions
            if let Some(contribs) = component_stackup_contribs.get(ext_cmp_id) {
                let mut builder = Builder::default();
                builder.push_record(["Stackup", "Contributor", "Direction"]);

                for (stackup, indices) in contribs {
                    let stack_short = short_ids
                        .get_short_id(&stackup.id.to_string())
                        .unwrap_or_else(|| stackup.id.to_string());

                    for &idx in indices {
                        let contrib = &stackup.contributors[idx];
                        let dir = match contrib.direction {
                            crate::entities::stackup::Direction::Positive => "+",
                            crate::entities::stackup::Direction::Negative => "-",
                        };

                        builder.push_record([
                            stack_short.clone(),
                            format!(
                                "{}: {:.3} ±{:.3}/{:.3}",
                                contrib.name, contrib.nominal, contrib.plus_tol, contrib.minus_tol
                            ),
                            dir.to_string(),
                        ]);
                    }
                }
                output.push_str(&builder.build().with(Style::markdown()).to_string());
                output.push('\n');
            }
        }

        output.push_str("---\n\n");
    }

    // Full stackups section
    if !relevant_stackups.is_empty() {
        output.push_str("## Stackups\n\n");

        for stackup in &relevant_stackups {
            let stack_short = short_ids
                .get_short_id(&stackup.id.to_string())
                .unwrap_or_else(|| stackup.id.to_string());

            output.push_str(&format!("### {}: {}\n\n", stack_short, stackup.title));

            // Target
            output.push_str(&format!(
                "**Target:** {} = {:.3} [{:.3} - {:.3}] {}\n\n",
                stackup.target.name,
                stackup.target.nominal,
                stackup.target.lower_limit,
                stackup.target.upper_limit,
                stackup.target.units
            ));

            // Contributors table
            let mut builder = Builder::default();
            builder.push_record([
                "#",
                "Contributor",
                "Component",
                "PN",
                "Nominal",
                "+Tol",
                "-Tol",
                "Dir",
            ]);

            for (idx, contrib) in stackup.contributors.iter().enumerate() {
                let (cmp_name, cmp_pn, external_marker) =
                    if let Some(ref feat_ref) = contrib.feature {
                        if let Some(cmp_id) = feature_to_component.get(&feat_ref.id.to_string()) {
                            let is_external =
                                args.assembly.is_some() && !in_scope_components.contains(cmp_id);
                            if let Some(cmp) = component_map.get(cmp_id) {
                                (
                                    cmp.title.clone(),
                                    if cmp.part_number.is_empty() {
                                        "-".to_string()
                                    } else {
                                        cmp.part_number.clone()
                                    },
                                    if is_external { "*" } else { "" },
                                )
                            } else {
                                ("-".to_string(), "-".to_string(), "")
                            }
                        } else {
                            ("-".to_string(), "-".to_string(), "")
                        }
                    } else {
                        ("-".to_string(), "-".to_string(), "")
                    };

                let dir = match contrib.direction {
                    crate::entities::stackup::Direction::Positive => "+",
                    crate::entities::stackup::Direction::Negative => "-",
                };

                builder.push_record([
                    (idx + 1).to_string(),
                    format!("{}{}", contrib.name, external_marker),
                    truncate_str(&cmp_name, 15),
                    cmp_pn,
                    format!("{:.3}", contrib.nominal),
                    format!("{:.3}", contrib.plus_tol),
                    format!("{:.3}", contrib.minus_tol),
                    dir.to_string(),
                ]);
            }
            output.push_str(&builder.build().with(Style::markdown()).to_string());

            if args.assembly.is_some()
                && external_components.iter().any(|ext| {
                    stackup.contributors.iter().any(|c| {
                        c.feature
                            .as_ref()
                            .and_then(|f| feature_to_component.get(&f.id.to_string()))
                            .map(|id| id == ext)
                            .unwrap_or(false)
                    })
                })
            {
                output.push_str("\n*\\* Not in assembly scope*\n");
            }

            output.push('\n');

            // Analysis results
            output.push_str("**Analysis:**\n\n");
            let mut analysis_builder = Builder::default();
            analysis_builder.push_record(["Method", "Result", "Value"]);

            if let Some(ref wc) = stackup.analysis_results.worst_case {
                let result_str = match wc.result {
                    AnalysisResult::Pass => "✓ Pass",
                    AnalysisResult::Marginal => "⚠ Marginal",
                    AnalysisResult::Fail => "✗ Fail",
                };
                analysis_builder.push_record([
                    "Worst Case".to_string(),
                    result_str.to_string(),
                    format!("{:.3} to {:.3} (margin: {:.3})", wc.min, wc.max, wc.margin),
                ]);
            }

            if let Some(ref rss) = stackup.analysis_results.rss {
                analysis_builder.push_record([
                    "RSS".to_string(),
                    format!("Cpk {:.2}", rss.cpk),
                    format!("mean: {:.3}, 3σ: ±{:.3}", rss.mean, rss.sigma_3),
                ]);
            }

            if let Some(ref mc) = stackup.analysis_results.monte_carlo {
                analysis_builder.push_record([
                    "Monte Carlo".to_string(),
                    format!("{:.1}% yield", mc.yield_percent),
                    format!("{} iterations", mc.iterations),
                ]);
            }

            output.push_str(&analysis_builder.build().with(Style::markdown()).to_string());
            output.push_str("\n---\n\n");
        }
    }

    // Summary section
    output.push_str("## Summary\n\n");

    // Components overview
    output.push_str("### Components\n\n");
    let mut summary_builder = Builder::default();
    summary_builder.push_record([
        "Part Number",
        "Title",
        "Features",
        "Mates",
        "Stackups",
        "In Scope",
    ]);

    let mut summary_components: Vec<_> = in_scope_components
        .iter()
        .chain(external_components.iter())
        .collect();
    summary_components.sort();
    summary_components.dedup();

    for cmp_id in summary_components {
        if let Some(cmp) = component_map.get(cmp_id) {
            let feat_count = component_features.get(cmp_id).map(|f| f.len()).unwrap_or(0);
            let mate_count = component_mates.get(cmp_id).map(|m| m.len()).unwrap_or(0);
            let stack_count = component_stackup_contribs
                .get(cmp_id)
                .map(|s| s.len())
                .unwrap_or(0);
            let in_scope = if in_scope_components.contains(cmp_id) {
                "✓"
            } else {
                "-"
            };

            summary_builder.push_record([
                if cmp.part_number.is_empty() {
                    "-".to_string()
                } else {
                    cmp.part_number.clone()
                },
                truncate_str(&cmp.title, 25),
                feat_count.to_string(),
                mate_count.to_string(),
                stack_count.to_string(),
                in_scope.to_string(),
            ]);
        }
    }
    output.push_str(&summary_builder.build().with(Style::markdown()).to_string());
    output.push('\n');

    // Stackup results summary
    if !relevant_stackups.is_empty() {
        output.push_str("### Stackup Results\n\n");

        let mut pass_count = 0;
        let mut marginal_count = 0;
        let mut fail_count = 0;
        let mut total_cpk = 0.0;
        let mut cpk_count = 0;
        let mut total_yield = 0.0;
        let mut yield_count = 0;

        for stackup in &relevant_stackups {
            if let Some(ref wc) = stackup.analysis_results.worst_case {
                match wc.result {
                    AnalysisResult::Pass => pass_count += 1,
                    AnalysisResult::Marginal => marginal_count += 1,
                    AnalysisResult::Fail => fail_count += 1,
                }
            }
            if let Some(ref rss) = stackup.analysis_results.rss {
                total_cpk += rss.cpk;
                cpk_count += 1;
            }
            if let Some(ref mc) = stackup.analysis_results.monte_carlo {
                total_yield += mc.yield_percent;
                yield_count += 1;
            }
        }

        let total = relevant_stackups.len();
        let pass_pct = if total > 0 {
            (pass_count as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let marginal_pct = if total > 0 {
            (marginal_count as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let fail_pct = if total > 0 {
            (fail_count as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let mut results_builder = Builder::default();
        results_builder.push_record(["Result", "Count", "Percentage"]);
        results_builder.push_record([
            "Pass".to_string(),
            pass_count.to_string(),
            format!("{:.1}%", pass_pct),
        ]);
        results_builder.push_record([
            "Marginal".to_string(),
            marginal_count.to_string(),
            format!("{:.1}%", marginal_pct),
        ]);
        results_builder.push_record([
            "Fail".to_string(),
            fail_count.to_string(),
            format!("{:.1}%", fail_pct),
        ]);
        output.push_str(&results_builder.build().with(Style::markdown()).to_string());
        output.push('\n');

        // Averages
        if cpk_count > 0 || yield_count > 0 {
            let avg_cpk = if cpk_count > 0 {
                total_cpk / cpk_count as f64
            } else {
                0.0
            };
            let avg_yield = if yield_count > 0 {
                total_yield / yield_count as f64
            } else {
                0.0
            };

            output.push_str(&format!(
                "**Average Cpk:** {:.2} | **Average Yield:** {:.1}%\n",
                avg_cpk, avg_yield
            ));
        }
    }

    // Report metadata
    output.push_str(&format!(
        "\n---\n\n*Generated: {}*\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    ));

    write_output(&output, args.file)?;
    Ok(())
}
