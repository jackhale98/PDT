//! `tdt dmm` command - Domain Mapping Matrix for cross-entity analysis
//!
//! DMM shows relationships between different entity types:
//! - Components vs Requirements (allocation analysis)
//! - Components vs Processes (manufacturing dependencies)
//! - Requirements vs Tests (verification coverage)
//! - And other combinations

use console::style;
use miette::Result;
use std::collections::HashMap;

use crate::cli::helpers::format_short_id_str;
use crate::cli::GlobalOpts;
use crate::core::cache::EntityCache;
use crate::core::project::Project;

/// Supported entity types for DMM
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EntityType {
    /// Components (CMP)
    Cmp,
    /// Requirements (REQ)
    Req,
    /// Processes (PROC)
    Proc,
    /// Tests (TEST)
    Test,
    /// Risks (RISK)
    Risk,
    /// Controls (CTRL)
    Ctrl,
}

impl EntityType {
    fn prefix(&self) -> &'static str {
        match self {
            EntityType::Cmp => "CMP",
            EntityType::Req => "REQ",
            EntityType::Proc => "PROC",
            EntityType::Test => "TEST",
            EntityType::Risk => "RISK",
            EntityType::Ctrl => "CTRL",
        }
    }

    fn display_name(&self) -> &'static str {
        match self {
            EntityType::Cmp => "Components",
            EntityType::Req => "Requirements",
            EntityType::Proc => "Processes",
            EntityType::Test => "Tests",
            EntityType::Risk => "Risks",
            EntityType::Ctrl => "Controls",
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct DmmArgs {
    /// Row entity type
    #[arg(value_enum)]
    pub row_type: EntityType,

    /// Column entity type
    #[arg(value_enum)]
    pub col_type: EntityType,

    /// Output format: table, csv, json
    #[arg(long, short = 'o', default_value = "table")]
    pub output: String,

    /// Show full IDs instead of short IDs
    #[arg(long)]
    pub full_ids: bool,

    /// Show coverage statistics
    #[arg(long, short = 's')]
    pub stats: bool,
}

/// Entity info for DMM
#[derive(Debug, Clone)]
struct DmmEntity {
    id: String,
    short_id: String,
    title: String,
}

/// The DMM structure
struct Dmm {
    row_entities: Vec<DmmEntity>,
    col_entities: Vec<DmmEntity>,
    matrix: Vec<Vec<bool>>, // Simple binary matrix
    row_index: HashMap<String, usize>,
    col_index: HashMap<String, usize>,
}

impl Dmm {
    fn new(row_entities: Vec<DmmEntity>, col_entities: Vec<DmmEntity>) -> Self {
        let rows = row_entities.len();
        let cols = col_entities.len();

        let mut row_index = HashMap::new();
        for (i, e) in row_entities.iter().enumerate() {
            row_index.insert(e.id.clone(), i);
        }

        let mut col_index = HashMap::new();
        for (i, e) in col_entities.iter().enumerate() {
            col_index.insert(e.id.clone(), i);
        }

        let matrix = vec![vec![false; cols]; rows];

        Self {
            row_entities,
            col_entities,
            matrix,
            row_index,
            col_index,
        }
    }

    fn add_link(&mut self, row_id: &str, col_id: &str) {
        if let (Some(&i), Some(&j)) = (self.row_index.get(row_id), self.col_index.get(col_id)) {
            self.matrix[i][j] = true;
        }
    }

    fn coverage_stats(&self) -> (usize, usize, usize, usize, f64, f64) {
        let total_rows = self.row_entities.len();
        let total_cols = self.col_entities.len();

        // Count rows with at least one link
        let rows_with_links = self
            .matrix
            .iter()
            .filter(|row| row.iter().any(|&v| v))
            .count();

        // Count cols with at least one link
        let cols_with_links = (0..total_cols)
            .filter(|&j| self.matrix.iter().any(|row| row[j]))
            .count();

        let row_coverage = if total_rows > 0 {
            (rows_with_links as f64 / total_rows as f64) * 100.0
        } else {
            0.0
        };

        let col_coverage = if total_cols > 0 {
            (cols_with_links as f64 / total_cols as f64) * 100.0
        } else {
            0.0
        };

        (
            total_rows,
            rows_with_links,
            total_cols,
            cols_with_links,
            row_coverage,
            col_coverage,
        )
    }
}

pub fn run(args: DmmArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project)?;

    // Validate that row and col types are different
    if args.row_type == args.col_type {
        return Err(miette::miette!(
            "Row and column entity types must be different. Use 'tdt dsm' for same-entity analysis."
        ));
    }

    // Get entities for rows and columns
    let row_entities = get_entities(&cache, args.row_type);
    let col_entities = get_entities(&cache, args.col_type);

    if row_entities.is_empty() {
        println!(
            "No {} found in project",
            args.row_type.display_name().to_lowercase()
        );
        return Ok(());
    }

    if col_entities.is_empty() {
        println!(
            "No {} found in project",
            args.col_type.display_name().to_lowercase()
        );
        return Ok(());
    }

    // Build DMM
    let mut dmm = Dmm::new(row_entities, col_entities);

    // Find relationships based on entity type combination
    find_relationships(&cache, &mut dmm, args.row_type, args.col_type);

    // Output
    match args.output.as_str() {
        "csv" => output_csv(&dmm, args.full_ids),
        "json" => output_json(&dmm, args.row_type, args.col_type),
        _ => output_table(&dmm, args.full_ids, args.stats, args.row_type, args.col_type),
    }

    Ok(())
}

fn get_entities(cache: &EntityCache, entity_type: EntityType) -> Vec<DmmEntity> {
    match entity_type {
        EntityType::Cmp => cache
            .list_components(None, None, None, None, None, None)
            .into_iter()
            .map(|e| DmmEntity {
                short_id: cache
                    .get_short_id(&e.id)
                    .unwrap_or_else(|| format_short_id_str(&e.id)),
                id: e.id,
                title: e.title,
            })
            .collect(),
        EntityType::Req => cache
            .list_requirements(None, None, None, None, None, None, None)
            .into_iter()
            .map(|e| DmmEntity {
                short_id: cache
                    .get_short_id(&e.id)
                    .unwrap_or_else(|| format_short_id_str(&e.id)),
                id: e.id,
                title: e.title,
            })
            .collect(),
        EntityType::Proc => cache
            .list_processes(None, None, None, None, None, None)
            .into_iter()
            .map(|e| DmmEntity {
                short_id: cache
                    .get_short_id(&e.id)
                    .unwrap_or_else(|| format_short_id_str(&e.id)),
                id: e.id,
                title: e.title,
            })
            .collect(),
        EntityType::Test => cache
            .list_tests(None, None, None, None, None, None, None, None, None)
            .into_iter()
            .map(|e| DmmEntity {
                short_id: cache
                    .get_short_id(&e.id)
                    .unwrap_or_else(|| format_short_id_str(&e.id)),
                id: e.id,
                title: e.title,
            })
            .collect(),
        EntityType::Risk => cache
            .list_risks(None, None, None, None, None, None, None, None)
            .into_iter()
            .map(|e| DmmEntity {
                short_id: cache
                    .get_short_id(&e.id)
                    .unwrap_or_else(|| format_short_id_str(&e.id)),
                id: e.id,
                title: e.title,
            })
            .collect(),
        EntityType::Ctrl => cache
            .list_controls(None, None, None, None, None, None, None)
            .into_iter()
            .map(|e| DmmEntity {
                short_id: cache
                    .get_short_id(&e.id)
                    .unwrap_or_else(|| format_short_id_str(&e.id)),
                id: e.id,
                title: e.title,
            })
            .collect(),
    }
}

fn find_relationships(
    cache: &EntityCache,
    dmm: &mut Dmm,
    row_type: EntityType,
    col_type: EntityType,
) {
    let row_prefix = row_type.prefix();
    let col_prefix = col_type.prefix();

    // For each row entity, find links to column entities
    for row_entity in &dmm.row_entities.clone() {
        // Check outgoing links
        let links = cache.get_links_from(&row_entity.id);
        for link in links {
            if link.target_id.starts_with(col_prefix) {
                dmm.add_link(&row_entity.id, &link.target_id);
            }
        }

        // Check incoming links
        let reverse_links = cache.get_links_to(&row_entity.id);
        for link in reverse_links {
            if link.source_id.starts_with(col_prefix) {
                dmm.add_link(&row_entity.id, &link.source_id);
            }
        }
    }

    // Also check from column entities (in case links are asymmetric)
    for col_entity in &dmm.col_entities.clone() {
        let links = cache.get_links_from(&col_entity.id);
        for link in links {
            if link.target_id.starts_with(row_prefix) {
                dmm.add_link(&link.target_id, &col_entity.id);
            }
        }

        let reverse_links = cache.get_links_to(&col_entity.id);
        for link in reverse_links {
            if link.source_id.starts_with(row_prefix) {
                dmm.add_link(&link.source_id, &col_entity.id);
            }
        }
    }
}

fn output_table(
    dmm: &Dmm,
    full_ids: bool,
    show_stats: bool,
    row_type: EntityType,
    col_type: EntityType,
) {
    println!(
        "{}",
        style(format!(
            "Domain Mapping Matrix: {} × {}",
            row_type.display_name(),
            col_type.display_name()
        ))
        .bold()
        .cyan()
    );
    println!();

    let rows = dmm.row_entities.len();
    let cols = dmm.col_entities.len();

    if rows == 0 || cols == 0 {
        println!("No data to display");
        return;
    }

    // Calculate row label width
    let id_width = if full_ids {
        dmm.row_entities
            .iter()
            .map(|e| e.id.len())
            .max()
            .unwrap_or(8)
            .min(20)
    } else {
        dmm.row_entities
            .iter()
            .map(|e| e.short_id.len())
            .max()
            .unwrap_or(6)
            .max(6)
    };

    // Calculate column width
    let col_width = if full_ids {
        dmm.col_entities
            .iter()
            .map(|e| e.short_id.len())
            .max()
            .unwrap_or(5)
            .max(5)
    } else {
        dmm.col_entities
            .iter()
            .map(|e| e.short_id.len())
            .max()
            .unwrap_or(5)
            .max(5)
    };
    let cell_width = col_width + 2;

    // Print column headers
    print!("{:width$} ", "", width = id_width);
    for e in &dmm.col_entities {
        let label = if full_ids {
            format_short_id_str(&e.id)
        } else {
            e.short_id.clone()
        };
        print!("{:^width$}", label, width = cell_width);
    }
    println!();

    // Separator
    print!("{:-<width$} ", "", width = id_width);
    for _ in &dmm.col_entities {
        print!("{:-<width$}", "", width = cell_width);
    }
    println!();

    // Print rows
    for (i, e) in dmm.row_entities.iter().enumerate() {
        let label = if full_ids {
            format_short_id_str(&e.id)
        } else {
            e.short_id.clone()
        };
        print!("{:<width$} ", label, width = id_width);

        for j in 0..cols {
            if dmm.matrix[i][j] {
                print!("{:^width$}", style("X").green(), width = cell_width);
            } else {
                print!("{:^width$}", "·", width = cell_width);
            }
        }
        println!();
    }

    // Statistics
    if show_stats {
        let (total_rows, rows_linked, total_cols, cols_linked, row_cov, col_cov) =
            dmm.coverage_stats();

        println!();
        println!("{}", style("Coverage Statistics").bold());
        println!(
            "  {}: {}/{} ({:.1}% coverage)",
            row_type.display_name(),
            rows_linked,
            total_rows,
            row_cov
        );
        println!(
            "  {}: {}/{} ({:.1}% coverage)",
            col_type.display_name(),
            cols_linked,
            total_cols,
            col_cov
        );

        // Count total links
        let total_links: usize = dmm.matrix.iter().flatten().filter(|&&v| v).count();
        println!("  Total links: {}", total_links);
    } else {
        // Just show summary
        println!();
        let total_links: usize = dmm.matrix.iter().flatten().filter(|&&v| v).count();
        println!(
            "{}: {} {} × {} {} ({} links)",
            style("Summary").bold(),
            rows,
            row_type.display_name().to_lowercase(),
            cols,
            col_type.display_name().to_lowercase(),
            total_links
        );
    }
}

fn output_csv(dmm: &Dmm, full_ids: bool) {
    // Header
    print!("Entity");
    for e in &dmm.col_entities {
        let label = if full_ids { &e.id } else { &e.short_id };
        print!(",{}", label);
    }
    println!();

    // Rows
    for (i, e) in dmm.row_entities.iter().enumerate() {
        let label = if full_ids { &e.id } else { &e.short_id };
        print!("{}", label);

        for j in 0..dmm.col_entities.len() {
            if dmm.matrix[i][j] {
                print!(",X");
            } else {
                print!(",");
            }
        }
        println!();
    }
}

fn output_json(dmm: &Dmm, row_type: EntityType, col_type: EntityType) {
    let row_entities: Vec<serde_json::Value> = dmm
        .row_entities
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "short_id": e.short_id,
                "title": e.title,
            })
        })
        .collect();

    let col_entities: Vec<serde_json::Value> = dmm
        .col_entities
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "short_id": e.short_id,
                "title": e.title,
            })
        })
        .collect();

    // Build links array
    let mut links: Vec<serde_json::Value> = Vec::new();
    for (i, row) in dmm.row_entities.iter().enumerate() {
        for (j, col) in dmm.col_entities.iter().enumerate() {
            if dmm.matrix[i][j] {
                links.push(serde_json::json!({
                    "row": row.id,
                    "col": col.id,
                }));
            }
        }
    }

    let (total_rows, rows_linked, total_cols, cols_linked, row_cov, col_cov) = dmm.coverage_stats();

    let output = serde_json::json!({
        "row_type": format!("{:?}", row_type).to_lowercase(),
        "col_type": format!("{:?}", col_type).to_lowercase(),
        "row_entities": row_entities,
        "col_entities": col_entities,
        "links": links,
        "coverage": {
            "row_coverage_pct": row_cov,
            "col_coverage_pct": col_cov,
            "rows_with_links": rows_linked,
            "total_rows": total_rows,
            "cols_with_links": cols_linked,
            "total_cols": total_cols,
        }
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
}
