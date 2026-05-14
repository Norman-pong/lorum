//! Codex adapter for reading/writing MCP configuration.
//!
//! Configuration file: `~/.codex/config.toml` (global)
//!
//! Format (TOML):
//! ```toml
//! [mcp_servers.server-name]
//! command = "npx"
//! args = ["-y", "some-pkg"]
//!
//! [mcp_servers.server-name.env]
//! KEY = "value"
//! ```
//!
//! Hooks files: `{project_root}/.codex/hooks.json` (project-level)
//!   and `~/.codex/hooks.json` (user-level)
//!
//! Hooks format (JSON):
//! ```json
//! {
//!   "hooks": {
//!     "EventName": [
//!       {
//!         "hooks": [
//!           {
//!             "type": "command",
//!             "command": "echo hello",
//!             "timeout": 30
//!           }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```

use std::path::{Path, PathBuf};

use crate::adapters::{
    ConfigValidator, HooksAdapter, RulesAdapter, ToolAdapter, ValidationIssue,
    default_validate_config, json_utils, kebab_to_pascal, pascal_to_kebab, toml_utils,
};
use crate::config::{HookHandler, HooksConfig, McpConfig};
use crate::error::LorumError;

/// Adapter for Codex.
///
/// Reads and writes MCP server configurations from Codex's
/// `~/.codex/config.toml` file, preserving any non-MCP fields.
pub struct CodexAdapter {
    project_root: Option<PathBuf>,
}

/// Top-level TOML key used by Codex for MCP servers.
const MCP_FIELD: &str = "mcp_servers";

/// Returns the global Codex config path: `~/.codex/config.toml`.
fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("config.toml"))
}

impl CodexAdapter {
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

    /// Returns the project-level Codex hooks path: `.codex/hooks.json`.
    fn project_hooks_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".codex").join("hooks.json"))
    }

    /// Returns the user-level Codex hooks path: `~/.codex/hooks.json`.
    fn user_hooks_path(&self) -> Option<PathBuf> {
        Some(dirs::home_dir()?.join(".codex").join("hooks.json"))
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
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
                return Ok(parse_codex_hooks_from_json(root.get("hooks")));
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
        root["hooks"] = codex_hooks_config_to_json_value(config);
        json_utils::write_json(&path, &root)
    }

    fn lorum_to_tool_event(&self, lorum_event: &str) -> Option<String> {
        match lorum_event {
            "pre-tool-use" => Some("PreToolUse".into()),
            "post-tool-use" => Some("PostToolUse".into()),
            "session-end" => Some("Stop".into()),
            _ => None,
        }
    }

    fn tool_to_lorum_event(&self, tool_event: &str) -> Option<String> {
        match tool_event {
            "PreToolUse" => Some("pre-tool-use".into()),
            "PostToolUse" => Some("post-tool-use".into()),
            "Stop" => Some("session-end".into()),
            _ => None,
        }
    }
}

impl ConfigValidator for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        default_validate_config(self)
    }
}

impl ToolAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
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
        Ok(toml_utils::parse_mcp_servers_toml(&root, MCP_FIELD))
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
        let root_table = root.as_table_mut().ok_or_else(|| LorumError::Other {
            message: format!("expected table at root of {}", path.display()),
        })?;
        root_table.insert(
            MCP_FIELD.into(),
            toml_utils::mcp_config_to_toml_value(config),
        );
        toml_utils::write_toml(&path, &root)
    }
}

/// Adapter for Codex rules files.
///
/// Reads and writes rules content from Codex's `.codex/rules.md` file
/// located at the project root.
pub struct CodexRulesAdapter;

impl RulesAdapter for CodexRulesAdapter {
    fn name(&self) -> &str {
        "codex"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".codex").join("rules.md")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        crate::adapters::read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        crate::adapters::write_rules_file(&self.rules_path(project_root), content)
    }
}

/// Convert a Codex event name to a lorum kebab-case event name.
///
/// Known events are mapped explicitly; unknown events fall back to
/// `pascal_to_kebab` so they are preserved on read.
fn codex_event_to_lorum(tool_event: &str) -> String {
    match tool_event {
        "PreToolUse" => "pre-tool-use".to_string(),
        "PostToolUse" => "post-tool-use".to_string(),
        "Stop" => "session-end".to_string(),
        _ => pascal_to_kebab(tool_event),
    }
}

/// Convert a lorum kebab-case event name to a Codex event name.
///
/// Known events are mapped explicitly; unknown events fall back to
/// `kebab_to_pascal`.
fn lorum_event_to_codex(lorum_event: &str) -> String {
    match lorum_event {
        "pre-tool-use" => "PreToolUse".to_string(),
        "post-tool-use" => "PostToolUse".to_string(),
        "session-end" => "Stop".to_string(),
        _ => kebab_to_pascal(lorum_event),
    }
}

/// Parse hooks from Codex's nested JSON structure.
fn parse_codex_hooks_from_json(value: Option<&serde_json::Value>) -> HooksConfig {
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return HooksConfig::default();
    };
    let mut events = std::collections::BTreeMap::new();
    for (tool_event, handlers_value) in obj {
        let lorum_event = codex_event_to_lorum(tool_event);
        let Some(outer_array) = handlers_value.as_array() else {
            continue;
        };
        let mut handlers = Vec::new();
        for outer_item in outer_array {
            let Some(inner_hooks) = outer_item.get("hooks").and_then(|v| v.as_array()) else {
                continue;
            };
            for handler_value in inner_hooks {
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
        }
        if !handlers.is_empty() {
            events.insert(lorum_event, handlers);
        }
    }
    HooksConfig { events }
}

/// Convert a `HooksConfig` to Codex's nested JSON structure.
fn codex_hooks_config_to_json_value(config: &HooksConfig) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (event_name, handlers) in &config.events {
        let tool_event = lorum_event_to_codex(event_name);
        let inner_handlers: Vec<serde_json::Value> = handlers
            .iter()
            .map(|h| {
                let mut obj = serde_json::Map::new();
                if let Some(ref ty) = h.handler_type {
                    obj.insert("type".into(), serde_json::Value::String(ty.clone()));
                }
                obj.insert(
                    "command".into(),
                    serde_json::Value::String(h.command.clone()),
                );
                if let Some(t) = h.timeout {
                    obj.insert("timeout".into(), serde_json::Value::Number(t.into()));
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        let outer_item = serde_json::json!({ "hooks": inner_handlers });
        map.insert(tool_event, serde_json::Value::Array(vec![outer_item]));
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod codex_rules_tests {
    use super::*;

    #[test]
    fn rules_path_returns_codex_rules_md() {
        let adapter = CodexRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/.codex/rules.md"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CodexRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file_and_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CodexRulesAdapter;
        let path = adapter.rules_path(dir.path());
        assert!(!path.exists());
        assert!(!dir.path().join(".codex").exists());

        adapter
            .write_rules(dir.path(), "Use 4-space indentation.")
            .unwrap();
        assert!(path.exists());
        assert!(dir.path().join(".codex").is_dir());
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CodexRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn rules_adapter_name() {
        let adapter = CodexRulesAdapter;
        assert_eq!(adapter.name(), "codex");
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

[mcp_servers.test-server]
command = "npx"
args = ["-y", "some-pkg"]

[mcp_servers.test-server.env]
KEY = "value"
"#;
        fs::write(&path, toml_str).unwrap();

        let root: toml::Value = toml::from_str(toml_str).unwrap();
        let config = toml_utils::parse_mcp_servers_toml(&root, MCP_FIELD);

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    fn read_mcp_empty_when_no_field() {
        let root: toml::Value = toml::from_str("other = true").unwrap();
        let config = toml_utils::parse_mcp_servers_toml(&root, MCP_FIELD);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn write_mcp_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let original = r#"other_field = true
[mcp_servers]
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
        root.as_table_mut().unwrap().insert(
            MCP_FIELD.into(),
            toml_utils::mcp_config_to_toml_value(&config),
        );
        toml_utils::write_toml(&path, &root).unwrap();

        let result: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["other_field"].as_bool(), Some(true));
        assert_eq!(
            result["mcp_servers"]["svr"]["command"].as_str(),
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
        root.as_table_mut().unwrap().insert(
            MCP_FIELD.into(),
            toml_utils::mcp_config_to_toml_value(&config),
        );
        toml_utils::write_toml(&path, &root).unwrap();

        assert!(path.exists());
        let result: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["mcp_servers"]["s"]["command"].as_str(), Some("c"));
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
        let parsed = toml_utils::parse_mcp_servers_toml(
            &toml::Value::Table({
                let mut t = toml::map::Map::new();
                t.insert(MCP_FIELD.into(), toml_val);
                t
            }),
            MCP_FIELD,
        );
        assert_eq!(config, parsed);
    }

    #[test]
    fn adapter_name() {
        let adapter = CodexAdapter::new();
        assert_eq!(ToolAdapter::name(&adapter), "codex");
    }

    // --- hooks -------------------------------------------------------------

    #[test]
    fn codex_hooks_event_mapping() {
        let adapter = CodexAdapter::new();

        // lorum -> tool
        assert_eq!(
            adapter.lorum_to_tool_event("pre-tool-use"),
            Some("PreToolUse".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("post-tool-use"),
            Some("PostToolUse".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("session-end"),
            Some("Stop".into())
        );
        assert_eq!(adapter.lorum_to_tool_event("session-start"), None);
        assert_eq!(adapter.lorum_to_tool_event("unknown-event"), None);

        // tool -> lorum
        assert_eq!(
            adapter.tool_to_lorum_event("PreToolUse"),
            Some("pre-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("PostToolUse"),
            Some("post-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("Stop"),
            Some("session-end".into())
        );
        assert_eq!(adapter.tool_to_lorum_event("SessionStart"), None);
        assert_eq!(adapter.tool_to_lorum_event("CustomEvent"), None);
    }

    #[test]
    fn codex_hooks_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CodexAdapter::with_project_root(dir.path().to_path_buf());
        let codex_dir = dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "check.sh".into(),
                timeout: Some(30),
                handler_type: Some("command".into()),
            }],
        );
        config.events.insert(
            "post-tool-use".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "notify.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );

        adapter.write_hooks(&config).unwrap();

        let read = adapter.read_hooks().unwrap();
        assert_eq!(read.events.len(), 2);
        let pre = &read.events["pre-tool-use"];
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].command, "check.sh");
        assert_eq!(pre[0].timeout, Some(30));
        assert_eq!(pre[0].handler_type, Some("command".into()));
        let post = &read.events["post-tool-use"];
        assert_eq!(post.len(), 1);
        assert_eq!(post[0].command, "notify.sh");
        assert_eq!(post[0].timeout, None);
        assert_eq!(post[0].handler_type, None);
    }

    #[test]
    fn codex_hooks_preserves_nested_structure() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CodexAdapter::with_project_root(dir.path().to_path_buf());
        let codex_dir = dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "echo hello".into(),
                timeout: Some(30),
                handler_type: Some("command".into()),
            }],
        );

        adapter.write_hooks(&config).unwrap();

        let path = codex_dir.join("hooks.json");
        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

        // Check nested structure: hooks -> EventName -> [ { hooks: [ { ... } ] } ]
        let event_array = result["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(event_array.len(), 1);
        let inner_hooks = event_array[0]["hooks"].as_array().unwrap();
        assert_eq!(inner_hooks.len(), 1);
        assert_eq!(inner_hooks[0]["type"], "command");
        assert_eq!(inner_hooks[0]["command"], "echo hello");
        assert_eq!(inner_hooks[0]["timeout"], 30);
    }

    #[test]
    fn codex_hooks_skips_unsupported_events() {
        let adapter = CodexAdapter::new();

        // lorum_to_tool_event returns None for unsupported events
        assert_eq!(adapter.lorum_to_tool_event("session-start"), None);
        assert_eq!(adapter.lorum_to_tool_event("pre-read-file"), None);
        assert_eq!(adapter.lorum_to_tool_event("unknown-event"), None);

        // tool_to_lorum_event returns None for unsupported events
        assert_eq!(adapter.tool_to_lorum_event("SessionStart"), None);
        assert_eq!(adapter.tool_to_lorum_event("CustomEvent"), None);
    }

    #[test]
    fn codex_hooks_unknown_events_preserved_on_read() {
        let dir = tempfile::tempdir().unwrap();
        let codex_dir = dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let path = codex_dir.join("hooks.json");

        let json = serde_json::json!({
            "hooks": {
                "CustomEvent": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "run.sh"
                            }
                        ]
                    }
                ]
            }
        });
        fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

        let adapter = CodexAdapter::with_project_root(dir.path().to_path_buf());
        let config = adapter.read_hooks().unwrap();

        // "CustomEvent" -> "custom-event" via pascal_to_kebab fallback
        assert!(config.events.contains_key("custom-event"));
        let handlers = &config.events["custom-event"];
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].command, "run.sh");
        assert_eq!(handlers[0].handler_type, Some("command".into()));
        assert_eq!(handlers[0].matcher, "*");
    }

    #[test]
    fn codex_hooks_config_paths() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CodexAdapter::with_project_root(dir.path().to_path_buf());
        let paths = HooksAdapter::config_paths(&adapter);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], dir.path().join(".codex").join("hooks.json"));
        assert!(paths[1].ends_with(".codex/hooks.json"));
    }

    #[test]
    #[serial_test::serial]
    fn codex_hooks_reads_user_level_fallback() {
        let dir = tempfile::tempdir().unwrap();
        // Override HOME so user-level path resolves inside the temp dir
        // (which does not contain a .codex/hooks.json file).
        unsafe { std::env::set_var("HOME", dir.path()) };
        let adapter = CodexAdapter::with_project_root(dir.path().join("nonexistent").to_path_buf());
        let result = adapter.read_hooks().unwrap();
        assert!(result.events.is_empty());
        unsafe { std::env::remove_var("HOME") };
    }

    #[test]
    fn codex_hooks_preserves_non_hooks_fields() {
        let dir = tempfile::tempdir().unwrap();
        let codex_dir = dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let path = codex_dir.join("hooks.json");

        let original = r#"{"version": 2, "hooks": {}}"#;
        fs::write(&path, original).unwrap();

        let adapter = CodexAdapter::with_project_root(dir.path().to_path_buf());
        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "check.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        adapter.write_hooks(&config).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["version"], 2);
        assert_eq!(
            result["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "check.sh"
        );
    }
}
