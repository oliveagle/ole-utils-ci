// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A throwaway copy of a project's sources, for tools that only know how to work in place.
//!
//! `mojo format` has no check mode: it rewrites the files it is given. A gate cannot reformat
//! someone's working tree, so it formats a copy and reports what changed. The directory removes
//! itself on the way out, whether the gate passed, failed or panicked.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fsutil;

/// A temporary directory that deletes itself.
pub struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    /// Creates a directory nothing else will be using, tagged with the gate that asked for it.
    pub fn new(tag: &str) -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "devine-utils-ci-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        Ok(Scratch { dir })
    }

    /// Where a project-relative path lands inside the scratch tree.
    pub fn path_of(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    /// Copies one source into its place in the tree, creating the directories it needs.
    pub fn copy_in(&self, root: &Path, rel: &str) -> Result<String, String> {
        let target = self.path_of(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        std::fs::copy(root.join(rel), &target)
            .map_err(|error| format!("cannot copy {rel} to {}: {error}", target.display()))?;
        Ok(target.to_string_lossy().into_owned())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The file stem of a relative path, without its extension.
pub fn stem_of(rel: &str) -> String {
    Path::new(rel)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mojo".to_string())
}

/// The 1-based line where two versions of a file first differ.
pub fn first_difference(before: &[u8], after: &[u8]) -> usize {
    let before = String::from_utf8_lossy(before);
    let after = String::from_utf8_lossy(after);
    for (index, (left, right)) in before.lines().zip(after.lines()).enumerate() {
        if left != right {
            return index + 1;
        }
    }
    before.lines().count().max(after.lines().count()).max(1)
}

/// Whether a copy of a file differs from the original on disk.
pub fn changed(original: &Path, copy: &Path) -> bool {
    fsutil::read_file(original).unwrap_or_default() != fsutil::read_file(copy).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_change_is_the_line_that_moved() {
        assert_eq!(first_difference(b"def f( x ):\n", b"def f(x):\n"), 1);
        assert_eq!(
            first_difference(b"def f():\n    pass\n", b"def f():\n    return 1\n"),
            2
        );
    }

    #[test]
    fn a_scratch_copy_is_removed_with_the_scratch() {
        let root = tempfile::tempdir().expect("root");
        std::fs::write(root.path().join("a.mojo"), "def main():\n    pass\n").expect("write");
        let copy = {
            let scratch = Scratch::new("unit").expect("scratch");
            let copy = scratch.copy_in(root.path(), "a.mojo").expect("copy");
            assert!(Path::new(&copy).is_file());
            copy
        };
        assert!(
            !Path::new(&copy).exists(),
            "the scratch tree outlived its guard"
        );
    }
}
