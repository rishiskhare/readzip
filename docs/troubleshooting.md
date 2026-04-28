# Troubleshooting

## Claude Code is still reading full files

```bash
readzip doctor
```

Confirm `Claude hook installed: true`. If not, rerun `readzip init --yes`. If the hook is registered but still not firing, restart Claude Code so it reloads `~/.claude/settings.json`.

## MCP filesystem reads bypass readzip

readzip's hook fires on Claude Code's **native** `Read` tool. If your agent uses an MCP filesystem server and calls a tool like `mcp__filesystem__read_file`, the native `Read` hook does not apply.

This is a known Claude Code limitation tracked at [anthropics/claude-code#33106](https://github.com/anthropics/claude-code/issues/33106) — `PreToolUse` deny isn't enforced for MCP server tools yet. Workaround: disable the MCP filesystem server when using readzip, or rely on Claude Code's native `Read` for the affected files.

## Non-Claude agent isn't saving tokens

readzip is a CLI for non-Claude agents — they have to actually call it. If you're seeing zero savings on Codex / Cursor / Cline / Windsurf / Gemini after a session:

```bash
readzip stats
```

If `files intercepted: 0`, the agent isn't running `readzip read` from Bash. Check that it's mentioned in the agent's instructions (`AGENTS.md` / `.cursorrules` / `.windsurfrules` / system prompt). The README's [Quick start — any other agent](../README.md#quick-start--any-other-agent-codex-cursor-cline-windsurf-gemini-aider-) section has the recommended snippet.

## `readzip: command not found`

Add `~/.local/bin` to your PATH:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
# or ~/.bashrc
```

Then re-open the terminal and run `readzip --version` to confirm.
