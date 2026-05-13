//! Continue.dev adapter for reading/writing MCP configuration.
//!
//! Continue.dev supports both YAML v1 and JSON (deprecated) formats, each
//! with project-level and global variants.
//!
//! **YAML v1** (preferred):
//! ```yaml
//! mcpServers:
//!   - name: server-name
//!     command: npx
//!     args: ["-y", "some-pkg"]
//!     env:
//!       KEY: value
//! ```
//!
//! **JSON** (deprecated):
//! ```json
//! {
//!   "experimental": {
//!     "modelContextProtocolServers": [
//!       {
//!         "name": "server-name",
//!         "transport": {
//!           "type": "stdio",
//!           "command": "npx",
//!           "args": ["-y", "some-pkg"]
//!         }
//!       }
//!     ]
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::{ToolAdapter, json_utils};
use crate::config::{McpConfig, McpServer};
use crate::error::LorumError;

/// Adapter for Continue.dev.
///
/// Reads and writes MCP server configurations from Continue.dev's
/// configuration files, supporting both YAML v1 and JSON deprecated formats
/// at project-level and global locations.
pub struct ContinueDevAdapter {
    project_root: Option<PathBuf>,
}

/// Field name used by Continue.dev YAML v1 for MCP servers.
const YAML_MCP_FIELD: &str = "mcpServers";
/// Field name used by Continue.dev JSON deprecated format.
const JSON_MCP_FIELD: &str = "modelContextProtocolServers";
/// Top-level experimental field for JSON deprecated format.
const JSON_EXPERIMENTAL_FIELD: &str = "experimental";

impl ContinueDevAdapter {
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

    /// Returns the project-level Continue.dev YAML config path.
    fn project_yaml_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".continue").join("config.yaml"))
    }

    /// Returns the global Continue.dev YAML config path.
    fn global_yaml_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".continue").join("config.yaml"))
    }

    /// Returns the project-level Continue.dev JSON config path.
    fn project_json_path(&self) -> Option<PathBuf> {
        let root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())?;
        Some(root.join(".continue").join("config.json"))
    }

    /// Returns the global Continue.dev JSON config path.
    fn global_json_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".continue").join("config.json"))
    }

    /// Detect which format is currently in use by checking existing files.
    ///
    /// Returns the first existing file in priority order, or defaults to
    /// the project-level YAML path if none exist.
    fn detect_format(&self) -> (PathBuf, ConfigFormat) {
        let candidates = [
            (self.project_yaml_path(), ConfigFormat::Yaml),
            (Self::global_yaml_path(), ConfigFormat::Yaml),
            (self.project_json_path(), ConfigFormat::Json),
            (Self::global_json_path(), ConfigFormat::Json),
        ];

        for (path, format) in candidates {
            if let Some(ref p) = path {
                if p.exists() {
                    return (p.clone(), format);
                }
            }
        }

        // Default: project-level YAML
        (
            self.project_yaml_path()
                .unwrap_or_else(|| PathBuf::from(".continue/config.yaml")),
            ConfigFormat::Yaml,
        )
    }
}

impl Default for ContinueDevAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration format used by Continue.dev.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigFormat {
    /// YAML v1 format with `mcpServers` array.
    Yaml,
    /// JSON deprecated format with `experimental.modelContextProtocolServers`.
    Json,
}

impl ToolAdapter for ContinueDevAdapter {
    fn name(&self) -> &str {
        "continue"
    }

    fn config_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::with_capacity(4);
        if let Some(p) = self.project_yaml_path() {
            paths.push(p);
        }
        if let Some(p) = Self::global_yaml_path() {
            paths.push(p);
        }
        if let Some(p) = self.project_json_path() {
            paths.push(p);
        }
        if let Some(p) = Self::global_json_path() {
            paths.push(p);
        }
        paths
    }

    fn read_mcp(&self) -> Result<McpConfig, LorumError> {
        let candidates = [
            (self.project_yaml_path(), ConfigFormat::Yaml),
            (Self::global_yaml_path(), ConfigFormat::Yaml),
            (self.project_json_path(), ConfigFormat::Json),
            (Self::global_json_path(), ConfigFormat::Json),
        ];

        for (path, format) in candidates {
            let Some(p) = path else { continue };
            if !p.exists() {
                continue;
            }
            return match format {
                ConfigFormat::Yaml => read_mcp_from_yaml(&p),
                ConfigFormat::Json => read_mcp_from_json(&p),
            };
        }

        Ok(McpConfig::default())
    }

    fn write_mcp(&self, config: &McpConfig) -> Result<(), LorumError> {
        let (path, format) = self.detect_format();

        match format {
            ConfigFormat::Yaml => write_mcp_to_yaml(&path, config),
            ConfigFormat::Json => write_mcp_to_json(&path, config),
        }
    }
}

/// Read MCP servers from a Continue.dev YAML v1 config file.
///
/// The YAML format stores `mcpServers` as an array of server objects,
/// where each object has a `name` field used as the map key.
fn read_mcp_from_yaml(path: &Path) -> Result<McpConfig, LorumError> {
    let contents = std::fs::read_to_string(path)?;
    let root: serde_yaml::Value =
        serde_yaml::from_str(&contents).map_err(|e| LorumError::ConfigParse {
            format: "yaml".into(),
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;

    let Some(servers) = root.get(YAML_MCP_FIELD).and_then(|v| v.as_sequence()) else {
        return Ok(McpConfig::default());
    };

    let mut map = BTreeMap::new();
    for (index, entry) in servers.iter().enumerate() {
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("unnamed-server-{index}"));

        if let Some(server) = parse_mcp_server_from_yaml_value(entry) {
            map.insert(name, server);
        }
    }

    Ok(McpConfig { servers: map })
}

/// Parse a single MCP server entry from a YAML value.
fn parse_mcp_server_from_yaml_value(value: &serde_yaml::Value) -> Option<McpServer> {
    let obj = value.as_mapping()?;
    let command = obj
        .get(serde_yaml::Value::String("command".into()))
        .and_then(|v| v.as_str())?
        .to_string();
    let args = obj
        .get(serde_yaml::Value::String("args".into()))
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let env = obj
        .get(serde_yaml::Value::String("env".into()))
        .and_then(|v| v.as_mapping())
        .map(|mapping| {
            mapping
                .iter()
                .filter_map(|(k, v)| {
                    k.as_str()
                        .and_then(|key| v.as_str().map(|val| (key.to_string(), val.to_string())))
                })
                .collect()
        })
        .unwrap_or_default();

    Some(McpServer { command, args, env })
}

/// Read MCP servers from a Continue.dev JSON deprecated config file.
///
/// The JSON format stores servers under `experimental.modelContextProtocolServers`
/// as an array, where each element has a `transport` field wrapping the actual
/// server configuration.
fn read_mcp_from_json(path: &Path) -> Result<McpConfig, LorumError> {
    let root = json_utils::read_existing_json(path)?;

    let Some(servers) = root
        .get(JSON_EXPERIMENTAL_FIELD)
        .and_then(|v| v.get(JSON_MCP_FIELD))
        .and_then(|v| v.as_array())
    else {
        return Ok(McpConfig::default());
    };

    let mut map = BTreeMap::new();
    for (index, entry) in servers.iter().enumerate() {
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("unnamed-server-{index}"));

        // The transport field wraps the actual server config; fall back to entry itself
        let transport = entry.get("transport").unwrap_or(entry);
        if let Some(server) = json_utils::parse_mcp_server(transport) {
            map.insert(name, server);
        }
    }

    Ok(McpConfig { servers: map })
}

/// Write MCP servers to a Continue.dev YAML v1 config file.
///
/// Preserves non-MCP fields in the existing config. Servers are written
/// as an array under the `mcpServers` key.
fn write_mcp_to_yaml(path: &Path, config: &McpConfig) -> Result<(), LorumError> {
    let mut root = if path.exists() {
        let contents = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&contents).map_err(|e| LorumError::ConfigParse {
            format: "yaml".into(),
            path: path.to_path_buf(),
            source: Box::new(e),
        })?
    } else {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    };

    let servers_array: Vec<serde_yaml::Value> = config
        .servers
        .iter()
        .map(|(name, server)| mcp_server_to_yaml_value(name, server))
        .collect();

    root.as_mapping_mut()
        .ok_or_else(|| LorumError::Other {
            message: format!("expected mapping at root of {}", path.display()),
        })?
        .insert(
            serde_yaml::Value::String(YAML_MCP_FIELD.into()),
            serde_yaml::Value::Sequence(servers_array),
        );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
            path: path.to_path_buf(),
            source: e,
        })?;
    }

    let formatted = serde_yaml::to_string(&root).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    std::fs::write(path, formatted).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: e,
    })?;

    Ok(())
}

/// Convert a single McpServer to a YAML value for Continue.dev format.
fn mcp_server_to_yaml_value(name: &str, server: &McpServer) -> serde_yaml::Value {
    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(
        serde_yaml::Value::String("name".into()),
        serde_yaml::Value::String(name.into()),
    );
    mapping.insert(
        serde_yaml::Value::String("command".into()),
        serde_yaml::Value::String(server.command.clone()),
    );
    mapping.insert(
        serde_yaml::Value::String("args".into()),
        serde_yaml::Value::Sequence(
            server
                .args
                .iter()
                .map(|a| serde_yaml::Value::String(a.clone()))
                .collect(),
        ),
    );
    if !server.env.is_empty() {
        let env_mapping: serde_yaml::Mapping = server
            .env
            .iter()
            .map(|(k, v)| {
                (
                    serde_yaml::Value::String(k.clone()),
                    serde_yaml::Value::String(v.clone()),
                )
            })
            .collect();
        mapping.insert(
            serde_yaml::Value::String("env".into()),
            serde_yaml::Value::Mapping(env_mapping),
        );
    }
    serde_yaml::Value::Mapping(mapping)
}

/// Write MCP servers to a Continue.dev JSON deprecated config file.
///
/// Preserves non-MCP fields in the existing config. Servers are written
/// under `experimental.modelContextProtocolServers` with each entry
/// wrapped in a `transport` object.
fn write_mcp_to_json(path: &Path, config: &McpConfig) -> Result<(), LorumError> {
    let mut root = json_utils::read_existing_json(path)?;

    let servers_array: Vec<serde_json::Value> = config
        .servers
        .iter()
        .map(|(name, server)| {
            let mut transport = serde_json::Map::new();
            transport.insert("type".into(), serde_json::Value::String("stdio".into()));
            transport.insert(
                "command".into(),
                serde_json::Value::String(server.command.clone()),
            );
            transport.insert(
                "args".into(),
                serde_json::Value::Array(
                    server
                        .args
                        .iter()
                        .map(|a| serde_json::Value::String(a.clone()))
                        .collect(),
                ),
            );
            if !server.env.is_empty() {
                let env_obj: serde_json::Map<String, serde_json::Value> = server
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                transport.insert("env".into(), serde_json::Value::Object(env_obj));
            }

            let mut entry = serde_json::Map::new();
            entry.insert("name".into(), serde_json::Value::String(name.clone()));
            entry.insert("transport".into(), serde_json::Value::Object(transport));
            serde_json::Value::Object(entry)
        })
        .collect();

    let experimental = root
        .as_object_mut()
        .ok_or_else(|| LorumError::Other {
            message: format!("expected object at root of {}", path.display()),
        })?
        .entry(JSON_EXPERIMENTAL_FIELD)
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| LorumError::Other {
            message: format!(
                "expected object for '{}' at {}",
                JSON_EXPERIMENTAL_FIELD,
                path.display()
            ),
        })?;
    experimental.insert(
        JSON_MCP_FIELD.into(),
        serde_json::Value::Array(servers_array),
    );

    json_utils::write_json(path, &root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_utils::make_server;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn read_mcp_from_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let yaml = r#"
mcpServers:
  - name: test-server
    command: npx
    args:
      - "-y"
      - "some-pkg"
    env:
      KEY: value
otherField: true
"#;
        fs::write(&path, yaml).unwrap();

        let config = read_mcp_from_yaml(&path).unwrap();

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    fn read_mcp_from_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let json = r#"{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "name": "test-server",
        "transport": {
          "type": "stdio",
          "command": "npx",
          "args": ["-y", "some-pkg"],
          "env": { "KEY": "value" }
        }
      }
    ]
  },
  "otherField": true
}"#;
        fs::write(&path, json).unwrap();

        let config = read_mcp_from_json(&path).unwrap();

        assert_eq!(config.servers.len(), 1);
        let server = &config.servers["test-server"];
        assert_eq!(server.command, "npx");
        assert_eq!(server.args, vec!["-y", "some-pkg"]);
        assert_eq!(server.env.get("KEY").unwrap(), "value");
    }

    #[test]
    fn read_mcp_empty_when_no_field_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "otherField: true\n").unwrap();

        let config = read_mcp_from_yaml(&path).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn read_mcp_empty_when_no_field_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"otherField": true}"#).unwrap();

        let config = read_mcp_from_json(&path).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn read_mcp_generates_synthetic_name_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let yaml = r#"
mcpServers:
  - command: npx
    args: ["-y", "some-pkg"]
"#;
        fs::write(&path, yaml).unwrap();

        let config = read_mcp_from_yaml(&path).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert!(config.servers.contains_key("unnamed-server-0"));
    }

    #[test]
    fn write_mcp_preserves_other_fields_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let original = "otherField: true\nmcpServers: []\n";
        fs::write(&path, original).unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("svr".into(), make_server("cmd", &["a"], &[("K", "V")]));
                m
            },
        };
        write_mcp_to_yaml(&path, &config).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        let root: serde_yaml::Value = serde_yaml::from_str(&result).unwrap();
        assert_eq!(root.get("otherField").and_then(|v| v.as_bool()), Some(true));
        let servers = root
            .get(YAML_MCP_FIELD)
            .and_then(|v| v.as_sequence())
            .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].get("name").and_then(|v| v.as_str()), Some("svr"));
        assert_eq!(
            servers[0].get("command").and_then(|v| v.as_str()),
            Some("cmd")
        );
    }

    #[test]
    fn write_mcp_preserves_other_fields_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let original =
            r#"{"otherField": true, "experimental": {"modelContextProtocolServers": []}}"#;
        fs::write(&path, original).unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("svr".into(), make_server("cmd", &["a"], &[("K", "V")]));
                m
            },
        };
        write_mcp_to_json(&path, &config).unwrap();

        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(result["otherField"], true);
        let servers = result[JSON_EXPERIMENTAL_FIELD][JSON_MCP_FIELD]
            .as_array()
            .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["name"], "svr");
        assert_eq!(servers[0]["transport"]["command"], "cmd");
        assert_eq!(servers[0]["transport"]["type"], "stdio");
    }

    #[test]
    fn write_mcp_creates_file_when_missing_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".continue").join("config.yaml");
        assert!(!path.exists());

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        write_mcp_to_yaml(&path, &config).unwrap();

        assert!(path.exists());
        let result = fs::read_to_string(&path).unwrap();
        let root: serde_yaml::Value = serde_yaml::from_str(&result).unwrap();
        let servers = root
            .get(YAML_MCP_FIELD)
            .and_then(|v| v.as_sequence())
            .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].get("name").and_then(|v| v.as_str()), Some("s"));
        assert_eq!(
            servers[0].get("command").and_then(|v| v.as_str()),
            Some("c")
        );
    }

    #[test]
    fn write_mcp_creates_file_when_missing_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".continue").join("config.json");
        assert!(!path.exists());

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        write_mcp_to_json(&path, &config).unwrap();

        assert!(path.exists());
        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let servers = result[JSON_EXPERIMENTAL_FIELD][JSON_MCP_FIELD]
            .as_array()
            .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["name"], "s");
        assert_eq!(servers[0]["transport"]["command"], "c");
        assert_eq!(servers[0]["transport"]["type"], "stdio");
    }

    #[test]
    fn roundtrip_yaml() {
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

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        write_mcp_to_yaml(&path, &config).unwrap();
        let parsed = read_mcp_from_yaml(&path).unwrap();
        assert_eq!(config, parsed);
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

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        write_mcp_to_json(&path, &config).unwrap();
        let parsed = read_mcp_from_json(&path).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn adapter_name() {
        let adapter = ContinueDevAdapter::new();
        assert_eq!(adapter.name(), "continue");
    }

    #[test]
    fn with_project_root_overrides_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
        let paths = adapter.config_paths();
        assert_eq!(paths.len(), 4);
        assert_eq!(paths[0], dir.path().join(".continue").join("config.yaml"));
        assert_eq!(paths[2], dir.path().join(".continue").join("config.json"));
    }

    #[test]
    fn adapter_read_mcp_prefers_yaml_over_json() {
        let dir = tempfile::tempdir().unwrap();
        let continue_dir = dir.path().join(".continue");
        fs::create_dir_all(&continue_dir).unwrap();

        let yaml_path = continue_dir.join("config.yaml");
        let yaml = r#"
mcpServers:
  - name: yaml-server
    command: npx
    args: ["-y", "yaml-pkg"]
"#;
        fs::write(&yaml_path, yaml).unwrap();

        let json_path = continue_dir.join("config.json");
        let json = r#"{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "name": "json-server",
        "transport": {
          "type": "stdio",
          "command": "npx",
          "args": ["-y", "json-pkg"]
        }
      }
    ]
  }
}"#;
        fs::write(&json_path, json).unwrap();

        let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
        let config = adapter.read_mcp().unwrap();
        assert_eq!(config.servers.len(), 1);
        assert!(config.servers.contains_key("yaml-server"));
    }

    #[test]
    fn adapter_detect_format_defaults_to_project_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
        let (path, format) = adapter.detect_format();
        assert_eq!(path, dir.path().join(".continue").join("config.yaml"));
        assert_eq!(format, ConfigFormat::Yaml);
    }

    #[test]
    fn adapter_detect_format_finds_existing_json() {
        let dir = tempfile::tempdir().unwrap();
        let continue_dir = dir.path().join(".continue");
        fs::create_dir_all(&continue_dir).unwrap();
        let json_path = continue_dir.join("config.json");
        fs::write(&json_path, "{}").unwrap();

        let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
        let (path, format) = adapter.detect_format();
        assert_eq!(path, json_path);
        assert_eq!(format, ConfigFormat::Json);
    }

    #[test]
    fn adapter_write_mcp_defaults_to_yaml_when_no_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        adapter.write_mcp(&config).unwrap();

        let yaml_path = dir.path().join(".continue").join("config.yaml");
        assert!(yaml_path.exists());
        let parsed = read_mcp_from_yaml(&yaml_path).unwrap();
        assert_eq!(parsed.servers.len(), 1);
        assert!(parsed.servers.contains_key("s"));
    }

    #[test]
    fn adapter_write_mcp_keeps_json_format_when_json_exists() {
        let dir = tempfile::tempdir().unwrap();
        let continue_dir = dir.path().join(".continue");
        fs::create_dir_all(&continue_dir).unwrap();
        let json_path = continue_dir.join("config.json");
        fs::write(&json_path, "{}").unwrap();

        let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        adapter.write_mcp(&config).unwrap();

        assert!(json_path.exists());
        let parsed = read_mcp_from_json(&json_path).unwrap();
        assert_eq!(parsed.servers.len(), 1);
        assert!(parsed.servers.contains_key("s"));
    }

    #[test]
    fn read_mcp_from_yaml_errors_on_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "mcpServers: [unclosed").unwrap();

        let result = read_mcp_from_yaml(&path);
        assert!(matches!(result, Err(LorumError::ConfigParse { .. })));
    }

    #[test]
    fn write_yaml_errors_when_root_not_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "[1, 2, 3]\n").unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let result = write_mcp_to_yaml(&path, &config);
        assert!(matches!(result, Err(LorumError::Other { .. })));
        if let Err(LorumError::Other { message }) = result {
            assert!(message.contains("expected mapping at root"));
        }
    }

    #[test]
    fn write_yaml_errors_when_dir_creation_fails() {
        // Use /dev/null as a parent to trigger directory creation failure
        let path = PathBuf::from("/dev/null/config.yaml");
        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let result = write_mcp_to_yaml(&path, &config);
        assert!(matches!(result, Err(LorumError::ConfigWrite { .. })));
    }

    #[test]
    #[allow(clippy::permissions_set_readonly_false)]
    fn write_yaml_errors_when_file_not_writable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "existing: true\n").unwrap();

        // Make file read-only
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let result = write_mcp_to_yaml(&path, &config);

        // Restore permissions so tempdir can clean up
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(false);
        let _ = fs::set_permissions(&path, perms);

        assert!(matches!(result, Err(LorumError::ConfigWrite { .. })));
    }

    #[test]
    fn write_json_errors_when_root_not_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "[1, 2, 3]").unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let result = write_mcp_to_json(&path, &config);
        assert!(matches!(result, Err(LorumError::Other { .. })));
        if let Err(LorumError::Other { message }) = result {
            assert!(message.contains("expected object at root"));
        }
    }

    #[test]
    fn write_json_errors_when_experimental_not_object() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"experimental": 42}"#).unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let result = write_mcp_to_json(&path, &config);
        assert!(matches!(result, Err(LorumError::Other { .. })));
        if let Err(LorumError::Other { message }) = result {
            assert!(message.contains("expected object for 'experimental'"));
        }
    }

    #[test]
    fn detect_format_fallback_when_no_project_root_and_no_files() {
        // Create adapter with no project_root, simulating a scenario where
        // current dir has no .continue config. The fallback should return
        // a project-level YAML path relative to cwd.
        let adapter = ContinueDevAdapter::new();
        let (path, format) = adapter.detect_format();
        // When no files exist, it falls back to project_yaml_path which uses cwd
        assert!(path.to_string_lossy().ends_with(".continue/config.yaml"));
        assert_eq!(format, ConfigFormat::Yaml);
    }

    #[test]
    fn write_yaml_errors_on_invalid_existing_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "{{invalid").unwrap();

        let config = McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("s".into(), make_server("c", &[], &[]));
                m
            },
        };
        let result = write_mcp_to_yaml(&path, &config);
        assert!(matches!(result, Err(LorumError::ConfigParse { .. })));
    }

    #[test]
    #[cfg(unix)]
    fn detect_format_fallback_when_current_dir_removed() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&subdir).unwrap();

        // Remove the directory while it is the current working directory.
        // On Unix this succeeds and subsequent getcwd() calls fail.
        fs::remove_dir(&subdir).unwrap();

        let adapter = ContinueDevAdapter::new();
        let (path, format) = adapter.detect_format();
        assert_eq!(path, PathBuf::from(".continue/config.yaml"));
        assert_eq!(format, ConfigFormat::Yaml);

        // Restore cwd so the test runner doesn't break.
        let _ = std::env::set_current_dir(&original_dir);
    }
}
