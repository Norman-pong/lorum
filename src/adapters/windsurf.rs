//! Windsurf adapter for reading/writing rules and MCP configuration.
//!
//! Rules file: `{project_root}/.windsurfrules`
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

use crate::adapters::{RulesAdapter, ToolAdapter, json_utils, read_rules_file, write_rules_file};
use crate::config::McpConfig;
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

/// Adapter for Windsurf MCP configuration.
///
/// Reads and writes MCP server configurations from Windsurf's
/// global `~/.codeium/windsurf/mcp_config.json` file, preserving any
/// non-MCP fields.
///
/// SSE remote MCP servers (entries with `serverUrl` instead of `command`)
/// are skipped during read with a warning.
pub struct WindsurfAdapter;

/// Field name used by Windsurf for MCP servers.
const MCP_FIELD: &str = "mcpServers";

/// Returns the global Windsurf config path: `~/.codeium/windsurf/mcp_config.json`.
fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codeium").join("windsurf").join("mcp_config.json"))
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

        let adapter = WindsurfAdapter;
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

        let adapter = WindsurfAdapter;
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

        let adapter = WindsurfAdapter;
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

        let adapter = WindsurfAdapter;
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
        let adapter = WindsurfAdapter;
        assert_eq!(adapter.name(), "windsurf");
    }
}
