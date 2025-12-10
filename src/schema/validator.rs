//! Schema validation with detailed error reporting

use jsonschema::{validator_for, ValidationError as JsonSchemaError, Validator as JsonValidator};
use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use thiserror::Error;

use crate::core::EntityPrefix;
use crate::schema::registry::SchemaRegistry;

/// Validation error with source location information
#[derive(Debug, Error, Diagnostic)]
#[error("Schema validation failed: {summary}")]
#[diagnostic(code(pdt::schema::validation_error))]
pub struct ValidationError {
    summary: String,

    #[source_code]
    src: NamedSource<String>,

    #[related]
    violations: Vec<SchemaViolation>,
}

/// A single schema violation
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct SchemaViolation {
    #[label("{}", self.hint)]
    span: SourceSpan,

    message: String,
    hint: String,

    #[help]
    help: Option<String>,
}

impl SchemaViolation {
    pub fn new(message: String, hint: String, span: SourceSpan, help: Option<String>) -> Self {
        Self {
            span,
            message,
            hint,
            help,
        }
    }
}

impl ValidationError {
    pub fn new(filename: &str, source: &str, violations: Vec<SchemaViolation>) -> Self {
        let count = violations.len();
        let summary = if count == 1 {
            "1 error".to_string()
        } else {
            format!("{} errors", count)
        };
        Self {
            summary,
            src: NamedSource::new(filename, source.to_string()),
            violations,
        }
    }

    /// Get the number of violations
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

/// Result of validation
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }
}

/// A validation issue (error or warning)
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Schema validator with compiled schemas
pub struct Validator {
    /// Compiled JSON schemas by entity prefix
    compiled: HashMap<EntityPrefix, JsonValidator>,
}

impl Validator {
    /// Create a new validator with schemas from the registry
    pub fn new(registry: &SchemaRegistry) -> Self {
        let mut compiled = HashMap::new();

        for prefix in EntityPrefix::all() {
            if let Some(schema_str) = registry.get(*prefix) {
                if let Ok(schema_json) = serde_json::from_str::<JsonValue>(schema_str) {
                    if let Ok(compiled_schema) = validator_for(&schema_json) {
                        compiled.insert(*prefix, compiled_schema);
                    }
                }
            }
        }

        Self { compiled }
    }

    /// Validate YAML content against the schema for the given entity type
    pub fn validate(
        &self,
        content: &str,
        filename: &str,
        prefix: EntityPrefix,
    ) -> Result<ValidationResult, ValidationError> {
        // First parse YAML to JSON value
        let yaml_value: serde_yml::Value = match serde_yml::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                // YAML parse error - convert to validation error
                let span = find_error_span(content, e.location());
                let violation = SchemaViolation::new(
                    format!("YAML parse error: {}", e),
                    "invalid YAML".to_string(),
                    span,
                    Some("Check YAML syntax - proper indentation, colons, quotes".to_string()),
                );
                return Err(ValidationError::new(filename, content, vec![violation]));
            }
        };

        // Convert YAML value to JSON value for schema validation
        let json_value: JsonValue = match serde_json::to_value(&yaml_value) {
            Ok(v) => v,
            Err(e) => {
                let violation = SchemaViolation::new(
                    format!("Failed to convert YAML to JSON: {}", e),
                    "conversion error".to_string(),
                    (0, content.len()).into(),
                    None,
                );
                return Err(ValidationError::new(filename, content, vec![violation]));
            }
        };

        // Get compiled schema
        let schema = match self.compiled.get(&prefix) {
            Some(s) => s,
            None => {
                // No schema available - validation passes (schema optional)
                return Ok(ValidationResult::success());
            }
        };

        // Validate against schema
        let validation_result = schema.validate(&json_value);
        if let Err(error) = validation_result {
            let violation = error_to_violation(content, &error);
            Err(ValidationError::new(filename, content, vec![violation]))
        } else {
            Ok(ValidationResult::success())
        }
    }

    /// Iterate over all errors (useful for detailed error reporting)
    pub fn iter_errors<'a>(
        &'a self,
        content: &str,
        filename: &str,
        prefix: EntityPrefix,
    ) -> Result<ValidationResult, ValidationError> {
        // First parse YAML to JSON value
        let yaml_value: serde_yml::Value = match serde_yml::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                let span = find_error_span(content, e.location());
                let violation = SchemaViolation::new(
                    format!("YAML parse error: {}", e),
                    "invalid YAML".to_string(),
                    span,
                    Some("Check YAML syntax - proper indentation, colons, quotes".to_string()),
                );
                return Err(ValidationError::new(filename, content, vec![violation]));
            }
        };

        // Convert YAML value to JSON value for schema validation
        let json_value: JsonValue = match serde_json::to_value(&yaml_value) {
            Ok(v) => v,
            Err(e) => {
                let violation = SchemaViolation::new(
                    format!("Failed to convert YAML to JSON: {}", e),
                    "conversion error".to_string(),
                    (0, content.len()).into(),
                    None,
                );
                return Err(ValidationError::new(filename, content, vec![violation]));
            }
        };

        // Get compiled schema
        let schema = match self.compiled.get(&prefix) {
            Some(s) => s,
            None => {
                return Ok(ValidationResult::success());
            }
        };

        // Collect all errors
        let violations: Vec<SchemaViolation> = schema
            .iter_errors(&json_value)
            .map(|e| error_to_violation(content, &e))
            .collect();

        if violations.is_empty() {
            Ok(ValidationResult::success())
        } else {
            Err(ValidationError::new(filename, content, violations))
        }
    }

    /// Validate a file directly
    pub fn validate_file(
        &self,
        path: &std::path::Path,
    ) -> Result<ValidationResult, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        // Determine entity type from filename
        let prefix = EntityPrefix::from_filename(&filename)
            .or_else(|| EntityPrefix::from_path(path));

        match prefix {
            Some(p) => self
                .iter_errors(&content, &filename, p)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
            None => Ok(ValidationResult::success()), // Unknown entity type - skip
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        let registry = SchemaRegistry::default();
        Self::new(&registry)
    }
}

/// Convert a JSON Schema validation error to our violation format
fn error_to_violation(content: &str, error: &JsonSchemaError) -> SchemaViolation {
    let path = error.instance_path.to_string();
    let message = format_schema_error(error);
    let hint = format_error_hint(error);
    let help = generate_help_message(error);

    // Try to find the span in the YAML where this error occurred
    let span = find_path_span(content, &path);

    SchemaViolation::new(message, hint, span, help)
}

/// Format a JSON Schema error into a user-friendly message
fn format_schema_error(error: &JsonSchemaError) -> String {
    let path = if error.instance_path.as_str().is_empty() {
        "document root".to_string()
    } else {
        format!("'{}'", error.instance_path)
    };

    match &error.kind {
        jsonschema::error::ValidationErrorKind::Required { property } => {
            let prop_str = property.as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| property.to_string());
            format!("Missing required field: {} at {}", prop_str, path)
        }
        jsonschema::error::ValidationErrorKind::Type { kind } => {
            format!("Wrong type at {}: expected {:?}", path, kind)
        }
        jsonschema::error::ValidationErrorKind::Enum { options } => {
            let opts = format_enum_options(options);
            format!(
                "Invalid value at {}: must be one of: {}",
                path, opts
            )
        }
        jsonschema::error::ValidationErrorKind::Pattern { pattern } => {
            format!("Value at {} doesn't match pattern: {}", path, pattern)
        }
        jsonschema::error::ValidationErrorKind::MinLength { limit } => {
            format!("Value at {} is too short: minimum {} characters", path, limit)
        }
        jsonschema::error::ValidationErrorKind::MaxLength { limit } => {
            format!("Value at {} is too long: maximum {} characters", path, limit)
        }
        jsonschema::error::ValidationErrorKind::Minimum { limit } => {
            format!("Value at {} is too small: minimum {}", path, limit)
        }
        jsonschema::error::ValidationErrorKind::Maximum { limit } => {
            format!("Value at {} is too large: maximum {}", path, limit)
        }
        jsonschema::error::ValidationErrorKind::AdditionalProperties { unexpected } => {
            format!(
                "Unknown field(s) at {}: {}",
                path,
                unexpected.join(", ")
            )
        }
        _ => {
            format!("Validation error at {}: {}", path, error)
        }
    }
}

/// Format enum options as a string
fn format_enum_options(options: &JsonValue) -> String {
    if let Some(arr) = options.as_array() {
        arr.iter()
            .map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        options.to_string()
    }
}

/// Generate a short hint for the error label
fn format_error_hint(error: &JsonSchemaError) -> String {
    match &error.kind {
        jsonschema::error::ValidationErrorKind::Required { .. } => "required field missing".to_string(),
        jsonschema::error::ValidationErrorKind::Type { .. } => "wrong type".to_string(),
        jsonschema::error::ValidationErrorKind::Enum { .. } => "invalid value".to_string(),
        jsonschema::error::ValidationErrorKind::Pattern { .. } => "pattern mismatch".to_string(),
        jsonschema::error::ValidationErrorKind::MinLength { .. } => "too short".to_string(),
        jsonschema::error::ValidationErrorKind::MaxLength { .. } => "too long".to_string(),
        jsonschema::error::ValidationErrorKind::AdditionalProperties { .. } => "unknown field".to_string(),
        _ => "validation error".to_string(),
    }
}

/// Generate a help message with suggestions for fixing the error
fn generate_help_message(error: &JsonSchemaError) -> Option<String> {
    match &error.kind {
        jsonschema::error::ValidationErrorKind::Required { property } => {
            let prop_str = property.as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| property.to_string());
            Some(format!("Add the '{}' field to your file", prop_str))
        }
        jsonschema::error::ValidationErrorKind::Enum { options } => {
            let opts = format_enum_options(options);
            Some(format!("Valid values: {}", opts))
        }
        jsonschema::error::ValidationErrorKind::Pattern { pattern } => {
            // Special case for ID pattern
            if pattern.contains("REQ-") {
                Some("ID format: REQ-[26 alphanumeric characters], e.g., REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD".to_string())
            } else {
                None
            }
        }
        jsonschema::error::ValidationErrorKind::Type { kind } => {
            Some(format!("Expected value of type: {:?}", kind))
        }
        jsonschema::error::ValidationErrorKind::AdditionalProperties { unexpected } => {
            if unexpected.len() == 1 {
                Some(format!("Remove the '{}' field or check spelling", unexpected[0]))
            } else {
                Some("Remove unknown fields or check spelling".to_string())
            }
        }
        _ => None,
    }
}

/// Find the span (byte offset, length) for an error location
fn find_error_span(content: &str, location: Option<serde_yml::Location>) -> SourceSpan {
    if let Some(loc) = location {
        let line = loc.line().saturating_sub(1);
        let column = loc.column().saturating_sub(1);

        // Calculate byte offset
        let mut offset = 0;
        for (i, line_content) in content.lines().enumerate() {
            if i == line {
                offset += column;
                break;
            }
            offset += line_content.len() + 1; // +1 for newline
        }

        // Find a reasonable span length (rest of line or some characters)
        let rest_of_content = &content[offset.min(content.len())..];
        let len = rest_of_content
            .find('\n')
            .unwrap_or(rest_of_content.len())
            .max(1);

        (offset, len).into()
    } else {
        // No location - highlight first line
        let len = content.find('\n').unwrap_or(content.len()).max(1);
        (0, len).into()
    }
}

/// Find the span for a JSON path in YAML content
fn find_path_span(content: &str, json_path: &str) -> SourceSpan {
    // Parse the path (e.g., "/status" or "/links/satisfied_by/0")
    let parts: Vec<&str> = json_path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        // Root path - highlight first line
        let len = content.find('\n').unwrap_or(content.len()).max(1);
        return (0, len).into();
    }

    // Look for the last path component in the YAML
    let search_key = parts.last().unwrap_or(&"");

    // Handle array indices
    if search_key.parse::<usize>().is_ok() {
        // It's an array index - search for parent key
        if parts.len() >= 2 {
            let parent_key = parts[parts.len() - 2];
            if let Some(span) = find_key_span(content, parent_key) {
                return span;
            }
        }
    }

    // Search for the key in the YAML
    if let Some(span) = find_key_span(content, search_key) {
        return span;
    }

    // Fallback - highlight first line
    let len = content.find('\n').unwrap_or(content.len()).max(1);
    (0, len).into()
}

/// Find the span of a key in YAML content
fn find_key_span(content: &str, key: &str) -> Option<SourceSpan> {
    // Simple search for "key:" at the start of a line (with optional leading whitespace)
    let search_pattern = format!("{}:", key);

    let mut offset = 0;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&search_pattern) {
            // Found it - calculate the offset within the line
            let key_start = offset + (line.len() - trimmed.len());
            let key_len = line.len() - (line.len() - trimmed.len());
            return Some((key_start, key_len).into());
        }
        offset += line.len() + 1; // +1 for newline
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);
        assert!(validator.compiled.contains_key(&EntityPrefix::Req));
    }

    #[test]
    fn test_valid_requirement() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
title: "Test Requirement"
text: |
  The system shall do something.
status: draft
priority: medium
created: 2024-01-01T00:00:00Z
author: Test
revision: 1
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Req);
        assert!(result.is_ok(), "Valid requirement should pass: {:?}", result);
    }

    #[test]
    fn test_missing_required_field() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
# missing: title, text, status, priority, etc.
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Req);
        assert!(result.is_err(), "Missing required fields should fail");
        let err = result.unwrap_err();
        assert!(err.violation_count() > 0);
    }

    #[test]
    fn test_invalid_enum_value() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
title: "Test Requirement"
text: "The system shall work."
status: pending
priority: medium
created: 2024-01-01T00:00:00Z
modified: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Req);
        assert!(result.is_err(), "Invalid enum value should fail");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("status") || err.violations.iter().any(|v| v.message.contains("status")),
            "Error should mention status field"
        );
    }

    #[test]
    fn test_invalid_id_pattern() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: REQ-invalid
type: input
title: "Test Requirement"
text: "The system shall work."
status: draft
priority: medium
created: 2024-01-01T00:00:00Z
modified: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Req);
        assert!(result.is_err(), "Invalid ID pattern should fail");
    }

    #[test]
    fn test_unknown_field() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
title: "Test Requirement"
text: "The system shall work."
status: draft
priority: medium
created: 2024-01-01T00:00:00Z
modified: 2024-01-01T00:00:00Z
author: Test
unknown_field: "This should not be here"
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Req);
        assert!(result.is_err(), "Unknown field should fail");
    }

    #[test]
    fn test_find_key_span() {
        let content = "id: REQ-123\ntitle: \"Test\"\nstatus: draft\n";
        let span = find_key_span(content, "status");
        assert!(span.is_some());
        let span = span.unwrap();
        let offset: usize = span.offset();
        assert!(offset > 0);
    }

    #[test]
    fn test_valid_risk() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: design
title: "Test Risk"
description: |
  A test risk for validation.
status: draft
created: 2024-01-01T00:00:00Z
author: Test
revision: 1
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_ok(), "Valid risk should pass: {:?}", result);
    }

    #[test]
    fn test_risk_missing_required_fields() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: design
# missing: title, description, status, created, author
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_err(), "Missing required fields should fail");
        let err = result.unwrap_err();
        assert!(err.violation_count() > 0);
    }

    #[test]
    fn test_risk_invalid_type() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: safety
title: "Test Risk"
description: "A test risk."
status: draft
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_err(), "Invalid type value should fail");
    }

    #[test]
    fn test_risk_with_fmea_fields() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: design
title: "Test Risk"
description: "A test risk."
failure_mode: "Component fails to operate"
cause: "Manufacturing defect"
effect: "System shutdown"
severity: 8
occurrence: 3
detection: 5
rpn: 120
status: draft
risk_level: medium
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_ok(), "Risk with FMEA fields should pass: {:?}", result);
    }

    #[test]
    fn test_risk_severity_out_of_range() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: design
title: "Test Risk"
description: "A test risk."
severity: 15
status: draft
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_err(), "Severity > 10 should fail");
    }

    #[test]
    fn test_risk_with_mitigations() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: process
title: "Test Risk"
description: "A test risk."
mitigations:
  - action: "Add safety interlock"
    type: prevention
    status: proposed
    owner: "John Doe"
  - action: "Add monitoring alarm"
    type: detection
    status: completed
status: draft
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_ok(), "Risk with mitigations should pass: {:?}", result);
    }

    #[test]
    fn test_risk_invalid_id_pattern() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RISK-invalid
type: design
title: "Test Risk"
description: "A test risk."
status: draft
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Risk);
        assert!(result.is_err(), "Invalid RISK ID pattern should fail");
    }

    // ========================================================================
    // TEST Schema Tests
    // ========================================================================

    #[test]
    fn test_valid_test() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: verification
title: "Temperature Validation Test"
objective: |
  Verify the device operates within specified temperature range.
status: draft
created: 2024-01-01T00:00:00Z
author: Test Engineer
revision: 1
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Test);
        assert!(result.is_ok(), "Valid test should pass: {:?}", result);
    }

    #[test]
    fn test_test_missing_required_fields() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: verification
# missing: title, objective, status, created, author
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Test);
        assert!(result.is_err(), "Missing required fields should fail");
        let err = result.unwrap_err();
        assert!(err.violation_count() > 0);
    }

    #[test]
    fn test_test_invalid_type() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: unit
title: "Test"
objective: "Test objective"
status: draft
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Test);
        assert!(result.is_err(), "Invalid type value should fail");
    }

    #[test]
    fn test_test_with_procedure() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: verification
test_level: system
test_method: test
title: "Thermal Test"
objective: "Verify thermal performance"
preconditions:
  - "Unit at room temperature"
  - "All sensors connected"
equipment:
  - name: "Thermal Chamber"
    specification: "-40C to +85C range"
    calibration_required: true
procedure:
  - step: 1
    action: "Place unit in thermal chamber"
    expected: "Unit fits in chamber"
    acceptance: "Unit placed without damage"
  - step: 2
    action: "Set chamber to -20C"
    expected: "Chamber reaches setpoint within 30 minutes"
    acceptance: "Temperature stable at -20C ±2C"
acceptance_criteria:
  - "All procedure steps pass"
  - "Unit powers on at temperature extremes"
status: draft
created: 2024-01-01T00:00:00Z
author: Test Engineer
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Test);
        assert!(result.is_ok(), "Test with procedure should pass: {:?}", result);
    }

    #[test]
    fn test_test_with_links() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: verification
title: "Requirement Verification Test"
objective: "Verify requirement REQ-01 is satisfied"
links:
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTE
  mitigates:
    - RISK-01HC2JB7SMQX7RS1Y0GFKBHPTF
status: approved
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Test);
        assert!(result.is_ok(), "Test with links should pass: {:?}", result);
    }

    #[test]
    fn test_test_invalid_id_pattern() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: TEST-invalid
type: verification
title: "Test"
objective: "Test objective"
status: draft
created: 2024-01-01T00:00:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Test);
        assert!(result.is_err(), "Invalid TEST ID pattern should fail");
    }

    // ========================================================================
    // RSLT Schema Tests
    // ========================================================================

    #[test]
    fn test_valid_result() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTD
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTE
verdict: pass
executed_date: 2024-01-15T10:30:00Z
executed_by: "Test Engineer"
status: draft
created: 2024-01-15T10:30:00Z
author: Test Engineer
revision: 1
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_ok(), "Valid result should pass: {:?}", result);
    }

    #[test]
    fn test_result_missing_required_fields() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTD
# missing: test_id, verdict, executed_date, executed_by, status, created, author
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_err(), "Missing required fields should fail");
        let err = result.unwrap_err();
        assert!(err.violation_count() > 0);
    }

    #[test]
    fn test_result_invalid_verdict() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTD
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTE
verdict: success
executed_date: 2024-01-15T10:30:00Z
executed_by: "Test Engineer"
status: draft
created: 2024-01-15T10:30:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_err(), "Invalid verdict value should fail");
    }

    #[test]
    fn test_result_with_step_results() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTD
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTE
verdict: pass
executed_date: 2024-01-15T10:30:00Z
executed_by: "Test Engineer"
step_results:
  - step: 1
    result: pass
    observed: "Unit placed in chamber successfully"
  - step: 2
    result: pass
    observed: "Chamber reached -20C in 25 minutes"
    measurement:
      value: -20.1
      unit: "°C"
      min: -22
      max: -18
status: approved
created: 2024-01-15T10:30:00Z
author: Test Engineer
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_ok(), "Result with step results should pass: {:?}", result);
    }

    #[test]
    fn test_result_with_failures() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTD
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTE
verdict: fail
verdict_rationale: "Unit failed to power on at low temperature"
executed_date: 2024-01-15T10:30:00Z
executed_by: "Test Engineer"
failures:
  - description: "Unit did not power on at -20C"
    step: 3
    root_cause: "Battery unable to deliver sufficient current at low temperature"
    corrective_action: "Evaluate battery specification or add heater"
status: draft
created: 2024-01-15T10:30:00Z
author: Test Engineer
"#;

        let result = validator.validate(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_ok(), "Result with failures should pass: {:?}", result);
    }

    #[test]
    fn test_result_invalid_test_id_pattern() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTD
test_id: TEST-invalid
verdict: pass
executed_date: 2024-01-15T10:30:00Z
executed_by: "Test Engineer"
status: draft
created: 2024-01-15T10:30:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_err(), "Invalid test_id pattern should fail");
    }

    #[test]
    fn test_result_invalid_id_pattern() {
        let registry = SchemaRegistry::default();
        let validator = Validator::new(&registry);

        let yaml = r#"
id: RSLT-invalid
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTE
verdict: pass
executed_date: 2024-01-15T10:30:00Z
executed_by: "Test Engineer"
status: draft
created: 2024-01-15T10:30:00Z
author: Test
"#;

        let result = validator.iter_errors(yaml, "test.pdt.yaml", EntityPrefix::Rslt);
        assert!(result.is_err(), "Invalid RSLT ID pattern should fail");
    }
}
