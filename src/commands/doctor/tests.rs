// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for `doctor`: the wording of every line the report can print.
//!
//! The probes that read the environment are exercised through the values
//! they take as arguments rather than by rewriting the process environment,
//! so nothing here races with the rest of the test binary.

use std::process::Command;

use super::{checks, metadata, project, render, toolchain};

#[test]
fn a_missing_go_mod_is_worded_the_way_go_words_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let results = toolchain::go_version(dir.path());
    assert_eq!(results.len(), 1);
    let check = &results[0];
    assert_eq!(check.name, "Go version");
    assert_eq!(check.status, "warn");
    assert_eq!(
        check.detail,
        format!(
            "Cannot read go.mod: open {}: no such file or directory",
            dir.path().join("go.mod").display()
        )
    );
    assert_eq!(
        check.hint,
        "Run doctor from the project root or set --project-root"
    );
}

#[test]
fn go_version_names_the_reference_build_and_the_directive() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("go.mod"),
        "module example.com/x\n\ngo 1.21\n",
    )
    .expect("go.mod");
    let results = toolchain::go_version(dir.path());
    assert_eq!(results[0].status, "pass");
    // The reference binary reports the toolchain that built it, not the
    // toolchain installed here; the number is `go version -m` on it.
    assert_eq!(
        results[0].detail,
        "Running go1.26.8, go.mod requires go 1.21"
    );
}

#[test]
fn a_go_mod_without_a_go_line_warns_without_a_hint() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("go.mod"), "module example.com/x\n").expect("go.mod");
    let results = toolchain::go_version(dir.path());
    assert_eq!(results[0].status, "warn");
    assert_eq!(results[0].detail, "No 'go X.Y' directive found in go.mod");
    assert_eq!(results[0].hint, "");
}

#[test]
fn go_directives_are_read_the_way_go_reads_them() {
    assert_eq!(
        toolchain::go_directive("module x\ngo 1.21\n"),
        Some("1.21".to_string())
    );
    assert_eq!(
        toolchain::go_directive("  go 1.22  \n"),
        Some("1.22".to_string())
    );
    assert_eq!(
        toolchain::go_directive("module x\ntoolchain go1.22.2\n"),
        None
    );
    assert_eq!(toolchain::go_directive("go\n"), None);
    // A line that is only `go` is not a directive once it has been
    // trimmed, so the scan carries on to the next one.
    assert_eq!(
        toolchain::go_directive("go\ngo 1.23\n"),
        Some("1.23".to_string())
    );
    // The first directive wins when the file carries more than one.
    assert_eq!(
        toolchain::go_directive("go 1.21\ngo 1.23\n"),
        Some("1.21".to_string())
    );
}

#[test]
fn the_checksum_variables_carry_gos_status_and_hints() {
    let off = toolchain::gosumdb_check("off");
    assert_eq!(off.status, "pass");
    assert_eq!(
        off.detail,
        "GOSUMDB=off — checksum database disabled for all modules"
    );
    assert_eq!(off.hint, "");

    let configured = toolchain::gosumdb_check("sum.golang.org");
    assert_eq!(configured.status, "pass");
    assert_eq!(configured.detail, "GOSUMDB=sum.golang.org");

    let unset = toolchain::gosumdb_check("");
    assert_eq!(unset.status, "warn");
    assert_eq!(
        unset.detail,
        "GOSUMDB is not set — checksum database is enabled"
    );
    assert!(!unset.hint.is_empty());

    let skipped = toolchain::gonosumcheck_check("git.dev.sh.ctripcorp.com");
    assert_eq!(skipped.status, "pass");
    assert_eq!(
        skipped.detail,
        "GONOSUMCHECK=git.dev.sh.ctripcorp.com — go.sum verification skipped for matching modules"
    );

    let enforced = toolchain::gonosumcheck_check("");
    assert_eq!(enforced.status, "fail");
    assert_eq!(
        enforced.detail,
        "GONOSUMCHECK is not set — go.sum verification is enabled for ALL modules"
    );
    assert!(!enforced.hint.is_empty());
}

#[test]
fn metadata_status_follows_json_unmarshals_leniency() {
    assert_eq!(metadata::payload_status(b"null"), Ok(0));
    assert_eq!(metadata::payload_status(b"  null  "), Ok(0));
    assert_eq!(metadata::payload_status(br#"{"status":0}"#), Ok(0));
    assert_eq!(metadata::payload_status(br#"{"status":7}"#), Ok(7));
    // `null` and an absent key both leave Go's `int` at zero.
    assert_eq!(metadata::payload_status(br#"{"status":null}"#), Ok(0));
    assert_eq!(metadata::payload_status(br#"{}"#), Ok(0));
    assert!(metadata::payload_status(b"not json").is_err());
}

#[test]
fn verify_output_is_summarised_the_way_go_summarises_it() {
    assert_eq!(project::verify_detail(""), "All modules verified");
    assert_eq!(
        project::verify_detail("all modules verified\n"),
        "all modules verified"
    );
    assert_eq!(project::verify_detail("a\nb\n"), "a; b");
    assert_eq!(project::verify_status(""), "pass");
    assert_eq!(project::verify_status("all modules verified\n"), "pass");
    assert_eq!(project::verify_status("pkg/a: error\n"), "fail");
    assert_eq!(project::verify_status("checksum mismatch\n"), "fail");
}

#[test]
fn exit_status_is_worded_the_way_go_words_it() {
    let exited = Command::new("sh")
        .arg("-c")
        .arg("exit 3")
        .output()
        .expect("sh");
    assert_eq!(crate::process::exit_status(&exited.status), "exit status 3");

    let killed = Command::new("sh")
        .arg("-c")
        .arg("kill -9 $$")
        .output()
        .expect("sh");
    assert_eq!(
        crate::process::exit_status(&killed.status),
        "signal: killed"
    );
}

#[test]
fn combined_output_is_stdout_followed_by_stderr() {
    let output = Command::new("sh")
        .arg("-c")
        .arg("echo out; echo err >&2")
        .output()
        .expect("sh");
    assert_eq!(checks::combined(&output), "out\nerr\n");
}

#[test]
fn only_a_failure_fails_the_command() {
    assert!(!checks::Check::pass("a", "b").failed());
    assert!(!checks::Check::warn("a", "b", "hint").failed());
    assert!(checks::Check::fail("a", "b", "hint").failed());
}

#[test]
fn the_report_frames_every_check_the_way_go_frames_it() {
    let results = vec![
        checks::Check::pass("Go version", "Running go1.26.8, go.mod requires go 1.21"),
        checks::Check::warn("GOPROXY", "GOPROXY is not set", "Set GOPROXY"),
        checks::Check::fail(
            "GONOSUMCHECK",
            "GONOSUMCHECK is not set",
            "Set GONOSUMCHECK",
        ),
    ];
    let report = render::text(&results);
    let separator = format!("{}\n", "─".repeat(60));
    assert_eq!(report.matches('─').count(), 120);
    assert!(report.starts_with(&format!(
        "🔍 devine-utils-ci doctor\n{separator}\n✅  Go version\n   Running go1.26.8, go.mod requires go 1.21\n"
    )));
    assert!(report.contains("\n⚠️  GOPROXY\n   GOPROXY is not set\n   💡 Set GOPROXY\n"));
    assert!(
        report.contains("\n❌  GONOSUMCHECK\n   GONOSUMCHECK is not set\n   💡 Set GONOSUMCHECK\n")
    );
    assert!(report.ends_with(&format!(
        "{separator}📊 1 passed, 1 warnings, 1 failures\n❌ Some checks failed — review the hints above.\n"
    )));
}

#[test]
fn the_verdict_names_the_worst_status() {
    let healthy = vec![checks::Check::pass("a", "b")];
    assert!(render::text(&healthy).ends_with("✅ Environment looks healthy.\n"));

    let warned = vec![checks::Check::warn("a", "b", "")];
    assert!(
        render::text(&warned).ends_with("⚠️ All critical checks passed, but review warnings.\n")
    );

    let failed = vec![checks::Check::fail("a", "b", "")];
    assert!(render::text(&failed).ends_with("❌ Some checks failed — review the hints above.\n"));
}
