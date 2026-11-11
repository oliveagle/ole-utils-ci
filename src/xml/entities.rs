// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Entity and character-reference decoding for the XML reader.

/// Decodes the five predefined entities and numeric character references;
/// anything else is an error, the way `encoding/xml` is strict by default.
pub(super) fn decode(raw: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(raw);
    if !text.contains('&') {
        return Ok(text.to_string());
    }
    let mut out = String::with_capacity(text.len());
    let mut rest: &str = &text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let end = tail
            .find(';')
            .ok_or_else(|| "unterminated entity".to_string())?;
        let entity = &tail[1..end];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                let decoded = code
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("invalid character entity &{entity};"))?;
                out.push(decoded);
            }
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
