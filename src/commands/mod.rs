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
pub fn run_check(config_path: Option<&str>) -> Result<(), LorumError> {
    let config =
        config::resolve_effective_config_from_cwd(config_path.map(PathBuf::from).as_deref())?;

    let mut issues = Vec::new();

    for (name, server) in &config.mcp.servers {
        if server.command.is_empty() {
            issues.push(format!("server '{name}' has empty command"));
        }
    }

    if issues.is_empty() {
        println!("config is valid ({} servers)", config.mcp.servers.len());
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

/// Run the `status` subcommand: shows installation status per tool.
pub fn run_status(_config_path: Option<&str>) -> Result<(), LorumError> {
    for adapter in crate::adapters::all_adapters() {
        let paths = adapter.config_paths();
        let any_exists = paths.iter().any(|p| p.exists());

        let mcp_count = if any_exists {
            adapter.read_mcp().map(|m| m.servers.len()).unwrap_or(0)
        } else {
            0
        };

        let status = if any_exists { "installed" } else { "not found" };
        println!(
            "{:<15} {:<12} {} servers",
            adapter.name(),
            status,
            mcp_count
        );
    }
    Ok(())
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
