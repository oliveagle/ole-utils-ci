// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The column geometry of the dashboard.
//!
//! The header row and the body rows share one column list, so the two
//! always stay in lockstep.

use super::ansi;

/// One column of the dashboard table.
///
/// The width is the visible-cell budget; ANSI escapes do not count.
pub(super) struct Col {
    /// The column header.
    pub(super) title: &'static str,
    /// The visible width of the column.
    pub(super) width: usize,
}

/// The columns of the dashboard, in order.
pub(super) fn columns(dot_count: usize) -> Vec<Col> {
    vec![
        Col {
            title: "REPO",
            width: 32,
        },
        Col {
            title: "MR/1D",
            width: 8,
        },
        // The strip is one three-character cell per pipeline, plus the
        // leading state indicator and its space.
        Col {
            title: "PIPELINE STATUS STRIP",
            width: dot_count * 3 + 2,
        },
        Col {
            title: "JOBS",
            width: 5,
        },
        Col {
            title: "CI DUR",
            width: 6,
        },
        Col {
            title: "LAST PIPE",
            width: 16,
        },
        Col {
            title: "@",
            width: 5,
        },
        Col {
            title: "AL",
            width: 2,
        },
        Col {
            title: "LATEST",
            width: 16,
        },
    ]
}

/// Pads every cell to its column width and concatenates them.
///
/// The header row and the body rows share this, so the columns stay in
/// lockstep.
pub(super) fn join_row(cols: &[Col], render: &dyn Fn(usize) -> String) -> String {
    let mut out = String::new();
    for (index, col) in cols.iter().enumerate() {
        if index > 0 {
            out.push_str(" \u{2502} ");
        }
        let mut text = render(index);
        let mut visible = ansi::visible_length(&text);
        if visible > col.width {
            text = ansi::truncate_visible(&text, col.width);
            visible = col.width;
        }
        out.push_str(&text);
        if visible < col.width {
            out.push_str(&" ".repeat(col.width - visible));
        }
    }
    out
}

/// The visible width of a whole row.
pub(super) fn row_width(cols: &[Col]) -> usize {
    let mut width = 0;
    for (index, col) in cols.iter().enumerate() {
        if index > 0 {
            width += 3;
        }
        width += col.width;
    }
    width
}
