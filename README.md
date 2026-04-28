<div align="center">
  <img src="assets/brand/readzip.png" alt="READZIP" width="820">

  <p>
    <b>AST-aware Read for AI coding agents.</b><br/>
    Returns a structural skeleton with exact line ranges instead of dumping the whole file.<br/>
    <i>~80% fewer tokens on full-file reads. Zero new tools to learn.</i>
  </p>

  <p>
    <img alt="rust" src="https://img.shields.io/badge/rust-stable-4ea8ff?labelColor=0b1220">
    <img alt="license" src="https://img.shields.io/badge/license-MIT-7f88ff?labelColor=0b1220">
    <img alt="claude code" src="https://img.shields.io/badge/Claude%20Code-native%20hook-4ea8ff?labelColor=0b1220">
    <img alt="tree-sitter" src="https://img.shields.io/badge/tree--sitter-16%20languages-7f88ff?labelColor=0b1220">
  </p>
</div>

---

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/rishiskhare/readzip/main/install.sh | sh
readzip init
```

The installer pulls a pre-built binary for your OS/arch from [GitHub Releases](https://github.com/rishiskhare/readzip/releases) and drops it in `~/.local/bin`. `readzip init` wires up Claude Code (and any of Codex / Cursor / Cline / Windsurf / Gemini CLI it detects on your machine).

<details>
<summary>Other install methods</summary>

```bash
# Via Cargo
cargo install --git https://github.com/rishiskhare/readzip readzip-cli

# Pinned version
READZIP_VERSION=v0.1.0 curl -fsSL .../install.sh | sh

# Custom directory
READZIP_INSTALL_DIR=/usr/local/bin curl -fsSL .../install.sh | sh
```
</details>

## What it does

Your agent calls `Read("crates/readzip-cli/src/main.rs")` — a 1,372-line Rust file. Without readzip, that's 11,833 tokens of full source dumped into context, most of which the agent never references. With readzip, the call gets intercepted and the agent receives a tree-sitter-derived skeleton instead:

```text
# crates/readzip-cli/src/main.rs -- 1372 lines (Rust)

L15-39   fn main()
L48-85   fn init(args: &[String]) -> Result<(), String>
L100-125 impl AgentSelection
L163-226 fn hook() -> Result<(), String>
L391-439 fn doctor(args: &[String]) -> Result<(), String>
L466-628 fn eval_cmd(args: &[String]) -> Result<(), String>
L940-1002 fn install_claude_hook(yes: bool) -> Result<(), String>
... 68 entries total, every symbol with its exact line range
```

The agent then re-issues `Read("…/main.rs", offset=391, limit=49)` to pull just the doctor handler — about 530 tokens. Same `Read` tool, same call shape, no new APIs to learn.

**Real measurement on this repo's own crates:**

```text
$ readzip eval crates/

  files intercepted (lines >= 500): 2
  total original tokens: 19.6K
  total skeleton tokens:  2.7K
  tokens saved:          16.8K
  average reduction:     86.1%
```

Run it on your own codebase: `readzip eval ~/code/your-project/`.

## AST-aware, not heuristic

readzip uses **tree-sitter** — the same incremental parser GitHub, Neovim, Helix, and Zed use — to walk a real syntax tree for every supported language. Symbol end-lines come from the parse tree's actual closing position, not indent guesses.

| | |
|---|---|
| **Languages** | Python · JavaScript · TypeScript · Go · Rust · Java · Ruby · C · C++ · C# · PHP · Swift · Kotlin · Scala · Lua · Bash |
| **Fallback** | Heuristic line-prefix matching kicks in when tree-sitter parses with > 5% ERROR nodes (broken or partially-edited source). You always get *something* back. |
| **Runtime** | Single static binary. No Node, no Python, no per-language download. |

Adding a 17th language is a 4-file PR — see [docs/adding-a-language.md](docs/adding-a-language.md).

## How the hook works

readzip installs a Claude Code `PreToolUse` hook for native `Read`. When the input has no `offset` or `limit` and the file exceeds 500 lines (configurable), the hook returns a structured deny — not a rejection, but a navigation hint carrying the skeleton in `permissionDecisionReason`. The agent sees the skeleton and re-issues a scoped `Read(file_path, offset=N, limit=M)` for the section it actually wants. Scoped reads pass through unchanged.

This is the path that's reliable across current Claude Code versions. readzip deliberately does **not** rely on `updatedInput` rewrites — that field is documented but applied inconsistently in practice.

## Agent support

| Agent | Mode | Transparent? |
|---|---|:---:|
| **Claude Code** | Native `PreToolUse` hook on `Read` | ✓ |
| Codex | MCP server + AGENTS.md hint | – |
| Cursor / Cline / Windsurf / Gemini CLI | MCP server | – |

Only Claude Code is fully transparent. For the other agents, real savings depend on the agent *choosing* readzip's MCP tools (`readzip_skeleton`, `readzip_section`, `readzip_stats`) over its built-in file-read. Verify with `readzip stats`.

## Stats

Local-only. Always recording. Nothing leaves the machine.

```text
$ readzip stats

  files intercepted:    <N>
  tokens saved:         <K/M>
  avg reduction:        <P>%
  context windows:      ~<saved/200_000>
  cache dir:            ~/.cache/readzip
```

To wipe: `rm ~/.cache/readzip/stats.tsv`.

## Config

`~/.config/readzip/config.toml`

```toml
min_lines = 500              # files smaller than this pass through
max_skeleton_tokens = 1500   # cap skeleton size
skeleton_detail = "medium"   # minimal | medium | verbose
bypass_for = []              # globs that always pass through
force_full_for = ["*.md", "package.json"]
```

## Limitations

- **MCP filesystem servers are not intercepted.** Claude Code currently doesn't enforce `PreToolUse` deny on `mcp__filesystem__read_file`-style tools (issue [#33106](https://github.com/anthropics/claude-code/issues/33106)). Use native `Read` for transparent gating.
- **Codex doesn't expose native `read_file` hooks** — MCP-only there until the upstream surface lands.
- **Binary files and notebooks** pass through untouched.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p readzip-cli -- demo --json
cargo run -p readzip-cli -- eval crates/
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/](docs/) for the rest.

## License

[MIT](LICENSE)
