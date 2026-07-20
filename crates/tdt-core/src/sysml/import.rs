//! SysML v2 import - parses SysML textual notation and converts to TDT entities

use crate::core::suspect::LinkRef;
use std::collections::HashMap;

use chrono::Utc;
use pest::Parser;
use pest_derive::Parser;

use crate::core::entity::{Priority, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::entities::component::{Component, ComponentCategory, MakeBuy};
use crate::entities::requirement::{Level, Requirement, RequirementType};
use crate::entities::test::{Test, TestMethod, TestType};
use crate::sysml::mapping::{sysml_name_to_title, sysml_to_method};
use crate::sysml::model::*;

#[derive(Parser)]
#[grammar = "sysml/sysml_subset.pest"]
struct SysmlParser;

/// Parse a SysML v2 text file into the intermediate representation.
///
/// Unrecognized SysML constructs (e.g., `action def`, `state def`, `port def`)
/// are silently skipped. The count of skipped constructs is returned in
/// `SysmlPackage::skipped_count` for diagnostic reporting.
pub fn parse_sysml(text: &str) -> Result<SysmlPackage, String> {
    let pairs =
        SysmlParser::parse(Rule::file, text).map_err(|e| format!("SysML parse error: {}", e))?;

    let mut package = SysmlPackage::default();

    for pair in pairs {
        if pair.as_rule() == Rule::file {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::package {
                    parse_package(inner, &mut package)?;
                }
            }
        }
    }

    Ok(package)
}

fn parse_package(pair: pest::iterators::Pair<Rule>, pkg: &mut SysmlPackage) -> Result<(), String> {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::identifier => {
                pkg.name = inner.as_str().to_string();
            }
            Rule::package_body => {
                for item in inner.into_inner() {
                    parse_package_item(item, pkg)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_package_item(
    pair: pest::iterators::Pair<Rule>,
    pkg: &mut SysmlPackage,
) -> Result<(), String> {
    match pair.as_rule() {
        Rule::package_item => {
            for inner in pair.into_inner() {
                parse_package_item(inner, pkg)?;
            }
        }
        Rule::requirement_def => {
            pkg.requirements.push(parse_requirement_def(pair)?);
        }
        Rule::verification_def => {
            pkg.verifications.push(parse_verification_def(pair)?);
        }
        Rule::part_def => {
            pkg.parts.push(parse_part_def(pair)?);
        }
        Rule::satisfy_stmt => {
            pkg.satisfy_rels.push(parse_satisfy_stmt(pair)?);
        }
        Rule::import_stmt | Rule::line_comment | Rule::block_comment => {
            // Skip
        }
        Rule::unknown_item => {
            pkg.skipped_count += 1;
        }
        _ => {}
    }
    Ok(())
}

fn parse_requirement_def(pair: pest::iterators::Pair<Rule>) -> Result<SysmlRequirement, String> {
    let mut req = SysmlRequirement {
        tdt_id: String::new(),
        short_id: String::new(),
        name: String::new(),
        doc: String::new(),
        priority: None,
        level: None,
        status: None,
        author: None,
        category: None,
        tags: Vec::new(),
        rationale: None,
        req_type: None,
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::short_id => {
                req.short_id = extract_short_id(inner);
                req.tdt_id = req.short_id.clone();
            }
            Rule::identifier => {
                req.name = inner.as_str().to_string();
            }
            Rule::req_body => {
                for item in inner.into_inner() {
                    parse_req_item(item, &mut req);
                }
            }
            _ => {}
        }
    }

    Ok(req)
}

fn parse_req_item(pair: pest::iterators::Pair<Rule>, req: &mut SysmlRequirement) {
    match pair.as_rule() {
        Rule::req_item => {
            for inner in pair.into_inner() {
                parse_req_item(inner, req);
            }
        }
        Rule::doc_block => {
            req.doc = extract_doc_content(pair);
        }
        Rule::annotation => {
            let (name, kvs) = parse_annotation(pair);
            if name == "TdtMetadata" {
                apply_tdt_metadata_to_req(req, &kvs);
            }
        }
        _ => {}
    }
}

fn parse_verification_def(
    pair: pest::iterators::Pair<Rule>,
) -> Result<SysmlVerificationCase, String> {
    let mut vc = SysmlVerificationCase {
        tdt_id: String::new(),
        short_id: String::new(),
        name: String::new(),
        doc: String::new(),
        method: None,
        verifies: Vec::new(),
        status: None,
        author: None,
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::short_id => {
                vc.short_id = extract_short_id(inner);
                vc.tdt_id = vc.short_id.clone();
            }
            Rule::identifier => {
                vc.name = inner.as_str().to_string();
            }
            Rule::verif_body => {
                for item in inner.into_inner() {
                    parse_verif_item(item, &mut vc);
                }
            }
            _ => {}
        }
    }

    Ok(vc)
}

fn parse_verif_item(pair: pest::iterators::Pair<Rule>, vc: &mut SysmlVerificationCase) {
    match pair.as_rule() {
        Rule::verif_item => {
            for inner in pair.into_inner() {
                parse_verif_item(inner, vc);
            }
        }
        Rule::doc_block => {
            vc.doc = extract_doc_content(pair);
        }
        Rule::annotation => {
            let (name, kvs) = parse_annotation(pair);
            if name == "VerificationMethod" {
                if let Some(kind) = kvs.get("kind") {
                    vc.method = Some(kind.clone());
                }
            } else if name == "TdtMetadata" {
                if let Some(status) = kvs.get("status") {
                    vc.status = Some(status.clone());
                }
                if let Some(author) = kvs.get("author") {
                    vc.author = Some(author.clone());
                }
            }
        }
        Rule::objective_block => {
            for item in pair.into_inner() {
                if item.as_rule() == Rule::objective_item {
                    for inner in item.into_inner() {
                        if inner.as_rule() == Rule::verify_req_stmt {
                            for id_pair in inner.into_inner() {
                                if id_pair.as_rule() == Rule::identifier {
                                    vc.verifies.push(id_pair.as_str().to_string());
                                }
                            }
                        }
                    }
                } else if item.as_rule() == Rule::verify_req_stmt {
                    for id_pair in item.into_inner() {
                        if id_pair.as_rule() == Rule::identifier {
                            vc.verifies.push(id_pair.as_str().to_string());
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_part_def(pair: pest::iterators::Pair<Rule>) -> Result<SysmlPartDef, String> {
    let mut part = SysmlPartDef {
        tdt_id: String::new(),
        short_id: String::new(),
        name: String::new(),
        doc: None,
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::short_id => {
                part.short_id = extract_short_id(inner);
                part.tdt_id = part.short_id.clone();
            }
            Rule::identifier => {
                if part.name.is_empty() {
                    part.name = inner.as_str().to_string();
                }
            }
            Rule::part_body => {
                for item in inner.into_inner() {
                    if let Rule::part_item = item.as_rule() {
                        for inner_item in item.into_inner() {
                            if inner_item.as_rule() == Rule::doc_block {
                                part.doc = Some(extract_doc_content(inner_item));
                            }
                        }
                    } else if item.as_rule() == Rule::doc_block {
                        part.doc = Some(extract_doc_content(item));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(part)
}

fn parse_satisfy_stmt(pair: pest::iterators::Pair<Rule>) -> Result<SatisfyRelationship, String> {
    let mut names: Vec<String> = Vec::new();
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::identifier {
            names.push(inner.as_str().to_string());
        }
    }

    if names.len() >= 2 {
        Ok(SatisfyRelationship {
            requirement_name: names[0].clone(),
            satisfied_by: names[1].clone(),
        })
    } else {
        Err("Invalid satisfy statement: expected two identifiers".to_string())
    }
}

// Helper functions

fn extract_short_id(pair: pest::iterators::Pair<Rule>) -> String {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::short_id_content {
            return inner.as_str().to_string();
        }
    }
    String::new()
}

fn extract_doc_content(pair: pest::iterators::Pair<Rule>) -> String {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::block_comment_raw {
            let s = inner.as_str();
            // Strip /* and */ delimiters
            let content = s
                .strip_prefix("/*")
                .unwrap_or(s)
                .strip_suffix("*/")
                .unwrap_or(s);
            return content.trim().to_string();
        }
    }
    String::new()
}

fn parse_annotation(pair: pest::iterators::Pair<Rule>) -> (String, HashMap<String, String>) {
    let mut name = String::new();
    let mut kvs = HashMap::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::identifier => {
                if name.is_empty() {
                    name = inner.as_str().to_string();
                }
            }
            Rule::annotation_body => {
                for item in inner.into_inner() {
                    match item.as_rule() {
                        Rule::annotation_item => {
                            for kv in item.into_inner() {
                                parse_annotation_kv_pair(kv, &mut kvs);
                            }
                        }
                        Rule::annotation_kv | Rule::annotation_enum_kv => {
                            parse_annotation_kv_pair(item, &mut kvs);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    (name, kvs)
}

fn parse_annotation_kv_pair(pair: pest::iterators::Pair<Rule>, kvs: &mut HashMap<String, String>) {
    match pair.as_rule() {
        Rule::annotation_kv => {
            let mut key = String::new();
            let mut value = String::new();
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::identifier => {
                        if key.is_empty() {
                            key = inner.as_str().to_string();
                        }
                    }
                    Rule::annotation_value => {
                        for v in inner.into_inner() {
                            match v.as_rule() {
                                Rule::quoted_string => {
                                    let s = v.as_str();
                                    // Strip quotes
                                    value = s[1..s.len() - 1].to_string();
                                }
                                Rule::identifier => {
                                    value = v.as_str().to_string();
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !key.is_empty() {
                kvs.insert(key, value);
            }
        }
        Rule::annotation_enum_kv => {
            let mut key = String::new();
            let mut parts: Vec<String> = Vec::new();
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::identifier {
                    if key.is_empty() {
                        key = inner.as_str().to_string();
                    } else {
                        parts.push(inner.as_str().to_string());
                    }
                }
            }
            if !key.is_empty() && !parts.is_empty() {
                // Store just the last part (the enum variant)
                kvs.insert(key, parts.last().unwrap().clone());
            }
        }
        _ => {}
    }
}

fn apply_tdt_metadata_to_req(req: &mut SysmlRequirement, kvs: &HashMap<String, String>) {
    if let Some(status) = kvs.get("status") {
        req.status = Some(status.clone());
    }
    if let Some(author) = kvs.get("author") {
        req.author = Some(author.clone());
    }
    if let Some(priority) = kvs.get("priority") {
        req.priority = Some(priority.clone());
    }
    if let Some(level) = kvs.get("level") {
        req.level = Some(level.clone());
    }
    if let Some(category) = kvs.get("category") {
        req.category = Some(category.clone());
    }
    if let Some(tags) = kvs.get("tags") {
        req.tags = tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Some(rationale) = kvs.get("rationale") {
        req.rationale = Some(rationale.clone());
    }
    if let Some(req_type) = kvs.get("req_type") {
        req.req_type = Some(req_type.clone());
    }
}

/// Convert a parsed SysML package into TDT entity YAML files.
pub fn convert_to_entities(pkg: &SysmlPackage, author: &str) -> Result<ImportResult, String> {
    let mut result = ImportResult::default();
    let now = Utc::now();

    // Build name->ID maps for link resolution
    // First, map SysML names to their TDT IDs or generate new ones
    let mut req_name_to_id: HashMap<String, EntityId> = HashMap::new();
    let mut cmp_name_to_id: HashMap<String, EntityId> = HashMap::new();
    let mut test_name_to_id: HashMap<String, EntityId> = HashMap::new();

    // Process requirements: assign IDs
    for sysml_req in &pkg.requirements {
        let id = resolve_or_generate_id(&sysml_req.tdt_id, EntityPrefix::Req);
        req_name_to_id.insert(sysml_req.name.clone(), id);
    }

    // Process parts: assign IDs
    for sysml_part in &pkg.parts {
        let id = resolve_or_generate_id(&sysml_part.tdt_id, EntityPrefix::Cmp);
        cmp_name_to_id.insert(sysml_part.name.clone(), id);
    }

    // Process verifications: assign IDs
    for sysml_vc in &pkg.verifications {
        let id = resolve_or_generate_id(&sysml_vc.tdt_id, EntityPrefix::Test);
        test_name_to_id.insert(sysml_vc.name.clone(), id);
    }

    // Build satisfy lookup: req_name -> vec of cmp names
    let mut satisfy_map: HashMap<String, Vec<String>> = HashMap::new();
    for rel in &pkg.satisfy_rels {
        satisfy_map
            .entry(rel.requirement_name.clone())
            .or_default()
            .push(rel.satisfied_by.clone());
    }

    // Convert requirements
    for sysml_req in &pkg.requirements {
        let id = req_name_to_id[&sysml_req.name].clone();
        let title = sysml_name_to_title(&sysml_req.name);

        let req_type = sysml_req
            .req_type
            .as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "input" => Some(RequirementType::Input),
                "output" => Some(RequirementType::Output),
                _ => None,
            })
            .unwrap_or(RequirementType::Input);

        let status = sysml_req
            .status
            .as_deref()
            .and_then(|s| s.parse::<Status>().ok())
            .unwrap_or(Status::Draft);

        let priority = sysml_req
            .priority
            .as_deref()
            .and_then(|s| s.parse::<Priority>().ok())
            .unwrap_or(Priority::Medium);

        let level = sysml_req
            .level
            .as_deref()
            .and_then(|s| s.parse::<Level>().ok())
            .unwrap_or(Level::System);

        let req_author = sysml_req.author.as_deref().unwrap_or(author);

        let mut req = Requirement::new(
            req_type,
            title.clone(),
            sysml_req.doc.clone(),
            req_author.to_string(),
        );
        req.id = id.clone();
        req.status = status;
        req.priority = priority;
        req.level = level;
        req.category = sysml_req.category.clone();
        req.tags = sysml_req.tags.clone();
        req.rationale = sysml_req.rationale.clone();
        req.created = now;

        // Build verified_by links from verification objectives
        for sysml_vc in &pkg.verifications {
            if sysml_vc.verifies.contains(&sysml_req.name) {
                if let Some(test_id) = test_name_to_id.get(&sysml_vc.name) {
                    req.links.verified_by.push(LinkRef::from(test_id));
                }
            }
        }

        // Build satisfied_by links from satisfy relationships
        if let Some(satisfiers) = satisfy_map.get(&sysml_req.name) {
            for cmp_name in satisfiers {
                if let Some(cmp_id) = cmp_name_to_id.get(cmp_name) {
                    req.links.satisfied_by.push(LinkRef::from(cmp_id));
                }
            }
        }

        let yaml =
            serde_yml::to_string(&req).map_err(|e| format!("YAML serialization error: {}", e))?;
        result.entities.push(ImportedEntity {
            prefix: "REQ".to_string(),
            id: id.to_string(),
            title,
            yaml,
        });
    }

    // Convert parts to components
    for sysml_part in &pkg.parts {
        let id = cmp_name_to_id[&sysml_part.name].clone();
        let title = sysml_name_to_title(&sysml_part.name);

        let mut cmp = Component::new(
            String::new(), // part_number - empty for imports
            title.clone(),
            MakeBuy::Buy,
            ComponentCategory::Mechanical,
            author.to_string(),
        );
        cmp.id = id.clone();
        cmp.description = sysml_part.doc.clone();
        cmp.created = now;

        let yaml =
            serde_yml::to_string(&cmp).map_err(|e| format!("YAML serialization error: {}", e))?;
        result.entities.push(ImportedEntity {
            prefix: "CMP".to_string(),
            id: id.to_string(),
            title,
            yaml,
        });
    }

    // Convert verifications to tests
    for sysml_vc in &pkg.verifications {
        let id = test_name_to_id[&sysml_vc.name].clone();
        let title = sysml_name_to_title(&sysml_vc.name);

        let method =
            sysml_vc
                .method
                .as_deref()
                .map(|m| match sysml_to_method(m).to_lowercase().as_str() {
                    "inspection" => TestMethod::Inspection,
                    "analysis" => TestMethod::Analysis,
                    "demonstration" => TestMethod::Demonstration,
                    _ => TestMethod::Test,
                });

        let status = sysml_vc
            .status
            .as_deref()
            .and_then(|s| s.parse::<Status>().ok())
            .unwrap_or(Status::Draft);

        let test_author = sysml_vc.author.as_deref().unwrap_or(author);

        let mut test = Test::new(
            TestType::Verification,
            title.clone(),
            sysml_vc.doc.clone(),
            test_author.to_string(),
        );
        test.id = id.clone();
        test.test_method = method;
        test.status = status;
        test.created = now;

        // Build verifies links
        for req_name in &sysml_vc.verifies {
            if let Some(req_id) = req_name_to_id.get(req_name) {
                test.links.verifies.push(LinkRef::from(req_id));
            } else {
                result.warnings.push(format!(
                    "Verification '{}' references unknown requirement '{}'",
                    sysml_vc.name, req_name
                ));
            }
        }

        let yaml =
            serde_yml::to_string(&test).map_err(|e| format!("YAML serialization error: {}", e))?;
        result.entities.push(ImportedEntity {
            prefix: "TEST".to_string(),
            id: id.to_string(),
            title,
            yaml,
        });
    }

    Ok(result)
}

/// If the short_id looks like a valid TDT EntityId, parse it; otherwise generate a new one.
fn resolve_or_generate_id(short_id: &str, prefix: EntityPrefix) -> EntityId {
    if !short_id.is_empty() {
        if let Ok(id) = EntityId::parse(short_id) {
            return id;
        }
    }
    EntityId::new(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_package() {
        let input = r#"package TestPackage {
    import Requirements::*;
    import VerificationCases::*;
}"#;
        let pkg = parse_sysml(input).unwrap();
        assert_eq!(pkg.name, "TestPackage");
        assert!(pkg.requirements.is_empty());
    }

    #[test]
    fn test_parse_requirement() {
        let input = r#"package Test {
    requirement def <'REQ-01AAAAAAAAAAAAAAAAAAAAAAAAAA'> StrokeLength {
        doc /* The actuator shall have a minimum stroke length of 100mm. */
        @TdtMetadata { status = approved; author = "Jack Hale"; priority = high; level = system; }
    }
}"#;
        let pkg = parse_sysml(input).unwrap();
        assert_eq!(pkg.requirements.len(), 1);
        let req = &pkg.requirements[0];
        assert_eq!(req.name, "StrokeLength");
        assert_eq!(req.short_id, "REQ-01AAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert!(req.doc.contains("minimum stroke length"));
        assert_eq!(req.status.as_deref(), Some("approved"));
        assert_eq!(req.priority.as_deref(), Some("high"));
    }

    #[test]
    fn test_parse_verification() {
        let input = r#"package Test {
    verification def <'TEST-01BBBBBBBBBBBBBBBBBBBBBBBBBB'> ClearanceCheck {
        doc /* Verify shaft-bushing clearance. */
        @VerificationMethod { kind = VerificationMethodKind::inspection; }
        objective {
            verify requirement : StrokeLength;
        }
        return verdict : VerdictKind;
    }
}"#;
        let pkg = parse_sysml(input).unwrap();
        assert_eq!(pkg.verifications.len(), 1);
        let vc = &pkg.verifications[0];
        assert_eq!(vc.name, "ClearanceCheck");
        assert_eq!(vc.method.as_deref(), Some("inspection"));
        assert_eq!(vc.verifies, vec!["StrokeLength"]);
    }

    #[test]
    fn test_parse_part_def() {
        let input = r#"package Test {
    part def <'CMP-01CCCCCCCCCCCCCCCCCCCCCCCCCC'> ActuatorShaft;
}"#;
        let pkg = parse_sysml(input).unwrap();
        assert_eq!(pkg.parts.len(), 1);
        assert_eq!(pkg.parts[0].name, "ActuatorShaft");
    }

    #[test]
    fn test_parse_satisfy() {
        let input = r#"package Test {
    satisfy requirement : StrokeLength by ActuatorShaft;
}"#;
        let pkg = parse_sysml(input).unwrap();
        assert_eq!(pkg.satisfy_rels.len(), 1);
        assert_eq!(pkg.satisfy_rels[0].requirement_name, "StrokeLength");
        assert_eq!(pkg.satisfy_rels[0].satisfied_by, "ActuatorShaft");
    }

    #[test]
    fn test_parse_with_unknown_constructs() {
        let input = r#"package MixedModel {
    import Requirements::*;
    import VerificationCases::*;

    // Known constructs
    requirement def <'REQ-01AAAAAAAAAAAAAAAAAAAAAAAAAA'> StrokeLength {
        doc /* The actuator shall have a minimum stroke length of 100mm. */
    }

    // Unknown constructs that should be skipped
    action def DoSomething {
        first start;
        then done;
    }

    attribute def Power {
        attribute watts : Real;
    }

    enum def Color {
        enum red;
        enum green;
        enum blue;
    }

    state def Running {
        entry; then do; then exit;
    }

    port def SensorPort;

    allocate something to somethingElse;

    // Another known construct after unknowns
    part def <'CMP-01CCCCCCCCCCCCCCCCCCCCCCCCCC'> ActuatorShaft;

    satisfy requirement : StrokeLength by ActuatorShaft;
}"#;
        let pkg = parse_sysml(input).unwrap();
        assert_eq!(pkg.name, "MixedModel");
        assert_eq!(pkg.requirements.len(), 1);
        assert_eq!(pkg.parts.len(), 1);
        assert_eq!(pkg.satisfy_rels.len(), 1);
        // 5 unknown constructs: action def, attribute def, enum def, state def, port def + allocate stmt = 6
        assert_eq!(pkg.skipped_count, 6);
    }

    #[test]
    fn test_convert_to_entities() {
        let pkg = SysmlPackage {
            name: "Test".to_string(),
            requirements: vec![SysmlRequirement {
                tdt_id: String::new(),
                short_id: String::new(),
                name: "StrokeLength".to_string(),
                doc: "The actuator shall have a minimum stroke length.".to_string(),
                priority: Some("high".to_string()),
                level: Some("system".to_string()),
                status: Some("draft".to_string()),
                author: Some("test".to_string()),
                category: None,
                tags: vec![],
                rationale: None,
                req_type: Some("input".to_string()),
            }],
            verifications: vec![],
            parts: vec![],
            satisfy_rels: vec![],
            skipped_count: 0,
        };

        let result = convert_to_entities(&pkg, "test").unwrap();
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].prefix, "REQ");
        assert!(result.entities[0].yaml.contains("Stroke Length"));
    }
}
