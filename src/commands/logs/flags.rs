// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The private flag set of `logs`.
//!
//! Names, order, defaults and the Chinese usage text are the Go original's,
//! because `flag.PrintDefaults` renders them verbatim in the `-h` block and
//! in every bad-flag refusal.

use crate::cli::{report_parse_error, FlagSet, ParseError};

use super::LogsConfig;

/// `DefaultLogsConfig`'s app id: devine-fe.
const DEFAULT_APP_ID: i64 = 100075290;
/// `DefaultLogsConfig`'s endpoint.
const DEFAULT_URL: &str = "http://osg.ops.ctripcorp.com/api/getClogPost";
/// `DefaultLogsConfig`'s poll interval, in nanoseconds.
const DEFAULT_INTERVAL_NANOS: i64 = 5_000_000_000;
/// `DefaultLogsConfig`'s page size.
const DEFAULT_LIMIT: i64 = 100;

/// The flag set `runLogs` registers.
pub(super) fn build_flags(token: &str) -> FlagSet {
    let mut flags = FlagSet::new();
    flags.int_flag("app-id", DEFAULT_APP_ID);
    flags.str_flag("token", token);
    flags.str_flag("from", "10m");
    flags.str_flag("to", "");
    flags.int_flag("limit", DEFAULT_LIMIT);
    flags.str_flag("level", "1");
    flags.str_flag("title", "");
    flags.str_flag("url", DEFAULT_URL);
    flags.bool_flag("follow", false);
    // `-f` and `-follow` write ONE Go variable, so the last one on the
    // command line wins. An alias reproduces that exactly, while registering
    // `f` separately keeps it in the `PrintDefaults` block.
    flags.bool_flag("f", false);
    flags.alias("f", "follow");
    flags.duration_flag("interval", "5s");
    flags.str_flag("output", "text");

    flags.describe("app-id", "应用 ID (默认: 100075290 = devine-fe)");
    flags.describe("token", "OSG token (或设置 OSG_CLOG_TOKEN 环境变量)");
    flags.describe("from", "查询时间范围 (如 10m, 1h, 30s)");
    flags.describe("to", "结束时间 (默认: 当前时间, 格式: 2006-01-02 15:04:05)");
    flags.describe("limit", "最大返回条数");
    flags.describe("level", "日志级别 0=DEBUG 1=INFO 2=WARN 3=ERROR 4=FATAL");
    flags.describe("title", "标题过滤 (like 模式, 如 *MatchTripRecord*)");
    flags.describe("url", "OSG getClogPost 端点地址");
    flags.describe("follow", "持续 tail 模式 (轮询)");
    flags.describe("f", "持续 tail 模式 (简写)");
    flags.describe("interval", "tail 轮询间隔 (如 5s, 10s)");
    flags.describe("output", "输出格式: text|json");
    flags
}

/// One string flag's value; the flag set always holds the default.
fn text(flags: &FlagSet, name: &str) -> String {
    flags
        .get(name)
        .map(|value| value.as_str(""))
        .unwrap_or_default()
}

/// One integer flag's value. The flag set always holds the default, so the
/// fallback is only here to keep the accessor total.
fn number(flags: &FlagSet, name: &str, default: i64) -> i64 {
    flags
        .get(name)
        .map_or(default, |value| value.as_int(default))
}

/// Parses `argv` into a [`LogsConfig`], reporting refusals the way Go does:
/// the error line and the usage block on stderr, then the error travels up so
/// `main` can add its own `Error:` line.
pub(super) fn parse(args: &[String]) -> Result<LogsConfig, ParseError> {
    let token = std::env::var("OSG_CLOG_TOKEN").unwrap_or_default();
    let mut flags = build_flags(&token);
    if let Err(error) = flags.parse(args) {
        report_parse_error(&error, &flags.render_usage("logs"));
        return Err(error);
    }
    Ok(LogsConfig {
        app_id: number(&flags, "app-id", DEFAULT_APP_ID),
        token: text(&flags, "token"),
        from: text(&flags, "from"),
        to: text(&flags, "to"),
        limit: number(&flags, "limit", DEFAULT_LIMIT),
        level: text(&flags, "level"),
        title: text(&flags, "title"),
        url: text(&flags, "url"),
        // `-f` resolves to `follow`, so this is the last one given.
        follow: flags
            .get("follow")
            .is_some_and(|value| value.as_bool(false)),
        interval: flags
            .get("interval")
            .map_or(DEFAULT_INTERVAL_NANOS, |value| {
                value.as_duration(DEFAULT_INTERVAL_NANOS)
            }),
        output: text(&flags, "output"),
    })
}
