# Rule: How to add an MCP tool

MCP tools are the contract between Plumb and AI coding agents. Every
tool call costs token budget, so responses must be compact.

## Steps

### 1. Define the tool

In `crates/plumb-mcp/src/lib.rs`, add it to the `tools/list` response
with:

- **`name`** — lowercase `snake_case`.
- **`description`** — one sentence. Agents fan-call tools by description.
- **`inputSchema`** — minimal JSON Schema. Only required fields. No
  nested-object gymnastics.

### 2. Handle the call

Add a match arm in `tools_call`. Return a value shaped like PRD §14.2:

```json
{
  "content": [{ "type": "text", "text": "<compact human summary>" }],
  "isError": false,
  "structuredContent": { /* machine-readable payload */ }
}
```

The `text` block is what a chatty agent surfaces to the user. The
`structuredContent` block is what a tool-using agent parses. Both must
be present — never rely on the agent to re-parse text.

## Token efficiency rules

- Aggregate duplicate violations server-side; return counts + examples,
  not the full list.
- Cap response size. The default for `lint_url` should be ≤ 10 KB of
  `structuredContent`.
- Never echo the snapshot back — it's huge.

## Determinism

Tool responses are pure functions of inputs. No `SystemTime::now` leaking
into the `text` block. No random ordering.

## Testing

Every tool gets a case in `crates/plumb-mcp/tests/mcp_protocol.rs`:

- Happy path.
- Invalid args → clear JSON-RPC error.
- Cancellation semantics (when we add them).
