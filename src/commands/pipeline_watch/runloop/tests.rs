// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the interactive loop.

use super::*;

#[test]
fn the_interval_is_clamped_to_the_supported_range() {
    assert_eq!(clamp(1), MIN_INTERVAL);
    assert_eq!(clamp(5), 5);
    assert_eq!(clamp(60), 60);
    assert_eq!(clamp(500), MAX_INTERVAL);
}

#[test]
fn the_selection_stays_inside_the_row_list() {
    let mut state = State {
        rows: vec![super::super::rows::RepoRow::default()],
        selected: 0,
        interval: 10,
        per_repo: 7,
    };
    state.move_down();
    assert_eq!(state.selected, 0, "there is only one row");
    state.move_up();
    assert_eq!(state.selected, 0);
}

#[test]
fn the_selection_moves_down_and_up() {
    let mut state = State {
        rows: vec![
            super::super::rows::RepoRow::default(),
            super::super::rows::RepoRow::default(),
        ],
        selected: 0,
        interval: 10,
        per_repo: 7,
    };
    state.move_down();
    assert_eq!(state.selected, 1);
    state.move_down();
    assert_eq!(state.selected, 1, "the last row is the end");
    state.move_up();
    assert_eq!(state.selected, 0);
}
