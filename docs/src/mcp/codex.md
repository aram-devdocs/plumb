# Codex

OpenAI Codex manages MCP servers through the `codex mcp` CLI. See the
[MCP server reference](../mcp.md) for the full tool list and response
shapes.

## Install the server

Make sure the `plumb` binary is on your `PATH`. If you installed via
`cargo install plumb-cli`, it should already be available.

## Register the server

Add Plumb as an MCP server:

```sh
codex mcp add plumb -- plumb mcp
```

For a source checkout (useful when hacking on Plumb itself):

```sh
codex mcp add plumb -- cargo run --quiet -p plumb-cli -- mcp
```

Confirm the registration:

```sh
codex mcp list
```

You should see `plumb` in the output.

## Verify the connection

Start a new Codex session in the project directory. Codex picks up the
registered server and makes its tools available.

Test the transport:

> Use plumb's echo tool to send "hello".

## Lint a page

Ask Codex:

> Use plumb to lint https://example.com

Codex calls `lint_url` and returns the violation summary.

## Gotchas

**PATH resolution.** Codex runs commands in a sandboxed environment.
If `plumb` is not on the default `PATH`, register the server with an
absolute path:

```sh
codex mcp add plumb -- /home/you/.cargo/bin/plumb mcp
```

**Network access.** Codex sandboxes may restrict outbound network
access. `lint_url` with real URLs requires the sandbox to allow
Chromium to connect to the target site. `plumb-fake://hello` works
without network access and is useful for verifying the tool chain.

**Working directory.** The MCP server resolves `plumb.toml` from the
working directory. Place your `plumb.toml` in the project root where
Codex runs.

**Large responses.** The same 50 KB cap on `detail: "full"` applies.
Use `compact` mode for pages with many violations.

## See also

- [MCP server reference](../mcp.md) — tool list, response shapes,
  resource URIs.
- [Configuration](../configuration.md) — `plumb.toml` reference.
- [Install](../install.md) — binary installation options.
