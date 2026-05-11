//! Shared JSON utilities for adapters that use JSON config files.
//!
//! Provides parsing, serialization, reading, and writing of MCP server
//! configurations in JSON format.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{McpConfig, McpServer};
use crate::error::LorumError;

/// Parse an MCP servers field from a JSON value into `McpConfig`.
///
/// `field` is the top-level key that holds the server map
/// (e.g. `"mcpServers"` or `"servers"`).
pub fn parse_mcp_servers(value: &serde_json::Value, field: &str) -> McpConfig {
    let Some(servers) = value.get(field).and_then(|v| v.as_object()) else {
        return McpConfig::default();
    };

    let mut map = BTreeMap::new();
    for (name, entry) in servers {
        if let Some(server) = parse_mcp_server(entry) {
            map.insert(name.clone(), server);
        }
    }
    McpConfig { servers: map }
}

/// Parse a single MCP server entry from a JSON value.
pub fn parse_mcp_server(value: &serde_json::Value) -> Option<McpServer> {
    let obj = value.as_object()?;
    let command = obj.get("command")?.as_str()?.to_string();
    let args = obj
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let env = obj
        .get("env")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Some(McpServer { command, args, env })
}

/// Read an existing JSON file, returning an empty object if it doesn't exist.
pub fn read_existing_json(path: &Path) -> Result<serde_json::Value, LorumError> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    let contents = std::fs::read_to_string(path)?;
    serde_json::from_str(&contents).map_err(|e| LorumError::ConfigParse {
        format: "json".into(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

/// Convert an `McpConfig` to a JSON `Value`.
pub fn mcp_config_to_json_value(config: &McpConfig) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (name, server) in &config.servers {
        let mut server_obj = serde_json::Map::new();
        server_obj.insert(
            "command".into(),
            serde_json::Value::String(server.command.clone()),
        );
        server_obj.insert(
            "args".into(),
            serde_json::Value::Array(
                server
                    .args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect(),
            ),
        );
        if !server.env.is_empty() {
            let env_obj: serde_json::Map<String, serde_json::Value> = server
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            server_obj.insert("env".into(), serde_json::Value::Object(env_obj));
        }
        map.insert(name.clone(), serde_json::Value::Object(server_obj));
    }
    serde_json::Value::Object(map)
}

/// Write a JSON value to a file, creating parent directories as needed.
pub fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), LorumError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    let formatted = serde_json::to_string_pretty(value).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    std::fs::write(path, formatted).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}
