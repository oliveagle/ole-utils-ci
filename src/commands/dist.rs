// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `verify-dist` command: check that a `dist/` directory is a valid
//! SPA production build.
//!
//! The Go original validates the markers Vite, Vue and React production
//! builds always leave behind. A `dist/` without them is a placeholder or a
//! failed build, and deploying it through Go's `embed` ships a white screen.

use std::path::Path;

use crate::fsutil;

/// The `verify-dist` command.
///
/// Go reads `os.Args[2]` verbatim: there is no flag set at all, so the first
/// argument is the directory to check even when it looks like a flag.
pub fn verify_dist(args: &[String]) -> Result<i32, String> {
    let dist_dir = dist_dir(args);
    let abs =
        fsutil::absolute(Path::new(dist_dir)).map_err(|e| format!("resolve dist path: {e}"))?;
    println!("🔍 Verifying dist: {}", abs.display());
    check(&abs)?;
    Ok(0)
}

/// The directory to verify: the first argument, or `dist` when it is absent
/// or empty — exactly the Go default.
fn dist_dir(args: &[String]) -> &str {
    args.first()
        .map(String::as_str)
        .filter(|raw| !raw.is_empty())
        .unwrap_or("dist")
}

/// Whether the page carries an ES-module script tag, in either quote style.
fn has_module_marker(content: &str) -> bool {
    content.contains("type=\"module\"") || content.contains("type='module'")
}

/// Runs every check in order; the first failure is the error Go reports.
fn check(dist: &Path) -> Result<(), String> {
    let content = read_index(dist)?;
    if !has_module_marker(&content) {
        return Err(
            "dist/index.html is missing <script type=\"module\"> — this is not a valid Vite production build"
                .to_string(),
        );
    }
    let entries = asset_entries(dist)?;
    let mut has_js = false;
    let mut has_css = false;
    for entry in &entries {
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        has_js |= name.ends_with(".js");
        has_css |= name.ends_with(".css");
    }
    if !has_js {
        return Err("dist/assets/ contains no .js files — Vite build may have failed".to_string());
    }
    if !has_css {
        return Err("dist/assets/ contains no .css files — Vite build may have failed".to_string());
    }
    println!("  ✓ dist/index.html — exists, contains <script type=\"module\">");
    println!(
        "  ✓ dist/assets/ — {} files (JS: {}, CSS: {})",
        entries.len(),
        has_js,
        has_css
    );
    println!("✅ dist/ is a valid SPA production build");
    Ok(())
}

/// Reads `dist/index.html` the way `os.ReadFile` does: bytes first, so a
/// non-UTF-8 page is still searched for the ASCII markers.
fn read_index(dist: &Path) -> Result<String, String> {
    let index = dist.join("index.html");
    match std::fs::read(&index) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err("dist/index.html not found — build likely failed or dist/ is empty".to_string())
        }
        Err(e) => Err(format!("read dist/index.html: {e}")),
    }
}

/// Lists `dist/assets/`, reporting a missing directory the way Go does.
fn asset_entries(dist: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
    let assets = dist.join("assets");
    let read = match std::fs::read_dir(&assets) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(
                "dist/assets/ not found — Vite build may have failed or produced no assets"
                    .to_string(),
            )
        }
        Err(e) => return Err(format!("read dist/assets/: {e}")),
    };
    let mut entries = Vec::new();
    for entry in read {
        match entry {
            Ok(entry) => entries.push(entry),
            Err(e) => return Err(format!("read dist/assets/: {e}")),
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `dist/` tree with the given page body and asset names.
    fn fixture(body: &str, assets: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("index.html"), body).expect("index.html");
        let assets_dir = dir.path().join("assets");
        std::fs::create_dir(&assets_dir).expect("assets");
        for name in assets {
            std::fs::write(assets_dir.join(name), b"").expect("asset");
        }
        dir
    }

    #[test]
    fn the_first_argument_is_the_dist_directory() {
        assert_eq!(dist_dir(&[]), "dist");
        assert_eq!(dist_dir(&["".to_string()]), "dist");
        assert_eq!(dist_dir(&["build/out".to_string()]), "build/out");
        assert_eq!(dist_dir(&["--flag".to_string()]), "--flag");
    }

    #[test]
    fn both_quote_styles_count_as_a_module_script() {
        assert!(has_module_marker("<script type=\"module\" src=/a.js>"));
        assert!(has_module_marker("<script type='module' src=/a.js>"));
        assert!(!has_module_marker("<script src=/a.js>"));
    }

    #[test]
    fn a_valid_spa_build_passes() {
        let dir = fixture(
            "<!doctype html><script type=\"module\" src=\"/assets/app.js\">",
            &["app.js", "app.css"],
        );
        let path = dir.path().to_string_lossy().to_string();
        assert_eq!(verify_dist(&[path]), Ok(0));
    }

    #[test]
    fn a_missing_index_reports_the_go_wording() {
        let dir = fixture("", &[]);
        std::fs::remove_file(dir.path().join("index.html")).expect("remove");
        let path = dir.path().to_string_lossy().to_string();
        assert_eq!(
            verify_dist(&[path]),
            Err("dist/index.html not found — build likely failed or dist/ is empty".to_string())
        );
    }

    #[test]
    fn a_page_without_a_module_script_is_rejected() {
        let dir = fixture(
            "<!doctype html><script src=/app.js>",
            &["app.js", "app.css"],
        );
        let path = dir.path().to_string_lossy().to_string();
        assert_eq!(
            verify_dist(&[path]),
            Err(
                "dist/index.html is missing <script type=\"module\"> — this is not a valid Vite production build"
                    .to_string()
            )
        );
    }

    #[test]
    fn missing_assets_and_missing_files_are_rejected() {
        let page = "<script type=\"module\">";
        let no_assets = fixture(page, &[]);
        std::fs::remove_dir(no_assets.path().join("assets")).expect("remove assets");
        let path = no_assets.path().to_string_lossy().to_string();
        assert_eq!(
            verify_dist(&[path]),
            Err(
                "dist/assets/ not found — Vite build may have failed or produced no assets"
                    .to_string()
            )
        );

        let no_js = fixture(page, &["app.css"]);
        let path = no_js.path().to_string_lossy().to_string();
        assert_eq!(
            verify_dist(&[path]),
            Err("dist/assets/ contains no .js files — Vite build may have failed".to_string())
        );

        let no_css = fixture(page, &["app.js"]);
        let path = no_css.path().to_string_lossy().to_string();
        assert_eq!(
            verify_dist(&[path]),
            Err("dist/assets/ contains no .css files — Vite build may have failed".to_string())
        );
    }
}
