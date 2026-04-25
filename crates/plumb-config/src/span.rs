//! Resolve a dotted config path to a byte span in the original source.
//!
//! TOML span recovery is precise via [`toml::de::DeTable`] (and its
//! recursive [`toml::de::DeValue`] companion), both of which preserve
//! per-key offsets. YAML and JSON span recovery is best-effort: we
//! fall back to a textual search for the leaf key.

// pub(crate) in a private mod triggers redundant_pub_crate; bare pub triggers unreachable_pub.
#![allow(clippy::redundant_pub_crate)]

use std::ops::Range;

use miette::SourceSpan;
use toml::de::{DeTable, DeValue};

/// Source format of a config file.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SourceFormat {
    /// `.toml` — span recovery is precise.
    Toml,
    /// `.yaml` / `.yml` — span recovery is best-effort.
    Yaml,
    /// `.json` — span recovery is best-effort.
    Json,
}

/// Resolve a dotted path to a byte span in `source`.
///
/// Returns `None` if span recovery isn't possible (path doesn't exist,
/// source is malformed, the format doesn't expose offsets).
pub(crate) fn locate_path(
    source: &str,
    format: SourceFormat,
    segments: &[String],
) -> Option<SourceSpan> {
    if segments.is_empty() {
        return None;
    }

    match format {
        SourceFormat::Toml => locate_in_toml(source, segments),
        SourceFormat::Yaml | SourceFormat::Json => locate_by_key_search(source, segments),
    }
}

fn locate_in_toml(source: &str, segments: &[String]) -> Option<SourceSpan> {
    let document = DeTable::parse(source).ok()?;
    walk_table(document.get_ref(), segments).map(into_source_span)
}

fn walk_table(table: &DeTable<'_>, segments: &[String]) -> Option<Range<usize>> {
    let (head, rest) = segments.split_first()?;
    let mut entry_iter = table.iter();
    let (_key, value) = entry_iter.find(|(k, _)| k.get_ref().as_ref() == head.as_str())?;
    if rest.is_empty() {
        return Some(value.span());
    }
    match value.get_ref() {
        DeValue::Table(child) => walk_table(child, rest).or_else(|| Some(value.span())),
        _ => Some(value.span()),
    }
}

fn into_source_span(range: Range<usize>) -> SourceSpan {
    let len = range.end.saturating_sub(range.start);
    (range.start, len).into()
}

/// Best-effort key-based span recovery for YAML/JSON. We can't easily
/// re-parse with offsets in those formats, so we look for the leaf
/// segment as a key in the source and return a span covering the
/// rest of that line.
fn locate_by_key_search(source: &str, segments: &[String]) -> Option<SourceSpan> {
    let leaf = segments.last()?;

    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        if let Some(idx) = find_word(line, leaf) {
            // Verify it's a key (followed by `:` or `=` after optional
            // closing quote and whitespace).
            let after = &line[idx + leaf.len()..];
            let after_trim = after.trim_start_matches('"').trim_start();
            if after_trim.starts_with(':') || after_trim.starts_with('=') {
                let line_start = offset + idx;
                let line_end = offset + line.trim_end_matches(['\n', '\r']).len();
                let len = line_end.saturating_sub(line_start);
                return Some((line_start, len).into());
            }
        }
        offset += line.len();
    }

    // Fallback: span the bare token if present.
    if let Some(idx) = find_word(source, leaf) {
        return Some((idx, leaf.len()).into());
    }
    None
}

/// Find the first occurrence of `needle` in `haystack` not embedded in
/// a larger identifier. Allows leading `"` (quoted JSON/YAML keys).
fn find_word(haystack: &str, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.is_empty() || bytes.len() < needle_bytes.len() {
        return None;
    }
    for i in 0..=bytes.len() - needle_bytes.len() {
        if &bytes[i..i + needle_bytes.len()] != needle_bytes {
            continue;
        }
        let prev_ok = i == 0
            || matches!(
                bytes[i - 1],
                b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b':' | b',' | b'{' | b'['
            );
        let next = i + needle_bytes.len();
        let next_ok = next >= bytes.len()
            || !matches!(bytes[next], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_');
        if prev_ok && next_ok {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_nested_toml_value_span() {
        let source = "\
[color.tokens]
brand = \"#0b7285\"
bg = \"not-a-hex\"
";
        let span = locate_path(
            source,
            SourceFormat::Toml,
            &["color".to_owned(), "tokens".to_owned(), "bg".to_owned()],
        )
        .expect("should locate bg");
        let start = span.offset();
        let end = start + span.len();
        let snippet = &source[start..end];
        assert!(
            snippet.contains("not-a-hex"),
            "snippet {snippet:?} should contain not-a-hex"
        );
    }

    #[test]
    fn locates_unknown_top_level_toml_key_value() {
        let source = "\
bogus_top_level = true

[viewports.mobile]
width = 320
height = 568
";
        let span = locate_path(source, SourceFormat::Toml, &["bogus_top_level".to_owned()])
            .expect("should locate top-level key");
        let start = span.offset();
        let end = start + span.len();
        let snippet = &source[start..end];
        assert!(snippet.contains("true"), "snippet {snippet:?}");
    }

    #[test]
    fn locates_yaml_leaf_via_key_search() {
        let source = "\
color:
  tokens:
    bg: not-a-hex
";
        let span = locate_path(
            source,
            SourceFormat::Yaml,
            &["color".to_owned(), "tokens".to_owned(), "bg".to_owned()],
        )
        .expect("yaml key search");
        let start = span.offset();
        let end = start + span.len();
        let snippet = &source[start..end];
        assert!(snippet.contains("bg"), "snippet {snippet:?}");
    }
}
