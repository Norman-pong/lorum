//! Shared TOML utilities for adapters that use TOML config files.
//!
//! Provides parsing, serialization, reading, and writing of MCP server
//! configurations in TOML format.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{McpConfig, McpServer};
use crate::error::LorumError;

/// Parse an MCP servers section from a TOML value into `McpConfig`.
///
/// `field` is the top-level key that holds the server table
/// (e.g. `"mcp_servers"`).
pub fn parse_mcp_servers_toml(value: &toml::Value, field: &str) -> McpConfig {
    let Some(servers) = value.get(field).and_then(|v| v.as_table()) else {
        return McpConfig::default();
    };

    let mut map = BTreeMap::new();
    for (name, entry) in servers {
        if let Some(server) = parse_mcp_server_toml(entry.as_table()) {
            map.insert(name.clone(), server);
        }
    }
    McpConfig { servers: map }
}

/// Parse a single MCP server entry from a TOML table.
pub fn parse_mcp_server_toml(
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Option<McpServer> {
    let table = table?;
    let command = table.get("command")?.as_str()?.to_string();
    let args = table
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let env = table
        .get("env")
        .and_then(|v| v.as_table())
        .map(|tbl| {
            tbl.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Some(McpServer { command, args, env })
}

/// Read an existing TOML file, returning an empty table if it doesn't exist.
pub fn read_existing_toml(path: &Path) -> Result<toml::Value, LorumError> {
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let contents = std::fs::read_to_string(path)?;
    toml::from_str(&contents).map_err(|e| LorumError::ConfigParse {
        format: "toml".into(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

/// Convert an `McpConfig` to a TOML `Value` (just the servers table).
pub fn mcp_config_to_toml_value(config: &McpConfig) -> toml::Value {
    let mut outer = toml::map::Map::new();
    for (name, server) in &config.servers {
        let mut server_table = toml::map::Map::new();
        server_table.insert(
            "command".into(),
            toml::Value::String(server.command.clone()),
        );
        server_table.insert(
            "args".into(),
            toml::Value::Array(
                server
                    .args
                    .iter()
                    .map(|a| toml::Value::String(a.clone()))
                    .collect(),
            ),
        );
        if !server.env.is_empty() {
            let env_table: toml::map::Map<String, toml::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                .collect();
            server_table.insert("env".into(), toml::Value::Table(env_table));
        }
        outer.insert(name.clone(), toml::Value::Table(server_table));
    }
    toml::Value::Table(outer)
}

/// Write a TOML value to a file, creating parent directories as needed.
pub fn write_toml(path: &Path, value: &toml::Value) -> Result<(), LorumError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    let formatted = toml::to_string_pretty(value).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    std::fs::write(path, formatted).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}
