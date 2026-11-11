// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn render(value: &RepoStatus) -> serde_json::Value {
    serde_json::to_value(value).expect("serializes")
}

#[test]
fn a_bare_answer_carries_the_project_and_the_meta_only() {
    let value = render(&RepoStatus::default());
    let object = value.as_object().expect("object");
    let keys: Vec<&String> = object.keys().collect();
    assert_eq!(keys, vec!["project", "meta"]);
}

#[test]
fn go_never_omits_a_struct_timestamp() {
    // `time.Time` is a struct, so `omitempty` cannot drop it; Go prints the
    // zero instant instead.
    let value = render(&RepoStatus {
        pipelines: vec![PipelineInfo::default()],
        ..RepoStatus::default()
    });
    let pipeline = &value["pipelines"][0];
    assert_eq!(pipeline["created_at"], "0001-01-01T00:00:00Z");
    assert_eq!(pipeline["updated_at"], "0001-01-01T00:00:00Z");
}

#[test]
fn a_pointer_timestamp_is_omitted_when_absent() {
    let value = render(&RepoStatus {
        pipelines: vec![PipelineInfo {
            jobs: vec![JobInfo::default()],
            ..PipelineInfo::default()
        }],
        ..RepoStatus::default()
    });
    let job = &value["pipelines"][0]["jobs"][0];
    assert!(job.get("created_at").is_none());
    assert!(job.get("started_at").is_none());
    assert!(job.get("finished_at").is_none());
}

#[test]
fn an_integral_duration_prints_without_a_fraction() {
    let value = render(&RepoStatus {
        pipelines: vec![PipelineInfo {
            duration_seconds: 600.0,
            ..PipelineInfo::default()
        }],
        ..RepoStatus::default()
    });
    let text = serde_json::to_string(&value).expect("serializes");
    assert!(text.contains("\"duration_seconds\":600"), "{text}");
}

#[test]
fn a_zero_duration_is_omitted() {
    let value = render(&RepoStatus {
        pipelines: vec![PipelineInfo::default()],
        ..RepoStatus::default()
    });
    assert!(value["pipelines"][0].get("duration_seconds").is_none());
}

#[test]
fn empty_strings_and_zero_counts_are_omitted() {
    let value = render(&RepoStatus {
        project: ProjectInfo::default(),
        ..RepoStatus::default()
    });
    let project = value["project"].as_object().expect("object");
    assert_eq!(project.len(), 1, "only the path survives: {project:?}");
}

#[test]
fn the_release_timestamp_is_always_present() {
    let value = render(&RepoStatus {
        latest_tag: Some(LatestTag::default()),
        ..RepoStatus::default()
    });
    assert_eq!(value["latest_tag"]["released_at"], "0001-01-01T00:00:00Z");
    assert!(value["latest_tag"].get("tag_name").is_none());
}
