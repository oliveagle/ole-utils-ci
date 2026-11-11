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
mod entities;
mod reader;

use reader::Cursor;

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
