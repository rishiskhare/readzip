use readzip_core::{
    build_skeleton, cached_skeleton, default_config_path, default_config_text, load_config,
    should_intercept, Config,
};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
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
        Some("mcp") => mcp_server(),
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
        "readzip {VERSION}\n\nUSAGE:\n  readzip init [--yes] [--only=a,b | --skip=a,b]\n  readzip hook\n  readzip demo [--json]\n  readzip stats [--json]\n  readzip eval <dir> [--json]\n  readzip uninstall [--keep-cache]\n  readzip doctor [--json]\n  readzip skeleton <file>\n  readzip mcp\n\nKnown agents (init only registers integrations for ones it detects):\n  claude    native PreToolUse hook (transparent)\n  codex     advisory AGENTS.md hint + MCP server\n  cursor    MCP server in ~/.cursor/mcp.json\n  cline     MCP server in the Cline VS Code extension config\n  windsurf  MCP server in ~/.codeium/windsurf/mcp_config.json\n  gemini    MCP server in ~/.gemini/settings.json\n\nUse --only=a,b to limit, --skip=a,b to exclude, or --only=<x> to force install\nfor an agent that init didn't auto-detect.\n"
    );
    Ok(())
}

fn init(args: &[String]) -> Result<(), String> {
    let yes = args.iter().any(|arg| arg == "--yes" || arg == "-y");
    let agents = parse_agents_flag(args);
    let force = agents.user_specified();
    print_banner(false);
    ensure_config(false)?;

    enum Outcome {
        Installed,
        NotInstalled,
        Skipped,
    }

    let mut report: Vec<(&'static str, Outcome, String)> = Vec::new();

    let run_agent = |wants: bool,
                     present: bool,
                     install_fn: &dyn Fn() -> Result<(), String>|
     -> Result<Outcome, String> {
        if !wants {
            return Ok(Outcome::Skipped);
        }
        if !present && !force {
            return Ok(Outcome::NotInstalled);
        }
        install_fn()?;
        Ok(Outcome::Installed)
    };

    let claude_path = home_path(".claude/settings.json")
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let claude_outcome = run_agent(
        agents.wants("claude"),
        agent_present("claude"),
        &|| install_claude_hook(yes),
    )?;
    report.push(("Claude Code", claude_outcome, claude_path));

    let codex_path = home_path(".codex/mcp.json").map(|p| p.display().to_string()).unwrap_or_default();
    let codex_outcome = run_agent(
        agents.wants("codex"),
        agent_present("codex"),
        &install_codex_hint,
    )?;
    report.push(("Codex", codex_outcome, codex_path));

    let cursor_path = home_path(".cursor/mcp.json").map(|p| p.display().to_string()).unwrap_or_default();
    let cursor_outcome = run_agent(
        agents.wants("cursor"),
        agent_present("cursor"),
        &install_cursor_mcp,
    )?;
    report.push(("Cursor", cursor_outcome, cursor_path));

    let cline_path = "Cline VS Code extension".to_string();
    let cline_outcome = run_agent(
        agents.wants("cline"),
        agent_present("cline"),
        &install_cline_mcp,
    )?;
    report.push(("Cline", cline_outcome, cline_path));

    let windsurf_path = home_path(".codeium/windsurf/mcp_config.json")
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let windsurf_outcome = run_agent(
        agents.wants("windsurf"),
        agent_present("windsurf"),
        &install_windsurf_mcp,
    )?;
    report.push(("Windsurf", windsurf_outcome, windsurf_path));

    let gemini_path = home_path(".gemini/settings.json").map(|p| p.display().to_string()).unwrap_or_default();
    let gemini_outcome = run_agent(
        agents.wants("gemini"),
        agent_present("gemini"),
        &install_gemini_mcp,
    )?;
    report.push(("Gemini CLI", gemini_outcome, gemini_path));

    for unsupported in agents.requested_unsupported() {
        eprintln!(
            "readzip: agent '{unsupported}' install path not yet implemented; skipping."
        );
    }

    println!();
    let mut installed_count = 0;
    for (agent, outcome, path) in &report {
        match outcome {
            Outcome::Installed => {
                installed_count += 1;
                println!("  ✓  {:<12}  {}", agent, path);
            }
            Outcome::NotInstalled => {
                println!("  ·  {:<12}  not installed (skipped — use --only={} to force)", agent, agent.to_lowercase().replace(' ', ""));
            }
            Outcome::Skipped => {
                println!("  ·  {:<12}  skipped via --skip / --only", agent);
            }
        }
    }

    println!();
    if installed_count == 0 {
        println!("No agents were detected on this machine. Install one (Claude Code, Codex, Cursor,");
        println!("Cline, Windsurf, or Gemini CLI), or rerun `readzip init --only=<agent>` to force.");
        return Ok(());
    }

    println!("readzip is now active. Try it:");
    println!("  1. Open your agent in a project with a file > 500 lines.");
    println!("  2. Ask it to read that file (without specifying a line range).");
    println!("  3. After a few minutes:  readzip stats");
    println!();
    println!("Or run a corpus eval right now:  readzip eval <some-source-dir>");
    Ok(())
}

fn agent_present(agent: &str) -> bool {
    fn dir_exists(rel: &str) -> bool {
        home_path(rel).map(|p| p.exists()).unwrap_or(false)
    }
    fn any_exists(rels: &[&str]) -> bool {
        rels.iter().any(|r| dir_exists(r))
    }
    match agent {
        "claude" => any_exists(&[".claude"]),
        "codex" => any_exists(&[".codex"]),
        "cursor" => any_exists(&[".cursor"]),
        "cline" => any_exists(&[
            "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev",
            ".config/Code/User/globalStorage/saoudrizwan.claude-dev",
            ".config/cline",
        ]),
        "windsurf" => any_exists(&[".codeium/windsurf", ".codeium"]),
        "gemini" => any_exists(&[".gemini"]),
        _ => false,
    }
}

#[derive(Debug, Default)]
struct AgentSelection {
    only: Option<Vec<String>>,
    skip: Vec<String>,
    requested: Vec<String>,
}

const SUPPORTED_AGENTS_NATIVE: &[&str] = &["claude"];
const SUPPORTED_AGENTS_HINT: &[&str] = &["codex", "cursor", "cline", "windsurf", "gemini"];
const KNOWN_AGENTS: &[&str] = &[
    "claude", "codex", "cursor", "cline", "windsurf", "gemini",
];

impl AgentSelection {
    fn wants(&self, agent: &str) -> bool {
        if self.skip.iter().any(|s| s == agent) {
            return false;
        }
        if let Some(only) = &self.only {
            return only.iter().any(|a| a == agent);
        }
        // Default behavior: install everything we currently support.
        SUPPORTED_AGENTS_NATIVE.contains(&agent) || SUPPORTED_AGENTS_HINT.contains(&agent)
    }

    /// True iff the user passed --only/--agents/--skip — i.e. force-install regardless of detection.
    fn user_specified(&self) -> bool {
        self.only.is_some() || !self.skip.is_empty()
    }

    fn requested_unsupported(&self) -> Vec<String> {
        let supported: Vec<&str> = SUPPORTED_AGENTS_NATIVE
            .iter()
            .chain(SUPPORTED_AGENTS_HINT.iter())
            .copied()
            .collect();
        self.requested
            .iter()
            .filter(|a| KNOWN_AGENTS.contains(&a.as_str()))
            .filter(|a| !supported.iter().any(|s| s == &a.as_str()))
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
    uninstall_claude_hook()?;
    for path in mcp_agent_paths().into_iter().flatten() {
        uninstall_mcp_from(&path)?;
    }
    if !keep_cache {
        let config = load_config();
        let _ = fs::remove_dir_all(&config.cache_dir);
    }
    println!("readzip uninstalled.");
    Ok(())
}

fn mcp_agent_paths() -> Vec<Option<PathBuf>> {
    vec![
        home_path(".codex/mcp.json"),
        home_path(".cursor/mcp.json"),
        home_path("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"),
        home_path(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"),
        home_path(".config/cline/cline_mcp_settings.json"),
        home_path(".codeium/windsurf/mcp_config.json"),
        home_path(".gemini/settings.json"),
    ]
}

fn uninstall_mcp_from(path: &Path) -> Result<(), String> {
    let Ok(existing) = fs::read_to_string(path) else {
        return Ok(());
    };
    if !existing.contains("\"readzip\"") {
        return Ok(());
    }
    let mut settings: Value = match serde_json::from_str(&existing) {
        Ok(v) => v,
        Err(_) => return Ok(()), // Don't damage non-JSON files we partially wrote.
    };
    if let Some(servers) = settings
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
    {
        servers.remove("readzip");
    }
    let pretty = serde_json::to_string_pretty(&settings)
        .map_err(|err| format!("failed to serialize settings: {err}"))?;
    fs::write(path, format!("{pretty}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
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
    let mcp_status = check_mcp_agents();
    if json {
        println!(
            "{}",
            json!({
                "version": VERSION,
                "config_path": config_path,
                "cache_dir": config.cache_dir,
                "claude_hook_installed": claude_installed,
                "mcp_agents": mcp_status.iter()
                    .map(|(agent, installed, _)| (agent.to_string(), Value::Bool(*installed)))
                    .collect::<serde_json::Map<String, Value>>(),
                "known_limitations": [
                    "Native Read hooks fire for Claude Code only; MCP agents must choose readzip's MCP tools",
                    "Claude MCP filesystem reads bypass native Read hooks (issue #33106)"
                ]
            })
        );
        return Ok(());
    }
    println!("readzip doctor");
    println!("  version:               {VERSION}");
    println!("  config:                {}", config_path.display());
    println!("  cache:                 {}", config.cache_dir.display());
    println!("  Claude hook installed: {claude_installed}");
    for (agent, installed, path) in &mcp_status {
        println!(
            "  {:<10} MCP:        {} ({})",
            agent,
            if *installed { "yes" } else { "no" },
            path
        );
    }
    println!("  limitation: only Claude Code is transparent — other agents must choose readzip's MCP tools.");
    println!("  limitation: Claude MCP filesystem reads bypass native Read hooks (issue #33106).");
    Ok(())
}

fn check_mcp_agents() -> Vec<(&'static str, bool, String)> {
    let entries: &[(&str, &str)] = &[
        ("Codex", ".codex/mcp.json"),
        ("Cursor", ".cursor/mcp.json"),
        ("Windsurf", ".codeium/windsurf/mcp_config.json"),
        ("Gemini", ".gemini/settings.json"),
    ];
    entries
        .iter()
        .map(|(name, rel)| {
            let path = home_path(rel);
            let installed = path
                .as_ref()
                .and_then(|p| fs::read_to_string(p).ok())
                .map(|text| text.contains("\"readzip\""))
                .unwrap_or(false);
            let display = path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| rel.to_string());
            (*name, installed, display)
        })
        .collect()
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

fn mcp_server() -> Result<(), String> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout();
    loop {
        let Some((message, framed)) = read_mcp_message(&mut reader)? else {
            break;
        };
        let payload: Value =
            serde_json::from_str(&message).map_err(|err| format!("invalid MCP JSON: {err}"))?;
        let method = payload
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if method == "notifications/initialized" {
            continue;
        }
        let response = handle_mcp_message(&payload);
        write_mcp_response(&mut stdout, &response, framed)?;
    }
    Ok(())
}

fn read_mcp_message<R: BufRead>(reader: &mut R) -> Result<Option<(String, bool)>, String> {
    let mut first = String::new();
    let bytes = reader
        .read_line(&mut first)
        .map_err(|err| format!("failed to read MCP stdin: {err}"))?;
    if bytes == 0 {
        return Ok(None);
    }
    if first.trim().is_empty() {
        return read_mcp_message(reader);
    }
    if first.trim_start().starts_with('{') {
        return Ok(Some((first, false)));
    }

    let mut content_length = None;
    let mut header = first;
    loop {
        let trimmed = header.trim();
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }
        header.clear();
        let bytes = reader
            .read_line(&mut header)
            .map_err(|err| format!("failed to read MCP header: {err}"))?;
        if bytes == 0 || header.trim().is_empty() {
            break;
        }
    }

    let Some(length) = content_length else {
        return Err("MCP message missing Content-Length".to_string());
    };
    let mut body = vec![0_u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|err| format!("failed to read MCP body: {err}"))?;
    let message = String::from_utf8(body).map_err(|err| format!("MCP body is not UTF-8: {err}"))?;
    Ok(Some((message, true)))
}

fn handle_mcp_message(payload: &Value) -> String {
    let id = payload.get("id").cloned().unwrap_or(Value::Null);
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "readzip", "version": VERSION}
            }
        })
        .to_string(),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"tools": mcp_tools()}
        })
        .to_string(),
        "tools/call" => {
            let params = payload.get("params").unwrap_or(&Value::Null);
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params.get("arguments").unwrap_or(&Value::Null);
            mcp_tool_call_response(&id, name, args)
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": "unknown method"}
        })
        .to_string(),
    }
}

fn write_mcp_response<W: Write>(
    writer: &mut W,
    response: &str,
    framed: bool,
) -> Result<(), String> {
    if framed {
        write!(
            writer,
            "Content-Length: {}\r\n\r\n{}",
            response.len(),
            response
        )
        .map_err(|err| format!("failed to write MCP response: {err}"))?;
    } else {
        writeln!(writer, "{response}")
            .map_err(|err| format!("failed to write MCP response: {err}"))?;
    }
    writer
        .flush()
        .map_err(|err| format!("failed to flush MCP stdout: {err}"))
}

fn mcp_tools() -> Value {
    json!([
        {
            "name": "readzip_skeleton",
            "description": "Return a compact structural skeleton for a source file.",
            "inputSchema": {
                "type": "object",
                "properties": {"file_path": {"type": "string"}},
                "required": ["file_path"]
            }
        },
        {
            "name": "readzip_section",
            "description": "Return a scoped section of a file by offset and limit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"},
                    "offset": {"type": "number"},
                    "limit": {"type": "number"}
                },
                "required": ["file_path", "offset", "limit"]
            }
        },
        {
            "name": "readzip_stats",
            "description": "Return local readzip savings stats.",
            "inputSchema": {"type": "object", "properties": {}}
        }
    ])
}

fn mcp_tool_call_response(id: &Value, name: &str, args: &Value) -> String {
    let result = match name {
        "readzip_skeleton" => {
            let Some(file_path) = args.get("file_path").and_then(Value::as_str) else {
                return mcp_error(id, "missing file_path");
            };
            let config = load_config();
            match build_skeleton(Path::new(&file_path), &config) {
                Ok(skeleton) => skeleton.text,
                Err(err) => return mcp_error(id, &format!("failed to build skeleton: {err}")),
            }
        }
        "readzip_section" => {
            let Some(file_path) = args.get("file_path").and_then(Value::as_str) else {
                return mcp_error(id, "missing file_path");
            };
            let offset = args
                .get("offset")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1) as usize;
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(120)
                .max(1) as usize;
            match read_section(Path::new(&file_path), offset, limit) {
                Ok(section) => section,
                Err(err) => return mcp_error(id, &format!("failed to read section: {err}")),
            }
        }
        "readzip_stats" => {
            let config = load_config();
            let stats = read_stats(&config);
            json!({
                "files_intercepted": stats.files_intercepted,
                "tokens_saved": stats.tokens_saved(),
                "avg_reduction_percent": rounded_percent(stats.avg_reduction_percent())
            })
            .to_string()
        }
        _ => return mcp_error(id, "unknown tool"),
    };
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {"content": [{"type": "text", "text": result}]}
    })
    .to_string()
}

fn mcp_error(id: &Value, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32000, "message": message}
    })
    .to_string()
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
        "When inspecting large source files, prefer the readzip MCP tools: readzip_skeleton first, then readzip_section for the needed symbol or range. Native read_file may bypass readzip; verify savings with `readzip stats`.\n",
    )
    .map_err(|err| format!("failed to write Codex hint: {err}"))?;
    install_mcp_into(home_path(".codex/mcp.json"), "Codex")
}

fn install_cursor_mcp() -> Result<(), String> {
    install_mcp_into(home_path(".cursor/mcp.json"), "Cursor")
}

fn install_cline_mcp() -> Result<(), String> {
    // Cline's VS Code extension stores its MCP config under the user data dir;
    // the location varies by OS. We try the most common path; if missing,
    // we drop a config in `~/.config/cline/` and let the user move it.
    let candidates = [
        home_path("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"),
        home_path(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"),
        home_path(".config/cline/cline_mcp_settings.json"),
    ];
    let target = candidates
        .into_iter()
        .flatten()
        .find(|p| p.parent().map(|d| d.exists()).unwrap_or(false))
        .or_else(|| home_path(".config/cline/cline_mcp_settings.json"));
    install_mcp_into(target, "Cline")
}

fn install_windsurf_mcp() -> Result<(), String> {
    install_mcp_into(home_path(".codeium/windsurf/mcp_config.json"), "Windsurf")
}

fn install_gemini_mcp() -> Result<(), String> {
    install_mcp_into(home_path(".gemini/settings.json"), "Gemini CLI")
}

fn install_mcp_into(target: Option<PathBuf>, agent_label: &str) -> Result<(), String> {
    let Some(path) = target else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {agent_label} config dir: {err}"))?;
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("\"readzip\"") {
        return Ok(());
    }
    if path.exists() && !existing.trim().is_empty() {
        let suffix = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("json");
        let backup =
            path.with_extension(format!("{suffix}.readzip-bak-{}", unix_time()));
        fs::copy(&path, &backup)
            .map_err(|err| format!("failed to back up {agent_label} settings: {err}"))?;
    }

    let mut settings: Value = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&existing).map_err(|err| {
            format!(
                "{agent_label} settings at {} are not valid JSON; refusing to edit: {err}",
                path.display()
            )
        })?
    };
    ensure_object(&mut settings)?;
    let servers = settings
        .as_object_mut()
        .expect("validated object")
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    ensure_object(servers)?;
    let entry = readzip_mcp_entry();
    servers
        .as_object_mut()
        .expect("validated object")
        .insert("readzip".to_string(), entry);

    let pretty = serde_json::to_string_pretty(&settings)
        .map_err(|err| format!("failed to serialize {agent_label} settings: {err}"))?;
    fs::write(&path, format!("{pretty}\n"))
        .map_err(|err| format!("failed to write {agent_label} settings: {err}"))?;
    Ok(())
}

fn readzip_mcp_entry() -> Value {
    let exe = env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "readzip".to_string());
    json!({
        "command": exe,
        "args": ["mcp"],
    })
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
