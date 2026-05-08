# Claude Code

Claude Code connects to MCP servers through a `.mcp.json` file in
your project root or home directory. See the
[MCP server reference](../mcp.md) for the full tool list and response
shapes.

## Install the server

If you installed Plumb via `cargo install plumb-cli`, the `plumb`
binary is already on your `PATH`. If you built from source, make sure
the binary is accessible from the directory where Claude Code runs.

## Configure `.mcp.json`

Create or edit `.mcp.json` in your project root:

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

For a source checkout (useful when hacking on Plumb itself):

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

After saving `.mcp.json`, restart Claude Code or run `/mcp` in the
Claude Code prompt to list connected servers. You should see `plumb`
with its tools (`lint_url`, `explain_rule`, `list_rules`, `get_config`,
`echo`).

Run a quick smoke test by asking Claude Code:

> Use plumb's echo tool to send "hello".

If the tool returns your message, the transport is working.

## Lint a page

Ask Claude Code:

> Use plumb to lint https://example.com

Claude Code calls `lint_url` and returns a compact summary of
violations. Use `detail: "full"` when you need the complete JSON
envelope (capped at 50 KB).

## Common issues

> PATH resolution, working directory, large responses, and tool
> approval prompts apply to every agent integration. See
> [Common issues](../mcp.md#common-issues) for the consolidated list.

The Claude-specific note: when `plumb` is not on the GUI-process
`PATH`, point `.mcp.json` at the absolute binary path (e.g.
`/Users/you/.cargo/bin/plumb`) rather than the bare `plumb` command.

## See also

- [MCP server reference](../mcp.md) — tool list, response shapes,
  resource URIs.
- [Configuration](../configuration.md) — `plumb.toml` reference.
- [Install](../install.md) — binary installation options.
