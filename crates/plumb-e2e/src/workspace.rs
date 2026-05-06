//! Workspace-root discovery for the e2e harness.
//!
//! The harness needs to locate `e2e-sites/` and the `target/` directory
//! the `plumb` binary is built into. Both are anchored at the workspace
//! root, identified by the workspace `Cargo.toml`. We walk upward from
//! `CARGO_MANIFEST_DIR` until we find a `Cargo.toml` that declares
//! `[workspace]`.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

/// Locate the workspace root by walking up from `start` until a
/// `Cargo.toml` containing `[workspace]` is found.
///
/// Falls back to walking up from `CARGO_MANIFEST_DIR` if `start` is
/// `None`. Returns the directory containing the workspace `Cargo.toml`.
///
/// # Errors
///
/// Returns an error if no workspace root is found before the
/// filesystem root.
pub fn find_workspace_root(start: Option<&Path>) -> Result<PathBuf, anyhow::Error> {
    let initial = match start {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    };
    let mut cursor: Option<&Path> = Some(initial.as_path());
    while let Some(dir) = cursor {
        let candidate = dir.join("Cargo.toml");
        if candidate.is_file() {
            let bytes = std::fs::read(&candidate)
                .with_context(|| format!("read {}", candidate.display()))?;
            let text = String::from_utf8_lossy(&bytes);
            if text.contains("[workspace]") {
                return Ok(dir.to_path_buf());
            }
        }
        cursor = dir.parent();
    }
    Err(anyhow::anyhow!(
        "no workspace Cargo.toml found above {}",
        initial.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::find_workspace_root;

    #[test]
    fn finds_workspace_from_crate_dir() {
        let root = find_workspace_root(None).expect("workspace exists");
        // The workspace root must contain `Cargo.toml` and `e2e-sites/`.
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("e2e-sites").is_dir());
    }
}
