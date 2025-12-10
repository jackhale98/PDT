//! YAML parsing with error handling

use serde::de::DeserializeOwned;

use crate::yaml::diagnostics::{YamlError, YamlSyntaxError};

/// Parse YAML content into a typed value with nice error messages
pub fn parse_yaml<T: DeserializeOwned>(content: &str, filename: &str) -> Result<T, YamlError> {
    serde_yml::from_str(content).map_err(|e| {
        YamlError::Syntax(YamlSyntaxError::from_serde_error(&e, content, filename))
    })
}

/// Parse YAML from a file path
pub fn parse_yaml_file<T: DeserializeOwned>(path: &std::path::Path) -> Result<T, YamlError> {
    let content = std::fs::read_to_string(path)?;
    let filename = path.display().to_string();
    parse_yaml(&content, &filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestStruct {
        name: String,
        value: i32,
    }

    #[test]
    fn test_parse_valid_yaml() {
        let yaml = "name: test\nvalue: 42";
        let result: TestStruct = parse_yaml(yaml, "test.yaml").unwrap();
        assert_eq!(result.name, "test");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn test_parse_invalid_yaml_returns_error() {
        let yaml = "name: test\n  invalid indentation";
        let result: Result<TestStruct, _> = parse_yaml(yaml, "test.yaml");
        assert!(result.is_err());
    }
}
