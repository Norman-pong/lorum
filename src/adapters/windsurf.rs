//! Windsurf adapter for reading/writing rules, hooks, and MCP configuration.
//!
//! Rules file: `{project_root}/.windsurfrules`
//!
//! Hooks file: `{project_root}/.windsurf/hooks.json` (project-level)
//!   and `~/.codeium/windsurf/hooks.json` (user-level)
//!
//! MCP configuration file: `~/.codeium/windsurf/mcp_config.json` (global)
//!
//! Format (JSON):
//! ```json
//! {
//!   "mcpServers": {
//!     "server-name": {
//!       "command": "npx",
//!       "args": ["-y", "some-pkg"],
//!       "env": { "KEY": "value" }
//!     }
//!   }
//! }
//! ```
//!
//! Note: Windsurf also supports SSE remote MCP servers via the `serverUrl`
//! field. These are skipped during read because lorum only synchronises
//! stdio-based MCPs.

use std::path::{Path, PathBuf};

use crate::adapters::{
    ConfigValidator, HooksAdapter, RulesAdapter, ToolAdapter, ValidationIssue,
    default_validate_config, json_utils, read_rules_file, write_rules_file,
};
use crate::config::{HooksConfig, McpConfig};
use crate::error::LorumError;

/// Adapter for Windsurf rules.
///
/// Reads and writes rules content from Windsurf's `.windsurfrules` file
/// located at the project root.
pub struct WindsurfRulesAdapter;

impl RulesAdapter for WindsurfRulesAdapter {
    fn name(&self) -> &str {
        "windsurf"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".windsurfrules")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        write_rules_file(&self.rules_path(project_root), content)
    }
}

/// Adapter for Windsurf MCP configuration and hooks.
///
/// Reads and writes MCP server configurations from Windsurf's
/// global `~/.codeium/windsurf/mcp_config.json` file, preserving any
/// non-MCP fields.
///
/// SSE remote MCP servers (entries with `serverUrl` instead of `command`)
/// are skipped during read with a warning.
///
/// Hooks are read/written from/to project-level `.windsurf/hooks.json`
/// with fallback to user-level `~/.codeium/windsurf/hooks.json`.
pub struct WindsurfAdapter {
    project_root: Option<PathBuf>,
}

/// Field name used by Windsurf for MCP servers.
const MCP_FIELD: &str = "mcpServers";

/// Returns the global Windsurf config path: `~/.codeium/windsurf/mcp_config.json`.
fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codeium").join("windsurf").join("mcp_config.json"))
}

impl WindsurfAdapter {
    /// Create a new adapter that uses the current working directory.
    pub fn new() -> Self {
        Self { project_root: None }
    }

    /// Create an adapter with an explicit project root.
    pub fn with_project_root(root: PathBuf) -> Self {
        Self {
            project_root: Some(root),
        }
    }

    /// Returns the project-level Windsurf hooks path: `.windsurf/hooks.json`.
    fn project_hooks_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".windsurf").join("hooks.json"))
    }

    /// Returns the user-level Windsurf hooks path: `~/.codeium/windsurf/hooks.json`.
    fn user_hooks_path(&self) -> Option<PathBuf> {
        Some(
            dirs::home_dir()?
                .join(".codeium")
                .join("windsurf")
                .join("hooks.json"),
        )
    }
}

impl Default for WindsurfAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksAdapter for WindsurfAdapter {
    fn name(&self) -> &str {
        "windsurf"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(p) = self.project_hooks_path() {
            paths.push(p);
        }
        if let Some(p) = self.user_hooks_path() {
            paths.push(p);
        }
        paths
    }

    fn read_hooks(&self) -> Result<HooksConfig, LorumError> {
        let paths = [self.project_hooks_path(), self.user_hooks_path()];
        for path in paths.into_iter().flatten() {
            if path.exists() {
                let root = json_utils::read_existing_json(&path)?;
                return Ok(parse_windsurf_hooks(root.get("hooks")));
            }
        }
        Ok(HooksConfig::default())
    }

    fn write_hooks(&self, config: &HooksConfig) -> Result<(), LorumError> {
        let path = match self.project_hooks_path() {
            Some(p) => p,
            None => match self.user_hooks_path() {
                Some(p) => p,
                None => {
                    return Err(LorumError::Other {
                        message: "cannot determine hooks directory".into(),
                    });
                }
            },
        };

        let mut root = json_utils::read_existing_json(&path)?;
        let existing_hooks = root.get("hooks").and_then(|v| v.as_object()).cloned();

        let mut hooks_map = serde_json::Map::new();
        for (lorum_event, handlers) in &config.events {
            let tool_event = match lorum_to_windsurf_event(lorum_event) {
                Some(e) => e,
                None => continue,
            };

            let existing_handlers = existing_hooks
                .as_ref()
                .and_then(|h| h.get(&tool_event))
                .and_then(|v| v.as_array());

            let mut handlers_array = Vec::new();
            for (i, handler) in handlers.iter().enumerate() {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "command".into(),
                    serde_json::Value::String(handler.command.clone()),
                );

                // Preserve extra fields from existing handler at same position.
                if let Some(existing_array) = existing_handlers {
                    if let Some(existing_handler) =
                        existing_array.get(i).and_then(|v| v.as_object())
                    {
                        for (key, value) in existing_handler {
                            if key != "command" && key != "timeout" && key != "type" {
                                obj.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }

                if let Some(t) = handler.timeout {
                    obj.insert("timeout".into(), serde_json::Value::Number(t.into()));
                }
                if let Some(ref ty) = handler.handler_type {
                    obj.insert("type".into(), serde_json::Value::String(ty.clone()));
                }
                handlers_array.push(serde_json::Value::Object(obj));
            }
            hooks_map.insert(tool_event, serde_json::Value::Array(handlers_array));
        }
        root["hooks"] = serde_json::Value::Object(hooks_map);
        json_utils::write_json(&path, &root)
    }

    fn lorum_to_tool_event(&self, lorum_event: &str) -> Option<String> {
        lorum_to_windsurf_event(lorum_event)
    }

    fn tool_to_lorum_event(&self, tool_event: &str) -> Option<String> {
        windsurf_to_lorum_event(tool_event)
    }
}

/// Convert a lorum kebab-case event name to Windsurf's snake_case format.
fn lorum_to_windsurf_event(lorum_event: &str) -> Option<String> {
    match lorum_event {
        "pre-tool-use" => Some("pre_mcp_tool_use".into()),
        "post-tool-use" => Some("post_mcp_tool_use".into()),
        "pre-read-file" => Some("pre_read_code".into()),
        "post-file-edit" => Some("post_write_code".into()),
        "session-start" | "session-end" => None,
        other => Some(other.replace('-', "_")),
    }
}

/// Convert a Windsurf snake_case event name to lorum's kebab-case format.
fn windsurf_to_lorum_event(tool_event: &str) -> Option<String> {
    match tool_event {
        "pre_mcp_tool_use" => Some("pre-tool-use".into()),
        "post_mcp_tool_use" => Some("post-tool-use".into()),
        "pre_read_code" => Some("pre-read-file".into()),
        "post_write_code" => Some("post-file-edit".into()),
        other => Some(other.replace('_', "-")),
    }
}

/// Parse hooks from a JSON value (Windsurf has no matcher field).
fn parse_windsurf_hooks(value: Option<&serde_json::Value>) -> HooksConfig {
    use crate::config::HookHandler;

    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return HooksConfig::default();
    };
    let mut events = std::collections::BTreeMap::new();
    for (tool_event, handlers_value) in obj {
        let converted_event =
            windsurf_to_lorum_event(tool_event).unwrap_or_else(|| tool_event.replace('_', "-"));
        let Some(handlers_array) = handlers_value.as_array() else {
            continue;
        };
        let mut handlers = Vec::new();
        for handler_value in handlers_array {
            let Some(handler_obj) = handler_value.as_object() else {
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
                matcher: "*".to_string(),
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

impl ConfigValidator for WindsurfAdapter {
    fn name(&self) -> &str {
        "windsurf"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        default_validate_config(self)
    }
}

impl ToolAdapter for WindsurfAdapter {
    fn name(&self) -> &str {
        "windsurf"
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
        let root = json_utils::read_existing_json(&path)?;

        let Some(servers) = root.get(MCP_FIELD).and_then(|v| v.as_object()) else {
            return Ok(McpConfig::default());
        };

        let mut map = std::collections::BTreeMap::new();
        for (name, entry) in servers {
            if entry.get("serverUrl").is_some() {
                eprintln!(
                    "warning: skipping SSE remote MCP server '{}' in windsurf",
                    name
                );
                continue;
            }
            if let Some(server) = json_utils::parse_mcp_server(entry) {
                map.insert(name.clone(), server);
            } else {
                eprintln!(
                    "warning: skipping invalid MCP server '{}' in windsurf",
                    name
                );
            }
        }
        Ok(McpConfig { servers: map })
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
        let mut root = json_utils::read_existing_json(&path)?;
        root[MCP_FIELD] = json_utils::mcp_config_to_json_value(config);
        json_utils::write_json(&path, &root)
    }
}

#[cfg(test)]
mod windsurf_rules_tests {
    use super::*;

    #[test]
    fn rules_path_returns_windsurfrules() {
        let adapter = WindsurfRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/.windsurfrules"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfRulesAdapter;
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
        let adapter = WindsurfRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn rules_adapter_name() {
        let adapter = WindsurfRulesAdapter;
        assert_eq!(adapter.name(), "windsurf");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_utils::make_server;
    use crate::config::HookHandler;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    #[serial_test::serial]
    fn read_mcp_from_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir
            .path()
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let json = r#"{
            "mcpServers": {
                "test-server": {
                    "command": "npx",
                    "args": ["-y", "some-pkg"],
                    "env": { "KEY": "value" }
                }
            },
            "otherField": true
        }"#;
        fs::write(&path, json).unwrap();

        let adapter = WindsurfAdapter::new();
        let config = adapter.read_mcp().unwrap();

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    #[serial_test::serial]
    fn read_mcp_skips_sse_remote_servers() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir
            .path()
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let json = r#"{
            "mcpServers": {
                "local-server": {
                    "command": "npx",
                    "args": ["-y", "some-pkg"]
                },
                "remote-server": {
                    "serverUrl": "http://localhost:3000/sse"
                }
            }
        }"#;
        fs::write(&path, json).unwrap();

        let adapter = WindsurfAdapter::new();
        let config = adapter.read_mcp().unwrap();
        assert_eq!(config.servers.len(), 1);
        assert!(config.servers.contains_key("local-server"));
        assert!(!config.servers.contains_key("remote-server"));
    }

    #[test]
    fn read_mcp_empty_when_no_field() {
        let root: serde_json::Value = serde_json::json!({ "otherField": true });
        let config = json_utils::parse_mcp_servers(&root, MCP_FIELD);
        assert!(config.servers.is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn write_mcp_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir
            .path()
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let original = r#"{"otherField": true, "mcpServers": {}}"#;
        fs::write(&path, original).unwrap();

        let adapter = WindsurfAdapter::new();
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("svr".into(), make_server("cmd", &["a"], &[("K", "V")]));
                m
            },
        };
        adapter.write_mcp(&config).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["otherField"], true);
        assert_eq!(result["mcpServers"]["svr"]["command"], "cmd");
    }

    #[test]
    #[serial_test::serial]
    fn write_mcp_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir
            .path()
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json");
        assert!(!path.exists());

        let adapter = WindsurfAdapter::new();
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        adapter.write_mcp(&config).unwrap();

        assert!(path.exists());
        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["mcpServers"]["s"]["command"], "c");
    }

    #[test]
    fn roundtrip_json() {
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
        let json_val = json_utils::mcp_config_to_json_value(&config);
        let wrapped = serde_json::json!({ "mcpServers": json_val });
        let parsed = json_utils::parse_mcp_servers(&wrapped, MCP_FIELD);
        assert_eq!(config, parsed);
    }

    #[test]
    fn adapter_name() {
        let adapter = WindsurfAdapter::new();
        assert_eq!(ToolAdapter::name(&adapter), "windsurf");
    }

    // -----------------------------------------------------------------------
    // Hooks tests
    // -----------------------------------------------------------------------

    #[test]
    fn windsurf_hooks_event_mapping() {
        let adapter = WindsurfAdapter::new();

        // lorum -> windsurf
        assert_eq!(
            adapter.lorum_to_tool_event("pre-tool-use"),
            Some("pre_mcp_tool_use".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("post-tool-use"),
            Some("post_mcp_tool_use".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("pre-read-file"),
            Some("pre_read_code".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("post-file-edit"),
            Some("post_write_code".into())
        );
        assert_eq!(adapter.lorum_to_tool_event("session-start"), None);
        assert_eq!(adapter.lorum_to_tool_event("session-end"), None);

        // windsurf -> lorum
        assert_eq!(
            adapter.tool_to_lorum_event("pre_mcp_tool_use"),
            Some("pre-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("post_mcp_tool_use"),
            Some("post-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("pre_read_code"),
            Some("pre-read-file".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("post_write_code"),
            Some("post-file-edit".into())
        );

        // Roundtrip
        assert_eq!(
            adapter.tool_to_lorum_event(&adapter.lorum_to_tool_event("pre-tool-use").unwrap()),
            Some("pre-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event(&adapter.lorum_to_tool_event("pre-read-file").unwrap()),
            Some("pre-read-file".into())
        );

        // Unknown event fallback
        assert_eq!(
            adapter.lorum_to_tool_event("custom-event"),
            Some("custom_event".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("custom_event"),
            Some("custom-event".into())
        );
    }

    #[test]
    fn windsurf_hooks_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_project_root(dir.path().to_path_buf());
        let windsurf_dir = dir.path().join(".windsurf");
        fs::create_dir_all(&windsurf_dir).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "echo hello".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        config.events.insert(
            "post-file-edit".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "notify.sh".into(),
                timeout: Some(30),
                handler_type: Some("command".into()),
            }],
        );

        adapter.write_hooks(&config).unwrap();

        let read = adapter.read_hooks().unwrap();
        assert_eq!(read.events.len(), 2);
        let pre_handlers = &read.events["pre-tool-use"];
        assert_eq!(pre_handlers.len(), 1);
        assert_eq!(pre_handlers[0].command, "echo hello");
        assert_eq!(pre_handlers[0].matcher, "*");
        let post_handlers = &read.events["post-file-edit"];
        assert_eq!(post_handlers.len(), 1);
        assert_eq!(post_handlers[0].command, "notify.sh");
        assert_eq!(post_handlers[0].timeout, Some(30));
        assert_eq!(post_handlers[0].handler_type, Some("command".into()));
    }

    #[test]
    fn windsurf_hooks_preserves_extra_fields() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_project_root(dir.path().to_path_buf());
        let windsurf_dir = dir.path().join(".windsurf");
        fs::create_dir_all(&windsurf_dir).unwrap();
        let path = windsurf_dir.join("hooks.json");

        let original = serde_json::json!({
            "hooks": {
                "pre_mcp_tool_use": [
                    {
                        "command": "echo hello",
                        "show_output": true,
                        "working_directory": "/tmp"
                    }
                ]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: String::new(),
                command: "echo hello".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        adapter.write_hooks(&config).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let handler = &result["hooks"]["pre_mcp_tool_use"][0];
        assert_eq!(handler["command"], "echo hello");
        assert_eq!(handler["show_output"], true);
        assert_eq!(handler["working_directory"], "/tmp");
    }

    #[test]
    fn windsurf_hooks_skips_unsupported_events() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_project_root(dir.path().to_path_buf());
        let windsurf_dir = dir.path().join(".windsurf");
        fs::create_dir_all(&windsurf_dir).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: String::new(),
                command: "check.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        config.events.insert(
            "session-start".into(),
            vec![HookHandler {
                matcher: String::new(),
                command: "start.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );

        adapter.write_hooks(&config).unwrap();

        let result: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(adapter.project_hooks_path().unwrap()).unwrap(),
        )
        .unwrap();
        assert!(result["hooks"]["pre_mcp_tool_use"].is_array());
        assert!(result["hooks"]["session_start"].is_null());
    }

    #[test]
    fn windsurf_hooks_unknown_events_preserved_on_read() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_project_root(dir.path().to_path_buf());
        let windsurf_dir = dir.path().join(".windsurf");
        fs::create_dir_all(&windsurf_dir).unwrap();
        let path = windsurf_dir.join("hooks.json");

        let json = serde_json::json!({
            "hooks": {
                "custom_event": [
                    { "command": "run.sh" }
                ]
            }
        });
        fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

        let config = adapter.read_hooks().unwrap();
        assert!(config.events.contains_key("custom-event"));
        let handlers = &config.events["custom-event"];
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].command, "run.sh");
    }

    #[test]
    fn windsurf_hooks_config_paths() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_project_root(dir.path().to_path_buf());
        let paths = HooksAdapter::config_paths(&adapter);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], dir.path().join(".windsurf").join("hooks.json"));
        assert!(paths[1].ends_with(".codeium/windsurf/hooks.json"));
    }

    #[test]
    fn windsurf_hooks_reads_user_level_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let adapter =
            WindsurfAdapter::with_project_root(dir.path().join("nonexistent").to_path_buf());
        let result = adapter.read_hooks().unwrap();
        assert!(result.events.is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn windsurf_hooks_reads_user_level_file() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let adapter =
            WindsurfAdapter::with_project_root(dir.path().join("nonexistent").to_path_buf());
        let path = dir
            .path()
            .join(".codeium")
            .join("windsurf")
            .join("hooks.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let json = serde_json::json!({
            "hooks": {
                "pre_mcp_tool_use": [
                    { "command": "user-level.sh" }
                ]
            }
        });
        fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

        let config = adapter.read_hooks().unwrap();
        assert_eq!(config.events.len(), 1);
        assert_eq!(config.events["pre-tool-use"][0].command, "user-level.sh");
    }
}
