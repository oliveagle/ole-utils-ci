// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The static executable-line analysis: which AST positions can execute.

use std::collections::BTreeSet;

use rhai::{Stmt, AST};

use super::instrument::{skip_noise, word_at};
use super::source::parse;

pub(super) fn executable_lines(ast: &AST, source: &str) -> BTreeSet<usize> {
    let mut lines = BTreeSet::new();
    for statement in ast.statements() {
        collect_statement(statement, &mut lines);
    }
    for body in function_bodies(source) {
        if let Ok(function_ast) = parse(&body) {
            for statement in function_ast.statements() {
                collect_statement(statement, &mut lines);
            }
        }
    }
    lines
}

/// Extracts function bodies, prefixed with blank lines to keep line numbers.
fn function_bodies(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let offsets = char_offsets(source);
    let mut bodies = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        skip_noise(&chars, &mut index);
        if index >= chars.len() {
            break;
        }
        let Some((open, close)) = function_extent(&chars, index) else {
            index += 1;
            continue;
        };
        let line = chars[..index].iter().filter(|ch| **ch == '\n').count() + 1;
        let body = slice_chars(source, &offsets, open + 1, close);
        bodies.push(format!("{}{body}", "\n".repeat(line - 1)));
        index = close + 1;
    }
    bodies
}

/// Finds the body braces of the `fn` starting at `start`, if any.
fn function_extent(chars: &[char], start: usize) -> Option<(usize, usize)> {
    if !word_at(chars, start, "fn") {
        return None;
    }
    let open = signature_open_brace(chars, start + 2)?;
    Some((open, matching_brace(chars, open)?))
}

/// Finds the `{` that opens a function body, stopping at `;`.
fn signature_open_brace(chars: &[char], mut probe: usize) -> Option<usize> {
    while probe < chars.len() {
        skip_noise(chars, &mut probe);
        if probe >= chars.len() {
            break;
        }
        match chars[probe] {
            '{' => return Some(probe),
            ';' => return None,
            _ => probe += 1,
        }
    }
    None
}

/// Finds the `}` that closes the brace at `open`.
fn matching_brace(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut probe = open;
    while probe < chars.len() {
        skip_noise(chars, &mut probe);
        if probe >= chars.len() {
            break;
        }
        match chars[probe] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(probe);
                }
            }
            _ => {}
        }
        probe += 1;
    }
    None
}

/// Maps every char index to its byte offset, plus the source length.
fn char_offsets(source: &str) -> Vec<usize> {
    source
        .char_indices()
        .map(|(byte, _)| byte)
        .chain([source.len()])
        .collect()
}

/// Slices `source` between two char indices.
fn slice_chars<'a>(source: &'a str, offsets: &[usize], start: usize, end: usize) -> &'a str {
    let start = offsets.get(start).copied().unwrap_or(0);
    let end = offsets.get(end).copied().unwrap_or(source.len());
    &source[start..end]
}

/// Adds one statement and recursively every nested statement/expression.
fn collect_statement(statement: &Stmt, lines: &mut BTreeSet<usize>) {
    if let Some(line) = statement.position().line() {
        lines.insert(line);
    }
    match statement {
        Stmt::If(flow, _) | Stmt::While(flow, _) | Stmt::Do(flow, _, _) => {
            collect_expression(&flow.expr, lines);
            collect_block(&flow.body, lines);
            collect_block(&flow.branch, lines);
        }
        Stmt::For(boxed, _) => {
            let (_, _, flow) = &**boxed;
            collect_expression(&flow.expr, lines);
            collect_block(&flow.body, lines);
            collect_block(&flow.branch, lines);
        }
        Stmt::Switch(boxed, _) => {
            let (subject, cases) = &**boxed;
            collect_expression(subject, lines);
            for binary in cases.expressions.iter() {
                collect_expression(&binary.lhs, lines);
                collect_expression(&binary.rhs, lines);
            }
        }
        Stmt::TryCatch(flow, _) => {
            collect_expression(&flow.expr, lines);
            collect_block(&flow.body, lines);
            collect_block(&flow.branch, lines);
        }
        Stmt::Block(block) => collect_block(block, lines),
        Stmt::Expr(expr) => collect_expression(expr, lines),
        Stmt::Assignment(pair) => {
            collect_expression(&pair.1.lhs, lines);
            collect_expression(&pair.1.rhs, lines);
        }
        Stmt::Return(Some(expr), _, _) => collect_expression(expr, lines),
        Stmt::BreakLoop(Some(expr), _, _) => collect_expression(expr, lines),
        _ => {}
    }
}

/// Adds the statements of a block.
fn collect_block(block: &rhai::StmtBlock, lines: &mut BTreeSet<usize>) {
    if let Some(line) = block.position().line() {
        lines.insert(line);
    }
    for statement in block.statements() {
        collect_statement(statement, lines);
    }
}

/// Adds one expression and recursively its children.
fn collect_expression(expr: &rhai::Expr, lines: &mut BTreeSet<usize>) {
    if let Some(line) = expr.position().line() {
        lines.insert(line);
    }
    match expr {
        rhai::Expr::InterpolatedString(parts, _) | rhai::Expr::Array(parts, _) => {
            for part in parts.iter() {
                collect_expression(part, lines);
            }
        }
        rhai::Expr::MethodCall(call, _) | rhai::Expr::FnCall(call, _) => {
            for argument in call.args.iter() {
                collect_expression(argument, lines);
            }
        }
        rhai::Expr::And(parts, _) | rhai::Expr::Or(parts, _) | rhai::Expr::Coalesce(parts, _) => {
            for part in parts.iter() {
                collect_expression(part, lines);
            }
        }
        rhai::Expr::Dot(binary, _, _) | rhai::Expr::Index(binary, _, _) => {
            collect_expression(&binary.lhs, lines);
            collect_expression(&binary.rhs, lines);
        }
        rhai::Expr::Stmt(block) => collect_block(block, lines),
        _ => {}
    }
}
