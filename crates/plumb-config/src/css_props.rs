//! CSS custom-properties scraper for token discovery (e.g. `plumb init`).
//!
//! Scans each input file for `:root { ... }` blocks at the top level or
//! wrapped inside a single `@media` / `@supports` at-rule, then extracts
//! every `--foo: <value>;` declaration.
//!
//! Values are lightly typed:
//!
//! - hex literals, `rgb`/`rgba`/`hsl`/`hsla` → [`ScrapedValue::Color`]
//!   (normalized to lower-case `#rrggbb` or `#rrggbbaa`).
//! - integer/decimal `px` → [`ScrapedValue::Px`] (rounded to nearest u32).
//! - decimal `rem` → [`ScrapedValue::Rem`] (callers can fold to px at
//!   16px/rem; surfaced separately so the unit warning is recoverable).
//! - decimal `em` → [`ScrapedValue::Em`] (caller emits a warning).
//! - everything else → [`ScrapedValue::Other`] (raw, trimmed).
//!
//! The parser is hand-rolled and intentionally narrow: cssparser would
//! balloon the dep tree for what is essentially a brace-and-semicolon
//! state machine. The scope is `:root` discovery — full CSS is out.
//!
//! Comments (`/* ... */`) and quoted strings (`"…"` / `'…'`) inside
//! declarations are skipped; semicolons inside strings or comments do
//! not terminate a declaration.

// pub items inside a private mod trigger unreachable_pub; pub(crate) is
// the right scope but pedantic flags the redundancy.
#![allow(clippy::redundant_pub_crate)]

use std::fs;
use std::path::{Path, PathBuf};

use miette::{NamedSource, SourceSpan};

use crate::ConfigError;

/// One `--name: value;` declaration discovered inside a `:root` block.
#[derive(Debug, Clone, PartialEq)]
pub struct CssPropertyScrape {
    /// Source path the declaration came from.
    pub source: PathBuf,
    /// `None` for top-level `:root`. `Some("@media (...)")` (or
    /// `"@supports (...)"`) when the `:root` block was wrapped in a
    /// single at-rule. Preserves the at-rule prelude verbatim.
    pub at_rule: Option<String>,
    /// Custom-property name, e.g. `--bg-canvas`.
    pub name: String,
    /// Raw value string, trimmed but otherwise unmodified
    /// (no comment stripping, quotes preserved).
    pub raw_value: String,
    /// Light typing of `raw_value`. See module docs.
    pub value: ScrapedValue,
}

/// Light classification of a custom-property value.
#[derive(Debug, Clone, PartialEq)]
pub enum ScrapedValue {
    /// A color literal, normalized to `#rrggbb` or `#rrggbbaa`.
    Color(String),
    /// A `px` length, rounded to nearest u32.
    Px(u32),
    /// A `rem` length (caller applies 16px/rem if it wants px).
    Rem(f32),
    /// An `em` length (caller surfaces a warning — context-dependent).
    Em(f32),
    /// Anything else (font stacks, line-heights, gradients, …).
    Other(String),
}

/// Scan each path in `files` for CSS custom-properties declared inside
/// `:root` blocks. See module docs for scope and value typing.
///
/// # Errors
///
/// Returns [`ConfigError::Read`] if a path can't be read,
/// or [`ConfigError::CssParse`] if the file contains an unterminated
/// block, comment, or string. Missing-file errors come back as
/// [`ConfigError::Read`].
pub fn scrape_css_properties(files: &[PathBuf]) -> Result<Vec<CssPropertyScrape>, ConfigError> {
    let mut out = Vec::new();
    for path in files {
        let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;
        scrape_one(path, &contents, &mut out)?;
    }
    Ok(out)
}

/// Parse `contents` (already loaded from `path`) and append discovered
/// scrapes to `out`. Errors carry a miette-friendly span pointing at
/// the offending region.
fn scrape_one(
    path: &Path,
    contents: &str,
    out: &mut Vec<CssPropertyScrape>,
) -> Result<(), ConfigError> {
    let mut parser = Parser::new(contents);

    while parser
        .skip_trivia()
        .map_err(|fault| fault.into_error(path, contents))?
    {
        // Re-anchor the prelude span at the first non-trivia byte.
        let prelude_start = parser.cursor;
        let prelude = parser
            .read_prelude()
            .map_err(|fault| fault.into_error(path, contents))?;
        let prelude_trimmed = prelude.trim();

        if prelude_trimmed.is_empty() {
            // Stray `{}` or trailing whitespace — nothing to do.
            if parser.cursor < parser.bytes.len() && parser.bytes[parser.cursor] == b'{' {
                // Skip a stray block at top level.
                parser.cursor += 1;
                parser
                    .skip_block()
                    .map_err(|fault| fault.into_error(path, contents))?;
            }
            continue;
        }

        if !parser.consume_byte_eq(b'{') {
            return Err(parse_error(
                path,
                contents,
                prelude_start..parser.cursor,
                "expected `{` after selector or at-rule prelude",
            ));
        }

        if is_root_selector(prelude_trimmed) {
            collect_root_block(&mut parser, path, contents, None, out)?;
        } else if let Some(at_rule) = parse_at_rule_prelude(prelude_trimmed) {
            // We allow a single level of @media / @supports wrapping a
            // :root block. Any other at-rule (or nested rules inside
            // this one beyond a single :root) gets skipped without
            // erroring — that's tolerant by design.
            scan_at_rule_body(&mut parser, path, contents, &at_rule, out)?;
        } else {
            // Plain selector that isn't :root — skip its block.
            parser
                .skip_block()
                .map_err(|fault| fault.into_error(path, contents))?;
        }
    }

    Ok(())
}

/// True if the selector list resolves to `:root` (possibly with extra
/// whitespace). We keep this strict — `:root, html` is intentionally
/// out of scope (matches the issue spec).
fn is_root_selector(prelude: &str) -> bool {
    prelude.split_whitespace().collect::<String>() == ":root"
}

/// Recognize `@media (...)` / `@supports (...)` and return the
/// trimmed prelude (e.g. `@media (prefers-color-scheme: dark)`).
fn parse_at_rule_prelude(prelude: &str) -> Option<String> {
    let trimmed = prelude.trim();
    if !trimmed.starts_with('@') {
        return None;
    }
    let (kw, _) = trimmed
        .split_once(|c: char| c.is_ascii_whitespace() || c == '(')
        .unwrap_or((trimmed, ""));
    if kw == "@media" || kw == "@supports" {
        Some(trimmed.to_owned())
    } else {
        None
    }
}

/// We're sitting at the open brace of a `:root { ... }` block. Walk
/// every declaration inside it.
fn collect_root_block(
    parser: &mut Parser<'_>,
    path: &Path,
    contents: &str,
    at_rule: Option<&str>,
    out: &mut Vec<CssPropertyScrape>,
) -> Result<(), ConfigError> {
    let block_start = parser.cursor.saturating_sub(1);

    loop {
        let still_open = parser
            .skip_trivia()
            .map_err(|fault| fault.into_error(path, contents))?;
        if !still_open {
            return Err(parse_error(
                path,
                contents,
                block_start..contents.len(),
                "unterminated `:root` block",
            ));
        }
        if parser.consume_byte_eq(b'}') {
            return Ok(());
        }

        let decl_start = parser.cursor;
        let (name, raw_value) = parser.read_declaration(path, contents, decl_start)?;
        if let Some(name) = name.strip_prefix("--") {
            // Custom property — keep the leading `--` in the surfaced name.
            let stripped = raw_value.trim().to_owned();
            let value = classify_value(&stripped);
            out.push(CssPropertyScrape {
                source: path.to_path_buf(),
                at_rule: at_rule.map(str::to_owned),
                name: format!("--{name}"),
                raw_value: stripped,
                value,
            });
        }
        // Non-custom declarations (`color: red;`) are silently ignored.
    }
}

/// We're sitting at the open brace of an `@media (...)` /
/// `@supports (...)` block. Look inside for a single `:root { ... }`
/// rule and collect from it. Anything else is tolerantly skipped.
fn scan_at_rule_body(
    parser: &mut Parser<'_>,
    path: &Path,
    contents: &str,
    at_rule: &str,
    out: &mut Vec<CssPropertyScrape>,
) -> Result<(), ConfigError> {
    let body_start = parser.cursor.saturating_sub(1);

    loop {
        let still_open = parser
            .skip_trivia()
            .map_err(|fault| fault.into_error(path, contents))?;
        if !still_open {
            return Err(parse_error(
                path,
                contents,
                body_start..contents.len(),
                "unterminated at-rule block",
            ));
        }
        if parser.consume_byte_eq(b'}') {
            return Ok(());
        }
        let prelude_start = parser.cursor;
        let prelude = parser
            .read_prelude()
            .map_err(|fault| fault.into_error(path, contents))?;
        let prelude_trimmed = prelude.trim();
        if !parser.consume_byte_eq(b'{') {
            return Err(parse_error(
                path,
                contents,
                prelude_start..parser.cursor,
                "expected `{` after selector inside at-rule",
            ));
        }
        if is_root_selector(prelude_trimmed) {
            collect_root_block(parser, path, contents, Some(at_rule), out)?;
        } else {
            parser
                .skip_block()
                .map_err(|fault| fault.into_error(path, contents))?;
        }
    }
}

// ---------- value classification ---------------------------------------------

fn classify_value(raw: &str) -> ScrapedValue {
    let value = raw.trim();
    if value.is_empty() {
        return ScrapedValue::Other(String::new());
    }
    if let Some(color) = parse_color(value) {
        return ScrapedValue::Color(color);
    }
    if let Some(px) = parse_unit_suffix(value, "px") {
        return ScrapedValue::Px(f32_to_u32_px(px));
    }
    if let Some(rem) = parse_unit_suffix(value, "rem") {
        return ScrapedValue::Rem(rem);
    }
    if let Some(em) = parse_unit_suffix(value, "em") {
        return ScrapedValue::Em(em);
    }
    ScrapedValue::Other(value.to_owned())
}

fn parse_unit_suffix(value: &str, unit: &str) -> Option<f32> {
    let stripped = value.strip_suffix(unit)?;
    let trimmed = stripped.trim_end();
    // Disallow embedded whitespace in the numeric portion.
    if trimmed.bytes().any(|b| b.is_ascii_whitespace()) {
        return None;
    }
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<f32>().ok()
}

fn parse_color(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix('#') {
        return parse_hex_color(rest);
    }
    if let Some(inner) = strip_func(trimmed, "rgb").or_else(|| strip_func(trimmed, "rgba")) {
        return parse_rgb_func(inner);
    }
    if let Some(inner) = strip_func(trimmed, "hsl").or_else(|| strip_func(trimmed, "hsla")) {
        return parse_hsl_func(inner);
    }
    None
}

fn strip_func<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let lower = value.as_bytes();
    if lower.len() < name.len() + 2 {
        return None;
    }
    let prefix_eq = value.as_bytes()[..name.len()]
        .iter()
        .zip(name.as_bytes())
        .all(|(a, b)| a.eq_ignore_ascii_case(b));
    if !prefix_eq {
        return None;
    }
    let rest = value[name.len()..].trim_start();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner)
}

fn parse_hex_color(rest: &str) -> Option<String> {
    let hex = rest.trim();
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let normalized = match hex.len() {
        3 => {
            let mut s = String::with_capacity(7);
            s.push('#');
            for ch in hex.chars() {
                s.push(ch.to_ascii_lowercase());
                s.push(ch.to_ascii_lowercase());
            }
            s
        }
        4 => {
            let mut s = String::with_capacity(9);
            s.push('#');
            for ch in hex.chars() {
                s.push(ch.to_ascii_lowercase());
                s.push(ch.to_ascii_lowercase());
            }
            s
        }
        6 | 8 => {
            let mut s = String::with_capacity(hex.len() + 1);
            s.push('#');
            s.extend(hex.chars().map(|c| c.to_ascii_lowercase()));
            s
        }
        _ => return None,
    };
    Some(normalized)
}

fn parse_rgb_func(inner: &str) -> Option<String> {
    let parts = split_color_args(inner);
    if !(parts.len() == 3 || parts.len() == 4) {
        return None;
    }
    let r = parse_color_byte(parts[0])?;
    let g = parse_color_byte(parts[1])?;
    let b = parse_color_byte(parts[2])?;
    if let Some(alpha_str) = parts.get(3) {
        let alpha = parse_alpha(alpha_str)?;
        Some(format!("#{r:02x}{g:02x}{b:02x}{alpha:02x}"))
    } else {
        Some(format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

fn parse_hsl_func(inner: &str) -> Option<String> {
    let parts = split_color_args(inner);
    if !(parts.len() == 3 || parts.len() == 4) {
        return None;
    }
    let hue = parse_hsl_hue(parts[0])?;
    let saturation = parse_percent(parts[1])?;
    let lightness = parse_percent(parts[2])?;
    let alpha = if let Some(alpha_str) = parts.get(3) {
        parse_alpha(alpha_str)?
    } else {
        255
    };
    let (red, green, blue) = hsl_to_rgb(hue, saturation, lightness);
    if parts.len() == 4 {
        Some(format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}"))
    } else {
        Some(format!("#{red:02x}{green:02x}{blue:02x}"))
    }
}

fn split_color_args(inner: &str) -> Vec<&str> {
    // Support both comma-separated (legacy) and space-separated (modern,
    // with `/` for alpha) syntaxes. Normalize to comma-style by walking
    // the string once.
    if inner.contains(',') {
        inner.split(',').map(str::trim).collect()
    } else {
        let (channels, alpha) = inner
            .split_once('/')
            .map_or((inner, None), |(c, a)| (c, Some(a)));
        let mut parts: Vec<&str> = channels.split_ascii_whitespace().collect();
        if let Some(alpha) = alpha {
            parts.push(alpha.trim());
        }
        parts
    }
}

fn parse_color_byte(s: &str) -> Option<u8> {
    let trimmed = s.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        let value: f32 = pct.parse().ok()?;
        return Some(f32_to_u8_byte((value / 100.0) * 255.0));
    }
    let value: f32 = trimmed.parse().ok()?;
    Some(f32_to_u8_byte(value))
}

fn parse_alpha(s: &str) -> Option<u8> {
    let trimmed = s.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        let value: f32 = pct.parse().ok()?;
        return Some(f32_to_u8_byte((value / 100.0) * 255.0));
    }
    let value: f32 = trimmed.parse().ok()?;
    Some(f32_to_u8_byte(value * 255.0))
}

fn parse_percent(s: &str) -> Option<f32> {
    let trimmed = s.trim().strip_suffix('%')?;
    trimmed.parse::<f32>().ok().map(|v| v / 100.0)
}

fn parse_hsl_hue(s: &str) -> Option<f32> {
    let trimmed = s.trim();
    let (number, scale) = if let Some(rest) = trimmed.strip_suffix("deg") {
        (rest, 1.0)
    } else if let Some(rest) = trimmed.strip_suffix("rad") {
        (rest, 360.0 / (2.0 * std::f32::consts::PI))
    } else if let Some(rest) = trimmed.strip_suffix("turn") {
        (rest, 360.0)
    } else if let Some(rest) = trimmed.strip_suffix("grad") {
        (rest, 360.0 / 400.0)
    } else {
        (trimmed, 1.0)
    };
    let raw: f32 = number.parse().ok()?;
    Some((raw * scale).rem_euclid(360.0))
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - 2.0f32.mul_add(lightness, -1.0).abs()) * saturation;
    let hue_prime = hue / 60.0;
    let intermediate = chroma * (1.0 - (hue_prime.rem_euclid(2.0) - 1.0).abs());
    let segment = clamp_hue_segment(hue_prime);
    let (r1, g1, b1) = match segment {
        0 => (chroma, intermediate, 0.0),
        1 => (intermediate, chroma, 0.0),
        2 => (0.0, chroma, intermediate),
        3 => (0.0, intermediate, chroma),
        4 => (intermediate, 0.0, chroma),
        _ => (chroma, 0.0, intermediate),
    };
    let lightness_adj = lightness - chroma / 2.0;
    (
        f32_to_u8_byte((r1 + lightness_adj) * 255.0),
        f32_to_u8_byte((g1 + lightness_adj) * 255.0),
        f32_to_u8_byte((b1 + lightness_adj) * 255.0),
    )
}

/// Pick the HSL hue segment without spending an unchecked `as i32` cast.
fn clamp_hue_segment(h_prime: f32) -> u8 {
    let normalized = h_prime.rem_euclid(6.0);
    if normalized < 1.0 {
        0
    } else if normalized < 2.0 {
        1
    } else if normalized < 3.0 {
        2
    } else if normalized < 4.0 {
        3
    } else if normalized < 5.0 {
        4
    } else {
        5
    }
}

/// Round a clamped `[0, 255]` float to a byte. Allowed because the
/// caller explicitly clamps into the byte range before calling this.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn f32_to_u8_byte(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

/// Round a non-negative pixel float to `u32`, saturating on overflow.
/// Allowed because we explicitly clamp into the `u32` range before
/// the cast.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn f32_to_u32_px(value: f32) -> u32 {
    let rounded = value.max(0.0).round();
    if rounded >= u32::MAX as f32 {
        u32::MAX
    } else {
        rounded as u32
    }
}

// ---------- low-level state machine ------------------------------------------

struct Parser<'a> {
    bytes: &'a [u8],
    /// Current byte offset in `bytes`.
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            bytes: source.as_bytes(),
            cursor: 0,
        }
    }

    /// Advance past whitespace and CSS comments. Returns `true` if any
    /// non-trivia byte remains; `false` at EOF.
    ///
    /// Errors only on unterminated comments.
    fn skip_trivia(&mut self) -> Result<bool, ParseFault> {
        loop {
            while self.cursor < self.bytes.len() && self.bytes[self.cursor].is_ascii_whitespace() {
                self.cursor += 1;
            }
            if self.cursor + 1 < self.bytes.len()
                && self.bytes[self.cursor] == b'/'
                && self.bytes[self.cursor + 1] == b'*'
            {
                let start = self.cursor;
                self.cursor += 2;
                while self.cursor + 1 < self.bytes.len() {
                    if self.bytes[self.cursor] == b'*' && self.bytes[self.cursor + 1] == b'/' {
                        self.cursor += 2;
                        break;
                    }
                    self.cursor += 1;
                }
                if self.cursor + 1 >= self.bytes.len()
                    && !(self.cursor >= 2
                        && self.bytes[self.cursor - 2] == b'*'
                        && self.bytes[self.cursor - 1] == b'/')
                {
                    return Err(ParseFault {
                        range: start..self.bytes.len(),
                        message: "unterminated CSS comment",
                    });
                }
                continue;
            }
            return Ok(self.cursor < self.bytes.len());
        }
    }

    /// Read everything up to the next top-level `{` or `;` (whichever
    /// is first). Returns the prelude as a `&str`. Leaves the cursor
    /// pointing at the delimiter (the caller decides what to do with it).
    fn read_prelude(&mut self) -> Result<&'a str, ParseFault> {
        let start = self.cursor;
        while self.cursor < self.bytes.len() {
            let byte = self.bytes[self.cursor];
            match byte {
                b'{' | b';' | b'}' => {
                    return Ok(self.slice(start, self.cursor));
                }
                b'/' if self.peek1() == Some(b'*') => {
                    self.skip_comment(start)?;
                }
                b'"' | b'\'' => {
                    self.skip_string(byte)?;
                }
                _ => self.cursor += 1,
            }
        }
        Ok(self.slice(start, self.cursor))
    }

    /// Read a single declaration starting at `decl_start`. Expects the
    /// cursor at the first non-trivia byte of the declaration. Returns
    /// `(name, raw_value)`. Trailing semicolon is consumed.
    fn read_declaration(
        &mut self,
        path: &Path,
        contents: &str,
        decl_start: usize,
    ) -> Result<(String, String), ConfigError> {
        let name_start = self.cursor;
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b':' => break,
                b'{' | b'}' | b';' => {
                    return Err(parse_error(
                        path,
                        contents,
                        decl_start..self.cursor,
                        "expected `:` in declaration",
                    ));
                }
                _ => self.cursor += 1,
            }
        }
        if self.cursor >= self.bytes.len() {
            return Err(parse_error(
                path,
                contents,
                decl_start..self.bytes.len(),
                "unterminated declaration",
            ));
        }
        let name = self.slice(name_start, self.cursor).trim().to_owned();
        // consume the `:`
        self.cursor += 1;
        let value_start = self.cursor;
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b';' => {
                    let value = self.slice(value_start, self.cursor).trim().to_owned();
                    self.cursor += 1; // consume `;`
                    return Ok((name, strip_inline_comments(&value)));
                }
                b'}' => {
                    let value = self.slice(value_start, self.cursor).trim().to_owned();
                    return Ok((name, strip_inline_comments(&value)));
                }
                b'/' if self.peek1() == Some(b'*') => {
                    self.skip_comment(decl_start)
                        .map_err(|fault| fault.into_error(path, contents))?;
                }
                b'"' | b'\'' => {
                    self.skip_string(self.bytes[self.cursor])
                        .map_err(|fault| fault.into_error(path, contents))?;
                }
                _ => self.cursor += 1,
            }
        }
        Err(parse_error(
            path,
            contents,
            decl_start..self.bytes.len(),
            "unterminated declaration",
        ))
    }

    /// Skip a brace-delimited block. Cursor must point one past the
    /// opening `{`. Leaves the cursor one past the matching `}`.
    fn skip_block(&mut self) -> Result<(), ParseFault> {
        let start = self.cursor.saturating_sub(1);
        let mut depth: usize = 1;
        while self.cursor < self.bytes.len() && depth > 0 {
            match self.bytes[self.cursor] {
                b'{' => {
                    depth += 1;
                    self.cursor += 1;
                }
                b'}' => {
                    depth -= 1;
                    self.cursor += 1;
                }
                b'/' if self.peek1() == Some(b'*') => {
                    self.skip_comment(start)?;
                }
                b'"' | b'\'' => {
                    let q = self.bytes[self.cursor];
                    self.skip_string(q)?;
                }
                _ => self.cursor += 1,
            }
        }
        if depth == 0 {
            Ok(())
        } else {
            Err(ParseFault {
                range: start..self.bytes.len(),
                message: "unterminated block",
            })
        }
    }

    fn skip_comment(&mut self, anchor: usize) -> Result<(), ParseFault> {
        // `cursor` points at `/`. Verify and walk to closing `*/`.
        debug_assert_eq!(self.bytes[self.cursor], b'/');
        debug_assert_eq!(self.bytes[self.cursor + 1], b'*');
        self.cursor += 2;
        while self.cursor + 1 < self.bytes.len() {
            if self.bytes[self.cursor] == b'*' && self.bytes[self.cursor + 1] == b'/' {
                self.cursor += 2;
                return Ok(());
            }
            self.cursor += 1;
        }
        Err(ParseFault {
            range: anchor..self.bytes.len(),
            message: "unterminated CSS comment",
        })
    }

    fn skip_string(&mut self, quote: u8) -> Result<(), ParseFault> {
        let start = self.cursor;
        self.cursor += 1; // open quote
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b'\\' if self.cursor + 1 < self.bytes.len() => self.cursor += 2,
                b if b == quote => {
                    self.cursor += 1;
                    return Ok(());
                }
                _ => self.cursor += 1,
            }
        }
        Err(ParseFault {
            range: start..self.bytes.len(),
            message: "unterminated string",
        })
    }

    fn consume_byte_eq(&mut self, byte: u8) -> bool {
        if self.cursor < self.bytes.len() && self.bytes[self.cursor] == byte {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek1(&self) -> Option<u8> {
        self.bytes.get(self.cursor + 1).copied()
    }

    fn slice(&self, start: usize, end: usize) -> &'a str {
        // SAFETY substitute: we only ever index at byte positions we
        // discovered while walking the slice via ASCII checks or by
        // recognizing UTF-8-safe boundary bytes (`{`, `}`, `;`, `:`,
        // `"`, `'`, `/`, `*`). Recover with `from_utf8_lossy` if a
        // future regression introduces a mid-codepoint slice.
        std::str::from_utf8(&self.bytes[start..end]).unwrap_or("")
    }
}

/// Strip trailing `/* ... */` comments inside a declaration's raw
/// value. Comments embedded mid-value (rare) are also stripped.
fn strip_inline_comments(value: &str) -> String {
    if !value.contains("/*") {
        return value.to_owned();
    }
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut i = 0;
    let mut run_start = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Flush the non-comment run as a UTF-8 string slice rather than
            // pushing bytes one at a time — bytes[i] as char would corrupt
            // multi-byte codepoints (e.g. unicode font names).
            out.push_str(&value[run_start..i]);
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2;
            } else {
                i = bytes.len();
            }
            run_start = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&value[run_start..]);
    out.trim().to_owned()
}

// ---------- error plumbing ---------------------------------------------------

struct ParseFault {
    range: std::ops::Range<usize>,
    message: &'static str,
}

impl ParseFault {
    fn into_error(self, path: &Path, contents: &str) -> ConfigError {
        parse_error(path, contents, self.range, self.message)
    }
}

fn parse_error(
    path: &Path,
    contents: &str,
    range: std::ops::Range<usize>,
    message: &'static str,
) -> ConfigError {
    ConfigError::CssParse {
        path: path.display().to_string(),
        message: message.to_owned(),
        source_code: Some(
            NamedSource::new(path.display().to_string(), contents.to_owned()).with_language("css"),
        ),
        span: Some(into_span(range)),
    }
}

fn into_span(range: std::ops::Range<usize>) -> SourceSpan {
    let len = range.end.saturating_sub(range.start);
    (range.start, len).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_value_handles_hex_and_units() {
        assert!(matches!(
            classify_value("#abc"),
            ScrapedValue::Color(s) if s == "#aabbcc"
        ));
        assert!(matches!(
            classify_value("#AABBCCDD"),
            ScrapedValue::Color(s) if s == "#aabbccdd"
        ));
        assert!(matches!(classify_value("16px"), ScrapedValue::Px(16)));
        assert!(matches!(classify_value("1.5rem"), ScrapedValue::Rem(_)));
        assert!(matches!(classify_value("1.5em"), ScrapedValue::Em(_)));
        assert!(matches!(
            classify_value("rgb(1, 2, 3)"),
            ScrapedValue::Color(s) if s == "#010203"
        ));
        assert!(matches!(
            classify_value("rgba(255, 255, 255, 0.5)"),
            ScrapedValue::Color(s) if s == "#ffffff80"
        ));
        assert!(matches!(
            classify_value("rgb(100%, 0%, 0%)"),
            ScrapedValue::Color(s) if s == "#ff0000"
        ));
        assert!(matches!(
            classify_value("hsl(0, 100%, 50%)"),
            ScrapedValue::Color(s) if s == "#ff0000"
        ));
        assert!(matches!(
            classify_value("\"Inter\", sans-serif"),
            ScrapedValue::Other(_)
        ));
    }

    #[test]
    fn strip_inline_comments_works() {
        assert_eq!(strip_inline_comments("4px /* trail */"), "4px");
        assert_eq!(strip_inline_comments("/* lead */ 4px /* trail */"), "4px");
        assert_eq!(strip_inline_comments("8px"), "8px");
    }
}
