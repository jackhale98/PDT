//! Schema registry - embedded JSON schemas

use rust_embed::Embed;
use std::collections::HashMap;

use crate::core::EntityPrefix;

#[derive(Embed)]
#[folder = "schemas/"]
struct EmbeddedSchemas;

/// Registry of JSON schemas for entity validation
pub struct SchemaRegistry {
    schemas: HashMap<EntityPrefix, String>,
}

impl SchemaRegistry {
    /// Create a new schema registry with embedded schemas
    pub fn new() -> Self {
        let mut schemas = HashMap::new();

        // Load embedded schemas
        for prefix in EntityPrefix::all() {
            let filename = format!("{}.schema.json", prefix.as_str().to_lowercase());
            if let Some(file) = EmbeddedSchemas::get(&filename) {
                if let Ok(content) = std::str::from_utf8(&file.data) {
                    schemas.insert(*prefix, content.to_string());
                }
            }
        }

        Self { schemas }
    }

    /// Get the JSON schema for an entity type
    pub fn get(&self, prefix: EntityPrefix) -> Option<&str> {
        self.schemas.get(&prefix).map(|s| s.as_str())
    }

    /// Check if a schema exists for the given prefix
    pub fn has_schema(&self, prefix: EntityPrefix) -> bool {
        self.schemas.contains_key(&prefix)
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
