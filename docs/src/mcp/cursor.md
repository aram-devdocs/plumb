# Cursor

Cursor supports MCP servers through its settings UI or a
`.cursor/mcp.json` file. See the
[MCP server reference](../mcp.md) for the full tool list and response
shapes.

## Install the server

Make sure the `plumb` binary is on your `PATH`. If you installed via
`cargo install plumb-cli`, it should already be available. If you built
from source, confirm with `which plumb` or `where plumb` on Windows.

## Configure via `.cursor/mcp.json`

Create `.cursor/mcp.json` in your project root:

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

Alternatively, open Cursor Settings → Features → MCP Servers → Add
Server, then enter the command `plumb` with arguments `mcp`.

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

After saving the config, restart Cursor or reload the MCP connection
from Settings → Features → MCP Servers. The server should appear as
connected with its tools listed.

Test the transport:

> Use plumb's echo tool to send "hello".

## Lint a page

Ask Cursor's agent:

> Use plumb to lint https://example.com

The agent calls `lint_url` and returns the violation summary. Request
`detail: "full"` for the complete JSON output.

## Gotchas

**PATH resolution.** Cursor may not inherit your shell's full `PATH`,
especially on macOS where GUI apps get a minimal environment. If
`plumb` is not found, use the absolute path in the config:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "/Users/you/.cargo/bin/plumb",
      "args": ["mcp"]
    }
  }
}
```

**Working directory.** The MCP server resolves `plumb.toml` relative
to the working directory where Cursor launches the command. This is
usually your project root. Place `plumb.toml` there.

**Tool approval.** Cursor may prompt you to approve MCP tool calls
on first use. Accept the prompt to allow Plumb tools.

**Large responses.** The 50 KB cap on `detail: "full"` applies here
as well. Use `compact` mode (the default) for pages with many
violations.

## See also

- [MCP server reference](../mcp.md) — tool list, response shapes,
  resource URIs.
- [Configuration](../configuration.md) — `plumb.toml` reference.
- [Install](../install.md) — binary installation options.
