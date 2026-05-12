//! Proma adapter for reading/writing MCP configuration.
//!
//! Configuration file: `~/.proma/mcp.json` (global)
//!
//! Format (JSON):
//! ```json
//! {
//!   "servers": {
//!     "server-name": {
//!       "command": "npx",
//!       "args": ["-y", "some-pkg"],
//!       "env": { "KEY": "value" }
//!     }
//!   }
//! }
//! ```

use std::path::{Path, PathBuf};

use crate::adapters::json_utils;
use crate::adapters::{SkillsAdapter, ToolAdapter};
use crate::config::McpConfig;
use crate::error::LorumError;
use crate::skills::{SkillEntry, copy_dir_recursive, scan_skills_dir};

/// Adapter for Proma.
///
/// Reads and writes MCP server configurations from Proma's
/// `~/.proma/mcp.json` file, preserving any non-MCP fields.
pub struct PromaAdapter;

/// Field name used by Proma for MCP servers.
const MCP_FIELD: &str = "servers";

/// Returns the global Proma config path: `~/.proma/mcp.json`.
fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".proma").join("mcp.json"))
}

/// Returns the Proma workspace skills base dir for the current workspace.
fn workspace_skills_dir() -> Option<PathBuf> {
    std::env::var("PROMA_WORKSPACE_ROOT")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            dirs::home_dir().map(|h| h.join(".proma").join("agent-workspaces").join("lorum"))
        })
        .map(|root| root.join("skills"))
}

/// Adapter for Proma skills.
pub struct PromaSkillsAdapter;

impl SkillsAdapter for PromaSkillsAdapter {
    fn name(&self) -> &str {
        "proma"
    }

    fn skills_base_dir(&self) -> Option<PathBuf> {
        workspace_skills_dir()
    }

    fn read_skills(&self) -> Result<Vec<SkillEntry>, LorumError> {
        let Some(dir) = self.skills_base_dir() else {
            return Ok(Vec::new());
        };
        scan_skills_dir(&dir)
    }

    fn write_skill(&self, name: &str, source_dir: &Path) -> Result<(), LorumError> {
        let dir = self.skills_base_dir().ok_or_else(|| LorumError::Other {
            message: "cannot determine Proma workspace directory".into(),
        })?;
        let target = dir.join(name);
        if target.exists() {
            std::fs::remove_dir_all(&target)?;
        }
        copy_dir_recursive(source_dir, &target)
    }

    fn remove_skill(&self, name: &str) -> Result<(), LorumError> {
        let dir = self.skills_base_dir().ok_or_else(|| LorumError::Other {
            message: "cannot determine Proma workspace directory".into(),
        })?;
        let target = dir.join(name);
        if target.exists() {
            std::fs::remove_dir_all(target)?;
        }
        Ok(())
    }
}

impl ToolAdapter for PromaAdapter {
    fn name(&self) -> &str {
        "proma"
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
        Ok(json_utils::parse_mcp_servers(&root, MCP_FIELD))
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
            "servers": {
                "test-server": {
                    "command": "npx",
                    "args": ["-y", "some-pkg"],
                    "env": { "KEY": "value" }
                }
            },
            "version": 1
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
        let root: serde_json::Value = serde_json::json!({ "version": 1 });
        let config = json_utils::parse_mcp_servers(&root, MCP_FIELD);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn write_mcp_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let original = r#"{"version": 1, "servers": {}}"#;
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
        assert_eq!(result["version"], 1);
        assert_eq!(result["servers"]["svr"]["command"], "cmd");
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
        assert_eq!(result["servers"]["s"]["command"], "c");
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
        let wrapped = serde_json::json!({ "servers": json_val });
        let parsed = json_utils::parse_mcp_servers(&wrapped, MCP_FIELD);
        assert_eq!(config, parsed);
    }

    #[test]
    fn skills_adapter_name() {
        let adapter = PromaSkillsAdapter;
        assert_eq!(adapter.name(), "proma");
    }

    #[test]
    fn write_skill_copies_directory_contents() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(
            src.path().join("SKILL.md"),
            "---\nname: test-skill\ndescription: \"Test\"\n---\n# Body\n",
        )
        .unwrap();
        std::fs::create_dir_all(src.path().join("references")).unwrap();
        std::fs::write(src.path().join("references/info.md"), "hello\n").unwrap();

        let dst_root = tempfile::tempdir().unwrap();
        let target = dst_root.path().join("test-skill");
        copy_dir_recursive(src.path(), &target).unwrap();

        assert!(target.join("SKILL.md").exists());
        assert!(target.join("references/info.md").exists());
    }
}
