# Troubleshooting

## Claude Code is still reading full files

Run:

```bash
readzip doctor
```

Confirm `Claude hook installed: true`. If you already had a complex `~/.claude/settings.json`, `readzip init` may have written a merge snippet instead of editing your settings directly.

## MCP filesystem reads are not intercepted

READZIP intercepts Claude Code's native `Read` tool. If your agent uses an MCP filesystem server and calls a tool like `mcp__filesystem__read_file`, the native `Read` hook does not apply.

This is a known Claude Code limitation for MCP tool denials. Use native `Read` for transparent skeleton gating.

Public tracker reference: anthropics/claude-code issue `#33106`.

## Codex savings are inconsistent

Codex support is MCP-only. The agent may still choose native `read_file`, especially for small files. Use:

```bash
readzip stats
```

to verify whether READZIP intercepted any reads.
