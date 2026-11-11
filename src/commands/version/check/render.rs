// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The two renderings of a `version-check` result.
//!
//! Without `--output` the command prints Go's human report; with one it
//! prints Go's `output.Report` in whichever mode was asked for. The keys of
//! the `extra` object are inserted in the lexicographic order Go's
//! `map[string]any` marshals with, because JSON is rendered straight from
//! this map while TOON sorts on its way out.

use serde_json::{Map, Value};

use super::{strip_rc, Info, MirrorCheck, TreeMirrorCheck};
use crate::config::Config;
use crate::output::{parse_mode, Diagnostic, Report};

/// The default (no `--output`) rendering.
pub(super) fn text(info: &Info) {
    println!("🏷️  Checking VERSION consistency...");
    println!("  VERSION={}", info.version);
    version_go_lines(info);
    mirror_lines(info);
    git_lines(info);
    println!("✅ VERSION check passed");
}

/// The optional `pkg/version/version.go` line.
fn version_go_lines(info: &Info) {
    if info.version_go_match {
        println!("  ✓ pkg/version/version.go matches VERSION");
    } else if !info.version_go_note.is_empty() {
        println!(
            "  ⚠ pkg/version/version.go: {} (optional)",
            info.version_go_note
        );
    }
}

/// One line per declared mirror, file and tree alike.
fn mirror_lines(info: &Info) {
    for mirror in &info.mirrors {
        if mirror.matched {
            println!("  ✓ mirror {} matches VERSION", mirror.path);
        } else {
            println!("  ⚠ mirror {}: {}", mirror.path, mirror.note);
        }
    }
    for mirror in &info.tree_mirrors {
        if mirror.matched {
            println!("  ✓ tree mirror {} == {}", mirror.src, mirror.dst);
        } else {
            println!(
                "  ⚠ tree mirror {} vs {}: {}",
                mirror.src, mirror.dst, mirror.note
            );
        }
    }
}

/// The branch, the pre-release lane and the two heads.
fn git_lines(info: &Info) {
    if !info.in_git {
        println!("  ⚠ Not in a git repo, skipping branch checks");
        return;
    }
    println!("  Branch: {}", info.branch);
    if info.is_rc {
        println!(
            "  ✓ rc pre-release (stable base: {})",
            strip_rc(&info.version)
        );
    }
    if info.tag_on_release.is_empty() {
        return;
    }
    println!("  ✓ Tag {} exists", info.tag_on_release);
    if info.release_head.is_empty() || info.main_head.is_empty() {
        return;
    }
    if info.release_is_main {
        println!("  ✓ main HEAD == origin/main HEAD");
    } else {
        println!("  ⚠ main HEAD != origin/main HEAD");
    }
}

/// The `--output` rendering: Go's `output.Report` in the selected mode.
pub(super) fn structured(cfg: &Config, info: &Info) {
    let mut extra = Map::new();
    extra.insert("branch".into(), Value::String(info.branch.clone()));
    extra.insert("in_git".into(), Value::Bool(info.in_git));
    extra.insert("is_rc".into(), Value::Bool(info.is_rc));
    extra.insert("main_head".into(), Value::String(info.main_head.clone()));
    extra.insert(
        "mirrors".into(),
        Value::Array(info.mirrors.iter().map(mirror_value).collect()),
    );
    extra.insert(
        "release_head".into(),
        Value::String(info.release_head.clone()),
    );
    extra.insert("release_is_main".into(), Value::Bool(info.release_is_main));
    extra.insert("tag_found".into(), Value::Bool(info.tag_found));
    extra.insert(
        "tag_on_release".into(),
        Value::String(info.tag_on_release.clone()),
    );
    extra.insert(
        "tree_mirrors".into(),
        Value::Array(info.tree_mirrors.iter().map(tree_value).collect()),
    );
    extra.insert("version".into(), Value::String(info.version.clone()));
    extra.insert(
        "version_go_match".into(),
        Value::Bool(info.version_go_match),
    );
    extra.insert(
        "version_go_note".into(),
        Value::String(info.version_go_note.clone()),
    );

    let mut report = Report::new("version-check");
    report.ok = info.ok;
    report.summary = format!(
        "version-check: VERSION={} branch={}",
        info.version, info.branch
    );
    for warning in &info.warnings {
        report.add(Diagnostic::warning("", "version-check", warning.clone()));
    }
    report.extra = Some(Value::Object(extra));
    print!("{}", report.render(parse_mode(&cfg.output)));
}

/// One `mirrors` entry, keys in Go's sorted order.
fn mirror_value(mirror: &MirrorCheck) -> Value {
    let mut map = Map::new();
    map.insert("exists".into(), Value::Bool(mirror.exists));
    map.insert("match".into(), Value::Bool(mirror.matched));
    map.insert("note".into(), Value::String(mirror.note.clone()));
    map.insert("path".into(), Value::String(mirror.path.clone()));
    Value::Object(map)
}

/// One `tree_mirrors` entry, keys in Go's sorted order.
fn tree_value(mirror: &TreeMirrorCheck) -> Value {
    let mut map = Map::new();
    map.insert("dst".into(), Value::String(mirror.dst.clone()));
    map.insert("exists".into(), Value::Bool(mirror.exists));
    map.insert("match".into(), Value::Bool(mirror.matched));
    map.insert("note".into(), Value::String(mirror.note.clone()));
    map.insert("src".into(), Value::String(mirror.src.clone()));
    Value::Object(map)
}
