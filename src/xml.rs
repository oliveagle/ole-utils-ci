// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! A very small XML reader for the OSG Clog response.
//!
//! The Go original uses `encoding/xml`; pulling an XML crate in would cost a
//! dependency the gates cannot audit, and `logs` needs exactly one shape:
//! elements, attributes, character data, CDATA and the five predefined
//! entities. Anything stricter (a DTD, an undefined entity, mismatched
//! tags) is a parse error — which is also what `encoding/xml` reports, and
//! what `parseClogXML` turns into "no entries".

/// One parsed element.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Node {
    /// The element name, without any namespace prefix handling.
    pub name: String,
    /// Attributes in document order, values entity-decoded.
    pub attrs: Vec<(String, String)>,
    /// Child elements in document order.
    pub children: Vec<Node>,
    /// The element's own character data, entities decoded.
    pub text: String,
}

impl Node {
    /// The first direct child with `name`, if any.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|node| node.name == name)
    }

    /// Every direct child with `name`.
    pub fn children_named(&self, name: &str) -> Vec<&Node> {
        self.children
            .iter()
            .filter(|node| node.name == name)
            .collect()
    }

    /// The character data of the first child called `name`, or empty.
    pub fn child_text(&self, name: &str) -> String {
        self.child(name)
            .map(|node| node.text.clone())
            .unwrap_or_default()
    }
}

/// Parses one XML document, returning its root element.
pub fn parse(raw: &str) -> Result<Node, String> {
    let bytes = raw.as_bytes();
    let mut cursor = Cursor { bytes, pos: 0 };
    cursor.skip_space();
    while cursor.starts_with(b"<?")
        || cursor.starts_with(b"<!--")
        || cursor.starts_with(b"<!DOCTYPE")
        || cursor.starts_with(b"<!doctype")
    {
        cursor.skip_misc()?;
    }
    cursor.skip_space();
    let root = cursor.element()?;
    cursor.skip_space();
    if cursor.pos != bytes.len() {
        return Err(format!(
            "junk after document element at byte {}",
            cursor.pos
        ));
    }
    Ok(root)
}

/// A byte cursor over the document.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.bytes[self.pos..].starts_with(prefix)
    }

    fn skip_space(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    /// Skips a prolog comment, processing instruction or doctype.
    fn skip_misc(&mut self) -> Result<(), String> {
        if self.starts_with(b"<!--") {
            self.pos += 4;
            return self.skip_until(b"-->");
        }
        if self.starts_with(b"<?") {
            self.pos += 2;
            return self.skip_until(b"?>");
        }
        // Doctype: skip to the closing `>`, tolerating an internal subset.
        self.pos += 9;
        let mut depth = 0usize;
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b'[' => depth += 1,
                b']' => depth = depth.saturating_sub(1),
                b'>' if depth == 0 => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => {}
            }
            self.pos += 1;
        }
        Err("unterminated doctype".to_string())
    }

    fn skip_until(&mut self, marker: &[u8]) -> Result<(), String> {
        match self.bytes[self.pos..]
            .windows(marker.len())
            .position(|window| window == marker)
        {
            Some(offset) => {
                self.pos += offset + marker.len();
                Ok(())
            }
            None => Err("unterminated markup".to_string()),
        }
    }

    /// Parses one element, assuming the cursor sits on its `<`.
    fn element(&mut self) -> Result<Node, String> {
        if !self.starts_with(b"<") {
            return Err(format!("expected '<' at byte {}", self.pos));
        }
        self.pos += 1;
        let name = self.name()?;
        let mut node = Node {
            name,
            ..Node::default()
        };
        if !self.start_tag(&mut node)? {
            self.content(&mut node)?;
        }
        Ok(node)
    }

    /// Reads the attributes up to the tag's `>`; reports whether the tag
    /// closed itself with `/>` (in which case there is no content).
    fn start_tag(&mut self, node: &mut Node) -> Result<bool, String> {
        loop {
            self.skip_space();
            if self.pos >= self.bytes.len() {
                return Err("unterminated start tag".to_string());
            }
            if self.starts_with(b"/>") {
                self.pos += 2;
                return Ok(true);
            }
            if self.starts_with(b">") {
                self.pos += 1;
                return Ok(false);
            }
            let attr = self.name()?;
            self.skip_space();
            if !self.starts_with(b"=") {
                return Err(format!("attribute {attr} has no value"));
            }
            self.pos += 1;
            self.skip_space();
            let quote = match self.bytes.get(self.pos) {
                Some(b'"') | Some(b'\'') => self.bytes[self.pos],
                _ => return Err(format!("attribute {attr} is not quoted")),
            };
            self.pos += 1;
            let start = self.pos;
            while self.pos < self.bytes.len() && self.bytes[self.pos] != quote {
                self.pos += 1;
            }
            if self.pos >= self.bytes.len() {
                return Err(format!("attribute {attr} is unterminated"));
            }
            let value = decode(&self.bytes[start..self.pos])?;
            self.pos += 1;
            node.attrs.push((attr, value));
        }
    }

    /// Reads everything between the start tag and the matching close tag.
    fn content(&mut self, node: &mut Node) -> Result<(), String> {
        loop {
            if self.pos >= self.bytes.len() {
                return Err(format!("element {} is unterminated", node.name));
            }
            if self.starts_with(b"</") {
                return self.close_tag(node);
            }
            if self.starts_with(b"<![CDATA[") {
                self.cdata(node)?;
                continue;
            }
            if self.starts_with(b"<!--") {
                self.pos += 4;
                self.skip_until(b"-->")?;
                continue;
            }
            if self.starts_with(b"<?") {
                self.pos += 2;
                self.skip_until(b"?>")?;
                continue;
            }
            if self.starts_with(b"<!") {
                self.skip_declaration();
                continue;
            }
            if self.starts_with(b"<") {
                let child = self.element()?;
                node.children.push(child);
                continue;
            }
            self.text(node)?;
        }
    }

    /// Consumes `</name>` and checks it against the element's own name.
    fn close_tag(&mut self, node: &mut Node) -> Result<(), String> {
        self.pos += 2;
        let closing = self.name()?;
        self.skip_space();
        if !self.starts_with(b">") {
            return Err(format!("malformed close tag for {closing}"));
        }
        self.pos += 1;
        if closing != node.name {
            return Err(format!("mismatched close tag: {closing} != {}", node.name));
        }
        Ok(())
    }

    /// Consumes one `<![CDATA[ ... ]]>` section, appending its payload.
    fn cdata(&mut self, node: &mut Node) -> Result<(), String> {
        self.pos += 9;
        let start = self.pos;
        let Some(offset) = self.bytes[self.pos..]
            .windows(3)
            .position(|window| window == b"]]>")
        else {
            return Err("unterminated CDATA".to_string());
        };
        node.text.push_str(&String::from_utf8_lossy(
            &self.bytes[start..self.pos + offset],
        ));
        self.pos += offset + 3;
        Ok(())
    }

    /// Consumes an `<! ... >` declaration that is neither a comment nor
    /// CDATA, up to the first `>`.
    fn skip_declaration(&mut self) {
        self.pos += 2;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'>' {
            self.pos += 1;
        }
        self.pos += 1;
    }

    /// Consumes the character data up to the next `<`, appending it.
    fn text(&mut self, node: &mut Node) -> Result<(), String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'<' {
            self.pos += 1;
        }
        node.text.push_str(&decode(&self.bytes[start..self.pos])?);
        Ok(())
    }

    /// Reads an element or attribute name.
    fn name(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.bytes.len() {
            let byte = self.bytes[self.pos];
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.') {
                self.pos += 1;
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(format!("expected a name at byte {}", self.pos));
        }
        Ok(String::from_utf8_lossy(&self.bytes[start..self.pos]).to_string())
    }
}

/// Decodes the five predefined entities and numeric character references;
/// anything else is an error, the way `encoding/xml` is strict by default.
fn decode(raw: &[u8]) -> Result<String, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_nested_response() {
        let raw = r#"<?xml version="1.0"?>
<response size="1">
  <logs>
    <log>
      <appId>100075290</appId>
      <title>devine-fe</title>
      <content>hello &amp; goodbye</content>
      <note><![CDATA[raw <text> here]]></note>
    </log>
  </logs>
</response>"#;
        let root = parse(raw).unwrap();
        assert_eq!(root.name, "response");
        assert_eq!(root.attrs[0], ("size".to_string(), "1".to_string()));
        let logs = root.child("logs").unwrap();
        let entry = logs.child("log").unwrap();
        assert_eq!(entry.child_text("appId"), "100075290");
        assert_eq!(entry.child_text("content"), "hello & goodbye");
        assert_eq!(entry.child_text("note"), "raw <text> here");
    }

    #[test]
    fn an_undefined_entity_is_an_error_like_encoding_xml() {
        assert!(parse("<a>&nbsp;</a>").is_err());
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<a></a><b></b>").is_err());
    }

    #[test]
    fn self_closing_and_empty_elements_parse() {
        let root = parse("<logs><log/><log></log></logs>").unwrap();
        assert_eq!(root.children_named("log").len(), 2);
        assert_eq!(root.child_text("absent"), "");
    }
}
