# Agent Support

## Claude Code (transparent)

readzip installs a `PreToolUse` hook for native `Read`. When the agent calls `Read` on a file ≥500 lines without `offset`/`limit`, the hook returns a structured `permissionDecision: "deny"` carrying a tree-sitter skeleton in `permissionDecisionReason`. The agent sees the skeleton, picks the section it wants, and re-issues `Read(file_path, offset=N, limit=M)` — which passes through unchanged.

Auto-wired by `install.sh` if `~/.claude/` exists. Verify with `readzip doctor`.

## Every other agent (Codex, Cursor, Cline, Windsurf, Gemini CLI, Aider, …)

readzip is also a regular CLI. From the agent's perspective, three Bash commands replace `cat`:

```bash
readzip read <file>                       # smart cat: skeleton if large, full if small
readzip section <file> <offset> <limit>   # scoped slice (1-indexed lines)
readzip skeleton <file>                   # always print the skeleton
```

No MCP server, no plugin, no protocol shim. If the agent has shell access, it has readzip.

For **Codex** specifically, `readzip init` drops a hint file at `~/.codex/readzip-AGENTS-snippet.md` describing the three commands. Codex reads `AGENTS.md` files automatically, so the snippet gets surfaced into context.

For other agents, you can manually drop the equivalent text into your `CLAUDE.md` / `.cursorrules` / `.windsurfrules` / `AGENTS.md` / system prompt:

> When inspecting source files larger than ~500 lines, prefer `readzip read <file>` (smart cat: skeleton if large, full if small) and `readzip section <file> <offset> <limit>` (scoped line range) over plain `cat` or built-in `read_file`. Run `readzip stats` to see token savings.

Verify the agent is actually using it after a session: `readzip stats`.
