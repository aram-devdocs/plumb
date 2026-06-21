//! Conservative parser for literal TypeScript/JavaScript token modules.

// Items here are crate-private but live inside a private module; the
// `redundant_pub_crate` lint flips between deny on `pub(crate)` and the
// rust-level `unreachable_pub` lint on bare `pub`. Allow the former
// scoped to this module so the items keep the explicit visibility.
#![allow(clippy::redundant_pub_crate)]

use std::path::Path;

use plumb_core::Config;

/// Summary of token module values inserted into a [`Config`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenModuleImport {
    /// Number of color tokens added.
    pub(crate) colors: usize,
    /// Number of spacing tokens added.
    pub(crate) spacing: usize,
    /// Number of typography size tokens added.
    pub(crate) type_sizes: usize,
    /// Number of font families added.
    pub(crate) type_families: usize,
    /// Number of font weights added.
    pub(crate) type_weights: usize,
    /// Number of radius values added.
    pub(crate) radii: usize,
}

/// Merge conservative literal exports from `contents` into `config`.
pub(crate) fn merge_literal_token_module(
    config: &mut Config,
    path: &Path,
    contents: &str,
) -> TokenModuleImport {
    let mut exports = Parser::new(contents).parse_exported_objects();
    exports.sort_by(|a, b| {
        export_priority(&a.name)
            .cmp(&export_priority(&b.name))
            .then_with(|| token_sort_key(&a.name).cmp(&token_sort_key(&b.name)))
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut import = TokenModuleImport::default();
    for export in exports {
        let mut leaves = Vec::new();
        collect_leaves(&export.properties, &mut Vec::new(), &mut leaves);
        leaves.sort_by(|a, b| {
            token_path_key(&a.path)
                .cmp(&token_path_key(&b.path))
                .then_with(|| a.path.cmp(&b.path))
        });
        for leaf in leaves {
            merge_leaf(config, path, &export.name, &leaf, &mut import);
        }
    }

    config.spacing.scale.sort_unstable();
    config.spacing.scale.dedup();
    config.radius.scale.sort_unstable();
    config.radius.scale.dedup();
    config.type_scale.scale.sort_unstable();
    config.type_scale.scale.dedup();
    config.type_scale.weights.sort_unstable();
    config.type_scale.weights.dedup();

    import
}

fn export_priority(name: &str) -> u8 {
    match token_sort_key(name).as_str() {
        "light" | "default" | "tokens" | "designtokens" => 0,
        "dark" => 2,
        _ => 1,
    }
}

#[derive(Debug)]
struct ExportedObject {
    name: String,
    properties: Vec<Property>,
}

#[derive(Debug)]
struct Property {
    key: String,
    value: LiteralValue,
}

#[derive(Debug)]
enum LiteralValue {
    String(String),
    Number(String),
    Object(Vec<Property>),
}

#[derive(Debug)]
struct TokenLeaf<'a> {
    path: Vec<String>,
    value: &'a LiteralValue,
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    fn parse_exported_objects(&mut self) -> Vec<ExportedObject> {
        let mut exports = Vec::new();
        while !self.is_eof() {
            self.skip_ws_and_comments();
            if self.consume_keyword("export") {
                self.skip_ws_and_comments();
                if self.consume_keyword("const")
                    && let Some(export) = self.parse_const_export()
                {
                    exports.push(export);
                }
                continue;
            }
            self.skip_non_code_char();
        }
        exports
    }

    fn parse_const_export(&mut self) -> Option<ExportedObject> {
        self.skip_ws_and_comments();
        let name = self.parse_identifier()?;
        if !self.consume_until_equals() {
            return None;
        }
        self.skip_ws_and_comments();
        let properties = self.parse_object()?;
        Some(ExportedObject { name, properties })
    }

    fn parse_object(&mut self) -> Option<Vec<Property>> {
        if !self.consume_byte(b'{') {
            return None;
        }
        let mut properties = Vec::new();

        loop {
            self.skip_ws_and_comments();
            if self.consume_byte(b'}') {
                return Some(properties);
            }
            if self.is_eof() {
                return None;
            }

            let Some(key) = self.parse_key() else {
                self.skip_unsupported_value();
                let _ = self.consume_byte(b',');
                continue;
            };
            self.skip_ws_and_comments();
            if !self.consume_byte(b':') {
                self.skip_unsupported_value();
                let _ = self.consume_byte(b',');
                continue;
            }
            self.skip_ws_and_comments();
            if let Some(value) = self.parse_value() {
                properties.push(Property { key, value });
            }
            self.skip_ws_and_comments();
            if self.consume_byte(b',') {
                continue;
            }
            if self.consume_byte(b'}') {
                return Some(properties);
            }
        }
    }

    fn parse_key(&mut self) -> Option<String> {
        self.skip_ws_and_comments();
        match self.current_byte()? {
            b'\'' | b'"' => self.parse_string(),
            b'-' | b'0'..=b'9' => self.parse_number_literal(),
            _ => self.parse_identifier(),
        }
    }

    fn parse_value(&mut self) -> Option<LiteralValue> {
        self.skip_ws_and_comments();
        match self.current_byte()? {
            b'\'' | b'"' => self.parse_string().map(LiteralValue::String),
            b'{' => self.parse_object().map(LiteralValue::Object),
            b'-' | b'0'..=b'9' => {
                if let Some(value) = self.parse_number_literal() {
                    Some(LiteralValue::Number(value))
                } else {
                    self.skip_unsupported_value();
                    None
                }
            }
            _ => {
                self.skip_unsupported_value();
                None
            }
        }
    }

    fn consume_until_equals(&mut self) -> bool {
        let mut depth = 0usize;
        while !self.is_eof() {
            self.skip_ws_and_comments();
            let Some(byte) = self.current_byte() else {
                return false;
            };
            match byte {
                b'=' if depth == 0 => {
                    self.pos += 1;
                    return true;
                }
                b';' if depth == 0 => return false,
                b'\'' | b'"' => self.skip_string_literal(),
                b'`' => self.skip_template_literal(),
                b'(' | b'[' | b'{' | b'<' => {
                    depth = depth.saturating_add(1);
                    self.pos += 1;
                }
                b')' | b']' | b'}' | b'>' => {
                    depth = depth.saturating_sub(1);
                    self.pos += 1;
                }
                _ => {
                    let _ = self.bump_char();
                }
            }
        }
        false
    }

    fn skip_unsupported_value(&mut self) {
        let mut depth = 0usize;
        while !self.is_eof() {
            self.skip_ws_and_comments();
            let Some(byte) = self.current_byte() else {
                return;
            };
            match byte {
                b',' | b'}' if depth == 0 => return,
                b'\'' | b'"' => self.skip_string_literal(),
                b'`' => self.skip_template_literal(),
                b'(' | b'[' | b'{' => {
                    depth = depth.saturating_add(1);
                    self.pos += 1;
                }
                b')' | b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    self.pos += 1;
                }
                _ => {
                    let _ = self.bump_char();
                }
            }
        }
    }

    fn parse_string(&mut self) -> Option<String> {
        let quote = self.current_byte()?;
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        self.pos += 1;
        let mut out = String::new();
        while !self.is_eof() {
            let ch = self.bump_char()?;
            if ch == char::from(quote) {
                return Some(out);
            }
            if ch == '\\' {
                let escaped = self.bump_char()?;
                match escaped {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    '\\' | '\'' | '"' => out.push(escaped),
                    other => out.push(other),
                }
            } else {
                out.push(ch);
            }
        }
        None
    }

    fn parse_number_literal(&mut self) -> Option<String> {
        let start = self.pos;
        if self.current_byte() == Some(b'-') {
            self.pos += 1;
        }

        let integer_start = self.pos;
        while self.current_byte().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == integer_start {
            self.pos = start;
            return None;
        }

        if self.current_byte() == Some(b'.') {
            self.pos += 1;
            let fraction_start = self.pos;
            while self.current_byte().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == fraction_start {
                self.pos = start;
                return None;
            }
        }
        if !self.at_number_literal_delimiter() {
            self.pos = start;
            return None;
        }

        Some(self.source[start..self.pos].to_owned())
    }

    fn at_number_literal_delimiter(&self) -> bool {
        match self.current_byte() {
            Some(byte) if byte.is_ascii_whitespace() => true,
            None | Some(b',' | b'}' | b']' | b')' | b':' | b';') => true,
            Some(_) => false,
        }
    }

    fn parse_identifier(&mut self) -> Option<String> {
        let mut chars = self.source[self.pos..].char_indices();
        let (_, first) = chars.next()?;
        if !is_ident_start(first) {
            return None;
        }
        let mut end = self.pos + first.len_utf8();
        for (offset, ch) in chars {
            if !is_ident_continue(ch) {
                break;
            }
            end = self.pos + offset + ch.len_utf8();
        }
        let ident = self.source[self.pos..end].to_owned();
        self.pos = end;
        Some(ident)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while self
                .source
                .get(self.pos..)
                .and_then(|rest| rest.chars().next())
                .is_some_and(char::is_whitespace)
            {
                let _ = self.bump_char();
            }

            if self.starts_with("//") {
                self.pos += 2;
                while !self.is_eof() && self.current_byte() != Some(b'\n') {
                    let _ = self.bump_char();
                }
                continue;
            }
            if self.starts_with("/*") {
                self.pos += 2;
                while !self.is_eof() && !self.starts_with("*/") {
                    let _ = self.bump_char();
                }
                if self.starts_with("*/") {
                    self.pos += 2;
                }
                continue;
            }
            break;
        }
    }

    fn skip_non_code_char(&mut self) {
        match self.current_byte() {
            Some(b'\'' | b'"') => self.skip_string_literal(),
            Some(b'`') => self.skip_template_literal(),
            Some(_) => {
                let _ = self.bump_char();
            }
            None => {}
        }
    }

    fn skip_string_literal(&mut self) {
        let Some(quote) = self.current_byte() else {
            return;
        };
        if quote != b'\'' && quote != b'"' {
            return;
        }
        self.pos += 1;
        while !self.is_eof() {
            let Some(ch) = self.bump_char() else {
                return;
            };
            if ch == '\\' {
                let _ = self.bump_char();
            } else if ch == char::from(quote) {
                return;
            }
        }
    }

    fn skip_template_literal(&mut self) {
        if self.current_byte() != Some(b'`') {
            return;
        }
        self.pos += 1;
        while !self.is_eof() {
            let Some(ch) = self.bump_char() else {
                return;
            };
            if ch == '\\' {
                let _ = self.bump_char();
            } else if ch == '`' {
                return;
            }
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        if !self.keyword_at(keyword) {
            return false;
        }
        self.pos += keyword.len();
        true
    }

    fn keyword_at(&self, keyword: &str) -> bool {
        if !self.starts_with(keyword) {
            return false;
        }
        let before_ok = self.source[..self.pos]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_ident_continue(ch));
        let after_pos = self.pos + keyword.len();
        let after_ok = self
            .source
            .get(after_pos..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|ch| !is_ident_continue(ch));
        before_ok && after_ok
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.current_byte() != Some(byte) {
            return false;
        }
        self.pos += 1;
        true
    }

    fn current_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(value))
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos..)?.chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }
}

fn collect_leaves<'a>(
    properties: &'a [Property],
    prefix: &mut Vec<String>,
    out: &mut Vec<TokenLeaf<'a>>,
) {
    for property in properties {
        prefix.push(property.key.clone());
        match &property.value {
            LiteralValue::Object(children) => collect_leaves(children, prefix, out),
            LiteralValue::String(_) | LiteralValue::Number(_) => out.push(TokenLeaf {
                path: prefix.clone(),
                value: &property.value,
            }),
        }
        let _ = prefix.pop();
    }
}

fn merge_leaf(
    config: &mut Config,
    path: &Path,
    object_name: &str,
    leaf: &TokenLeaf<'_>,
    import: &mut TokenModuleImport,
) {
    let mut hints = hint_tokens(object_name, &leaf.path);
    if !has_any_token_hint(&hints)
        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
    {
        hints.extend(split_hint(stem));
    }
    let token_name = token_path_key(&leaf.path);

    if has_font_family_hint(&hints)
        && let LiteralValue::String(value) = leaf.value
    {
        add_font_families(config, value, import);
        return;
    }

    if has_font_weight_hint(&hints)
        && let Some(weight) = parse_weight(leaf.value)
    {
        if !config.type_scale.weights.contains(&weight) {
            config.type_scale.weights.push(weight);
            import.type_weights += 1;
        }
        return;
    }

    if has_radius_hint(&hints)
        && let Some(px) = parse_px(leaf.value)
    {
        if !config.radius.scale.contains(&px) {
            config.radius.scale.push(px);
            import.radii += 1;
        }
        return;
    }

    if has_spacing_hint(&hints)
        && let Some(px) = parse_px(leaf.value)
    {
        if !config.spacing.tokens.contains_key(&token_name) {
            config.spacing.tokens.insert(token_name, px);
            config.spacing.scale.push(px);
            import.spacing += 1;
        }
        return;
    }

    if has_type_size_hint(&hints)
        && let Some(px) = parse_px(leaf.value)
    {
        if !config.type_scale.tokens.contains_key(&token_name) {
            config.type_scale.tokens.insert(token_name, px);
            config.type_scale.scale.push(px);
            import.type_sizes += 1;
        }
        return;
    }

    if has_color_hint(&hints)
        && let LiteralValue::String(value) = leaf.value
        && is_hex_color(value)
        && !config.color.tokens.contains_key(&token_name)
    {
        config
            .color
            .tokens
            .insert(token_name, value.trim().to_owned());
        import.colors += 1;
    }
}

fn hint_tokens(object_name: &str, token_path: &[String]) -> Vec<String> {
    let mut hints = Vec::new();
    hints.extend(split_hint(object_name));
    for segment in token_path {
        hints.extend(split_hint(segment));
    }
    hints
}

fn split_hint(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut previous_was_lower_or_digit = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_was_lower_or_digit && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            current.push(ch.to_ascii_lowercase());
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            previous_was_lower_or_digit = false;
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn has_font_family_hint(hints: &[String]) -> bool {
    has_joined(hints, "fontfamily")
        || has_joined(hints, "fontfamilies")
        || hints.iter().any(|hint| hint == "families")
}

fn has_font_weight_hint(hints: &[String]) -> bool {
    has_joined(hints, "fontweight")
        || has_joined(hints, "fontweights")
        || hints
            .iter()
            .any(|hint| hint == "weights" || hint == "weight")
}

fn has_radius_hint(hints: &[String]) -> bool {
    hints
        .iter()
        .any(|hint| hint == "radius" || hint == "radii" || hint == "borderradius")
        || has_joined(hints, "borderradius")
}

fn has_spacing_hint(hints: &[String]) -> bool {
    hints
        .iter()
        .any(|hint| hint == "spacing" || hint == "space")
}

fn has_type_size_hint(hints: &[String]) -> bool {
    has_joined(hints, "fontsize")
        || has_joined(hints, "fontsizes")
        || hints
            .iter()
            .any(|hint| matches!(hint.as_str(), "typography" | "type" | "font" | "text"))
}

fn has_color_hint(hints: &[String]) -> bool {
    hints.iter().any(|hint| hint == "color" || hint == "colors")
        || has_joined(hints, "color")
        || has_joined(hints, "colors")
}

fn has_any_token_hint(hints: &[String]) -> bool {
    has_font_family_hint(hints)
        || has_font_weight_hint(hints)
        || has_radius_hint(hints)
        || has_spacing_hint(hints)
        || has_type_size_hint(hints)
        || has_color_hint(hints)
}

fn has_joined(hints: &[String], needle: &str) -> bool {
    let joined = hints.join("");
    joined.contains(needle)
}

fn parse_px(value: &LiteralValue) -> Option<u32> {
    match value {
        LiteralValue::String(raw) => {
            let trimmed = raw.trim().to_ascii_lowercase();
            if let Some(number) = trimmed.strip_suffix("px") {
                return decimal_to_u32(number.trim());
            }
            if let Some(number) = trimmed
                .strip_suffix("rem")
                .or_else(|| trimmed.strip_suffix("em"))
            {
                return number
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .and_then(|n| decimal_to_u32_value(n * 16.0));
            }
            None
        }
        LiteralValue::Number(raw) => decimal_to_u32(raw),
        LiteralValue::Object(_) => None,
    }
}

fn decimal_to_u32(raw: &str) -> Option<u32> {
    let number = raw.parse::<f64>().ok()?;
    decimal_to_u32_value(number)
}

fn decimal_to_u32_value(number: f64) -> Option<u32> {
    if !number.is_finite() || number.is_sign_negative() || number > f64::from(u32::MAX) {
        return None;
    }
    // The finite/sign/range checks above make the rounded value fit in u32.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    Some(number.round() as u32)
}

fn parse_weight(value: &LiteralValue) -> Option<u16> {
    let parsed = match value {
        LiteralValue::Number(raw) => raw.parse::<u64>().ok(),
        LiteralValue::String(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "thin" | "hairline" => Some(100),
            "extra-light" | "extralight" | "ultralight" => Some(200),
            "light" => Some(300),
            "regular" | "normal" => Some(400),
            "medium" => Some(500),
            "semi-bold" | "semibold" | "demibold" => Some(600),
            "bold" => Some(700),
            "extra-bold" | "extrabold" | "ultrabold" => Some(800),
            "black" | "heavy" => Some(900),
            other => other.parse::<u64>().ok(),
        },
        LiteralValue::Object(_) => None,
    }?;
    u16::try_from(parsed).ok()
}

fn add_font_families(config: &mut Config, raw: &str, import: &mut TokenModuleImport) {
    for family in split_font_stack(raw) {
        if !config.type_scale.families.iter().any(|f| f == &family) {
            config.type_scale.families.push(family);
            import.type_families += 1;
        }
    }
}

fn split_font_stack(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter_map(|part| {
            let trimmed = part.trim().trim_matches(['\'', '"']).trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
        .collect()
}

fn is_hex_color(raw: &str) -> bool {
    let trimmed = raw.trim();
    let Some(body) = trimmed.strip_prefix('#') else {
        return false;
    };
    matches!(body.len(), 3 | 4 | 6 | 8) && body.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn token_path_key(path: &[String]) -> String {
    path.join("/")
}

fn token_sort_key(value: &str) -> String {
    split_hint(value).join("")
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn merges_flappy_gouda_style_literal_exports() {
        let source = r#"
            export const SPACING = {
              0.5: '2px',
              1: '4px',
              1.5: '6px',
            } as const;

            export const RADIUS = {
              sm: '4px',
              md: '6px',
              lg: '8px',
              xl: '12px',
              '2xl': '16px',
              pill: '100px',
            } as const;

            export const COLOR_TOKENS = {
              navy: '#0A3D5C',
              violet: '#5AAFA5',
            } as const;

            export const STATUS_COLORS = {
              success: '#22c55e',
            } as const;

            export const COLOR_RGB = {
              navy: '10 61 92',
            } as const;

            export const RGBA_TOKENS = {
              overlay: `rgba(${COLOR_RGB.navy} / 0.45)`,
            } as const;

            export const FONT_FAMILY = {
              heading: '"Poppins", sans-serif',
              body: '"apertura", "Inter", system-ui, sans-serif',
            } as const;

            export const FONT_SIZE = {
              '2xs': '9px',
              xs: '10px',
            } as const;

            export const FONT_WEIGHT = {
              normal: 400,
              semibold: 600,
              bold: 700,
              extrabold: 800,
            } as const;

            export const DESIGN_TOKENS = {
              colors: COLOR_TOKENS,
            } as const;
        "#;
        let mut config = Config::default();

        let import = merge_literal_token_module(
            &mut config,
            Path::new("packages/types/src/tokens/spacing.ts"),
            source,
        );

        assert_eq!(import.spacing, 3);
        assert_eq!(config.spacing.tokens["0.5"], 2);
        assert_eq!(config.spacing.tokens["1.5"], 6);
        assert_eq!(config.spacing.scale, vec![2, 4, 6]);

        assert_eq!(import.radii, 6);
        assert_eq!(config.radius.scale, vec![4, 6, 8, 12, 16, 100]);

        assert_eq!(import.colors, 3);
        assert_eq!(config.color.tokens["navy"], "#0A3D5C");
        assert_eq!(config.color.tokens["success"], "#22c55e");
        assert!(!config.color.tokens.contains_key("overlay"));

        assert_eq!(import.type_sizes, 2);
        assert_eq!(config.type_scale.tokens["2xs"], 9);
        assert_eq!(config.type_scale.tokens["xs"], 10);
        assert_eq!(config.type_scale.scale, vec![9, 10]);

        assert_eq!(import.type_weights, 4);
        assert_eq!(config.type_scale.weights, vec![400, 600, 700, 800]);

        assert_eq!(import.type_families, 5);
        assert!(config.type_scale.families.contains(&"Poppins".to_owned()));
        assert!(config.type_scale.families.contains(&"apertura".to_owned()));
        assert!(config.type_scale.families.contains(&"Inter".to_owned()));
        assert!(config.type_scale.families.contains(&"system-ui".to_owned()));
        assert!(
            config
                .type_scale
                .families
                .contains(&"sans-serif".to_owned())
        );
    }

    #[test]
    fn prefers_light_exports_before_dark_for_duplicate_color_keys() {
        let source = r"
            export const dark = {
              background: '#000000',
              primary: '#111111',
              onlyDark: '#222222',
            } as const;

            export const light = {
              background: '#ffffff',
              primary: '#007068',
            } as const;

            export const otherColors = {
              accent: '#123456',
            } as const;
        ";
        let mut config = Config::default();

        let import = merge_literal_token_module(&mut config, Path::new("tokens/colors.ts"), source);

        assert_eq!(import.colors, 4);
        assert_eq!(config.color.tokens["background"], "#ffffff");
        assert_eq!(config.color.tokens["primary"], "#007068");
        assert_eq!(config.color.tokens["accent"], "#123456");
        assert_eq!(config.color.tokens["onlyDark"], "#222222");
    }

    #[test]
    fn converts_rem_and_em_lengths_in_token_modules() {
        let source = r"
            export const radius = {
              base: '0.75rem',
              badge: '0.5em',
            } as const;

            export const spacing = {
              md: '1.5rem',
            } as const;

            export const fontSize = {
              sm: '0.875rem',
            } as const;
        ";
        let mut config = Config::default();

        let import = merge_literal_token_module(
            &mut config,
            Path::new("packages/theme/src/tokens/radius.ts"),
            source,
        );

        assert_eq!(import.radii, 2);
        assert_eq!(config.radius.scale, vec![8, 12]);
        assert_eq!(import.spacing, 1);
        assert_eq!(config.spacing.tokens["md"], 24);
        assert_eq!(config.spacing.scale, vec![24]);
        assert_eq!(import.type_sizes, 1);
        assert_eq!(config.type_scale.tokens["sm"], 14);
        assert_eq!(config.type_scale.scale, vec![14]);
    }

    #[test]
    fn skips_unsupported_numeric_literals_without_truncating() {
        let source = r"
            export const SPACING = {
              ok: 8,
              decimal: 1.5,
              exponent: 1e3,
              separator: 1_000,
              hex: 0x10,
              binary: 0b10,
              octal: 0o10,
              bigint: 100n,
              unit: 12px,
            } as const;

            export const FONT_WEIGHT = {
              regular: 400,
              badBigInt: 700n,
            } as const;
        ";
        let mut config = Config::default();

        let import = merge_literal_token_module(
            &mut config,
            Path::new("packages/types/src/tokens/spacing.ts"),
            source,
        );

        assert_eq!(import.spacing, 2);
        assert_eq!(config.spacing.tokens["ok"], 8);
        assert_eq!(config.spacing.tokens["decimal"], 2);
        for key in [
            "exponent",
            "separator",
            "hex",
            "binary",
            "octal",
            "bigint",
            "unit",
        ] {
            assert!(!config.spacing.tokens.contains_key(key));
        }

        assert_eq!(import.type_weights, 1);
        assert_eq!(config.type_scale.weights, vec![400]);
    }
}
