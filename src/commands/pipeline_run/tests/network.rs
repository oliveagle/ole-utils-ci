// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The networked half of the `pipeline-run` tests: trigger, wait and the
//! error wording of every failing exchange.

use super::*;

#[test]
fn an_http_error_is_reported_without_failing_the_command() {
    let server = serve(|_| ("404 Not Found".to_string(), b"project not found".to_vec()));
    let base = format!("{}/", server.base_url);
    let code = run(
        &args(&[
            "faas/x",
            "--base-url",
            &base,
            "--token",
            "x",
            "--output",
            "text",
        ]),
        Timeouts::default(),
    )
    .expect("an HTTP error is still exit 0");
    assert_eq!(code, 0);
    let request = server.requests().into_iter().next().expect("one request");
    assert_eq!(request.method, "POST");
    assert!(!request.path.starts_with("//"), "{}", request.path);
    assert!(
        request.path.ends_with("/api/v4/projects/faas%2Fx/pipeline"),
        "{}",
        request.path
    );
    server.shutdown();
}

#[test]
fn the_request_body_sorts_its_keys_the_way_json_marshal_does() {
    let server = serve(gitlab(1, "created"));
    run(
        &args(&[
            "faas/x",
            "--base-url",
            &server.base_url,
            "--token",
            "secret",
            "--output",
            "text",
            "--var",
            "B=2",
            "--var",
            "A=1",
        ]),
        Timeouts::default(),
    )
    .expect("triggers");
    let request = server.requests().into_iter().next().expect("one request");
    assert_eq!(
        request.body,
        "{\"ref\":\"main\",\"variables\":[{\"key\":\"A\",\"value\":\"1\"},\
         {\"key\":\"B\",\"value\":\"2\"}]}"
    );
    server.shutdown();
}

#[test]
fn the_wait_loop_outlives_the_trigger_timeout() {
    let polls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&polls);
    let server = serve(move |request| {
        if request.method == "POST" {
            return (
                "201 Created".to_string(),
                b"{\"id\":4242,\"status\":\"created\",\"ref\":\"main\"}".to_vec(),
            );
        }
        let seen = counter.fetch_add(1, Ordering::SeqCst) + 1;
        if seen < 5 {
            return (
                "200 OK".to_string(),
                b"{\"id\":4242,\"status\":\"running\"}".to_vec(),
            );
        }
        (
            "200 OK".to_string(),
            b"{\"id\":4242,\"status\":\"success\"}".to_vec(),
        )
    });
    let code = run(
        &args(&[
            "faas/whatever",
            "--base-url",
            &server.base_url,
            "--token",
            "x",
            "--output",
            "text",
            "--wait",
            "--wait-timeout",
            "20",
            "--expect-success",
        ]),
        Timeouts {
            trigger: Duration::from_millis(150),
            poll_step: Duration::from_millis(50),
        },
    )
    .expect("--wait must reach success");
    assert_eq!(code, 0);
    let seen = polls.load(Ordering::SeqCst);
    assert!(seen >= 5, "polled {seen} times");
    server.shutdown();
}

#[test]
fn expect_success_turns_a_failed_pipeline_into_an_error() {
    let server = serve(gitlab(7, "failed"));
    let base = args(&[
        "faas/whatever",
        "--base-url",
        &server.base_url,
        "--token",
        "x",
        "--output",
        "text",
        "--wait",
        "--wait-timeout",
        "5",
    ]);
    let timeouts = Timeouts {
        trigger: Duration::from_secs(30),
        poll_step: Duration::from_millis(20),
    };
    assert_eq!(run(&base, timeouts).expect("without --expect-success"), 0);
    let mut with_expect = base.clone();
    with_expect.push("--expect-success".to_string());
    let error = run(&with_expect, timeouts).expect_err("with --expect-success");
    assert!(error.contains("expect-success"), "{error}");
    assert!(error.contains("\"failed\""), "{error}");
    server.shutdown();
}

#[test]
fn a_malformed_response_is_a_decode_error() {
    let server = serve(|_| ("200 OK".to_string(), b"not json".to_vec()));
    let error = run(
        &args(&[
            "faas/x",
            "--base-url",
            &server.base_url,
            "--token",
            "x",
            "--output",
            "text",
        ]),
        Timeouts::default(),
    )
    .expect_err("must refuse");
    assert!(
        error.starts_with("pipeline-run: decode response: "),
        "{error}"
    );
    assert!(error.contains("(body: not json)"), "{error}");
    server.shutdown();
}

#[test]
fn a_pipeline_that_never_finishes_times_out_with_go_wording() {
    let server = serve(gitlab(9, "running"));
    let error = run(
        &args(&[
            "faas/x",
            "--base-url",
            &server.base_url,
            "--token",
            "x",
            "--output",
            "text",
            "--wait",
            "--wait-timeout",
            "1",
        ]),
        Timeouts {
            trigger: Duration::from_secs(30),
            poll_step: Duration::from_millis(100),
        },
    )
    .expect_err("must time out");
    assert_eq!(
        error,
        "pipeline-run: wait: timeout after 1s waiting for pipeline #9"
    );
    server.shutdown();
}
