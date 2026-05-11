//! MCP server CRUD command handlers.
//!
//! Functions for adding, removing, listing, and editing MCP server entries
//! in the lorum configuration file.

use std::collections::BTreeMap;

use crate::config::{self, McpServer};
use crate::error::LorumError;

use super::resolve_path;

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
    let path = resolve_path(config_path)?;
    let mut cfg = super::load_config_or_default(&path)?;
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
    let path = resolve_path(config_path)?;
    let mut cfg = super::load_config_or_default(&path)?;
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
    let path = resolve_path(config_path)?;
    let cfg = super::load_config_or_default(&path)?;
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
    let path = resolve_path(config_path)?;
    let mut cfg = super::load_config_or_default(&path)?;
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

/// Parses `KEY=VALUE` string pairs into a `BTreeMap`.
///
/// Pairs that do not contain `=` or have an empty key are silently skipped.
pub fn parse_env_pairs(pairs: &[String]) -> BTreeMap<String, String> {
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
