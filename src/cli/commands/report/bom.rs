//! BOM (Bill of Materials) report

use miette::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tabled::{builder::Builder, settings::Style};

use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::component::Component;
use crate::entities::quote::Quote;

use super::{
    load_all_assemblies, load_all_components, load_all_quotes, load_assembly, write_output,
};

#[derive(clap::Args, Debug)]
pub struct BomArgs {
    /// Assembly ID to generate BOM for
    pub assembly_id: String,

    /// Output to file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Include cost rollup
    #[arg(long)]
    pub with_cost: bool,

    /// Include mass rollup
    #[arg(long)]
    pub with_mass: bool,
}

pub fn run(args: BomArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let resolved_id = short_ids
        .resolve(&args.assembly_id)
        .unwrap_or_else(|| args.assembly_id.clone());

    // Load assembly
    let assembly = load_assembly(&project, &resolved_id)?;

    // Load all components for lookup
    let components = load_all_components(&project);
    let component_map: HashMap<String, &Component> =
        components.iter().map(|c| (c.id.to_string(), c)).collect();

    // Load all assemblies for subassembly lookup
    let assemblies = load_all_assemblies(&project);
    let assembly_map: HashMap<String, &crate::entities::assembly::Assembly> =
        assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

    // Load quotes for price lookup (used when --with-cost)
    let quotes = load_all_quotes(&project);
    let quote_map: HashMap<String, &Quote> = quotes.iter().map(|q| (q.id.to_string(), q)).collect();

    // Generate indented BOM
    let mut output = String::new();
    output.push_str(&format!("# Bill of Materials: {}\n\n", assembly.title));
    output.push_str(&format!("Assembly ID: {}\n", assembly.id));
    output.push_str(&format!("Part Number: {}\n\n", assembly.part_number));

    let mut total_cost = 0.0;
    let mut total_mass = 0.0;

    output.push_str("```\n");

    // Recursively print BOM
    fn print_bom_item(
        output: &mut String,
        component_map: &HashMap<String, &Component>,
        assembly_map: &HashMap<String, &crate::entities::assembly::Assembly>,
        quote_map: &HashMap<String, &Quote>,
        short_ids: &ShortIdIndex,
        bom: &[crate::entities::assembly::BomItem],
        indent: usize,
        total_cost: &mut f64,
        total_mass: &mut f64,
        with_cost: bool,
        with_mass: bool,
        visited: &mut std::collections::HashSet<String>,
    ) {
        let prefix = "│  ".repeat(indent);
        for (i, item) in bom.iter().enumerate() {
            let is_last = i == bom.len() - 1;
            let branch = if is_last { "└─ " } else { "├─ " };

            let item_id = item.component_id.to_string();
            let item_short = short_ids
                .get_short_id(&item_id)
                .unwrap_or_else(|| item_id.clone());

            // Check if it's a component or subassembly
            if let Some(cmp) = component_map.get(&item_id) {
                let cost_str = if with_cost {
                    // Priority 1: Use selected quote if set
                    let unit_price = if let Some(ref quote_id) = cmp.selected_quote {
                        if let Some(quote) = quote_map.get(quote_id) {
                            quote.price_for_qty(item.quantity).unwrap_or(0.0)
                        } else {
                            cmp.unit_cost.unwrap_or(0.0)
                        }
                    } else {
                        // Priority 2: Fall back to unit_cost
                        cmp.unit_cost.unwrap_or(0.0)
                    };

                    if unit_price > 0.0 {
                        let line_cost = unit_price * item.quantity as f64;
                        *total_cost += line_cost;
                        format!(" ${:.2}", line_cost)
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };

                let mass_str = if with_mass {
                    cmp.mass_kg.map_or("".to_string(), |m| {
                        let line_mass = m * item.quantity as f64;
                        *total_mass += line_mass;
                        format!(" {:.3}kg", line_mass)
                    })
                } else {
                    "".to_string()
                };

                output.push_str(&format!(
                    "{}{}{}: {} (qty: {}){}{}\n",
                    prefix, branch, item_short, cmp.title, item.quantity, cost_str, mass_str
                ));
            } else if let Some(asm) = assembly_map.get(&item_id) {
                // Subassembly - check for cycles
                if visited.contains(&item_id) {
                    output.push_str(&format!(
                        "{}{}{}: {} (qty: {}) [CYCLE DETECTED]\n",
                        prefix, branch, item_short, asm.title, item.quantity
                    ));
                } else {
                    output.push_str(&format!(
                        "{}{}{}: {} (qty: {})\n",
                        prefix, branch, item_short, asm.title, item.quantity
                    ));

                    visited.insert(item_id.clone());
                    print_bom_item(
                        output,
                        component_map,
                        assembly_map,
                        quote_map,
                        short_ids,
                        &asm.bom,
                        indent + 1,
                        total_cost,
                        total_mass,
                        with_cost,
                        with_mass,
                        visited,
                    );
                    visited.remove(&item_id);
                }
            } else {
                output.push_str(&format!(
                    "{}{}{}: (not found) (qty: {})\n",
                    prefix, branch, item_short, item.quantity
                ));
            }
        }
    }

    let mut visited = std::collections::HashSet::new();
    visited.insert(assembly.id.to_string());
    print_bom_item(
        &mut output,
        &component_map,
        &assembly_map,
        &quote_map,
        &short_ids,
        &assembly.bom,
        0,
        &mut total_cost,
        &mut total_mass,
        args.with_cost,
        args.with_mass,
        &mut visited,
    );

    output.push_str("```\n");

    // Totals
    if args.with_cost {
        output.push_str(&format!("\n**Total Cost:** ${:.2}\n", total_cost));
    }
    if args.with_mass {
        output.push_str(&format!("**Total Mass:** {:.3} kg\n", total_mass));
    }

    // Collect all unique components in the BOM for supply risk analysis
    let mut bom_components: HashSet<String> = HashSet::new();
    fn collect_bom_components(
        bom: &[crate::entities::assembly::BomItem],
        assembly_map: &HashMap<String, &crate::entities::assembly::Assembly>,
        component_set: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        for item in bom {
            let item_id = item.component_id.to_string();
            if item_id.starts_with("CMP-") {
                component_set.insert(item_id);
            } else if item_id.starts_with("ASM-") {
                if !visited.contains(&item_id) {
                    visited.insert(item_id.clone());
                    if let Some(asm) = assembly_map.get(&item_id) {
                        collect_bom_components(&asm.bom, assembly_map, component_set, visited);
                    }
                }
            }
        }
    }

    let mut visited_asm: HashSet<String> = HashSet::new();
    visited_asm.insert(assembly.id.to_string());
    collect_bom_components(&assembly.bom, &assembly_map, &mut bom_components, &mut visited_asm);

    // Analyze supply risks
    struct SupplyRisk {
        id: String,
        title: String,
        risk_type: String,
        details: String,
    }
    let mut supply_risks: Vec<SupplyRisk> = Vec::new();

    let long_lead_threshold = 30; // days

    for cmp_id in &bom_components {
        if let Some(cmp) = component_map.get(cmp_id) {
            let cmp_short = short_ids
                .get_short_id(cmp_id)
                .unwrap_or_else(|| cmp_id.clone());

            // Check for single source (only 1 supplier)
            let supplier_count = cmp.suppliers.iter().filter(|s| !s.name.is_empty()).count();
            if supplier_count == 1 {
                supply_risks.push(SupplyRisk {
                    id: cmp_short.clone(),
                    title: cmp.title.clone(),
                    risk_type: "Single Source".to_string(),
                    details: cmp.suppliers.first().map(|s| s.name.clone()).unwrap_or_default(),
                });
            } else if supplier_count == 0 {
                supply_risks.push(SupplyRisk {
                    id: cmp_short.clone(),
                    title: cmp.title.clone(),
                    risk_type: "No Supplier".to_string(),
                    details: "No suppliers defined".to_string(),
                });
            }

            // Check for long lead time
            let max_lead = cmp.suppliers.iter().filter_map(|s| s.lead_time_days).max();
            if let Some(lead) = max_lead {
                if lead > long_lead_threshold {
                    // Only add if not already flagged for supplier risk
                    if supplier_count > 1 {
                        supply_risks.push(SupplyRisk {
                            id: cmp_short.clone(),
                            title: cmp.title.clone(),
                            risk_type: "Long Lead Time".to_string(),
                            details: format!("{} days", lead),
                        });
                    }
                }
            }

            // Check for no quotes
            let has_quote = cmp.selected_quote.is_some()
                || quotes.iter().any(|q| q.component.as_deref() == Some(cmp_id));
            if !has_quote && cmp.unit_cost.is_none() {
                supply_risks.push(SupplyRisk {
                    id: cmp_short.clone(),
                    title: cmp.title.clone(),
                    risk_type: "No Pricing".to_string(),
                    details: "No quote or unit cost".to_string(),
                });
            }
        }
    }

    // Output supply risk analysis
    if !supply_risks.is_empty() {
        output.push_str("\n## Supply Chain Risk Analysis\n\n");

        let mut risk_table = Builder::default();
        risk_table.push_record(["Component", "Title", "Risk Type", "Details"]);

        // Sort by risk type then by ID
        supply_risks.sort_by(|a, b| {
            let type_order = |t: &str| match t {
                "No Supplier" => 0,
                "Single Source" => 1,
                "Long Lead Time" => 2,
                "No Pricing" => 3,
                _ => 4,
            };
            type_order(&a.risk_type)
                .cmp(&type_order(&b.risk_type))
                .then(a.id.cmp(&b.id))
        });

        for risk in &supply_risks {
            risk_table.push_record([
                risk.id.clone(),
                if risk.title.len() > 25 {
                    format!("{}...", &risk.title[..22])
                } else {
                    risk.title.clone()
                },
                risk.risk_type.clone(),
                risk.details.clone(),
            ]);
        }
        output.push_str(&risk_table.build().with(Style::markdown()).to_string());

        // Summary
        let no_supplier = supply_risks.iter().filter(|r| r.risk_type == "No Supplier").count();
        let single_source = supply_risks.iter().filter(|r| r.risk_type == "Single Source").count();
        let long_lead = supply_risks.iter().filter(|r| r.risk_type == "Long Lead Time").count();
        let no_pricing = supply_risks.iter().filter(|r| r.risk_type == "No Pricing").count();

        output.push_str(&format!(
            "\n*{} components in BOM: {} no supplier, {} single-source, {} long lead (>{}d), {} no pricing*\n",
            bom_components.len(), no_supplier, single_source, long_lead, long_lead_threshold, no_pricing
        ));
    }

    write_output(&output, args.output)?;
    Ok(())
}
