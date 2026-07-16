//! Link type inference and reciprocal link handling
//!
//! This module provides functions for inferring link types between entities
//! and determining reciprocal links for bidirectional relationships.

use crate::core::identity::EntityPrefix;

/// Infer the appropriate link type when linking from source to target entity.
///
/// Returns the field name on the source entity that should contain the link,
/// or None if no automatic inference is possible for this entity combination.
///
/// # Examples
/// ```
/// use tdt_core::core::links::infer_link_type;
/// use tdt_core::core::identity::EntityPrefix;
///
/// // REQ linking to TEST -> verified_by
/// assert_eq!(
///     infer_link_type(EntityPrefix::Req, EntityPrefix::Test),
///     Some("verified_by".to_string())
/// );
///
/// // RISK linking to CMP -> component
/// assert_eq!(
///     infer_link_type(EntityPrefix::Risk, EntityPrefix::Cmp),
///     Some("component".to_string())
/// );
/// ```
pub fn infer_link_type(source_prefix: EntityPrefix, target_prefix: EntityPrefix) -> Option<String> {
    match (source_prefix, target_prefix) {
        // Requirements linking to verification entities
        (EntityPrefix::Req, EntityPrefix::Test) => Some("verified_by".to_string()),
        (EntityPrefix::Req, EntityPrefix::Ctrl) => Some("verified_by".to_string()),

        // Requirements linking to other requirements (decomposition)
        (EntityPrefix::Req, EntityPrefix::Req) => Some("derives_from".to_string()),

        // Requirements allocated to features
        (EntityPrefix::Req, EntityPrefix::Feat) => Some("allocated_to".to_string()),

        // Requirements linking to risks
        (EntityPrefix::Req, EntityPrefix::Risk) => Some("risks".to_string()),

        // Requirements satisfied by components/assemblies
        (EntityPrefix::Req, EntityPrefix::Cmp) => Some("satisfied_by".to_string()),
        (EntityPrefix::Req, EntityPrefix::Asm) => Some("satisfied_by".to_string()),

        // Verification entities linking to requirements
        (EntityPrefix::Test, EntityPrefix::Req) => Some("verifies".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Req) => Some("verifies".to_string()),

        // Tests linking to components/assemblies (item under test)
        (EntityPrefix::Test, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Test, EntityPrefix::Asm) => Some("assembly".to_string()),

        // Results linking to components/assemblies (item tested)
        (EntityPrefix::Rslt, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Rslt, EntityPrefix::Asm) => Some("assembly".to_string()),

        // Risks linking to specific entities (single-value links)
        (EntityPrefix::Risk, EntityPrefix::Req) => Some("requirement".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Asm) => Some("assembly".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Proc) => Some("process".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Feat) => Some("affects".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Test) => Some("verified_by".to_string()),
        // Controls mitigate risks, so link via mitigated_by
        (EntityPrefix::Risk, EntityPrefix::Ctrl) => Some("mitigated_by".to_string()),

        // Entities referencing risks
        (EntityPrefix::Feat, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Cmp, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Proc, EntityPrefix::Risk) => Some("risks".to_string()),
        // Controls mitigate risks (symmetric with Risk->Ctrl = mitigated_by)
        (EntityPrefix::Ctrl, EntityPrefix::Risk) => Some("mitigates".to_string()),
        (EntityPrefix::Work, EntityPrefix::Risk) => Some("risks".to_string()),

        // Components/Assemblies linking to requirements
        (EntityPrefix::Cmp, EntityPrefix::Req) => Some("requirements".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Req) => Some("requirements".to_string()),

        // Components/Assemblies linking to processes
        (EntityPrefix::Cmp, EntityPrefix::Proc) => Some("processes".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Proc) => Some("processes".to_string()),

        // Components/Assemblies linking to tests
        (EntityPrefix::Cmp, EntityPrefix::Test) => Some("tests".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Test) => Some("tests".to_string()),

        // Components/Assemblies linking to features
        (EntityPrefix::Cmp, EntityPrefix::Feat) => Some("features".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Feat) => Some("features".to_string()),

        // Features linking to components (back-link)
        (EntityPrefix::Feat, EntityPrefix::Cmp) => Some("component".to_string()),

        // Processes linking to requirements
        (EntityPrefix::Proc, EntityPrefix::Req) => Some("requirements".to_string()),

        // Processes linking to components/assemblies (produces)
        (EntityPrefix::Proc, EntityPrefix::Cmp) => Some("produces".to_string()),
        (EntityPrefix::Proc, EntityPrefix::Asm) => Some("produces".to_string()),

        // Processes linking to controls
        (EntityPrefix::Proc, EntityPrefix::Ctrl) => Some("controls".to_string()),

        // Controls linking to components and features
        (EntityPrefix::Ctrl, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Feat) => Some("feature".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Proc) => Some("process".to_string()),

        // Work instructions linking to components/assemblies/processes/controls
        (EntityPrefix::Work, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Work, EntityPrefix::Asm) => Some("assembly".to_string()),
        (EntityPrefix::Work, EntityPrefix::Proc) => Some("process".to_string()),
        (EntityPrefix::Work, EntityPrefix::Ctrl) => Some("controls".to_string()),

        // Features to requirements (allocation back-link)
        (EntityPrefix::Feat, EntityPrefix::Req) => Some("allocated_from".to_string()),

        // Results and NCRs
        (EntityPrefix::Rslt, EntityPrefix::Ncr) => Some("ncrs".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Rslt) => Some("from_result".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Capa) => Some("capa".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Sup) => Some("supplier".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Proc) => Some("process".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Lot) => Some("lot".to_string()),

        // Lots linking to NCRs
        (EntityPrefix::Lot, EntityPrefix::Ncr) => Some("ncrs".to_string()),

        // CAPAs
        (EntityPrefix::Capa, EntityPrefix::Ncr) => Some("ncrs".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Proc) => Some("processes_modified".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Ctrl) => Some("controls_added".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Sup) => Some("supplier".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Risk) => Some("risks".to_string()),

        // Hazard links
        (EntityPrefix::Haz, EntityPrefix::Cmp) => Some("originates_from".to_string()),
        (EntityPrefix::Haz, EntityPrefix::Asm) => Some("originates_from".to_string()),
        (EntityPrefix::Haz, EntityPrefix::Risk) => Some("causes".to_string()),
        (EntityPrefix::Haz, EntityPrefix::Ctrl) => Some("controlled_by".to_string()),
        (EntityPrefix::Haz, EntityPrefix::Test) => Some("verified_by".to_string()),
        (EntityPrefix::Haz, EntityPrefix::Req) => Some("related_to".to_string()),

        // Entities referencing hazards
        (EntityPrefix::Risk, EntityPrefix::Haz) => Some("caused_by".to_string()),
        (EntityPrefix::Cmp, EntityPrefix::Haz) => Some("hazards".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Haz) => Some("hazards".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Haz) => Some("controls_hazard".to_string()),

        // Process/Control back-links to CAPA
        (EntityPrefix::Proc, EntityPrefix::Capa) => Some("modified_by_capa".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Capa) => Some("added_by_capa".to_string()),

        // Component supersession (default to replaces)
        (EntityPrefix::Cmp, EntityPrefix::Cmp) => Some("replaces".to_string()),

        // Default to related_to for same-type entities (except CMP)
        (a, b) if a == b => Some("related_to".to_string()),

        // No inference available for other combinations
        _ => None,
    }
}

/// Get the reciprocal link type for a given forward link type, target entity prefix,
/// and source entity prefix.
///
/// When entity A links to entity B, this function returns what field on B
/// should link back to A (if any). The `source_prefix` parameter is needed
/// to disambiguate link type names that are reused across different source
/// entity types (e.g., "component", "risks", "process" all have different
/// reciprocals depending on what entity the link originates from).
///
/// Returns None if no reciprocal link should be created.
pub fn get_reciprocal_link_type(
    link_type: &str,
    target_prefix: EntityPrefix,
    source_prefix: EntityPrefix,
) -> Option<String> {
    match (link_type, target_prefix, source_prefix) {
        // === Verification links ===
        // REQ/RISK/HAZ.verified_by -> TEST/CTRL means TEST/CTRL.verifies -> source
        ("verified_by", EntityPrefix::Test, _) => Some("verifies".to_string()),
        ("verified_by", EntityPrefix::Ctrl, _) => Some("verifies".to_string()),
        // TEST/CTRL.verifies -> REQ means REQ.verified_by -> source
        ("verifies", EntityPrefix::Req, _) => Some("verified_by".to_string()),

        // === Satisfaction links ===
        // REQ.satisfied_by -> REQ (symmetric)
        ("satisfied_by", EntityPrefix::Req, _) => Some("satisfied_by".to_string()),
        // REQ.satisfied_by -> CMP/ASM means CMP/ASM.requirements -> REQ
        ("satisfied_by", EntityPrefix::Cmp, _) => Some("requirements".to_string()),
        ("satisfied_by", EntityPrefix::Asm, _) => Some("requirements".to_string()),
        // CMP/ASM/PROC.requirements -> REQ means REQ.satisfied_by
        ("requirements", EntityPrefix::Req, _) => Some("satisfied_by".to_string()),

        // === Requirement decomposition ===
        ("derives_from", EntityPrefix::Req, _) => Some("derived_by".to_string()),
        ("derived_by", EntityPrefix::Req, _) => Some("derives_from".to_string()),

        // === Requirement allocation ===
        ("allocated_to", EntityPrefix::Feat, _) => Some("allocated_from".to_string()),
        ("allocated_from", EntityPrefix::Req, _) => Some("allocated_to".to_string()),

        // === "risks" link: many entities -> RISK (reciprocal depends on source) ===
        ("risks", EntityPrefix::Risk, EntityPrefix::Req) => Some("requirement".to_string()),
        ("risks", EntityPrefix::Risk, EntityPrefix::Feat) => Some("affects".to_string()),
        ("risks", EntityPrefix::Risk, EntityPrefix::Cmp) => Some("component".to_string()),
        ("risks", EntityPrefix::Risk, EntityPrefix::Asm) => Some("assembly".to_string()),
        ("risks", EntityPrefix::Risk, EntityPrefix::Proc) => Some("process".to_string()),
        ("risks", EntityPrefix::Risk, EntityPrefix::Capa) => Some("related_to".to_string()),
        // Ctrl/Work -> Risk: no clean reciprocal field on Risk
        ("risks", EntityPrefix::Risk, _) => None,

        // === RISK single-value links back to entities ===
        ("requirement", EntityPrefix::Req, _) => Some("risks".to_string()),
        ("affects", EntityPrefix::Feat, _) => Some("risks".to_string()),
        ("affects", EntityPrefix::Cmp, _) => Some("risks".to_string()),
        ("affects", EntityPrefix::Asm, _) => Some("risks".to_string()),
        ("affects", EntityPrefix::Proc, _) => Some("risks".to_string()),

        // === "component" link: many entities -> CMP (reciprocal depends on source) ===
        ("component", EntityPrefix::Cmp, EntityPrefix::Test) => Some("tests".to_string()),
        ("component", EntityPrefix::Cmp, EntityPrefix::Feat) => Some("features".to_string()),
        ("component", EntityPrefix::Cmp, EntityPrefix::Risk) => Some("risks".to_string()),
        ("component", EntityPrefix::Cmp, EntityPrefix::Quot) => Some("quotes".to_string()),
        // Rslt, Ctrl, Work, Ncr, Capa -> CMP: no reciprocal on CMP
        ("component", EntityPrefix::Cmp, _) => None,

        // === "assembly" link: many entities -> ASM (reciprocal depends on source) ===
        ("assembly", EntityPrefix::Asm, EntityPrefix::Test) => Some("tests".to_string()),
        ("assembly", EntityPrefix::Asm, EntityPrefix::Risk) => Some("risks".to_string()),
        ("assembly", EntityPrefix::Asm, EntityPrefix::Quot) => Some("quotes".to_string()),
        // Rslt, Work -> ASM: no reciprocal on ASM
        ("assembly", EntityPrefix::Asm, _) => None,

        // === "process" link: many entities -> PROC (reciprocal depends on source) ===
        ("process", EntityPrefix::Proc, EntityPrefix::Risk) => Some("risks".to_string()),
        ("process", EntityPrefix::Proc, EntityPrefix::Ctrl) => Some("controls".to_string()),
        ("process", EntityPrefix::Proc, EntityPrefix::Work) => {
            Some("work_instructions".to_string())
        }
        // Ncr -> PROC: no reciprocal on PROC
        ("process", EntityPrefix::Proc, _) => None,

        // === Tests/Features/Processes arrays ===
        ("tests", EntityPrefix::Test, EntityPrefix::Cmp) => Some("component".to_string()),
        ("tests", EntityPrefix::Test, EntityPrefix::Asm) => Some("assembly".to_string()),
        ("tests", EntityPrefix::Test, _) => None,
        ("features", EntityPrefix::Feat, EntityPrefix::Cmp) => Some("component".to_string()),
        ("features", EntityPrefix::Feat, _) => None,
        ("processes", EntityPrefix::Proc, _) => Some("produces".to_string()),
        ("produces", EntityPrefix::Cmp, _) => Some("processes".to_string()),
        ("produces", EntityPrefix::Asm, _) => Some("processes".to_string()),

        // === Process child entities ===
        ("controls", EntityPrefix::Ctrl, EntityPrefix::Proc) => Some("process".to_string()),
        ("controls", EntityPrefix::Ctrl, _) => None,
        ("work_instructions", EntityPrefix::Work, _) => Some("process".to_string()),

        // === Mitigation ===
        ("mitigated_by", EntityPrefix::Ctrl, _) => Some("mitigates".to_string()),
        ("mitigates", EntityPrefix::Risk, _) => Some("mitigated_by".to_string()),

        // === NCR / Result / CAPA chain ===
        ("from_result", EntityPrefix::Rslt, _) => Some("ncrs".to_string()),
        ("ncrs", EntityPrefix::Ncr, EntityPrefix::Rslt) => Some("from_result".to_string()),
        ("ncrs", EntityPrefix::Ncr, EntityPrefix::Capa) => Some("capa".to_string()),
        ("ncrs", EntityPrefix::Ncr, EntityPrefix::Lot) => Some("lot".to_string()),
        ("ncrs", EntityPrefix::Ncr, _) => None,
        ("capa", EntityPrefix::Capa, _) => Some("ncrs".to_string()),

        // === NCR <-> LOT ===
        ("lot", EntityPrefix::Lot, EntityPrefix::Ncr) => Some("ncrs".to_string()),

        // === CAPA -> Process/Control modifications ===
        ("processes_modified", EntityPrefix::Proc, _) => Some("modified_by_capa".to_string()),
        ("modified_by_capa", EntityPrefix::Capa, _) => Some("processes_modified".to_string()),
        ("controls_added", EntityPrefix::Ctrl, _) => Some("added_by_capa".to_string()),
        ("added_by_capa", EntityPrefix::Capa, _) => Some("controls_added".to_string()),

        // === Component supersession ===
        ("replaces", EntityPrefix::Cmp, _) => Some("replaced_by".to_string()),
        ("replaced_by", EntityPrefix::Cmp, _) => Some("replaces".to_string()),
        ("interchangeable_with", EntityPrefix::Cmp, _) => Some("interchangeable_with".to_string()),

        // === Symmetric ===
        ("related_to", _, _) => Some("related_to".to_string()),

        // === Supplier → quotes reciprocal (only from QUOT entities) ===
        ("supplier", EntityPrefix::Sup, EntityPrefix::Quot) => Some("quotes".to_string()),
        // NCR/CAPA.supplier → SUP: no reciprocal on SUP
        ("supplier", EntityPrefix::Sup, _) => None,
        ("quotes", EntityPrefix::Quot, _) => Some("supplier".to_string()),

        // === Hazard reciprocals ===
        ("originates_from", EntityPrefix::Cmp, _) => Some("hazards".to_string()),
        ("originates_from", EntityPrefix::Asm, _) => Some("hazards".to_string()),
        ("hazards", EntityPrefix::Haz, _) => Some("originates_from".to_string()),
        ("causes", EntityPrefix::Risk, _) => Some("caused_by".to_string()),
        ("caused_by", EntityPrefix::Haz, _) => Some("causes".to_string()),
        ("controlled_by", EntityPrefix::Ctrl, _) => Some("controls_hazard".to_string()),
        ("controls_hazard", EntityPrefix::Haz, _) => Some("controlled_by".to_string()),

        // === Fallback: try reverse inference ===
        // If no explicit reciprocal is defined, check if the reverse direction
        // has an inferred link type. This ensures new entity combinations added
        // to infer_link_type automatically get reciprocals.
        (_, _, _) => infer_link_type(target_prefix, source_prefix),
    }
}

/// Classify whether a link type represents a downstream relationship.
///
/// A downstream link means the target entity is "downstream" in the engineering
/// hierarchy (a dependent, product, or verification). An upstream link means the
/// target is a parent, dependency, or source.
///
/// Examples:
/// - `verified_by` is downstream: the test that verifies is downstream of the requirement
/// - `verifies` is upstream: the requirement being verified is upstream of the test
/// - `satisfied_by` is downstream: the component satisfying is downstream of the requirement
/// - `component` is upstream: the component under test is upstream of the test
pub fn is_downstream_link(link_type: &str) -> bool {
    match link_type {
        // Upstream-pointing links (target is a parent/dependency/source)
        "verifies" | "satisfies" | "mitigates" | "requirement" | "requirements" | "component"
        | "assembly" | "process" | "feature" | "derives_from" | "allocated_from"
        | "from_result" | "capa" | "supplier" | "originates_from" | "caused_by"
        | "controls_hazard" | "added_by_capa" | "modified_by_capa" | "replaces" | "affects"
        | "contained_in" | "parent" => false,

        // Everything else is downstream-pointing (target is a child/dependent/product)
        // Includes: verified_by, satisfied_by, mitigated_by, allocated_to,
        // risks, tests, features, controls, work_instructions, ncrs,
        // processes, produces, processes_modified, controls_added,
        // hazards, causes, controlled_by, derived_by, replaced_by,
        // related_to, interchangeable_with, etc.
        _ => true,
    }
}

/// A direct field reference on an entity that should produce a reciprocal link
/// on the target entity.
#[derive(Debug, Clone)]
pub struct FieldReferenceRule {
    /// The YAML field name on the source entity (e.g., "supplier", "component")
    pub field_name: &'static str,
    /// The link type to add on the target entity (e.g., "quotes", "features")
    pub reciprocal_link_type: &'static str,
}

/// Get the field reference rules for a given entity type.
///
/// These define direct fields (outside the `links:` section) that reference
/// other entities and should have reciprocal links added to the target.
/// Used by `validate` and `link sync` to detect and fix missing reciprocal links.
pub fn get_field_reference_rules(source_prefix: EntityPrefix) -> Vec<FieldReferenceRule> {
    match source_prefix {
        EntityPrefix::Quot => vec![
            FieldReferenceRule {
                field_name: "supplier",
                reciprocal_link_type: "quotes",
            },
            FieldReferenceRule {
                field_name: "component",
                reciprocal_link_type: "quotes",
            },
            FieldReferenceRule {
                field_name: "assembly",
                reciprocal_link_type: "quotes",
            },
        ],
        EntityPrefix::Feat => vec![FieldReferenceRule {
            field_name: "component",
            reciprocal_link_type: "features",
        }],
        EntityPrefix::Rslt => vec![
            FieldReferenceRule {
                field_name: "component",
                reciprocal_link_type: "results",
            },
            FieldReferenceRule {
                field_name: "assembly",
                reciprocal_link_type: "results",
            },
        ],
        _ => vec![],
    }
}

/// Add a link to an entity file using automatic type inference.
///
/// This function reads the entity file, determines the appropriate link type based on
/// the entity prefixes, adds the link, and writes the file back.
///
/// # Arguments
/// * `source_path` - Path to the source entity file
/// * `source_prefix` - Entity prefix of the source (e.g., EntityPrefix::Req)
/// * `target_id` - Full ID of the target entity
/// * `target_prefix` - Entity prefix of the target
/// * `target_title` - Optional title of the target entity (stored alongside ID)
///
/// # Returns
/// * `Ok(link_type)` - The link type that was added
/// * `Err` - If no link type could be inferred or if file operations failed
pub fn add_inferred_link(
    source_path: &std::path::Path,
    source_prefix: EntityPrefix,
    target_id: &str,
    target_prefix: EntityPrefix,
    target_title: Option<&str>,
) -> Result<String, String> {
    // Infer the link type
    let link_type = infer_link_type(source_prefix, target_prefix).ok_or_else(|| {
        format!(
            "Cannot infer link type for {} → {}",
            source_prefix, target_prefix
        )
    })?;

    // Read the file
    let content =
        std::fs::read_to_string(source_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Add the link
    let updated_content = add_link_to_yaml(&content, &link_type, target_id, target_title)?;

    // Write back
    std::fs::write(source_path, &updated_content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(link_type)
}

/// Build a link entry as a YAML Value with id and optional title.
fn make_link_entry(target_id: &str, target_title: Option<&str>) -> serde_yml::Value {
    let mut entry = serde_yml::Mapping::new();
    entry.insert(
        serde_yml::Value::String("id".to_string()),
        serde_yml::Value::String(target_id.to_string()),
    );
    if let Some(title) = target_title {
        entry.insert(
            serde_yml::Value::String("title".to_string()),
            serde_yml::Value::String(title.to_string()),
        );
    }
    serde_yml::Value::Mapping(entry)
}

/// Extract the entity ID from a link entry (supports both `{id, title}` mappings and bare strings).
pub fn extract_link_id(entry: &serde_yml::Value) -> Option<&str> {
    if let Some(m) = entry.as_mapping() {
        m.get(serde_yml::Value::String("id".to_string()))
            .and_then(|v| v.as_str())
    } else {
        entry.as_str()
    }
}

/// Extract the entity ID from a link value that may be a mapping or bare string (for single-value links).
fn extract_single_link_id(value: &serde_yml::Value) -> Option<&str> {
    extract_link_id(value)
}

/// Add a link to YAML content.
///
/// This function parses the YAML, adds the link to the appropriate field in the links section,
/// and returns the updated YAML string. Links are stored as `{id, title}` mappings.
fn add_link_to_yaml(
    content: &str,
    link_type: &str,
    target_id: &str,
    target_title: Option<&str>,
) -> Result<String, String> {
    // Parse YAML
    let mut value: serde_yml::Value =
        serde_yml::from_str(content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    // Navigate to links section, creating it if it doesn't exist
    if value.get("links").is_none() {
        value["links"] = serde_yml::Value::Mapping(serde_yml::Mapping::new());
    }

    let links = value
        .get_mut("links")
        .ok_or_else(|| "No 'links' section found in file".to_string())?;

    // Check if link type exists; if not, create it
    let link_value = if let Some(existing) = links.get_mut(link_type) {
        existing
    } else {
        // Determine if this should be an array or single-value link
        let is_array = is_array_link_type(link_type);
        let links_map = links
            .as_mapping_mut()
            .ok_or_else(|| "Links section is not a mapping".to_string())?;

        if is_array {
            links_map.insert(
                serde_yml::Value::String(link_type.to_string()),
                serde_yml::Value::Sequence(vec![]),
            );
        } else {
            links_map.insert(
                serde_yml::Value::String(link_type.to_string()),
                serde_yml::Value::Null,
            );
        }
        links
            .get_mut(link_type)
            .ok_or_else(|| "Failed to create link type".to_string())?
    };

    // Handle both array links and single-value links
    if let Some(arr) = link_value.as_sequence_mut() {
        // Array link - add to array if not already present
        let already_present = arr.iter().any(|v| extract_link_id(v) == Some(target_id));
        if !already_present {
            arr.push(make_link_entry(target_id, target_title));
        }
    } else if link_value.is_null()
        || link_value.as_str().is_some()
        || link_value.as_mapping().is_some()
    {
        // Single-value link - only replace if pointing at a different target,
        // to avoid dropping extended metadata (suspect, verified_revision) when
        // the user re-adds the same link.
        let current_id = extract_single_link_id(link_value).map(|s| s.to_string());
        if current_id.as_deref() != Some(target_id) {
            *link_value = make_link_entry(target_id, target_title);
        }
    } else {
        return Err(format!(
            "Link type '{}' has unexpected format (not array or single value)",
            link_type
        ));
    }

    // Serialize back
    serde_yml::to_string(&value).map_err(|e| format!("Failed to serialize YAML: {}", e))
}

/// Add a link to an entity file using an explicit link type.
///
/// This function reads the entity file, adds the link to the specified field,
/// and writes the file back. Unlike `add_inferred_link`, this allows specifying
/// any valid link type directly.
///
/// # Arguments
/// * `source_path` - Path to the source entity file
/// * `link_type` - The explicit link type field name (e.g., "mitigated_by", "verified_by")
/// * `target_id` - Full ID of the target entity
/// * `target_title` - Optional title of the target entity (stored alongside ID)
///
/// # Returns
/// * `Ok(())` - If the link was added successfully
/// * `Err` - If file operations failed
pub fn add_explicit_link(
    source_path: &std::path::Path,
    link_type: &str,
    target_id: &str,
    target_title: Option<&str>,
) -> Result<(), String> {
    // Read the file
    let content =
        std::fs::read_to_string(source_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Add the link
    let updated_content = add_link_to_yaml(&content, link_type, target_id, target_title)?;

    // Write back
    std::fs::write(source_path, &updated_content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Determine if a link type should be an array (multiple values) or single-value
fn is_array_link_type(link_type: &str) -> bool {
    match link_type {
        // Single-value links (can only have one target)
        "component" | "assembly" | "requirement" | "process" | "parent" | "supplier" | "capa"
        | "from_result" | "control" | "feature" | "test" | "created_ncr" | "product" => false,
        // Everything else is an array (can have multiple targets)
        _ => true,
    }
}

/// Remove a link from YAML content.
///
/// This function parses the YAML, removes the link from the appropriate field in the links section,
/// and returns the updated YAML string. Handles both `{id, title}` mappings and bare string entries.
fn remove_link_from_yaml(
    content: &str,
    link_type: &str,
    target_id: &str,
) -> Result<String, String> {
    // Parse YAML
    let mut value: serde_yml::Value =
        serde_yml::from_str(content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    // Navigate to links section
    let links = match value.get_mut("links") {
        Some(links) => links,
        None => return Ok(content.to_string()), // No links section, nothing to remove
    };

    // Check if link type exists
    let link_value = match links.get_mut(link_type) {
        Some(v) => v,
        None => return Ok(content.to_string()), // Link type doesn't exist, nothing to remove
    };

    // Handle both array links and single-value links
    let mut removed = false;
    if let Some(arr) = link_value.as_sequence_mut() {
        // Array link - remove entry matching target_id
        let original_len = arr.len();
        arr.retain(|v| extract_link_id(v) != Some(target_id));
        removed = arr.len() < original_len;

        // If array is now empty, remove the entire key
        if arr.is_empty() {
            if let Some(links_map) = links.as_mapping_mut() {
                links_map.remove(serde_yml::Value::String(link_type.to_string()));
            }
        }
    } else if extract_single_link_id(link_value) == Some(target_id) {
        // Single-value link matches - remove the key
        removed = true;
        if let Some(links_map) = links.as_mapping_mut() {
            links_map.remove(serde_yml::Value::String(link_type.to_string()));
        }
    }

    if !removed {
        return Err(format!(
            "Link '{}' -> '{}' not found in entity",
            link_type, target_id
        ));
    }

    // If links section is now empty, remove it entirely
    if let Some(links_map) = value.get("links").and_then(|l| l.as_mapping()) {
        if links_map.is_empty() {
            if let Some(root_map) = value.as_mapping_mut() {
                root_map.remove(serde_yml::Value::String("links".to_string()));
            }
        }
    }

    // Serialize back
    serde_yml::to_string(&value).map_err(|e| format!("Failed to serialize YAML: {}", e))
}

/// Remove a link from an entity file.
///
/// This function reads the entity file, removes the link from the specified field,
/// and writes the file back.
///
/// # Arguments
/// * `source_path` - Path to the source entity file
/// * `link_type` - The link type field name (e.g., "mitigated_by", "verified_by")
/// * `target_id` - Full ID of the target entity to unlink
///
/// # Returns
/// * `Ok(())` - If the link was removed successfully
/// * `Err` - If file operations failed or link wasn't found
pub fn remove_explicit_link(
    source_path: &std::path::Path,
    link_type: &str,
    target_id: &str,
) -> Result<(), String> {
    // Read the file
    let content =
        std::fs::read_to_string(source_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Remove the link
    let updated_content = remove_link_from_yaml(&content, link_type, target_id)?;

    // Write back
    std::fs::write(source_path, &updated_content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Update all link entries in a YAML file with current entity titles.
///
/// For each link entry, looks up the entity ID in `titles` and sets or updates
/// the `title` field. Also converts bare string entries to `{id, title}` mappings.
///
/// # Arguments
/// * `content` - YAML file content as a string
/// * `titles` - Map of entity ID → current title
///
/// # Returns
/// Updated YAML string, or error message
pub fn stamp_link_titles(
    content: &str,
    titles: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut value: serde_yml::Value =
        serde_yml::from_str(content).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    let mut changed = false;

    if let Some(links) = value.get_mut("links") {
        if let Some(links_map) = links.as_mapping_mut() {
            for (_key, link_val) in links_map.iter_mut() {
                if let Some(arr) = link_val.as_sequence_mut() {
                    for item in arr.iter_mut() {
                        if update_link_entry_title(item, titles) {
                            changed = true;
                        }
                    }
                } else {
                    // Single-value link
                    if update_link_entry_title(link_val, titles) {
                        changed = true;
                    }
                }
            }
        }
    }

    if !changed {
        return Ok(content.to_string());
    }

    serde_yml::to_string(&value).map_err(|e| format!("Failed to serialize YAML: {}", e))
}

/// Check all link entries in YAML content for missing or stale titles.
///
/// Returns a list of (entity_id, current_title_in_link, actual_title) tuples
/// for entries that need fixing.
pub fn check_link_titles(
    content: &str,
    titles: &std::collections::HashMap<String, String>,
) -> Vec<(String, Option<String>, String)> {
    let mut issues = Vec::new();

    let value: serde_yml::Value = match serde_yml::from_str(content) {
        Ok(v) => v,
        Err(_) => return issues,
    };

    if let Some(links) = value.get("links") {
        if let Some(links_map) = links.as_mapping() {
            for (_key, link_val) in links_map.iter() {
                if let Some(arr) = link_val.as_sequence() {
                    for item in arr {
                        check_entry_title(item, titles, &mut issues);
                    }
                } else {
                    check_entry_title(link_val, titles, &mut issues);
                }
            }
        }
    }

    issues
}

/// Check a single link entry for missing/stale title.
fn check_entry_title(
    entry: &serde_yml::Value,
    titles: &std::collections::HashMap<String, String>,
    issues: &mut Vec<(String, Option<String>, String)>,
) {
    let id = match extract_link_id(entry) {
        Some(id) => id.to_string(),
        None => return,
    };

    let actual_title = match titles.get(&id) {
        Some(t) => t.clone(),
        None => return, // Target entity not found - different validation concern
    };

    let current_title = entry
        .as_mapping()
        .and_then(|m| m.get(serde_yml::Value::String("title".to_string())))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if current_title.as_deref() != Some(&actual_title) {
        issues.push((id, current_title, actual_title));
    }
}

/// Update a single link entry's title from the titles map.
/// Returns true if the entry was modified.
fn update_link_entry_title(
    entry: &mut serde_yml::Value,
    titles: &std::collections::HashMap<String, String>,
) -> bool {
    // Handle mapping format {id: ..., title: ...}
    if let Some(m) = entry.as_mapping_mut() {
        if let Some(id) = m
            .get(serde_yml::Value::String("id".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            if let Some(title) = titles.get(&id) {
                let current = m
                    .get(serde_yml::Value::String("title".to_string()))
                    .and_then(|v| v.as_str());
                if current != Some(title.as_str()) {
                    m.insert(
                        serde_yml::Value::String("title".to_string()),
                        serde_yml::Value::String(title.clone()),
                    );
                    return true;
                }
            }
        }
        return false;
    }
    // Handle bare string format - convert to mapping
    if let Some(id) = entry.as_str().map(|s| s.to_string()) {
        if let Some(title) = titles.get(&id) {
            *entry = make_link_entry(&id, Some(title));
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_req_to_test_infers_verified_by() {
        assert_eq!(
            infer_link_type(EntityPrefix::Req, EntityPrefix::Test),
            Some("verified_by".to_string())
        );
    }

    #[test]
    fn test_test_to_req_infers_verifies() {
        assert_eq!(
            infer_link_type(EntityPrefix::Test, EntityPrefix::Req),
            Some("verifies".to_string())
        );
    }

    #[test]
    fn test_reciprocal_verified_by() {
        assert_eq!(
            get_reciprocal_link_type("verified_by", EntityPrefix::Test, EntityPrefix::Req),
            Some("verifies".to_string())
        );
    }

    #[test]
    fn test_reciprocal_verifies() {
        assert_eq!(
            get_reciprocal_link_type("verifies", EntityPrefix::Req, EntityPrefix::Test),
            Some("verified_by".to_string())
        );
    }

    #[test]
    fn test_reciprocal_ncrs_from_capa() {
        // CAPA.ncrs -> NCR should reciprocate as NCR.capa -> CAPA
        assert_eq!(
            get_reciprocal_link_type("ncrs", EntityPrefix::Ncr, EntityPrefix::Capa),
            Some("capa".to_string())
        );
    }

    #[test]
    fn test_reciprocal_capa_from_ncr() {
        // NCR.capa -> CAPA should reciprocate as CAPA.ncrs -> NCR
        assert_eq!(
            get_reciprocal_link_type("capa", EntityPrefix::Capa, EntityPrefix::Ncr),
            Some("ncrs".to_string())
        );
    }

    #[test]
    fn test_reciprocal_risks_disambiguated_by_source() {
        // CMP.risks -> RISK should reciprocate as RISK.component -> CMP
        assert_eq!(
            get_reciprocal_link_type("risks", EntityPrefix::Risk, EntityPrefix::Cmp),
            Some("component".to_string())
        );
        // REQ.risks -> RISK should reciprocate as RISK.requirement -> REQ
        assert_eq!(
            get_reciprocal_link_type("risks", EntityPrefix::Risk, EntityPrefix::Req),
            Some("requirement".to_string())
        );
        // FEAT.risks -> RISK should reciprocate as RISK.affects -> FEAT
        assert_eq!(
            get_reciprocal_link_type("risks", EntityPrefix::Risk, EntityPrefix::Feat),
            Some("affects".to_string())
        );
        // PROC.risks -> RISK should reciprocate as RISK.process -> PROC
        assert_eq!(
            get_reciprocal_link_type("risks", EntityPrefix::Risk, EntityPrefix::Proc),
            Some("process".to_string())
        );
    }

    #[test]
    fn test_reciprocal_component_disambiguated_by_source() {
        // TEST.component -> CMP should reciprocate as CMP.tests -> TEST
        assert_eq!(
            get_reciprocal_link_type("component", EntityPrefix::Cmp, EntityPrefix::Test),
            Some("tests".to_string())
        );
        // FEAT.component -> CMP should reciprocate as CMP.features -> FEAT
        assert_eq!(
            get_reciprocal_link_type("component", EntityPrefix::Cmp, EntityPrefix::Feat),
            Some("features".to_string())
        );
    }

    #[test]
    fn test_reciprocal_process_disambiguated_by_source() {
        // CTRL.process -> PROC should reciprocate as PROC.controls -> CTRL
        assert_eq!(
            get_reciprocal_link_type("process", EntityPrefix::Proc, EntityPrefix::Ctrl),
            Some("controls".to_string())
        );
        // WORK.process -> PROC should reciprocate as PROC.work_instructions -> WORK
        assert_eq!(
            get_reciprocal_link_type("process", EntityPrefix::Proc, EntityPrefix::Work),
            Some("work_instructions".to_string())
        );
    }

    #[test]
    fn test_order_symmetry_req_test() {
        // infer(REQ, TEST) and infer(TEST, REQ) should be reciprocals
        let forward = infer_link_type(EntityPrefix::Req, EntityPrefix::Test).unwrap();
        let reverse = infer_link_type(EntityPrefix::Test, EntityPrefix::Req).unwrap();
        assert_eq!(forward, "verified_by");
        assert_eq!(reverse, "verifies");
    }

    #[test]
    fn test_order_symmetry_ncr_capa() {
        // infer(NCR, CAPA) and infer(CAPA, NCR) should be reciprocals
        let forward = infer_link_type(EntityPrefix::Ncr, EntityPrefix::Capa).unwrap();
        let reverse = infer_link_type(EntityPrefix::Capa, EntityPrefix::Ncr).unwrap();
        assert_eq!(forward, "capa");
        assert_eq!(reverse, "ncrs");
    }

    #[test]
    fn test_same_type_defaults_to_related_to() {
        assert_eq!(
            infer_link_type(EntityPrefix::Proc, EntityPrefix::Proc),
            Some("related_to".to_string())
        );
    }

    /// Comprehensive test: for every entity pair (A, B) in the inference table,
    /// verify that when we infer a forward link type and look up its reciprocal,
    /// we get the same result as inferring in the reverse direction (B, A).
    /// This guarantees that `tdt link add A B` and `tdt link add B A` produce
    /// identical final state on both entities.
    #[test]
    fn test_all_inferred_pairs_have_symmetric_reciprocals() {
        use EntityPrefix::*;

        // Every (source, target) pair defined in infer_link_type (excluding same-type)
        let pairs: Vec<(EntityPrefix, EntityPrefix, &str)> = vec![
            // Requirements
            (Req, Test, "verified_by"),
            (Req, Ctrl, "verified_by"),
            (Req, Req, "derives_from"),
            (Req, Feat, "allocated_to"),
            (Req, Risk, "risks"),
            (Req, Cmp, "satisfied_by"),
            (Req, Asm, "satisfied_by"),
            // Verification
            (Test, Req, "verifies"),
            (Ctrl, Req, "verifies"),
            (Test, Cmp, "component"),
            (Test, Asm, "assembly"),
            // Results
            (Rslt, Cmp, "component"),
            (Rslt, Asm, "assembly"),
            (Rslt, Ncr, "ncrs"),
            // Risks
            (Risk, Req, "requirement"),
            (Risk, Cmp, "component"),
            (Risk, Asm, "assembly"),
            (Risk, Proc, "process"),
            (Risk, Feat, "affects"),
            (Risk, Test, "verified_by"),
            (Risk, Ctrl, "mitigated_by"),
            (Risk, Haz, "caused_by"),
            // Entities -> Risks
            (Feat, Risk, "risks"),
            (Cmp, Risk, "risks"),
            (Asm, Risk, "risks"),
            (Proc, Risk, "risks"),
            (Ctrl, Risk, "mitigates"),
            (Work, Risk, "risks"),
            // Components/Assemblies
            (Cmp, Req, "requirements"),
            (Asm, Req, "requirements"),
            (Cmp, Proc, "processes"),
            (Asm, Proc, "processes"),
            (Cmp, Test, "tests"),
            (Asm, Test, "tests"),
            (Cmp, Feat, "features"),
            (Asm, Feat, "features"),
            (Cmp, Cmp, "replaces"),
            (Cmp, Haz, "hazards"),
            (Asm, Haz, "hazards"),
            // Features
            (Feat, Cmp, "component"),
            (Feat, Req, "allocated_from"),
            // Processes
            (Proc, Req, "requirements"),
            (Proc, Cmp, "produces"),
            (Proc, Asm, "produces"),
            (Proc, Capa, "modified_by_capa"),
            // Processes -> Controls
            (Proc, Ctrl, "controls"),
            // Controls
            (Ctrl, Cmp, "component"),
            (Ctrl, Feat, "feature"),
            (Ctrl, Proc, "process"),
            (Ctrl, Capa, "added_by_capa"),
            (Ctrl, Haz, "controls_hazard"),
            // Work Instructions
            (Work, Cmp, "component"),
            (Work, Asm, "assembly"),
            (Work, Proc, "process"),
            (Work, Ctrl, "controls"),
            // NCR chain
            (Ncr, Rslt, "from_result"),
            (Ncr, Capa, "capa"),
            (Ncr, Cmp, "component"),
            (Ncr, Sup, "supplier"),
            (Ncr, Proc, "process"),
            // CAPA chain
            (Capa, Ncr, "ncrs"),
            (Capa, Proc, "processes_modified"),
            (Capa, Ctrl, "controls_added"),
            (Capa, Cmp, "component"),
            (Capa, Sup, "supplier"),
            (Capa, Risk, "risks"),
            // Hazards
            (Haz, Cmp, "originates_from"),
            (Haz, Asm, "originates_from"),
            (Haz, Risk, "causes"),
            (Haz, Ctrl, "controlled_by"),
            (Haz, Test, "verified_by"),
            (Haz, Req, "related_to"),
        ];

        let mut failures = Vec::new();

        for (source, target, expected_link) in &pairs {
            // Verify inference gives the expected link type
            let inferred = infer_link_type(*source, *target);
            if inferred.as_deref() != Some(expected_link) {
                failures.push(format!(
                    "INFER: ({:?}, {:?}) expected {:?} but got {:?}",
                    source, target, expected_link, inferred
                ));
                continue;
            }

            // Get the reciprocal of this forward link
            let reciprocal = get_reciprocal_link_type(expected_link, *target, *source);

            // Check if the reverse direction also has inference defined
            let reverse_inferred = infer_link_type(*target, *source);

            // For pairs where both directions have inference, the reciprocal
            // should match the reverse inference — EXCEPT for same-type pairs
            // with directional semantics (derives_from/derived_by, replaces/replaced_by)
            // where the inference always returns the default direction but the
            // reciprocal correctly returns the opposite direction.
            if let (Some(ref recip), Some(ref reverse)) = (&reciprocal, &reverse_inferred) {
                if recip != reverse && *source != *target {
                    failures.push(format!(
                        "MISMATCH: ({:?}, {:?}) link={:?}, reciprocal={:?} but reverse_infer={:?}",
                        source, target, expected_link, recip, reverse
                    ));
                }
            }

            // For pairs where the forward direction has inference, the reciprocal
            // should exist (either from get_reciprocal_link_type or from the
            // fallback to infer_link_type in the match)
            if reciprocal.is_none() && reverse_inferred.is_some() {
                failures.push(format!(
                    "MISSING RECIPROCAL: ({:?}, {:?}) link={:?}, no reciprocal but reverse_infer={:?}",
                    source, target, expected_link, reverse_inferred
                ));
            }
        }

        if !failures.is_empty() {
            panic!(
                "Link symmetry failures ({}):\n{}",
                failures.len(),
                failures.join("\n")
            );
        }
    }
}
