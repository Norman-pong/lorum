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

/// Parse hooks from a JSON value with standard structure.
///
/// Input format: `{"event_name": [{"command": "...", matcher_key: "...", ...}]}`
///
/// `event_converter` converts tool event names to lorum kebab-case.
/// `matcher_key` is the JSON key for the matcher field (e.g. `"match"` for
/// Claude, `"matcher"` for Cursor). If the key is not found in a handler,
/// that handler is skipped.
///
/// Handlers without a `command` field are also skipped.
pub fn parse_hooks_from_json_value(
    value: Option<&serde_json::Value>,
    event_converter: impl Fn(&str) -> String,
    matcher_key: &str,
) -> crate::config::HooksConfig {
    use crate::config::{HookHandler, HooksConfig};

    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return HooksConfig::default();
    };
    let mut events = std::collections::BTreeMap::new();
    for (tool_event, handlers_value) in obj {
        let converted_event = event_converter(tool_event);
        let Some(handlers_array) = handlers_value.as_array() else {
            continue;
        };
        let mut handlers = Vec::new();
        for handler_value in handlers_array {
            let Some(handler_obj) = handler_value.as_object() else {
                continue;
            };
            let Some(matcher) = handler_obj
                .get(matcher_key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let Some(command) = handler_obj
                .get("command")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let timeout = handler_obj.get("timeout").and_then(|v| v.as_u64());
            let handler_type = handler_obj
                .get("type")
                .and_then(|v| v.as_str())
                .map(String::from);
            handlers.push(HookHandler {
                matcher: matcher.to_string(),
                command: command.to_string(),
                timeout,
                handler_type,
            });
        }
        if !handlers.is_empty() {
            events.insert(converted_event, handlers);
        }
    }
    HooksConfig { events }
}

/// Convert a `HooksConfig` to a JSON value with standard structure.
///
/// Output format: `{"event_name": [{matcher_key: "...", "command": "...", ...}]}`
///
/// `event_converter` converts lorum kebab-case event names to tool-specific
/// format. `matcher_key` is the JSON key used for the matcher field.
pub fn hooks_config_to_json_value(
    config: &crate::config::HooksConfig,
    event_converter: impl Fn(&str) -> String,
    matcher_key: &str,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (event_name, handlers) in &config.events {
        let tool_event = event_converter(event_name);
        let handlers_array: Vec<serde_json::Value> = handlers
            .iter()
            .map(|h| {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    matcher_key.into(),
                    serde_json::Value::String(h.matcher.clone()),
                );
                obj.insert(
                    "command".into(),
                    serde_json::Value::String(h.command.clone()),
                );
                if let Some(t) = h.timeout {
                    obj.insert("timeout".into(), serde_json::Value::Number(t.into()));
                }
                if let Some(ref ty) = h.handler_type {
                    obj.insert("type".into(), serde_json::Value::String(ty.clone()));
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        map.insert(tool_event, serde_json::Value::Array(handlers_array));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HookHandler, HooksConfig};

    fn identity(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn parse_hooks_from_json_value_empty_input() {
        let config = parse_hooks_from_json_value(None, identity, "matcher");
        assert!(config.events.is_empty());
    }

    #[test]
    fn parse_hooks_from_json_value_valid() {
        let json = serde_json::json!({
            "pre-tool-use": [
                {
                    "matcher": "Bash",
                    "command": "check.sh",
                    "timeout": 30,
                    "type": "command"
                }
            ]
        });
        let config = parse_hooks_from_json_value(Some(&json), identity, "matcher");
        assert_eq!(config.events.len(), 1);
        let handlers = &config.events["pre-tool-use"];
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].matcher, "Bash");
        assert_eq!(handlers[0].command, "check.sh");
        assert_eq!(handlers[0].timeout, Some(30));
        assert_eq!(handlers[0].handler_type, Some("command".to_string()));
    }

    #[test]
    fn parse_hooks_skips_handler_without_matcher() {
        let json = serde_json::json!({
            "event": [
                { "command": "ok.sh", "matcher": "ok" },
                { "command": "no-matcher.sh" }
            ]
        });
        let config = parse_hooks_from_json_value(Some(&json), identity, "matcher");
        let handlers = &config.events["event"];
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].command, "ok.sh");
    }

    #[test]
    fn parse_hooks_skips_handler_without_command() {
        let json = serde_json::json!({
            "event": [
                { "matcher": "ok", "command": "ok.sh" },
                { "matcher": "no-command" }
            ]
        });
        let config = parse_hooks_from_json_value(Some(&json), identity, "matcher");
        let handlers = &config.events["event"];
        assert_eq!(handlers.len(), 1);
    }

    #[test]
    fn parse_hooks_with_event_converter() {
        let json = serde_json::json!({
            "PreToolUse": [
                { "matcher": "Bash", "command": "check.sh" }
            ]
        });
        let to_kebab = |s: &str| {
            s.chars().fold(String::new(), |mut acc, c| {
                if c.is_uppercase() && !acc.is_empty() {
                    acc.push('-');
                }
                acc.push(c.to_ascii_lowercase());
                acc
            })
        };
        let config = parse_hooks_from_json_value(Some(&json), to_kebab, "matcher");
        assert!(config.events.contains_key("pre-tool-use"));
    }

    #[test]
    fn hooks_config_to_json_value_roundtrip() {
        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "Bash".into(),
                command: "check.sh".into(),
                timeout: Some(30),
                handler_type: Some("command".into()),
            }],
        );

        let value = hooks_config_to_json_value(&config, identity, "matcher");
        let parsed = parse_hooks_from_json_value(Some(&value), identity, "matcher");
        assert_eq!(parsed.events, config.events);
    }

    #[test]
    fn hooks_config_to_json_value_omits_optional_fields() {
        let mut config = HooksConfig::default();
        config.events.insert(
            "event".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "echo".into(),
                timeout: None,
                handler_type: None,
            }],
        );

        let value = hooks_config_to_json_value(&config, identity, "matcher");
        let obj = value.as_object().unwrap();
        let handlers = obj["event"].as_array().unwrap();
        let handler = handlers[0].as_object().unwrap();
        assert!(handler.contains_key("matcher"));
        assert!(handler.contains_key("command"));
        assert!(!handler.contains_key("timeout"));
        assert!(!handler.contains_key("type"));
    }

    #[test]
    fn hooks_config_with_custom_matcher_key() {
        let mut config = HooksConfig::default();
        config.events.insert(
            "event".into(),
            vec![HookHandler {
                matcher: "all".into(),
                command: "run.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );

        let value = hooks_config_to_json_value(&config, identity, "match");
        let obj = value.as_object().unwrap();
        let handlers = obj["event"].as_array().unwrap();
        let handler = handlers[0].as_object().unwrap();
        assert!(handler.contains_key("match"));
        assert!(!handler.contains_key("matcher"));
    }
}
