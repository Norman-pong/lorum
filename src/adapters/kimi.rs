//! Kimi adapter for reading/writing MCP and hooks configuration.
//!
//! Configuration file: `~/.kimi/config.toml` (global)
//!
//! MCP format (TOML):
//! ```toml
//! [mcp.client.server-name]
//! command = "npx"
//! args = ["-y", "some-pkg"]
//!
//! [mcp.client.server-name.env]
//! KEY = "value"
//! ```
//!
//! Hooks format (TOML):
//! ```toml
//! [[hooks]]
//! event = "PreToolUse"
//! matcher = "Shell"
//! command = ".kimi/hooks/safety-check.sh"
//! timeout = 10
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{
    ConfigValidator, HooksAdapter, RulesAdapter, Severity, ToolAdapter, ValidationIssue,
    kebab_to_pascal, pascal_to_kebab, read_rules_file, toml_utils, validate_all_syntax,
    write_rules_file,
};
use crate::config::{HookHandler, HooksConfig, McpConfig};
use crate::error::LorumError;

/// Adapter for Kimi rules.
///
/// Reads and writes rules content from Kimi's `AGENTS.md`
/// file located at the project root.
pub struct KimiRulesAdapter;

impl RulesAdapter for KimiRulesAdapter {
    fn name(&self) -> &str {
        "kimi"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join("AGENTS.md")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        write_rules_file(&self.rules_path(project_root), content)
    }
}

/// Adapter for Kimi.
///
/// Reads and writes MCP server configurations from Kimi's
/// `~/.kimi/config.toml` file under the `[mcp.client]` section,
/// preserving any non-MCP fields.
pub struct KimiAdapter;

/// Top-level TOML key for the mcp section.
const MCP_TOP: &str = "mcp";
/// Nested key under `mcp` for client (server definitions).
const MCP_CLIENT: &str = "client";

/// Returns the global Kimi config path: `~/.kimi/config.toml`.
fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".kimi").join("config.toml"))
}

impl HooksAdapter for KimiAdapter {
    fn name(&self) -> &str {
        "kimi"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        global_config_path().into_iter().collect()
    }

    fn read_hooks(&self) -> Result<HooksConfig, LorumError> {
        let path = match global_config_path() {
            Some(p) => p,
            None => return Ok(HooksConfig::default()),
        };
        if !path.exists() {
            return Ok(HooksConfig::default());
        }
        let root = toml_utils::read_existing_toml(&path)?;
        Ok(parse_hooks_from_toml(&root))
    }

    fn write_hooks(&self, config: &HooksConfig) -> Result<(), LorumError> {
        let path = match global_config_path() {
            Some(p) => p,
            None => {
                return Err(LorumError::Other {
                    message: "cannot determine home directory".into(),
                });
            }
        };
        let mut root = toml_utils::read_existing_toml(&path)?;
        let hooks_array = hooks_config_to_toml_array(config);

        let root_table = root.as_table_mut().ok_or_else(|| LorumError::Other {
            message: format!("expected table at root of {}", path.display()),
        })?;
        root_table.insert("hooks".into(), toml::Value::Array(hooks_array));

        toml_utils::write_toml(&path, &root)
    }

    fn lorum_to_tool_event(&self, lorum_event: &str) -> Option<String> {
        Some(kebab_to_pascal(lorum_event))
    }

    fn tool_to_lorum_event(&self, tool_event: &str) -> Option<String> {
        Some(pascal_to_kebab(tool_event))
    }
}

impl ConfigValidator for KimiAdapter {
    fn name(&self) -> &str {
        "kimi"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        // 1. Run default syntax validation (TOML)
        let mut issues = validate_all_syntax(&<Self as ToolAdapter>::config_paths(self));

        // 2. Extra check: validate [mcp.client] field structure
        if let Some(ref path) = global_config_path() {
            if path.exists() {
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            message: format!("failed to read file: {e}"),
                            path: Some(path.clone()),
                            line: None,
                        });
                        return Ok(issues);
                    }
                };

                let root: toml::Value = match toml::from_str(&content) {
                    Ok(v) => v,
                    Err(_) => {
                        // Syntax errors already reported by validate_all_syntax
                        return Ok(issues);
                    }
                };

                if let Some(client) = root
                    .get(MCP_TOP)
                    .and_then(|v| v.get(MCP_CLIENT))
                    .and_then(|v| v.as_table())
                {
                    for (server_name, server_value) in client {
                        if let Some(server_table) = server_value.as_table() {
                            // Check for required `command` field
                            if !server_table.contains_key("command") {
                                issues.push(ValidationIssue {
                                    severity: Severity::Warning,
                                    message: format!(
                                        "MCP server '{}' is missing required 'command' field",
                                        server_name
                                    ),
                                    path: Some(path.clone()),
                                    line: None,
                                });
                            }

                            // Check `args` is an array if present
                            if let Some(args) = server_table.get("args") {
                                if !args.is_array() {
                                    issues.push(ValidationIssue {
                                        severity: Severity::Warning,
                                        message: format!(
                                            "MCP server '{}' has 'args' that is not an array",
                                            server_name
                                        ),
                                        path: Some(path.clone()),
                                        line: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(issues)
    }
}

impl ToolAdapter for KimiAdapter {
    fn name(&self) -> &str {
        "kimi"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        global_config_path().into_iter().collect()
    }

    fn read_mcp(&self) -> Result<McpConfig, LorumError> {
        let path = match global_config_path() {
            Some(p) => p,
            None => return Ok(McpConfig::default()),
        };
        if !path.exists() {
            return Ok(McpConfig::default());
        }
        let root = toml_utils::read_existing_toml(&path)?;
        Ok(parse_mcp_client(&root))
    }

    fn write_mcp(&self, config: &McpConfig) -> Result<(), LorumError> {
        let path = match global_config_path() {
            Some(p) => p,
            None => {
                return Err(LorumError::Other {
                    message: "cannot determine home directory".into(),
                });
            }
        };
        let mut root = toml_utils::read_existing_toml(&path)?;
        let client_table = toml_utils::mcp_config_to_toml_value(config);

        let root_table = root.as_table_mut().ok_or_else(|| LorumError::Other {
            message: format!("expected table at root of {}", path.display()),
        })?;
        let mcp_entry = root_table
            .entry(MCP_TOP)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        mcp_entry
            .as_table_mut()
            .ok_or_else(|| LorumError::Other {
                message: format!("expected table for '{}' at {}", MCP_TOP, path.display()),
            })?
            .insert(MCP_CLIENT.into(), client_table);

        toml_utils::write_toml(&path, &root)
    }
}

/// Parse the `mcp.client` section from a TOML value into `McpConfig`.
fn parse_mcp_client(root: &toml::Value) -> McpConfig {
    let Some(servers) = root
        .get(MCP_TOP)
        .and_then(|v| v.get(MCP_CLIENT))
        .and_then(|v| v.as_table())
    else {
        return McpConfig::default();
    };

    let mut map = std::collections::BTreeMap::new();
    for (name, value) in servers {
        if let Some(server) = toml_utils::parse_mcp_server_toml(value.as_table()) {
            map.insert(name.clone(), server);
        }
    }
    McpConfig { servers: map }
}

/// Parse hooks from a TOML value.
fn parse_hooks_from_toml(root: &toml::Value) -> HooksConfig {
    let Some(hooks_array) = root.get("hooks").and_then(|v| v.as_array()) else {
        return HooksConfig::default();
    };
    let mut events: BTreeMap<String, Vec<HookHandler>> = BTreeMap::new();
    for entry in hooks_array {
        let Some(table) = entry.as_table() else {
            continue;
        };
        let Some(pascal_event) = table
            .get("event")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let kebab_event = pascal_to_kebab(pascal_event);
        let Some(matcher) = table
            .get("matcher")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let Some(command) = table
            .get("command")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let timeout = table
            .get("timeout")
            .and_then(|v| v.as_integer())
            .and_then(|v| u64::try_from(v).ok());
        let handler_type = table.get("type").and_then(|v| v.as_str()).map(String::from);
        events.entry(kebab_event).or_default().push(HookHandler {
            matcher: matcher.to_string(),
            command: command.to_string(),
            timeout,
            handler_type,
        });
    }
    HooksConfig { events }
}

/// Convert a HooksConfig to a TOML array of hook tables.
fn hooks_config_to_toml_array(config: &HooksConfig) -> Vec<toml::Value> {
    let mut array = Vec::new();
    for (event_name, handlers) in &config.events {
        let pascal_event = kebab_to_pascal(event_name);
        for h in handlers {
            let mut table = toml::map::Map::new();
            table.insert("event".into(), toml::Value::String(pascal_event.clone()));
            table.insert("matcher".into(), toml::Value::String(h.matcher.clone()));
            table.insert("command".into(), toml::Value::String(h.command.clone()));
            if let Some(t) = h.timeout {
                table.insert("timeout".into(), toml::Value::Integer(t as i64));
            }
            if let Some(ref ty) = h.handler_type {
                table.insert("type".into(), toml::Value::String(ty.clone()));
            }
            array.push(toml::Value::Table(table));
        }
    }
    array
}

#[cfg(test)]
mod kimi_rules_tests {
    use super::*;

    #[test]
    fn rules_path_returns_agents_md() {
        let adapter = KimiRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/AGENTS.md"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = KimiRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = KimiRulesAdapter;
        let path = adapter.rules_path(dir.path());
        assert!(!path.exists());

        adapter
            .write_rules(dir.path(), "Use 4-space indentation.")
            .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = KimiRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn rules_adapter_name() {
        let adapter = KimiRulesAdapter;
        assert_eq!(adapter.name(), "kimi");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_utils::make_server;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn read_mcp_from_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let toml_str = r#"
other_field = true

[mcp.client.test-server]
command = "npx"
args = ["-y", "some-pkg"]

[mcp.client.test-server.env]
KEY = "value"
"#;
        fs::write(&path, toml_str).unwrap();

        let root: toml::Value = toml::from_str(toml_str).unwrap();
        let config = parse_mcp_client(&root);

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    fn read_mcp_empty_when_no_field() {
        let root: toml::Value = toml::from_str("other = true").unwrap();
        let config = parse_mcp_client(&root);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn write_mcp_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let original = r#"other_field = true
[mcp]
"#;
        fs::write(&path, original).unwrap();

        let mut root = toml_utils::read_existing_toml(&path).unwrap();
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("svr".into(), make_server("cmd", &["a"], &[("K", "V")]));
                m
            },
        };

        let client_table = toml_utils::mcp_config_to_toml_value(&config);
        let root_table = root.as_table_mut().unwrap();
        let mcp_entry = root_table
            .entry(MCP_TOP)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        mcp_entry
            .as_table_mut()
            .unwrap()
            .insert(MCP_CLIENT.into(), client_table);
        toml_utils::write_toml(&path, &root).unwrap();

        let result: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["other_field"].as_bool(), Some(true));
        assert_eq!(
            result["mcp"]["client"]["svr"]["command"].as_str(),
            Some("cmd")
        );
    }

    #[test]
    fn write_mcp_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("config.toml");
        assert!(!path.exists());

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let mut root = toml::Value::Table(toml::map::Map::new());
        let client_table = toml_utils::mcp_config_to_toml_value(&config);
        root.as_table_mut()
            .unwrap()
            .entry(MCP_TOP)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .unwrap()
            .insert(MCP_CLIENT.into(), client_table);
        toml_utils::write_toml(&path, &root).unwrap();

        assert!(path.exists());
        let result: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["mcp"]["client"]["s"]["command"].as_str(), Some("c"));
    }

    #[test]
    fn roundtrip_toml() {
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "a".into(),
                    make_server("node", &["index.js"], &[("PORT", "3000")]),
                );
                m.insert("b".into(), make_server("python", &["main.py"], &[]));
                m
            },
        };
        let toml_val = toml_utils::mcp_config_to_toml_value(&config);
        let mut outer = toml::map::Map::new();
        let mut mcp_table = toml::map::Map::new();
        mcp_table.insert(MCP_CLIENT.into(), toml_val);
        outer.insert(MCP_TOP.into(), toml::Value::Table(mcp_table));
        let parsed = parse_mcp_client(&toml::Value::Table(outer));
        assert_eq!(config, parsed);
    }

    #[test]
    fn adapter_name() {
        let adapter = KimiAdapter;
        assert_eq!(ToolAdapter::name(&adapter), "kimi");
    }

    // --- hooks -------------------------------------------------------------

    #[test]
    fn parse_hooks_from_valid_toml() {
        let toml_str = r#"
[[hooks]]
event = "PreToolUse"
matcher = "Bash"
command = "scripts/check.sh"
timeout = 60

[[hooks]]
event = "PreToolUse"
matcher = "Write"
command = "scripts/write-check.sh"

[[hooks]]
event = "PostToolUse"
matcher = "Edit"
command = "cargo fmt"
"#;
        let root: toml::Value = toml::from_str(toml_str).unwrap();
        let config = parse_hooks_from_toml(&root);
        assert_eq!(config.events.len(), 2);
        let pre = &config.events["pre-tool-use"];
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0].matcher, "Bash");
        assert_eq!(pre[0].timeout, Some(60));
        assert_eq!(pre[1].matcher, "Write");
        assert_eq!(pre[1].timeout, None);
        let post = &config.events["post-tool-use"];
        assert_eq!(post.len(), 1);
        assert_eq!(post[0].matcher, "Edit");
    }

    #[test]
    fn parse_hooks_empty_when_no_field() {
        let root: toml::Value = toml::from_str("other = true").unwrap();
        let config = parse_hooks_from_toml(&root);
        assert!(config.events.is_empty());
    }

    #[test]
    fn write_hooks_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "other_field = true\n").unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "Shell".into(),
                command: "check.sh".into(),
                timeout: Some(10),
                handler_type: None,
            }],
        );

        // Test via helper functions (adapter uses global path).
        let mut root = toml_utils::read_existing_toml(&path).unwrap();
        let hooks_array = hooks_config_to_toml_array(&config);
        root.as_table_mut()
            .unwrap()
            .insert("hooks".into(), toml::Value::Array(hooks_array));
        toml_utils::write_toml(&path, &root).unwrap();

        let result: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["other_field"].as_bool(), Some(true));
        let hooks = result["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["event"].as_str(), Some("PreToolUse"));
        assert_eq!(hooks[0]["matcher"].as_str(), Some("Shell"));
        assert_eq!(hooks[0]["timeout"].as_integer(), Some(10));
    }

    #[test]
    fn hooks_roundtrip_toml() {
        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "Bash".into(),
                command: "check.sh".into(),
                timeout: Some(60),
                handler_type: Some("command".into()),
            }],
        );
        let array = hooks_config_to_toml_array(&config);
        let mut root = toml::map::Map::new();
        root.insert("hooks".into(), toml::Value::Array(array));
        let parsed = parse_hooks_from_toml(&toml::Value::Table(root));
        assert_eq!(config, parsed);
    }
}
