// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `mojo` gates that drive the toolchain.
//!
//! They shell out to `mojo`, so they are pinned here against a fake `mojo` that records the
//! command line it was handed. The shapes matter: Mojo 1.2's `format` has no check mode, `build`
//! and `doc` take exactly one input, `precompile` takes a package directory, and `test` is not
//! a command at all. A gate that gets one of those wrong cannot pass on any project, and this file
//! is what stops that from happening again.

mod support;

#[cfg(unix)]
use support::{fake_mojo, invocations, main_binary_env};

#[test]
#[cfg(unix)]
fn format_checks_a_copy_and_never_touches_the_sources() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(
        dir.path().join("ugly.mojo"),
        "def  ugly( x:Int)->Int:\n    return x\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "format",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[
            ("FAKE_ARGS", args_file.to_string_lossy().as_ref()),
            ("FAKE_REFORMAT", "ugly.mojo"),
        ],
    );
    assert_eq!(code, 1, "stdout: {stdout} stderr: {stderr}");
    assert!(stdout.contains("mojo.unformatted"), "stdout: {stdout}");
    // The source is exactly what it was: the formatter only ever saw a scratch copy.
    let untouched = std::fs::read_to_string(dir.path().join("ugly.mojo")).expect("read back");
    assert_eq!(untouched, "def  ugly( x:Int)->Int:\n    return x\n");
    let calls = invocations(&args_file);
    assert_eq!(calls.len(), 1, "calls: {calls:?}");
    assert!(calls[0].starts_with("format "), "calls: {calls:?}");
    assert!(
        !calls[0].contains("--check"),
        "no check mode exists: {calls:?}"
    );
    assert!(
        !calls[0].contains(dir.path().to_string_lossy().as_ref()),
        "not in the project: {calls:?}"
    );
}

#[test]
#[cfg(unix)]
fn a_formatted_project_passes_format() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(
        dir.path().join("clean.mojo"),
        "def one() -> Int:\n    return 1\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "format",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[("FAKE_ARGS", args_file.to_string_lossy().as_ref())],
    );
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    assert!(stdout.contains("already formatted"), "stdout: {stdout}");
}

#[test]
#[cfg(unix)]
fn build_runs_once_per_entry_point_and_only_for_files_with_a_main() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("app.mojo"), "def main():\n    print(1)\n").expect("write");
    std::fs::write(
        dir.path().join("lib.mojo"),
        "def twice(x: Int) -> Int:\n    return x\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "build",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[("FAKE_ARGS", args_file.to_string_lossy().as_ref())],
    );
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    let calls = invocations(&args_file);
    assert_eq!(calls.len(), 1, "one invocation per entry point: {calls:?}");
    assert!(
        calls[0].starts_with("build app.mojo -o "),
        "calls: {calls:?}"
    );
    assert!(
        !calls[0].contains("lib.mojo"),
        "a library module is not a build target: {calls:?}"
    );
}

#[test]
#[cfg(unix)]
fn test_runs_each_test_file_because_there_is_no_test_command() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join("tests")).expect("mkdir");
    std::fs::write(
        dir.path().join("tests/test_one.mojo"),
        "def main() raises:\n    pass\n",
    )
    .expect("write");
    std::fs::write(
        dir.path().join("tests/math_test.mojo"),
        "def main() raises:\n    pass\n",
    )
    .expect("write");
    std::fs::write(
        dir.path().join("tests/other.mojo"),
        "def helper() -> Int:\n    return 1\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "test",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[("FAKE_ARGS", args_file.to_string_lossy().as_ref())],
    );
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    let calls = invocations(&args_file);
    assert_eq!(
        calls.len(),
        2,
        "both test spellings, nothing else: {calls:?}"
    );
    assert!(
        calls.iter().all(|call| call.starts_with("run ")),
        "calls: {calls:?}"
    );
}

#[test]
#[cfg(unix)]
fn package_precompiles_the_package_directory() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join("src/pkg")).expect("mkdir");
    std::fs::write(
        dir.path().join("src/pkg/__init__.mojo"),
        "def f() -> Int:\n    return 1\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "package",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[("FAKE_ARGS", args_file.to_string_lossy().as_ref())],
    );
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    let calls = invocations(&args_file);
    assert_eq!(calls.len(), 1, "calls: {calls:?}");
    assert!(calls[0].starts_with("precompile "), "calls: {calls:?}");
    assert!(
        calls[0].contains("src/pkg"),
        "the directory, not the files: {calls:?}"
    );
    assert!(
        calls[0].contains(".mojoc"),
        "the extension 1.2 insists on: {calls:?}"
    );
}

#[test]
#[cfg(unix)]
fn doc_takes_one_target() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join("src/pkg")).expect("mkdir");
    std::fs::write(
        dir.path().join("src/pkg/__init__.mojo"),
        "def f() -> Int:\n    return 1\n",
    )
    .expect("write");
    std::fs::write(
        dir.path().join("src/app.mojo"),
        "def main():\n    print(1)\n",
    )
    .expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "doc",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[("FAKE_ARGS", args_file.to_string_lossy().as_ref())],
    );
    assert_eq!(code, 0, "stdout: {stdout} stderr: {stderr}");
    let calls = invocations(&args_file);
    assert_eq!(calls.len(), 1, "calls: {calls:?}");
    assert!(calls[0].starts_with("doc "), "calls: {calls:?}");
    assert!(
        calls[0].contains("-o "),
        "the JSON goes to a file: {calls:?}"
    );
}

#[test]
#[cfg(unix)]
fn a_project_of_only_retired_extensions_fails_rather_than_passing_unread() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("main.🔥"), "def main():\n    print(1)\n").expect("write");
    let root = dir.path().to_string_lossy().into_owned();
    let script = fake_mojo(dir.path());
    let args_file = dir.path().join("args.txt");
    let (code, stdout, stderr) = main_binary_env(
        &[
            "mojo",
            "format",
            "--project-root",
            &root,
            "--tool",
            script.to_string_lossy().as_ref(),
        ],
        &[("FAKE_ARGS", args_file.to_string_lossy().as_ref())],
    );
    assert_eq!(code, 1, "stdout: {stdout} stderr: {stderr}");
    assert!(stderr.contains("stopped accepting"), "stderr: {stderr}");
    assert!(
        invocations(&args_file).is_empty(),
        "the tool never saw the file"
    );
}
