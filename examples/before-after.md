# Before / After

Real outputs from running readzip on real source files. Every number on this page is from a verbatim run on this repo. Reproduce them yourself:

```bash
git clone https://github.com/rishiskhare/readzip
cd readzip
cargo build --release -p readzip-cli
./target/release/readzip eval crates/
./target/release/readzip skeleton crates/readzip-core/src/lib.rs
```

## Eval on this repo (the dogfood)

```text
$ readzip eval crates/

# readzip eval

- readzip version: 0.1.0
- target(s): crates/
- min_lines threshold: 500
- files walked: 5
- source files recognized: 3
- files intercepted (lines >= 500): 2
- total original tokens: 19.6K
- total skeleton tokens: 2.7K
- tokens saved: 16.8K
- average reduction: 86.1%

| File | Lang | Lines | Original | Skeleton | Saved | Reduction |
|---|---|---:|---:|---:|---:|---:|
| crates/readzip-cli/src/main.rs | Rust | 1372 | 11833 | 1463 | 10370 | 87.6% |
| crates/readzip-core/src/lib.rs | Rust |  924 |  7723 | 1248 |  6475 | 83.8% |
```

## Single-file before/after — `crates/readzip-core/src/lib.rs`

A 924-line Rust file. Full read = 7,723 tokens. Skeleton = 1,248 tokens. **83.8% reduction.**

```text
$ readzip skeleton crates/readzip-core/src/lib.rs

# crates/readzip-core/src/lib.rs -- 924 lines (Rust) skeleton view
# Use Read(file_path="crates/readzip-core/src/lib.rs", offset=N, limit=M) for a specific section.

L1-7    imports / module header (Read offset=1 limit=7)
L8-8  mod mod parsers;  (Read offset=8 limit=1)
L14-22  struct pub struct Config  (Read offset=14 limit=9)
  # [derive(Debug, Clone)]
L25-29  enum pub enum SkeletonDetail  (Read offset=25 limit=5)
  # [derive(Debug, Clone, Copy, PartialEq, Eq)]
L32-40  struct pub struct Skeleton  (Read offset=32 limit=9)
  # [derive(Debug, Clone)]
L43-61  enum pub enum Language  (Read offset=43 limit=19)
  # [derive(Debug, Clone, Copy, PartialEq, Eq)]
L64-70  struct struct Symbol  (Read offset=64 limit=7)
  # [derive(Debug, Clone)]
L72-84  impl impl Default for Config  (Read offset=72 limit=13)
    L73-83  function fn default() -> Self  (Read offset=73 limit=11)
L86-91  function pub fn default_config_path() -> PathBuf  (Read offset=86 limit=6)
L93-98  function pub fn default_cache_dir() -> PathBuf  (Read offset=93 limit=6)
L100-115  function pub fn default_config_text(config: &Config) -> String  (Read offset=100 limit=16)
L117-160  function pub fn load_config() -> Config  (Read offset=117 limit=44)
L162-182  function pub fn should_intercept(path: &Path, line_count: usize, config: &Config) -> bool  (Read offset=162 limit=21)
L184-187  function pub fn build_skeleton(path: &Path, config: &Config) -> io::Result<Skeleton>  (Read offset=184 limit=4)
L189-225  function pub fn build_skeleton_from_source(path: &Path, source: &str, config: &Config) -> Skeleton  (Read offset=189 limit=37)
L227-313  function pub fn cached_skeleton(path: &Path, config: &Config) -> io::Result<Skeleton>  (Read offset=227 limit=87)
L315-338  function pub fn detect_language(path: &Path) -> Language  (Read offset=315 limit=24)
L340-360  function pub fn language_name(language: Language) -> &'static str  (Read offset=340 limit=21)
L362-366  function pub fn estimate_tokens(text: &str) -> usize  (Read offset=362 limit=5)
L368-372  function pub fn stable_hash(value: &str) -> String  (Read offset=368 limit=5)
L378-397  function fn extract_symbols_dispatch(language: Language, lines: &[&str], source: &str) -> Vec<Symbol>  (Read offset=378 limit=20)
  # - the parse returned no symbols (rare; usually means the file is data, not code).
L399-429  function fn extract_symbols(language: Language, lines: &[&str]) -> Vec<Symbol>  (Read offset=399 limit=31)
L431-543  function fn symbol_signature(language: Language, trimmed: &str) -> Option<(&'static str, String)>  (Read offset=431 limit=113)
L545-556  function fn looks_like_js_function(trimmed: &str) -> bool  (Read offset=545 limit=12)
L558-562  function fn clean_signature(line: &str) -> String  (Read offset=558 limit=5)
L564-621  function fn render_skeleton(  (Read offset=564 limit=58)
L623-654  function fn import_block_end(language: Language, lines: &[&str]) -> usize  (Read offset=623 limit=32)
L656-679  function fn nearby_doc(lines: &[&str], line: usize) -> Option<String>  (Read offset=656 limit=24)
L681-689  function fn first_body_line(lines: &[&str], line: usize, end_line: usize) -> Option<String>  (Read offset=681 limit=9)
L691-724  function fn truncate_to_token_budget(  (Read offset=691 limit=34)
L726-733  function fn truncate_chars(value: &str, max_chars: usize) -> String  (Read offset=726 limit=8)
L735-741  function fn metadata_mtime_secs(meta: &fs::Metadata) -> u64  (Read offset=735 limit=7)
L744-752  struct struct CacheMeta  (Read offset=744 limit=9)
  # [derive(Default)]
L754-787  impl impl CacheMeta  (Read offset=754 limit=34)
    L755-773  function fn parse(text: &str) -> Self  (Read offset=755 limit=19)
    L775-786  function fn to_text(&self) -> String  (Read offset=775 limit=12)
L789-791  function fn home_dir() -> Option<PathBuf>  (Read offset=789 limit=3)
L793-803  function fn parse_string_array(value: &str) -> Vec<String>  (Read offset=793 limit=11)
L805-807  function fn unquote(value: &str) -> String  (Read offset=805 limit=3)
L809-811  function fn escape_toml(value: &str) -> String  (Read offset=809 limit=3)
L813-820  function fn format_string_array(values: &[String]) -> String  (Read offset=813 limit=8)
L822-833  function fn glob_match(pattern: &str, path: &str) -> bool  (Read offset=822 limit=12)
L836-924  mod mod tests  (Read offset=836 limit=89)
  # [cfg(test)]
    L840-846  function fn extracts_python_symbols()  (Read offset=840 limit=7)
      # [test]
    L849-868  function fn treesitter_extracts_rust_semantic_end_lines()  (Read offset=849 limit=20)
      # [test]
    L871-887  function fn treesitter_finds_typescript_classes_and_interfaces()  (Read offset=871 limit=17)
      # [test]
    L890-897  function fn treesitter_falls_back_on_garbage()  (Read offset=890 limit=8)
      # [test]
    L900-904  function fn detects_supported_languages()  (Read offset=900 limit=5)
      # [test]
    L907-923  function fn token_budget_truncates()  (Read offset=907 limit=17)
      # [test]
```

The agent reads this skeleton and decides "I want `cached_skeleton`" → re-issues `Read(file_path, offset=227, limit=87)` → gets 87 lines instead of 924.

## Single-file before/after — `crates/readzip-cli/src/main.rs`

1,372 lines / 11,833 tokens. Skeleton = 1,463 tokens. **87.6% reduction.**

The skeleton lists every CLI subcommand handler with its exact line range:

```text
$ readzip skeleton crates/readzip-cli/src/main.rs

# crates/readzip-cli/src/main.rs -- 1372 lines (Rust) skeleton view
...
L15-39   fn main()
L41-46   fn help() -> Result<(), String>
L48-85   fn init(args: &[String]) -> Result<(), String>
L88-92   struct AgentSelection
L100-125 impl AgentSelection
L127-161 fn parse_agents_flag(args: &[String]) -> AgentSelection
L163-226 fn hook() -> Result<(), String>
L228-272 fn demo(args: &[String]) -> Result<(), String>
L274-329 fn stats(args: &[String]) -> Result<(), String>
L341-353 fn uninstall(args: &[String]) -> Result<(), String>
L391-439 fn doctor(args: &[String]) -> Result<(), String>
L466-628 fn eval_cmd(args: &[String]) -> Result<(), String>
L679-688 fn skeleton_cmd(args: &[String]) -> Result<(), String>
L690-711 fn mcp_server() -> Result<(), String>
L940-1002  fn install_claude_hook(yes: bool) -> Result<(), String>
L1105-1107 fn install_cursor_mcp() -> Result<(), String>
L1109-1124 fn install_cline_mcp() -> Result<(), String>
L1126-1128 fn install_windsurf_mcp() -> Result<(), String>
L1130-1132 fn install_gemini_mcp() -> Result<(), String>
... (full output: 68 lines, all symbols accounted for)
```

Agent says "find the doctor handler" → `Read(file_path, offset=391, limit=49)` → ~600 tokens vs the original 11,833.

## Reproduce

```bash
cargo build --release -p readzip-cli
./target/release/readzip eval crates/
./target/release/readzip eval --json crates/    # for dashboards / CI
```

The numbers above are from running these exact commands on this repo at the commit where this file was last modified. Run on any of your own codebases for your own numbers.
