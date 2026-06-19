//! Brand banner for the `plumb` CLI.
//!
//! Renders a compact, TTY-aware brand header at the top of the
//! top-level help (`plumb --help`, and a bare `plumb` invocation). The
//! mark is the Plumb set square; the wordmark and tagline carry the
//! brand-blue accent (`#1a4faa`) only when stdout is a terminal and
//! `NO_COLOR` is unset, per <https://no-color.org>.
//!
//! The banner is attached via clap's `before_help` / `before_long_help`,
//! so it only renders when help is displayed. Running `plumb lint`,
//! `plumb init`, `plumb explain`, or the `plumb mcp` stdio server never
//! prints it — a hard requirement for `mcp`, whose stdout carries the
//! JSON-RPC stream.

use clap::Command;
use std::io::IsTerminal;

/// Brand blue (`#1a4faa`) as a bold truecolor SGR prefix.
const BRAND: &str = "\x1b[1;38;2;26;79;170m";
/// Faint (dim) SGR prefix for the version and tagline.
const DIM: &str = "\x1b[2m";
/// SGR reset.
const RESET: &str = "\x1b[0m";

/// The Plumb mark — a set square, rendered as a single angular glyph.
const MARK: &str = "◣";
/// The product tagline.
const TAGLINE: &str = "deterministic design-system linter for rendered websites";
/// The crate version, resolved at compile time (deterministic — no
/// wall-clock, no environment lookup beyond Cargo's build-time inject).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Whether the banner SHOULD emit ANSI colour.
///
/// Colour is emitted only when stdout is a terminal and `NO_COLOR` is
/// unset or empty. The `NO_COLOR` test matches `anstream`'s detection
/// (non-empty disables) so the banner and the rest of clap's coloured
/// `--help` agree. See <https://no-color.org>.
pub fn should_color() -> bool {
    std::io::stdout().is_terminal() && !no_color_env()
}

/// `true` when `NO_COLOR` is set to a non-empty value.
fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

/// The full two-line banner shown above the top-level help.
fn full(color: bool) -> String {
    if color {
        format!("{BRAND}{MARK} plumb{RESET} {DIM}{VERSION}{RESET}\n  {DIM}{TAGLINE}{RESET}")
    } else {
        format!("{MARK} plumb {VERSION}\n  {TAGLINE}")
    }
}

/// The compact one-line brand header shown above each subcommand's help.
fn line(color: bool) -> String {
    if color {
        format!("{BRAND}{MARK} plumb{RESET} {DIM}· {TAGLINE}{RESET}")
    } else {
        format!("{MARK} plumb · {TAGLINE}")
    }
}

/// Attach the brand banner to a clap [`Command`].
///
/// The top-level command gets the full two-line banner above its help;
/// every direct subcommand gets the compact one-line header (the task
/// contract: subcommand help need not repeat the full banner). The
/// banner only renders when help is displayed, so normal command
/// execution — including the `mcp` stdio server — never emits it.
pub fn brand(command: Command, color: bool) -> Command {
    let full_banner = full(color);
    let line_banner = line(color);

    let names: Vec<String> = command
        .get_subcommands()
        .map(|sub| sub.get_name().to_owned())
        .collect();

    let mut command = command
        .before_help(full_banner.clone())
        .before_long_help(full_banner);

    for name in names {
        let header = line_banner.clone();
        command = command.mut_subcommand(name, move |sub| {
            sub.before_help(header.clone()).before_long_help(header)
        });
    }

    command
}

#[cfg(test)]
mod tests {
    use super::{TAGLINE, brand, full, line};
    use clap::Command;

    /// The plain (no-colour) banner MUST contain no ANSI escape bytes.
    /// This is the contract verified end-to-end by piping `plumb --help`
    /// into a non-terminal.
    #[test]
    fn plain_banner_has_no_escape_codes() {
        let plain_full = full(false);
        let plain_line = line(false);
        assert!(!plain_full.contains('\x1b'), "full banner leaked an escape");
        assert!(!plain_line.contains('\x1b'), "line banner leaked an escape");
        assert!(plain_full.contains("plumb"));
        assert!(plain_full.contains(TAGLINE));
    }

    /// The coloured banner MUST carry the brand-blue truecolor SGR and a
    /// reset, and still spell out the wordmark and tagline as literal
    /// text between the escapes.
    #[test]
    fn coloured_banner_wraps_text_in_brand_blue() {
        let coloured = full(true);
        assert!(
            coloured.contains("\x1b[1;38;2;26;79;170m"),
            "missing brand blue"
        );
        assert!(coloured.contains("\x1b[0m"), "missing reset");
        assert!(coloured.contains("plumb"));
        assert!(coloured.contains(TAGLINE));
    }

    /// `brand` MUST attach `before_help` to the root command and to each
    /// subcommand without dropping any subcommand.
    #[test]
    fn brand_attaches_help_to_root_and_subcommands() {
        let base = Command::new("plumb")
            .subcommand(Command::new("lint"))
            .subcommand(Command::new("mcp"));
        let branded = brand(base, false);

        assert!(branded.get_before_help().is_some(), "root lost its banner");
        let sub_names: Vec<_> = branded
            .get_subcommands()
            .map(clap::Command::get_name)
            .collect();
        assert!(sub_names.contains(&"lint"));
        assert!(sub_names.contains(&"mcp"));
        for sub in branded.get_subcommands() {
            assert!(
                sub.get_before_help().is_some(),
                "subcommand {} lost its header",
                sub.get_name()
            );
        }
    }
}
