//! # plumb-config
//!
//! Config loading + JSON Schema emission for Plumb.
//!
//! Accepts TOML, YAML, or JSON on disk; emits a single JSON Schema for
//! editor autocompletion.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use figment::Figment;
use figment::providers::{Format, Json, Toml, Yaml};
use plumb_core::Config;
use thiserror::Error;

/// Config-loading errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// File extension isn't one we recognize.
    #[error("unsupported config extension `{0}` (expected .toml, .yaml, .yml, or .json)")]
    UnsupportedExtension(String),
    /// The file is missing.
    #[error("config file not found: {0}")]
    NotFound(String),
    /// The file exists but couldn't be parsed or the content didn't
    /// match the config schema.
    #[error("failed to parse config file `{path}`: {source}")]
    Parse {
        /// Path that failed to parse.
        path: String,
        /// Underlying figment error.
        #[source]
        source: Box<figment::Error>,
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

    let figment = match ext.as_str() {
        "toml" => Figment::new().merge(Toml::file(path)),
        "yaml" | "yml" => Figment::new().merge(Yaml::file(path)),
        "json" => Figment::new().merge(Json::file(path)),
        other => return Err(ConfigError::UnsupportedExtension(other.to_owned())),
    };

    figment
        .extract::<Config>()
        .map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source: Box::new(source),
        })
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
