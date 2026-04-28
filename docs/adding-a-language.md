# Adding a Language

readzip ships with 16 languages out of the box (Python, JavaScript, TypeScript, Go, Rust, Java, Ruby, C, C++, C#, PHP, Swift, Kotlin, Scala, Lua, Bash). Each one is wired up via a tree-sitter grammar crate plus a small list of node kinds we treat as top-level symbols. Adding a new language is a 4-file PR.

## Steps

### 1. Add the grammar crate

Edit `crates/readzip-core/Cargo.toml` and add the tree-sitter grammar dependency:

```toml
tree-sitter-elixir = "0.3"
```

Most language grammars publish to crates.io as `tree-sitter-<name>` and export a `LANGUAGE: LanguageFn` constant.

### 2. Add the Language variant

Edit `crates/readzip-core/src/lib.rs`:

```rust
pub enum Language {
    // ... existing variants ...
    Elixir,
    Unknown,
}
```

### 3. Wire up file extension detection

In the same file, find `detect_language` and add the extensions:

```rust
match ext {
    // ... existing arms ...
    "ex" | "exs" => Language::Elixir,
    _ => Language::Unknown,
}
```

Also add a name in `language_name`:

```rust
Language::Elixir => "Elixir",
```

### 4. Add the parser spec

Edit `crates/readzip-core/src/parsers.rs`. Add a match arm in `language_spec`:

```rust
Lang::Elixir => Some((
    tree_sitter_elixir::LANGUAGE.into(),
    &[
        ("call", "function"),         // Elixir's `def`/`defp` macros parse as `call` nodes.
        ("anonymous_function", "function"),
    ],
)),
```

The second tuple is `&[(node_kind_in_grammar, our_kind_label)]`. Open the grammar's `grammar.js` or `node-types.json` to find the right node kinds — for most languages these are obvious (`class_definition`, `function_declaration`, etc.). For more macro-heavy languages like Elixir or Ruby, you may need to walk through tree-sitter's playground (https://tree-sitter.github.io/tree-sitter/playground) on a sample file to see how it parses.

### 5. Add a heuristic fallback (optional but recommended)

In `lib.rs`, find `symbol_signature` and add a match arm. This kicks in when tree-sitter parse has > 5% ERROR nodes (broken or partial source):

```rust
Language::Elixir => {
    if trimmed.starts_with("def ") || trimmed.starts_with("defp ") {
        Some(("function", clean_signature(trimmed)))
    } else if trimmed.starts_with("defmodule ") {
        Some(("module", clean_signature(trimmed)))
    } else {
        None
    }
}
```

Also add the import-block prefixes used in `import_block_end`:

```rust
Language::Elixir => trimmed.starts_with("import ") || trimmed.starts_with("alias ") || trimmed.starts_with("require ") || trimmed.starts_with("use "),
```

### 6. Add a test

In the `tests` module at the bottom of `lib.rs`:

```rust
#[test]
fn extracts_elixir_symbols() {
    let source = "defmodule MyApp.Auth do\n  def login(user, pass) do\n    :ok\n  end\nend\n";
    let config = Config::default();
    let skeleton = build_skeleton_from_source(Path::new("auth.ex"), source, &config);
    assert!(skeleton.text.contains("MyApp.Auth"));
    assert!(skeleton.text.contains("login"));
}
```

Run:

```bash
cargo test -p readzip-core
```

### 7. Add an issue template entry

Optional: edit `.github/ISSUE_TEMPLATE/language_request.md` to remove the new language from the "not yet supported" list.

## Verification end-to-end

```bash
cargo build --release -p readzip-cli
./target/release/readzip skeleton path/to/some_real_elixir_file.ex
```

You should see the skeleton with line ranges. If tree-sitter fails to parse, the heuristic from step 5 takes over and you'll still get a usable skeleton.

To check error rates on a real corpus:

```bash
./target/release/readzip eval path/to/elixir/repo/ --json | jq .reduction_percent
```

Aim for >70% reduction on real files. If reduction is much lower, the spec is probably missing a node kind. Add it and re-run.

## Why two-tier (tree-sitter + heuristic)

Tree-sitter is precise but bails on broken / templated / partially-edited files — exactly the files an agent encounters most. The heuristic catches those cases and ships *something* useful instead of nothing. The fall-through threshold is 5% ERROR nodes by source byte coverage; tune in `parsers.rs::MAX_ERROR_RATIO` if a language consistently misclassifies.
