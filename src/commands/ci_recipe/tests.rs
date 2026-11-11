// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `ci-recipe` model.

use super::*;

/// The exact script Go's `SetupScript` prints for the default recipe at
/// `v0.2.6`, captured from the reference binary.
const EXPECTED_SCRIPT: &str = r#"set -eu
mkdir -p bin
curl -fL --header "JOB-TOKEN: ${CI_JOB_TOKEN}" \
  "http://git.dev.sh.ctripcorp.com/api/v4/projects/158442/jobs/artifacts/v0.2.6/raw/dist/devine-utils-ci-linux-amd64.gz?job=release" \
  -o /tmp/devine-utils-ci.gz
gunzip -f /tmp/devine-utils-ci.gz
mv /tmp/devine-utils-ci bin/devine-utils-ci
chmod +x bin/devine-utils-ci
EXPECTED="0.2.6"
ACTUAL=$(./bin/devine-utils-ci version | awk '{print $NF}')
test "$ACTUAL" = "$EXPECTED" || { echo "version mismatch: want=$EXPECTED got=$ACTUAL" >&2; exit 1; }
"#;

#[test]
fn artifact_url_is_the_ref_based_form() {
    let recipe = CIRecipe {
        version: "v0.2.6".to_string(),
        ..CIRecipe::default()
    };
    let url = recipe.artifact_url().unwrap();
    assert_eq!(
        url,
        "http://git.dev.sh.ctripcorp.com/api/v4/projects/158442/jobs/artifacts/v0.2.6/raw/dist/devine-utils-ci-linux-amd64.gz?job=release"
    );
    assert!(url.contains("/jobs/artifacts/v0.2.6/raw/"));
    assert!(!url.contains("/jobs/188669002/artifacts/"));
}

#[test]
fn artifact_url_honours_the_overrides() {
    let recipe = CIRecipe {
        base_url: "http://x/".to_string(),
        project_id: "99".to_string(),
        version: "v0.2.6".to_string(),
        release_job: "release/*".to_string(),
        ..CIRecipe::default()
    };
    assert_eq!(
        recipe.artifact_url().unwrap(),
        "http://x/api/v4/projects/99/jobs/artifacts/v0.2.6/raw/dist/devine-utils-ci-linux-amd64.gz?job=release%2F%2A"
    );
}

#[test]
fn validate_rejects_bad_inputs_in_the_go_order() {
    let cases: [(&str, CIRecipe, &str); 7] = [
        (
            "empty version",
            CIRecipe {
                version: String::new(),
                ..CIRecipe::default()
            },
            "Version is empty",
        ),
        (
            "non-semver version",
            CIRecipe {
                version: "0.2.6".to_string(),
                ..CIRecipe::default()
            },
            "must match vX.Y.Z",
        ),
        (
            "unknown artifact",
            CIRecipe {
                version: "v0.2.6".to_string(),
                artifact: "devine-utils-ci-plan9-mips.gz".to_string(),
                ..CIRecipe::default()
            },
            "not one of",
        ),
        (
            "missing project id",
            CIRecipe {
                version: "v0.2.6".to_string(),
                project_id: String::new(),
                ..CIRecipe::default()
            },
            "ProjectID is empty",
        ),
        (
            "missing base url",
            CIRecipe {
                version: "v0.2.6".to_string(),
                base_url: String::new(),
                ..CIRecipe::default()
            },
            "BaseURL is empty",
        ),
        (
            "missing release job name",
            CIRecipe {
                version: "v0.2.6".to_string(),
                release_job: String::new(),
                ..CIRecipe::default()
            },
            "ReleaseJob is empty",
        ),
        (
            "missing artifact",
            CIRecipe {
                version: "v0.2.6".to_string(),
                artifact: String::new(),
                ..CIRecipe::default()
            },
            "Artifact is empty",
        ),
    ];
    for (name, recipe, expected) in cases {
        let error = recipe.validate().unwrap_err();
        assert!(
            error.contains(expected),
            "{name}: error {error:?} lacks {expected:?}"
        );
    }
    // The Go original reports BaseURL before every other problem.
    let recipe = CIRecipe {
        base_url: String::new(),
        ..CIRecipe::default()
    };
    assert_eq!(
        recipe.validate().unwrap_err(),
        "ci-recipe: BaseURL is empty"
    );
}

#[test]
fn version_and_artifact_errors_use_go_quoting() {
    let recipe = CIRecipe {
        version: "v1.2".to_string(),
        ..CIRecipe::default()
    };
    assert_eq!(
        recipe.validate().unwrap_err(),
        "ci-recipe: Version \"v1.2\" must match vX.Y.Z"
    );
    let recipe = CIRecipe {
        version: "v0.2.6".to_string(),
        artifact: "nope".to_string(),
        ..CIRecipe::default()
    };
    assert_eq!(
        recipe.validate().unwrap_err(),
        "ci-recipe: Artifact \"nope\" is not one of [devine-utils-ci-linux-amd64.gz \
         devine-utils-ci-linux-arm64.gz devine-utils-ci-darwin-amd64.gz \
         devine-utils-ci-darwin-arm64.gz devine-utils-ci-windows-amd64.exe.gz]"
    );
}

#[test]
fn supported_artifacts_match_the_release_job() {
    assert_eq!(
        SUPPORTED_ARTIFACTS,
        [
            "devine-utils-ci-linux-amd64.gz",
            "devine-utils-ci-linux-arm64.gz",
            "devine-utils-ci-darwin-amd64.gz",
            "devine-utils-ci-darwin-arm64.gz",
            "devine-utils-ci-windows-amd64.exe.gz",
        ]
    );
    assert_eq!(DEFAULT_ARTIFACT, SUPPORTED_ARTIFACTS[0]);
}

#[test]
fn setup_script_matches_the_go_original() {
    let recipe = CIRecipe {
        version: "v0.2.6".to_string(),
        ..CIRecipe::default()
    };
    assert_eq!(recipe.setup_script().unwrap(), EXPECTED_SCRIPT);
}

#[test]
fn setup_script_without_verification_omits_the_check() {
    let recipe = CIRecipe {
        version: "v0.2.6".to_string(),
        expect_verify: false,
        ..CIRecipe::default()
    };
    let script = recipe.setup_script().unwrap();
    assert!(!script.contains("EXPECTED="));
    assert!(!script.contains("version mismatch"));
    assert!(script.ends_with("chmod +x bin/devine-utils-ci\n"));
}

#[test]
fn an_empty_install_dir_falls_back_to_bin() {
    let recipe = CIRecipe {
        version: "v0.2.6".to_string(),
        install_dir: String::new(),
        ..CIRecipe::default()
    };
    let script = recipe.setup_script().unwrap();
    assert!(script.contains("\nmkdir -p bin\n"));
    assert!(script.contains("\nmv /tmp/devine-utils-ci bin/devine-utils-ci\n"));
}

#[test]
fn query_escape_matches_go() {
    assert_eq!(query_escape("release"), "release");
    assert_eq!(query_escape("release/*"), "release%2F%2A");
    assert_eq!(query_escape("a b"), "a+b");
    assert_eq!(query_escape("~._-"), "~._-");
    assert_eq!(query_escape("a+b:c?d=e@f"), "a%2Bb%3Ac%3Fd%3De%40f");
    assert_eq!(query_escape("it's"), "it%27s");
    assert_eq!(query_escape("é"), "%C3%A9");
}

#[test]
fn clean_and_join_follow_path_filepath() {
    assert_eq!(go_clean("a//b/"), "a/b");
    assert_eq!(go_clean("."), ".");
    assert_eq!(go_clean(""), ".");
    assert_eq!(go_clean("/tmp/../x"), "/x");
    assert_eq!(go_clean("../a/../b"), "../b");
    assert_eq!(go_dir("/a/b"), "/a");
    assert_eq!(go_dir("/a"), "/");
    assert_eq!(go_dir("a"), ".");
    assert_eq!(go_dir("/"), "/");
    assert_eq!(
        join_install("bin", "devine-utils-ci"),
        "bin/devine-utils-ci"
    );
    assert_eq!(
        join_install("/tmp/x/", "devine-utils-ci"),
        "/tmp/x/devine-utils-ci"
    );
    assert_eq!(join_install("", "devine-utils-ci"), "devine-utils-ci");
}
