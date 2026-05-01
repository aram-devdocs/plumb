# Plumb v0.6 is here

Plumb now has a public docs site, a better place for release notes, and
an open Discussions tab for questions, ideas, and field reports.

If you want to see the project as it exists today, start with
[plumb.aramhammoudeh.com](https://plumb.aramhammoudeh.com/). That is
the current public docs URL. `plumb.dev` is not the canonical docs
domain yet, so keep using the `aramhammoudeh.com` address for now.

## What shipped in v0.6

This release is about making Plumb easier to follow from the outside.
The book is live, the roadmap issue is pinned, and GitHub Discussions is
open.

Plumb itself is still a deterministic design-system linter for rendered
websites. It has two entry points:

- `plumb lint <url>` for local runs and CI
- `plumb mcp` for AI coding agents

## Install status

The install page in the book lists four channels:

- install script
- `cargo install`
- Homebrew
- build from source

Today, the supported path is still build from source. The other channels
are documented so the book does not need a rewrite on release day, but
they do not replace the source build yet.

If you want to try Plumb right now:

```bash
git clone https://github.com/aram-devdocs/plumb
cd plumb
just setup
just build-release
target/release/plumb --version
target/release/plumb lint plumb-fake://hello
```

The full setup notes live on the [Install](../install.md) and
[Quick start](../quickstart.md) pages.

## Demo and docs

The live docs site is the easiest demo at the moment:

- [Docs home](https://plumb.aramhammoudeh.com/)
- [Install](https://plumb.aramhammoudeh.com/install.html)
- [Quick start](https://plumb.aramhammoudeh.com/quickstart.html)

If you already have a local build, you can lint the live site directly:

```bash
plumb lint https://plumb.aramhammoudeh.com
```

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
