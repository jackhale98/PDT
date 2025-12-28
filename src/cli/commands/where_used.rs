//! `tdt where-used` command - Find where an entity is referenced

use console::style;
use miette::Result;
use std::collections::HashMap;

use crate::cli::helpers::format_short_id_str;
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct WhereUsedArgs {
    /// Entity ID or short ID to search for (e.g., CMP@1, FEAT@3, REQ@5)
    pub id: String,

    /// Show only direct references (not transitive)
    #[arg(long)]
    pub direct_only: bool,
}

pub fn run(args: WhereUsedArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve the ID
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Determine entity type from prefix
    let entity_type = if resolved_id.starts_with("CMP-") {
        "component"
    } else if resolved_id.starts_with("ASM-") {
        "assembly"
    } else if resolved_id.starts_with("FEAT-") {
        "feature"
    } else if resolved_id.starts_with("REQ-") {
        "requirement"
    } else if resolved_id.starts_with("TEST-") {
        "test"
    } else if resolved_id.starts_with("RISK-") {
        "risk"
    } else if resolved_id.starts_with("PROC-") {
        "process"
    } else if resolved_id.starts_with("SUP-") {
        "supplier"
    } else if resolved_id.starts_with("QUOTE-") {
        "quote"
    } else {
        "unknown"
    };

    println!(
        "{} {}",
        style("Searching for references to:").bold(),
        style(&resolved_id).cyan()
    );
    println!("{}: {}\n", style("Entity type").dim(), entity_type);

    let mut found_refs: Vec<(String, String, String)> = Vec::new(); // (ref_id, ref_type, relationship)

    // Search for component/assembly usage in BOMs
    if resolved_id.starts_with("CMP-") || resolved_id.starts_with("ASM-") {
        find_bom_references(&project, &resolved_id, &short_ids, &mut found_refs)?;
    }

    // Search for feature usage in mates and stackups
    if resolved_id.starts_with("FEAT-") {
        find_mate_references(&project, &resolved_id, &short_ids, &mut found_refs)?;
        find_stackup_references(&project, &resolved_id, &short_ids, &mut found_refs)?;
    }

    // Search for requirement verification (what tests verify this requirement)
    if resolved_id.starts_with("REQ-") {
        find_test_references(&project, &resolved_id, &short_ids, &mut found_refs)?;
    }

    // Search for supplier usage in quotes
    if resolved_id.starts_with("SUP-") {
        find_quote_references(&project, &resolved_id, &short_ids, &mut found_refs)?;
    }

    // Search for component usage in quotes
    if resolved_id.starts_with("CMP-") {
        find_component_quote_references(&project, &resolved_id, &short_ids, &mut found_refs)?;
    }

    // Search for links in any entity that references this one
    find_generic_link_references(&project, &resolved_id, &short_ids, &mut found_refs)?;

    // Output results
    if found_refs.is_empty() {
        println!("{}", style("No references found.").yellow());
    } else {
        let format = match global.output {
            OutputFormat::Auto => OutputFormat::Tsv,
            f => f,
        };

        match format {
            OutputFormat::Json => {
                let refs: Vec<HashMap<&str, &str>> = found_refs
                    .iter()
                    .map(|(id, typ, rel)| {
                        let mut map = HashMap::new();
                        map.insert("id", id.as_str());
                        map.insert("type", typ.as_str());
                        map.insert("relationship", rel.as_str());
                        map
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&refs).unwrap_or_default()
                );
            }
            OutputFormat::Csv => {
                println!("ref_id,ref_type,relationship");
                for (ref_id, ref_type, rel) in &found_refs {
                    println!("{},{},{}", ref_id, ref_type, rel);
                }
            }
            _ => {
                println!(
                    "{:<12} {:<20} {}",
                    style("REF ID").bold(),
                    style("TYPE").bold(),
                    style("RELATIONSHIP").bold()
                );
                println!("{}", "-".repeat(60));
                for (ref_id, ref_type, rel) in &found_refs {
                    let ref_short = short_ids
                        .get_short_id(ref_id)
                        .unwrap_or_else(|| format_short_id_str(ref_id));
                    println!("{:<12} {:<20} {}", style(&ref_short).cyan(), ref_type, rel);
                }
                println!();
                println!("{} reference(s) found.", style(found_refs.len()).cyan());
            }
        }
    }

    Ok(())
}

fn find_bom_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
    let asm_dir = project.root().join("bom/assemblies");
    if !asm_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&asm_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(asm) =
            crate::yaml::parse_yaml_file::<crate::entities::assembly::Assembly>(entry.path())
        {
            for item in &asm.bom {
                if item.component_id == target_id {
                    found_refs.push((
                        asm.id.to_string(),
                        "assembly".to_string(),
                        format!("bom (qty: {})", item.quantity),
                    ));
                    break; // Only count once per assembly
                }
            }
        }
    }

    Ok(())
}

fn find_mate_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
    let mate_dir = project.root().join("tolerances/mates");
    if !mate_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&mate_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(mate) = crate::yaml::parse_yaml_file::<crate::entities::mate::Mate>(entry.path())
        {
            let mut found = false;
            let mut which_feature = "";

            if mate.feature_a.id.to_string() == target_id {
                found = true;
                which_feature = "feature_a";
            }
            if mate.feature_b.id.to_string() == target_id {
                found = true;
                which_feature = "feature_b";
            }

            if found {
                found_refs.push((
                    mate.id.to_string(),
                    "mate".to_string(),
                    which_feature.to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn find_stackup_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
    let stackup_dir = project.root().join("tolerances/stackups");
    if !stackup_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&stackup_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(stackup) =
            crate::yaml::parse_yaml_file::<crate::entities::stackup::Stackup>(entry.path())
        {
            for (i, contrib) in stackup.contributors.iter().enumerate() {
                if contrib
                    .feature
                    .as_ref()
                    .is_some_and(|f| f.id.to_string() == target_id)
                {
                    found_refs.push((
                        stackup.id.to_string(),
                        "stackup".to_string(),
                        format!("contributor[{}]", i),
                    ));
                    break;
                }
            }
        }
    }

    Ok(())
}

fn find_test_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
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
                let verifies_it = test
                    .links
                    .verifies
                    .iter()
                    .any(|id| id.to_string() == target_id);
                let validates_it = test
                    .links
                    .validates
                    .iter()
                    .any(|id| id.to_string() == target_id);

                if verifies_it {
                    found_refs.push((
                        test.id.to_string(),
                        "test".to_string(),
                        "verifies".to_string(),
                    ));
                }
                if validates_it {
                    found_refs.push((
                        test.id.to_string(),
                        "test".to_string(),
                        "validates".to_string(),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn find_quote_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
    let quote_dir = project.root().join("procurement/quotes");
    if !quote_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&quote_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(quote) =
            crate::yaml::parse_yaml_file::<crate::entities::quote::Quote>(entry.path())
        {
            if quote.supplier == target_id {
                found_refs.push((
                    quote.id.to_string(),
                    "quote".to_string(),
                    "supplier".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn find_component_quote_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
    let quote_dir = project.root().join("procurement/quotes");
    if !quote_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&quote_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(quote) =
            crate::yaml::parse_yaml_file::<crate::entities::quote::Quote>(entry.path())
        {
            if quote.component.as_ref().is_some_and(|c| c == target_id) {
                found_refs.push((
                    quote.id.to_string(),
                    "quote".to_string(),
                    "component".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn find_generic_link_references(
    project: &Project,
    target_id: &str,
    _short_ids: &ShortIdIndex,
    found_refs: &mut Vec<(String, String, String)>,
) -> Result<()> {
    // This searches through common entity directories for any links to the target
    let search_dirs = vec![
        ("requirements/inputs", "requirement"),
        ("requirements/outputs", "requirement"),
        ("risks/design", "risk"),
        ("risks/process", "risk"),
        ("manufacturing/ncrs", "ncr"),
        ("manufacturing/capas", "capa"),
    ];

    for (dir_name, entity_type) in search_dirs {
        let dir = project.root().join(dir_name);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            // Read file and check for the target ID in links
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains(target_id) {
                    // Parse to get the entity ID
                    if let Ok(yaml) = serde_yml::from_str::<serde_yml::Value>(&content) {
                        if let Some(id) = yaml.get("id").and_then(|v| v.as_str()) {
                            // Avoid duplicates and self-references
                            if id != target_id
                                && !found_refs.iter().any(|(ref_id, _, _)| ref_id == id)
                            {
                                found_refs.push((
                                    id.to_string(),
                                    entity_type.to_string(),
                                    "links".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
