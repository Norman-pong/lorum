//! Trae adapter for reading/writing MCP configuration.
//!
//! Configuration file: `.trae/mcp.json` (project-level only)
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
    ConfigValidator, RulesAdapter, SkillsAdapter, ToolAdapter, ValidationIssue,
    default_validate_config, json_utils, read_rules_file, write_rules_file,
};
use crate::config::McpConfig;
use crate::error::LorumError;
use crate::skills::{SkillEntry, copy_dir_recursive, scan_skills_dir};

/// Adapter for Trae skills.
///
/// Reads and writes skills from Trae's `~/.trae/skills/` directory.
pub struct TraeSkillsAdapter;

impl SkillsAdapter for TraeSkillsAdapter {
    fn name(&self) -> &str {
        "trae"
    }

    fn skills_base_dir(&self) -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".trae").join("skills"))
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

/// Adapter for Trae.
///
/// Reads and writes MCP server configurations from Trae's
/// project-level `.trae/mcp.json` file, preserving any non-MCP fields.
/// Adapter for Trae rules.
///
/// Reads and writes rules content from Trae's `.trae/rules/project_rules.md`
/// file located at the project root.
pub struct TraeRulesAdapter;

pub struct TraeAdapter {
    project_root: Option<PathBuf>,
}

/// Field name used by Trae for MCP servers.
const MCP_FIELD: &str = "mcpServers";

impl TraeAdapter {
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

    /// Returns the project-level Trae config path: `.trae/mcp.json`.
    fn project_config_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".trae").join("mcp.json"))
    }
}

impl Default for TraeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigValidator for TraeAdapter {
    fn name(&self) -> &str {
        "trae"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        default_validate_config(self)
    }
}

impl RulesAdapter for TraeRulesAdapter {
    fn name(&self) -> &str {
        "trae"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root
            .join(".trae")
            .join("rules")
            .join("project_rules.md")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        write_rules_file(&self.rules_path(project_root), content)
    }
}

impl ToolAdapter for TraeAdapter {
    fn name(&self) -> &str {
        "trae"
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

#[cfg(test)]
mod trae_rules_tests {
    use super::*;

    #[test]
    fn rules_adapter_name() {
        let adapter = TraeRulesAdapter;
        assert_eq!(adapter.name(), "trae");
    }

    #[test]
    fn rules_path_returns_trae_project_rules_md() {
        let adapter = TraeRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(
            path,
            PathBuf::from("/tmp/myproject/.trae/rules/project_rules.md")
        );
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = TraeRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file_and_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = TraeRulesAdapter;
        let path = adapter.rules_path(dir.path());
        assert!(!path.exists());
        assert!(!dir.path().join(".trae").exists());

        adapter
            .write_rules(dir.path(), "Use 4-space indentation.")
            .unwrap();
        assert!(path.exists());
        assert!(dir.path().join(".trae").join("rules").is_dir());
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = TraeRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
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
        let path = dir.path().join("mcp.json");
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
        let path = dir.path().join("mcp.json");

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
        let path = dir.path().join("subdir").join("mcp.json");
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
        let adapter = TraeAdapter::new();
        assert_eq!(ToolAdapter::name(&adapter), "trae");
    }

    #[test]
    fn with_project_root_overrides_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = TraeAdapter::with_project_root(dir.path().to_path_buf());
        let paths = adapter.config_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], dir.path().join(".trae").join("mcp.json"));
    }
}

#[cfg(test)]
mod skills_tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn read_skills_empty_when_no_dir() {
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", home.path()) };
        let adapter = TraeSkillsAdapter;
        let skills = adapter.read_skills().unwrap();
        assert!(skills.is_empty());
        unsafe { std::env::remove_var("HOME") };
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

        let adapter = TraeSkillsAdapter;
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
        let adapter = TraeSkillsAdapter;
        adapter
            .write_skill("test-skill", tempfile::tempdir().unwrap().path())
            .unwrap();
        adapter.remove_skill("test-skill").unwrap();
        let skills = adapter.read_skills().unwrap();
        assert!(!skills.iter().any(|s| s.manifest.name == "test-skill"));
        unsafe { std::env::remove_var("HOME") };
    }
}
