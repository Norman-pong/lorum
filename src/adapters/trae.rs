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

use std::path::PathBuf;

use crate::adapters::ToolAdapter;
use crate::adapters::json_utils;
use crate::config::McpConfig;
use crate::error::LorumError;

/// Adapter for Trae.
///
/// Reads and writes MCP server configurations from Trae's
/// project-level `.trae/mcp.json` file, preserving any non-MCP fields.
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
        assert_eq!(adapter.name(), "trae");
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
