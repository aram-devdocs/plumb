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

## Common issues

> PATH resolution, working directory, large responses, and sandboxed
> network access apply to every agent integration. See
> [Common issues](../mcp.md#common-issues) for the consolidated list.

The Codex-specific note: register the server with an absolute path
when the sandbox `PATH` does not include `plumb`:

```sh
codex mcp add plumb -- /home/you/.cargo/bin/plumb mcp
```

Codex sandboxes may also block outbound network. Use
`plumb-fake://hello` to verify the tool chain without granting
network access; only `lint_url` against a real host needs the
network allowance.

## See also

- [MCP server reference](../mcp.md) — tool list, response shapes,
  resource URIs.
- [Configuration](../configuration.md) — `plumb.toml` reference.
- [Install](../install.md) — binary installation options.
