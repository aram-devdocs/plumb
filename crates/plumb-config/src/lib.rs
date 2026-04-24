//! # plumb-config
//!
//! Config loading + JSON Schema emission for Plumb.
//!
//! Accepts TOML, YAML, or JSON on disk; emits a single JSON Schema for
//! editor autocompletion.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::ops::Range;
use std::path::Path;

use figment::Figment;
use figment::providers::{Format, Json, Yaml};
use miette::{Diagnostic, NamedSource, SourceSpan};
use plumb_core::Config;
use thiserror::Error;

/// Underlying config parse errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigParseSource {
    /// TOML parser or schema error.
    #[error("{0}")]
    Toml(#[from] toml::de::Error),
    /// Figment parser or schema error.
    #[error("{0}")]
    Figment(#[from] figment::Error),
}

/// Config-loading errors.
#[derive(Debug, Error, Diagnostic)]
#[non_exhaustive]
pub enum ConfigError {
    /// File extension isn't one we recognize.
    #[error("unsupported config extension `{0}` (expected .toml, .yaml, .yml, or .json)")]
    UnsupportedExtension(String),
    /// The file is missing.
    #[error("config file not found: {0}")]
    NotFound(String),
    /// The file exists but could not be read.
    #[error("failed to read config file `{path}`: {source}")]
    Read {
        /// Path that failed to read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The file exists but couldn't be parsed or the content didn't
    /// match the config schema.
    #[error("failed to parse config file `{path}`: {source}")]
    #[diagnostic(code(plumb::config::parse))]
    Parse {
        /// Path that failed to parse.
        path: String,
        /// Underlying parse error.
        #[source]
        source: ConfigParseSource,
        /// Source text for span-annotated diagnostics.
        #[source_code]
        source_code: Option<NamedSource<String>>,
        /// Label pointing at the invalid TOML span, when available.
        #[label("invalid config")]
        span: Option<SourceSpan>,
    },
    /// Schema emission failed.
    #[error("failed to emit schema: {0}")]
    Schema(#[source] serde_json::Error),
}

/// Load a `Config` from disk. The file extension decides the parser.
///
/// # Errors
///
/// Returns [`ConfigError::NotFound`] if the file is missing,
/// [`ConfigError::UnsupportedExtension`] if the extension is unrecognized,
/// or [`ConfigError::Parse`] if parsing or schema validation fails.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.display().to_string()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "toml" => load_toml(path),
        "yaml" | "yml" => {
            let figment = Figment::new().merge(Yaml::file(path));
            extract_config(&figment, path)
        }
        "json" => {
            let figment = Figment::new().merge(Json::file(path));
            extract_config(&figment, path)
        }
        other => Err(ConfigError::UnsupportedExtension(other.to_owned())),
    }
}

fn extract_config(figment: &Figment, path: &Path) -> Result<Config, ConfigError> {
    figment
        .extract::<Config>()
        .map_err(|source| ConfigError::Parse {
            path: config_error_path(&source).unwrap_or_else(|| path.display().to_string()),
            source: ConfigParseSource::Figment(source),
            source_code: None,
            span: None,
        })
}

fn load_toml(path: &Path) -> Result<Config, ConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;

    toml::from_str::<Config>(&contents).map_err(|source| {
        let span = source.span().and_then(source_span);
        ConfigError::Parse {
            path: path.display().to_string(),
            source: ConfigParseSource::Toml(source),
            source_code: Some(
                NamedSource::new(path.display().to_string(), contents).with_language("toml"),
            ),
            span,
        }
    })
}

fn source_span(range: Range<usize>) -> Option<SourceSpan> {
    let len = range.end.checked_sub(range.start)?;
    Some((range.start, len).into())
}

fn config_error_path(source: &figment::Error) -> Option<String> {
    source
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.source.as_ref())
        .map(ToString::to_string)
}

/// Emit the JSON Schema for [`Config`] as a pretty-printed string.
///
/// # Errors
///
/// Returns [`ConfigError::Schema`] if JSON serialization fails.
pub fn emit_schema() -> Result<String, ConfigError> {
    let schema = schemars::schema_for!(Config);
    serde_json::to_string_pretty(&schema).map_err(ConfigError::Schema)
}
