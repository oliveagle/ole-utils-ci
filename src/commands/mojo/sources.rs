// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! What the files in a Mojo project are, so a gate can ask for the shape it needs.
//!
//! There is no single "list of sources" the Mojo gates can share. In the CLI Mojo actually
//! ships (1.2), `mojo build` and `mojo doc` each take exactly one input, `precompile` (the
//! command `package` became) takes one *package directory*, and `mojo test` does not exist at
//! all — a test is a file you run. Each gate below therefore starts from the same walk and picks
//! the set the tool accepts, instead of handing every source to a command that only reads one.

use std::collections::BTreeSet;
use std::path::Path;

use crate::fsutil;

/// The extension the toolchain compiles.
pub const MOJO_EXTENSION: &str = ".mojo";

/// The extension Mojo used to accept as a synonym for `.mojo`.
///
/// It is not a synonym any more: in 1.2 the CLI answers "Extension no longer supported (`.🔥`)"
/// and its formatter calls the file "invalid input". A source that still carries it is therefore
/// reported to the caller rather than handed to a tool that refuses it — a file the toolchain
/// will never read is not a file this gate can check.
pub const LEGACY_EXTENSION: &str = ".🔥";

/// The package manifest, the one file that makes a directory a Mojo package.
const PACKAGE_MANIFEST: &str = "__init__.mojo";

/// What the walk found in the project.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Project {
    /// Every `.mojo` module, relative and sorted.
    pub modules: Vec<String>,
    /// Files that still use the retired `.🔥` extension.
    pub legacy: Vec<String>,
    /// Directories holding an `__init__.mojo`, relative and sorted: what `precompile` and `doc`
    /// accept as one input.
    pub packages: Vec<String>,
}

impl Project {
    /// Walks the project root once and sorts what it found.
    pub fn read(root: &Path) -> Self {
        let mut modules = Vec::new();
        let mut legacy = Vec::new();
        let mut packages = BTreeSet::new();
        for entry in fsutil::walk_files(root) {
            let extension = entry.extension();
            if extension == LEGACY_EXTENSION {
                legacy.push(entry.rel);
                continue;
            }
            if extension != MOJO_EXTENSION {
                continue;
            }
            if Path::new(&entry.rel)
                .file_name()
                .map(|name| name == PACKAGE_MANIFEST)
                == Some(true)
            {
                packages.insert(package_dir_of(&entry.rel));
            }
            modules.push(entry.rel);
        }
        modules.sort();
        legacy.sort();
        Project {
            modules,
            legacy,
            packages: packages.into_iter().collect(),
        }
    }

    /// The modules `mojo build` can take: only a file with a `main` compiles to a program.
    ///
    /// "module does not contain a 'main' function" is what the tool says about everything else,
    /// so a library module is not a build failure — it is not a build target.
    pub fn entry_points(&self, root: &Path) -> Vec<String> {
        self.modules
            .iter()
            .filter(|rel| {
                fsutil::read_file(&root.join(rel.as_str()))
                    .map(|bytes| declares_main(&String::from_utf8_lossy(&bytes)))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// The files that look like tests under either naming convention.
    ///
    /// Both spellings are in the wild (`test_*.mojo` and `*_test.mojo`), and neither is enforced
    /// by the compiler — which is exactly why the gate takes `--paths` as well.
    pub fn tests(&self) -> Vec<String> {
        self.modules
            .iter()
            .filter(|rel| is_test_file(rel))
            .cloned()
            .collect()
    }
}

/// The directory a package manifest belongs to, relative to the root.
fn package_dir_of(rel: &str) -> String {
    match Path::new(rel).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().into_owned(),
        _ => ".".to_string(),
    }
}

/// Whether a file name follows the test convention.
fn is_test_file(rel: &str) -> bool {
    let name = Path::new(rel)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = name.strip_suffix(MOJO_EXTENSION).unwrap_or(&name);
    stem.starts_with("test_") || stem.ends_with("_test")
}

/// Whether a source declares the entry point `mojo build` insists on.
///
/// `def` is the keyword since `fn` was removed; both are accepted here because a file written for
/// an older Mojo is still a file someone has to fix rather than one this gate should ignore.
pub fn declares_main(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_start();
        let rest = line
            .strip_prefix("def ")
            .or_else(|| line.strip_prefix("fn "))
            .unwrap_or_default();
        rest.starts_with("main(") || rest.starts_with("main (")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_main_is_declared_with_def_or_fn() {
        assert!(declares_main("def main():\n    print(1)\n"));
        assert!(declares_main("def main() raises:\n    pass\n"));
        assert!(declares_main("fn main():\n    pass\n"));
        assert!(!declares_main("def twice(x: Int) -> Int:\n    return x\n"));
        assert!(!declares_main("# def main():\n"));
    }

    #[test]
    fn both_test_spellings_count() {
        assert!(is_test_file("tests/test_math.mojo"));
        assert!(is_test_file("src/math_test.mojo"));
        assert!(!is_test_file("src/math.mojo"));
        assert!(!is_test_file("src/testing.mojo"));
    }

    #[test]
    fn a_package_is_the_directory_holding_its_manifest() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("src/pkg")).expect("mkdir");
        std::fs::write(
            dir.path().join("src/pkg/__init__.mojo"),
            "def f():\n    pass\n",
        )
        .expect("write");
        std::fs::write(dir.path().join("src/main.mojo"), "def main():\n    pass\n").expect("write");
        std::fs::write(dir.path().join("src/old.🔥"), "def g():\n    pass\n").expect("write");
        let root = dir.path();
        let project = Project::read(root);
        assert_eq!(
            project.modules,
            vec!["src/main.mojo", "src/pkg/__init__.mojo"]
        );
        assert_eq!(project.packages, vec!["src/pkg"]);
        assert_eq!(project.legacy, vec!["src/old.🔥"]);
        assert_eq!(project.entry_points(root), vec!["src/main.mojo"]);
    }
}
