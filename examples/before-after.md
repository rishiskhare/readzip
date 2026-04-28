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
- total original tokens: 17.5K
- total skeleton tokens: 2.5K
- tokens saved: 15.0K
- average reduction: 85.8%

| File | Lang | Lines | Original | Skeleton | Saved | Reduction |
|---|---|---:|---:|---:|---:|---:|
| crates/readzip-cli/src/main.rs | Rust | 1086 | 9561 | 1199 | 8362 | 87.5% |
| crates/readzip-core/src/lib.rs | Rust |  941 | 7917 | 1276 | 6641 | 83.9% |
```

## Single-file before/after — `crates/readzip-core/src/lib.rs`

A 941-line Rust file. Full read = 7,917 tokens. Skeleton = 1,276 tokens. **83.9% reduction.**

```text
$ readzip skeleton crates/readzip-core/src/lib.rs

# crates/readzip-core/src/lib.rs -- 941 lines (Rust) skeleton view
# Use Read(file_path="crates/readzip-core/src/lib.rs", offset=N, limit=M) for a specific section.

L1-7    imports / module header (Read offset=1 limit=7)
L8-8    mod parsers;
L14-21  pub struct Config
L24-28  pub enum SkeletonDetail
L31-39  pub struct Skeleton
L42-60  pub enum Language
L71-82  impl Default for Config
    L72-81  fn default() -> Self
L84-89  pub fn default_config_path() -> PathBuf
L98-112 pub fn default_config_text(config: &Config) -> String
L114-160 pub fn load_config() -> Config
L162-182 pub fn should_intercept(path: &Path, line_count: usize, config: &Config) -> bool
L184-187 pub fn build_skeleton(path: &Path, config: &Config) -> io::Result<Skeleton>
L189-225 pub fn build_skeleton_from_source(path: &Path, source: &str, config: &Config) -> Skeleton
L227-313 pub fn cached_skeleton(path: &Path, config: &Config) -> io::Result<Skeleton>
L315-338 pub fn detect_language(path: &Path) -> Language
L378-397 fn extract_symbols_dispatch(...)
L431-543 fn symbol_signature(...)
L564-621 fn render_skeleton(...)
L691-724 fn truncate_to_token_budget(...)
L744-752 struct CacheMeta
L754-787 impl CacheMeta
L822-833 pub fn glob_match(pattern: &str, path: &str) -> bool
L836-941 mod tests
... (full output: 70 entries, every symbol with its exact line range)
```

The agent reads this skeleton and decides "I want `cached_skeleton`" → re-reads `Read(file_path, offset=227, limit=87)` → gets 87 lines instead of 941.

## Single-file before/after — `crates/readzip-cli/src/main.rs`

1,086 lines / 9,561 tokens. Skeleton = 1,199 tokens. **87.5% reduction.**

The skeleton lists every CLI subcommand handler with its exact line range:

```text
$ readzip skeleton crates/readzip-cli/src/main.rs

# crates/readzip-cli/src/main.rs -- 1086 lines (Rust) skeleton view
...
L15-40   fn main()
L42-47   fn help() -> Result<(), String>
L49-106  fn init(args: &[String]) -> Result<(), String>
L108-115 fn agent_present(agent: &str) -> bool
L118-122 struct AgentSelection
L126-150 impl AgentSelection
L152-186 fn parse_agents_flag(args: &[String]) -> AgentSelection
L188-301 fn hook() -> Result<(), String>
L304-348 fn demo(args: &[String]) -> Result<(), String>
L350-380 fn stats(args: &[String]) -> Result<(), String>
L392-414 fn uninstall(args: &[String]) -> Result<(), String>
L416-452 fn doctor(args: &[String]) -> Result<(), String>
L454-616 fn eval_cmd(args: &[String]) -> Result<(), String>
L667-676 fn skeleton_cmd(args: &[String]) -> Result<(), String>
L682-705 fn read_cmd(args: &[String]) -> Result<(), String>
L708-730 fn section_cmd(args: &[String]) -> Result<(), String>
... (full output: 59 entries)
```

Agent says "find the doctor handler" → `Read(file_path, offset=416, limit=37)` → ~530 tokens vs the original 9,561.

## Reproduce

```bash
cargo build --release -p readzip-cli
./target/release/readzip eval crates/
./target/release/readzip eval --json crates/    # for dashboards / CI
```

The numbers above are from running these exact commands on this repo at the commit where this file was last modified. Run on any of your own codebases for your own numbers.
