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

use std::path::PathBuf;

use crate::adapters::ToolAdapter;
use crate::adapters::toml_utils;
use crate::config::McpConfig;
use crate::error::LorumError;

/// Adapter for Codex.
///
/// Reads and writes MCP server configurations from Codex's
/// `~/.codex/config.toml` file, preserving any non-MCP fields.
pub struct CodexAdapter;

/// Top-level TOML key used by Codex for MCP servers.
const MCP_FIELD: &str = "mcp_servers";

/// Returns the global Codex config path: `~/.codex/config.toml`.
fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("config.toml"))
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
        let adapter = CodexAdapter;
        assert_eq!(adapter.name(), "codex");
    }
}
