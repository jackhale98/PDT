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
    pub risk_type: Option<String>,
    pub priority: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    // FMEA fields for RISK
    pub severity: Option<u8>,
    pub occurrence: Option<u8>,
    pub detection: Option<u8>,
    pub risk_level: Option<String>,
    // TEST fields
    pub test_type: Option<String>,
    pub test_level: Option<String>,
    pub test_method: Option<String>,
    pub estimated_duration: Option<String>,
    // RSLT fields
    pub test_id: Option<EntityId>,
    pub verdict: Option<String>,
    pub executed_by: Option<String>,
    pub executed_date: Option<DateTime<Utc>>,
    pub duration: Option<String>,
    // CMP fields
    pub part_number: Option<String>,
    pub part_revision: Option<String>,
    pub make_buy: Option<String>,
    pub component_category: Option<String>,
    pub material: Option<String>,
    // FEAT fields
    pub component_id: Option<String>,
    pub feature_type: Option<String>,
    // MATE fields
    pub feature_a: Option<String>,
    pub feature_b: Option<String>,
    pub mate_type: Option<String>,
    // TOL (Stackup) fields
    pub target_name: Option<String>,
    pub target_nominal: Option<f64>,
    pub target_upper: Option<f64>,
    pub target_lower: Option<f64>,
    // QUOT fields
    pub supplier: Option<String>,
    // SUP fields
    pub short_name: Option<String>,
    pub website: Option<String>,
    pub payment_terms: Option<String>,
    pub notes: Option<String>,
}

impl TemplateContext {
    pub fn new(id: EntityId, author: String) -> Self {
        Self {
            id,
            author,
            created: Utc::now(),
            title: None,
            req_type: None,
            risk_type: None,
            priority: None,
            category: None,
            tags: Vec::new(),
            severity: None,
            occurrence: None,
            detection: None,
            risk_level: None,
            test_type: None,
            test_level: None,
            test_method: None,
            estimated_duration: None,
            test_id: None,
            verdict: None,
            executed_by: None,
            executed_date: None,
            duration: None,
            part_number: None,
            part_revision: None,
            make_buy: None,
            component_category: None,
            material: None,
            component_id: None,
            feature_type: None,
            feature_a: None,
            feature_b: None,
            mate_type: None,
            target_name: None,
            target_nominal: None,
            target_upper: None,
            target_lower: None,
            supplier: None,
            short_name: None,
            website: None,
            payment_terms: None,
            notes: None,
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

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_risk_type(mut self, risk_type: impl Into<String>) -> Self {
        self.risk_type = Some(risk_type.into());
        self
    }

    pub fn with_severity(mut self, severity: u8) -> Self {
        self.severity = Some(severity);
        self
    }

    pub fn with_occurrence(mut self, occurrence: u8) -> Self {
        self.occurrence = Some(occurrence);
        self
    }

    pub fn with_detection(mut self, detection: u8) -> Self {
        self.detection = Some(detection);
        self
    }

    pub fn with_risk_level(mut self, risk_level: impl Into<String>) -> Self {
        self.risk_level = Some(risk_level.into());
        self
    }

    pub fn with_test_type(mut self, test_type: impl Into<String>) -> Self {
        self.test_type = Some(test_type.into());
        self
    }

    pub fn with_test_level(mut self, test_level: impl Into<String>) -> Self {
        self.test_level = Some(test_level.into());
        self
    }

    pub fn with_test_method(mut self, test_method: impl Into<String>) -> Self {
        self.test_method = Some(test_method.into());
        self
    }

    pub fn with_estimated_duration(mut self, duration: impl Into<String>) -> Self {
        self.estimated_duration = Some(duration.into());
        self
    }

    pub fn with_test_id(mut self, test_id: EntityId) -> Self {
        self.test_id = Some(test_id);
        self
    }

    pub fn with_verdict(mut self, verdict: impl Into<String>) -> Self {
        self.verdict = Some(verdict.into());
        self
    }

    pub fn with_executed_by(mut self, executed_by: impl Into<String>) -> Self {
        self.executed_by = Some(executed_by.into());
        self
    }

    pub fn with_executed_date(mut self, date: DateTime<Utc>) -> Self {
        self.executed_date = Some(date);
        self
    }

    pub fn with_duration(mut self, duration: impl Into<String>) -> Self {
        self.duration = Some(duration.into());
        self
    }

    pub fn with_part_number(mut self, part_number: impl Into<String>) -> Self {
        self.part_number = Some(part_number.into());
        self
    }

    pub fn with_part_revision(mut self, revision: impl Into<String>) -> Self {
        self.part_revision = Some(revision.into());
        self
    }

    pub fn with_make_buy(mut self, make_buy: impl Into<String>) -> Self {
        self.make_buy = Some(make_buy.into());
        self
    }

    pub fn with_component_category(mut self, category: impl Into<String>) -> Self {
        self.component_category = Some(category.into());
        self
    }

    pub fn with_material(mut self, material: impl Into<String>) -> Self {
        self.material = Some(material.into());
        self
    }

    pub fn with_component_id(mut self, component_id: impl Into<String>) -> Self {
        self.component_id = Some(component_id.into());
        self
    }

    pub fn with_feature_type(mut self, feature_type: impl Into<String>) -> Self {
        self.feature_type = Some(feature_type.into());
        self
    }

    pub fn with_feature_a(mut self, feature_a: impl Into<String>) -> Self {
        self.feature_a = Some(feature_a.into());
        self
    }

    pub fn with_feature_b(mut self, feature_b: impl Into<String>) -> Self {
        self.feature_b = Some(feature_b.into());
        self
    }

    pub fn with_mate_type(mut self, mate_type: impl Into<String>) -> Self {
        self.mate_type = Some(mate_type.into());
        self
    }

    pub fn with_target(mut self, name: impl Into<String>, nominal: f64, upper: f64, lower: f64) -> Self {
        self.target_name = Some(name.into());
        self.target_nominal = Some(nominal);
        self.target_upper = Some(upper);
        self.target_lower = Some(lower);
        self
    }

    pub fn with_supplier(mut self, supplier: impl Into<String>) -> Self {
        self.supplier = Some(supplier.into());
        self
    }

    pub fn with_short_name(mut self, short_name: impl Into<String>) -> Self {
        self.short_name = Some(short_name.into());
        self
    }

    pub fn with_website(mut self, website: impl Into<String>) -> Self {
        self.website = Some(website.into());
        self
    }

    pub fn with_payment_terms(mut self, payment_terms: impl Into<String>) -> Self {
        self.payment_terms = Some(payment_terms.into());
        self
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
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

    /// Generate a test template
    pub fn generate_test(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("test_type", &ctx.test_type.clone().unwrap_or_else(|| "verification".to_string()));
        context.insert("test_level", &ctx.test_level.clone().unwrap_or_else(|| "system".to_string()));
        context.insert("test_method", &ctx.test_method.clone().unwrap_or_else(|| "test".to_string()));
        context.insert("priority", &ctx.priority.clone().unwrap_or_else(|| "medium".to_string()));
        context.insert("category", &ctx.category.clone().unwrap_or_default());
        context.insert("estimated_duration", &ctx.estimated_duration.clone().unwrap_or_else(|| "1 hour".to_string()));

        if self.tera.get_template_names().any(|n| n == "test.yaml.tera") {
            self.tera
                .render("test.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_test_template(ctx))
        }
    }

    /// Generate a result template
    pub fn generate_result(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("test_id", &ctx.test_id.as_ref().map(|id| id.to_string()).unwrap_or_default());
        context.insert("verdict", &ctx.verdict.clone().unwrap_or_else(|| "pass".to_string()));
        context.insert("executed_by", &ctx.executed_by.clone().unwrap_or_else(|| ctx.author.clone()));
        context.insert("executed_date", &ctx.executed_date.unwrap_or(ctx.created).to_rfc3339());
        context.insert("category", &ctx.category.clone().unwrap_or_default());
        context.insert("duration", &ctx.duration.clone().unwrap_or_default());

        if self.tera.get_template_names().any(|n| n == "rslt.yaml.tera") {
            self.tera
                .render("rslt.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_result_template(ctx))
        }
    }

    /// Generate a risk template
    pub fn generate_risk(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("created_date", &ctx.created.format("%Y-%m-%d").to_string());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("risk_type", &ctx.risk_type.clone().unwrap_or_else(|| "design".to_string()));
        context.insert("category", &ctx.category.clone().unwrap_or_default());
        context.insert("severity", &ctx.severity.unwrap_or(5));
        context.insert("occurrence", &ctx.occurrence.unwrap_or(5));
        context.insert("detection", &ctx.detection.unwrap_or(5));
        let s = ctx.severity.unwrap_or(5) as u16;
        let o = ctx.occurrence.unwrap_or(5) as u16;
        let d = ctx.detection.unwrap_or(5) as u16;
        context.insert("rpn", &(s * o * d));
        context.insert("risk_level", &ctx.risk_level.clone().unwrap_or_else(|| "medium".to_string()));

        // Try to use embedded template, fall back to hardcoded
        if self.tera.get_template_names().any(|n| n == "risk.yaml.tera") {
            self.tera
                .render("risk.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            // Hardcoded fallback template
            Ok(self.hardcoded_risk_template(ctx))
        }
    }

    /// Generate a component template
    pub fn generate_component(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("part_number", &ctx.part_number.clone().unwrap_or_default());
        context.insert("part_revision", &ctx.part_revision.clone().unwrap_or_default());
        context.insert("make_buy", &ctx.make_buy.clone().unwrap_or_else(|| "buy".to_string()));
        context.insert("category", &ctx.component_category.clone().unwrap_or_else(|| "mechanical".to_string()));
        context.insert("material", &ctx.material.clone().unwrap_or_default());

        if self.tera.get_template_names().any(|n| n == "component.yaml.tera") {
            self.tera
                .render("component.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_component_template(ctx))
        }
    }

    /// Generate an assembly template
    pub fn generate_assembly(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("part_number", &ctx.part_number.clone().unwrap_or_default());
        context.insert("part_revision", &ctx.part_revision.clone().unwrap_or_default());

        if self.tera.get_template_names().any(|n| n == "assembly.yaml.tera") {
            self.tera
                .render("assembly.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_assembly_template(ctx))
        }
    }

    fn hardcoded_assembly_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let part_number = ctx.part_number.clone().unwrap_or_default();
        let part_revision = ctx.part_revision.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Assembly: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
part_number: "{part_number}"
revision: "{part_revision}"
title: "{title}"

description: |
  # Detailed description of this assembly
  # Include key specifications and assembly requirements

# Bill of Materials
bom:
  - component_id: ""
    quantity: 1
    reference_designators: []
    notes: ""

# Sub-assembly references (ASM-... IDs)
subassemblies: []

# Associated documents
documents:
  - type: drawing
    path: ""
    revision: ""

tags: []
status: draft

links:
  related_to: []
  parent: null

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            title = title,
            part_number = part_number,
            part_revision = part_revision,
            created = created,
            author = ctx.author,
        )
    }

    /// Generate a feature template
    pub fn generate_feature(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("component_id", &ctx.component_id.clone().unwrap_or_default());
        context.insert("feature_type", &ctx.feature_type.clone().unwrap_or_else(|| "hole".to_string()));

        if self.tera.get_template_names().any(|n| n == "feature.yaml.tera") {
            self.tera
                .render("feature.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_feature_template(ctx))
        }
    }

    fn hardcoded_feature_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let component_id = ctx.component_id.clone().unwrap_or_default();
        let feature_type = ctx.feature_type.clone().unwrap_or_else(|| "hole".to_string());
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Feature: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
component: {component_id}
feature_type: {feature_type}
title: "{title}"

description: |
  # Detailed description of this feature
  # Include key dimensional requirements

# Dimensions with tolerances
# Uses plus_tol/minus_tol format (not +/- symbol)
# Distribution: normal (default), uniform, or triangular
dimensions:
  - name: "diameter"
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
    units: "mm"
    distribution: normal

# GD&T controls (optional)
gdt: []

# Drawing reference
drawing:
  number: ""
  revision: ""
  zone: ""

tags: []
status: draft

links:
  used_in_mates: []
  used_in_stackups: []

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            title = title,
            component_id = component_id,
            feature_type = feature_type,
            created = created,
            author = ctx.author,
        )
    }

    /// Generate a mate template
    pub fn generate_mate(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("feature_a", &ctx.feature_a.clone().unwrap_or_default());
        context.insert("feature_b", &ctx.feature_b.clone().unwrap_or_default());
        context.insert("mate_type", &ctx.mate_type.clone().unwrap_or_else(|| "clearance_fit".to_string()));

        if self.tera.get_template_names().any(|n| n == "mate.yaml.tera") {
            self.tera
                .render("mate.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_mate_template(ctx))
        }
    }

    fn hardcoded_mate_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let feature_a = ctx.feature_a.clone().unwrap_or_default();
        let feature_b = ctx.feature_b.clone().unwrap_or_default();
        let mate_type = ctx.mate_type.clone().unwrap_or_else(|| "clearance_fit".to_string());
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Mate: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
title: "{title}"

description: |
  # Detailed description of this mate
  # Describe the contact and fit requirements

# Features being mated (both REQUIRED)
feature_a: {feature_a}   # Typically hole/bore
feature_b: {feature_b}   # Typically shaft/pin

mate_type: {mate_type}

# Fit analysis (auto-calculated when features have dimensions)
# Run 'pdt mate recalc MATE@N' to update
fit_analysis: null

notes: |
  # Additional assembly or fit notes

tags: []
status: draft

links:
  used_in_stackups: []
  verifies: []

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            title = title,
            feature_a = feature_a,
            feature_b = feature_b,
            mate_type = mate_type,
            created = created,
            author = ctx.author,
        )
    }

    /// Generate a stackup template
    pub fn generate_stackup(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("target_name", &ctx.target_name.clone().unwrap_or_else(|| "Gap".to_string()));
        context.insert("target_nominal", &ctx.target_nominal.unwrap_or(1.0));
        context.insert("target_upper", &ctx.target_upper.unwrap_or(1.5));
        context.insert("target_lower", &ctx.target_lower.unwrap_or(0.5));

        if self.tera.get_template_names().any(|n| n == "stackup.yaml.tera") {
            self.tera
                .render("stackup.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_stackup_template(ctx))
        }
    }

    fn hardcoded_stackup_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let target_name = ctx.target_name.clone().unwrap_or_else(|| "Gap".to_string());
        let target_nominal = ctx.target_nominal.unwrap_or(1.0);
        let target_upper = ctx.target_upper.unwrap_or(1.5);
        let target_lower = ctx.target_lower.unwrap_or(0.5);
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Stackup: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
title: "{title}"

description: |
  # Detailed description of this tolerance stackup
  # Include the tolerance chain being analyzed

# Target specification
target:
  name: "{target_name}"
  nominal: {target_nominal}
  upper_limit: {target_upper}
  lower_limit: {target_lower}
  units: "mm"
  critical: false

# Contributors to the stackup
# Use plus_tol/minus_tol format (not +/- symbol)
contributors:
  - name: "Part A Length"
    feature_id: null
    direction: positive
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
    distribution: normal
    source: "DWG-001 Rev A"

# Analysis results (auto-calculated)
# Run 'pdt tol analyze TOL@N' to calculate
analysis_results:
  worst_case: null
  rss: null
  monte_carlo: null

disposition: under_review

tags: []
status: draft

links:
  verifies: []
  mates_used: []

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            title = title,
            target_name = target_name,
            target_nominal = target_nominal,
            target_upper = target_upper,
            target_lower = target_lower,
            created = created,
            author = ctx.author,
        )
    }

    fn hardcoded_component_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let part_number = ctx.part_number.clone().unwrap_or_default();
        let part_revision = ctx.part_revision.clone().unwrap_or_default();
        let make_buy = ctx.make_buy.clone().unwrap_or_else(|| "buy".to_string());
        let category = ctx.component_category.clone().unwrap_or_else(|| "mechanical".to_string());
        let material = ctx.material.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Component: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
part_number: "{part_number}"
revision: "{part_revision}"
title: "{title}"

description: |
  # Detailed description of this component
  # Include key specifications and requirements

make_buy: {make_buy}
category: {category}

# Physical properties
material: "{material}"
mass_kg: null
unit_cost: null

# Supplier information
suppliers:
  - name: ""
    supplier_pn: ""
    lead_time_days: null
    moq: null
    unit_cost: null

# Associated documents
documents:
  - type: drawing
    path: ""
    revision: ""

tags: []
status: draft

links:
  related_to: []
  used_in: []

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            title = title,
            part_number = part_number,
            part_revision = part_revision,
            make_buy = make_buy,
            category = category,
            material = material,
            created = created,
            author = ctx.author,
        )
    }

    fn hardcoded_risk_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let risk_type = ctx.risk_type.clone().unwrap_or_else(|| "design".to_string());
        let category = ctx.category.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();
        let severity = ctx.severity.unwrap_or(5);
        let occurrence = ctx.occurrence.unwrap_or(5);
        let detection = ctx.detection.unwrap_or(5);
        let rpn = severity as u16 * occurrence as u16 * detection as u16;
        let risk_level = ctx.risk_level.clone().unwrap_or_else(|| "medium".to_string());

        format!(
            r#"# Risk: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
type: {risk_type}
title: "{title}"

category: "{category}"
tags: []

description: |
  # Describe the risk scenario here
  # What could go wrong? Under what conditions?

# FMEA Fields (Failure Mode and Effects Analysis)
failure_mode: |
  # How does this failure manifest?

cause: |
  # What is the root cause or mechanism?

effect: |
  # What is the impact or consequence?

# Risk Assessment (1-10 scale)
severity: {severity}
occurrence: {occurrence}
detection: {detection}
rpn: {rpn}

mitigations:
  - action: ""
    type: prevention
    status: proposed
    owner: ""

status: draft
risk_level: {risk_level}

links:
  related_to: []
  mitigated_by: []
  verified_by: []

# Auto-managed metadata
created: {created}
author: {author}
revision: 1
"#,
            id = ctx.id,
            title = title,
            risk_type = risk_type,
            category = category,
            severity = severity,
            occurrence = occurrence,
            detection = detection,
            rpn = rpn,
            risk_level = risk_level,
            created = created,
            author = ctx.author,
        )
    }

    fn hardcoded_requirement_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let req_type = ctx.req_type.clone().unwrap_or_else(|| "input".to_string());
        let priority = ctx.priority.clone().unwrap_or_else(|| "medium".to_string());
        let category = ctx.category.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();
        let created_date = ctx.created.format("%Y-%m-%d");
        let tags = if ctx.tags.is_empty() {
            "[]".to_string()
        } else {
            format!("[{}]", ctx.tags.join(", "))
        };

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
tags: {tags}

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
author: {author}
revision: 1
"#,
            id = ctx.id,
            title = title,
            req_type = req_type,
            priority = priority,
            category = category,
            tags = tags,
            created = created,
            created_date = created_date,
            author = ctx.author,
        )
    }

    fn hardcoded_test_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let test_type = ctx.test_type.clone().unwrap_or_else(|| "verification".to_string());
        let test_level = ctx.test_level.clone().unwrap_or_else(|| "system".to_string());
        let test_method = ctx.test_method.clone().unwrap_or_else(|| "test".to_string());
        let priority = ctx.priority.clone().unwrap_or_else(|| "medium".to_string());
        let category = ctx.category.clone().unwrap_or_default();
        let estimated_duration = ctx.estimated_duration.clone().unwrap_or_else(|| "1 hour".to_string());
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Test: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
type: {test_type}
test_level: {test_level}
test_method: {test_method}
title: "{title}"

category: "{category}"
tags: []

objective: |
  # What does this test verify or validate?
  # Be specific about success criteria

description: |
  # Detailed description of the test
  # Include any background or context

preconditions:
  - "Unit under test is at room temperature"
  - "All required equipment is calibrated"

equipment:
  - name: ""
    specification: ""
    calibration_required: false

procedure:
  - step: 1
    action: |
      # What to do
    expected: |
      # What should happen
    acceptance: |
      # Pass/fail criteria

acceptance_criteria:
  - "All steps pass"

environment:
  temperature: "23 ± 2°C"
  humidity: "50 ± 10% RH"
  other: ""

estimated_duration: "{estimated_duration}"

priority: {priority}
status: draft

links:
  verifies: []
  validates: []
  mitigates: []
  depends_on: []

# Auto-managed metadata (do not edit manually)
created: {created}
author: {author}
revision: 1
"#,
            id = ctx.id,
            title = title,
            test_type = test_type,
            test_level = test_level,
            test_method = test_method,
            priority = priority,
            category = category,
            estimated_duration = estimated_duration,
            created = created,
            author = ctx.author,
        )
    }

    fn hardcoded_result_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let test_id = ctx.test_id.as_ref().map(|id| id.to_string()).unwrap_or_default();
        let verdict = ctx.verdict.clone().unwrap_or_else(|| "pass".to_string());
        let executed_by = ctx.executed_by.clone().unwrap_or_else(|| ctx.author.clone());
        let executed_date = ctx.executed_date.unwrap_or(ctx.created).to_rfc3339();
        let category = ctx.category.clone().unwrap_or_default();
        let duration = ctx.duration.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Result: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
test_id: {test_id}
test_revision: 1
title: "{title}"

verdict: {verdict}
verdict_rationale: |
  # Explain the verdict
  # Especially important for fail or conditional results

category: "{category}"
tags: []

# Execution information
executed_date: {executed_date}
executed_by: {executed_by}

# Sample identification
sample_info:
  sample_id: ""
  serial_number: ""
  lot_number: ""
  configuration: ""

# Actual test environment
environment:
  temperature: ""
  humidity: ""
  location: ""
  other: ""

# Equipment used (with calibration info)
equipment_used:
  - name: ""
    asset_id: ""
    calibration_date: ""
    calibration_due: ""

# Results for each procedure step
step_results:
  - step: 1
    result: pass
    observed: |
      # What was actually observed
    notes: ""

deviations: []

failures: []

attachments: []

duration: "{duration}"
notes: |
  # General observations and notes

status: draft

links:
  test: {test_id}
  actions: []

# Auto-managed metadata (do not edit manually)
created: {created}
author: {author}
revision: 1
"#,
            id = ctx.id,
            title = title,
            test_id = test_id,
            verdict = verdict,
            executed_by = executed_by,
            executed_date = executed_date,
            category = category,
            duration = duration,
            created = created,
            author = ctx.author,
        )
    }

    /// Generate a quote template
    pub fn generate_quote(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("title", &ctx.title.clone().unwrap_or_default());
        context.insert("component_id", &ctx.component_id.clone().unwrap_or_default());
        context.insert("supplier", &ctx.supplier.clone().unwrap_or_default());

        if self.tera.get_template_names().any(|n| n == "quote.yaml.tera") {
            self.tera
                .render("quote.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_quote_template(ctx))
        }
    }

    fn hardcoded_quote_template(&self, ctx: &TemplateContext) -> String {
        let title = ctx.title.clone().unwrap_or_default();
        let component_id = ctx.component_id.clone().unwrap_or_default();
        let supplier = ctx.supplier.clone().unwrap_or_default();
        let created = ctx.created.to_rfc3339();

        format!(
            r#"# Quote: {title}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
title: "{title}"

# Supplier ID (SUP@N or full ID) - REQUIRED
supplier: {supplier}

# Component this quote is for (use --component or --assembly, not both)
component: {component_id}
# assembly: null

# Supplier's quote reference number
# quote_ref: ""

description: |
  # Notes about this quote
  # Include any special terms or conditions

# Currency for all prices
currency: USD

# Price breaks (quantity-based pricing)
price_breaks:
  - min_qty: 1
    unit_price: 0.00
    lead_time_days: 14

# Order constraints
moq: null
lead_time_days: 14

# One-time costs
tooling_cost: null
nre_costs: []

# Validity
quote_date: null
valid_until: null

# Quote-specific status
quote_status: pending

tags: []
status: draft

links:
  related_quotes: []

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            title = title,
            component_id = component_id,
            supplier = supplier,
            created = created,
            author = ctx.author,
        )
    }

    /// Generate a supplier template
    pub fn generate_supplier(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        let mut context = tera::Context::new();
        context.insert("id", &ctx.id.to_string());
        context.insert("author", &ctx.author);
        context.insert("created", &ctx.created.to_rfc3339());
        context.insert("name", &ctx.title.clone().unwrap_or_default());
        context.insert("short_name", &ctx.short_name.clone().unwrap_or_default());
        context.insert("website", &ctx.website.clone().unwrap_or_default());
        context.insert("payment_terms", &ctx.payment_terms.clone().unwrap_or_default());
        context.insert("notes", &ctx.notes.clone().unwrap_or_default());

        if self.tera.get_template_names().any(|n| n == "supplier.yaml.tera") {
            self.tera
                .render("supplier.yaml.tera", &context)
                .map_err(|e| TemplateError::RenderError(e.to_string()))
        } else {
            Ok(self.hardcoded_supplier_template(ctx))
        }
    }

    fn hardcoded_supplier_template(&self, ctx: &TemplateContext) -> String {
        let name = ctx.title.clone().unwrap_or_default();
        let short_name = ctx.short_name.clone();
        let website = ctx.website.clone();
        let payment_terms = ctx.payment_terms.clone();
        let notes = ctx.notes.clone();
        let created = ctx.created.to_rfc3339();

        let short_name_line = short_name
            .map(|s| format!("short_name: \"{}\"\n", s))
            .unwrap_or_default();
        let website_line = website
            .map(|w| format!("website: \"{}\"\n", w))
            .unwrap_or_default();
        let payment_terms_line = payment_terms
            .map(|t| format!("payment_terms: \"{}\"\n", t))
            .unwrap_or_default();
        let notes_line = notes
            .map(|n| format!("notes: |\n  {}\n", n))
            .unwrap_or_default();

        format!(
            r#"# Supplier: {name}
# Created by PDT - Plain-text Product Development Toolkit

id: {id}
name: "{name}"
{short_name_line}{website_line}
# Contact information
contacts: []
# Example:
#   - name: "John Smith"
#     role: "Sales"
#     email: "john@example.com"
#     phone: "+1-555-0100"
#     primary: true

# Physical addresses
addresses: []
# Example:
#   - type: headquarters
#     street: "123 Main St"
#     city: "San Francisco"
#     state: "CA"
#     postal: "94102"
#     country: "USA"

# Payment and currency
{payment_terms_line}currency: USD

# Quality certifications
certifications: []
# Example:
#   - name: "ISO 9001:2015"
#     expiry: 2025-12-31
#     certificate_number: "CERT-12345"

# Manufacturing capabilities
capabilities: []
# Options: machining, sheet_metal, casting, injection, extrusion, pcb,
#          pcb_assembly, cable_assembly, assembly, testing, finishing, packaging

{notes_line}tags: []
status: draft

links:
  approved_for: []

# Auto-managed metadata
created: {created}
author: {author}
entity_revision: 1
"#,
            id = ctx.id,
            name = name,
            short_name_line = short_name_line,
            website_line = website_line,
            payment_terms_line = payment_terms_line,
            notes_line = notes_line,
            created = created,
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
