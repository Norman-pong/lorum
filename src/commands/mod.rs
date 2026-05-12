//! Command handlers for the lorum CLI.
//!
//! Each public function corresponds to a CLI subcommand and returns a
//! `Result<(), LorumError>` for uniform error reporting.

use std::path::PathBuf;

use crate::config;
use crate::error::LorumError;

pub mod backup_cmds;
pub mod hook;
#[cfg(test)]
mod hook_tests;
pub mod mcp;
#[cfg(test)]
mod mcp_tests;
pub mod rule;
#[cfg(test)]
mod rule_tests;
pub mod skill;
#[cfg(test)]
mod skill_tests;
#[cfg(test)]
mod tests;

/// Resolve a config path: returns `config_path` or the global default.
fn resolve_path(config_path: Option<&str>) -> Result<PathBuf, LorumError> {
    match config_path {
        Some(p) => Ok(PathBuf::from(p)),
        None => config::global_config_path(),
    }
}

/// Load config, treating "file not found" as empty default; propagates parse errors.
fn load_config_or_default(path: &std::path::Path) -> Result<config::LorumConfig, LorumError> {
    match config::load_config(path) {
        Ok(cfg) => Ok(cfg),
        Err(LorumError::ConfigNotFound { .. }) => Ok(config::LorumConfig::default()),
        Err(e) => Err(e),
    }
}

/// Run the `init` subcommand: creates a default config file.
pub fn run_init(config_path: Option<&str>, local: bool) -> Result<(), LorumError> {
    let path = if local {
        std::env::current_dir()?.join(".lorum").join("config.yaml")
    } else {
        resolve_path(config_path)?
    };

    if path.exists() {
        println!("config already exists: {}", path.display());
        return Ok(());
    }

    let detected = detect_installed_tools();
    let cfg = config::LorumConfig::default();
    config::save_config(&path, &cfg)?;

    println!("created config at: {}", path.display());
    if !detected.is_empty() {
        println!("detected tools: {}", detected.join(", "));
        println!("run `lorum import --from <tool>` to import MCP configuration");
    }
    Ok(())
}

/// Detect which AI coding tools are installed by checking for their config dirs.
fn detect_installed_tools() -> Vec<String> {
    let mut tools = Vec::new();
    if let Some(home) = dirs::home_dir() {
        if home.join(".claude").exists() {
            tools.push("claude-code".into());
        }
        if home.join(".codex").exists() {
            tools.push("codex".into());
        }
        if home.join(".proma").exists() {
            tools.push("proma".into());
        }
        if home.join(".kimi").exists() {
            tools.push("kimi".into());
        }
    }
    if std::env::current_dir()
        .map(|d| d.join(".trae").exists())
        .unwrap_or(false)
    {
        tools.push("trae".into());
    }
    tools
}
/// Run the `import` subcommand: reads MCP config from tools and merges.
pub fn run_import(from: &str, config_path: Option<&str>) -> Result<(), LorumError> {
    let path = resolve_path(config_path)?;
    let mut lorum_config = load_config_or_default(&path)?;

    let adapters = if from == "all" {
        crate::adapters::all_adapters()
    } else {
        vec![
            crate::adapters::find_adapter(from)
                .ok_or_else(|| LorumError::AdapterNotFound { name: from.into() })?,
        ]
    };

    let mut total_imported = 0;
    for adapter in adapters {
        match adapter.read_mcp() {
            Ok(mcp) => {
                for (name, server) in &mcp.servers {
                    lorum_config
                        .mcp
                        .servers
                        .insert(name.clone(), server.clone());
                    total_imported += 1;
                }
                println!(
                    "imported {} servers from {}",
                    mcp.servers.len(),
                    adapter.name()
                );
            }
            Err(e) => eprintln!("warning: failed to read from {}: {e}", adapter.name()),
        }
    }

    config::save_config(&path, &lorum_config)?;
    println!("imported {total_imported} servers total");
    Ok(())
}

/// Run the `sync` subcommand: synchronises (or dry-runs) MCP configuration.
pub fn run_sync(
    dry_run: bool,
    tools: &[String],
    expand_env: bool,
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    let config =
        config::resolve_effective_config_from_cwd(config_path.map(PathBuf::from).as_deref())?;
    let mcp = crate::env_interpolate::interpolate_mcp_config(&config.mcp, expand_env);

    if dry_run {
        let results = if tools.is_empty() {
            crate::sync::dry_run_all(&mcp)
        } else {
            crate::sync::dry_run_tools(&mcp, tools)
        };
        print_dry_run_results(&results);
    } else {
        let results = if tools.is_empty() {
            crate::sync::sync_all(&mcp)
        } else {
            crate::sync::sync_tools(&mcp, tools)
        };
        let failed = print_sync_results(&results);
        if failed > 0 {
            eprintln!("{failed} tool(s) failed to sync");
        }
    }
    Ok(())
}

fn print_dry_run_results(results: &[crate::sync::DryRunResult]) {
    for r in results {
        let status = if r.success { "OK" } else { "FAIL" };
        if let Some(diff) = &r.diff {
            let summary = format!(
                "+{}/-{}/~{}/={}",
                diff.added.len(),
                diff.removed.len(),
                diff.modified.len(),
                diff.unchanged.len()
            );
            println!("{:<15} {:<6} {summary}", r.tool, status);
        } else {
            println!("{:<15} {:<6}", r.tool, status);
        }
        if let Some(err) = &r.error {
            println!("  error: {err}");
        }
    }
}

fn print_sync_results(results: &[crate::sync::SyncResult]) -> usize {
    for r in results {
        let status = if r.success { "OK" } else { "FAIL" };
        println!("{:<15} {:<6} {} servers", r.tool, status, r.servers_synced);
        if let Some(err) = &r.error {
            println!("  error: {err}");
        }
    }
    results.iter().filter(|r| !r.success).count()
}

/// Run the `check` subcommand: validates the effective configuration.
///
/// Checks MCP servers (command availability, env references), hooks (event
/// names, handler fields), and the unified skills directory structure.
pub fn run_check(config_path: Option<&str>) -> Result<(), LorumError> {
    let config =
        config::resolve_effective_config_from_cwd(config_path.map(PathBuf::from).as_deref())?;

    let mut issues = Vec::new();

    // ── MCP server checks ───────────────────────────────────────────
    for (name, server) in &config.mcp.servers {
        if server.command.is_empty() {
            issues.push(format!("server '{name}' has empty command"));
            continue;
        }
        if !command_exists(&server.command) {
            issues.push(format!(
                "server '{name}' command '{}' not found on PATH",
                server.command
            ));
        }
        // Check for unset env references in command, args, and env values.
        let mut refs = find_unset_env_refs(&server.command);
        for arg in &server.args {
            refs.extend(find_unset_env_refs(arg));
        }
        for val in server.env.values() {
            refs.extend(find_unset_env_refs(val));
        }
        for var in refs {
            issues.push(format!(
                "server '{name}' references unset environment variable '${{{var}}}'"
            ));
        }
    }

    // ── Hooks checks ────────────────────────────────────────────────
    for (event, handlers) in &config.hooks.events {
        if event.is_empty() {
            issues.push("hooks: empty event name".into());
            continue;
        }
        if !is_valid_kebab_case(event) {
            issues.push(format!(
                "hooks: event '{event}' is not valid kebab-case"
            ));
        }
        for (i, h) in handlers.iter().enumerate() {
            if h.matcher.is_empty() {
                issues.push(format!(
                    "hooks: event '{event}' handler {i} has empty matcher"
                ));
            }
            if h.command.is_empty() {
                issues.push(format!(
                    "hooks: event '{event}' handler {i} has empty command"
                ));
            }
        }
    }

    // ── Skills directory checks ─────────────────────────────────────
    match crate::skills::global_skills_dir() {
        Ok(dir) if dir.exists() => {
            match crate::skills::scan_skills_dir(&dir) {
                Ok(entries) => {
                    for entry in &entries {
                        if entry.manifest.name.is_empty() {
                            issues.push(format!(
                                "skill '{}' has empty manifest name",
                                entry.dir_path.display()
                            ));
                        }
                    }
                }
                Err(e) => {
                    issues.push(format!(
                        "failed to scan skills directory '{}': {e}",
                        dir.display()
                    ));
                }
            }
        }
        _ => {} // No global skills directory yet — that's fine.
    }

    // ── Summary ─────────────────────────────────────────────────────
    if issues.is_empty() {
        let hook_events = config.hooks.events.len();
        let hook_handlers: usize = config.hooks.events.values().map(|v| v.len()).sum();
        println!(
            "config is valid ({} servers, {} hook events, {} handlers)",
            config.mcp.servers.len(),
            hook_events,
            hook_handlers,
        );
    } else {
        for issue in &issues {
            eprintln!("issue: {issue}");
        }
        return Err(LorumError::Other {
            message: format!("{} issue(s) found", issues.len()),
        });
    }
    Ok(())
}

/// Check whether a command exists on PATH or as an absolute/relative path.
fn command_exists(cmd: &str) -> bool {
    if cmd.contains('/') || cmd.contains('\\') {
        return std::path::Path::new(cmd).is_file();
    }
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in path_env.split(if cfg!(windows) { ';' } else { ':' }) {
            let full = std::path::Path::new(dir).join(cmd);
            if full.is_file() {
                return true;
            }
            #[cfg(windows)]
            if std::path::Path::new(dir).join(format!("{cmd}.exe")).is_file() {
                return true;
            }
        }
    }
    false
}

/// Find all `${VAR}` references in a string and return those that are unset.
fn find_unset_env_refs(value: &str) -> Vec<String> {
    let mut unset = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
            i += 2;
            let mut var_name = String::new();
            let mut found_close = false;
            while i < chars.len() {
                if chars[i] == '}' {
                    found_close = true;
                    i += 1;
                    break;
                }
                var_name.push(chars[i]);
                i += 1;
            }
            if found_close && std::env::var(&var_name).is_err() {
                unset.push(var_name);
            }
        } else {
            i += 1;
        }
    }
    unset
}

/// Validate that a string is valid kebab-case (lowercase letters, digits, hyphens).
fn is_valid_kebab_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Run the `status` subcommand: shows installation status per tool.
///
/// Displays a panoramic view across all four dimensions (MCP, Rules, Hooks,
/// Skills) for every registered tool.
pub fn run_status(_config_path: Option<&str>) -> Result<(), LorumError> {
    let mut tool_names = std::collections::BTreeSet::new();

    for a in crate::adapters::all_adapters() {
        tool_names.insert(a.name().to_string());
    }
    for a in crate::adapters::all_rules_adapters() {
        tool_names.insert(a.name().to_string());
    }
    for a in crate::adapters::all_hooks_adapters() {
        tool_names.insert(a.name().to_string());
    }
    for a in crate::adapters::all_skills_adapters() {
        tool_names.insert(a.name().to_string());
    }

    let cwd = std::env::current_dir().ok();

    println!(
        "{:<15} {:>6} {:>8} {:>8} {:>8}",
        "TOOL", "MCP", "RULES", "HOOKS", "SKILLS"
    );

    for name in tool_names {
        let mcp = mcp_status(&name);
        let rules = rules_status(&name, cwd.as_deref());
        let hooks = hooks_status(&name);
        let skills = skills_status(&name);

        println!(
            "{:<15} {:>6} {:>8} {:>8} {:>8}",
            name,
            fmt_count(mcp),
            fmt_count(rules),
            fmt_count(hooks),
            fmt_count(skills),
        );
    }
    Ok(())
}

/// Format a dimension count for display: `None` → "-", `Some(0)` → "·", `Some(n)` → "n".
fn fmt_count(count: Option<usize>) -> String {
    match count {
        None => "-".to_string(),
        Some(0) => "·".to_string(),
        Some(n) => n.to_string(),
    }
}

/// Query MCP server count for a tool, returning `None` if the tool has no MCP adapter.
fn mcp_status(name: &str) -> Option<usize> {
    let adapter = crate::adapters::find_adapter(name)?;
    let paths = adapter.config_paths();
    if !paths.iter().any(|p| p.exists()) {
        return Some(0);
    }
    adapter.read_mcp().map(|m| m.servers.len()).ok()
}

/// Query rules section count for a tool, returning `None` if unsupported.
fn rules_status(name: &str, project_root: Option<&std::path::Path>) -> Option<usize> {
    let adapter = crate::adapters::find_rules_adapter(name)?;
    let root = project_root?;
    let content = adapter.read_rules(root).ok()?;
    Some(content.map(|c| crate::rules::parse_rules(&c).sections.len()).unwrap_or(0))
}

/// Query hooks count for a tool, returning `None` if unsupported.
fn hooks_status(name: &str) -> Option<usize> {
    let adapter = crate::adapters::find_hooks_adapter(name)?;
    let paths = adapter.config_paths();
    if !paths.iter().any(|p| p.exists()) {
        return Some(0);
    }
    adapter.read_hooks().map(|h| h.events.values().map(|v| v.len()).sum()).ok()
}

/// Query skills count for a tool, returning `None` if unsupported.
fn skills_status(name: &str) -> Option<usize> {
    let adapter = crate::adapters::find_skills_adapter(name)?;
    adapter.read_skills().map(|s| s.len()).ok()
}

/// Run the `config` subcommand: outputs resolved configuration as YAML.
pub fn run_config(
    resolve_env: bool,
    local: bool,
    global: bool,
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    let config = if let Some(p) = config_path {
        config::load_config(std::path::Path::new(p))?
    } else if global {
        let path = config::global_config_path()?;
        load_config_or_default(&path)?
    } else if local {
        let cwd = std::env::current_dir()?;
        match config::find_project_config(&cwd) {
            Some(p) => {
                let proj = config::load_project_config(&p)?;
                config::LorumConfig {
                    mcp: proj.mcp,
                    hooks: proj.hooks,
                }
            }
            None => {
                return Err(LorumError::ConfigNotFound {
                    path: cwd.join(".lorum").join("config.yaml"),
                });
            }
        }
    } else {
        config::resolve_effective_config_from_cwd(None)?
    };

    let output = if resolve_env {
        let mcp = crate::env_interpolate::interpolate_mcp_config(&config.mcp, true);
        config::LorumConfig {
            mcp,
            hooks: config.hooks,
        }
    } else {
        config
    };
    let yaml = serde_yaml::to_string(&output).map_err(|e| LorumError::Other {
        message: format!("failed to serialize config: {e}"),
    })?;
    print!("{yaml}");
    Ok(())
}

/// Run the `backup list` subcommand.
pub fn run_backup_list(config_path: Option<&str>) -> Result<(), LorumError> {
    backup_cmds::run_backup_list(config_path)
}

/// Run the `backup restore` subcommand.
pub fn run_backup_restore(tool: &str, config_path: Option<&str>) -> Result<(), LorumError> {
    backup_cmds::run_backup_restore(tool, config_path)
}
