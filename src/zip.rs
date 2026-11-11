// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A minimal ZIP writer: stored (uncompressed) entries, no extras, no comment.
//!
//! The Go original archives `captain-ci-test`'s `index.html` with
//! `archive/zip`'s deflate method. Deflate would mean a compressor this crate
//! has no dependency for, so the archive written here stores the entry
//! verbatim: still a valid ZIP that unzips to exactly the same bytes, just
//! larger. That is the one deliberate difference in `captain-ci-test`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::SystemTime;

/// The local-file-header signature (`PK\3\4`).
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
/// The central-directory-header signature (`PK\1\2`).
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
/// The end-of-central-directory signature (`PK\5\6`).
const END_SIGNATURE: u32 = 0x0605_4b50;
/// `PKZIP` version 2.0, the floor `archive/zip` writes.
const VERSION: u16 = 20;

/// Writes every `(entry name, source file)` pair into a ZIP at `path`.
///
/// The entry name is used verbatim, so a caller that wants the archive's
/// single top-level `index.html` simply passes the basename.
pub fn write_zip(path: &Path, entries: &[(&str, &Path)]) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    let mut out = BufWriter::new(file);
    let mut directory: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;
    for (name, source) in entries {
        let data = std::fs::read(source).map_err(|e| format!("open {}: {e}", source.display()))?;
        let crc = crc32(&data);
        let (time, date) = dos_time(std::fs::metadata(source).and_then(|m| m.modified()).ok());
        let size = data.len() as u32;
        let header = local_header(name, crc, size, time, date);
        out.write_all(&header).map_err(write_error)?;
        out.write_all(&data).map_err(write_error)?;
        directory.extend_from_slice(&central_header(name, crc, size, time, date, offset));
        offset += header.len() as u32 + size;
    }
    let directory_size = directory.len() as u32;
    out.write_all(&directory).map_err(write_error)?;
    out.write_all(&end_of_directory(entries.len(), directory_size, offset))
        .map_err(write_error)?;
    out.flush().map_err(write_error)
}

/// One local file header followed by the entry's name.
fn local_header(name: &str, crc: u32, size: u32, time: u16, date: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(30 + name.len());
    out.extend_from_slice(&LOCAL_SIGNATURE.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // general purpose flags
    out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
    out.extend_from_slice(&time.to_le_bytes());
    out.extend_from_slice(&date.to_le_bytes());
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes()); // compressed size
    out.extend_from_slice(&size.to_le_bytes()); // uncompressed size
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
    out.extend_from_slice(name.as_bytes());
    out
}

/// One central directory header followed by the entry's name.
fn central_header(name: &str, crc: u32, size: u32, time: u16, date: u16, offset: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(46 + name.len());
    out.extend_from_slice(&CENTRAL_SIGNATURE.to_le_bytes());
    out.extend_from_slice(&0x0314u16.to_le_bytes()); // made by: unix, 2.0
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // general purpose flags
    out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
    out.extend_from_slice(&time.to_le_bytes());
    out.extend_from_slice(&date.to_le_bytes());
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number start
    out.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
    out.extend_from_slice(&(0o644u32 << 16).to_le_bytes()); // external attributes
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    out
}

/// The archive's trailing end-of-central-directory record.
fn end_of_directory(count: usize, size: u32, offset: u32) -> Vec<u8> {
    let count = count as u16;
    let mut out = Vec::with_capacity(22);
    out.extend_from_slice(&END_SIGNATURE.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0u16.to_le_bytes()); // disk with the directory
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    out
}

/// The modification stamp in MS-DOS form, clamped to the format's range.
fn dos_time(modified: Option<SystemTime>) -> (u16, u16) {
    use chrono::{Datelike, TimeZone, Timelike};
    let seconds = modified
        .and_then(|value| value.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |value| value.as_secs() as i64);
    let Some(local) = chrono::Local.timestamp_opt(seconds, 0).single() else {
        return (0, 0x0021); // 1980-01-01 00:00:00, the format's floor
    };
    let year = local.year().clamp(1980, 2107) as u16;
    let date = ((year - 1980) << 9) | ((local.month() as u16) << 5) | (local.day() as u16);
    let time = ((local.hour() as u16) << 11)
        | ((local.minute() as u16) << 5)
        | ((local.second() as u16) / 2);
    (time, date)
}

/// CRC-32/ISO-HDLC, the checksum the ZIP format requires.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            // 0 or all-ones: the branchless form of `if crc & 1 == 1`.
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Turns a write failure into this module's message shape.
fn write_error(error: std::io::Error) -> String {
    format!("write: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crc_matches_the_standard_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"hello"), 0x3610_A686);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn an_archive_carries_both_headers_and_the_payload() {
        let dir = std::env::temp_dir().join(format!("duci-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let html = dir.join("index.html");
        std::fs::write(&html, b"<html>hi</html>").expect("write html");
        let archive = dir.join("cover.zip");
        write_zip(&archive, &[("index.html", &html)]).expect("zip");
        let bytes = std::fs::read(&archive).expect("read zip");
        assert_eq!(&bytes[0..4], &LOCAL_SIGNATURE.to_le_bytes());
        assert!(bytes
            .windows(4)
            .any(|w| w == CENTRAL_SIGNATURE.to_le_bytes()));
        assert_eq!(
            &bytes[bytes.len() - 22..bytes.len() - 18],
            &END_SIGNATURE.to_le_bytes()
        );
        assert!(bytes
            .windows(b"index.html".len())
            .any(|w| w == b"index.html"));
        assert!(bytes
            .windows(b"<html>hi</html>".len())
            .any(|w| w == b"<html>hi</html>"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_end_record_counts_the_entries() {
        let record = end_of_directory(1, 46 + 10 + 15, 0);
        assert_eq!(record.len(), 22);
        assert_eq!(&record[8..10], &1u16.to_le_bytes());
        assert_eq!(&record[10..12], &1u16.to_le_bytes());
    }
}
