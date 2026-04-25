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

mod span;
mod validate;

use span::{SourceFormat, locate_path};
use validate::ValidationIssue;

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
        source: Box<ConfigParseSource>,
        /// Source text for span-annotated diagnostics.
        #[source_code]
        source_code: Option<NamedSource<String>>,
        /// Label pointing at the invalid config span, when available.
        #[label("invalid config")]
        span: Option<SourceSpan>,
    },
    /// The file parsed structurally but failed semantic validation
    /// (e.g. a palette token whose value isn't a hex color).
    #[error("invalid config value at `{value_path}` in `{path}`: {message}")]
    #[diagnostic(code(plumb::config::validation))]
    Validation {
        /// Path of the file that failed validation.
        path: String,
        /// Dotted path of the offending value (e.g. `color.tokens.bg`).
        value_path: String,
        /// Why the value is invalid.
        message: String,
        /// Source text for span-annotated diagnostics.
        #[source_code]
        source_code: Option<NamedSource<String>>,
        /// Label pointing at the offending value, when the source format
        /// allows span recovery.
        #[label("invalid value")]
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
/// [`ConfigError::Parse`] if structural parsing fails, or
/// [`ConfigError::Validation`] if a value fails semantic validation
/// (e.g. a non-hex palette token).
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.display().to_string()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let (config, contents, format) = match ext.as_str() {
        "toml" => {
            let (cfg, body) = load_toml(path)?;
            (cfg, body, SourceFormat::Toml)
        }
        "yaml" | "yml" => {
            let (cfg, body) = load_yaml(path)?;
            (cfg, body, SourceFormat::Yaml)
        }
        "json" => {
            let (cfg, body) = load_json(path)?;
            (cfg, body, SourceFormat::Json)
        }
        other => return Err(ConfigError::UnsupportedExtension(other.to_owned())),
    };

    if let Some(issue) = validate::validate(&config) {
        return Err(validation_error(path, contents, format, issue));
    }

    Ok(config)
}

fn validation_error(
    path: &Path,
    contents: String,
    format: SourceFormat,
    issue: ValidationIssue,
) -> ConfigError {
    let span = locate_path(&contents, format, &issue.path_segments);
    let language = match format {
        SourceFormat::Toml => "toml",
        SourceFormat::Yaml => "yaml",
        SourceFormat::Json => "json",
    };
    ConfigError::Validation {
        path: path.display().to_string(),
        value_path: issue.path_segments.join("."),
        message: issue.message,
        source_code: Some(
            NamedSource::new(path.display().to_string(), contents).with_language(language),
        ),
        span,
    }
}

fn load_toml(path: &Path) -> Result<(Config, String), ConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let parsed = toml::from_str::<Config>(&contents).map_err(|source| {
        let span = source.span().and_then(source_span);
        ConfigError::Parse {
            path: path.display().to_string(),
            source: Box::new(ConfigParseSource::Toml(source)),
            source_code: Some(
                NamedSource::new(path.display().to_string(), contents.clone())
                    .with_language("toml"),
            ),
            span,
        }
    })?;

    Ok((parsed, contents))
}

fn load_yaml(path: &Path) -> Result<(Config, String), ConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let figment = Figment::new().merge(Yaml::file(path));
    let cfg = figment
        .extract::<Config>()
        .map_err(|source| build_figment_parse_error(path, &contents, SourceFormat::Yaml, source))?;
    Ok((cfg, contents))
}

fn load_json(path: &Path) -> Result<(Config, String), ConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let figment = Figment::new().merge(Json::file(path));
    let cfg = figment
        .extract::<Config>()
        .map_err(|source| build_figment_parse_error(path, &contents, SourceFormat::Json, source))?;
    Ok((cfg, contents))
}

fn build_figment_parse_error(
    path: &Path,
    contents: &str,
    format: SourceFormat,
    source: figment::Error,
) -> ConfigError {
    let segments: Vec<String> = source.path.clone();
    let span = if segments.is_empty() {
        None
    } else {
        locate_path(contents, format, &segments)
    };
    let language = match format {
        SourceFormat::Toml => "toml",
        SourceFormat::Yaml => "yaml",
        SourceFormat::Json => "json",
    };
    let display_path = config_error_path(&source).unwrap_or_else(|| path.display().to_string());
    ConfigError::Parse {
        path: display_path,
        source: Box::new(ConfigParseSource::Figment(source)),
        source_code: Some(
            NamedSource::new(path.display().to_string(), contents.to_owned())
                .with_language(language),
        ),
        span,
    }
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
