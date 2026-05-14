//! Import integration tests: verify adapters can read configs and the
//! results can be saved via the public config API.

use serial_test::serial;
use std::collections::BTreeMap;
use std::fs;

use lorum::adapters::{all_adapters, find_adapter, json_utils, toml_utils};
use lorum::config::{LorumConfig, load_config, save_config};

// ---------------------------------------------------------------------------
// Import from JSON adapter (Claude Code format)
// ---------------------------------------------------------------------------

#[test]
fn import_from_json_adapter() {
    let dir = tempfile::tempdir().unwrap();
    let lorum_path = dir.path().join("config.yaml");

    // Create a JSON file in Claude Code format.
    let json_path = dir.path().join("settings.json");
    let json_content = r#"{
        "mcpServers": {
            "fetch": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-fetch"],
                "env": {}
            },
            "github": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-github"],
                "env": {
                    "GITHUB_TOKEN": "${GITHUB_PERSONAL_ACCESS_TOKEN}"
                }
            }
        },
        "otherSetting": true
    }"#;
    fs::write(&json_path, json_content).unwrap();

    // Parse the JSON using the public utility.
    let root: serde_json::Value = serde_json::from_str(json_content).unwrap();
    let mcp = json_utils::parse_mcp_servers(&root, "mcpServers");

    assert_eq!(mcp.servers.len(), 2);
    assert_eq!(mcp.servers["fetch"].command, "npx");
    assert_eq!(
        mcp.servers["fetch"].args,
        vec!["-y", "@modelcontextprotocol/server-fetch"]
    );
    assert_eq!(
        mcp.servers["github"].env.get("GITHUB_TOKEN").unwrap(),
        "${GITHUB_PERSONAL_ACCESS_TOKEN}"
    );

    // Now save the imported servers into a lorum config.
    let mut lorum_config = LorumConfig::default();
    for (name, server) in &mcp.servers {
        lorum_config
            .mcp
            .servers
            .insert(name.clone(), server.clone());
    }
    save_config(&lorum_path, &lorum_config).unwrap();

    // Verify the lorum config round-trips correctly.
    let loaded = load_config(&lorum_path).unwrap();
    assert_eq!(loaded.mcp.servers.len(), 2);
    assert!(loaded.mcp.servers.contains_key("fetch"));
    assert!(loaded.mcp.servers.contains_key("github"));
}

// ---------------------------------------------------------------------------
// Import from TOML adapter (Codex format)
// ---------------------------------------------------------------------------

#[test]
fn import_from_toml_adapter() {
    let dir = tempfile::tempdir().unwrap();
    let lorum_path = dir.path().join("config.yaml");

    // Create a TOML file in Codex format.
    let toml_path = dir.path().join("config.toml");
    let toml_content = r#"
other_field = "preserved"

[mcp_servers.sequential-thinking]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-sequential-thinking"]

[mcp_servers.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]

[mcp_servers.memory.env]
DATA_DIR = "/tmp/memory"
"#;
    fs::write(&toml_path, toml_content).unwrap();

    // Parse the TOML using the public utility.
    let root: toml::Value = toml::from_str(toml_content).unwrap();
    let mcp = toml_utils::parse_mcp_servers_toml(&root, "mcp_servers");

    assert_eq!(mcp.servers.len(), 2);
    assert_eq!(mcp.servers["sequential-thinking"].command, "npx");
    assert_eq!(
        mcp.servers["sequential-thinking"].args,
        vec!["-y", "@modelcontextprotocol/server-sequential-thinking"]
    );
    assert_eq!(
        mcp.servers["memory"].env.get("DATA_DIR").unwrap(),
        "/tmp/memory"
    );

    // Save into lorum config.
    let mut lorum_config = LorumConfig::default();
    for (name, server) in &mcp.servers {
        lorum_config
            .mcp
            .servers
            .insert(name.clone(), server.clone());
    }
    save_config(&lorum_path, &lorum_config).unwrap();

    let loaded = load_config(&lorum_path).unwrap();
    assert_eq!(loaded.mcp.servers.len(), 2);
    assert!(loaded.mcp.servers.contains_key("sequential-thinking"));
    assert!(loaded.mcp.servers.contains_key("memory"));
}

// ---------------------------------------------------------------------------
// Verify all_adapters returns all 5 registered adapters
// ---------------------------------------------------------------------------

#[test]
fn all_adapters_registered() {
    let adapters = all_adapters();
    assert_eq!(adapters.len(), 9);

    let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"codex"));
    assert!(names.contains(&"continue"));
    assert!(names.contains(&"cursor"));
    assert!(names.contains(&"proma"));
    assert!(names.contains(&"kimi"));
    assert!(names.contains(&"opencode"));
    assert!(names.contains(&"trae"));
    assert!(names.contains(&"windsurf"));
}

// ---------------------------------------------------------------------------
// Verify find_adapter works for known names
// ---------------------------------------------------------------------------

#[test]
fn find_adapter_by_name() {
    assert!(find_adapter("claude-code").is_some());
    assert!(find_adapter("codex").is_some());
    assert!(find_adapter("cursor").is_some());
    assert!(find_adapter("proma").is_some());
    assert!(find_adapter("kimi").is_some());
    assert!(find_adapter("opencode").is_some());
    assert!(find_adapter("trae").is_some());
    assert!(find_adapter("windsurf").is_some());
    assert!(find_adapter("nonexistent").is_none());
}

// ---------------------------------------------------------------------------
// Verify read_mcp returns empty config when tool config doesn't exist
// ---------------------------------------------------------------------------

#[test]
fn read_mcp_returns_empty_when_no_config() {
    for adapter in all_adapters() {
        let result = adapter.read_mcp();
        assert!(
            result.is_ok(),
            "adapter '{}' returned error: {:?}",
            adapter.name(),
            result
        );
        let mcp = result.unwrap();
        // Configs may or may not exist on the test machine, but read_mcp
        // should always succeed (returning empty when file is missing).
        // We just verify it doesn't panic or return Err.
        let _ = mcp.servers.len();
    }
}

// ---------------------------------------------------------------------------
// Import from TOML adapter (Kimi format with nested mcp.client)
// ---------------------------------------------------------------------------

#[test]
fn import_from_kimi_toml_format() {
    let dir = tempfile::tempdir().unwrap();
    let lorum_path = dir.path().join("config.yaml");

    // Create a TOML file in Kimi format.
    let toml_path = dir.path().join("config.toml");
    let toml_content = r#"
[mcp.client.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp@latest"]

[mcp.client.context7.env]
API_KEY = "test-key"
"#;
    fs::write(&toml_path, toml_content).unwrap();

    // Parse through the Kimi nested structure using public toml_utils.
    let root: toml::Value = toml::from_str(toml_content).unwrap();
    let servers = root
        .get("mcp")
        .and_then(|v| v.get("client"))
        .and_then(|v| v.as_table())
        .expect("expected mcp.client table");

    let mut mcp_servers = BTreeMap::new();
    for (name, value) in servers {
        if let Some(server) = toml_utils::parse_mcp_server_toml(value.as_table()) {
            mcp_servers.insert(name.clone(), server);
        }
    }

    assert_eq!(mcp_servers.len(), 1);
    assert_eq!(mcp_servers["context7"].command, "npx");
    assert_eq!(
        mcp_servers["context7"].env.get("API_KEY").unwrap(),
        "test-key"
    );

    // Save to lorum config.
    let mut lorum_config = LorumConfig::default();
    for (name, server) in &mcp_servers {
        lorum_config
            .mcp
            .servers
            .insert(name.clone(), server.clone());
    }
    save_config(&lorum_path, &lorum_config).unwrap();

    let loaded = load_config(&lorum_path).unwrap();
    assert!(loaded.mcp.servers.contains_key("context7"));
}

// ---------------------------------------------------------------------------
// Verify all_adapter_tool_names returns union of all dimensions
// ---------------------------------------------------------------------------

#[test]
fn all_adapter_tool_names_returns_union() {
    let names = lorum::adapters::all_adapter_tool_names();
    // Should include MCP adapters
    assert!(names.iter().any(|n| n == "claude-code"));
    assert!(names.iter().any(|n| n == "codex"));
    // Should include rules adapters
    assert!(names.iter().any(|n| n == "claude-code"));
    assert!(names.iter().any(|n| n == "cursor"));
    assert!(names.iter().any(|n| n == "windsurf"));
    assert!(names.iter().any(|n| n == "codex"));
    assert!(names.iter().any(|n| n == "kimi"));
    assert!(names.iter().any(|n| n == "opencode"));
    assert!(names.iter().any(|n| n == "trae"));
    // Should include hooks adapters (claude-code, kimi)
    assert!(names.iter().any(|n| n == "kimi"));
    // Should include skills adapters (claude-code, proma)
    assert!(names.iter().any(|n| n == "proma"));
    // Should be deduplicated
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(unique.len(), names.len());
}

// ---------------------------------------------------------------------------
// Import dry-run does not write any files
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn import_dry_run_does_not_create_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    let path_s = config_path.to_str().unwrap();

    lorum::commands::run_import("all", true, Some(path_s)).unwrap();

    assert!(!config_path.exists());
}

// ---------------------------------------------------------------------------
// Import rules from a tool creates .lorum/RULES.md
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn import_rules_from_tool_creates_rules_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    let path_s = config_path.to_str().unwrap();

    // Create a .cursorrules file in the temp directory
    let cursorrules = dir.path().join(".cursorrules");
    fs::write(&cursorrules, "## Style\nAlways use rustfmt.\n").unwrap();

    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = std::panic::catch_unwind(|| {
        lorum::commands::run_import("cursor", false, Some(path_s)).unwrap();
    });

    std::env::set_current_dir(&orig).unwrap();
    result.unwrap();

    // Verify RULES.md was created
    let rules_path = dir.path().join(".lorum").join("RULES.md");
    assert!(rules_path.exists());
    let content = fs::read_to_string(&rules_path).unwrap();
    assert!(content.contains("Style"));
    assert!(content.contains("Always use rustfmt."));
}

// ---------------------------------------------------------------------------
// Import rules merge: existing sections are preserved, new ones appended
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn import_rules_merges_without_overwriting_existing() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    let path_s = config_path.to_str().unwrap();

    // Pre-create .lorum/RULES.md with an existing section
    let lorum_dir = dir.path().join(".lorum");
    fs::create_dir_all(&lorum_dir).unwrap();
    let rules_md = lorum_dir.join("RULES.md");
    fs::write(
        &rules_md,
        "# Project Rules\n\n## Style\nExisting style rule.\n",
    )
    .unwrap();

    // Create a .cursorrules with a different section and same-name section
    let cursorrules = dir.path().join(".cursorrules");
    fs::write(
        &cursorrules,
        "## Style\nNew style rule (should be ignored).\n## Testing\nRun tests.\n",
    )
    .unwrap();

    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = std::panic::catch_unwind(|| {
        lorum::commands::run_import("cursor", false, Some(path_s)).unwrap();
    });

    std::env::set_current_dir(&orig).unwrap();
    result.unwrap();

    let content = fs::read_to_string(&rules_md).unwrap();
    // Existing "Style" section should be preserved
    assert!(content.contains("Existing style rule."));
    // New "Testing" section should be appended
    assert!(content.contains("Testing"));
    assert!(content.contains("Run tests."));
    // The new "Style" content should NOT overwrite the existing one
    assert!(!content.contains("New style rule (should be ignored)."));
}
