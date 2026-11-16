// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Cobertura line-coverage parsing for the MoonBit gate.

/// One file's coverage.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FileCoverage {
    /// Path relative to the module root.
    pub file: String,
    /// Lines with a non-zero hit count.
    pub covered: usize,
    /// Instrumented lines.
    pub total: usize,
}

/// The whole report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Coverage {
    /// Per-file entries, sorted by path.
    pub files: Vec<FileCoverage>,
    /// Total covered lines.
    pub covered: usize,
    /// Total instrumented lines.
    pub total: usize,
}

impl Coverage {
    /// Covered-line percentage, with the usual empty-set convention.
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            self.covered as f64 * 100.0 / self.total as f64
        }
    }
}

/// Parses the Cobertura XML `moon coverage report` emits.
pub fn parse(xml: &str) -> Coverage {
    let mut coverage = Coverage::default();
    let mut rest = xml;
    while let Some(start) = rest.find("<class ") {
        let after = &rest[start + 7..];
        let Some(end) = after.find("</class>") else {
            break;
        };
        let class = &after[..end];
        let file = xml_attr(class, "filename");
        let (mut covered, mut total) = (0usize, 0usize);
        for line in class.split("<line ").skip(1) {
            let Some(end) = line.find("/>").or_else(|| line.find('>')) else {
                continue;
            };
            let tag = &line[..end];
            let hits = xml_attr(tag, "hits").parse::<usize>().unwrap_or(0);
            total += 1;
            if hits > 0 {
                covered += 1;
            }
        }
        if total > 0 {
            coverage.files.push(FileCoverage {
                file,
                covered,
                total,
            });
            coverage.covered += covered;
            coverage.total += total;
        }
        rest = &after[end + 8..];
    }
    coverage.files.sort_by(|a, b| a.file.cmp(&b.file));
    coverage
}

/// Reads one attribute from an XML tag.
fn xml_attr(tag: &str, name: &str) -> String {
    let needle = format!("{name}=\"");
    let Some(start) = tag.find(&needle) else {
        return String::new();
    };
    let after = &tag[start + needle.len()..];
    let Some(end) = after.find('"') else {
        return String::new();
    };
    after[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cobertura_line_counts() {
        let xml = r#"<coverage lines-valid="3" lines-covered="2">
  <class name="a.mbt" filename="a.mbt">
    <lines>
      <line number="1" hits="2"/>
      <line number="2" hits="0"/>
      <line number="3" hits="1"/>
    </lines>
  </class>
</coverage>"#;
        let coverage = parse(xml);
        assert_eq!(coverage.covered, 2);
        assert_eq!(coverage.total, 3);
        assert!((coverage.percent() - 66.666_666).abs() < 0.01);
        assert_eq!(coverage.files[0].file, "a.mbt");
    }
}
