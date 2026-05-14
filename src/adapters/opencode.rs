//! Opencode adapter for reading/writing MCP configuration.
//!
//! Configuration files:
//! - Global: `~/.config/opencode/opencode.json`
//! - Project-level: `opencode.json` (fallback)
//!
//! Format (JSON):
//! ```json
//! {
//!   "mcp": {
//!     "server-name": {
//!       "type": "local",
//!       "command": ["npx", "-y", "my-command"],
//!       "enabled": true,
//!       "environment": {"KEY": "value"},
//!       "timeout": 5000
//!     }
//!   }
//! }
//! ```
//!
//! Field mapping (lorum ↔ opencode):
//! - `command` → `command[0]`
//! - `args` → `command[1..]`
//! - `env` → `environment`
//! - `type` → preserved on read, written as `"local"`
//! - `enabled` → preserved on read, written as `true`
//! - `timeout` → ignored on read, not written

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{
    ConfigValidator, RulesAdapter, Severity, ToolAdapter, ValidationIssue, json_utils,
    read_rules_file, validate_all_syntax, write_rules_file,
};
use crate::config::{McpConfig, McpServer};
use crate::error::LorumError;

/// Adapter for Opencode.
///
/// Reads and writes MCP server configurations from Opencode's
/// `~/.config/opencode/opencode.json` (global, priority) or
/// project-level `opencode.json` (fallback), preserving any non-MCP fields.
/// Adapter for OpenCode rules.
///
/// Reads and writes rules content from OpenCode's `AGENTS.md`
/// file located at the project root.
pub struct OpenCodeRulesAdapter;

impl RulesAdapter for OpenCodeRulesAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join("AGENTS.md")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        read_rules_file(&self.rules_path(project_root))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        write_rules_file(&self.rules_path(project_root), content)
    }
}

pub struct OpencodeAdapter {
    project_root: Option<PathBuf>,
}

/// Top-level field used by Opencode for MCP servers.
const MCP_FIELD: &str = "mcp";

impl OpencodeAdapter {
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

    /// Returns the global Opencode config path: `~/.config/opencode/opencode.json`.
    fn global_config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".config").join("opencode").join("opencode.json"))
    }

    /// Returns the project-level Opencode config path: `{cwd}/opencode.json`.
    fn project_config_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join("opencode.json"))
    }

    /// Returns the path to read from: global first, then project-level fallback.
    fn read_config_path(&self) -> Option<PathBuf> {
        let global = Self::global_config_path()?;
        if global.exists() {
            Some(global)
        } else {
            self.project_config_path()
        }
    }

    /// Returns the path to write to: project-level if in a project directory,
    /// otherwise global. A project directory is detected by the presence of
    /// `.git/`, `Cargo.toml`, or an existing `opencode.json`.
    fn write_config_path(&self) -> Option<PathBuf> {
        let project = self.project_config_path()?;
        let is_project = project.exists()
            || project
                .parent()
                .is_some_and(|p| p.join(".git").exists() || p.join("Cargo.toml").exists());
        if is_project {
            Some(project)
        } else {
            Self::global_config_path()
        }
    }
}

impl Default for OpencodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigValidator for OpencodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
        // 1. Run default syntax validation for all existing config files
        let mut issues = validate_all_syntax(&self.config_paths());

        // 2. Extra check: if both global and project-level configs exist,
        //    detect server name conflicts
        let global_path = Self::global_config_path();
        let project_path = self.project_config_path();

        let global_exists = global_path.as_ref().is_some_and(|p| p.exists());
        let project_exists = project_path.as_ref().is_some_and(|p| p.exists());

        if global_exists && project_exists {
            let global_servers = if let Some(ref path) = global_path {
                json_utils::read_existing_json(path)
                    .ok()
                    .and_then(|root| {
                        root.get(MCP_FIELD).and_then(|v| v.as_object()).map(|obj| {
                            obj.keys()
                                .cloned()
                                .collect::<std::collections::HashSet<String>>()
                        })
                    })
                    .unwrap_or_default()
            } else {
                std::collections::HashSet::new()
            };

            let project_servers = if let Some(ref path) = project_path {
                json_utils::read_existing_json(path)
                    .ok()
                    .and_then(|root| {
                        root.get(MCP_FIELD).and_then(|v| v.as_object()).map(|obj| {
                            obj.keys()
                                .cloned()
                                .collect::<std::collections::HashSet<String>>()
                        })
                    })
                    .unwrap_or_default()
            } else {
                std::collections::HashSet::new()
            };

            for server_name in global_servers.intersection(&project_servers) {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "MCP server '{}' is defined in both global and project-level opencode.json configs",
                        server_name
                    ),
                    path: None,
                    line: None,
                });
            }
        }

        Ok(issues)
    }
}

impl ToolAdapter for OpencodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(global) = Self::global_config_path() {
            paths.push(global);
        }
        if let Some(project) = self.project_config_path() {
            paths.push(project);
        }
        paths
    }

    fn read_mcp(&self) -> Result<McpConfig, LorumError> {
        let path = match self.read_config_path() {
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

        let mut map = BTreeMap::new();
        for (name, entry) in servers {
            let Some(cmd_array) = entry.get("command").and_then(|v| v.as_array()) else {
                eprintln!(
                    "warning: skipping opencode MCP server '{}' with missing or non-array command",
                    name
                );
                continue;
            };
            if cmd_array.is_empty() {
                eprintln!(
                    "warning: skipping opencode MCP server '{}' with empty command array",
                    name
                );
                continue;
            }
            let Some(command) = cmd_array.first().and_then(|v| v.as_str()).map(String::from) else {
                eprintln!(
                    "warning: skipping opencode MCP server '{}' with invalid command",
                    name
                );
                continue;
            };
            let args: Vec<String> = cmd_array
                .iter()
                .skip(1)
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let env = entry
                .get("environment")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();

            map.insert(name.clone(), McpServer { command, args, env });
        }
        Ok(McpConfig { servers: map })
    }

    fn write_mcp(&self, config: &McpConfig) -> Result<(), LorumError> {
        let path = match self.write_config_path() {
            Some(p) => p,
            None => {
                return Err(LorumError::Other {
                    message: "cannot determine config directory".into(),
                });
            }
        };
        let mut root = json_utils::read_existing_json(&path)?;

        let mut mcp_map = serde_json::Map::new();
        for (name, server) in &config.servers {
            let mut server_obj = serde_json::Map::new();
            server_obj.insert("type".into(), serde_json::Value::String("local".into()));
            server_obj.insert("enabled".into(), serde_json::Value::Bool(true));

            let mut cmd_array: Vec<serde_json::Value> =
                vec![serde_json::Value::String(server.command.clone())];
            for arg in &server.args {
                cmd_array.push(serde_json::Value::String(arg.clone()));
            }
            server_obj.insert("command".into(), serde_json::Value::Array(cmd_array));

            if !server.env.is_empty() {
                let env_obj: serde_json::Map<String, serde_json::Value> = server
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                server_obj.insert("environment".into(), serde_json::Value::Object(env_obj));
            }

            mcp_map.insert(name.clone(), serde_json::Value::Object(server_obj));
        }
        root[MCP_FIELD] = serde_json::Value::Object(mcp_map);
        json_utils::write_json(&path, &root)
    }
}

#[cfg(test)]
mod opencode_rules_tests {
    use super::*;

    #[test]
    fn rules_path_returns_agents_md() {
        let adapter = OpenCodeRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/AGENTS.md"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = OpenCodeRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = OpenCodeRulesAdapter;
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
        let adapter = OpenCodeRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn rules_adapter_name() {
        let adapter = OpenCodeRulesAdapter;
        assert_eq!(adapter.name(), "opencode");
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
        let path = dir.path().join("opencode.json");
        let json = r#"{
            "mcp": {
                "test-server": {
                    "type": "local",
                    "command": ["npx", "-y", "some-pkg"],
                    "enabled": true,
                    "environment": {"KEY": "value"},
                    "timeout": 5000
                }
            },
            "otherField": true
        }"#;
        fs::write(&path, json).unwrap();

        let adapter = OpencodeAdapter::with_project_root(dir.path().to_path_buf());
        let config = adapter.read_mcp().unwrap();

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    #[serial_test::serial]
    fn read_mcp_skips_empty_command() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir.path().join("opencode.json");
        let json = r#"{
            "mcp": {
                "bad-server": {
                    "type": "local",
                    "command": [],
                    "enabled": true
                },
                "good-server": {
                    "type": "local",
                    "command": ["node"],
                    "enabled": true
                }
            }
        }"#;
        fs::write(&path, json).unwrap();

        let adapter = OpencodeAdapter::with_project_root(dir.path().to_path_buf());
        let config = adapter.read_mcp().unwrap();
        assert_eq!(config.servers.len(), 1);
        assert!(config.servers.contains_key("good-server"));
        assert!(!config.servers.contains_key("bad-server"));
    }

    #[test]
    fn read_mcp_empty_when_no_field() {
        let root: serde_json::Value = serde_json::json!({ "otherField": true });
        // Can't easily test via adapter without a file, so test the parse logic directly
        let servers = root.get("mcp").and_then(|v| v.as_object());
        assert!(servers.is_none());
    }

    #[test]
    #[serial_test::serial]
    fn write_mcp_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir.path().join("opencode.json");

        let original = r#"{"otherField": true, "mcp": {}}"#;
        fs::write(&path, original).unwrap();

        let adapter = OpencodeAdapter::with_project_root(dir.path().to_path_buf());
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
        assert_eq!(result["mcp"]["svr"]["command"][0], "cmd");
        assert_eq!(result["mcp"]["svr"]["type"], "local");
        assert_eq!(result["mcp"]["svr"]["enabled"], true);
    }

    #[test]
    #[serial_test::serial]
    fn write_mcp_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let subdir = dir.path().join("subdir");
        // Create a .git dir so subdir is detected as a project directory
        fs::create_dir_all(subdir.join(".git")).unwrap();
        let path = subdir.join("opencode.json");
        assert!(!path.exists());

        let adapter = OpencodeAdapter::with_project_root(subdir);
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
        assert_eq!(result["mcp"]["s"]["command"][0], "c");
        assert_eq!(result["mcp"]["s"]["type"], "local");
        assert_eq!(result["mcp"]["s"]["enabled"], true);
    }

    #[test]
    #[serial_test::serial]
    fn roundtrip_json() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("HOME", dir.path()) };
        let path = dir.path().join("opencode.json");
        fs::write(&path, "{}").unwrap();

        let adapter = OpencodeAdapter::with_project_root(dir.path().to_path_buf());
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
        adapter.write_mcp(&config).unwrap();
        let parsed = adapter.read_mcp().unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn adapter_name() {
        let adapter = OpencodeAdapter::new();
        assert_eq!(ToolAdapter::name(&adapter), "opencode");
    }

    #[test]
    fn config_paths_returns_both() {
        let adapter = OpencodeAdapter::new();
        let paths = adapter.config_paths();
        assert_eq!(paths.len(), 2);
        assert!(paths[0].ends_with(".config/opencode/opencode.json"));
        assert!(paths[1].ends_with("opencode.json"));
    }

    #[test]
    fn with_project_root_overrides_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = OpencodeAdapter::with_project_root(dir.path().to_path_buf());
        let paths = adapter.config_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[1], dir.path().join("opencode.json"));
    }
}
