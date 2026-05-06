//! The fixture matrix. Adding a new fixture under `e2e-sites/` requires
//! appending a [`SiteMeta`] entry here so the harness picks it up.

/// Per-site metadata used by the harness.
///
/// `name` is both the directory name under `e2e-sites/` and the slug
/// used in CLI flags / matrix legs. `requires_node` controls whether
/// the harness will skip the fixture when Node is not present (the
/// vanilla `html-css` fixture is the only one that does not need Node).
#[derive(Debug, Clone, Copy)]
pub struct SiteMeta {
    /// Slug. Matches `e2e-sites/<name>/`.
    pub name: &'static str,
    /// Whether the fixture's build step requires Node + npm.
    pub requires_node: bool,
}

/// Every fixture the harness knows about, in a stable order.
///
/// The CI matrix in `.github/workflows/e2e-sites.yml` mirrors this list.
/// Insert in the canonical order (vanilla → tailwind → frameworks
/// alphabetically); the harness sorts by `name` before iterating so
/// the order here is a documentation aid, not a contract.
pub const SITES: &[SiteMeta] = &[
    SiteMeta {
        name: "html-css",
        requires_node: false,
    },
    SiteMeta {
        name: "tailwind-html",
        requires_node: true,
    },
    SiteMeta {
        name: "react-vite",
        requires_node: true,
    },
    SiteMeta {
        name: "vue",
        requires_node: true,
    },
    SiteMeta {
        name: "angular",
        requires_node: true,
    },
    SiteMeta {
        name: "nextjs",
        requires_node: true,
    },
];

/// Look up a fixture by name.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static SiteMeta> {
    SITES.iter().find(|s| s.name == name)
}
