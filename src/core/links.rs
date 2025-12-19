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
/// ```ignore
/// // REQ linking to TEST -> verified_by
/// infer_link_type(EntityPrefix::Req, EntityPrefix::Test) == Some("verified_by")
///
/// // RISK linking to CMP -> component
/// infer_link_type(EntityPrefix::Risk, EntityPrefix::Cmp) == Some("component")
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
        (EntityPrefix::Risk, EntityPrefix::Ctrl) => Some("controls".to_string()),

        // Entities referencing risks
        (EntityPrefix::Feat, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Cmp, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Proc, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Risk) => Some("risks".to_string()),
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

        // Processes linking to requirements
        (EntityPrefix::Proc, EntityPrefix::Req) => Some("requirements".to_string()),

        // Processes linking to components/assemblies (produces)
        (EntityPrefix::Proc, EntityPrefix::Cmp) => Some("produces".to_string()),
        (EntityPrefix::Proc, EntityPrefix::Asm) => Some("produces".to_string()),

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

        // CAPAs
        (EntityPrefix::Capa, EntityPrefix::Ncr) => Some("ncrs".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Proc) => Some("processes_modified".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Ctrl) => Some("controls_added".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Cmp) => Some("component".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Sup) => Some("supplier".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Risk) => Some("risks".to_string()),

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

/// Get the reciprocal link type for a given forward link type and target entity prefix.
///
/// When entity A links to entity B, this function returns what field on B
/// should link back to A (if any).
///
/// Returns None if no reciprocal link should be created.
pub fn get_reciprocal_link_type(link_type: &str, target_prefix: EntityPrefix) -> Option<String> {
    match (link_type, target_prefix) {
        // REQ.verified_by -> TEST means TEST.verifies -> REQ
        ("verified_by", EntityPrefix::Test) => Some("verifies".to_string()),

        // REQ.verified_by -> CTRL means CTRL.verifies -> REQ
        ("verified_by", EntityPrefix::Ctrl) => Some("verifies".to_string()),

        // REQ.satisfied_by -> REQ means bidirectional satisfaction
        ("satisfied_by", EntityPrefix::Req) => Some("satisfied_by".to_string()),

        // REQ.satisfied_by -> CMP/ASM means CMP/ASM.requirements -> REQ
        ("satisfied_by", EntityPrefix::Cmp) => Some("requirements".to_string()),
        ("satisfied_by", EntityPrefix::Asm) => Some("requirements".to_string()),

        // TEST.verifies -> REQ or CTRL.verifies -> REQ means REQ.verified_by
        ("verifies", EntityPrefix::Req) => Some("verified_by".to_string()),

        // TEST/RSLT linking to component/assembly - reciprocal is tests array
        ("component", EntityPrefix::Cmp) => Some("tests".to_string()),
        ("assembly", EntityPrefix::Asm) => Some("tests".to_string()),

        // CMP/ASM.tests -> TEST means TEST.component or TEST.assembly (no reciprocal needed)
        ("tests", EntityPrefix::Test) => None,

        // CMP/ASM.requirements -> REQ means REQ.satisfied_by -> CMP/ASM
        ("requirements", EntityPrefix::Req) => Some("satisfied_by".to_string()),

        // CMP/ASM.processes -> PROC means PROC.produces -> CMP
        ("processes", EntityPrefix::Proc) => Some("produces".to_string()),
        ("produces", EntityPrefix::Cmp) => Some("processes".to_string()),
        ("produces", EntityPrefix::Asm) => Some("processes".to_string()),

        // RISK single-value links
        ("requirement", EntityPrefix::Req) => Some("risks".to_string()),
        ("component", EntityPrefix::Risk) => None, // RISK.component is single-value
        ("assembly", EntityPrefix::Risk) => None,  // RISK.assembly is single-value
        ("process", EntityPrefix::Proc) => Some("risks".to_string()),

        // RISK.controls -> CTRL means CTRL.risks -> RISK
        ("controls", EntityPrefix::Ctrl) => Some("risks".to_string()),

        // related_to is symmetric
        ("related_to", _) => Some("related_to".to_string()),

        // capa link
        ("capa", EntityPrefix::Capa) => Some("ncrs".to_string()),

        // Requirement decomposition: derives_from <-> derived_by
        ("derives_from", EntityPrefix::Req) => Some("derived_by".to_string()),
        ("derived_by", EntityPrefix::Req) => Some("derives_from".to_string()),

        // Requirement allocation: allocated_to <-> allocated_from
        ("allocated_to", EntityPrefix::Feat) => Some("allocated_from".to_string()),
        ("allocated_from", EntityPrefix::Req) => Some("allocated_to".to_string()),

        // REQ.risks -> RISK means RISK.requirement -> REQ
        ("risks", EntityPrefix::Risk) => Some("requirement".to_string()),

        // RISK.affects -> target.risks
        ("affects", EntityPrefix::Feat) => Some("risks".to_string()),
        ("affects", EntityPrefix::Cmp) => Some("risks".to_string()),
        ("affects", EntityPrefix::Asm) => Some("risks".to_string()),
        ("affects", EntityPrefix::Proc) => Some("risks".to_string()),

        // Result -> NCR: from_result link
        ("from_result", EntityPrefix::Rslt) => Some("ncrs".to_string()),

        // NCR single-value links (component, supplier, process) - no reciprocals
        ("supplier", _) => None, // Supplier doesn't link back to NCRs/CAPAs

        // CAPA -> Process/Control modifications
        ("processes_modified", EntityPrefix::Proc) => Some("modified_by_capa".to_string()),
        ("modified_by_capa", EntityPrefix::Capa) => Some("processes_modified".to_string()),
        ("controls_added", EntityPrefix::Ctrl) => Some("added_by_capa".to_string()),
        ("added_by_capa", EntityPrefix::Capa) => Some("controls_added".to_string()),

        // Component supersession: replaces <-> replaced_by
        ("replaces", EntityPrefix::Cmp) => Some("replaced_by".to_string()),
        ("replaced_by", EntityPrefix::Cmp) => Some("replaces".to_string()),

        // Component interchangeability is symmetric
        ("interchangeable_with", EntityPrefix::Cmp) => Some("interchangeable_with".to_string()),

        // No reciprocal defined for other cases
        (_, _) => None,
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
///
/// # Returns
/// * `Ok(link_type)` - The link type that was added
/// * `Err` - If no link type could be inferred or if file operations failed
pub fn add_inferred_link(
    source_path: &std::path::Path,
    source_prefix: EntityPrefix,
    target_id: &str,
    target_prefix: EntityPrefix,
) -> Result<String, String> {
    // Infer the link type
    let link_type = infer_link_type(source_prefix, target_prefix)
        .ok_or_else(|| format!(
            "Cannot infer link type for {} → {}",
            source_prefix, target_prefix
        ))?;

    // Read the file
    let content = std::fs::read_to_string(source_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Add the link
    let updated_content = add_link_to_yaml(&content, &link_type, target_id)?;

    // Write back
    std::fs::write(source_path, &updated_content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(link_type)
}

/// Add a link to YAML content.
///
/// This function parses the YAML, adds the link to the appropriate field in the links section,
/// and returns the updated YAML string.
fn add_link_to_yaml(content: &str, link_type: &str, target_id: &str) -> Result<String, String> {
    // Parse YAML
    let mut value: serde_yml::Value = serde_yml::from_str(content)
        .map_err(|e| format!("Failed to parse YAML: {}", e))?;

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
        let new_value = serde_yml::Value::String(target_id.to_string());
        if !arr.contains(&new_value) {
            arr.push(new_value);
        }
    } else if link_value.is_null() || link_value.as_str().is_some() {
        // Single-value link (null or existing string) - replace with new value
        *link_value = serde_yml::Value::String(target_id.to_string());
    } else {
        return Err(format!(
            "Link type '{}' has unexpected format (not array or single value)",
            link_type
        ));
    }

    // Serialize back
    serde_yml::to_string(&value).map_err(|e| format!("Failed to serialize YAML: {}", e))
}

/// Determine if a link type should be an array (multiple values) or single-value
fn is_array_link_type(link_type: &str) -> bool {
    match link_type {
        // Single-value links (can only have one target)
        "component" | "assembly" | "requirement" | "process" | "parent" | "supplier" | "capa"
        | "from_result" | "control" | "feature" | "test" => false,
        // Everything else is an array (can have multiple targets)
        _ => true,
    }
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
            get_reciprocal_link_type("verified_by", EntityPrefix::Test),
            Some("verifies".to_string())
        );
    }

    #[test]
    fn test_reciprocal_verifies() {
        assert_eq!(
            get_reciprocal_link_type("verifies", EntityPrefix::Req),
            Some("verified_by".to_string())
        );
    }

    #[test]
    fn test_same_type_defaults_to_related_to() {
        assert_eq!(
            infer_link_type(EntityPrefix::Proc, EntityPrefix::Proc),
            Some("related_to".to_string())
        );
    }
}
