# Rule: Documentation conventions

Plumb's docs live in two places:

- **`docs/src/`** — the user-facing Book rendered at `plumb.aramhammoudeh.com` by mdBook.
- **Rustdoc** — the API reference rendered at `docs.rs/plumb-core` (etc.).

Both must be human-readable, specific, and concise.

## Anti-AI-writing list

Avoid these phrases and shapes when writing docs:

- "dive in" / "dive into"
- "comprehensive" / "comprehensively"
- "leverage" (as a verb)
- "seamless" / "seamlessly"
- "streamline" / "streamlined"
- "unleash" / "unlock"
- "journey" (for non-narrative contexts)
- "delve" / "delves"
- "elevate" / "elevates"
- "in the world of" / "in today's fast-paced"
- Excess hedging: "generally speaking, it's possible that…"
- Bullet lists where prose would be shorter.
- Three-clause sentences with a rhetorical flourish in the third.

The `humanizer` skill flags these. Every PR that touches `docs/src/**`
runs it before merge.

## RFC 2119 keywords

When documenting a contract (the Rule trait, the MCP protocol, config
file semantics), use **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**,
**MAY** per RFC 2119. Lowercase the same words for non-normative prose.

## Rustdoc checklist

- Every public item documented (`missing_docs` denies this).
- Examples compile (`cargo test --doc` must pass).
- Cross-reference with intra-doc links, not raw URLs.
- `# Errors` section on every `-> Result<_, _>` public fn.
- `# Panics` section on any public fn that panics (prefer not to).
