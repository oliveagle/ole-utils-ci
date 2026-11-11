// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

use super::query::{parse_clog_xml, parse_from_flag, parse_time};
use super::*;

fn config() -> LogsConfig {
    LogsConfig {
        app_id: 100075290,
        token: "tok".to_string(),
        from: "10m".to_string(),
        to: String::new(),
        limit: 100,
        level: "1".to_string(),
        title: String::new(),
        url: "http://example/x".to_string(),
        follow: false,
        interval: 5_000_000_000,
        output: "text".to_string(),
    }
}

#[test]
fn the_usage_block_is_the_go_one() {
    let usage = flags::build_flags("").render_usage("logs");
    let expected = [
        "Usage of logs:",
        "  -app-id int",
        "    \t应用 ID (默认: 100075290 = devine-fe) (default 100075290)",
        "  -f\t持续 tail 模式 (简写)",
        "  -follow",
        "    \t持续 tail 模式 (轮询)",
        "  -from string",
        "    \t查询时间范围 (如 10m, 1h, 30s) (default \"10m\")",
        "  -interval duration",
        "    \ttail 轮询间隔 (如 5s, 10s) (default 5s)",
        "  -level string",
        "    \t日志级别 0=DEBUG 1=INFO 2=WARN 3=ERROR 4=FATAL (default \"1\")",
        "  -limit int",
        "    \t最大返回条数 (default 100)",
        "  -output string",
        "    \t输出格式: text|json (default \"text\")",
        "  -title string",
        "    \t标题过滤 (like 模式, 如 *MatchTripRecord*)",
        "  -to string",
        "    \t结束时间 (默认: 当前时间, 格式: 2006-01-02 15:04:05)",
        "  -token string",
        "    \tOSG token (或设置 OSG_CLOG_TOKEN 环境变量)",
        "  -url string",
        "    \tOSG getClogPost 端点地址 (default \"http://osg.ops.ctripcorp.com/api/getClogPost\")",
    ]
    .join("\n");
    assert_eq!(usage, format!("{expected}\n"));
}

#[test]
fn a_token_default_shows_up_in_the_usage_block() {
    let usage = flags::build_flags("sekret").render_usage("logs");
    assert!(
        usage.contains("OSG token (或设置 OSG_CLOG_TOKEN 环境变量) (default \"sekret\")"),
        "{usage}"
    );
}

#[test]
fn the_body_matches_go_byte_for_byte() {
    let cfg = config();
    let from = parse_time("2026-11-11 10:00:00");
    let to = parse_time("2026-11-11 11:00:00");
    let body = query::build_request_body(&cfg, &from, &to);
    assert_eq!(
        String::from_utf8_lossy(&body),
        concat!(
            r#"{"access_token":"tok","request_body":{"appId":100075290,"#,
            r#""fromDate":"2026-11-11 10:00:00","limit":100,"logLevel":"1","#,
            r#""queryLargeFields":true,"toDate":"2026-11-11 11:00:00"}}"#
        )
    );
}

#[test]
fn a_title_is_added_in_sorted_position_and_html_escaped() {
    let mut cfg = config();
    cfg.title = "a&b<*c*>".to_string();
    let from = parse_time("2026-11-11 10:00:00");
    let body = String::from_utf8_lossy(&query::build_request_body(&cfg, &from, &from)).to_string();
    assert!(
        body.contains(r#""title":"a\u0026b\u003c*c*\u003e","toDate""#),
        "{body}"
    );
}

#[test]
fn the_response_root_yields_its_nested_logs() {
    let raw = br#"<response><logs>
        <log><appId>7</appId><title>t</title><logTime>2026-11-11 12:00:00,1</logTime>
        <logLevel>1</logLevel><content>c</content></log>
        </logs><size>1</size><hasMoreResult>false</hasMoreResult></response>"#;
    let entries = parse_clog_xml(raw);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].app_id, "7");
    assert_eq!(entries[0].content, "c");
}

#[test]
fn the_logs_root_is_a_second_shape() {
    let raw = b"<logs><log><appId>8</appId><content>x</content></log></logs>";
    let entries = parse_clog_xml(raw);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].app_id, "8");
}

#[test]
fn the_flat_entries_shape_is_a_third() {
    let raw = b"<log><Entries><appId>9</appId><content>y</content></Entries></log>";
    let entries = parse_clog_xml(raw);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].app_id, "9");
}

#[test]
fn every_unparsable_body_means_no_entries() {
    for raw in [
        &b"not xml at all"[..],
        b"<response></response>",
        b"<unknown><log/></unknown>",
        b"",
    ] {
        assert!(parse_clog_xml(raw).is_empty(), "{raw:?}");
    }
}

#[test]
fn the_text_rendering_keeps_two_spaces_and_drops_the_millis() {
    let entry = super::query::ClogEntry {
        log_time: "2026-11-11 12:34:56,789".to_string(),
        log_level: "1".to_string(),
        title: "MatchTripRecord".to_string(),
        content: "order ok".to_string(),
        ..Default::default()
    };
    assert_eq!(
        entry.render(),
        "2026-11-11 12:34:56 [1] MatchTripRecord  order ok"
    );
}

#[test]
fn a_short_log_time_is_not_padded_or_cut() {
    let entry = super::query::ClogEntry {
        log_time: "12:00".to_string(),
        ..Default::default()
    };
    assert_eq!(entry.render(), "12:00 []   ");
}

#[test]
fn the_json_rendering_omits_the_empty_tail_fields() {
    let entry = super::query::ClogEntry {
        app_id: "1".to_string(),
        title: "t".to_string(),
        log_time: "lt".to_string(),
        log_level: "2".to_string(),
        content: "c".to_string(),
        ..Default::default()
    };
    assert_eq!(
        entry.to_json(),
        r#"{"appId":"1","title":"t","logTime":"lt","logLevel":"2","content":"c"}"#
    );
}

#[test]
fn the_json_rendering_keeps_the_tail_fields_when_present() {
    let entry = super::query::ClogEntry {
        app_id: "1".to_string(),
        title: "t".to_string(),
        log_time: "lt".to_string(),
        log_level: "2".to_string(),
        content: "c".to_string(),
        message_id: "m".to_string(),
        service_code: "s".to_string(),
        operation: "o".to_string(),
    };
    assert_eq!(
        entry.to_json(),
        concat!(
            r#"{"appId":"1","title":"t","logTime":"lt","logLevel":"2","content":"c","#,
            r#""messageId":"m","serviceCode":"s","operationName":"o"}"#
        )
    );
}

#[test]
fn from_flag_accepts_go_durations_and_falls_back_to_ten_minutes() {
    let before = super::query::now();
    let ten = parse_from_flag("10m");
    let fallback = parse_from_flag("nonsense");
    let after = super::query::now();
    let window = chrono::Duration::minutes(10);
    // `now - 10m` for some `now` between the two probes.
    assert!(ten >= before - window, "{ten} {before}");
    assert!(ten <= after - window, "{ten} {after}");
    // A value that is not a duration lands in the same place.
    assert!(fallback >= before - window, "{fallback} {before}");
    assert!(fallback <= after - window, "{fallback} {after}");
}

#[test]
fn to_flag_reads_all_three_layouts_and_defaults_to_now() {
    let space = parse_time("2026-11-11 10:00:00");
    assert_eq!(space.to_string(), "2026-11-11 10:00:00 +00:00");
    let tee = parse_time("2026-11-11T10:00:00");
    assert_eq!(tee.to_string(), "2026-11-11 10:00:00 +00:00");
    let rfc = parse_time("2026-11-11T10:00:00+08:00");
    assert_eq!(rfc.to_string(), "2026-11-11 10:00:00 +08:00");
    let fallback = parse_time("not a time");
    assert!((fallback - super::query::now()).num_seconds().abs() < 5);
}

#[test]
fn follow_is_a_boolean_alias_with_last_one_wins() {
    let cfg = flags::parse(&["-f".to_string()]).unwrap();
    assert!(cfg.follow);
    let cfg = flags::parse(&["--follow=false".to_string(), "-f".to_string()]).unwrap();
    assert!(cfg.follow);
    let cfg = flags::parse(&["-f".to_string(), "--follow=false".to_string()]).unwrap();
    assert!(!cfg.follow);
}

#[test]
fn the_defaults_are_the_go_ones() {
    let cfg = flags::parse(&[]).unwrap();
    assert_eq!(cfg.app_id, 100075290);
    assert_eq!(cfg.from, "10m");
    assert_eq!(cfg.limit, 100);
    assert_eq!(cfg.level, "1");
    assert_eq!(cfg.url, "http://osg.ops.ctripcorp.com/api/getClogPost");
    assert_eq!(cfg.interval, 5_000_000_000);
    assert_eq!(cfg.output, "text");
    assert!(!cfg.follow);
    assert!(cfg.to.is_empty());
    assert!(cfg.title.is_empty());
}

#[test]
fn a_missing_token_is_reported_the_go_way() {
    let mut cfg = config();
    cfg.token = String::new();
    assert_eq!(
        run(&cfg).unwrap_err(),
        "OSG token required: set OSG_CLOG_TOKEN or pass --token"
    );
}

#[test]
fn help_exits_zero_without_an_error() {
    assert_eq!(logs(&["-h".to_string()]).unwrap(), 0);
}

#[test]
fn an_unknown_flag_is_refused() {
    let error = logs(&["--bogus".to_string()]).unwrap_err();
    assert_eq!(error, "flag provided but not defined: -bogus");
}
