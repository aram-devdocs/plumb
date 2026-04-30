# Codex

> **Reviewer check:** The Codex MCP config shape below is based on
> published documentation as of April 2026. If you have access to a
> Codex environment, verify that the config path and format are
> correct.

OpenAI Codex connects to MCP servers through a `.codex/mcp.json` file
in your project root. See the
[MCP server reference](../mcp.md) for the full tool list and response
shapes.

## Install the server

Make sure the `plumb` binary is on your `PATH`. If you installed via
`cargo install plumb-cli`, it should already be available.

## Configure `.codex/mcp.json`

Create `.codex/mcp.json` in your project root:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "plumb",
      "args": ["mcp"]
    }
  }
}
```

For a source checkout:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "cargo",
      "args": ["run", "--quiet", "-p", "plumb-cli", "--", "mcp"]
    }
  }
}
```

## Verify the connection

After saving the config, start a new Codex session in the project
directory. Codex should detect the MCP server and make its tools
available.

Test the transport:

> Use plumb's echo tool to send "hello".

## Lint a page

Ask Codex:

> Use plumb to lint https://example.com

Codex calls `lint_url` and returns the violation summary.

## Gotchas

**PATH resolution.** Codex runs commands in a sandboxed environment.
If `plumb` is not on the default `PATH`, use an absolute path in the
config:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "/home/you/.cargo/bin/plumb",
      "args": ["mcp"]
    }
  }
}
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
