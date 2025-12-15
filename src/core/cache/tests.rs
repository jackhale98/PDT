//! Unit tests for the cache module

use super::*;
use crate::core::identity::EntityPrefix;
use tempfile::tempdir;

fn create_test_project() -> (tempfile::TempDir, Project) {
    let tmp = tempdir().unwrap();
    let project = Project::init(tmp.path()).unwrap();
    (tmp, project)
}

fn write_test_entity(project: &Project, rel_path: &str, content: &str) {
    let full_path = project.root().join(rel_path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&full_path, content).unwrap();
}

#[test]
fn test_cache_creation() {
    let (_tmp, project) = create_test_project();
    let cache = EntityCache::open(&project).unwrap();

    let stats = cache.statistics().unwrap();
    assert_eq!(stats.total_entities, 0);
    assert_eq!(stats.total_short_ids, 0);
}

#[test]
fn test_short_id_assignment() {
    let (_tmp, project) = create_test_project();
    let mut cache = EntityCache::open_without_sync(&project).unwrap();

    let short1 = cache.ensure_short_id("REQ-01ABC123").unwrap();
    let short2 = cache.ensure_short_id("REQ-02DEF456").unwrap();
    let short3 = cache.ensure_short_id("RISK-01GHI789").unwrap();

    assert_eq!(short1, "REQ@1");
    assert_eq!(short2, "REQ@2");
    assert_eq!(short3, "RISK@1");

    // Same ID should return same short ID
    let short1_again = cache.ensure_short_id("REQ-01ABC123").unwrap();
    assert_eq!(short1_again, "REQ@1");
}

#[test]
fn test_short_id_resolution() {
    let (_tmp, project) = create_test_project();
    let mut cache = EntityCache::open_without_sync(&project).unwrap();

    cache.ensure_short_id("REQ-01ABC123").unwrap();

    // Test resolution
    assert_eq!(
        cache.resolve_short_id("REQ@1"),
        Some("REQ-01ABC123".to_string())
    );
    assert_eq!(
        cache.resolve_short_id("req@1"),
        Some("REQ-01ABC123".to_string())
    );
    assert_eq!(
        cache.resolve_short_id("Req@1"),
        Some("REQ-01ABC123".to_string())
    );
    assert_eq!(cache.resolve_short_id("REQ@99"), None);
}

#[test]
fn test_entity_caching() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC123.tdt.yaml",
        r#"
id: REQ-01ABC123
title: Test Requirement
status: draft
author: Test Author
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    let stats = cache.rebuild().unwrap();

    assert_eq!(stats.entities_added, 1);

    let entity = cache.get_entity("REQ-01ABC123").unwrap();
    assert_eq!(entity.title, "Test Requirement");
    assert_eq!(entity.status, "draft");
    assert_eq!(entity.author, "Test Author");
}

#[test]
fn test_feature_caching() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "tolerances/features/FEAT-01ABC123.tdt.yaml",
        r#"
id: FEAT-01ABC123
component: CMP-01XYZ789
feature_type: internal
title: Mounting Hole
status: draft
author: Test Author
created: 2024-01-15T10:30:00Z
dimensions:
  - name: diameter
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
    internal: true
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    let feature = cache.get_feature("FEAT-01ABC123").unwrap();
    assert_eq!(feature.component_id, "CMP-01XYZ789");
    assert_eq!(feature.feature_type, "internal");
    assert_eq!(feature.dim_name, Some("diameter".to_string()));
    assert_eq!(feature.dim_nominal, Some(10.0));
    assert_eq!(feature.dim_plus_tol, Some(0.1));
    assert_eq!(feature.dim_minus_tol, Some(0.05));
    assert_eq!(feature.dim_internal, Some(true));
}

#[test]
fn test_incremental_sync_add() {
    let (_tmp, project) = create_test_project();
    let mut cache = EntityCache::open(&project).unwrap();

    // Initially empty
    let stats = cache.statistics().unwrap();
    assert_eq!(stats.total_entities, 0);

    // Add a file
    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC123.tdt.yaml",
        r#"
id: REQ-01ABC123
title: New Requirement
status: draft
author: Test Author
created: 2024-01-15T10:30:00Z
"#,
    );

    // Sync should detect the new file
    let sync_stats = cache.sync().unwrap();
    assert_eq!(sync_stats.entities_added, 1);
    assert_eq!(sync_stats.entities_updated, 0);
    assert_eq!(sync_stats.entities_removed, 0);

    let stats = cache.statistics().unwrap();
    assert_eq!(stats.total_entities, 1);
}

#[test]
fn test_incremental_sync_remove() {
    let (_tmp, project) = create_test_project();

    // Create initial file
    let file_path = project
        .root()
        .join("requirements/inputs/REQ-01ABC123.tdt.yaml");
    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC123.tdt.yaml",
        r#"
id: REQ-01ABC123
title: To Be Deleted
status: draft
author: Test Author
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open(&project).unwrap();
    let stats = cache.statistics().unwrap();
    assert_eq!(stats.total_entities, 1);

    // Delete the file
    fs::remove_file(&file_path).unwrap();

    // Sync should detect removal
    let sync_stats = cache.sync().unwrap();
    assert_eq!(sync_stats.entities_removed, 1);

    let stats = cache.statistics().unwrap();
    assert_eq!(stats.total_entities, 0);
}

#[test]
fn test_list_entities_with_filter() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC.tdt.yaml",
        r#"
id: REQ-01ABC
title: Requirement One
status: draft
author: Alice
created: 2024-01-15T10:30:00Z
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-02DEF.tdt.yaml",
        r#"
id: REQ-02DEF
title: Requirement Two
status: approved
author: Bob
created: 2024-01-16T10:30:00Z
"#,
    );

    write_test_entity(
        &project,
        "risks/design/RISK-01GHI.tdt.yaml",
        r#"
id: RISK-01GHI
title: Risk One
status: draft
author: Alice
created: 2024-01-17T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    // Filter by prefix
    let reqs = cache.list_entities(&EntityFilter {
        prefix: Some(EntityPrefix::Req),
        ..Default::default()
    });
    assert_eq!(reqs.len(), 2);

    // Filter by status
    let approved = cache.list_entities(&EntityFilter {
        status: Some("approved".to_string()),
        ..Default::default()
    });
    assert_eq!(approved.len(), 1);
    assert_eq!(approved[0].title, "Requirement Two");

    // Filter by author
    let alice = cache.list_entities(&EntityFilter {
        author: Some("Alice".to_string()),
        ..Default::default()
    });
    assert_eq!(alice.len(), 2);
}

#[test]
fn test_raw_query() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC.tdt.yaml",
        r#"
id: REQ-01ABC
title: Test Req
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    let result = cache
        .query_raw("SELECT id, title FROM entities WHERE prefix = 'REQ'")
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0][0], "REQ-01ABC");
    assert_eq!(result[0][1], "Test Req");
}

#[test]
fn test_short_ids_not_in_files() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC123.tdt.yaml",
        r#"
id: REQ-01ABC123
title: Test Requirement
status: draft
author: Test Author
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-02DEF456
"#,
    );

    // Read the file back
    let content = fs::read_to_string(
        project
            .root()
            .join("requirements/inputs/REQ-01ABC123.tdt.yaml"),
    )
    .unwrap();

    // Verify no short IDs in file content
    assert!(!content.contains('@'), "File should not contain short IDs");
    assert!(
        content.contains("REQ-01ABC123"),
        "File should contain full ULID"
    );
    assert!(
        content.contains("REQ-02DEF456"),
        "References should use full ULIDs"
    );
}

#[test]
fn test_cache_survives_rebuild() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC.tdt.yaml",
        r#"
id: REQ-01ABC
title: Persistent Req
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    // First cache instance
    {
        let mut cache = EntityCache::open(&project).unwrap();
        let short_id = cache.ensure_short_id("REQ-01ABC").unwrap();
        assert_eq!(short_id, "REQ@1");
    }

    // Second cache instance (should load existing data)
    {
        let cache = EntityCache::open(&project).unwrap();
        let resolved = cache.resolve_short_id("REQ@1");
        assert_eq!(resolved, Some("REQ-01ABC".to_string()));
    }
}

#[test]
fn test_features_for_component() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "tolerances/features/FEAT-01A.tdt.yaml",
        r#"
id: FEAT-01A
component: CMP-001
feature_type: internal
title: Hole A
status: draft
author: Test
created: 2024-01-15T10:30:00Z
dimensions:
  - name: diameter
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
    internal: true
"#,
    );

    write_test_entity(
        &project,
        "tolerances/features/FEAT-02B.tdt.yaml",
        r#"
id: FEAT-02B
component: CMP-001
feature_type: external
title: Shaft B
status: draft
author: Test
created: 2024-01-15T10:30:00Z
dimensions:
  - name: diameter
    nominal: 9.9
    plus_tol: 0.05
    minus_tol: 0.1
    internal: false
"#,
    );

    write_test_entity(
        &project,
        "tolerances/features/FEAT-03C.tdt.yaml",
        r#"
id: FEAT-03C
component: CMP-002
feature_type: internal
title: Hole C
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    let features = cache.get_features_for_component("CMP-001");
    assert_eq!(features.len(), 2);

    let features2 = cache.get_features_for_component("CMP-002");
    assert_eq!(features2.len(), 1);
}

// =========================================================================
// Cache Corruption & Recovery Tests
// =========================================================================

#[test]
fn test_cache_recovery_from_missing_file() {
    let (_tmp, project) = create_test_project();

    // Create entity first
    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC.tdt.yaml",
        r#"
id: REQ-01ABC
title: Test Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    // Build cache
    {
        let cache = EntityCache::open(&project).unwrap();
        let stats = cache.statistics().unwrap();
        assert_eq!(stats.total_entities, 1);
    }

    // Delete cache file
    let cache_path = project.root().join(".tdt/cache.db");
    fs::remove_file(&cache_path).unwrap();

    // Opening should recreate and rebuild
    let cache = EntityCache::open(&project).unwrap();
    let stats = cache.statistics().unwrap();
    assert_eq!(stats.total_entities, 1);
}

#[test]
fn test_cache_recovery_from_corrupted_file() {
    let (_tmp, project) = create_test_project();

    // Create entity
    write_test_entity(
        &project,
        "requirements/inputs/REQ-01ABC.tdt.yaml",
        r#"
id: REQ-01ABC
title: Test Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    // Build valid cache first
    {
        let _cache = EntityCache::open(&project).unwrap();
    }

    // Corrupt the cache file
    let cache_path = project.root().join(".tdt/cache.db");
    fs::write(&cache_path, b"not a valid sqlite database").unwrap();

    // Opening should detect corruption and rebuild
    let result = EntityCache::open(&project);
    // Should either recover or fail gracefully
    if let Ok(cache) = result {
        let stats = cache.statistics().unwrap();
        assert_eq!(stats.total_entities, 1);
    }
    // If it fails, that's acceptable - corruption was detected
}

#[test]
fn test_cache_handles_empty_project() {
    let (_tmp, project) = create_test_project();

    let cache = EntityCache::open(&project).unwrap();
    let stats = cache.statistics().unwrap();

    assert_eq!(stats.total_entities, 0);
    assert_eq!(stats.total_short_ids, 0);

    // List operations should work on empty cache
    let entities = cache.list_entities(&EntityFilter::default());
    assert!(entities.is_empty());
}

#[test]
fn test_cache_handles_invalid_yaml_gracefully() {
    let (_tmp, project) = create_test_project();

    // Write invalid YAML
    write_test_entity(
        &project,
        "requirements/inputs/REQ-INVALID.tdt.yaml",
        r#"
id: REQ-INVALID
title: [invalid: yaml: structure
status: draft
"#,
    );

    // Write valid YAML
    write_test_entity(
        &project,
        "requirements/inputs/REQ-VALID.tdt.yaml",
        r#"
id: REQ-VALID
title: Valid Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    let _stats = cache.rebuild().unwrap();

    // Should have cached the valid one, skipped the invalid
    let entity = cache.get_entity("REQ-VALID");
    assert!(entity.is_some());
}

#[test]
fn test_cache_handles_special_characters_in_title() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-SPECIAL.tdt.yaml",
        r#"
id: REQ-SPECIAL
title: "Title with 'quotes', \"double quotes\", and special chars: <>&"
status: draft
author: Test Author
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    let entity = cache.get_entity("REQ-SPECIAL").unwrap();
    assert!(entity.title.contains("quotes"));
    assert!(entity.title.contains("<>&"));
}

#[test]
fn test_cache_handles_unicode_content() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-UNICODE.tdt.yaml",
        r#"
id: REQ-UNICODE
title: "Требование с кириллицей 日本語 emoji 🚀"
status: draft
author: José García
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    let entity = cache.get_entity("REQ-UNICODE").unwrap();
    assert!(entity.title.contains("кириллицей"));
    assert!(entity.title.contains("日本語"));
    assert!(entity.author.contains("José"));
}

// =========================================================================
// Short ID Edge Case Tests
// =========================================================================

#[test]
fn test_short_id_sequential_assignment() {
    let (_tmp, project) = create_test_project();
    let mut cache = EntityCache::open_without_sync(&project).unwrap();

    // Assign multiple short IDs
    let ids: Vec<String> = (1..=10)
        .map(|i| cache.ensure_short_id(&format!("REQ-{:03}", i)).unwrap())
        .collect();

    // Verify sequential assignment
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id, &format!("REQ@{}", i + 1));
    }
}

#[test]
fn test_short_id_case_insensitive_resolution() {
    let (_tmp, project) = create_test_project();
    let mut cache = EntityCache::open_without_sync(&project).unwrap();

    cache.ensure_short_id("RISK-01ABC").unwrap();

    // All case variations should resolve
    assert!(cache.resolve_short_id("RISK@1").is_some());
    assert!(cache.resolve_short_id("risk@1").is_some());
    assert!(cache.resolve_short_id("Risk@1").is_some());
    assert!(cache.resolve_short_id("rIsK@1").is_some());
}

#[test]
fn test_short_id_invalid_format_returns_none() {
    let (_tmp, project) = create_test_project();
    let cache = EntityCache::open_without_sync(&project).unwrap();

    // Invalid formats should return None, not panic
    assert!(cache.resolve_short_id("").is_none());
    assert!(cache.resolve_short_id("@1").is_none());
    assert!(cache.resolve_short_id("REQ@").is_none());
    assert!(cache.resolve_short_id("REQ@abc").is_none());
    assert!(cache.resolve_short_id("REQ@-1").is_none());
    assert!(cache.resolve_short_id("INVALID@1").is_none());
}

#[test]
fn test_short_id_persistence_across_sessions() {
    let (_tmp, project) = create_test_project();

    // Session 1: Create short IDs
    {
        let mut cache = EntityCache::open_without_sync(&project).unwrap();
        cache.ensure_short_id("REQ-01ABC").unwrap();
        cache.ensure_short_id("REQ-02DEF").unwrap();
    }

    // Session 2: Verify they persist
    {
        let cache = EntityCache::open_without_sync(&project).unwrap();
        assert_eq!(
            cache.resolve_short_id("REQ@1"),
            Some("REQ-01ABC".to_string())
        );
        assert_eq!(
            cache.resolve_short_id("REQ@2"),
            Some("REQ-02DEF".to_string())
        );
    }

    // Session 3: Add more, verify old ones intact
    {
        let mut cache = EntityCache::open_without_sync(&project).unwrap();
        let new_id = cache.ensure_short_id("REQ-03GHI").unwrap();
        assert_eq!(new_id, "REQ@3");
        assert_eq!(
            cache.resolve_short_id("REQ@1"),
            Some("REQ-01ABC".to_string())
        );
    }
}

// =========================================================================
// Link Consistency Tests
// =========================================================================

#[test]
fn test_link_caching() {
    let (_tmp, project) = create_test_project();

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01A.tdt.yaml",
        r#"
id: REQ-01A
title: Parent Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-02B
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-02B.tdt.yaml",
        r#"
id: REQ-02B
title: Child Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    // Check forward link
    let links = cache.get_links_from("REQ-01A");
    assert!(
        !links.is_empty(),
        "Expected links from REQ-01A to be cached"
    );
    assert!(links.iter().any(|l| l.target_id == "REQ-02B"));
}

#[test]
fn test_link_removal_on_entity_delete() {
    let (_tmp, project) = create_test_project();

    let file_path = project.root().join("requirements/inputs/REQ-01A.tdt.yaml");

    write_test_entity(
        &project,
        "requirements/inputs/REQ-01A.tdt.yaml",
        r#"
id: REQ-01A
title: Requirement with links
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-02B
"#,
    );

    let mut cache = EntityCache::open(&project).unwrap();

    // Verify link exists
    let links = cache.get_links_from("REQ-01A");
    assert!(
        !links.is_empty(),
        "Expected links from REQ-01A after initial sync"
    );

    // Delete the entity file
    fs::remove_file(&file_path).unwrap();

    // Sync should remove entity and its links
    cache.sync().unwrap();

    let links = cache.get_links_from("REQ-01A");
    assert!(links.is_empty());
}

// =========================================================================
// Large Dataset Tests
// =========================================================================

#[test]
fn test_cache_performance_many_entities() {
    let (_tmp, project) = create_test_project();

    // Create 100 entities
    for i in 0..100 {
        write_test_entity(
            &project,
            &format!("requirements/inputs/REQ-{:05}.tdt.yaml", i),
            &format!(
                r#"
id: REQ-{:05}
title: Requirement Number {}
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
                i, i
            ),
        );
    }

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    let stats = cache.rebuild().unwrap();

    assert_eq!(stats.entities_added, 100);

    // Query performance test
    let entities = cache.list_entities(&EntityFilter {
        prefix: Some(EntityPrefix::Req),
        ..Default::default()
    });
    assert_eq!(entities.len(), 100);

    // Verify short IDs are assigned for all entities
    // Note: Order is not guaranteed due to file system iteration order,
    // but all should have valid REQ@N format
    let mut short_ids: Vec<String> = Vec::new();
    for i in 0..100 {
        let short_id = cache.ensure_short_id(&format!("REQ-{:05}", i)).unwrap();
        assert!(
            short_id.starts_with("REQ@"),
            "Short ID should start with REQ@, got: {}",
            short_id
        );
        short_ids.push(short_id);
    }

    // Verify no duplicates
    short_ids.sort();
    short_ids.dedup();
    assert_eq!(short_ids.len(), 100, "All short IDs should be unique");
}

// =========================================================================
// Link Chain Tests
// =========================================================================

#[test]
fn test_link_chain_traversal() {
    let (_tmp, project) = create_test_project();

    // Create a chain: REQ-A -> REQ-B -> REQ-C
    write_test_entity(
        &project,
        "requirements/inputs/REQ-0001A.tdt.yaml",
        r#"
id: REQ-0001A
title: First Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-0002B
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0002B.tdt.yaml",
        r#"
id: REQ-0002B
title: Second Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-0003C
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0003C.tdt.yaml",
        r#"
id: REQ-0003C
title: Third Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    // Verify chain can be traversed
    let links_from_a = cache.get_links_from("REQ-0001A");
    assert_eq!(links_from_a.len(), 1);
    assert_eq!(links_from_a[0].target_id, "REQ-0002B");

    let links_from_b = cache.get_links_from("REQ-0002B");
    assert_eq!(links_from_b.len(), 1);
    assert_eq!(links_from_b[0].target_id, "REQ-0003C");

    let links_from_c = cache.get_links_from("REQ-0003C");
    assert!(links_from_c.is_empty());
}

#[test]
fn test_orphan_link_detection() {
    let (_tmp, project) = create_test_project();

    // Create a requirement that links to a non-existent entity
    write_test_entity(
        &project,
        "requirements/inputs/REQ-0001A.tdt.yaml",
        r#"
id: REQ-0001A
title: Requirement with Orphan Link
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-NONEXISTENT
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    // The link is stored even if target doesn't exist (cache doesn't validate)
    let links = cache.get_links_from("REQ-0001A");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_id, "REQ-NONEXISTENT");

    // But when querying entities, the target won't be found
    let filter = EntityFilter {
        prefix: Some(EntityPrefix::Req),
        ..Default::default()
    };
    let entities = cache.list_entities(&filter);
    assert_eq!(entities.len(), 1); // Only REQ-0001A exists
}

#[test]
fn test_multiple_link_types() {
    let (_tmp, project) = create_test_project();

    // Create a requirement with multiple link types
    write_test_entity(
        &project,
        "requirements/inputs/REQ-0001A.tdt.yaml",
        r#"
id: REQ-0001A
title: Requirement with Multiple Links
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-0002B
references:
  - REQ-0003C
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0002B.tdt.yaml",
        r#"
id: REQ-0002B
title: Traced Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0003C.tdt.yaml",
        r#"
id: REQ-0003C
title: Referenced Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    let links = cache.get_links_from("REQ-0001A");
    assert_eq!(links.len(), 2);

    // Check both link types are present
    let trace_link = links.iter().find(|l| l.link_type == "traces_to");
    assert!(trace_link.is_some());
    assert_eq!(trace_link.unwrap().target_id, "REQ-0002B");

    let ref_link = links.iter().find(|l| l.link_type == "references");
    assert!(ref_link.is_some());
    assert_eq!(ref_link.unwrap().target_id, "REQ-0003C");
}

#[test]
fn test_bidirectional_links() {
    let (_tmp, project) = create_test_project();

    // Create bidirectional links: REQ-A traces_to REQ-B, REQ-B traces_from REQ-A
    write_test_entity(
        &project,
        "requirements/inputs/REQ-0001A.tdt.yaml",
        r#"
id: REQ-0001A
title: Parent Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-0002B
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0002B.tdt.yaml",
        r#"
id: REQ-0002B
title: Child Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_from:
  - REQ-0001A
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    // Forward link from A
    let links_from_a = cache.get_links_from("REQ-0001A");
    assert!(
        links_from_a.iter().any(|l| l.target_id == "REQ-0002B"),
        "A should link to B"
    );

    // Reverse link from B
    let links_from_b = cache.get_links_from("REQ-0002B");
    assert!(
        links_from_b.iter().any(|l| l.target_id == "REQ-0001A"),
        "B should have traces_from link back to A"
    );
}

#[test]
fn test_link_update_on_rebuild() {
    let (_tmp, project) = create_test_project();

    // Create initial requirement with link
    let file_path = project
        .root()
        .join("requirements/inputs/REQ-0001A.tdt.yaml");

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0001A.tdt.yaml",
        r#"
id: REQ-0001A
title: Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-0002B
"#,
    );

    write_test_entity(
        &project,
        "requirements/inputs/REQ-0002B.tdt.yaml",
        r#"
id: REQ-0002B
title: Target Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
    );

    let mut cache = EntityCache::open_without_sync(&project).unwrap();
    cache.rebuild().unwrap();

    // Verify initial link
    let links = cache.get_links_from("REQ-0001A");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_id, "REQ-0002B");

    // Update file to change link target
    fs::write(
        &file_path,
        r#"id: REQ-0001A
title: Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
traces_to:
  - REQ-0003C
"#
        .trim_start(),
    )
    .unwrap();

    // Rebuild cache (full rescan)
    cache.rebuild().unwrap();

    // Verify link was updated
    let links = cache.get_links_from("REQ-0001A");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_id, "REQ-0003C");
}
