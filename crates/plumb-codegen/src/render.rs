//! Render an [`InferredConfig`] to a `plumb.toml` string.
//!
//! The output starts with a generated header comment that records, in
//! sorted order, every source the inference pass consumed. The body is
//! the [`plumb_core::Config`] serialized via `toml::to_string_pretty`,
//! preserving `IndexMap` insertion order for tokens.

use crate::{CodegenError, InferredConfig, TokenSource, TokenSourceKind};

/// Comment header byline. Single line so the rendered file's first
/// content line is always the same number of bytes regardless of
/// whether the inferrer found anything.
const HEADER: &str =
    "# Plumb configuration — bootstrapped by `plumb init --from <path>` from the sources below.";

/// Note appended when a Tailwind config was discovered. Plumb's
/// `extends = "./tailwind.config.*"` directive is still in flight; this
/// wording is shared with `examples/plumb-tailwind.toml`.
const TAILWIND_HINT: &str =
    "# Tailwind config detected. Plumb merges Tailwind theme tokens at lint time —";

/// Render `inferred` to a TOML string.
///
/// The output is byte-identical given the same [`InferredConfig`]. The
/// header lists discovered source files in stable order; the body is
/// `toml::to_string_pretty` over the [`plumb_core::Config`].
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
        out.push_str("# run `plumb lint` from the same directory and the adapter will resolve Tailwind's theme.\n");
    }

    out.push('\n');

    let body = toml::to_string_pretty(&inferred.config)?;
    out.push_str(&body);

    // Always end on a single newline. `toml::to_string_pretty` emits
    // one already; defensively ensure the file ends consistently.
    if !out.ends_with('\n') {
        out.push('\n');
    }

    Ok(out)
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
