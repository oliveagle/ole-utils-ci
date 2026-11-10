// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The global usage block.
//!
//! Go's `flag` package prints this block to stderr whenever the shared
//! flag set refuses a command line — after the error line — and by itself
//! for `-h`. It is reproduced verbatim so a bad flag produces
//! byte-identical output.

/// The `Usage of devine-utils-ci:` block, exactly as Go's
/// `PrintDefaults` renders it.
pub const GLOBAL_USAGE: &str = r#"Usage of devine-utils-ci:
  -base-branch string
    	增量 diff 的基础分支 (默认: main) (default "main")
  -branch-coverage-threshold int
    	分支覆盖率阈值百分比 (默认: 80) (default 80)
  -build-tags string
    	构建标签 (如: test_no_credis,test_no_mongo)
  -complexity-warn int
    	圈复杂度告警阈值 (默认: 15, 不得高于 --max-complexity) (default 15)
  -coverage-allowlist value
    	允许低于覆盖率阈值的包名 (逗号分隔) (default [])
  -coverage-html string
    	覆盖率 HTML 报告路径 (default "coverage.html")
  -coverage-json string
    	覆盖率 JSON 报告路径 (default "coverage.json")
  -coverage-profile string
    	使用已有的 coverprofile 文件 (跳过 go test, 直接检查阈值)
  -coverage-threshold int
    	覆盖率阈值百分比 (default 80)
  -covermode string
    	go test -covermode 值: set|atomic|count (默认: set, 不需要 CGO; atomic 需要 CGO 编译器) (default "set")
  -dry-run
    	dry-run 模式
  -include-generated
    	coverage 门禁默认过滤 protoc/stringer/mockgen 等生成的代码 (*.pb.go / "Code generated ... DO NOT EDIT." sentinel). 加此标志重新包含它们 (通常只在调试过滤器时用).
  -junit string
    	JUnit XML 输出路径 (default "junit.xml")
  -max-complexity int
    	圈复杂度阈值 (默认: 20, 只能降低不可提高) (default 20)
  -max-file-lines int
    	源码/脚本文件硬上限 (默认: 500, 只能降低不可提高; 是 HTML/声明式 profile 的可选全局上限) (default 500)
  -mode string
    	质量门禁模式: dev (增量) / release (全量) (default "release")
  -n	dry-run 模式 (shorthand)
  -o string
    	--output 的缩写
  -output string
    	输出格式: toon (默认, agent 友好, 紧凑) | json (pretty, canonical, jq 友好) | text (人类可读, legacy). 见 AGENTS.md §9
  -parallel int
    	go test -parallel N (默认: 0 = GOMAXPROCS)
  -project-root string
    	项目根目录 (default ".")
  -release-model string
    	发布模型: simple (tag on main) / strict (main→MR→release→tag) (default "simple")
  -v	详细输出 (shorthand)
  -verbose
    	详细输出
"#;
