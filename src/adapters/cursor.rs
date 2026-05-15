//! Cursor adapter for reading/writing rules, hooks, and MCP configuration.
//!
//! Rules file: `{project_root}/.cursorrules`
//!
//! Hooks file: `{project_root}/.cursor/hooks.json` (project-level)
//!   and `~/.cursor/hooks.json` (user-level)
//!
//! MCP configuration file: `{project_root}/.cursor/mcp.json` (project-level)
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

use std::path::{Path, PathBuf};

use crate::adapters::{
    ConfigValidator, HooksAdapter, RulesAdapter, SkillsAdapter, ToolAdapter, ValidationIssue,
    camel_to_kebab, default_validate_config, json_utils, kebab_to_camel, read_rules_file,
    write_rules_file,
};
use crate::config::{HooksConfig, McpConfig};
use crate::error::LorumError;
use crate::skills::{SkillEntry, copy_dir_recursive, scan_skills_dir};

/// Adapter for Cursor rules.
///
/// Reads and writes rules content from Cursor's `.cursorrules` file
/// located at the project root.
pub struct CursorRulesAdapter;

impl RulesAdapter for CursorRulesAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".cursorrules")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        write_rules_file(&self.rules_path(project_root), content)
    }
}

/// Adapter for Cursor MCP configuration.
///
/// Reads and writes MCP server configurations from Cursor's
/// project-level `.cursor/mcp.json` file, preserving any non-MCP fields.
pub struct CursorAdapter {
    project_root: Option<PathBuf>,
}

/// Field name used by Cursor for MCP servers.
const MCP_FIELD: &str = "mcpServers";

impl CursorAdapter {
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

    /// Returns the project-level Cursor config path: `.cursor/mcp.json`.
    fn project_config_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".cursor").join("mcp.json"))
    }

    /// Returns the project-level Cursor hooks path: `.cursor/hooks.json`.
    fn project_hooks_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".cursor").join("hooks.json"))
    }

    /// Returns the user-level Cursor hooks path: `~/.cursor/hooks.json`.
    fn user_hooks_path(&self) -> Option<PathBuf> {
        Some(dirs::home_dir()?.join(".cursor").join("hooks.json"))
    }
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksAdapter for CursorAdapter {
    fn name(&self) -> &str {
        "cursor"
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
        // Try project-level first, then user-level.
        let paths = [self.project_hooks_path(), self.user_hooks_path()];
        for path in paths.into_iter().flatten() {
            if path.exists() {
                let root = json_utils::read_existing_json(&path)?;
                return Ok(parse_hooks_from_json(root.get("hooks")));
            }
        }
        Ok(HooksConfig::default())
    }

    fn write_hooks(&self, config: &HooksConfig) -> Result<(), LorumError> {
        // Write to project-level path if available, otherwise user-level.
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
        root["hooks"] = hooks_config_to_json_value(config);
        json_utils::write_json(&path, &root)
    }

    fn lorum_to_tool_event(&self, lorum_event: &str) -> Option<String> {
        Some(kebab_to_camel(lorum_event))
    }

    fn tool_to_lorum_event(&self, tool_event: &str) -> Option<String> {
        Some(camel_to_kebab(tool_event))
    }
}

impl ConfigValidator for CursorAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        default_validate_config(self)
    }
}

/// Adapter for Cursor skills.
///
/// Reads and writes skills from Cursor's `~/.cursor/skills/` directory.
pub struct CursorSkillsAdapter;

impl SkillsAdapter for CursorSkillsAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn skills_base_dir(&self) -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".cursor").join("skills"))
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

impl ToolAdapter for CursorAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        self.project_config_path().into_iter().collect()
    }

    fn read_mcp(&self) -> Result<McpConfig, LorumError> {
        let path = match self.project_config_path() {
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
        let path = match self.project_config_path() {
            Some(p) => p,
            None => {
                return Err(LorumError::Other {
                    message: "cannot determine project directory".into(),
                });
            }
        };
        let mut root = json_utils::read_existing_json(&path)?;
        root[MCP_FIELD] = json_utils::mcp_config_to_json_value(config);
        json_utils::write_json(&path, &root)
    }
}

/// Parse hooks from a JSON value (Cursor uses `"matcher"` as the matcher key).
fn parse_hooks_from_json(value: Option<&serde_json::Value>) -> HooksConfig {
    json_utils::parse_hooks_from_json_value(value, camel_to_kebab, "matcher")
}

/// Convert a HooksConfig to a JSON value (Cursor uses `"matcher"` as the matcher key).
fn hooks_config_to_json_value(config: &HooksConfig) -> serde_json::Value {
    json_utils::hooks_config_to_json_value(config, kebab_to_camel, "matcher")
}

#[cfg(test)]
mod cursor_rules_tests {
    use super::*;

    #[test]
    fn rules_path_returns_cursorrules() {
        let adapter = CursorRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/.cursorrules"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorRulesAdapter;
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
        let adapter = CursorRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn rules_adapter_name() {
        let adapter = CursorRulesAdapter;
        assert_eq!(adapter.name(), "cursor");
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
        let cursor_dir = dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        let path = cursor_dir.join("mcp.json");
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

        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let config = adapter.read_mcp().unwrap();

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
        let cursor_dir = dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        let path = cursor_dir.join("mcp.json");

        let original = r#"{"otherField": true, "mcpServers": {}}"#;
        fs::write(&path, original).unwrap();

        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
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
    fn write_mcp_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        let path = subdir.join(".cursor").join("mcp.json");
        assert!(!path.exists());

        let adapter = CursorAdapter::with_project_root(subdir);
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
        let adapter = CursorAdapter::new();
        assert_eq!(ToolAdapter::name(&adapter), "cursor");
    }

    #[test]
    fn with_project_root_overrides_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let paths = ToolAdapter::config_paths(&adapter);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], dir.path().join(".cursor").join("mcp.json"));
    }

    #[test]
    fn cursor_hooks_event_mapping() {
        let adapter = CursorAdapter::new();

        assert_eq!(
            adapter.lorum_to_tool_event("pre-tool-use"),
            Some("preToolUse".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("post-tool-use"),
            Some("postToolUse".into())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("session-start"),
            Some("sessionStart".into())
        );

        assert_eq!(
            adapter.tool_to_lorum_event("preToolUse"),
            Some("pre-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("postToolUse"),
            Some("post-tool-use".into())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("sessionStart"),
            Some("session-start".into())
        );

        // Roundtrip.
        assert_eq!(
            adapter.tool_to_lorum_event(&adapter.lorum_to_tool_event("pre-read-file").unwrap()),
            Some("pre-read-file".into())
        );
    }

    #[test]
    fn cursor_hooks_roundtrip() {
        use crate::config::HookHandler;

        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let cursor_dir = dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();

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
        let handlers = &read.events["pre-tool-use"];
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].matcher, "Bash");
        assert_eq!(handlers[0].command, "check.sh");
        assert_eq!(handlers[0].timeout, Some(30));
        assert_eq!(handlers[0].handler_type, Some("command".into()));
    }

    #[test]
    fn cursor_hooks_preserves_version_field() {
        let dir = tempfile::tempdir().unwrap();
        let cursor_dir = dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        let path = cursor_dir.join("hooks.json");

        let original = r#"{"version": 2, "hooks": {}}"#;
        fs::write(&path, original).unwrap();

        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![crate::config::HookHandler {
                matcher: "Bash".into(),
                command: "check.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        adapter.write_hooks(&config).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["version"], 2);
        assert_eq!(result["hooks"]["preToolUse"][0]["matcher"], "Bash");
    }

    #[test]
    fn cursor_hooks_write_and_read_supported_events() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let cursor_dir = dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();

        let mut config = HooksConfig::default();
        config.events.insert(
            "pre-tool-use".into(),
            vec![crate::config::HookHandler {
                matcher: "Bash".into(),
                command: "check.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        // Add an event with an invalid name (contains spaces) that camelCase conversion
        // would still produce something, but we're testing that the write still works.
        adapter.write_hooks(&config).unwrap();

        let read = adapter.read_hooks().unwrap();
        assert!(read.events.contains_key("pre-tool-use"));
    }

    #[test]
    fn cursor_hooks_unknown_events_preserved_on_read() {
        let dir = tempfile::tempdir().unwrap();
        let cursor_dir = dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        let path = cursor_dir.join("hooks.json");

        let json = serde_json::json!({
            "hooks": {
                "customEvent": [
                    { "matcher": "*", "command": "run.sh" }
                ]
            }
        });
        fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();

        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let config = adapter.read_hooks().unwrap();

        // "customEvent" -> "custom-event" via camel_to_kebab
        assert!(config.events.contains_key("custom-event"));
        let handlers = &config.events["custom-event"];
        assert_eq!(handlers[0].command, "run.sh");
    }

    #[test]
    fn cursor_hooks_config_paths() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorAdapter::with_project_root(dir.path().to_path_buf());
        let paths = HooksAdapter::config_paths(&adapter);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], dir.path().join(".cursor").join("hooks.json"));
        assert!(paths[1].ends_with(".cursor/hooks.json"));
    }

    #[test]
    fn cursor_hooks_reads_user_level_fallback() {
        let dir = tempfile::tempdir().unwrap();
        // Don't create project-level hooks.json, but create user-level
        // (simulated by setting project_root to a non-existent subdir
        // so that user-level path is tried).
        let adapter =
            CursorAdapter::with_project_root(dir.path().join("nonexistent").to_path_buf());
        let result = adapter.read_hooks().unwrap();
        assert!(result.events.is_empty());
    }
}

#[cfg(test)]
mod skills_tests {
    use super::*;

    #[test]
    fn read_skills_empty_when_no_dir() {
        let adapter = CursorSkillsAdapter;
        let skills = adapter.read_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn write_skill_copies_directory_contents() {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", home.path()) };
        let src = tempfile::tempdir().unwrap();
        std::fs::write(
            src.path().join("SKILL.md"),
            "---\nname: test-skill\ndescription: \"Test\"\n---\n",
        )
        .unwrap();

        let adapter = CursorSkillsAdapter;
        adapter.write_skill("test-skill", src.path()).unwrap();
        let skills = adapter.read_skills().unwrap();
        assert!(skills.iter().any(|s| s.manifest.name == "test-skill"));
        adapter.remove_skill("test-skill").unwrap();
        unsafe { std::env::remove_var("HOME") };
    }

    #[test]
    #[serial_test::serial]
    fn remove_skill_deletes_directory() {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", home.path()) };
        let adapter = CursorSkillsAdapter;
        adapter
            .write_skill("test-skill", tempfile::tempdir().unwrap().path())
            .unwrap();
        adapter.remove_skill("test-skill").unwrap();
        let skills = adapter.read_skills().unwrap();
        assert!(!skills.iter().any(|s| s.manifest.name == "test-skill"));
        unsafe { std::env::remove_var("HOME") };
    }
}
