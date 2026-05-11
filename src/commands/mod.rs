//! Command handlers for the lorum CLI.
//!
//! Each public function corresponds to a CLI subcommand and returns a
//! `Result<(), LorumError>` so that the top-level `main` can report errors
//! uniformly.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::{self, McpServer};
use crate::error::LorumError;

/// Run the `init` subcommand stub.
pub fn run_init(local: bool) -> Result<(), LorumError> {
    eprintln!("TODO: implement init (local={local})");
    Ok(())
}

/// Run the `import` subcommand stub.
pub fn run_import(from: &str) -> Result<(), LorumError> {
    eprintln!("TODO: implement import (from={from})");
    Ok(())
}

/// Run the `sync` subcommand stub.
pub fn run_sync(dry_run: bool, tools: &[String], expand_env: bool) -> Result<(), LorumError> {
    eprintln!(
        "TODO: implement sync (dry_run={}, tools={:?}, expand_env={})",
        dry_run, tools, expand_env
    );
    Ok(())
}

/// Run the `check` subcommand stub.
pub fn run_check() -> Result<(), LorumError> {
    eprintln!("TODO: implement check");
    Ok(())
}

/// Run the `status` subcommand stub.
pub fn run_status() -> Result<(), LorumError> {
    eprintln!("TODO: implement status");
    Ok(())
}

/// Run the `config` subcommand stub.
pub fn run_config(resolve_env: bool, local: bool, global: bool) -> Result<(), LorumError> {
    eprintln!(
        "TODO: implement config (resolve_env={}, local={}, global={})",
        resolve_env, local, global
    );
    Ok(())
}

/// Run the `backup list` subcommand stub.
pub fn run_backup_list() -> Result<(), LorumError> {
    eprintln!("TODO: implement backup list");
    Ok(())
}

/// Run the `backup restore` subcommand stub.
pub fn run_backup_restore(tool: &str) -> Result<(), LorumError> {
    eprintln!("TODO: implement backup restore (tool={tool})");
    Ok(())
}

/// Run the `mcp add` subcommand.
///
/// Adds a new MCP server entry (or overwrites an existing one with the same
/// name). If `config_path` is provided, the entry is written to that file;
/// otherwise the global configuration file is used.
pub fn run_mcp_add(
    name: &str,
    command: &str,
    args: &[String],
    env: &[String],
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    let path = match config_path {
        Some(p) => PathBuf::from(p),
        None => config::global_config_path()?,
    };
    let mut cfg = config::load_config(&path).unwrap_or_default();
    let env_map = parse_env_pairs(env);
    let server = McpServer {
        command: command.to_string(),
        args: args.to_vec(),
        env: env_map,
    };
    cfg.mcp.servers.insert(name.to_string(), server);
    config::save_config(&path, &cfg)?;
    println!("added server: {name}");
    Ok(())
}

/// Run the `mcp remove` subcommand.
///
/// Removes the named MCP server entry. If `config_path` is provided, the entry
/// is removed from that file; otherwise the global configuration file is used.
/// Returns an error if no server with the given name exists.
pub fn run_mcp_remove(name: &str, config_path: Option<&str>) -> Result<(), LorumError> {
    let path = match config_path {
        Some(p) => PathBuf::from(p),
        None => config::global_config_path()?,
    };
    let mut cfg = config::load_config(&path).unwrap_or_default();
    if cfg.mcp.servers.remove(name).is_none() {
        return Err(LorumError::Other {
            message: format!("server not found: {name}"),
        });
    }
    config::save_config(&path, &cfg)?;
    println!("removed server: {name}");
    Ok(())
}

/// Run the `mcp list` subcommand.
///
/// Prints all configured MCP servers in a simple aligned table. If
/// `config_path` is provided, servers are read from that file; otherwise the
/// global configuration file is used.
pub fn run_mcp_list(config_path: Option<&str>) -> Result<(), LorumError> {
    let path = match config_path {
        Some(p) => PathBuf::from(p),
        None => config::global_config_path()?,
    };
    let cfg = config::load_config(&path).unwrap_or_default();
    if cfg.mcp.servers.is_empty() {
        println!("no MCP servers configured");
        return Ok(());
    }
    let max_name = cfg.mcp.servers.keys().map(|n| n.len()).max().unwrap_or(0);
    println!("{:<width$}  COMMAND", "NAME", width = max_name);
    for (name, server) in &cfg.mcp.servers {
        let args_str = if server.args.is_empty() {
            String::new()
        } else {
            format!(" {}", server.args.join(" "))
        };
        println!(
            "{:<width$}  {}{}",
            name,
            server.command,
            args_str,
            width = max_name
        );
    }
    Ok(())
}

/// Run the `mcp edit` subcommand.
///
/// Updates the specified fields of an existing MCP server entry. Fields that
/// are `None` are left unchanged. If `config_path` is provided, the entry is
/// read from and written to that file; otherwise the global configuration file
/// is used. Returns an error if no server with the given name exists.
pub fn run_mcp_edit(
    name: &str,
    command: Option<&str>,
    args: Option<&[String]>,
    env: Option<&[String]>,
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    let path = match config_path {
        Some(p) => PathBuf::from(p),
        None => config::global_config_path()?,
    };
    let mut cfg = config::load_config(&path).unwrap_or_default();
    let server = cfg
        .mcp
        .servers
        .get_mut(name)
        .ok_or_else(|| LorumError::Other {
            message: format!("server not found: {name}"),
        })?;
    if let Some(cmd) = command {
        server.command = cmd.to_string();
    }
    if let Some(new_args) = args {
        server.args = new_args.to_vec();
    }
    if let Some(new_env) = env {
        server.env = parse_env_pairs(new_env);
    }
    config::save_config(&path, &cfg)?;
    println!("updated server: {name}");
    Ok(())
}

/// Run the `hook` subcommand stub.
pub fn run_hook() -> Result<(), LorumError> {
    eprintln!("TODO: implement hook");
    Ok(())
}

/// Run the `skill` subcommand stub.
pub fn run_skill() -> Result<(), LorumError> {
    eprintln!("TODO: implement skill");
    Ok(())
}

/// Parses `KEY=VALUE` string pairs into a `BTreeMap`.
///
/// Pairs that do not contain `=` or have an empty key are silently skipped.
fn parse_env_pairs(pairs: &[String]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for pair in pairs {
        if let Some((k, v)) = pair.split_once('=') {
            if !k.is_empty() {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests;
