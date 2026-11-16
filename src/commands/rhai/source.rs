// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Parsing, loading, instrumentation setup and hook execution.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rhai::debugger::{DebuggerCommand, DebuggerEvent};
use rhai::{Dynamic, Engine, OptimizationLevel, Scope, AST};

use crate::fsutil;

use super::executable::executable_lines;
use super::instrument::{branch_sites, instrument};
use super::model::{BranchLocation, LoadedSource, RuntimeCoverage, SourceError, BRANCH_PROBE};

pub fn parse(source: &str) -> Result<AST, SourceError> {
    syntax_engine()
        .compile(source)
        .map_err(|error| SourceError {
            rule: "rhai/syntax".to_string(),
            message: error.to_string(),
            line: error.position().line().unwrap_or(0),
            col: error.position().position().unwrap_or(0),
        })
}

pub fn load(path: &Path) -> Result<LoadedSource, SourceError> {
    let data = std::fs::read(path).map_err(|error| SourceError {
        rule: "rhai/read".to_string(),
        message: format!("{}: {error}", path.display()),
        line: 0,
        col: 0,
    })?;
    let source = String::from_utf8_lossy(&data).into_owned();
    let ast = parse(&source)?;
    let sites = branch_sites(&source);
    let (instrumented, coverage_error) = instrument(&source, &sites).map_or_else(
        |error| (source.clone(), Some(error)),
        |text| match parse(&text) {
            Ok(_runtime_ast) => (text, None),
            Err(error) => (
                source.clone(),
                Some(format!("branch instrumentation failed: {}", error.message)),
            ),
        },
    );
    let runtime_ast = if coverage_error.is_some() {
        ast.clone()
    } else {
        parse(&instrumented).map_err(|error| SourceError {
            rule: "rhai/instrument".to_string(),
            message: error.message,
            line: error.line,
            col: error.col,
        })?
    };

    Ok(LoadedSource {
        lines: executable_lines(&ast, &source),
        branches: sites
            .iter()
            .map(|site| BranchLocation {
                id: site.id,
                line: site.line,
                col: site.col,
            })
            .collect(),
        source,
        runtime_ast,
        coverage_error,
    })
}

/// Creates a syntax-only engine with deterministic, unoptimised positions.
fn syntax_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_optimization_level(OptimizationLevel::None);
    engine
}

/// Creates a sandboxed engine wired to the shared coverage state.
pub fn coverage_engine(
    coverage: &Arc<Mutex<RuntimeCoverage>>,
    max_operations: u64,
) -> Result<Engine, String> {
    let mut engine = syntax_engine();
    engine.set_max_operations(max_operations.max(1));
    engine.set_max_call_levels(256);
    engine.set_max_expr_depths(128, 128);
    engine.set_max_array_size(100_000);
    engine.set_max_string_size(4_194_304);
    engine.set_max_variables(10_000);
    engine.set_max_functions(10_000);
    engine.set_max_modules(0);

    let branch_state = Arc::clone(coverage);
    engine.register_fn(BRANCH_PROBE, move |site: i64, taken: bool| -> bool {
        if let Ok(mut state) = branch_state.lock() {
            state.hit_branch(site.max(0) as usize, taken);
        }
        taken
    });

    let line_state = Arc::clone(coverage);
    #[allow(deprecated)]
    engine.register_debugger(
        |_, debugger| debugger,
        move |_context, event, node, _source, position| {
            if matches!(
                event,
                DebuggerEvent::Start
                    | DebuggerEvent::Step
                    | DebuggerEvent::FunctionExitWithValue(_)
            ) {
                if let Some(line) = node.position().line().or(position.line()) {
                    if let Ok(mut state) = line_state.lock() {
                        state.hit_line(line);
                    }
                }
            }
            Ok(DebuggerCommand::StepInto)
        },
    );
    Ok(engine)
}

/// Converts JSON arguments to Rhai values and invokes one hook.
pub fn call_hook(
    engine: &Engine,
    ast: &AST,
    function: &str,
    args: &[serde_json::Value],
) -> Result<Dynamic, String> {
    let mut rhai_args = Vec::with_capacity(args.len());
    for (index, value) in args.iter().enumerate() {
        rhai_args.push(
            rhai::serde::to_dynamic(value).map_err(|error| format!("argument {index}: {error}"))?,
        );
    }
    let mut scope = Scope::new();
    engine
        .call_fn::<Dynamic>(&mut scope, ast, function, rhai_args)
        .map_err(|error| error.to_string())
}

/// Converts a Rhai result into JSON without depending on Dynamic internals.
pub fn to_json(value: &Dynamic) -> Result<serde_json::Value, String> {
    rhai::serde::from_dynamic::<serde_json::Value>(value)
        .map_err(|error| format!("result is not JSON-compatible: {error}"))
}

pub fn resolve_source(root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let joined = if Path::new(name).is_absolute() {
        std::path::PathBuf::from(name)
    } else {
        root.join(name)
    };
    let resolved = fsutil::absolute(&joined)
        .map_err(|error| format!("resolve {}: {error}", joined.display()))?;
    let root = fsutil::absolute(root).map_err(|error| format!("resolve root: {error}"))?;
    if resolved != root && !resolved.starts_with(&root) {
        return Err(format!(
            "case script {name:?} escapes source root {}",
            root.display()
        ));
    }
    Ok(resolved)
}
