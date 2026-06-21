//! Render an [`InferredConfig`] to a `plumb.toml` string.
//!
//! The output starts with a generated header comment that records, in
//! sorted order, every source the inference pass consumed. The body
//! contains only sections whose inferred values differ from
//! [`plumb_core::Config::default`], preserving runtime defaults for
//! viewports and rules.

use crate::{CodegenError, InferredConfig, TokenSource, TokenSourceKind};

/// Comment header byline. Single line so the rendered file's first
/// content line is always the same number of bytes regardless of
/// whether the inferrer found anything.
const HEADER: &str =
    "# Plumb configuration — bootstrapped by `plumb init --from <path>` from the sources below.";

/// Note appended when a Tailwind config was discovered.
const TAILWIND_HINT: &str =
    "# Tailwind config detected. Resolved theme tokens are included when available —";

/// Render `inferred` to a TOML string.
///
/// The output is byte-identical given the same [`InferredConfig`]. The
/// header lists discovered source files in stable order; the body is a
/// minimal TOML table that omits default-empty sections.
///
/// # Errors
///
/// Returns [`CodegenError::Render`] if `toml::to_string_pretty` fails
/// (extremely rare — the [`plumb_core::Config`] schema is `Serialize`).
pub fn render_toml(inferred: &InferredConfig) -> Result<String, CodegenError> {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');

    if inferred.sources.is_empty() {
        out.push_str("# No design-token sources were discovered. Edit the values below to match your system.\n");
    } else {
        out.push_str("#\n");
        write_source_list(&mut out, &inferred.sources);
    }

    if has_tailwind(&inferred.sources) {
        out.push_str("#\n");
        out.push_str(TAILWIND_HINT);
        out.push('\n');
        out.push_str(
            "# edit the inferred values below if your project overrides them elsewhere.\n",
        );
    }

    out.push('\n');

    let body = render_config_body(&inferred.config)?;
    out.push_str(&body);

    // Always end on a single newline. `toml::to_string_pretty` emits
    // one already; defensively ensure the file ends consistently.
    if !out.ends_with('\n') {
        out.push('\n');
    }

    Ok(out)
}

fn render_config_body(config: &plumb_core::Config) -> Result<String, toml::ser::Error> {
    let default = plumb_core::Config::default();
    let mut table = toml::Table::new();

    if config.viewports != default.viewports {
        table.insert(
            "viewports".to_owned(),
            toml::Value::try_from(&config.viewports)?,
        );
    }
    if config.spacing != default.spacing {
        table.insert(
            "spacing".to_owned(),
            toml::Value::try_from(&config.spacing)?,
        );
    }
    if config.type_scale != default.type_scale {
        table.insert(
            "type".to_owned(),
            toml::Value::try_from(&config.type_scale)?,
        );
    }
    if config.color != default.color {
        table.insert("color".to_owned(), toml::Value::try_from(&config.color)?);
    }
    if config.radius != default.radius {
        table.insert("radius".to_owned(), toml::Value::try_from(&config.radius)?);
    }
    if config.alignment != default.alignment {
        table.insert(
            "alignment".to_owned(),
            toml::Value::try_from(&config.alignment)?,
        );
    }
    if config.shadow != default.shadow {
        table.insert("shadow".to_owned(), toml::Value::try_from(&config.shadow)?);
    }
    if config.z_index != default.z_index {
        table.insert(
            "z_index".to_owned(),
            toml::Value::try_from(&config.z_index)?,
        );
    }
    if config.opacity != default.opacity {
        table.insert(
            "opacity".to_owned(),
            toml::Value::try_from(&config.opacity)?,
        );
    }
    if config.rhythm != default.rhythm {
        table.insert("rhythm".to_owned(), toml::Value::try_from(&config.rhythm)?);
    }
    if config.a11y != default.a11y {
        table.insert("a11y".to_owned(), toml::Value::try_from(&config.a11y)?);
    }
    if config.rules != default.rules {
        table.insert("rules".to_owned(), toml::Value::try_from(&config.rules)?);
    }
    if config.ignore != default.ignore {
        table.insert("ignore".to_owned(), toml::Value::try_from(&config.ignore)?);
    }

    if table.is_empty() {
        Ok(String::new())
    } else {
        toml::to_string_pretty(&toml::Value::Table(table))
    }
}

fn write_source_list(out: &mut String, sources: &[TokenSource]) {
    use std::fmt::Write as _;
    let mut sorted: Vec<&TokenSource> = sources.iter().collect();
    sorted.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    for source in sorted {
        let label = source.kind.label();
        let path = display_path(&source.relative_path);
        // Writes to a `String` buffer cannot fail; ignore the result.
        let _ = writeln!(out, "# - {label}: {path}");
    }
}

fn has_tailwind(sources: &[TokenSource]) -> bool {
    sources
        .iter()
        .any(|s| s.kind == TokenSourceKind::TailwindConfig)
}

fn display_path(path: &std::path::Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use plumb_core::Config;
    use std::path::PathBuf;

    fn fixture(sources: Vec<TokenSource>) -> InferredConfig {
        InferredConfig {
            config: Config::default(),
            summary: Vec::new(),
            sources,
        }
    }

    #[test]
    fn empty_inputs_render_with_header_note() {
        let rendered = render_toml(&fixture(Vec::new())).unwrap();
        assert!(rendered.starts_with(HEADER));
        assert!(rendered.contains("No design-token sources were discovered"));
    }

    #[test]
    fn renders_sources_in_stable_order() {
        let inferred = fixture(vec![
            TokenSource {
                kind: TokenSourceKind::CssCustomProperties,
                relative_path: PathBuf::from("z.css"),
            },
            TokenSource {
                kind: TokenSourceKind::CssCustomProperties,
                relative_path: PathBuf::from("a.css"),
            },
            TokenSource {
                kind: TokenSourceKind::TailwindConfig,
                relative_path: PathBuf::from("tailwind.config.ts"),
            },
        ]);
        let rendered = render_toml(&inferred).unwrap();
        let tw_pos = rendered.find("tailwind.config.ts").unwrap();
        let a_pos = rendered.find("a.css").unwrap();
        let z_pos = rendered.find("z.css").unwrap();
        assert!(tw_pos < a_pos, "tailwind should come first");
        assert!(a_pos < z_pos, "css files should be alphabetical");
        assert!(rendered.contains("Tailwind config detected"));
    }

    #[test]
    fn render_emits_canonical_toml_body() {
        let mut config = Config::default();
        config
            .color
            .tokens
            .insert("bg/canvas".into(), "#ffffff".into());
        let rendered = render_toml(&InferredConfig {
            config,
            summary: Vec::new(),
            sources: Vec::new(),
        })
        .unwrap();
        assert!(rendered.contains("[color]"));
        assert!(rendered.contains("bg/canvas"));
        assert!(rendered.ends_with('\n'));
    }
}
