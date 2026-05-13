//! Integration tests for ContinueDevAdapter.
//!
//! These tests exercise the public `ToolAdapter` API for Continue.dev,
//! covering both YAML v1 and JSON deprecated formats, roundtrips,
//! format preference, non-MCP field preservation, and edge cases.

use std::collections::BTreeMap;
use std::fs;

use lorum::adapters::ToolAdapter;
use lorum::adapters::continue_dev::ContinueDevAdapter;
use lorum::config::{McpConfig, McpServer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_server(cmd: &str, args: &[&str], env: &[(&str, &str)]) -> McpServer {
    McpServer {
        command: cmd.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// T1 – YAML read/write roundtrip
// ---------------------------------------------------------------------------

#[test]
fn yaml_read_write_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let continue_dir = dir.path().join(".continue");
    fs::create_dir_all(&continue_dir).unwrap();

    let yaml_path = continue_dir.join("config.yaml");
    let yaml = r#"
mcpServers:
  - name: test-server
    command: npx
    args:
      - "-y"
      - "some-pkg"
    env:
      KEY: value
models:
  - title: GPT-4
prompts:
  - name: my-prompt
"#;
    fs::write(&yaml_path, yaml).unwrap();

    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let mcp = adapter.read_mcp().unwrap();

    assert_eq!(mcp.servers.len(), 1);
    let server = &mcp.servers["test-server"];
    assert_eq!(server.command, "npx");
    assert_eq!(server.args, vec!["-y", "some-pkg"]);
    assert_eq!(server.env.get("KEY").unwrap(), "value");

    // Modify config and write back.
    let mut modified = mcp.clone();
    modified.servers.insert(
        "new-server".into(),
        make_server("python", &["main.py"], &[("PORT", "3000")]),
    );
    adapter.write_mcp(&modified).unwrap();

    // Read again and verify roundtrip.
    let reparsed = adapter.read_mcp().unwrap();
    assert_eq!(reparsed.servers.len(), 2);
    assert_eq!(reparsed.servers["test-server"].command, "npx");
    assert_eq!(reparsed.servers["new-server"].command, "python");
    assert_eq!(reparsed.servers["new-server"].args, vec!["main.py"]);
    assert_eq!(
        reparsed.servers["new-server"].env.get("PORT").unwrap(),
        "3000"
    );
}

// ---------------------------------------------------------------------------
// T2 – JSON deprecated read/write roundtrip
// ---------------------------------------------------------------------------

#[test]
fn json_read_write_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let continue_dir = dir.path().join(".continue");
    fs::create_dir_all(&continue_dir).unwrap();

    let json_path = continue_dir.join("config.json");
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
    fs::write(&json_path, json).unwrap();

    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let mcp = adapter.read_mcp().unwrap();

    assert_eq!(mcp.servers.len(), 1);
    let server = &mcp.servers["test-server"];
    assert_eq!(server.command, "npx");
    assert_eq!(server.args, vec!["-y", "some-pkg"]);
    assert_eq!(server.env.get("KEY").unwrap(), "value");

    // Modify config and write back.
    let mut modified = mcp.clone();
    modified.servers.insert(
        "new-server".into(),
        make_server("python", &["main.py"], &[("PORT", "3000")]),
    );
    adapter.write_mcp(&modified).unwrap();

    // Read again and verify roundtrip.
    let reparsed = adapter.read_mcp().unwrap();
    assert_eq!(reparsed.servers.len(), 2);
    assert_eq!(reparsed.servers["test-server"].command, "npx");
    assert_eq!(reparsed.servers["new-server"].command, "python");
    assert_eq!(reparsed.servers["new-server"].args, vec!["main.py"]);
    assert_eq!(
        reparsed.servers["new-server"].env.get("PORT").unwrap(),
        "3000"
    );

    // Verify transport wrapper is present in the raw JSON.
    let raw = fs::read_to_string(&json_path).unwrap();
    let root: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let servers = root["experimental"]["modelContextProtocolServers"]
        .as_array()
        .unwrap();
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0]["transport"]["type"], "stdio");
    assert_eq!(servers[1]["transport"]["type"], "stdio");
}

// ---------------------------------------------------------------------------
// T3 – YAML format preferred over JSON when both exist
// ---------------------------------------------------------------------------

#[test]
fn yaml_format_preferred_over_json() {
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
    let mcp = adapter.read_mcp().unwrap();

    assert_eq!(mcp.servers.len(), 1);
    assert!(mcp.servers.contains_key("yaml-server"));
    assert!(!mcp.servers.contains_key("json-server"));
}

// ---------------------------------------------------------------------------
// T4 – Preserves non-MCP fields in YAML
// ---------------------------------------------------------------------------

#[test]
fn preserves_non_mcp_fields_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let continue_dir = dir.path().join(".continue");
    fs::create_dir_all(&continue_dir).unwrap();

    let yaml_path = continue_dir.join("config.yaml");
    let yaml = r#"
mcpServers:
  - name: test-server
    command: npx
    args: ["-y", "pkg"]
models:
  - title: GPT-4
prompts:
  - name: my-prompt
"#;
    fs::write(&yaml_path, yaml).unwrap();

    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let mcp = adapter.read_mcp().unwrap();

    // Write back (even with same content) and verify non-MCP fields remain.
    adapter.write_mcp(&mcp).unwrap();

    let raw = fs::read_to_string(&yaml_path).unwrap();
    let root: serde_yaml::Value = serde_yaml::from_str(&raw).unwrap();
    assert!(root.get("models").is_some());
    assert!(root.get("prompts").is_some());

    let models = root["models"].as_sequence().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(
        models[0].get("title").and_then(|v| v.as_str()),
        Some("GPT-4")
    );

    let prompts = root["prompts"].as_sequence().unwrap();
    assert_eq!(prompts.len(), 1);
    assert_eq!(
        prompts[0].get("name").and_then(|v| v.as_str()),
        Some("my-prompt")
    );
}

// ---------------------------------------------------------------------------
// T5 – Preserves non-MCP fields in JSON
// ---------------------------------------------------------------------------

#[test]
fn preserves_non_mcp_fields_json() {
    let dir = tempfile::tempdir().unwrap();
    let continue_dir = dir.path().join(".continue");
    fs::create_dir_all(&continue_dir).unwrap();

    let json_path = continue_dir.join("config.json");
    let json = r#"{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "name": "test-server",
        "transport": {
          "type": "stdio",
          "command": "npx",
          "args": ["-y", "pkg"]
        }
      }
    ]
  },
  "otherField": true
}"#;
    fs::write(&json_path, json).unwrap();

    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let mcp = adapter.read_mcp().unwrap();

    // Write back and verify non-MCP fields remain.
    adapter.write_mcp(&mcp).unwrap();

    let raw = fs::read_to_string(&json_path).unwrap();
    let root: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(root["otherField"], true);
    assert!(root["experimental"].is_object());
}

// ---------------------------------------------------------------------------
// T6 – Synthetic name for missing name field
// ---------------------------------------------------------------------------

#[test]
fn synthetic_name_for_missing_name() {
    let dir = tempfile::tempdir().unwrap();
    let continue_dir = dir.path().join(".continue");
    fs::create_dir_all(&continue_dir).unwrap();

    let yaml_path = continue_dir.join("config.yaml");
    let yaml = r#"
mcpServers:
  - command: npx
    args: ["-y", "some-pkg"]
"#;
    fs::write(&yaml_path, yaml).unwrap();

    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let mcp = adapter.read_mcp().unwrap();

    assert_eq!(mcp.servers.len(), 1);
    assert!(mcp.servers.contains_key("unnamed-server-0"));
    assert_eq!(mcp.servers["unnamed-server-0"].command, "npx");
}

// ---------------------------------------------------------------------------
// T7 – Defaults to YAML when no file exists
// ---------------------------------------------------------------------------

#[test]
fn defaults_to_yaml_when_no_file_exists() {
    let dir = tempfile::tempdir().unwrap();
    // No .continue directory or config files created.

    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let config = McpConfig {
        servers: {
            let mut m = BTreeMap::new();
            m.insert(
                "test-server".into(),
                make_server("npx", &["-y", "pkg"], &[]),
            );
            m
        },
    };
    adapter.write_mcp(&config).unwrap();

    let yaml_path = dir.path().join(".continue").join("config.yaml");
    assert!(yaml_path.exists());

    let reparsed = adapter.read_mcp().unwrap();
    assert_eq!(reparsed.servers.len(), 1);
    assert!(reparsed.servers.contains_key("test-server"));
}

// ---------------------------------------------------------------------------
// T8 – config_paths returns all four paths
// ---------------------------------------------------------------------------

#[test]
fn config_paths_returns_all_four_paths() {
    let dir = tempfile::tempdir().unwrap();
    let adapter = ContinueDevAdapter::with_project_root(dir.path().to_path_buf());
    let paths = adapter.config_paths();

    assert_eq!(paths.len(), 4);
    assert_eq!(paths[0], dir.path().join(".continue").join("config.yaml"));
    assert!(paths[1].ends_with(".continue/config.yaml"));
    assert_eq!(paths[2], dir.path().join(".continue").join("config.json"));
    assert!(paths[3].ends_with(".continue/config.json"));
}
