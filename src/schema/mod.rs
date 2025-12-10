//! Schema system - validation and template generation

pub mod registry;
pub mod template;
pub mod validator;

pub use registry::SchemaRegistry;
pub use template::{TemplateContext, TemplateGenerator};
pub use validator::{ValidationError, Validator};
