//! Kimi adapter for reading/writing MCP configuration.
//!
//! Configuration file: `~/.kimi/config.toml` (global)
//!
//! Format (TOML):
//! ```toml
//! [mcp.client.server-name]
//! command = "npx"
//! args = ["-y", "some-pkg"]
//!
//! [mcp.client.server-name.env]
//! KEY = "value"
//! ```

use std::path::PathBuf;

use crate::adapters::ToolAdapter;
use crate::adapters::toml_utils;
use crate::config::McpConfig;
use crate::error::LorumError;

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
        assert_eq!(adapter.name(), "kimi");
    }
}
