// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the help text: the config path is substituted, every
//! subcommand has its own block, and an unknown name falls back to the
//! general one.

use super::*;

#[test]
fn general_text_substitutes_the_user_config_path() {
    let text = general_text();
    assert!(!text.contains("$CONFIG$"), "placeholder must be gone");
    assert!(text.contains(DEFAULT_WORKFLOW_CONFIG_PATH), "{text}");
    assert!(text.starts_with("devine-utils-ci workflow — codify"));
}

#[test]
fn each_subcommand_has_its_own_block() {
    let general = general_text();
    for sub in super::super::SUBCOMMANDS {
        let text = usage_text(sub);
        assert_ne!(text, general, "{sub} should not print the general block");
        assert!(
            text.starts_with(&format!("workflow {sub} — ")),
            "{sub} block starts with: {}",
            text.lines().next().unwrap_or("")
        );
    }
}

#[test]
fn an_unknown_or_empty_name_falls_back_to_the_general_block() {
    assert_eq!(usage_text(""), general_text());
    assert_eq!(usage_text("no-such-thing"), general_text());
}

#[test]
fn print_usage_writes_the_selected_block() {
    let mut buffer: Vec<u8> = Vec::new();
    print_usage(&mut buffer, "promote");
    let written = String::from_utf8(buffer).unwrap();
    assert_eq!(written, usage_text("promote"));
    assert!(written.contains("没有完成集成测试 01"));
}
