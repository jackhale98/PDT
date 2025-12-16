//! Schema-driven interactive wizard for entity creation
//!
//! This module provides a generic wizard that can create any entity type
//! by reading its JSON Schema and prompting the user for values.

use console::style;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use miette::{IntoDiagnostic, Result};
use serde_json::Value;
use std::collections::HashMap;

use crate::core::identity::EntityPrefix;
use crate::schema::registry::SchemaRegistry;

/// A schema-driven wizard for creating entities
pub struct SchemaWizard {
    registry: SchemaRegistry,
    theme: ColorfulTheme,
}

/// Result of running the wizard - collected field values
#[derive(Debug, Default)]
pub struct WizardResult {
    pub values: HashMap<String, Value>,
}

impl WizardResult {
    /// Get a string value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }
}

/// Field information extracted from schema
#[derive(Debug)]
struct FieldInfo {
    name: String,
    description: Option<String>,
    field_type: FieldType,
    required: bool,
    default: Option<Value>,
}

#[derive(Debug)]
#[allow(dead_code)]
enum FieldType {
    String {
        min_length: Option<u64>,
        max_length: Option<u64>,
    },
    Enum {
        values: Vec<String>,
    },
    Integer {
        minimum: Option<i64>,
        maximum: Option<i64>,
    },
    Number {
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    Boolean,
    Array {
        item_type: Box<FieldType>,
    },
    Skip, // For fields we handle automatically (id, created, author, etc.)
}

impl SchemaWizard {
    /// Create a new wizard with the default schema registry
    pub fn new() -> Self {
        Self {
            registry: SchemaRegistry::default(),
            theme: ColorfulTheme::default(),
        }
    }

    /// Run the wizard for a specific entity type
    pub fn run(&self, prefix: EntityPrefix) -> Result<WizardResult> {
        let schema_str = self.registry.get(prefix).ok_or_else(|| {
            miette::miette!("No schema found for entity type: {}", prefix.as_str())
        })?;
        let schema: Value = serde_json::from_str(schema_str).into_diagnostic()?;

        println!();
        println!(
            "{} Creating new {} entity",
            style("◆").cyan(),
            style(prefix.as_str()).bold()
        );
        println!("{}", style("─".repeat(50)).dim());
        println!();

        // Extract field information from schema
        let fields = self.extract_fields(&schema)?;

        // Collect values for each field
        let mut result = WizardResult::default();

        for field in fields {
            if matches!(field.field_type, FieldType::Skip) {
                continue;
            }

            let value = self.prompt_field(&field)?;
            if let Some(v) = value {
                result.values.insert(field.name, v);
            }
        }

        println!();
        println!("{} Values collected!", style("✓").green());

        Ok(result)
    }

    /// Extract field information from a JSON Schema
    fn extract_fields(&self, schema: &Value) -> Result<Vec<FieldInfo>> {
        let mut fields = Vec::new();

        let properties = schema.get("properties").and_then(|p| p.as_object());
        let required = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if let Some(props) = properties {
            // Define the order we want to prompt fields for each entity type
            // This covers requirements, risks, tests, results, and components
            let field_order = [
                // Common fields
                "type",
                "test_type",
                "test_level",
                "test_method",
                "title",
                "priority",
                "category",
                "tags",
                // Component fields
                "part_number",
                "revision",
                "make_buy",
                "material",
                "mass_kg",
                "unit_cost",
                // Requirement fields
                "text",
                "rationale",
                "acceptance_criteria",
                // Risk FMEA fields (in logical order)
                "description",
                "failure_mode",
                "cause",
                "effect",
                "severity",
                "occurrence",
                "detection",
                // Test fields
                "objective",
                "preconditions",
                "equipment",
                "procedure",
                "sample_size",
                "environment",
                "estimated_duration",
                // Result fields
                "test_id",
                "verdict",
                "verdict_rationale",
                "executed_date",
                "executed_by",
                "sample_info",
                "equipment_used",
                "step_results",
                "deviations",
                "failures",
                "attachments",
                "duration",
                "notes",
            ];

            // Fields to skip entirely (auto-managed, calculated, or complex nested structures)
            let skip_fields = [
                // Auto-managed fields
                "id",
                "created",
                "author",
                "entity_revision",
                "source",
                "links",
                "status",
                // Calculated fields
                "rpn",
                "risk_level",
                "initial_risk",
                "mitigations",
                // Test result fields handled separately
                "test_revision",
                "reviewed_by",
                "reviewed_date",
                // Feature fields - complex nested objects
                "component",
                "dimensions",
                "gdt",
                "drawing",
                // Complex array-of-object fields (wizard can't handle these)
                "suppliers",
                "documents",
                "procedure",
                "preconditions",
                "equipment",
                "step_results",
                "deviations",
                "failures",
                "attachments",
                "containment",
                "affected_items",
                "defect",
                "disposition",
                "cost_impact",
                "detection",
                "safety",
                "tools_required",
                "materials_required",
                "quality_checks",
                "price_breaks",
                "actions",
                "root_cause_analysis",
                "effectiveness",
                "contributors",
                "stackup",
                "fit_analysis",
                "sample_info",
                "equipment_used",
            ];

            // First add fields in preferred order
            for name in &field_order {
                if skip_fields.contains(name) {
                    continue;
                }
                if let Some(prop_schema) = props.get(*name) {
                    if let Some(field) =
                        self.parse_field(*name, prop_schema, required.contains(&name.to_string()))
                    {
                        fields.push(field);
                    }
                }
            }

            // Then add any remaining fields not in our order (excluding skip fields)
            // Collect and sort remaining fields for consistent ordering
            let mut remaining: Vec<_> = props
                .iter()
                .filter(|(name, _)| {
                    !field_order.contains(&name.as_str()) && !skip_fields.contains(&name.as_str())
                })
                .collect();
            remaining.sort_by(|(a, _), (b, _)| a.cmp(b));

            for (name, prop_schema) in remaining {
                if let Some(field) = self.parse_field(name, prop_schema, required.contains(name)) {
                    fields.push(field);
                }
            }
        }

        Ok(fields)
    }

    /// Parse a single field from its schema
    fn parse_field(&self, name: &str, schema: &Value, required: bool) -> Option<FieldInfo> {
        // Skip auto-managed fields (but NOT part revision - only entity_revision)
        let auto_fields = ["id", "created", "author", "entity_revision"];
        if auto_fields.contains(&name) {
            return Some(FieldInfo {
                name: name.to_string(),
                description: None,
                field_type: FieldType::Skip,
                required,
                default: None,
            });
        }

        let description = schema
            .get("description")
            .and_then(|d| d.as_str())
            .map(String::from);
        let default = schema.get("default").cloned();

        let field_type = if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
            let values: Vec<String> = enum_values
                .iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect();
            FieldType::Enum { values }
        } else {
            // Handle both simple types ("string") and union types (["string", "null"])
            let type_value = schema.get("type");
            let primary_type = type_value
                .and_then(|t| {
                    // If it's a string, use it directly
                    if let Some(s) = t.as_str() {
                        Some(s.to_string())
                    } else if let Some(arr) = t.as_array() {
                        // If it's an array like ["string", "null"], get the first non-null type
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .find(|s| *s != "null")
                            .map(String::from)
                    } else {
                        None
                    }
                });

            match primary_type.as_deref() {
                Some("string") => FieldType::String {
                    min_length: schema.get("minLength").and_then(|v| v.as_u64()),
                    max_length: schema.get("maxLength").and_then(|v| v.as_u64()),
                },
                Some("integer") => FieldType::Integer {
                    minimum: schema.get("minimum").and_then(|v| v.as_i64()),
                    maximum: schema.get("maximum").and_then(|v| v.as_i64()),
                },
                Some("number") => FieldType::Number {
                    minimum: schema.get("minimum").and_then(|v| v.as_f64()),
                    maximum: schema.get("maximum").and_then(|v| v.as_f64()),
                },
                Some("boolean") => FieldType::Boolean,
                Some("array") => {
                    let item_schema = schema.get("items").unwrap_or(&Value::Null);
                    // Skip arrays of objects (too complex for wizard)
                    if item_schema.get("type").and_then(|t| t.as_str()) == Some("object") {
                        return None;
                    }
                    let item_type = if let Some(item_enum) =
                        item_schema.get("enum").and_then(|e| e.as_array())
                    {
                        FieldType::Enum {
                            values: item_enum
                                .iter()
                                .filter_map(|v| v.as_str())
                                .map(String::from)
                                .collect(),
                        }
                    } else {
                        FieldType::String {
                            min_length: None,
                            max_length: None,
                        }
                    };
                    FieldType::Array {
                        item_type: Box::new(item_type),
                    }
                }
                _ => return None, // Skip complex types we don't handle
            }
        };

        Some(FieldInfo {
            name: name.to_string(),
            description,
            field_type,
            required,
            default,
        })
    }

    /// Prompt the user for a field value
    fn prompt_field(&self, field: &FieldInfo) -> Result<Option<Value>> {
        let prompt = self.format_prompt(field);

        match &field.field_type {
            FieldType::Skip => Ok(None),

            FieldType::Enum { values } => {
                let default_idx = field
                    .default
                    .as_ref()
                    .and_then(|d| d.as_str())
                    .and_then(|d| values.iter().position(|v| v == d))
                    .unwrap_or(0);

                let selection = Select::with_theme(&self.theme)
                    .with_prompt(&prompt)
                    .items(values)
                    .default(default_idx)
                    .interact()
                    .into_diagnostic()?;

                Ok(Some(Value::String(values[selection].clone())))
            }

            FieldType::String { .. } => {
                let default_str = field
                    .default
                    .as_ref()
                    .and_then(|d| d.as_str())
                    .unwrap_or("");

                let value: String = if !default_str.is_empty() {
                    Input::with_theme(&self.theme)
                        .with_prompt(&prompt)
                        .default(default_str.to_string())
                        .allow_empty(!field.required)
                        .interact_text()
                        .into_diagnostic()?
                } else if !field.required {
                    Input::with_theme(&self.theme)
                        .with_prompt(&prompt)
                        .allow_empty(true)
                        .interact_text()
                        .into_diagnostic()?
                } else {
                    Input::with_theme(&self.theme)
                        .with_prompt(&prompt)
                        .interact_text()
                        .into_diagnostic()?
                };

                if value.is_empty() && !field.required {
                    Ok(None)
                } else {
                    Ok(Some(Value::String(value)))
                }
            }

            FieldType::Integer { .. } => {
                let default_val = field.default.as_ref().and_then(|d| d.as_i64()).unwrap_or(0);

                let value: String = Input::with_theme(&self.theme)
                    .with_prompt(&prompt)
                    .default(default_val.to_string())
                    .allow_empty(!field.required)
                    .interact_text()
                    .into_diagnostic()?;

                if value.is_empty() && !field.required {
                    Ok(None)
                } else {
                    let parsed: i64 = value.parse().unwrap_or(default_val);
                    Ok(Some(Value::Number(parsed.into())))
                }
            }

            FieldType::Number { .. } => {
                let default_val = field.default.as_ref().and_then(|d| d.as_f64()).unwrap_or(0.0);

                let value: String = Input::with_theme(&self.theme)
                    .with_prompt(&prompt)
                    .default(if default_val == 0.0 {
                        String::new()
                    } else {
                        default_val.to_string()
                    })
                    .allow_empty(!field.required)
                    .interact_text()
                    .into_diagnostic()?;

                if value.is_empty() && !field.required {
                    Ok(None)
                } else {
                    let parsed: f64 = value.parse().unwrap_or(default_val);
                    Ok(Some(Value::Number(
                        serde_json::Number::from_f64(parsed).unwrap_or_else(|| 0.into()),
                    )))
                }
            }

            FieldType::Boolean => {
                let default_val = field
                    .default
                    .as_ref()
                    .and_then(|d| d.as_bool())
                    .unwrap_or(false);

                let items = &["Yes", "No"];
                let default_idx = if default_val { 0 } else { 1 };

                let selection = Select::with_theme(&self.theme)
                    .with_prompt(&prompt)
                    .items(items)
                    .default(default_idx)
                    .interact()
                    .into_diagnostic()?;

                Ok(Some(Value::Bool(selection == 0)))
            }

            FieldType::Array { .. } => {
                // For arrays, we prompt for comma-separated values
                let value: String = Input::with_theme(&self.theme)
                    .with_prompt(&format!("{} (comma-separated)", prompt))
                    .allow_empty(true)
                    .interact_text()
                    .into_diagnostic()?;

                if value.is_empty() {
                    Ok(None)
                } else {
                    let items: Vec<Value> = value
                        .split(',')
                        .map(|s| Value::String(s.trim().to_string()))
                        .collect();
                    Ok(Some(Value::Array(items)))
                }
            }
        }
    }

    /// Format the prompt for a field
    fn format_prompt(&self, field: &FieldInfo) -> String {
        let name = field.name.replace('_', " ");
        let name = name
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if let Some(ref desc) = field.description {
            // Truncate long descriptions
            let short_desc = if desc.len() > 50 {
                format!("{}...", &desc[..47])
            } else {
                desc.clone()
            };
            format!("{} ({})", name, style(short_desc).dim())
        } else {
            name
        }
    }
}

impl Default for SchemaWizard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_creation() {
        let wizard = SchemaWizard::new();
        // Just verify it can be created and has the REQ schema
        assert!(wizard.registry.has_schema(EntityPrefix::Req));
    }
}
