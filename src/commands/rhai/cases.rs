// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Discovery and deserialisation of Rhai coverage cases.
//!
//! A case names one script function, supplies its arguments and optionally
//! asserts the JSON-compatible result. The format is deliberately boring:
//! one JSON file can be an array, or an object with a `cases` array.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One function call used by the coverage run.
#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct Case {
    /// Human-readable case name; derived when absent.
    #[serde(default)]
    pub name: String,
    /// Script relative to `--src`; optional when the file has one script.
    #[serde(default)]
    pub file: String,
    /// Alias accepted for clarity.
    #[serde(default)]
    pub script: String,
    /// Function to call.
    pub function: String,
    /// JSON-compatible function arguments.
    #[serde(default)]
    pub args: Vec<serde_json::Value>,
    /// Optional expected JSON result.
    #[serde(default)]
    pub expected: Option<serde_json::Value>,
}

impl Case {
    /// The script selector, accepting both `file` and `script`.
    pub fn selected_file(&self) -> Option<&str> {
        if self.file.is_empty() {
            if self.script.is_empty() {
                None
            } else {
                Some(self.script.as_str())
            }
        } else {
            Some(self.file.as_str())
        }
    }

    /// The display name used in diagnostics.
    pub fn display_name(&self, fallback: &str) -> String {
        if self.name.is_empty() {
            format!("{fallback}#{}", self.function)
        } else {
            self.name.clone()
        }
    }
}

/// Loads all case files named by the `--cases` selector.
///
/// The selector may be empty (no cases), one JSON file, or a directory walked
/// recursively for `*.json` files. Sorted traversal makes diagnostics stable.
pub fn load(selector: &str) -> Result<Vec<Case>, String> {
    if selector.trim().is_empty() {
        return Ok(Vec::new());
    }
    let path = PathBuf::from(selector);
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("cases path {}: {error}", path.display()))?;
    let mut files = Vec::new();
    if metadata.is_file() {
        files.push(path);
    } else {
        collect_json_files(&path, &mut files)?;
    }
    files.sort();
    let mut cases = Vec::new();
    for file in files {
        cases.extend(parse_file(&file)?);
    }
    Ok(cases)
}

/// Appends every JSON file at or below `dir`.
fn collect_json_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    let mut items: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    items.sort();
    for path in items {
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            files.push(path);
        }
    }
    Ok(())
}

/// Parses one case file and reports its path in any schema error.
fn parse_file(path: &Path) -> Result<Vec<Case>, String> {
    let file =
        std::fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_reader(file)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    let entries = match value {
        serde_json::Value::Array(entries) => entries,
        serde_json::Value::Object(object) => match object.get("cases") {
            Some(serde_json::Value::Array(entries)) => entries.clone(),
            Some(other) => {
                return Err(format!(
                    "parse {}: \"cases\" must be an array, got {}",
                    path.display(),
                    kind(other)
                ))
            }
            None => {
                return Err(format!(
                    "parse {}: an object must have a \"cases\" array",
                    path.display()
                ))
            }
        },
        other => {
            return Err(format!(
                "parse {}: expected an array or object with cases, got {}",
                path.display(),
                kind(&other)
            ))
        }
    };
    entries
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            serde_json::from_value(value)
                .map_err(|error| format!("parse {}: case {}: {error}", path.display(), index))
        })
        .collect()
}

/// A short JSON value kind for error messages.
fn kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn an_empty_selector_loads_no_cases() {
        assert_eq!(load("").expect("empty selector"), Vec::<Case>::new());
        assert_eq!(load("  ").expect("blank selector"), Vec::<Case>::new());
    }

    #[test]
    fn reads_an_array_and_the_wrapped_form() {
        let dir = tempdir().expect("temporary directory");
        std::fs::write(dir.path().join("one.json"), r#"[{"function":"f"}]"#).expect("write array");
        std::fs::write(
            dir.path().join("two.json"),
            r#"{"cases":[{"function":"g"}]}"#,
        )
        .expect("write object");
        let cases = load(dir.path().to_string_lossy().as_ref()).expect("cases");
        let names: Vec<&str> = cases.iter().map(|case| case.function.as_str()).collect();
        assert_eq!(names, ["f", "g"]);
    }

    #[test]
    fn a_schema_error_names_the_file_and_case() {
        let dir = tempdir().expect("temporary directory");
        std::fs::write(dir.path().join("bad.json"), r#"[{"args":[]}]"#).expect("write bad case");
        let error = load(dir.path().to_string_lossy().as_ref()).unwrap_err();
        assert!(error.contains("bad.json: case 0"), "{error}");
        assert!(error.contains("missing field `function`"), "{error}");
    }

    #[test]
    fn missing_selectors_are_clear_usage_errors() {
        let error = load("definitely-missing-rhai-cases").unwrap_err();
        assert!(error.starts_with("cases path"), "{error}");
        let _ = strings(&[]);
    }
}
