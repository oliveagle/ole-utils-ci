// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The package dependency graph.
//!
//! The graph is built from the imports of the source files: a Rust module
//! depends on the module it imports, a Go package depends on the package it
//! imports. Only intra-project edges are recorded.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One node of the dependency graph.
#[derive(Debug, Clone)]
pub struct Node {
    /// The package or module path, relative to the project root.
    pub path: String,
    /// The packages it depends on.
    pub deps: BTreeSet<String>,
}

/// A dependency graph of packages.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// The nodes, keyed by their path.
    pub nodes: BTreeMap<String, Node>,
}

impl Graph {
    /// Builds the graph of the project rooted at `root`.
    pub fn build(root: &Path) -> Graph {
        let mut graph = Graph::default();
        let rust_modules = rust_modules(root);
        for (module, file) in &rust_modules {
            let deps = rust_dependencies(file, &rust_modules);
            graph.insert(module.clone(), deps);
        }
        let go_packages = go_packages(root);
        for (package, files) in &go_packages {
            let mut deps = BTreeSet::new();
            for file in files {
                for dep in go_dependencies(file) {
                    if go_packages.contains_key(&dep) {
                        deps.insert(dep);
                    }
                }
            }
            graph.insert(package.clone(), deps);
        }
        graph
    }

    /// Records one node and its dependencies.
    fn insert(&mut self, path: String, deps: BTreeSet<String>) {
        self.nodes
            .entry(path.clone())
            .and_modify(|node| {
                node.deps.extend(deps.iter().cloned());
            })
            .or_insert(Node { path, deps });
    }

    /// The dependency cycles of the graph.
    ///
    /// Every cycle is reported once, starting at its alphabetically smallest
    /// node.
    pub fn cycles(&self) -> Vec<Vec<String>> {
        let mut cycles: Vec<Vec<String>> = Vec::new();
        let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
        for start in self.nodes.keys() {
            let mut stack = vec![start.clone()];
            let mut on_stack = vec![start.clone()];
            self.walk(
                start,
                start,
                &mut stack,
                &mut on_stack,
                &mut cycles,
                &mut seen,
            );
        }
        cycles
    }

    /// Depth-first search that collects the cycles through `node`.
    fn walk(
        &self,
        start: &str,
        node: &str,
        stack: &mut Vec<String>,
        on_stack: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
        seen: &mut BTreeSet<Vec<String>>,
    ) {
        let Some(current) = self.nodes.get(node) else {
            return;
        };
        for dep in &current.deps {
            if dep == start {
                let mut cycle = stack.clone();
                cycle.sort();
                if seen.insert(cycle.clone()) {
                    cycles.push(cycle);
                }
                continue;
            }
            if on_stack.contains(dep) {
                continue;
            }
            stack.push(dep.clone());
            on_stack.push(dep.clone());
            self.walk(start, dep, stack, on_stack, cycles, seen);
            stack.pop();
            on_stack.pop();
        }
    }
}

/// The Rust modules of the project, keyed by their module path.
fn rust_modules(root: &Path) -> BTreeMap<String, std::path::PathBuf> {
    let mut modules = BTreeMap::new();
    let src = root.join("src");
    collect_rust_modules(&src, &src, &mut modules);
    modules
}

/// Collects the modules of one directory, recursively.
fn collect_rust_modules(
    src: &Path,
    dir: &Path,
    modules: &mut BTreeMap<String, std::path::PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_modules(src, &path, modules);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Some(relative) = path.strip_prefix(src).ok() else {
            continue;
        };
        let mut parts: Vec<String> = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        let file = parts.pop().unwrap_or_default();
        let stem = file.trim_end_matches(".rs").to_string();
        if stem == "mod" || stem == "main" || stem == "lib" {
            continue;
        }
        let module = if parts.is_empty() {
            stem
        } else {
            format!("{}::{}", parts.join("::"), stem)
        };
        modules.insert(module, path);
    }
}

/// The modules a Rust file imports.
fn rust_dependencies(
    file: &Path,
    modules: &BTreeMap<String, std::path::PathBuf>,
) -> BTreeSet<String> {
    let Ok(source) = std::fs::read_to_string(file) else {
        return BTreeSet::new();
    };
    let mut deps = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("use ") else {
            continue;
        };
        let rest = rest.trim().trim_end_matches(';');
        let rest = rest
            .strip_prefix("crate::")
            .or_else(|| rest.strip_prefix("self::"))
            .unwrap_or(rest);
        if trimmed.starts_with("pub use ") {
            continue;
        }
        for module in modules.keys() {
            if rest == module || rest.starts_with(&format!("{module}::")) {
                deps.insert(module.clone());
            }
        }
    }
    deps
}

/// The Go packages of the project, keyed by their directory.
fn go_packages(root: &Path) -> BTreeMap<String, Vec<std::path::PathBuf>> {
    let mut packages: BTreeMap<String, Vec<std::path::PathBuf>> = BTreeMap::new();
    collect_go_packages(root, root, &mut packages, 0);
    packages
}

/// Collects the Go packages of one directory, recursively.
fn collect_go_packages(
    root: &Path,
    dir: &Path,
    packages: &mut BTreeMap<String, Vec<std::path::PathBuf>>,
    depth: usize,
) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), "vendor" | "node_modules" | ".git" | "target") {
                continue;
            }
            collect_go_packages(root, &path, packages, depth + 1);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("go") {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let key = relative
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| ".".to_string());
        packages.entry(key).or_default().push(path);
    }
}

/// The packages a Go file imports.
fn go_dependencies(file: &Path) -> Vec<String> {
    let Ok(source) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    let mut in_block = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if in_block {
            if trimmed == ")" {
                in_block = false;
                continue;
            }
            deps.push(trimmed.trim_matches('"').to_string());
            continue;
        }
        if trimmed.starts_with("import (") {
            in_block = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("import ") {
            deps.push(rest.trim().trim_matches('"').to_string());
        }
    }
    deps
}
