# Plumb v0.6 is here

Plumb now has a public docs site, a better place for release notes, and
an open Discussions tab for questions, ideas, and field reports.

If you want to see the project as it exists today, start with
[plumb.aramhammoudeh.com](https://plumb.aramhammoudeh.com/). That is
the current public docs URL.

## What shipped in v0.6

This release is about making Plumb easier to follow from the outside.
The book is live, the roadmap issue is pinned, and GitHub Discussions is
open.

Plumb itself is still a deterministic design-system linter for rendered
websites. It has two entry points:

- `plumb lint <url>` for local runs and CI
- `plumb mcp` for AI coding agents

## Install

Plumb is published to crates.io, npm, and a Homebrew tap, with a curl
install script and prebuilt platform archives. The full set of channels
lives on the [Install](../install.md) page; the [Quick start](../quickstart.md)
walks through the first run.

If you want to lint the live docs site as a smoke test:

```bash
plumb lint https://plumb.aramhammoudeh.com
```

## Demo and docs

The live docs site is the easiest demo at the moment:

- [Docs home](https://plumb.aramhammoudeh.com/)
- [Install](https://plumb.aramhammoudeh.com/install.html)
- [Quick start](https://plumb.aramhammoudeh.com/quickstart.html)

## Join the discussion

GitHub Discussions is now on for the repo:

- [GitHub Discussions](https://github.com/aram-devdocs/plumb/discussions)
- [Pinned roadmap issue](https://github.com/aram-devdocs/plumb/issues/56)

Use Discussions for setup questions, rule ideas, workflow feedback, or
examples from real sites. If you hit a concrete bug, open a GitHub
issue instead.

## Contact

The best public contact point right now is GitHub:

- start a thread in [Discussions](https://github.com/aram-devdocs/plumb/discussions)
- file a bug in [Issues](https://github.com/aram-devdocs/plumb/issues)

That keeps the conversation attached to the code and the roadmap.
