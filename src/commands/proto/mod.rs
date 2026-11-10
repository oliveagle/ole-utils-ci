// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The proto toolchain commands.

pub mod breaking;
pub mod generate;
pub mod lint;

pub use breaking::buf_breaking;
pub use generate::buf_generate;
pub use lint::buf_lint;
