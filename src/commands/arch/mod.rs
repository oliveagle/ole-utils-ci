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
use crate::output::{Diagnostic, Report};

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
            rule: "arch/dependency-cycle".to_string(),
            message: format!("dependency cycle: {}", cycle.join(" → ")),
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
                    rule: "arch/layer-direction".to_string(),
                    message: format!(
                        "{} must not depend on {}",
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
    let mut report = Report::new("arch");
    report.extra = Some(serde_json::json!({
        "packages": graph.nodes.len(),
        "violations": violations.len(),
    }));
    for v in &violations {
        let message = match &v.dependency {
            Some(dep) => format!("{}: {} → {dep}", v.message, v.package),
            None => v.message.clone(),
        };
        report.add(Diagnostic::error(
            v.package.clone(),
            v.rule.clone(),
            message,
        ));
    }
    if report.ok {
        report.summary = format!("arch: {} package(s), no violation", graph.nodes.len());
        print!("{}", report.render_text());
        Ok(0)
    } else {
        report.summary = format!("arch: {} violation(s)", violations.len());
        print!("{}", report.render_text());
        Err(report.summary)
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
}
