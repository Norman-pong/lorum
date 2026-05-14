//! Claude Code adapter for reading/writing MCP and hooks configuration.
//!
//! Configuration file: `~/.claude/settings.json` (global)
//!
//! MCP format (JSON):
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
//! Hooks format (JSON):
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "match": "Bash",
//!         "command": "scripts/check.sh",
//!         "timeout": 60
//!       }
//!     ]
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{
    ConfigValidator, HooksAdapter, RulesAdapter, SkillsAdapter, ToolAdapter, ValidationIssue,
    default_validate_config, json_utils, kebab_to_pascal, pascal_to_kebab, read_rules_file,
    write_rules_file,
};
use crate::config::{HookHandler, HooksConfig, McpConfig};
use crate::error::LorumError;
use crate::skills::{SkillEntry, copy_dir_recursive, scan_skills_dir};

/// Adapter for Claude Code rules.
///
/// Reads and writes rules content from Claude Code's `CLAUDE.md`
/// file located at the project root.
pub struct ClaudeRulesAdapter;

impl RulesAdapter for ClaudeRulesAdapter {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join("CLAUDE.md")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        write_rules_file(&self.rules_path(project_root), content)
    }
}

/// Adapter for Claude Code.
///
/// Reads and writes MCP server configurations from Claude Code's
/// `~/.claude/settings.json` file, preserving any non-MCP fields.
pub struct ClaudeAdapter;

/// Adapter for Claude Code skills.
pub struct ClaudeSkillsAdapter;

/// Field name used by Claude Code for MCP servers.
const MCP_FIELD: &str = "mcpServers";

/// Returns the global Claude Code settings path: `~/.claude/settings.json`.
fn global_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// Returns the global Claude Code skills base dir: `~/.claude/skills/`.
fn global_skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("skills"))
}

impl SkillsAdapter for ClaudeSkillsAdapter {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn skills_base_dir(&self) -> Option<PathBuf> {
        global_skills_dir()
    }

    fn read_skills(&self) -> Result<Vec<SkillEntry>, LorumError> {
        let Some(dir) = self.skills_base_dir() else {
            return Ok(Vec::new());
        };
        scan_skills_dir(&dir)
    }

    fn write_skill(&self, name: &str, source_dir: &Path) -> Result<(), LorumError> {
        let dir = self.skills_base_dir().ok_or_else(|| LorumError::Other {
            message: "cannot determine home directory".into(),
        })?;
        let target = dir.join(name);
        if target.exists() {
            let old = dir.join(format!(".old-{name}"));
            if old.exists() {
                std::fs::remove_dir_all(&old)?;
            }
            std::fs::rename(&target, &old)?;
        }
        copy_dir_recursive(source_dir, &target)
    }

    fn remove_skill(&self, name: &str) -> Result<(), LorumError> {
        let dir = self.skills_base_dir().ok_or_else(|| LorumError::Other {
            message: "cannot determine home directory".into(),
        })?;
        let target = dir.join(name);
        if target.exists() {
            std::fs::remove_dir_all(target)?;
        }
        Ok(())
    }
}

impl HooksAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        global_settings_path().into_iter().collect()
    }

    fn read_hooks(&self) -> Result<HooksConfig, LorumError> {
        let path = match global_settings_path() {
            Some(p) => p,
            None => return Ok(HooksConfig::default()),
        };
        if !path.exists() {
            return Ok(HooksConfig::default());
        }
        let root = json_utils::read_existing_json(&path)?;
        Ok(parse_hooks_from_json(root.get("hooks")))
    }

    fn write_hooks(&self, config: &HooksConfig) -> Result<(), LorumError> {
        let path = match global_settings_path() {
            Some(p) => p,
            None => {
                return Err(LorumError::Other {
                    message: "cannot determine home directory".into(),
                });
            }
        };
        let mut root = json_utils::read_existing_json(&path)?;
        root["hooks"] = hooks_config_to_json_value(config);
        json_utils::write_json(&path, &root)
    }

    fn lorum_to_tool_event(&self, lorum_event: &str) -> Option<String> {
        Some(kebab_to_pascal(lorum_event))
    }

    fn tool_to_lorum_event(&self, tool_event: &str) -> Option<String> {
        Some(pascal_to_kebab(tool_event))
    }
}

impl ConfigValidator for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        default_validate_config(self)
    }
}

impl ToolAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        global_settings_path().into_iter().collect()
    }

    fn read_mcp(&self) -> Result<McpConfig, LorumError> {
        let path = match global_settings_path() {
            Some(p) => p,
            None => return Ok(McpConfig::default()),
        };
        if !path.exists() {
            return Ok(McpConfig::default());
        }
        let root = json_utils::read_existing_json(&path)?;
        Ok(json_utils::parse_mcp_servers(&root, MCP_FIELD))
    }

    fn write_mcp(&self, config: &McpConfig) -> Result<(), LorumError> {
        let path = match global_settings_path() {
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

/// Parse hooks from a JSON value.
fn parse_hooks_from_json(value: Option<&serde_json::Value>) -> HooksConfig {
    let Some(obj) = value.and_then(|v| v.as_object()) else {
        return HooksConfig::default();
    };
    let mut events = BTreeMap::new();
    for (pascal_event, handlers_value) in obj {
        let kebab_event = pascal_to_kebab(pascal_event);
        let Some(handlers_array) = handlers_value.as_array() else {
            continue;
        };
        let mut handlers = Vec::new();
        for handler_value in handlers_array {
            let Some(handler_obj) = handler_value.as_object() else {
                continue;
            };
            let Some(matcher) = handler_obj
                .get("match")
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
            events.insert(kebab_event, handlers);
        }
    }
    HooksConfig { events }
}

/// Convert a HooksConfig to a JSON value.
fn hooks_config_to_json_value(config: &HooksConfig) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (event_name, handlers) in &config.events {
        let pascal_event = kebab_to_pascal(event_name);
        let handlers_array: Vec<serde_json::Value> = handlers
            .iter()
            .map(|h| {
                let mut obj = serde_json::Map::new();
                obj.insert("match".into(), serde_json::Value::String(h.matcher.clone()));
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
        map.insert(pascal_event, serde_json::Value::Array(handlers_array));
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod claude_rules_tests {
    use super::*;

    #[test]
    fn rules_path_returns_claude_md() {
        let adapter = ClaudeRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/CLAUDE.md"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ClaudeRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ClaudeRulesAdapter;
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
        let adapter = ClaudeRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn rules_adapter_name() {
        let adapter = ClaudeRulesAdapter;
        assert_eq!(adapter.name(), "claude-code");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_utils::make_server;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn read_mcp_from_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
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

        let root: serde_json::Value = serde_json::from_str(json).unwrap();
        let config = json_utils::parse_mcp_servers(&root, MCP_FIELD);

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    fn read_mcp_empty_when_no_field() {
        let root: serde_json::Value = serde_json::json!({ "otherField": true });
        let config = json_utils::parse_mcp_servers(&root, MCP_FIELD);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn write_mcp_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let original = r#"{"otherField": true, "mcpServers": {}}"#;
        fs::write(&path, original).unwrap();

        let mut root = json_utils::read_existing_json(&path).unwrap();
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("svr".into(), make_server("cmd", &["a"], &[("K", "V")]));
                m
            },
        };
        root[MCP_FIELD] = json_utils::mcp_config_to_json_value(&config);
        json_utils::write_json(&path, &root).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["otherField"], true);
        assert_eq!(result["mcpServers"]["svr"]["command"], "cmd");
    }

    #[test]
    fn write_mcp_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("settings.json");
        assert!(!path.exists());

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let mut root = serde_json::Value::Object(serde_json::Map::new());
        root[MCP_FIELD] = json_utils::mcp_config_to_json_value(&config);
        json_utils::write_json(&path, &root).unwrap();

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
        let adapter = ClaudeAdapter;
        assert_eq!(ToolAdapter::name(&adapter), "claude-code");
    }

    // --- hooks -------------------------------------------------------------

    #[test]
    fn parse_hooks_from_valid_json() {
        let json = serde_json::json!({
            "PreToolUse": [
                {
                    "match": "Bash",
                    "command": "scripts/check.sh",
                    "timeout": 60
                }
            ],
            "PostToolUse": [
                {
                    "match": "Write|Edit",
                    "command": "cargo fmt"
                }
            ]
        });
        let config = parse_hooks_from_json(Some(&json));
        assert_eq!(config.events.len(), 2);
        let pre = &config.events["pre-tool-use"];
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0].matcher, "Bash");
        assert_eq!(pre[0].command, "scripts/check.sh");
        assert_eq!(pre[0].timeout, Some(60));
        let post = &config.events["post-tool-use"];
        assert_eq!(post[0].matcher, "Write|Edit");
    }

    #[test]
    fn parse_hooks_empty_when_no_field() {
        let config = parse_hooks_from_json(None);
        assert!(config.events.is_empty());
    }

    #[test]
    fn write_hooks_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"otherField": true}"#).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "Bash".into(),
                command: "check.sh".into(),
                timeout: Some(30),
                handler_type: None,
            }],
        );

        // Test via helper functions (adapter uses global path).
        let mut root = json_utils::read_existing_json(&path).unwrap();
        root["hooks"] = hooks_config_to_json_value(&config);
        json_utils::write_json(&path, &root).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["otherField"], true);
        assert_eq!(result["hooks"]["PreToolUse"][0]["match"], "Bash");
        assert_eq!(result["hooks"]["PreToolUse"][0]["timeout"], 30);
    }

    #[test]
    fn write_hooks_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("settings.json");
        assert!(!path.exists());

        let mut config = HooksConfig::default();
        config.events.insert(
            "session-start".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "start.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        let json = hooks_config_to_json_value(&config);
        let mut root = serde_json::Value::Object(serde_json::Map::new());
        root["hooks"] = json;
        json_utils::write_json(&path, &root).unwrap();

        assert!(path.exists());
        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["hooks"]["SessionStart"][0]["match"], "*");
    }

    #[test]
    fn hooks_roundtrip_json() {
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
        let json = hooks_config_to_json_value(&config);
        let parsed = parse_hooks_from_json(Some(&json));
        assert_eq!(config, parsed);
    }

    // --- skills ------------------------------------------------------------

    #[test]
    fn skills_adapter_name() {
        let adapter = ClaudeSkillsAdapter;
        assert_eq!(adapter.name(), "claude-code");
    }

    #[test]
    fn read_skills_empty_when_no_dir() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("skills");
        let skills = scan_skills_dir(&dir).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn write_skill_copies_directory_contents() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(
            src.path().join("SKILL.md"),
            "---\nname: test-skill\ndescription: \"Test\"\n---\n# Body\n",
        )
        .unwrap();
        std::fs::create_dir_all(src.path().join("scripts")).unwrap();
        std::fs::write(src.path().join("scripts/run.sh"), "echo hi\n").unwrap();

        let dst_root = tempfile::tempdir().unwrap();
        let target = dst_root.path().join("test-skill");
        copy_dir_recursive(src.path(), &target).unwrap();

        assert!(target.join("SKILL.md").exists());
        assert!(target.join("scripts/run.sh").exists());
    }

    #[test]
    #[serial_test::serial]
    fn remove_skill_deletes_directory() {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", home.path()) };
        let skills_dir = home
            .path()
            .join(".claude")
            .join("skills")
            .join("test-skill");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("SKILL.md"), "# Skill\n").unwrap();
        assert!(skills_dir.exists());

        let adapter = ClaudeSkillsAdapter;
        adapter.remove_skill("test-skill").unwrap();
        assert!(!skills_dir.exists());
        unsafe { std::env::remove_var("HOME") };
    }
}
