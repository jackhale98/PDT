//! YAML parsing and error handling

pub mod diagnostics;
pub mod parser;

pub use diagnostics::{YamlError, YamlSyntaxError};
pub use parser::{parse_yaml, parse_yaml_file};
