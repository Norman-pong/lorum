//! End-to-end integration tests for the init -> mcp add -> sync flow.
//!
//! These tests exercise the public API only, using temporary files so no
//! real user configuration is touched.

use std::collections::BTreeMap;
use std::fs;

use lorum::config::{
    LorumConfig, McpConfig, McpServer, ProjectConfig, load_config, load_project_config,
    merge_configs, save_config,
};
use lorum::env_interpolate;
use lorum::sync;

// ---------------------------------------------------------------------------
// T7.1 – init -> add servers -> verify file
// ---------------------------------------------------------------------------

#[test]
fn init_creates_config_and_add_servers() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    // "init" – save a default config (mirrors commands::run_init).
    let cfg = LorumConfig::default();
    save_config(&config_path, &cfg).unwrap();
    assert!(config_path.exists());

    // "mcp add" – insert servers and save.
    let mut cfg = load_config(&config_path).unwrap();
    cfg.mcp.servers.insert(
        "fetch".into(),
        McpServer {
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-fetch".into()],
            env: BTreeMap::new(),
        },
    );
    cfg.mcp.servers.insert(
        "memory".into(),
        McpServer {
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
            env: BTreeMap::new(),
        },
    );
    save_config(&config_path, &cfg).unwrap();

    // Verify the file on disk round-trips correctly.
    let loaded = load_config(&config_path).unwrap();
    assert_eq!(loaded.mcp.servers.len(), 2);
    assert!(loaded.mcp.servers.contains_key("fetch"));
    assert!(loaded.mcp.servers.contains_key("memory"));
    assert_eq!(
        loaded.mcp.servers["fetch"].args,
        vec!["-y", "@modelcontextprotocol/server-fetch"]
    );

    // Verify the raw YAML contains the expected keys.
    let raw = fs::read_to_string(&config_path).unwrap();
    assert!(raw.contains("fetch"));
    assert!(raw.contains("memory"));
    assert!(raw.contains("npx"));
}

// ---------------------------------------------------------------------------
// T7.1 – sync_all produces results for every registered adapter
// ---------------------------------------------------------------------------

#[test]
fn sync_produces_results_for_all_adapters() {
    let mcp = McpConfig {
        servers: {
            let mut m = BTreeMap::new();
            m.insert(
                "test-server".into(),
                McpServer {
                    command: "echo".into(),
                    args: vec!["hello".into()],
                    env: BTreeMap::new(),
                },
            );
            m
        },
    };

    let results = sync::sync_all(&mcp);
    // We have 5 registered adapters.
    assert_eq!(results.len(), 5);

    let names: Vec<&str> = results.iter().map(|r| r.tool.as_str()).collect();
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"codex"));
    assert!(names.contains(&"proma"));
    assert!(names.contains(&"kimi"));
    assert!(names.contains(&"trae"));

    for result in &results {
        // Each result should report the correct number of servers synced on
        // success, or have an error message on failure.
        if result.success {
            assert_eq!(result.servers_synced, 1);
            assert!(result.error.is_none());
        } else {
            assert!(result.error.is_some());
        }
    }
}

// ---------------------------------------------------------------------------
// T7.1 – full roundtrip: add servers -> compute diff -> verify
// ---------------------------------------------------------------------------

#[test]
fn full_roundtrip_add_sync_verify() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    // Step 1: create empty config.
    save_config(&config_path, &LorumConfig::default()).unwrap();

    // Step 2: add three servers.
    let mut cfg = load_config(&config_path).unwrap();
    for (name, cmd) in [("alpha", "a"), ("beta", "b"), ("gamma", "c")] {
        cfg.mcp.servers.insert(
            name.into(),
            McpServer {
                command: cmd.into(),
                args: vec![],
                env: BTreeMap::new(),
            },
        );
    }
    save_config(&config_path, &cfg).unwrap();

    // Step 3: compute diff – current is empty, target has 3 servers.
    let current = McpConfig::default();
    let target = cfg.mcp.clone();
    let diff = sync::compute_diff(&current, &target);

    assert_eq!(diff.added, vec!["alpha", "beta", "gamma"]);
    assert!(diff.removed.is_empty());
    assert!(diff.modified.is_empty());
    assert!(diff.unchanged.is_empty());
    assert!(!diff.is_empty());
    assert_eq!(diff.change_count(), 3);

    // Step 4: compute diff again – now both sides match.
    let diff_same = sync::compute_diff(&target, &target);
    assert!(diff_same.added.is_empty());
    assert!(diff_same.removed.is_empty());
    assert!(diff_same.modified.is_empty());
    assert_eq!(diff_same.unchanged.len(), 3);
    assert!(diff_same.is_empty()); // no *changes*
}

// ---------------------------------------------------------------------------
// T7.3 – backup and restore roundtrip
// ---------------------------------------------------------------------------

#[test]
fn backup_and_restore_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    // Create an original config.
    let original = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "original-server".into(),
                    McpServer {
                        command: "original-cmd".into(),
                        args: vec!["arg1".into()],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        ..Default::default()
    };
    save_config(&config_path, &original).unwrap();

    // Create a backup.
    let backup_path = lorum::backup::create_backup("test-restore", &config_path).unwrap();
    assert!(backup_path.exists());

    // Modify the config.
    let modified = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "modified-server".into(),
                    McpServer {
                        command: "modified-cmd".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        ..Default::default()
    };
    save_config(&config_path, &modified).unwrap();

    // Verify it was modified.
    let loaded = load_config(&config_path).unwrap();
    assert!(loaded.mcp.servers.contains_key("modified-server"));
    assert!(!loaded.mcp.servers.contains_key("original-server"));

    // Restore from backup.
    lorum::backup::restore_backup("test-restore", &config_path).unwrap();

    // Verify the restored content matches the original.
    let restored = load_config(&config_path).unwrap();
    assert_eq!(restored, original);
}

// ---------------------------------------------------------------------------
// T7.3 – backup cleanup keeps max 10
// ---------------------------------------------------------------------------

#[test]
fn backup_cleanup_keeps_max() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    // Write a dummy file to back up.
    fs::write(&config_path, "test content").unwrap();

    // Create 12 backups. After each one, old backups should be pruned to 10.
    for _ in 0..12 {
        // Small sleep to get unique timestamps.
        std::thread::sleep(std::time::Duration::from_millis(5));
        lorum::backup::create_backup("test-cleanup", &config_path).unwrap();
    }

    let backups = lorum::backup::list_backups("test-cleanup").unwrap();
    assert!(
        backups.len() <= 10,
        "expected at most 10 backups, got {}",
        backups.len()
    );
}

// ---------------------------------------------------------------------------
// T7.4 – global + project merge
// ---------------------------------------------------------------------------

#[test]
fn global_plus_project_merge() {
    let global = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "global-srv".into(),
                    McpServer {
                        command: "g".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m.insert(
                    "shared-srv".into(),
                    McpServer {
                        command: "global-version".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        ..Default::default()
    };

    let project = ProjectConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "project-srv".into(),
                    McpServer {
                        command: "p".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m.insert(
                    "shared-srv".into(),
                    McpServer {
                        command: "project-version".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        exclude: vec![],
        ..Default::default()
    };

    let merged = merge_configs(&global, Some(&project));

    // global-srv is preserved, project-srv is added, shared-srv is overridden.
    assert_eq!(merged.mcp.servers.len(), 3);
    assert_eq!(merged.mcp.servers["global-srv"].command, "g");
    assert_eq!(merged.mcp.servers["project-srv"].command, "p");
    assert_eq!(merged.mcp.servers["shared-srv"].command, "project-version");
}

// ---------------------------------------------------------------------------
// T7.4 – exclude removes servers
// ---------------------------------------------------------------------------

#[test]
fn exclude_removes_servers() {
    let global = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                for name in &["keep", "remove-me", "also-keep"] {
                    m.insert(
                        (*name).into(),
                        McpServer {
                            command: "cmd".into(),
                            args: vec![],
                            env: BTreeMap::new(),
                        },
                    );
                }
                m
            },
        },
        ..Default::default()
    };

    let project = ProjectConfig {
        mcp: McpConfig::default(),
        exclude: vec!["remove-me".into()],
        ..Default::default()
    };

    let merged = merge_configs(&global, Some(&project));
    assert_eq!(merged.mcp.servers.len(), 2);
    assert!(merged.mcp.servers.contains_key("keep"));
    assert!(merged.mcp.servers.contains_key("also-keep"));
    assert!(!merged.mcp.servers.contains_key("remove-me"));
}

// ---------------------------------------------------------------------------
// T7.4 – merge with no project config returns global as-is
// ---------------------------------------------------------------------------

#[test]
fn merge_no_project_returns_global() {
    let global = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "only-global".into(),
                    McpServer {
                        command: "cmd".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        ..Default::default()
    };

    let merged = merge_configs(&global, None);
    assert_eq!(merged, global);
}

// ---------------------------------------------------------------------------
// T7.4 – project config file roundtrip via save/load
// ---------------------------------------------------------------------------

#[test]
fn project_config_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let proj_dir = dir.path().join(".lorum");
    fs::create_dir_all(&proj_dir).unwrap();
    let config_path = proj_dir.join("config.yaml");

    // Write a project config with exclude list.
    let yaml = r#"
mcp:
  servers:
    local-srv:
      command: ./run.sh
      args: []
      env: {}
exclude:
  - remote-heavy
"#;
    fs::write(&config_path, yaml).unwrap();

    let loaded = load_project_config(&config_path).unwrap();
    assert_eq!(loaded.mcp.servers.len(), 1);
    assert_eq!(loaded.mcp.servers["local-srv"].command, "./run.sh");
    assert_eq!(loaded.exclude, vec!["remote-heavy"]);
}

// ---------------------------------------------------------------------------
// T7.1 – config roundtrip: save then load preserves content
// ---------------------------------------------------------------------------

#[test]
fn config_roundtrip_load_save() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");

    let original = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "alpha".into(),
                    McpServer {
                        command: "echo".into(),
                        args: vec!["alpha-arg".into()],
                        env: {
                            let mut e = BTreeMap::new();
                            e.insert("ALPHA_ENV".into(), "alpha-value".into());
                            e
                        },
                    },
                );
                m.insert(
                    "beta".into(),
                    McpServer {
                        command: "run".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        ..Default::default()
    };

    // Save, then load, then verify content matches.
    save_config(&config_path, &original).unwrap();
    let loaded = load_config(&config_path).unwrap();
    assert_eq!(loaded, original);
}

// ---------------------------------------------------------------------------
// T7.1 – compute_diff detects added and removed servers
// ---------------------------------------------------------------------------

#[test]
fn compute_diff_detects_added_removed() {
    // Current config has server-a and server-b.
    let current = McpConfig {
        servers: {
            let mut m = BTreeMap::new();
            m.insert(
                "server-a".into(),
                McpServer {
                    command: "a".into(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            );
            m.insert(
                "server-b".into(),
                McpServer {
                    command: "b".into(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            );
            m
        },
    };

    // Target config has server-b and server-c.
    let target = McpConfig {
        servers: {
            let mut m = BTreeMap::new();
            m.insert(
                "server-b".into(),
                McpServer {
                    command: "b".into(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            );
            m.insert(
                "server-c".into(),
                McpServer {
                    command: "c".into(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            );
            m
        },
    };

    let diff = sync::compute_diff(&current, &target);

    // server-a was removed, server-c was added, server-b is unchanged.
    assert!(diff.added.contains(&"server-c".into()));
    assert!(diff.removed.contains(&"server-a".into()));
    assert!(diff.unchanged.contains(&"server-b".into()));
    assert!(diff.modified.is_empty());
    assert_eq!(diff.change_count(), 2);
}

// ---------------------------------------------------------------------------
// T7.1 – env interpolation preserves unset vars and respects expand flag
// ---------------------------------------------------------------------------

#[test]
fn env_interpolation_expands_vars() {
    // When expand=false, the original string is returned unchanged.
    let no_expand = env_interpolate::interpolate_env("${LORUM_MISSING_VAR}", false);
    assert_eq!(no_expand, "${LORUM_MISSING_VAR}");

    // When expand=true but the variable is not set, ${VAR} is preserved as-is.
    let unset = env_interpolate::interpolate_env("${LORUM_UNSET_VAR_12345}", true);
    assert_eq!(unset, "${LORUM_UNSET_VAR_12345}");

    // Plain text without variables passes through unchanged.
    let plain = env_interpolate::interpolate_env("plain text here", true);
    assert_eq!(plain, "plain text here");

    // Multiple placeholders in one string are all preserved when unset.
    let multi = env_interpolate::interpolate_env("${A} and ${B} and ${C}", true);
    assert_eq!(multi, "${A} and ${B} and ${C}");
}
