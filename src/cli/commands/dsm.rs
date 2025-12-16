//! `tdt dsm` command - Design Structure Matrix for component interactions
//!
//! Generates a DSM showing relationships between components based on:
//! - Mates (physical interfaces via features)
//! - Tolerance stackups (dimensional chains linking components)
//! - Processes (shared manufacturing processes)
//! - Requirements (allocated to same requirement)

use console::style;
use miette::{IntoDiagnostic, Result};
use std::collections::{HashMap, HashSet};
use std::fs;

use crate::cli::helpers::format_short_id_str;
use crate::cli::GlobalOpts;
use crate::core::cache::EntityCache;
use crate::core::project::Project;

#[derive(clap::Args, Debug)]
pub struct DsmArgs {
    /// Assembly ID to scope the DSM (optional - defaults to all components)
    pub assembly: Option<String>,

    /// Relationship types to include (mate, tolerance, process, requirement, all)
    #[arg(long, short = 't', default_value = "all")]
    pub rel_type: String,

    /// Output format: table, csv, dot (graphviz), json
    #[arg(long, short = 'o', default_value = "table")]
    pub output: String,

    /// Apply clustering optimization to group related components
    #[arg(long, short = 'c')]
    pub cluster: bool,

    /// Show full IDs instead of short IDs
    #[arg(long)]
    pub full_ids: bool,

    /// Show numeric dependency strength instead of relationship type symbols
    #[arg(long, short = 'w')]
    pub weighted: bool,

    /// Show coupling metrics (fan-in, fan-out, coupling coefficient)
    #[arg(long, short = 'm')]
    pub metrics: bool,

    /// Highlight and report dependency cycles
    #[arg(long)]
    pub cycles: bool,
}

/// Relationship between two components
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationType {
    Mate,
    Tolerance,
    Process,
    Requirement,
}

impl RelationType {
    fn symbol(&self) -> &'static str {
        match self {
            RelationType::Mate => "M",
            RelationType::Tolerance => "T",
            RelationType::Process => "P",
            RelationType::Requirement => "R",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            RelationType::Mate => "Mate",
            RelationType::Tolerance => "Tolerance",
            RelationType::Process => "Process",
            RelationType::Requirement => "Requirement",
        }
    }
}

/// A cell in the DSM matrix
#[derive(Debug, Clone, Default)]
pub struct DsmCell {
    pub relationships: HashSet<RelationType>,
}

impl DsmCell {
    fn is_empty(&self) -> bool {
        self.relationships.is_empty()
    }

    fn symbol(&self) -> String {
        if self.relationships.is_empty() {
            return String::new();
        }
        let mut symbols: Vec<&str> = self.relationships.iter().map(|r| r.symbol()).collect();
        symbols.sort();
        symbols.join(",")
    }

    fn count(&self) -> usize {
        self.relationships.len()
    }
}

/// Component info for DSM
#[derive(Debug, Clone)]
pub struct DsmComponent {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub part_number: Option<String>,
}

/// The full DSM structure
pub struct Dsm {
    pub components: Vec<DsmComponent>,
    pub matrix: Vec<Vec<DsmCell>>,
    pub component_index: HashMap<String, usize>,
}

impl Dsm {
    fn new(components: Vec<DsmComponent>) -> Self {
        let n = components.len();
        let mut component_index = HashMap::new();
        for (i, cmp) in components.iter().enumerate() {
            component_index.insert(cmp.id.clone(), i);
        }

        let matrix = vec![vec![DsmCell::default(); n]; n];

        Self {
            components,
            matrix,
            component_index,
        }
    }

    fn add_relationship(&mut self, cmp1_id: &str, cmp2_id: &str, rel_type: RelationType) {
        if let (Some(&i), Some(&j)) = (
            self.component_index.get(cmp1_id),
            self.component_index.get(cmp2_id),
        ) {
            if i != j {
                // Symmetric - add to both cells
                self.matrix[i][j].relationships.insert(rel_type.clone());
                self.matrix[j][i].relationships.insert(rel_type);
            }
        }
    }

    /// Apply clustering to minimize off-diagonal distance
    #[allow(clippy::needless_range_loop)]
    fn cluster(&mut self) {
        let n = self.components.len();
        if n <= 2 {
            return;
        }

        // Calculate coupling strength for each component pair
        let mut coupling: Vec<Vec<usize>> = vec![vec![0; n]; n];
        for i in 0..n {
            for j in 0..n {
                coupling[i][j] = self.matrix[i][j].count();
            }
        }

        // Simple greedy clustering
        let mut ordered: Vec<usize> = Vec::with_capacity(n);
        let mut remaining: HashSet<usize> = (0..n).collect();

        // Find component with most total connections
        let start = (0..n)
            .max_by_key(|&i| coupling[i].iter().sum::<usize>())
            .unwrap_or(0);
        ordered.push(start);
        remaining.remove(&start);

        // Greedily add most connected to current cluster
        while !remaining.is_empty() {
            let mut best_next = *remaining.iter().next().unwrap();
            let mut best_score = 0usize;

            for &candidate in &remaining {
                let score: usize = ordered.iter().map(|&o| coupling[candidate][o]).sum();
                if score > best_score {
                    best_score = score;
                    best_next = candidate;
                }
            }

            ordered.push(best_next);
            remaining.remove(&best_next);
        }

        // Reorder components and matrix
        let old_components = self.components.clone();
        let old_matrix = self.matrix.clone();

        for (new_i, &old_i) in ordered.iter().enumerate() {
            self.components[new_i] = old_components[old_i].clone();
            for (new_j, &old_j) in ordered.iter().enumerate() {
                self.matrix[new_i][new_j] = old_matrix[old_i][old_j].clone();
            }
        }

        // Rebuild index
        self.component_index.clear();
        for (i, cmp) in self.components.iter().enumerate() {
            self.component_index.insert(cmp.id.clone(), i);
        }
    }

    /// Identify clusters (groups of tightly coupled components)
    fn identify_clusters(&self) -> Vec<Vec<usize>> {
        let n = self.components.len();
        if n == 0 {
            return vec![];
        }

        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let pi = find(parent, i);
            let pj = find(parent, j);
            if pi != pj {
                parent[pi] = pj;
            }
        }

        for i in 0..n {
            for j in (i + 1)..n {
                if !self.matrix[i][j].is_empty() {
                    union(&mut parent, i, j);
                }
            }
        }

        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            clusters.entry(root).or_default().push(i);
        }

        clusters.into_values().collect()
    }

    /// Calculate coupling metrics for each component
    fn calculate_metrics(&self) -> Vec<CouplingMetrics> {
        let n = self.components.len();
        let mut metrics = Vec::with_capacity(n);

        for i in 0..n {
            let mut fan_out = 0; // Outgoing dependencies (row)
            let mut fan_in = 0; // Incoming dependencies (column)

            for j in 0..n {
                if i != j {
                    if !self.matrix[i][j].is_empty() {
                        fan_out += self.matrix[i][j].count();
                    }
                    if !self.matrix[j][i].is_empty() {
                        fan_in += self.matrix[j][i].count();
                    }
                }
            }

            // For symmetric DSM, fan_in == fan_out, so we use total connections
            let total_connections = fan_out; // Since symmetric
            let max_possible = (n - 1) * 4; // 4 relationship types max per cell (M, T, P, R)
            let coupling_coefficient = if max_possible > 0 {
                (total_connections as f64) / (max_possible as f64)
            } else {
                0.0
            };

            metrics.push(CouplingMetrics {
                component_idx: i,
                fan_in,
                fan_out,
                total: total_connections,
                coupling_coefficient,
                is_hub: fan_in > 0 && fan_out > 0 && total_connections >= n - 1,
            });
        }

        metrics
    }

    /// Detect dependency cycles (strongly connected components with size > 1)
    fn detect_cycles(&self) -> Vec<Vec<usize>> {
        let n = self.components.len();
        if n == 0 {
            return vec![];
        }

        // For symmetric DSM, any connected component with >1 elements forms a "cycle"
        // In the sense that changes propagate bidirectionally
        let clusters = self.identify_clusters();

        // Filter to clusters with more than one component (actual cycles)
        clusters.into_iter().filter(|c| c.len() > 1).collect()
    }
}

/// Coupling metrics for a single component
#[derive(Debug)]
pub struct CouplingMetrics {
    pub component_idx: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub total: usize,
    pub coupling_coefficient: f64,
    pub is_hub: bool, // High connectivity in both directions
}

pub fn run(args: DsmArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project)?;

    // Get components
    let components = if let Some(ref asm_id) = args.assembly {
        get_assembly_components(&cache, &project, asm_id)?
    } else {
        get_all_components(&cache)?
    };

    if components.is_empty() {
        if args.assembly.is_some() {
            println!("No components found in specified assembly");
        } else {
            println!("No components found in project");
        }
        return Ok(());
    }

    // Build DSM
    let mut dsm = Dsm::new(components);

    // Determine which relationship types to include
    let include_mate = args.rel_type == "all" || args.rel_type == "mate";
    let include_tolerance = args.rel_type == "all" || args.rel_type == "tolerance";
    let include_process = args.rel_type == "all" || args.rel_type == "process";
    let include_req = args.rel_type == "all" || args.rel_type == "requirement";

    // Find relationships
    if include_mate {
        add_mate_relationships(&project, &mut dsm)?;
    }

    if include_tolerance {
        add_tolerance_relationships(&project, &mut dsm)?;
    }

    if include_process {
        add_process_relationships(&cache, &mut dsm)?;
    }

    if include_req {
        add_requirement_relationships(&cache, &mut dsm)?;
    }

    // Apply clustering if requested
    if args.cluster {
        dsm.cluster();
    }

    // Output
    let display_opts = DisplayOptions {
        full_ids: args.full_ids,
        clustered: args.cluster,
        weighted: args.weighted,
        show_metrics: args.metrics,
        show_cycles: args.cycles,
    };

    match args.output.as_str() {
        "csv" => output_csv(&dsm, &display_opts),
        "dot" => output_dot(&dsm, &display_opts),
        "json" => output_json(&dsm, &display_opts),
        _ => output_table(&dsm, &display_opts),
    }

    Ok(())
}

/// Display options for DSM output
#[derive(Debug, Clone)]
struct DisplayOptions {
    full_ids: bool,
    clustered: bool,
    weighted: bool,
    show_metrics: bool,
    show_cycles: bool,
}

fn get_assembly_components(
    cache: &EntityCache,
    project: &Project,
    asm_id: &str,
) -> Result<Vec<DsmComponent>> {
    // Resolve assembly ID
    let resolved_id = cache
        .resolve_short_id(asm_id)
        .unwrap_or_else(|| asm_id.to_string());

    // Find the assembly file and load it
    let asm_dir = project.root().join("bom/assemblies");
    let mut components = Vec::new();

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(asm) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let file_asm_id = asm.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        if file_asm_id == resolved_id {
                            // Found the assembly - extract BOM component IDs
                            if let Some(bom) = asm.get("bom").and_then(|v| v.as_array()) {
                                for item in bom {
                                    if let Some(comp_id) =
                                        item.get("component_id").and_then(|v| v.as_str())
                                    {
                                        if let Some(cmp) = get_component_info(cache, comp_id) {
                                            components.push(cmp);
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(components)
}

fn get_all_components(cache: &EntityCache) -> Result<Vec<DsmComponent>> {
    let cmp_list = cache.list_components(None, None, None, None, None, None);

    let mut components = Vec::new();
    for cmp in cmp_list {
        let short_id = cache
            .get_short_id(&cmp.id)
            .unwrap_or_else(|| format_short_id_str(&cmp.id));
        components.push(DsmComponent {
            id: cmp.id,
            short_id,
            title: cmp.title,
            part_number: cmp.part_number,
        });
    }

    Ok(components)
}

fn get_component_info(cache: &EntityCache, id: &str) -> Option<DsmComponent> {
    // Try to get from cache list
    let components = cache.list_components(None, None, None, None, None, None);
    for cmp in components {
        if cmp.id == id {
            let short_id = cache
                .get_short_id(&cmp.id)
                .unwrap_or_else(|| format_short_id_str(&cmp.id));
            return Some(DsmComponent {
                id: cmp.id,
                short_id,
                title: cmp.title,
                part_number: cmp.part_number,
            });
        }
    }
    None
}

fn add_mate_relationships(project: &Project, dsm: &mut Dsm) -> Result<()> {
    // Build feature-to-component lookup from feature files
    let feature_dir = project.root().join("tolerances/features");
    let mut feature_to_component: HashMap<String, String> = HashMap::new();

    if feature_dir.exists() {
        for entry in fs::read_dir(&feature_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(feat) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let feat_id = feat.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let comp_id = feat.get("component").and_then(|v| v.as_str()).unwrap_or("");

                        if !feat_id.is_empty() && !comp_id.is_empty() {
                            feature_to_component.insert(feat_id.to_string(), comp_id.to_string());
                        }
                    }
                }
            }
        }
    }

    // Load mates and extract feature relationships
    let mate_dir = project.root().join("tolerances/mates");
    if mate_dir.exists() {
        for entry in fs::read_dir(&mate_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(mate) = serde_yml::from_str::<serde_json::Value>(&content) {
                        // Get component IDs from mate via feature_a and feature_b
                        let comp_a = get_component_from_feature_field(
                            &mate,
                            "feature_a",
                            &feature_to_component,
                        );
                        let comp_b = get_component_from_feature_field(
                            &mate,
                            "feature_b",
                            &feature_to_component,
                        );

                        if let (Some(cmp1), Some(cmp2)) = (comp_a, comp_b) {
                            if cmp1 != cmp2 {
                                dsm.add_relationship(&cmp1, &cmp2, RelationType::Mate);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extract component ID from a mate's feature field (feature_a or feature_b)
fn get_component_from_feature_field(
    mate: &serde_json::Value,
    field: &str,
    feature_to_component: &HashMap<String, String>,
) -> Option<String> {
    let feat = mate.get(field)?;

    // Format 1: Simple string "FEAT-xxx"
    if let Some(feat_id) = feat.as_str() {
        return feature_to_component.get(feat_id).cloned();
    }

    // Format 2: Object with component_id directly
    if let Some(comp_id) = feat.get("component_id").and_then(|v| v.as_str()) {
        return Some(comp_id.to_string());
    }

    // Format 3: Object with id field - look up from feature files
    if let Some(feat_id) = feat.get("id").and_then(|v| v.as_str()) {
        return feature_to_component.get(feat_id).cloned();
    }

    None
}

fn add_tolerance_relationships(project: &Project, dsm: &mut Dsm) -> Result<()> {
    // Build feature-to-component lookup from feature files
    let feature_dir = project.root().join("tolerances/features");
    let mut feature_to_component: HashMap<String, String> = HashMap::new();

    if feature_dir.exists() {
        for entry in fs::read_dir(&feature_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(feat) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let feat_id = feat.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let comp_id = feat.get("component").and_then(|v| v.as_str()).unwrap_or("");

                        if !feat_id.is_empty() && !comp_id.is_empty() {
                            feature_to_component.insert(feat_id.to_string(), comp_id.to_string());
                        }
                    }
                }
            }
        }
    }

    // Load tolerance stackups and extract component relationships
    let stackup_dir = project.root().join("tolerances/stackups");
    if stackup_dir.exists() {
        for entry in fs::read_dir(&stackup_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(stackup) = serde_yml::from_str::<serde_json::Value>(&content) {
                        // Collect all unique components in the stackup
                        let mut stackup_components: Vec<String> = Vec::new();

                        if let Some(contributors) =
                            stackup.get("contributors").and_then(|c| c.as_array())
                        {
                            for contrib in contributors {
                                let comp_id = if let Some(feat_id) =
                                    contrib.get("feature_id").and_then(|v| v.as_str())
                                {
                                    // Simple feature_id format - look up from feature file
                                    feature_to_component.get(feat_id).cloned()
                                } else if let Some(feature) = contrib.get("feature") {
                                    // Nested feature object
                                    if let Some(cid) =
                                        feature.get("component_id").and_then(|v| v.as_str())
                                    {
                                        // Has component_id directly
                                        Some(cid.to_string())
                                    } else if let Some(feat_id) =
                                        feature.get("id").and_then(|v| v.as_str())
                                    {
                                        // Only has feature id - look up component
                                        feature_to_component.get(feat_id).cloned()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if let Some(cid) = comp_id {
                                    if !stackup_components.contains(&cid) {
                                        stackup_components.push(cid);
                                    }
                                }
                            }
                        }

                        // Create relationships for all pairs in the stackup
                        for i in 0..stackup_components.len() {
                            for j in (i + 1)..stackup_components.len() {
                                dsm.add_relationship(
                                    &stackup_components[i],
                                    &stackup_components[j],
                                    RelationType::Tolerance,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn add_process_relationships(cache: &EntityCache, dsm: &mut Dsm) -> Result<()> {
    // Build a map of process -> components
    let mut process_components: HashMap<String, HashSet<String>> = HashMap::new();

    let processes = cache.list_processes(None, None, None, None, None, None);

    for proc in processes {
        // Get components linked to this process
        let links = cache.get_links_from(&proc.id);
        for link in links {
            if link.target_id.starts_with("CMP-") {
                process_components
                    .entry(proc.id.clone())
                    .or_default()
                    .insert(link.target_id);
            }
        }

        // Also check reverse links
        let reverse_links = cache.get_links_to(&proc.id);
        for link in reverse_links {
            if link.source_id.starts_with("CMP-") {
                process_components
                    .entry(proc.id.clone())
                    .or_default()
                    .insert(link.source_id);
            }
        }
    }

    // Components sharing a process are related
    for (_proc_id, cmp_ids) in process_components {
        let cmp_list: Vec<_> = cmp_ids.into_iter().collect();
        for i in 0..cmp_list.len() {
            for j in (i + 1)..cmp_list.len() {
                dsm.add_relationship(&cmp_list[i], &cmp_list[j], RelationType::Process);
            }
        }
    }

    Ok(())
}

fn add_requirement_relationships(cache: &EntityCache, dsm: &mut Dsm) -> Result<()> {
    // Build a map of requirement -> components
    let mut req_components: HashMap<String, HashSet<String>> = HashMap::new();

    let reqs = cache.list_requirements(None, None, None, None, None, None, None);

    for req in reqs {
        let links = cache.get_links_from(&req.id);
        for link in links {
            if link.target_id.starts_with("CMP-") {
                req_components
                    .entry(req.id.clone())
                    .or_default()
                    .insert(link.target_id);
            }
        }

        let reverse_links = cache.get_links_to(&req.id);
        for link in reverse_links {
            if link.source_id.starts_with("CMP-") {
                req_components
                    .entry(req.id.clone())
                    .or_default()
                    .insert(link.source_id);
            }
        }
    }

    for (_req_id, cmp_ids) in req_components {
        let cmp_list: Vec<_> = cmp_ids.into_iter().collect();
        for i in 0..cmp_list.len() {
            for j in (i + 1)..cmp_list.len() {
                dsm.add_relationship(&cmp_list[i], &cmp_list[j], RelationType::Requirement);
            }
        }
    }

    Ok(())
}

fn output_table(dsm: &Dsm, opts: &DisplayOptions) {
    let n = dsm.components.len();
    if n == 0 {
        return;
    }

    // Header
    let title = if opts.clustered {
        "Design Structure Matrix (Clustered)"
    } else {
        "Design Structure Matrix"
    };
    println!("{}", style(title).bold().cyan());
    println!();

    // Calculate row label width (left column)
    let id_width = if opts.full_ids {
        dsm.components
            .iter()
            .map(|c| c.id.len())
            .max()
            .unwrap_or(8)
            .min(20)
    } else {
        dsm.components
            .iter()
            .map(|c| c.short_id.len())
            .max()
            .unwrap_or(6)
            .max(6)
    };

    // Calculate column width based on longest short ID + padding
    let max_col_label = dsm
        .components
        .iter()
        .map(|c| c.short_id.len())
        .max()
        .unwrap_or(5);
    let cell_width = (max_col_label + 2).max(7); // At least 7 chars per column

    // Detect cycles if requested
    let cycles = if opts.show_cycles {
        dsm.detect_cycles()
    } else {
        vec![]
    };

    // Build set of component indices that are in cycles
    let in_cycle: HashSet<usize> = cycles.iter().flatten().copied().collect();

    // Print column headers
    print!("{:width$} ", "", width = id_width);
    for cmp in &dsm.components {
        let label = if opts.full_ids {
            format_short_id_str(&cmp.id)
        } else {
            cmp.short_id.clone()
        };
        print!("{:^width$}", label, width = cell_width);
    }
    println!();

    // Print separator
    print!("{:-<width$} ", "", width = id_width);
    for _ in &dsm.components {
        print!("{:-<width$}", "", width = cell_width);
    }
    println!();

    // Print rows
    for (i, cmp) in dsm.components.iter().enumerate() {
        let label = if opts.full_ids {
            format_short_id_str(&cmp.id)
        } else {
            cmp.short_id.clone()
        };

        // Highlight row label if in cycle
        if opts.show_cycles && in_cycle.contains(&i) {
            print!("{:<width$} ", style(label).red().bold(), width = id_width);
        } else {
            print!("{:<width$} ", label, width = id_width);
        }

        for (j, _) in dsm.components.iter().enumerate() {
            if i == j {
                print!("{:^width$}", style("■").dim(), width = cell_width);
            } else {
                let cell = &dsm.matrix[i][j];
                if cell.is_empty() {
                    print!("{:^width$}", "·", width = cell_width);
                } else if opts.weighted {
                    // Weighted mode: show numeric count
                    let count = cell.count();
                    let styled = match count {
                        1 => style(count.to_string()).dim(),
                        2 => style(count.to_string()).yellow(),
                        _ => style(count.to_string()).red().bold(),
                    };
                    print!("{:^width$}", styled, width = cell_width);
                } else {
                    // Symbol mode
                    let symbol = cell.symbol();
                    let styled = match symbol.as_str() {
                        "M" => style(symbol).green(),
                        "T" => style(symbol).magenta(),
                        "P" => style(symbol).blue(),
                        "R" => style(symbol).yellow(),
                        _ => style(symbol).white(),
                    };
                    print!("{:^width$}", styled, width = cell_width);
                }
            }
        }
        println!();
    }

    // Legend
    println!();
    if opts.weighted {
        println!(
            "{}: Numbers show dependency strength (count of relationship types)",
            style("Legend").bold()
        );
    } else {
        println!(
            "{}: {} = Mate {} = Tolerance {} = Process {} = Requirement",
            style("Legend").bold(),
            style("M").green(),
            style("T").magenta(),
            style("P").blue(),
            style("R").yellow()
        );
    }

    // Statistics
    let mut mate_count = 0;
    let mut tolerance_count = 0;
    let mut process_count = 0;
    let mut req_count = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            let cell = &dsm.matrix[i][j];
            if cell.relationships.contains(&RelationType::Mate) {
                mate_count += 1;
            }
            if cell.relationships.contains(&RelationType::Tolerance) {
                tolerance_count += 1;
            }
            if cell.relationships.contains(&RelationType::Process) {
                process_count += 1;
            }
            if cell.relationships.contains(&RelationType::Requirement) {
                req_count += 1;
            }
        }
    }

    println!();
    println!(
        "{}: {} components, {} mate, {} tolerance, {} process, {} requirement",
        style("Summary").bold(),
        n,
        mate_count,
        tolerance_count,
        process_count,
        req_count
    );

    // Clusters (if enabled)
    if opts.clustered {
        let clusters = dsm.identify_clusters();
        if clusters.len() > 1 {
            println!();
            println!("{}", style("Identified Clusters:").bold());
            for (i, cluster) in clusters.iter().enumerate() {
                let names: Vec<_> = cluster
                    .iter()
                    .map(|&idx| dsm.components[idx].short_id.clone())
                    .collect();
                println!("  Cluster {}: {}", i + 1, names.join(", "));
            }
        }
    }

    // Cycles (if enabled)
    if opts.show_cycles {
        println!();
        if cycles.is_empty() {
            println!("{}: None detected", style("Dependency Cycles").bold());
        } else {
            println!(
                "{}: {} cycle group(s) detected",
                style("Dependency Cycles").bold().red(),
                cycles.len()
            );
            for (i, cycle) in cycles.iter().enumerate() {
                let names: Vec<_> = cycle
                    .iter()
                    .map(|&idx| dsm.components[idx].short_id.clone())
                    .collect();
                println!(
                    "  {} {}: {}",
                    style("Cycle").red(),
                    i + 1,
                    names.join(" <-> ")
                );
            }
            println!();
            println!(
                "  {}",
                style("Components in cycles have bidirectional dependencies").dim()
            );
        }
    }

    // Metrics (if enabled)
    if opts.show_metrics {
        output_metrics(dsm);
    }
}

fn output_metrics(dsm: &Dsm) {
    let metrics = dsm.calculate_metrics();

    println!();
    println!("{}", style("Coupling Metrics").bold().cyan());
    println!();

    // Header
    println!(
        "  {:<12} {:>8} {:>8} {:>8} {:>12}",
        "Component", "Fan-in", "Fan-out", "Total", "Coupling %"
    );
    println!("  {:-<12} {:->8} {:->8} {:->8} {:->12}", "", "", "", "", "");

    // Metrics for each component
    for m in &metrics {
        let cmp = &dsm.components[m.component_idx];
        let coupling_pct = format!("{:.1}%", m.coupling_coefficient * 100.0);

        let hub_marker = if m.is_hub {
            style(" ★").yellow().to_string()
        } else {
            String::new()
        };

        println!(
            "  {:<12} {:>8} {:>8} {:>8} {:>12}{}",
            cmp.short_id, m.fan_in, m.fan_out, m.total, coupling_pct, hub_marker
        );
    }

    // Summary stats
    let total_connections: usize = metrics.iter().map(|m| m.total).sum();
    let avg_coupling: f64 =
        metrics.iter().map(|m| m.coupling_coefficient).sum::<f64>() / metrics.len() as f64;
    let hubs: Vec<_> = metrics
        .iter()
        .filter(|m| m.is_hub)
        .map(|m| dsm.components[m.component_idx].short_id.clone())
        .collect();

    println!();
    println!(
        "  Total connections: {} | Avg coupling: {:.1}%",
        total_connections / 2, // Divide by 2 since symmetric
        avg_coupling * 100.0
    );
    if !hubs.is_empty() {
        println!(
            "  {} {} (high connectivity)",
            style("Hubs:").yellow(),
            hubs.join(", ")
        );
    }
}

fn output_csv(dsm: &Dsm, opts: &DisplayOptions) {
    let n = dsm.components.len();
    if n == 0 {
        return;
    }

    // Header row
    print!("Component");
    for cmp in &dsm.components {
        let label = if opts.full_ids {
            &cmp.id
        } else {
            &cmp.short_id
        };
        print!(",{}", label);
    }
    println!();

    // Data rows
    for (i, cmp) in dsm.components.iter().enumerate() {
        let label = if opts.full_ids {
            &cmp.id
        } else {
            &cmp.short_id
        };
        print!("{}", label);

        for (j, _) in dsm.components.iter().enumerate() {
            if i == j {
                print!(",X");
            } else {
                let cell = &dsm.matrix[i][j];
                if cell.is_empty() {
                    print!(",");
                } else if opts.weighted {
                    print!(",{}", cell.count());
                } else {
                    print!(",{}", cell.symbol());
                }
            }
        }
        println!();
    }
}

fn output_dot(dsm: &Dsm, opts: &DisplayOptions) {
    println!("graph DSM {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box];");
    println!();

    // Detect cycles for highlighting
    let cycles = if opts.show_cycles {
        dsm.detect_cycles()
    } else {
        vec![]
    };
    let in_cycle: HashSet<usize> = cycles.iter().flatten().copied().collect();

    for (i, cmp) in dsm.components.iter().enumerate() {
        let label = format!("{}\\n{}", cmp.short_id, cmp.title);
        let color = if opts.show_cycles && in_cycle.contains(&i) {
            " fillcolor=lightcoral style=filled"
        } else {
            ""
        };
        println!("  \"{}\" [label=\"{}\"{}];", cmp.id, label, color);
    }
    println!();

    let n = dsm.components.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let cell = &dsm.matrix[i][j];
            if !cell.is_empty() {
                let color = if cell.relationships.contains(&RelationType::Mate) {
                    "green"
                } else if cell.relationships.contains(&RelationType::Tolerance) {
                    "magenta"
                } else if cell.relationships.contains(&RelationType::Process) {
                    "blue"
                } else {
                    "orange"
                };
                let label = if opts.weighted {
                    cell.count().to_string()
                } else {
                    cell.symbol()
                };
                let penwidth = if cell.count() > 1 { " penwidth=2" } else { "" };
                println!(
                    "  \"{}\" -- \"{}\" [label=\"{}\" color={}{}];",
                    dsm.components[i].id, dsm.components[j].id, label, color, penwidth
                );
            }
        }
    }

    println!("}}");
}

fn output_json(dsm: &Dsm, opts: &DisplayOptions) {
    let mut components_json: Vec<serde_json::Value> = Vec::new();

    // Calculate metrics if requested
    let metrics = if opts.show_metrics {
        Some(dsm.calculate_metrics())
    } else {
        None
    };

    for (i, cmp) in dsm.components.iter().enumerate() {
        let mut cmp_json = serde_json::json!({
            "id": cmp.id,
            "short_id": cmp.short_id,
            "title": cmp.title,
            "part_number": cmp.part_number,
        });

        if let Some(ref m) = metrics {
            cmp_json["metrics"] = serde_json::json!({
                "fan_in": m[i].fan_in,
                "fan_out": m[i].fan_out,
                "total": m[i].total,
                "coupling_coefficient": m[i].coupling_coefficient,
                "is_hub": m[i].is_hub,
            });
        }

        components_json.push(cmp_json);
    }

    let n = dsm.components.len();
    let mut relationships: Vec<serde_json::Value> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let cell = &dsm.matrix[i][j];
            if !cell.is_empty() {
                let types: Vec<&str> = cell.relationships.iter().map(|r| r.name()).collect();
                relationships.push(serde_json::json!({
                    "source": dsm.components[i].id,
                    "target": dsm.components[j].id,
                    "types": types,
                    "weight": cell.count(),
                }));
            }
        }
    }

    // Build output
    let mut output = serde_json::json!({
        "components": components_json,
        "relationships": relationships,
        "matrix_size": n,
    });

    // Add cycles if requested
    if opts.show_cycles {
        let cycles = dsm.detect_cycles();
        let cycle_json: Vec<Vec<String>> = cycles
            .iter()
            .map(|c| {
                c.iter()
                    .map(|&idx| dsm.components[idx].short_id.clone())
                    .collect()
            })
            .collect();
        output["cycles"] = serde_json::json!(cycle_json);
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
}
