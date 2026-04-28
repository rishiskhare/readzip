use readzip_core::{
    build_skeleton, cached_skeleton, default_config_path, default_config_text, load_config,
    should_intercept, Config,
};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        None | Some("--help") | Some("-h") => help(),
        Some("--version") | Some("-V") => {
            println!("readzip {VERSION}");
            Ok(())
        }
        Some("init") => init(&args[2..]),
        Some("hook") => hook(),
        Some("demo") => demo(&args[2..]),
        Some("stats") => stats(&args[2..]),
        Some("uninstall") => uninstall(&args[2..]),
        Some("doctor") => doctor(&args[2..]),
        Some("skeleton") => skeleton_cmd(&args[2..]),
        Some("read") => read_cmd(&args[2..]),
        Some("section") => section_cmd(&args[2..]),
        Some("eval") => eval_cmd(&args[2..]),
        Some(other) => Err(format!("unknown command: {other}")),
    };

    if let Err(error) = result {
        eprintln!("readzip: {error}");
        process::exit(1);
    }
}

fn help() -> Result<(), String> {
    println!(
        "readzip {VERSION}\n\nFile-read CLI for AI coding agents. ~80% fewer tokens on large files.\n\nUSAGE — primary commands:\n  readzip read <file>                     smart cat: skeleton if large, full if small\n  readzip section <file> <offset> <limit> print a scoped line range\n  readzip skeleton <file>                 always print the structural skeleton\n  readzip stats [--json]                  tokens saved so far (local-only)\n\nUSAGE — installation & diagnostics:\n  readzip doctor [--json]                 verify the Claude Code hook is wired up\n  readzip demo [--json]                   compression on a bundled fixture\n  readzip init [--yes]                    wire up Claude Code (auto-run by install.sh)\n  readzip uninstall [--keep-cache] [--purge]\n                                          remove hook (--purge also wipes config)\n\nUSAGE — advanced (rarely run by hand):\n  readzip hook                            PreToolUse handler (Claude Code calls this)\n  readzip eval <dir> [--json]             corpus eval (development)\n\nFor Claude Code, init installs a transparent `PreToolUse` hook on `Read`.\nFor every other agent (Codex, Cursor, Cline, Windsurf, Gemini, Aider, …),\njust call the primary commands from Bash — no setup required.\n"
    );
    Ok(())
}

fn init(args: &[String]) -> Result<(), String> {
    let yes = args.iter().any(|arg| arg == "--yes" || arg == "-y");
    let agents = parse_agents_flag(args);
    let force = agents.user_specified();
    print_banner(false);
    ensure_config(false)?;

    let mut installed = Vec::<&'static str>::new();
    let mut skipped = Vec::<&'static str>::new();

    if agents.wants("claude") {
        if agent_present("claude") || force {
            install_claude_hook(yes)?;
            installed.push("Claude Code");
        } else {
            skipped.push("Claude Code (not installed; --only=claude to force)");
        }
    }

    if agents.wants("codex") {
        if agent_present("codex") || force {
            install_codex_hint()?;
            installed.push("Codex (AGENTS.md hint)");
        } else {
            skipped.push("Codex (not installed)");
        }
    }

    for unsupported in agents.requested_unsupported() {
        eprintln!("readzip: agent '{unsupported}' install path not yet implemented; skipping.");
    }

    println!();
    for agent in &installed {
        println!("  ✓  {agent}");
    }
    for agent in &skipped {
        println!("  ·  {agent}");
    }

    if installed.is_empty() {
        println!();
        println!("No agents were wired up. Install Claude Code (or `--only=claude` to force).");
        println!();
        println!("Other agents (Codex, Cursor, Cline, Windsurf, Gemini, Aider, …) don't need");
        println!("any setup — just call readzip from Bash:");
        println!("  readzip read <file>                        # smart cat");
        println!("  readzip section <file> <offset> <limit>    # scoped slice");
        return Ok(());
    }

    println!();
    println!("readzip is now active. Try it:");
    println!("  1. Restart your AI tool.");
    println!("  2. Ask it to read a file >500 lines.");
    println!("  3. After a few minutes:  readzip stats");
    Ok(())
}

fn agent_present(agent: &str) -> bool {
    let rel = match agent {
        "claude" => ".claude",
        "codex" => ".codex",
        _ => return false,
    };
    home_path(rel).map(|p| p.exists()).unwrap_or(false)
}

#[derive(Debug, Default)]
struct AgentSelection {
    only: Option<Vec<String>>,
    skip: Vec<String>,
    requested: Vec<String>,
}

const KNOWN_AGENTS: &[&str] = &["claude", "codex"];

impl AgentSelection {
    fn wants(&self, agent: &str) -> bool {
        if self.skip.iter().any(|s| s == agent) {
            return false;
        }
        if let Some(only) = &self.only {
            return only.iter().any(|a| a == agent);
        }
        KNOWN_AGENTS.contains(&agent)
    }

    /// True iff the user passed --only/--skip — i.e. force-install regardless of detection.
    fn user_specified(&self) -> bool {
        self.only.is_some() || !self.skip.is_empty()
    }

    /// Names from --only/--skip that we don't recognize, so we can warn instead of silently ignoring.
    fn requested_unsupported(&self) -> Vec<String> {
        self.requested
            .iter()
            .filter(|a| !KNOWN_AGENTS.contains(&a.as_str()))
            .cloned()
            .collect()
    }
}

fn parse_agents_flag(args: &[String]) -> AgentSelection {
    let mut selection = AgentSelection::default();
    let pull = |args: &[String], key: &str| -> Option<String> {
        for (i, arg) in args.iter().enumerate() {
            if arg == key {
                return args.get(i + 1).cloned();
            }
            if let Some(rest) = arg.strip_prefix(&format!("{key}=")) {
                return Some(rest.to_string());
            }
        }
        None
    };
    let split = |value: &str| -> Vec<String> {
        value
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    };
    if let Some(value) = pull(args, "--agents") {
        let list = split(&value);
        selection.requested = list.clone();
        selection.only = Some(list);
    }
    if let Some(value) = pull(args, "--only") {
        let list = split(&value);
        selection.requested = list.clone();
        selection.only = Some(list);
    }
    if let Some(value) = pull(args, "--skip") {
        selection.skip = split(&value);
    }
    selection
}

fn hook() -> Result<(), String> {
    // Hot path: Claude Code spawns this on EVERY Read tool call. Every step before
    // we know we're going to intercept must be branch-light and avoid I/O. The
    // ordering below — payload → metadata → source — minimizes work for the calls
    // that pass through (the majority).
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| format!("failed to read hook stdin: {err}"))?;

    let payload: Value =
        serde_json::from_str(&input).map_err(|err| format!("invalid hook JSON: {err}"))?;
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if tool_name != "Read" {
        return Ok(());
    }
    let Some(tool_input) = payload.get("tool_input").and_then(Value::as_object) else {
        return Ok(());
    };
    let Some(file_path) = tool_input.get("file_path").and_then(Value::as_str) else {
        return Ok(());
    };
    if tool_input.contains_key("offset") || tool_input.contains_key("limit") {
        return Ok(());
    }

    let path = PathBuf::from(file_path);

    // Cheap path-only filter — no source read, no config load.
    // Skips any extension we don't have a parser for (binary, .log, .txt, etc.).
    if readzip_core::detect_language(&path) == readzip_core::Language::Unknown {
        return Ok(());
    }

    let config = load_config();
    let path_text = path.to_string_lossy();

    // Glob filters — also path-only.
    if config
        .bypass_for
        .iter()
        .any(|glob| readzip_core::glob_match(glob, &path_text))
        || config
            .force_full_for
            .iter()
            .any(|glob| readzip_core::glob_match(glob, &path_text))
    {
        return Ok(());
    }

    // Metadata-based fast skip: if the file is too small to plausibly be
    // `min_lines` lines (assume worst case ~8 bytes/line of dense code), pass
    // through without ever opening it. Saves a full `read_to_string` for the
    // common case of small source files.
    let min_bytes = (config.min_lines as u64).saturating_mul(8);
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };
    if meta.len() < min_bytes {
        return Ok(());
    }

    // Now we read the source. Confirm line count — the byte heuristic is a
    // lower bound; this is the authoritative check.
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(_) => return Ok(()),
    };
    let line_count = source.lines().count();
    if !should_intercept(&path, line_count, &config) {
        return Ok(());
    }

    let skeleton = cached_skeleton(&path, &config)
        .map_err(|err| format!("failed to build skeleton: {err}"))?;

    // If the file passes `should_intercept` but yields zero top-level symbols
    // (e.g., a 600-line all-constants config in a supported language), the deny
    // payload would carry a useless skeleton and the agent would have to re-read
    // with blind offsets. Pass through instead.
    if skeleton.text.contains("No top-level symbols detected") {
        return Ok(());
    }

    record_intercept(
        &config,
        &skeleton.source_path,
        skeleton.original_tokens_estimate,
        skeleton.skeleton_tokens_estimate,
    );
    let reason = format!(
        "readzip blocked a full-file Read because this file is large ({} lines, ~{} tokens).\n\nRead the skeleton below, then re-issue Read(file_path=\"{}\", offset=N, limit=M) for the specific section you need. Scoped reads are allowed.\n\n{}",
        skeleton.line_count,
        skeleton.original_tokens_estimate,
        path.to_string_lossy(),
        skeleton.text
    );
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
                "additionalContext": "READZIP is active in this session. For large source files, prefer reading the skeleton first, then use native Read with offset and limit for the exact section needed. Scoped reads are allowed."
            }
        })
    );
    Ok(())
}


fn demo(args: &[String]) -> Result<(), String> {
    let json = args.iter().any(|arg| arg == "--json");
    if !json {
        print_banner(false);
    }
    let tmp = env::temp_dir().join("readzip-demo-auth_service.py");
    fs::write(&tmp, demo_source()).map_err(|err| format!("failed to write demo file: {err}"))?;
    let config = Config {
        min_lines: 50,
        ..load_config()
    };
    let skeleton = build_skeleton(&tmp, &config)
        .map_err(|err| format!("failed to build demo skeleton: {err}"))?;
    let saved = skeleton
        .original_tokens_estimate
        .saturating_sub(skeleton.skeleton_tokens_estimate);
    let reduction = percent(saved, skeleton.original_tokens_estimate);

    if json {
        println!(
            "{}",
            json!({
                "file": tmp.to_string_lossy(),
                "lines": skeleton.line_count,
                "original_tokens": skeleton.original_tokens_estimate,
                "skeleton_tokens": skeleton.skeleton_tokens_estimate,
                "tokens_saved": saved,
                "reduction_percent": rounded_percent(reduction)
            })
        );
    } else {
        println!("Demo file: {}", tmp.display());
        println!(
            "Before: full Read ~{} tokens",
            skeleton.original_tokens_estimate
        );
        println!(
            "After:  skeleton ~{} tokens",
            skeleton.skeleton_tokens_estimate
        );
        println!("Saved:  {saved} tokens ({reduction:.1}%)");
        println!("\n{}", skeleton.text);
    }
    Ok(())
}

fn stats(args: &[String]) -> Result<(), String> {
    let json = args.iter().any(|arg| arg == "--json");

    let config = load_config();
    let stats = read_stats(&config);
    let saved = stats.tokens_saved();
    if json {
        println!(
            "{}",
            json!({
                "files_intercepted": stats.files_intercepted,
                "original_tokens": stats.original_tokens,
                "skeleton_tokens": stats.skeleton_tokens,
                "tokens_saved": saved,
                "avg_reduction_percent": rounded_percent(stats.avg_reduction_percent()),
            })
        );
        return Ok(());
    }

    print_banner(false);
    println!("readzip stats");
    println!("  files intercepted:    {}", stats.files_intercepted);
    println!("  tokens saved:         {}", format_tokens(saved));
    println!(
        "  avg reduction:        {:.1}%",
        stats.avg_reduction_percent()
    );
    println!("  cache dir:            {}", config.cache_dir.display());
    Ok(())
}

fn format_tokens(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn uninstall(args: &[String]) -> Result<(), String> {
    let keep_cache = args.iter().any(|arg| arg == "--keep-cache");
    let purge = args.iter().any(|arg| arg == "--purge");
    uninstall_claude_hook()?;
    // Drop the Codex AGENTS hint init may have written; otherwise orphaned.
    if let Some(hint) = home_path(".codex/readzip-AGENTS-snippet.md") {
        let _ = fs::remove_file(&hint);
    }
    if !keep_cache {
        let config = load_config();
        let _ = fs::remove_dir_all(&config.cache_dir);
    }
    if purge {
        if let Some(config_dir) = home_path(".config/readzip") {
            let _ = fs::remove_dir_all(&config_dir);
        }
    }
    println!("readzip uninstalled.");
    if !purge {
        println!("(config kept at ~/.config/readzip — pass --purge to delete it too.)");
    }
    Ok(())
}

fn doctor(args: &[String]) -> Result<(), String> {
    let json = args.iter().any(|arg| arg == "--json");
    let config = load_config();
    let config_path = default_config_path();
    let claude_settings = home_path(".claude/settings.json");
    let claude_installed = claude_settings
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|text| text.contains("readzip hook"))
        .unwrap_or(false);
    let codex_hint_installed = home_path(".codex/readzip-AGENTS-snippet.md")
        .map(|p| p.exists())
        .unwrap_or(false);
    if json {
        println!(
            "{}",
            json!({
                "version": VERSION,
                "config_path": config_path,
                "cache_dir": config.cache_dir,
                "claude_hook_installed": claude_installed,
                "codex_hint_installed": codex_hint_installed,
            })
        );
        return Ok(());
    }
    println!("readzip doctor");
    println!("  version:               {VERSION}");
    println!("  config:                {}", config_path.display());
    println!("  cache:                 {}", config.cache_dir.display());
    println!("  Claude hook installed: {claude_installed}");
    println!("  Codex hint installed:  {codex_hint_installed}");
    println!();
    println!("  Other agents (Cursor, Cline, Windsurf, Gemini, Aider, …) need no setup —");
    println!("  they call `readzip read <file>` from Bash directly.");
    Ok(())
}

fn eval_cmd(args: &[String]) -> Result<(), String> {
    let json = args.iter().any(|arg| arg == "--json");
    let mut targets: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .collect();
    if targets.is_empty() {
        targets.push(".".to_string());
    }
    let config = load_config();
    let mut files = Vec::new();
    let mut missing_targets: Vec<String> = Vec::new();
    for target in &targets {
        let path = Path::new(target);
        if !path.exists() {
            missing_targets.push(target.clone());
            continue;
        }
        if path.is_file() {
            files.push(path.to_path_buf());
        } else {
            collect_source_files(path, &mut files);
        }
    }

    if !missing_targets.is_empty() {
        eprintln!(
            "readzip: warning: target(s) not found: {}",
            missing_targets.join(", ")
        );
    }

    #[derive(Default)]
    struct Row {
        path: String,
        language: String,
        lines: usize,
        original_tokens: usize,
        skeleton_tokens: usize,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut walked = 0usize;
    let mut recognized = 0usize;
    let mut intercepted = 0usize;
    let mut total_original = 0usize;
    let mut total_skeleton = 0usize;
    for path in &files {
        walked += 1;
        let language = readzip_core::detect_language(path);
        if language == readzip_core::Language::Unknown {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        recognized += 1;
        let line_count = source.lines().count();
        if line_count < config.min_lines {
            continue;
        }
        intercepted += 1;
        let skeleton = readzip_core::build_skeleton_from_source(path, &source, &config);
        total_original += skeleton.original_tokens_estimate;
        total_skeleton += skeleton.skeleton_tokens_estimate;
        rows.push(Row {
            path: path.display().to_string(),
            language: language_name_str(language).to_string(),
            lines: skeleton.line_count,
            original_tokens: skeleton.original_tokens_estimate,
            skeleton_tokens: skeleton.skeleton_tokens_estimate,
        });
    }
    rows.sort_by(|a, b| b.original_tokens.cmp(&a.original_tokens));

    let total_saved = total_original.saturating_sub(total_skeleton);
    let avg_reduction = if total_original > 0 {
        (total_saved as f64 * 100.0) / total_original as f64
    } else {
        0.0
    };

    if json {
        println!(
            "{}",
            json!({
                "version": VERSION,
                "targets": targets,
                "missing_targets": missing_targets,
                "min_lines": config.min_lines,
                "files_walked": walked,
                "source_files_recognized": recognized,
                "files_intercepted": intercepted,
                "total_original_tokens": total_original,
                "total_skeleton_tokens": total_skeleton,
                "tokens_saved": total_saved,
                "reduction_percent": (avg_reduction * 10.0).round() / 10.0,
                "files": rows.iter().map(|r| json!({
                    "path": r.path,
                    "language": r.language,
                    "lines": r.lines,
                    "original_tokens": r.original_tokens,
                    "skeleton_tokens": r.skeleton_tokens,
                    "saved": r.original_tokens.saturating_sub(r.skeleton_tokens),
                    "reduction_percent": if r.original_tokens > 0 {
                        ((r.original_tokens.saturating_sub(r.skeleton_tokens) as f64 * 1000.0)
                            / r.original_tokens as f64).round() / 10.0
                    } else { 0.0 },
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(());
    }

    println!("# readzip eval");
    println!();
    println!("- readzip version: {VERSION}");
    println!("- target(s): {}", targets.join(", "));
    println!("- min_lines threshold: {}", config.min_lines);
    println!("- files walked: {walked}");
    println!("- source files recognized: {recognized}");
    println!("- files intercepted (lines >= {}): {intercepted}", config.min_lines);
    if intercepted == 0 {
        println!();
        if recognized == 0 {
            println!("(No source files in supported languages were found. Pick a code directory.)");
        } else {
            println!(
                "(No files exceeded the {}-line threshold. Lower `min_lines` in your config or eval a larger corpus.)",
                config.min_lines
            );
        }
        return Ok(());
    }
    println!("- total original tokens: {}", format_tokens(total_original));
    println!("- total skeleton tokens: {}", format_tokens(total_skeleton));
    println!("- tokens saved: {}", format_tokens(total_saved));
    println!("- average reduction: {:.1}%", avg_reduction);
    println!();
    println!("| File | Lang | Lines | Original | Skeleton | Saved | Reduction |");
    println!("|---|---|---:|---:|---:|---:|---:|");
    for r in rows.iter().take(20) {
        let saved = r.original_tokens.saturating_sub(r.skeleton_tokens);
        let pct = if r.original_tokens > 0 {
            saved as f64 * 100.0 / r.original_tokens as f64
        } else {
            0.0
        };
        println!(
            "| {} | {} | {} | {} | {} | {} | {:.1}% |",
            r.path, r.language, r.lines, r.original_tokens, r.skeleton_tokens, saved, pct
        );
    }
    if rows.len() > 20 {
        println!();
        println!(
            "(showing top 20 of {} files by original-token size; pass --json for the full set)",
            rows.len()
        );
    }
    Ok(())
}

fn collect_source_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Skip dotfiles, build artifacts, and dependency vendor dirs.
        if name.starts_with('.')
            || name == "target"
            || name == "node_modules"
            || name == "dist"
            || name == "build"
            || name == "vendor"
            || name == "__pycache__"
        {
            continue;
        }
        if path.is_dir() {
            collect_source_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn language_name_str(lang: readzip_core::Language) -> &'static str {
    use readzip_core::Language as L;
    match lang {
        L::Python => "Python",
        L::JavaScript => "JavaScript",
        L::TypeScript => "TypeScript",
        L::Go => "Go",
        L::Rust => "Rust",
        L::Java => "Java",
        L::Ruby => "Ruby",
        L::C => "C",
        L::Cpp => "C++",
        L::CSharp => "C#",
        L::Php => "PHP",
        L::Swift => "Swift",
        L::Kotlin => "Kotlin",
        L::Scala => "Scala",
        L::Lua => "Lua",
        L::Bash => "Bash",
        L::Unknown => "Unknown",
    }
}

fn skeleton_cmd(args: &[String]) -> Result<(), String> {
    let Some(file) = args.first() else {
        return Err("usage: readzip skeleton <file>".to_string());
    };
    let config = load_config();
    let skeleton = build_skeleton(Path::new(file), &config)
        .map_err(|err| format!("failed to build skeleton for {file}: {err}"))?;
    println!("{}", skeleton.text);
    Ok(())
}

/// Smart `cat` replacement for AI agents calling readzip from Bash.
/// If the file is large enough (and in a supported language) to be intercepted
/// by the hook, prints the structural skeleton. Otherwise prints the file
/// verbatim, exactly like `cat`.
fn read_cmd(args: &[String]) -> Result<(), String> {
    let Some(file) = args.first() else {
        return Err("usage: readzip read <file>".to_string());
    };
    let path = Path::new(file);
    let config = load_config();
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {file}: {err}"))?;
    let line_count = source.lines().count();
    if !should_intercept(path, line_count, &config) {
        // Small file, unsupported language, or in force_full_for / bypass_for —
        // pass through full content like cat.
        print!("{source}");
        return Ok(());
    }
    let skeleton = readzip_core::build_skeleton_from_source(path, &source, &config);
    if skeleton.text.contains("No top-level symbols detected") {
        // No useful skeleton — fall through to full content.
        print!("{source}");
        return Ok(());
    }
    println!("{}", skeleton.text);
    Ok(())
}

/// Print a 1-indexed line range of a file (matches Claude Code's `Read` semantics).
fn section_cmd(args: &[String]) -> Result<(), String> {
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let file = positional
        .first()
        .ok_or_else(|| "usage: readzip section <file> <offset> <limit>".to_string())?;
    let offset: usize = positional
        .get(1)
        .ok_or_else(|| "usage: readzip section <file> <offset> <limit>".to_string())?
        .parse()
        .map_err(|err| format!("offset must be a positive integer: {err}"))?;
    let limit: usize = positional
        .get(2)
        .ok_or_else(|| "usage: readzip section <file> <offset> <limit>".to_string())?
        .parse()
        .map_err(|err| format!("limit must be a positive integer: {err}"))?;
    if offset == 0 {
        return Err("offset is 1-indexed; use 1 for the first line".to_string());
    }
    let text = read_section(Path::new(file.as_str()), offset, limit)
        .map_err(|err| format!("failed to read section of {file}: {err}"))?;
    print!("{text}");
    Ok(())
}

fn read_section(path: &Path, offset: usize, limit: usize) -> io::Result<String> {
    let source = fs::read_to_string(path)?;
    let mut out = String::new();
    for (idx, line) in source
        .lines()
        .enumerate()
        .skip(offset.saturating_sub(1))
        .take(limit)
    {
        out.push_str(&format!("{:>6}  {}\n", idx + 1, line));
    }
    Ok(out)
}

fn ensure_config(force: bool) -> Result<(), String> {
    let config_path = default_config_path();
    if config_path.exists() && !force {
        return Ok(());
    }
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create config dir: {err}"))?;
    }
    fs::write(&config_path, default_config_text(&Config::default()))
        .map_err(|err| format!("failed to write config: {err}"))?;
    Ok(())
}

fn install_claude_hook(yes: bool) -> Result<(), String> {
    let Some(settings_path) = home_path(".claude/settings.json") else {
        return Ok(());
    };
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create Claude settings dir: {err}"))?;
    }
    let existing = fs::read_to_string(&settings_path).unwrap_or_default();
    if existing.contains("readzip hook") {
        return Ok(());
    }
    if settings_path.exists() && !existing.trim().is_empty() && !yes {
        let proceed = confirm(&format!(
            "About to modify {} (a backup will be written first). Continue? [y/N] ",
            settings_path.display()
        ))?;
        if !proceed {
            println!("readzip: skipped Claude Code hook install. Run with --yes to bypass this prompt.");
            return Ok(());
        }
    }
    if settings_path.exists() {
        let backup = settings_path.with_extension(format!("json.readzip-bak-{}", unix_time()));
        fs::copy(&settings_path, backup)
            .map_err(|err| format!("failed to back up Claude settings: {err}"))?;
    }

    let hook_command = current_exe_command("hook");
    let mut settings: Value = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&existing).map_err(|err| {
            format!(
                "Claude settings are not valid JSON; refusing to edit. Fix {}: {err}",
                settings_path.display()
            )
        })?
    };
    ensure_object(&mut settings)?;
    let hooks = settings
        .as_object_mut()
        .expect("validated object")
        .entry("hooks")
        .or_insert_with(|| json!({}));
    ensure_object(hooks)?;
    let pre_tool_use = hooks
        .as_object_mut()
        .expect("validated object")
        .entry("PreToolUse")
        .or_insert_with(|| json!([]));
    ensure_array(pre_tool_use)?;
    pre_tool_use
        .as_array_mut()
        .expect("validated array")
        .push(readzip_claude_hook(&hook_command));

    let pretty = serde_json::to_string_pretty(&settings)
        .map_err(|err| format!("failed to serialize Claude settings: {err}"))?;
    fs::write(&settings_path, format!("{pretty}\n"))
        .map_err(|err| format!("failed to write Claude settings: {err}"))?;
    Ok(())
}

fn uninstall_claude_hook() -> Result<(), String> {
    let Some(settings_path) = home_path(".claude/settings.json") else {
        return Ok(());
    };
    let Ok(existing) = fs::read_to_string(&settings_path) else {
        return Ok(());
    };
    if !existing.contains("readzip hook") {
        return Ok(());
    }
    let backup =
        settings_path.with_extension(format!("json.pre-readzip-uninstall-{}", unix_time()));
    fs::copy(&settings_path, backup)
        .map_err(|err| format!("failed to back up Claude settings: {err}"))?;

    let mut settings: Value = serde_json::from_str(&existing).map_err(|err| {
        format!("Claude settings are not valid JSON; backed up but could not edit: {err}")
    })?;
    if let Some(groups) = settings
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("PreToolUse"))
        .and_then(Value::as_array_mut)
    {
        groups.retain(|group| !value_contains_readzip_hook(group));
    }
    let pretty = serde_json::to_string_pretty(&settings)
        .map_err(|err| format!("failed to serialize Claude settings: {err}"))?;
    fs::write(&settings_path, format!("{pretty}\n"))
        .map_err(|err| format!("failed to write Claude settings: {err}"))?;
    Ok(())
}

fn readzip_claude_hook(command: &str) -> Value {
    json!({
        "matcher": "Read",
        "hooks": [
            {
                "type": "command",
                "command": command,
                "timeout": 10,
                "statusMessage": "readzip skeleton gate"
            }
        ]
    })
}

fn ensure_object(value: &mut Value) -> Result<(), String> {
    if value.is_object() {
        Ok(())
    } else {
        Err("expected JSON object while editing settings".to_string())
    }
}

fn ensure_array(value: &mut Value) -> Result<(), String> {
    if value.is_array() {
        Ok(())
    } else {
        Err("expected JSON array while editing settings".to_string())
    }
}

fn value_contains_readzip_hook(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("readzip hook"),
        Value::Array(items) => items.iter().any(value_contains_readzip_hook),
        Value::Object(map) => map.values().any(value_contains_readzip_hook),
        _ => false,
    }
}

fn confirm(prompt: &str) -> Result<bool, String> {
    if !io::stdin().is_terminal() {
        // Non-interactive: default to "no" so we never silently modify settings.
        return Ok(false);
    }
    use std::io::Write;
    print!("{prompt}");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|err| format!("failed to read confirmation: {err}"))?;
    let answer = answer.trim().to_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn install_codex_hint() -> Result<(), String> {
    let Some(codex_dir) = home_path(".codex") else {
        return Ok(());
    };
    fs::create_dir_all(&codex_dir).map_err(|err| format!("failed to create Codex dir: {err}"))?;
    let hint_path = codex_dir.join("readzip-AGENTS-snippet.md");
    fs::write(
        hint_path,
        "When inspecting source files larger than ~500 lines, run readzip from Bash:\n\
         - `readzip read <file>` — smart cat: skeleton if large, full if small. Saves ~80% tokens on large files.\n\
         - `readzip section <file> <offset> <limit>` — scoped line range when you know what to read.\n\
         - `readzip stats` — tokens saved so far (local-only).\n\n\
         These commands are a single static binary on PATH; they parse with tree-sitter and never make network calls.\n",
    )
    .map_err(|err| format!("failed to write Codex hint: {err}"))?;
    Ok(())
}

fn record_intercept(
    config: &Config,
    source_path: &Path,
    original_tokens: usize,
    skeleton_tokens: usize,
) {
    let path = config.cache_dir.join("stats.tsv");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let line = format!(
        "{}\t{}\t{}\t{}\n",
        unix_time(),
        readzip_core::stable_hash(&source_path.to_string_lossy()),
        original_tokens,
        skeleton_tokens
    );
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(line.as_bytes())
        });
}

#[derive(Default)]
struct Stats {
    files_intercepted: usize,
    original_tokens: usize,
    skeleton_tokens: usize,
}

impl Stats {
    fn tokens_saved(&self) -> usize {
        self.original_tokens.saturating_sub(self.skeleton_tokens)
    }

    fn avg_reduction_percent(&self) -> f64 {
        percent(self.tokens_saved(), self.original_tokens)
    }
}

fn read_stats(config: &Config) -> Stats {
    let path = config.cache_dir.join("stats.tsv");
    let Ok(text) = fs::read_to_string(path) else {
        return Stats::default();
    };
    let mut stats = Stats::default();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 4 {
            continue;
        }
        stats.files_intercepted += 1;
        stats.original_tokens += parts[2].parse::<usize>().unwrap_or(0);
        stats.skeleton_tokens += parts[3].parse::<usize>().unwrap_or(0);
    }
    stats
}

fn print_banner(json: bool) {
    if json || env::var_os("NO_COLOR").is_some() || !io::stdout().is_terminal() {
        return;
    }
    const RESET: &str = "\x1b[0m";
    let lines = [
        "██████╗ ███████╗ █████╗ ██████╗ ███████╗██╗██████╗ ",
        "██╔══██╗██╔════╝██╔══██╗██╔══██╗╚══███╔╝██║██╔══██╗",
        "██████╔╝█████╗  ███████║██║  ██║  ███╔╝ ██║██████╔╝",
        "██╔══██╗██╔══╝  ██╔══██║██║  ██║ ███╔╝  ██║██╔═══╝ ",
        "██║  ██║███████╗██║  ██║██████╔╝███████╗██║██║     ",
        "╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═════╝ ╚══════╝╚═╝╚═╝     ",
    ];
    let truecolor = env::var("COLORTERM")
        .map(|v| v.eq_ignore_ascii_case("truecolor") || v.eq_ignore_ascii_case("24bit"))
        .unwrap_or(false);

    if truecolor {
        // Vertical gradient matching oh-my-logo's grad-blue palette (#4ea8ff -> #7f88ff).
        let gradient: [(u8, u8, u8); 6] = [
            (78, 168, 255),
            (88, 162, 255),
            (98, 155, 255),
            (107, 149, 255),
            (117, 142, 255),
            (127, 136, 255),
        ];
        for (line, (r, g, b)) in lines.iter().zip(gradient.iter()) {
            println!("\x1b[38;2;{r};{g};{b}m{line}{RESET}");
        }
        println!("\x1b[38;2;143;211;255mstructural reads for coding agents{RESET}\n");
    } else {
        // 256-color fallback for Terminal.app and other terminals without 24-bit support.
        // Stepped blues approximating grad-blue.
        let palette: [u8; 6] = [39, 75, 75, 111, 111, 105];
        for (line, code) in lines.iter().zip(palette.iter()) {
            println!("\x1b[38;5;{code}m{line}{RESET}");
        }
        println!("\x1b[38;5;117mstructural reads for coding agents{RESET}\n");
    }
}

fn current_exe_command(subcommand: &str) -> String {
    match env::current_exe() {
        Ok(path) => format!("{} {}", shell_quote(&path.to_string_lossy()), subcommand),
        Err(_) => format!("readzip {subcommand}"),
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn home_path(relative: &str) -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(relative))
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        (numerator as f64 / denominator as f64) * 100.0
    }
}

fn rounded_percent(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn demo_source() -> String {
    let mut out = String::from("import time\nimport hashlib\n\nclass AuthError(Exception):\n    pass\n\nclass TokenManager:\n");
    for i in 0..80 {
        out.push_str(&format!(
            "    def helper_{i}(self, token):\n        return hashlib.sha256(str(token).encode()).hexdigest()\n\n"
        ));
    }
    out.push_str("    def refresh_token(self, token):\n        if not token:\n            raise AuthError('missing token')\n        return self.helper_42(token)\n\n");
    for i in 0..80 {
        out.push_str(&format!(
            "    def fixture_{i}(self):\n        return {{'id': {i}, 'created': time.time()}}\n\n"
        ));
    }
    out
}
