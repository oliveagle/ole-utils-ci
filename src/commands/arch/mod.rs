// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `arch` command: dependency cycles and layer direction.
//!
//! Two rules are enforced. The first is that the dependency graph has no
//! cycle — a cycle makes the package graph impossible to reason about. The
//! second is the layer direction: a handler may use a service and a service
//! may use a repository, but never the other way round.

pub mod graph;

use crate::config::Config;

use graph::Graph;

/// The architecture layers, from the outermost to the innermost.
const LAYERS: [(&str, [&str; 5]); 3] = [
    (
        "handler",
        ["handler", "api", "server", "controller", "http"],
    ),
    (
        "service",
        ["service", "usecase", "application", "biz", "domain"],
    ),
    (
        "repository",
        ["repository", "repo", "dao", "store", "storage"],
    ),
];

/// One architecture violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchViolation {
    /// The rule that was broken.
    pub rule: String,
    /// A one-line description.
    pub message: String,
    /// The package the rule applies to.
    pub package: String,
    /// The package it depends on, when the rule is about a dependency.
    pub dependency: Option<String>,
}

/// The layer a package belongs to, if any.
pub fn layer_of(package: &str) -> Option<(&'static str, usize)> {
    let segments: Vec<&str> = package.split(['/', ':']).collect();
    for (index, (name, patterns)) in LAYERS.iter().enumerate() {
        for segment in &segments {
            for pattern in patterns.iter() {
                if segment.eq_ignore_ascii_case(pattern) {
                    return Some((name, index));
                }
            }
        }
    }
    None
}

/// Checks the graph against the architecture rules.
pub fn check(graph: &Graph) -> Vec<ArchViolation> {
    let mut violations = Vec::new();
    for cycle in graph.cycles() {
        violations.push(ArchViolation {
            rule: "cycle-detection".to_string(),
            message: format!("circular dependency: {}", cycle.join(" → ")),
            package: cycle.first().cloned().unwrap_or_default(),
            dependency: None,
        });
    }
    for node in graph.nodes.values() {
        let Some((_, source_layer)) = layer_of(&node.path) else {
            continue;
        };
        for dep in &node.deps {
            let Some((_, target_layer)) = layer_of(dep) else {
                continue;
            };
            if target_layer < source_layer {
                violations.push(ArchViolation {
                    rule: "layer-dependency".to_string(),
                    message: format!(
                        "layer '{}' must not depend on layer '{}'",
                        layer_name(source_layer),
                        layer_name(target_layer)
                    ),
                    package: node.path.clone(),
                    dependency: Some(dep.clone()),
                });
            }
        }
    }
    violations
}

/// The name of a layer index.
fn layer_name(index: usize) -> &'static str {
    LAYERS
        .get(index)
        .map(|(name, _)| *name)
        .unwrap_or("unknown")
}

/// The `arch` command.
pub fn arch(cfg: &Config) -> Result<i32, String> {
    let graph = Graph::build(cfg.root());
    let violations = check(&graph);
    // Go's text layout only appears when `--output` was left alone; every
    // selected mode carries the same payload through `output.Encode`.
    if cfg.output.is_empty() {
        return text_mode(&violations);
    }
    print!("{}", encode(&cfg.output, &violations)?);
    Ok(0)
}

/// The selected-mode rendering: `output.Encode` of Go's report struct.
///
/// `json` and `toon` carry the `{command, ok, violations}` document; `text`
/// falls back to the `%v` rendering of the struct — `{arch <ok> [...]}` with
/// no trailing newline, the way `fmt.Fprintf` writes it.
fn encode(mode: &str, violations: &[ArchViolation]) -> Result<String, String> {
    let selected = mode.trim().to_ascii_lowercase();
    if selected != "json" && selected != "toon" {
        return Ok(format!(
            "{{arch {} [{}]}}",
            violations.is_empty(),
            violations
                .iter()
                .map(ArchViolation::render)
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    crate::output::render::encode_value(mode, false, &payload(violations))
}

/// The `{command, ok, violations}` document Go marshals, with Go's key order
/// and an omitted `dep` the way `omitempty` writes it.
fn payload(violations: &[ArchViolation]) -> serde_json::Value {
    let list = if violations.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(
            violations
                .iter()
                .map(|violation| {
                    let mut object = serde_json::Map::new();
                    object.insert("rule".into(), violation.rule.clone().into());
                    object.insert("message".into(), violation.message.clone().into());
                    object.insert("package".into(), violation.package.clone().into());
                    if let Some(dep) = &violation.dependency {
                        object.insert("dep".into(), dep.clone().into());
                    }
                    serde_json::Value::Object(object)
                })
                .collect(),
        )
    };
    serde_json::json!({
        "command": "arch",
        "ok": violations.is_empty(),
        "violations": list,
    })
}

/// The text rendering, line for line as `archRenderText` writes it.
fn text_mode(violations: &[ArchViolation]) -> Result<i32, String> {
    if violations.is_empty() {
        println!("arch: all checks passed ✓");
        return Ok(0);
    }
    println!("arch: {} violation(s) found:\n", violations.len());
    for violation in violations {
        println!("  {}", violation.render());
    }
    Err(format!("{} architecture violation(s)", violations.len()))
}

impl ArchViolation {
    /// `[rule] message`, with the dependency chain appended when there is one
    /// — `archViolation.String`.
    pub fn render(&self) -> String {
        match &self.dependency {
            Some(dep) => format!("[{}] {}: {} → {dep}", self.rule, self.message, self.package),
            None => format!("[{}] {}", self.rule, self.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_packages_to_layers() {
        assert_eq!(layer_of("api/handler").map(|l| l.1), Some(0));
        assert_eq!(layer_of("internal/service").map(|l| l.1), Some(1));
        assert_eq!(layer_of("internal/repository").map(|l| l.1), Some(2));
        assert_eq!(layer_of("internal/util"), None);
    }

    #[test]
    fn go_imports_are_edges_because_packages_are_keyed_by_import_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("go.mod"), "module example.com/x\n\ngo 1.21\n").unwrap();
        for (name, source) in [
            ("handler", "package handler\n\nvar R = 1\n"),
            (
                "repository",
                "package repository\n\nimport \"example.com/x/handler\"\n\nvar V = handler.R\n",
            ),
        ] {
            std::fs::create_dir(root.join(name)).unwrap();
            std::fs::write(root.join(name).join("main.go"), source).unwrap();
        }
        let graph = Graph::build(root);
        let repository = graph
            .nodes
            .get("example.com/x/repository")
            .expect("the package is a node under its import path");
        assert!(repository.deps.contains("example.com/x/handler"));
        assert_eq!(
            check(&graph)
                .iter()
                .map(|violation| violation.render())
                .collect::<Vec<_>>(),
            vec![
                "[layer-dependency] layer 'repository' must not depend on layer 'handler': example.com/x/repository → example.com/x/handler"
            ]
        );
    }

    #[test]
    fn a_selected_mode_carries_the_go_payload() {
        assert_eq!(
            payload(&[]).to_string(),
            r#"{"command":"arch","ok":true,"violations":null}"#
        );
        assert_eq!(
            encode("json", &[]).unwrap(),
            "{\n  \"command\": \"arch\",\n  \"ok\": true,\n  \"violations\": null\n}\n"
        );
        assert_eq!(encode("text", &[]).unwrap(), "{arch true []}");

        let violation = ArchViolation {
            rule: "cycle-detection".to_string(),
            message: "circular dependency: a → a".to_string(),
            package: "a".to_string(),
            dependency: None,
        };
        assert_eq!(
            encode("text", std::slice::from_ref(&violation)).unwrap(),
            "{arch false [[cycle-detection] circular dependency: a → a]}"
        );
        assert!(payload(&[violation]).to_string().starts_with(
            r#"{"command":"arch","ok":false,"violations":[{"rule":"cycle-detection""#
        ));
    }
}
