// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The subcommands that drive the Go toolchain.

use std::time::Duration;

use crate::config::{limits, Config};
use crate::runner::Runner;

/// The `install` subcommand: fetch the module dependencies.
pub fn install(cfg: &Config) -> Result<i32, String> {
    println!("📦 Installing Go module dependencies...");
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let proxies = [
        "https://proxy.golang.org,direct",
        "https://goproxy.cn,direct",
        "direct",
    ];
    let mut last_error = String::new();
    for proxy in proxies {
        let result =
            runner.run_with_timeout(Duration::from_secs(600), "go", &["mod", "vendor", "-e"]);
        match result {
            Ok(out) if out.success => {
                println!("✅ Dependencies vendored");
                return Ok(0);
            }
            Ok(out) => last_error = out.output,
            Err(e) => last_error = e,
        }
        cfg.log_verbose(&format!("proxy {proxy} failed, trying the next one"));
    }
    Err(format!("go mod vendor failed:\n{last_error}"))
}

/// One step of the composite lint gate.
struct LintStep {
    name: &'static str,
    program: &'static str,
    args: Vec<String>,
}

/// The `lint` subcommand: the composite static gate.
pub fn lint(cfg: &Config) -> Result<i32, String> {
    println!("🔍 Running the composite lint gate...");
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let mut steps = vec![
        LintStep {
            name: "gofmt -s",
            program: "gofmt",
            args: vec!["-s".into(), "-l".into(), ".".into()],
        },
        LintStep {
            name: "go mod edit -fmt",
            program: "go",
            args: vec!["mod".into(), "edit".into(), "-fmt".into()],
        },
        LintStep {
            name: "go vet",
            program: "go",
            args: vec!["vet".into(), "./...".into()],
        },
    ];
    if Runner::command_exists("golangci-lint") {
        steps.push(LintStep {
            name: "golangci-lint",
            program: "golangci-lint",
            args: vec!["run".into(), "--timeout=10m".into()],
        });
    }
    let mut failed = Vec::new();
    for step in &steps {
        let args: Vec<&str> = step.args.iter().map(|s| s.as_str()).collect();
        let result = runner.run_with_timeout(Duration::from_secs(900), step.program, &args)?;
        if result.success {
            println!("  ✓ {}", step.name);
        } else {
            println!("  ✗ {}", step.name);
            println!("{}", result.output);
            failed.push(step.name);
        }
    }
    if failed.is_empty() {
        println!("✅ Lint passed");
        Ok(0)
    } else {
        Err(format!("lint failed: {}", failed.join(", ")))
    }
}

/// The `test` subcommand: run the unit tests with the race detector.
pub fn test(cfg: &Config) -> Result<i32, String> {
    println!("🧪 Running tests...");
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let mut args: Vec<String> = vec!["test".into()];
    if cfg.test_parallel > 0 {
        args.push(format!("-parallel={}", cfg.test_parallel));
    }
    if !cfg.build_tags.is_empty() {
        args.push(format!("-tags={}", cfg.build_tags));
    }
    if race_supported() {
        args.push("-race".into());
    }
    args.push("./...".into());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = runner.run_with_timeout(Duration::from_secs(3600), "go", &refs)?;
    print!("{}", result.output);
    if result.success {
        println!("✅ Tests passed");
        Ok(0)
    } else {
        Err("tests failed".to_string())
    }
}

/// Whether the race detector can be used.
///
/// `-race` needs cgo; when `CGO_ENABLED=0` the Go toolchain refuses it, so
/// the flag is dropped instead of failing the run.
pub fn race_supported() -> bool {
    std::env::var("CGO_ENABLED")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// The `build` subcommand: compile the whole module.
pub fn build(cfg: &Config) -> Result<i32, String> {
    println!("🔨 Building...");
    let runner = Runner::new(cfg.root(), cfg.verbose);
    let mut args: Vec<String> = vec!["build".into()];
    if !cfg.build_tags.is_empty() {
        args.push(format!("-tags={}", cfg.build_tags));
    }
    args.push("./...".into());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = runner.run_with_timeout(Duration::from_secs(1800), "go", &refs)?;
    print!("{}", result.output);
    if result.success {
        println!("✅ Build succeeded");
        Ok(0)
    } else {
        Err("build failed".to_string())
    }
}

/// The `coverage` subcommand and the coverprofile analyser.
pub mod coverage;

/// The `test-report` subcommand: `go test -json` to JUnit XML.
pub mod junit;

pub use coverage::{analyze_coverage, coverage};
pub use junit::{junit_from_go_json, test_report};

/// The default coverage floor, re-exported for callers that need it.
pub fn coverage_floor() -> i64 {
    limits::COVERAGE_FLOOR
}

#[cfg(test)]
mod tests;
