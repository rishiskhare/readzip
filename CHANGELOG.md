# Changelog

## 0.1.0

Initial public release.

### Skeleton extraction
- Tree-sitter parsing for Python, JavaScript, TypeScript, Go, Rust, Java, Ruby, C, C++, C#, PHP, Swift, Kotlin, Scala, Lua, Bash. Symbol end-lines come from the parse tree, not indent heuristics.
- Heuristic line-prefix fallback when tree-sitter parses a file with > 5% ERROR / MISSING nodes (broken or partially-edited source).
- Skeleton elision marker emits an explicit `L<start>-<end> [N lines elided — Read offset=<start>]` hint instead of a generic truncation message when the cap is hit.

### Hook & cache
- Claude Code `PreToolUse` hook returns `permissionDecision: "deny"` with the skeleton in `permissionDecisionReason` — the path that's reliable across current Claude Code versions. Does not depend on `updatedInput`.
- Cache validates by mtime + size on the fast path; falls through to a content-hash bypass for files that were touched (git checkout, formatter no-op) but not actually changed.

### Multi-agent installer
- `readzip init` registers integrations for Claude Code (native hook), Codex (MCP + AGENTS.md hint), Cursor (MCP), Cline (MCP), Windsurf (MCP), and Gemini CLI (MCP).
- `--only=a,b` and `--skip=a,b` flags scope the install. `--yes` skips the confirmation prompt for modifying existing settings.
- `readzip uninstall` removes the readzip hook from Claude Code and the MCP entries from every supported agent.

### CLI
- `readzip eval <dir>` walks a corpus and reports total token savings as a markdown table or JSON.
- `readzip stats` is always-on local-only telemetry — no opt-in flag. Reports files intercepted, tokens saved, average reduction, and context-window equivalents.
- `readzip doctor` reports per-agent MCP installation status alongside the Claude hook check.

### Distribution
- Single static Rust binary (~28 MB with all 16 grammars bundled, no runtime parser deps).
- One-line install: `curl -fsSL https://raw.githubusercontent.com/rishiskhare/readzip/main/install.sh | sh`.
- Pre-built tarballs for x86_64 + aarch64 on macOS and Linux.
- `cargo install --git https://github.com/rishiskhare/readzip readzip-cli` as a fallback.

### Privacy
- Local-only. Tree-sitter parses on-disk; no network calls. Stats are written to `~/.cache/readzip/stats.tsv` and never leave the machine.
