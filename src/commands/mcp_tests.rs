//! Unit tests for MCP CRUD commands.
//!
//! Each test creates a temporary config file and passes its path directly to
//! the command functions via the `config_path` parameter, ensuring full
//! isolation from the user's real configuration.

use std::collections::BTreeMap;
use std::fs;

use tempfile::TempDir;

use crate::commands::mcp;
use crate::config::{self, LorumConfig, McpConfig, McpServer};

/// Helper: read and parse the config at `path`.
fn read_config(path: &std::path::Path) -> LorumConfig {
    let raw = fs::read_to_string(path).unwrap();
    serde_yaml::from_str(&raw).unwrap()
}

/// Helper: create a temp config file, optionally pre-populated, and return
/// the TempDir (caller must keep it alive) and the config file path.
fn setup_temp_config(initial: Option<&LorumConfig>) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.yaml");
    if let Some(cfg) = initial {
        config::save_config(&config_path, cfg).unwrap();
    }
    (dir, config_path)
}

/// Helper: convert a tempdir path to a string for passing as config_path.
fn path_str(path: &std::path::Path) -> String {
    path.to_str().unwrap().to_string()
}

/// Helper: build a simple McpServer.
fn make_server(command: &str, args: &[&str], env: &[(&str, &str)]) -> McpServer {
    McpServer {
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

#[test]
fn add_new_server_to_empty_config() {
    let (_dir, config_path) = setup_temp_config(None);
    let args: Vec<String> = vec!["-y".into(), "some-pkg".into()];
    let env: Vec<String> = vec!["KEY=val".into()];

    mcp::run_mcp_add(
        "my-server",
        "npx",
        &args,
        &env,
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    assert_eq!(reloaded.mcp.servers.len(), 1);
    let srv = &reloaded.mcp.servers["my-server"];
    assert_eq!(srv.command, "npx");
    assert_eq!(srv.args, vec!["-y", "some-pkg"]);
    assert_eq!(srv.env.get("KEY").unwrap(), "val");
}

#[test]
fn add_overwrites_existing_server() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("srv".into(), make_server("old-cmd", &["old-arg"], &[]));
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));
    let args: Vec<String> = vec!["new-arg".into()];
    let env: Vec<String> = vec!["K=V".into()];

    mcp::run_mcp_add("srv", "new-cmd", &args, &env, Some(&path_str(&config_path))).unwrap();

    let reloaded = read_config(&config_path);
    assert_eq!(reloaded.mcp.servers.len(), 1);
    let srv = &reloaded.mcp.servers["srv"];
    assert_eq!(srv.command, "new-cmd");
    assert_eq!(srv.args, vec!["new-arg"]);
    assert_eq!(srv.env.get("K").unwrap(), "V");
}

#[test]
fn remove_existing_server() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("keep".into(), make_server("keep-cmd", &[], &[]));
                m.insert("remove-me".into(), make_server("rm-cmd", &[], &[]));
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    mcp::run_mcp_remove("remove-me", Some(&path_str(&config_path))).unwrap();

    let reloaded = read_config(&config_path);
    assert_eq!(reloaded.mcp.servers.len(), 1);
    assert!(reloaded.mcp.servers.contains_key("keep"));
    assert!(!reloaded.mcp.servers.contains_key("remove-me"));
}

#[test]
fn remove_nonexistent_returns_error() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: BTreeMap::new(),
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = mcp::run_mcp_remove("no-such-server", Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn list_outputs_all_servers() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("alpha".into(), make_server("cmd-a", &["--flag"], &[]));
                m.insert("beta".into(), make_server("cmd-b", &[], &[]));
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    // Should succeed without panicking.
    mcp::run_mcp_list(Some(&path_str(&config_path))).unwrap();
}

#[test]
fn list_empty_config_outputs_no_servers() {
    let (_dir, config_path) = setup_temp_config(None);

    mcp::run_mcp_list(Some(&path_str(&config_path))).unwrap();
}

#[test]
fn edit_updates_command() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "srv".into(),
                    make_server("old-cmd", &["old-arg"], &[("K", "V")]),
                );
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    mcp::run_mcp_edit(
        "srv",
        Some("new-cmd"),
        None,
        None,
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    let srv = &reloaded.mcp.servers["srv"];
    assert_eq!(srv.command, "new-cmd");
    // args and env should be unchanged
    assert_eq!(srv.args, vec!["old-arg"]);
    assert_eq!(srv.env.get("K").unwrap(), "V");
}

#[test]
fn edit_updates_args() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("srv".into(), make_server("cmd", &["old"], &[]));
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));
    let new_args: Vec<String> = vec!["a".into(), "b".into()];

    mcp::run_mcp_edit(
        "srv",
        None,
        Some(&new_args),
        None,
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    assert_eq!(reloaded.mcp.servers["srv"].args, vec!["a", "b"]);
}

#[test]
fn edit_updates_env() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("srv".into(), make_server("cmd", &[], &[("OLD", "old")]));
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));
    let new_env: Vec<String> = vec!["NEW=new".into()];

    mcp::run_mcp_edit(
        "srv",
        None,
        None,
        Some(&new_env),
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    let srv = &reloaded.mcp.servers["srv"];
    assert!(!srv.env.contains_key("OLD"));
    assert_eq!(srv.env.get("NEW").unwrap(), "new");
}

#[test]
fn edit_nonexistent_returns_error() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: BTreeMap::new(),
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = mcp::run_mcp_edit(
        "no-such-server",
        None,
        None,
        None,
        Some(&path_str(&config_path)),
    );
    assert!(result.is_err());
}

#[test]
fn parse_env_pairs_valid() {
    let pairs: Vec<String> = vec!["KEY1=val1".into(), "KEY2=val2".into()];
    let map = mcp::parse_env_pairs(&pairs);
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("KEY1").unwrap(), "val1");
    assert_eq!(map.get("KEY2").unwrap(), "val2");
}

#[test]
fn parse_env_pairs_skips_invalid() {
    let pairs: Vec<String> = vec!["VALID=1".into(), "NOEQUALSSIGN".into()];
    let map = mcp::parse_env_pairs(&pairs);
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("VALID").unwrap(), "1");
}

#[test]
fn parse_env_pairs_skips_empty_key() {
    let pairs: Vec<String> = vec!["=value".into(), "OK=good".into()];
    let map = mcp::parse_env_pairs(&pairs);
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("OK").unwrap(), "good");
    assert!(!map.contains_key(""));
}
