use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

mod parsers;

pub const DEFAULT_MIN_LINES: usize = 500;
pub const DEFAULT_MAX_SKELETON_TOKENS: usize = 1500;

#[derive(Debug, Clone)]
pub struct Config {
    pub min_lines: usize,
    pub max_skeleton_tokens: usize,
    pub cache_dir: PathBuf,
    pub skeleton_detail: SkeletonDetail,
    pub bypass_for: Vec<String>,
    pub force_full_for: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkeletonDetail {
    Minimal,
    Medium,
    Verbose,
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub source_path: PathBuf,
    pub language: Language,
    pub line_count: usize,
    pub text: String,
    pub original_tokens_estimate: usize,
    pub skeleton_tokens_estimate: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Go,
    Rust,
    Java,
    Ruby,
    C,
    Cpp,
    CSharp,
    Php,
    Swift,
    Kotlin,
    Scala,
    Lua,
    Bash,
    Unknown,
}

#[derive(Debug, Clone)]
struct Symbol {
    line: usize,
    end_line: usize,
    indent: usize,
    kind: &'static str,
    signature: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_lines: DEFAULT_MIN_LINES,
            max_skeleton_tokens: DEFAULT_MAX_SKELETON_TOKENS,
            cache_dir: default_cache_dir(),
            skeleton_detail: SkeletonDetail::Medium,
            bypass_for: Vec::new(),
            force_full_for: vec!["*.md".to_string(), "package.json".to_string()],
        }
    }
}

pub fn default_config_path() -> PathBuf {
    if let Some(home) = home_dir() {
        return home.join(".config/readzip/config.toml");
    }
    PathBuf::from(".readzip/config.toml")
}

pub fn default_cache_dir() -> PathBuf {
    if let Some(home) = home_dir() {
        return home.join(".cache/readzip");
    }
    PathBuf::from(".readzip/cache")
}

pub fn default_config_text(config: &Config) -> String {
    format!(
        "min_lines = {}\nmax_skeleton_tokens = {}\nskeleton_detail = \"{}\"\ncache_dir = \"{}\"\nbypass_for = {}\nforce_full_for = {}\n",
        config.min_lines,
        config.max_skeleton_tokens,
        match config.skeleton_detail {
            SkeletonDetail::Minimal => "minimal",
            SkeletonDetail::Medium => "medium",
            SkeletonDetail::Verbose => "verbose",
        },
        escape_toml(&config.cache_dir.to_string_lossy()),
        format_string_array(&config.bypass_for),
        format_string_array(&config.force_full_for)
    )
}

pub fn load_config() -> Config {
    let path = default_config_path();
    let Ok(text) = fs::read_to_string(path) else {
        return Config::default();
    };

    let mut config = Config::default();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "min_lines" => {
                if let Ok(parsed) = value.parse() {
                    config.min_lines = parsed;
                }
            }
            "max_skeleton_tokens" => {
                if let Ok(parsed) = value.parse() {
                    config.max_skeleton_tokens = parsed;
                }
            }
            "stats_enabled" => {
                // Deprecated as of 0.1.0; stats are always recorded locally now.
                // Silently ignored to keep older config.toml files parsing cleanly.
            }
            "cache_dir" => config.cache_dir = PathBuf::from(unquote(value)),
            "skeleton_detail" => {
                config.skeleton_detail = match unquote(value).as_str() {
                    "minimal" => SkeletonDetail::Minimal,
                    "verbose" => SkeletonDetail::Verbose,
                    _ => SkeletonDetail::Medium,
                };
            }
            "bypass_for" => config.bypass_for = parse_string_array(value),
            "force_full_for" => config.force_full_for = parse_string_array(value),
            _ => {}
        }
    }
    config
}

pub fn should_intercept(path: &Path, line_count: usize, config: &Config) -> bool {
    if line_count < config.min_lines {
        return false;
    }
    let path_text = path.to_string_lossy();
    if config
        .bypass_for
        .iter()
        .any(|glob| glob_match(glob, &path_text))
    {
        return false;
    }
    if config
        .force_full_for
        .iter()
        .any(|glob| glob_match(glob, &path_text))
    {
        return false;
    }
    detect_language(path) != Language::Unknown
}

pub fn build_skeleton(path: &Path, config: &Config) -> io::Result<Skeleton> {
    let source = fs::read_to_string(path)?;
    Ok(build_skeleton_from_source(path, &source, config))
}

pub fn build_skeleton_from_source(path: &Path, source: &str, config: &Config) -> Skeleton {
    let language = detect_language(path);
    let lines: Vec<&str> = source.lines().collect();
    let line_count = lines.len();
    let symbols = extract_symbols_dispatch(language, &lines, source);
    let mut text = render_skeleton(
        path,
        language,
        line_count,
        &lines,
        &symbols,
        config.skeleton_detail,
    );
    let original_tokens_estimate = estimate_tokens(source);
    let mut skeleton_tokens_estimate = estimate_tokens(&text);
    let mut truncated = false;
    if skeleton_tokens_estimate > config.max_skeleton_tokens {
        text = truncate_to_token_budget(
            &text,
            config.max_skeleton_tokens,
            line_count,
            &symbols,
        );
        skeleton_tokens_estimate = estimate_tokens(&text);
        truncated = true;
    }

    Skeleton {
        source_path: path.to_path_buf(),
        language,
        line_count,
        text,
        original_tokens_estimate,
        skeleton_tokens_estimate,
        truncated,
    }
}

pub fn cached_skeleton(path: &Path, config: &Config) -> io::Result<Skeleton> {
    let source_meta = fs::metadata(path)?;
    let source_mtime = metadata_mtime_secs(&source_meta);
    let source_size = source_meta.len();
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let key = stable_hash(&canonical.to_string_lossy());
    let views_dir = config.cache_dir.join("views");
    let skeleton_path = views_dir.join(format!("{key}.skeleton"));
    let meta_path = views_dir.join(format!("{key}.meta"));

    let cached_meta_and_text = match (
        fs::read_to_string(&meta_path),
        fs::read_to_string(&skeleton_path),
    ) {
        (Ok(meta_text), Ok(text)) => Some((CacheMeta::parse(&meta_text), text)),
        _ => None,
    };

    // Fast path: same path + mtime + size — trust the cache without reading source.
    if let Some((meta, text)) = &cached_meta_and_text {
        if meta.source_path == canonical.to_string_lossy()
            && meta.mtime == source_mtime
            && meta.size == source_size
            && !meta.source_hash.is_empty()
        {
            return Ok(Skeleton {
                source_path: canonical,
                language: detect_language(path),
                line_count: meta.line_count,
                original_tokens_estimate: meta.original_tokens,
                skeleton_tokens_estimate: estimate_tokens(text),
                truncated: meta.truncated,
                text: text.clone(),
            });
        }
    }

    // Mtime/size mismatch (or no cache) — read source and use content hash as the source of truth.
    let source = fs::read_to_string(path)?;
    let source_hash = stable_hash(&source);

    // Hash-bypass: file was touched but content is identical (git checkout, formatter no-op).
    // Refresh the meta with the new mtime/size but reuse the cached skeleton text.
    if let Some((meta, text)) = &cached_meta_and_text {
        if meta.source_path == canonical.to_string_lossy() && meta.source_hash == source_hash {
            let refreshed = CacheMeta {
                source_path: canonical.to_string_lossy().to_string(),
                mtime: source_mtime,
                size: source_size,
                source_hash: source_hash.clone(),
                line_count: meta.line_count,
                original_tokens: meta.original_tokens,
                truncated: meta.truncated,
            };
            let _ = fs::create_dir_all(&views_dir);
            let _ = fs::write(&meta_path, refreshed.to_text());
            return Ok(Skeleton {
                source_path: canonical,
                language: detect_language(path),
                line_count: meta.line_count,
                original_tokens_estimate: meta.original_tokens,
                skeleton_tokens_estimate: estimate_tokens(text),
                truncated: meta.truncated,
                text: text.clone(),
            });
        }
    }

    let skeleton = build_skeleton_from_source(path, &source, config);
    if fs::create_dir_all(&views_dir).is_ok() {
        let _ = fs::write(&skeleton_path, &skeleton.text);
        let _ = fs::write(
            &meta_path,
            CacheMeta {
                source_path: canonical.to_string_lossy().to_string(),
                mtime: source_mtime,
                size: source_size,
                source_hash,
                line_count: skeleton.line_count,
                original_tokens: skeleton.original_tokens_estimate,
                truncated: skeleton.truncated,
            }
            .to_text(),
        );
    }
    Ok(skeleton)
}

pub fn detect_language(path: &Path) -> Language {
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("");
    match ext {
        "py" => Language::Python,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "ts" | "tsx" | "mts" | "cts" => Language::TypeScript,
        "go" => Language::Go,
        "rs" => Language::Rust,
        "java" => Language::Java,
        "rb" => Language::Ruby,
        "c" | "h" => Language::C,
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Language::Cpp,
        "cs" => Language::CSharp,
        "php" => Language::Php,
        "swift" => Language::Swift,
        "kt" | "kts" => Language::Kotlin,
        "scala" | "sc" => Language::Scala,
        "lua" => Language::Lua,
        "sh" | "bash" | "zsh" => Language::Bash,
        _ if name == "Bashfile" || name == ".bashrc" || name == ".zshrc" => Language::Bash,
        _ => Language::Unknown,
    }
}

pub fn language_name(language: Language) -> &'static str {
    match language {
        Language::Python => "Python",
        Language::JavaScript => "JavaScript",
        Language::TypeScript => "TypeScript",
        Language::Go => "Go",
        Language::Rust => "Rust",
        Language::Java => "Java",
        Language::Ruby => "Ruby",
        Language::C => "C",
        Language::Cpp => "C++",
        Language::CSharp => "C#",
        Language::Php => "PHP",
        Language::Swift => "Swift",
        Language::Kotlin => "Kotlin",
        Language::Scala => "Scala",
        Language::Lua => "Lua",
        Language::Bash => "Bash",
        Language::Unknown => "Unknown",
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    (chars / 4).max(words)
}

pub fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Tree-sitter-first; falls back to the heuristic when:
///   - the language has no tree-sitter spec wired up, OR
///   - the parser produced too many ERROR / MISSING nodes, OR
///   - the parse returned no symbols (rare; usually means the file is data, not code).
fn extract_symbols_dispatch(language: Language, lines: &[&str], source: &str) -> Vec<Symbol> {
    if let Some(parsed) = parsers::extract(language, source) {
        if !parsed.is_empty() {
            let mut symbols: Vec<Symbol> = parsed
                .into_iter()
                .map(|s| Symbol {
                    line: s.line,
                    end_line: s.end_line,
                    indent: s.indent,
                    kind: s.kind,
                    signature: s.signature,
                })
                .collect();
            // tree-sitter walk visits nested nodes; ensure ascending line order.
            symbols.sort_by_key(|s| s.line);
            return symbols;
        }
    }
    extract_symbols(language, lines)
}

fn extract_symbols(language: Language, lines: &[&str]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        if let Some((kind, signature)) = symbol_signature(language, trimmed) {
            let indent = line.len().saturating_sub(trimmed.len());
            symbols.push(Symbol {
                line: line_no,
                end_line: line_no,
                indent,
                kind,
                signature,
            });
        }
    }

    for i in 0..symbols.len() {
        let next_same_or_less = symbols
            .iter()
            .skip(i + 1)
            .find(|candidate| candidate.indent <= symbols[i].indent)
            .map(|candidate| candidate.line.saturating_sub(1))
            .unwrap_or(lines.len());
        symbols[i].end_line = next_same_or_less.max(symbols[i].line);
    }
    symbols
}

fn symbol_signature(language: Language, trimmed: &str) -> Option<(&'static str, String)> {
    match language {
        Language::Python => {
            if trimmed.starts_with("class ") {
                Some(("class", clean_signature(trimmed)))
            } else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Ruby => {
            if trimmed.starts_with("class ") || trimmed.starts_with("module ") {
                Some(("class", clean_signature(trimmed)))
            } else if trimmed.starts_with("def ") {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Rust => {
            if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
                Some(("struct", clean_signature(trimmed)))
            } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
                Some(("enum", clean_signature(trimmed)))
            } else if trimmed.starts_with("impl ") {
                Some(("impl", clean_signature(trimmed)))
            } else if trimmed.contains("fn ")
                && (trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub ")
                    || trimmed.starts_with("async "))
            {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Go => {
            if trimmed.starts_with("func ") {
                Some(("function", clean_signature(trimmed)))
            } else if trimmed.starts_with("type ")
                && (trimmed.contains(" struct") || trimmed.contains(" interface"))
            {
                Some(("type", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::JavaScript | Language::TypeScript => {
            if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
                Some(("class", clean_signature(trimmed)))
            } else if trimmed.starts_with("function ")
                || trimmed.starts_with("export function ")
                || trimmed.starts_with("async function ")
                || trimmed.starts_with("export async function ")
                || looks_like_js_function(trimmed)
            {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Java
        | Language::CSharp
        | Language::Kotlin
        | Language::Scala
        | Language::Swift
        | Language::Php => {
            if trimmed.contains(" class ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("public class ")
            {
                Some(("class", clean_signature(trimmed)))
            } else if trimmed.contains('(')
                && trimmed.contains(')')
                && (trimmed.ends_with('{') || trimmed.contains("=>"))
            {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::C | Language::Cpp => {
            if trimmed.starts_with("class ") || trimmed.starts_with("struct ") {
                Some(("type", clean_signature(trimmed)))
            } else if trimmed.contains('(') && trimmed.contains(')') && trimmed.ends_with('{') {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Lua => {
            if trimmed.starts_with("function ")
                || (trimmed.contains("function(") && trimmed.contains('='))
            {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Bash => {
            if trimmed.starts_with("function ")
                || trimmed.ends_with("() {")
                || trimmed.ends_with("(){")
            {
                Some(("function", clean_signature(trimmed)))
            } else {
                None
            }
        }
        Language::Unknown => None,
    }
}

fn looks_like_js_function(trimmed: &str) -> bool {
    (trimmed.contains("=>")
        && (trimmed.starts_with("const ")
            || trimmed.starts_with("let ")
            || trimmed.starts_with("export const ")))
        || (trimmed.contains('(')
            && trimmed.contains(')')
            && trimmed.ends_with('{')
            && !trimmed.starts_with("if ")
            && !trimmed.starts_with("for ")
            && !trimmed.starts_with("while "))
}

fn clean_signature(line: &str) -> String {
    let without_body = line.split('{').next().unwrap_or(line).trim();
    let without_colon = without_body.trim_end_matches(':').trim();
    truncate_chars(without_colon, 140)
}

fn render_skeleton(
    path: &Path,
    language: Language,
    line_count: usize,
    lines: &[&str],
    symbols: &[Symbol],
    detail: SkeletonDetail,
) -> String {
    let display_path = path.to_string_lossy();
    let mut out = String::new();
    out.push_str(&format!(
        "# {} -- {} lines ({}) skeleton view\n",
        display_path,
        line_count,
        language_name(language)
    ));
    out.push_str(&format!(
        "# Use Read(file_path=\"{}\", offset=N, limit=M) for a specific section.\n\n",
        display_path
    ));

    let import_end = import_block_end(language, lines);
    if import_end > 0 {
        out.push_str(&format!(
            "L1-{}    imports / module header (Read offset=1 limit={})\n",
            import_end, import_end
        ));
    }

    if symbols.is_empty() {
        out.push_str("No top-level symbols detected. Use scoped Read calls by line range.\n");
        return out;
    }

    for symbol in symbols {
        let indent = "  ".repeat(symbol.indent / 2);
        let limit = symbol
            .end_line
            .saturating_sub(symbol.line)
            .saturating_add(1)
            .min(120);
        out.push_str(&format!(
            "{}L{}-{}  {} {}  (Read offset={} limit={})\n",
            indent, symbol.line, symbol.end_line, symbol.kind, symbol.signature, symbol.line, limit
        ));
        if detail != SkeletonDetail::Minimal {
            if let Some(doc) = nearby_doc(lines, symbol.line) {
                out.push_str(&format!("{}  # {}\n", indent, doc));
            }
        }
        if detail == SkeletonDetail::Verbose {
            if let Some(first_body) = first_body_line(lines, symbol.line, symbol.end_line) {
                out.push_str(&format!("{}  first body line: {}\n", indent, first_body));
            }
        }
    }
    out
}

fn import_block_end(language: Language, lines: &[&str]) -> usize {
    let mut end = 0;
    for (idx, line) in lines.iter().take(80).enumerate() {
        let trimmed = line.trim();
        let is_import = match language {
            Language::Python => trimmed.starts_with("import ") || trimmed.starts_with("from "),
            Language::JavaScript | Language::TypeScript => {
                trimmed.starts_with("import ") || trimmed.starts_with("export ")
            }
            Language::Go => trimmed.starts_with("import"),
            Language::Rust => trimmed.starts_with("use "),
            Language::Java | Language::Kotlin | Language::Scala => {
                trimmed.starts_with("import ") || trimmed.starts_with("package ")
            }
            Language::C | Language::Cpp => trimmed.starts_with("#include"),
            Language::CSharp => trimmed.starts_with("using "),
            Language::Php => trimmed.starts_with("use ") || trimmed.starts_with("namespace "),
            Language::Swift => trimmed.starts_with("import "),
            Language::Lua => trimmed.starts_with("require") || trimmed.contains("require("),
            Language::Bash => trimmed.starts_with("source ") || trimmed.starts_with(". "),
            Language::Ruby => trimmed.starts_with("require ") || trimmed.starts_with("load "),
            Language::Unknown => false,
        };
        if is_import || trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#')
        {
            end = idx + 1;
        } else if end > 0 {
            break;
        }
    }
    end
}

fn nearby_doc(lines: &[&str], line: usize) -> Option<String> {
    let start = line.saturating_sub(4);
    for idx in (start..line.saturating_sub(1)).rev() {
        let trimmed = lines.get(idx)?.trim();
        if trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with('#')
            || trimmed.starts_with('*')
        {
            return Some(truncate_chars(
                trimmed
                    .trim_matches('/')
                    .trim_matches('*')
                    .trim_matches('#')
                    .trim(),
                100,
            ));
        }
        if !trimmed.is_empty() {
            break;
        }
    }
    None
}

fn first_body_line(lines: &[&str], line: usize, end_line: usize) -> Option<String> {
    for idx in line..end_line.min(lines.len()) {
        let trimmed = lines.get(idx)?.trim();
        if !trimmed.is_empty() && trimmed != "{" && trimmed != "}" {
            return Some(truncate_chars(trimmed, 100));
        }
    }
    None
}

fn truncate_to_token_budget(
    text: &str,
    max_tokens: usize,
    line_count: usize,
    symbols: &[Symbol],
) -> String {
    let approx_chars = max_tokens.saturating_mul(4);
    if text.len() <= approx_chars {
        return text.to_string();
    }
    let mut out = truncate_chars(text, approx_chars.saturating_sub(320));
    let last_kept_line = symbols
        .iter()
        .filter(|s| {
            // Only keep symbols whose line marker survived the char truncation.
            out.contains(&format!("L{}-", s.line))
        })
        .map(|s| s.end_line)
        .max()
        .unwrap_or(0);
    let elided_start = last_kept_line.saturating_add(1);
    let elided_lines = line_count.saturating_sub(last_kept_line);
    out.push('\n');
    if elided_start <= line_count && elided_lines > 0 {
        out.push_str(&format!(
            "L{}-{}  [{} lines elided to fit max_skeleton_tokens — Read offset={} for this range]\n",
            elided_start, line_count, elided_lines, elided_start
        ));
    } else {
        out.push_str("# ... skeleton truncated by max_skeleton_tokens.\n");
    }
    out.push_str("# Use scoped Read(file_path, offset=N, limit=M) for elided sections.\n");
    out
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn metadata_mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[derive(Default)]
struct CacheMeta {
    source_path: String,
    mtime: u64,
    size: u64,
    source_hash: String,
    line_count: usize,
    original_tokens: usize,
    truncated: bool,
}

impl CacheMeta {
    fn parse(text: &str) -> Self {
        let mut meta = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "source_path" => meta.source_path = value.to_string(),
                "mtime" => meta.mtime = value.parse().unwrap_or(0),
                "size" => meta.size = value.parse().unwrap_or(0),
                "source_hash" => meta.source_hash = value.to_string(),
                "line_count" => meta.line_count = value.parse().unwrap_or(0),
                "original_tokens" => meta.original_tokens = value.parse().unwrap_or(0),
                "truncated" => meta.truncated = value == "true",
                _ => {}
            }
        }
        meta
    }

    fn to_text(&self) -> String {
        format!(
            "source_path={}\nmtime={}\nsize={}\nsource_hash={}\nline_count={}\noriginal_tokens={}\ntruncated={}\n",
            self.source_path,
            self.mtime,
            self.size,
            self.source_hash,
            self.line_count,
            self.original_tokens,
            self.truncated
        )
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn parse_string_array(value: &str) -> Vec<String> {
    let trimmed = value.trim().trim_start_matches('[').trim_end_matches(']');
    if trimmed.trim().is_empty() {
        return Vec::new();
    }
    trimmed
        .split(',')
        .map(|item| unquote(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

fn escape_toml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn format_string_array(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|value| format!("\"{}\"", escape_toml(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

pub fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return path.ends_with(suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path.starts_with(prefix);
    }
    path == pattern || path.ends_with(&format!("/{pattern}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_python_symbols() {
        let source = "import os\n\nclass TokenManager:\n    def refresh_token(self, token):\n        return token\n";
        let config = Config::default();
        let skeleton = build_skeleton_from_source(Path::new("auth.py"), source, &config);
        assert!(skeleton.text.contains("class TokenManager"));
        assert!(skeleton.text.contains("def refresh_token"));
    }

    #[test]
    fn treesitter_extracts_rust_semantic_end_lines() {
        // The heuristic was indent-based; tree-sitter gives us the actual closing brace.
        let source = r#"pub fn foo() -> u32 {
    let x = 1;
    let y = 2;
    x + y
}

pub struct Bar {
    a: u32,
}
"#;
        let config = Config::default();
        let skeleton = build_skeleton_from_source(Path::new("a.rs"), source, &config);
        // foo spans L1-5 (closing brace on line 5), Bar spans L7-9.
        assert!(skeleton.text.contains("L1-5"), "got: {}", skeleton.text);
        assert!(skeleton.text.contains("L7-9"), "got: {}", skeleton.text);
        assert!(skeleton.text.contains("fn foo"));
        assert!(skeleton.text.contains("struct Bar"));
    }

    #[test]
    fn treesitter_finds_typescript_classes_and_interfaces() {
        let source = r#"export interface User {
    id: string;
}

export class UserService {
    async findById(id: string): Promise<User | null> {
        return null;
    }
}
"#;
        let config = Config::default();
        let skeleton = build_skeleton_from_source(Path::new("svc.ts"), source, &config);
        assert!(skeleton.text.contains("interface User"));
        assert!(skeleton.text.contains("class UserService"));
        assert!(skeleton.text.contains("findById"));
    }

    #[test]
    fn treesitter_falls_back_on_garbage() {
        // 100% non-parseable. Heuristic should still find nothing useful but
        // the dispatch shouldn't crash.
        let source = "@@@@ this is not valid python @@@@\n!!! &&& ???\n";
        let config = Config::default();
        let _ = build_skeleton_from_source(Path::new("bad.py"), source, &config);
        // No assertion on contents — just that we didn't panic.
    }

    #[test]
    fn no_top_level_symbols_marker_is_stable() {
        // Hook layer relies on this exact marker to detect "skeleton has nothing
        // useful" and pass through. If the marker text changes, the hook check
        // at crates/readzip-cli/src/main.rs::hook must be updated in lockstep.
        let source = (0..600)
            .map(|i| format!("FOO_{i} = 'bar_{i}'\n"))
            .collect::<String>();
        let config = Config::default();
        let skeleton = build_skeleton_from_source(Path::new("config.py"), &source, &config);
        assert!(
            skeleton.text.contains("No top-level symbols detected"),
            "expected the no-symbols marker; got: {}",
            skeleton.text
        );
    }

    #[test]
    fn detects_supported_languages() {
        assert_eq!(detect_language(Path::new("main.rs")), Language::Rust);
        assert_eq!(detect_language(Path::new("app.tsx")), Language::TypeScript);
        assert_eq!(detect_language(Path::new("script.sh")), Language::Bash);
    }

    #[test]
    fn token_budget_truncates() {
        let config = Config {
            max_skeleton_tokens: 20,
            ..Config::default()
        };
        let source = (0..200)
            .map(|i| format!("def function_{i}():\n    pass\n"))
            .collect::<String>();
        let skeleton = build_skeleton_from_source(Path::new("many.py"), &source, &config);
        assert!(skeleton.truncated);
        assert!(
            skeleton.text.contains("lines elided") || skeleton.text.contains("skeleton truncated"),
            "expected elision marker in: {}",
            skeleton.text
        );
        assert!(skeleton.text.contains("Use scoped Read"));
    }
}
