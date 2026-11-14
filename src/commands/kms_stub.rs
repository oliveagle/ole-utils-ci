// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The `kms-stub` data model and its HTTP face.
//!
//! Production binaries may not read credentials from env or files; the
//! runtime resolves every secret through KMS. That makes local development
//! awkward, so this stub speaks the two endpoints the credential layer uses:
//!
//! ```text
//! POST /query-pwd  {appid, token, type:"JSON"} -> {code:0, result:{...}}
//! POST /query-key  {appid, token}              -> {code:0, result:{...}}
//! ```
//!
//! The bind policy lives in [`validate_listen`]: a stub reachable from a
//! routable interface is a credential-harvesting service in a shared
//! cluster, so the refusal happens before the socket exists. The listener
//! itself is [`super::kms_stub_cmd`].

use std::collections::BTreeMap;
use std::io::Read;
use std::net::IpAddr;
use std::path::Path;

use serde::Deserialize;

/// One `/query-pwd` record in the stub data file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct KmsStubPwd {
    /// The account the record belongs to.
    #[serde(default)]
    pub account: String,
    /// A human label for the account.
    #[serde(default)]
    pub title: String,
    /// The secret itself, usually a JSON document.
    #[serde(default)]
    pub value: String,
}

/// One `/query-key` record in the stub data file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct KmsStubKey {
    /// The key name the runtime asks for.
    #[serde(default)]
    pub name: String,
    /// The key type, e.g. `RSA_2048_PRI`.
    #[serde(default, rename = "type")]
    pub kind: String,
    /// The PEM (or other) key material.
    #[serde(default)]
    pub value: String,
    /// An optional structured form of [`KmsStubKey::value`].
    #[serde(default, rename = "value_json", deserialize_with = "opt_map_or_null")]
    pub value_json: Option<BTreeMap<String, String>>,
}

/// The whole stub file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct KmsStubData {
    /// The app id the runtime reports; empty means "do not check".
    #[serde(default, rename = "appid")]
    pub app_id: String,
    /// The herald token the runtime sends; empty means "do not check".
    #[serde(default, rename = "herald_token")]
    pub herald_token: String,
    /// Records served by `/query-pwd`, keyed by token.
    #[serde(default, rename = "pwd", deserialize_with = "map_or_null")]
    pub pwd: BTreeMap<String, KmsStubPwd>,
    /// Records served by `/query-key`, keyed by token.
    #[serde(default, rename = "key", deserialize_with = "map_or_null")]
    pub key: BTreeMap<String, KmsStubKey>,
}

/// Reads an optional YAML mapping that may also be absent or `null`.
fn opt_map_or_null<'de, D, T>(deserializer: D) -> Result<Option<BTreeMap<String, T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<BTreeMap<String, T>>::deserialize(deserializer)
}

/// Reads a YAML mapping that may also be absent or `null`.
///
/// A stub file that says `pwd:` with nothing after it declares no records;
/// failing to parse it would report the wrong problem.
fn map_or_null<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<BTreeMap<String, T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// Reads the stub file.
///
/// A missing file is an error: starting an empty stub would answer every
/// lookup with "unknown token" and look like a working KMS.
pub fn load(path: &Path) -> Result<KmsStubData, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("read stub data: open {}: {}", path.display(), syscall(&e)))?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)
        .map_err(|e| format!("read stub data: read {}: {}", path.display(), syscall(&e)))?;
    let data: KmsStubData =
        serde_norway::from_slice(&raw).map_err(|e| format!("parse stub data: {e}"))?;
    if data.pwd.is_empty() && data.key.is_empty() {
        return Err("stub data declares no pwd or key records".to_string());
    }
    Ok(data)
}

/// Formats an I/O error the way Go's `syscall.Errno` does.
fn syscall(error: &std::io::Error) -> String {
    let text = error.to_string();
    let reason = text.split(" (os error").next().unwrap_or(&text);
    let mut chars = reason.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_lowercase(), chars.as_str()),
        None => String::new(),
    }
}

/// Splits `host:port` the way Go's `net.SplitHostPort` does, including its
/// error wording, because the refusal message quotes it verbatim.
fn split_host_port(address: &str) -> Result<(&str, &str), String> {
    fn addr_error(address: &str, why: &str) -> String {
        if address.is_empty() {
            why.to_string()
        } else {
            format!("address {address}: {why}")
        }
    }

    let Some(colon) = address.rfind(':') else {
        return Err(addr_error(address, "missing port in address"));
    };
    let (mut from, mut to) = (0usize, 0usize);
    let host;
    if address.as_bytes()[0] == b'[' {
        let Some(end) = address.find(']') else {
            return Err(addr_error(address, "missing ']' in address"));
        };
        match end + 1 {
            n if n == address.len() => return Err(addr_error(address, "missing port in address")),
            n if n == colon => {}
            _ => {
                if address.as_bytes().get(end + 1) == Some(&b':') {
                    return Err(addr_error(address, "too many colons in address"));
                }
                return Err(addr_error(address, "missing port in address"));
            }
        }
        host = &address[1..end];
        from = 1;
        to = end + 1;
    } else {
        host = &address[..colon];
        if host.contains(':') {
            return Err(addr_error(address, "too many colons in address"));
        }
    }
    if address[from..].contains('[') {
        return Err(addr_error(address, "unexpected '[' in address"));
    }
    if address[to..].contains(']') {
        return Err(addr_error(address, "unexpected ']' in address"));
    }
    Ok((host, &address[colon + 1..]))
}

/// Whether an address may be bound: loopback only.
pub fn validate_listen(address: &str) -> Result<(), String> {
    let (host, _) = split_host_port(address).map_err(|e| format!("--listen {address:?}: {e}"))?;
    if host.is_empty() {
        return Err(format!(
            "--listen {address:?}: an address (127.0.0.1) is required"
        ));
    }
    if host.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }
    if host.parse::<IpAddr>().is_ok_and(is_loopback) {
        return Ok(());
    }
    Err(format!(
        "--listen {address:?}: kms-stub only binds a loopback address"
    ))
}

/// Whether an address is loopback, treating IPv4-mapped IPv6 the way Go's
/// `net.IP.IsLoopback` does.
fn is_loopback(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}
