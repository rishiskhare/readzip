# Agent Support

## Claude Code

Claude Code is the primary integration. READZIP installs a `PreToolUse` hook for native `Read`.

Large full-file reads are blocked with a structured `permissionDecision: "deny"` response containing a skeleton. Scoped reads with `offset` or `limit` are allowed.

## Codex

Codex is MCP-only. Native read interception is not available, so savings depend on the agent choosing `readzip_skeleton` instead of native `read_file`.

The installer writes an `AGENTS.md` snippet, but that is advisory. Codex may still call native `read_file`, especially for small files. Real savings only happen when it chooses the READZIP MCP tools.

Verify actual usage with:

```bash
readzip stats
```

## Cursor, Cline, Windsurf, Gemini CLI

Use the MCP server:

```bash
readzip mcp
```

These integrations are opt-in and agent-dependent.
