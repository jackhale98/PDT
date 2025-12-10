//! Template generation for new entities

use chrono::{DateTime, Utc};
use rust_embed::Embed;
use tera::Tera;
use thiserror::Error;

use crate::core::identity::EntityId;

#[derive(Embed)]
#[folder = "templates/"]
struct EmbeddedTemplates;

/// Context for template generation
#[derive(Debug, Clone)]
pub struct TemplateContext {
    pub id: EntityId,
    pub author: String,
    pub created: DateTime<Utc>,
    pub title: Option<String>,
    pub req_type: Option<String>,
    pub priority: Option<String>,
    pub category: Option<String>,
}

impl TemplateContext {
    pub fn new(id: EntityId, author: String) -> Self {
        Self {
            id,
            author,
            created: Utc::now(),
            title: None,
            req_type: None,
            priority: None,
            category: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_req_type(mut self, req_type: impl Into<String>) -> Self {
        self.req_type = Some(req_type.into());
        self
    }

    pub fn with_priority(mut self, priority: impl Into<String>) -> Self {
        self.priority = Some(priority.into());
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// Template generator using Tera
pub struct TemplateGenerator {
    tera: Tera,
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Template not found: {0}")]
    NotFound(String),

    #[error("Template rendering error: {0}")]
    RenderError(String),
}

impl TemplateGenerator {
    /// Create a new template generator with embedded templates
    pub fn new() -> Result<Self, TemplateError> {
        let mut tera = Tera::default();

        // Load embedded templates
        for file in EmbeddedTemplates::iter() {
            let filename = file.as_ref();
            if let Some(content) = EmbeddedTemplates::get(filename) {
                if let Ok(template_str) = std::str::from_utf8(&content.data) {
                    tera.add_raw_template(filename, template_str)
                        .map_err(|e| TemplateError::RenderError(e.to_string()))?;
                }
            }
        }

        Ok(Self { tera })
    }

    /// Generate a requirement template
    pub fn generate_requirement(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("created_date", &ctx.created.format("%Y-%m-%d").to_string());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("req_type", &ctx.req_type.clone().unwrap_or_else(|| "input".to_string()));
        context.insert("priority", &ctx.priority.clone().unwrap_or_else(|| "medium".to_string()));
        context.insert("category", &ctx.category.clone().unwrap_or_default());

        // Try to use embedded template, fall back to hardcoded
        if self.tera.get_template_names().any(|n| n == "requirement.yaml.tera") {
            self.tera
                .render("requirement.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            // Hardcoded fallback template
            Ok(self.hardcoded_requirement_template(&ctx))
        }
    }

    fn hardcoded_requirement_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let req_type = ctx.req_type.clone().unwrap_or_else(|| "input".to_string());
        let priority = ctx.priority.clone().unwrap_or_else(|| "medium".to_string());
        let category = ctx.category.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();
        let created_date = ctx.created.format("%Y-%m-%d");

        format!(
            r#"# Requirement: {title}
# Created by PDT

id: {id}
type: {req_type}
title: "{title}"

source:
  document: ""
  revision: ""
  section: ""
  date: {created_date}

category: "{category}"
tags: []

text: |
  # Enter requirement text here
  # Use clear, testable language (shall, must, will)

rationale: ""

acceptance_criteria:
  - ""

priority: {priority}
status: draft

links:
  satisfied_by: []
  verified_by: []

# Auto-managed metadata
created: {created}
modified: {created}
author: {author}
revision: 1
"#,
            id = ctx.id,
            title = title,
            req_type = req_type,
            priority = priority,
            category = category,
            created = created,
            created_date = created_date,
            author = ctx.author,
        )
    }
}

impl Default for TemplateGenerator {
    fn default() -> Self {
        Self::new().expect("Failed to create template generator")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EntityPrefix;

    #[test]
    fn test_template_generates_valid_yaml() {
        let generator = TemplateGenerator::new().unwrap();
        let ctx = TemplateContext::new(
            EntityId::new(EntityPrefix::Req),
            "test".to_string(),
        )
        .with_title("Test Requirement")
        .with_req_type("input")
        .with_priority("high");

        let yaml = generator.generate_requirement(&ctx).unwrap();

        // Should be valid YAML
        let parsed: serde_yml::Value = serde_yml::from_str(&yaml).unwrap();
        assert!(parsed.get("id").is_some());
        assert!(parsed.get("title").is_some());
        assert_eq!(parsed.get("type").unwrap().as_str(), Some("input"));
        assert_eq!(parsed.get("priority").unwrap().as_str(), Some("high"));
    }
}
