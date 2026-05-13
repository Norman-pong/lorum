use super::*;
use serial_test::serial;
use std::fs;
use std::panic;
use tempfile::TempDir;

fn write_yaml(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn parses_valid_yaml_config() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.yaml");
    write_yaml(
        &path,
        "mcp:\n  servers:\n    my-server:\n      command: npx\n      args:\n        - -y\n        - some-pkg\n      env:\n        KEY: value\n",
    );

    let config = load_config(&path).unwrap();
    assert_eq!(config.mcp.servers.len(), 1);
    let server = &config.mcp.servers["my-server"];
    assert_eq!(server.command, "npx");
    assert_eq!(server.args, vec!["-y", "some-pkg"]);
    assert_eq!(server.env.get("KEY").unwrap(), "value");
}

#[test]
fn returns_error_for_missing_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.yaml");

    let result = load_config(&path);
    assert!(result.is_err());
    match result.unwrap_err() {
        LorumError::ConfigNotFound { .. } => {}
        other => panic!("expected ConfigNotFound, got {other:?}"),
    }
}

#[test]
fn merges_global_and_project() {
    let global = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "global-srv".into(),
                    McpServer {
                        command: "node".into(),
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
                        command: "python".into(),
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
    assert_eq!(merged.mcp.servers.len(), 2);
    assert!(merged.mcp.servers.contains_key("global-srv"));
    assert!(merged.mcp.servers.contains_key("project-srv"));
}

#[test]
fn project_server_overrides_global() {
    let global = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "shared".into(),
                    McpServer {
                        command: "global-cmd".into(),
                        args: vec!["old".into()],
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
                    "shared".into(),
                    McpServer {
                        command: "project-cmd".into(),
                        args: vec!["new".into()],
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
    assert_eq!(merged.mcp.servers.len(), 1);
    let server = &merged.mcp.servers["shared"];
    assert_eq!(server.command, "project-cmd");
    assert_eq!(server.args, vec!["new"]);
}

#[test]
fn exclude_removes_global_server() {
    let global = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "keep".into(),
                    McpServer {
                        command: "keep-cmd".into(),
                        args: vec![],
                        env: BTreeMap::new(),
                    },
                );
                m.insert(
                    "remove-me".into(),
                    McpServer {
                        command: "remove-cmd".into(),
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
            servers: BTreeMap::new(),
        },
        exclude: vec!["remove-me".into()],
        ..Default::default()
    };

    let merged = merge_configs(&global, Some(&project));
    assert_eq!(merged.mcp.servers.len(), 1);
    assert!(merged.mcp.servers.contains_key("keep"));
    assert!(!merged.mcp.servers.contains_key("remove-me"));
}

#[test]
fn config_path_overrides_all() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("custom.yaml");
    write_yaml(
        &path,
        "mcp:\n  servers:\n    only:\n      command: standalone\n",
    );

    let config = resolve_effective_config(Some(&path), dir.path()).unwrap();
    assert_eq!(config.mcp.servers.len(), 1);
    assert_eq!(config.mcp.servers["only"].command, "standalone");
}

#[test]
fn finds_project_config_in_parent_dir() {
    let root = TempDir::new().unwrap();
    let lorum_dir = root.path().join(".lorum");
    fs::create_dir_all(&lorum_dir).unwrap();
    let config_path = lorum_dir.join("config.yaml");
    fs::write(&config_path, "mcp:\n  servers: {}\n").unwrap();

    // Search from a subdirectory.
    let sub = root.path().join("sub").join("deep");
    fs::create_dir_all(&sub).unwrap();

    let found = find_project_config(&sub);
    assert_eq!(found, Some(config_path));
}

#[test]
fn returns_none_when_no_project_config() {
    let dir = TempDir::new().unwrap();
    let result = find_project_config(dir.path());
    assert!(result.is_none());
}

#[test]
fn parses_config_with_hooks() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.yaml");
    write_yaml(
        &path,
        "hooks:\n  pre-tool-use:\n    - matcher: Bash\n      command: scripts/check.sh\n      timeout: 30\n",
    );

    let config = load_config(&path).unwrap();
    assert_eq!(config.hooks.events.len(), 1);
    let handlers = &config.hooks.events["pre-tool-use"];
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].matcher, "Bash");
    assert_eq!(handlers[0].command, "scripts/check.sh");
    assert_eq!(handlers[0].timeout, Some(30));
    assert_eq!(handlers[0].handler_type, None);
}

#[test]
fn merges_hooks_global_and_project() {
    let global = LorumConfig {
        mcp: McpConfig::default(),
        hooks: HooksConfig {
            events: {
                let mut m = BTreeMap::new();
                m.insert(
                    "pre-tool-use".into(),
                    vec![HookHandler {
                        matcher: "Bash".into(),
                        command: "global.sh".into(),
                        timeout: None,
                        handler_type: None,
                    }],
                );
                m.insert(
                    "session-start".into(),
                    vec![HookHandler {
                        matcher: "*".into(),
                        command: "start.sh".into(),
                        timeout: None,
                        handler_type: None,
                    }],
                );
                m
            },
        },
    };

    let project = ProjectConfig {
        mcp: McpConfig::default(),
        hooks: HooksConfig {
            events: {
                let mut m = BTreeMap::new();
                m.insert(
                    "pre-tool-use".into(),
                    vec![HookHandler {
                        matcher: "Write".into(),
                        command: "project.sh".into(),
                        timeout: Some(60),
                        handler_type: None,
                    }],
                );
                m
            },
        },
        exclude: vec![],
    };

    let merged = merge_configs(&global, Some(&project));
    assert_eq!(merged.hooks.events.len(), 2);
    // Overridden event should use project handler.
    let pre = &merged.hooks.events["pre-tool-use"];
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0].matcher, "Write");
    assert_eq!(pre[0].command, "project.sh");
    // Non-overridden global event should remain.
    assert!(merged.hooks.events.contains_key("session-start"));
}

#[test]
fn global_config_path_returns_expected_suffix() {
    let path = global_config_path().unwrap();
    let s = path.to_string_lossy();
    assert!(
        s.ends_with(".config/lorum/config.yaml"),
        "expected path ending in '.config/lorum/config.yaml', got: {}",
        s
    );
}

#[test]
#[serial]
fn global_config_path_respects_xdg_config_home() {
    let original = std::env::var_os("XDG_CONFIG_HOME");
    let result = panic::catch_unwind(|| {
        let dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
        }
        let path = global_config_path().unwrap();
        assert_eq!(path, dir.path().join("lorum").join("config.yaml"));
    });
    unsafe {
        match original {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(result.is_ok());
}

#[test]
fn save_config_creates_file_and_loads_back() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("subdir").join("config.yaml");

    let config = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "test-srv".into(),
                    McpServer {
                        command: "echo".into(),
                        args: vec!["hello".into()],
                        env: BTreeMap::new(),
                    },
                );
                m
            },
        },
        ..Default::default()
    };

    save_config(&path, &config).unwrap();
    assert!(path.exists(), "saved config file should exist");

    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.mcp.servers.len(), 1);
    assert_eq!(loaded.mcp.servers["test-srv"].command, "echo");
    assert_eq!(loaded.mcp.servers["test-srv"].args, vec!["hello"]);
}

#[test]
fn load_project_config_reads_valid_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("project.yaml");
    write_yaml(
        &path,
        "mcp:\n  servers:\n    proj-srv:\n      command: python\nexclude:\n  - old-srv\n",
    );

    let config = load_project_config(&path).unwrap();
    assert_eq!(config.mcp.servers.len(), 1);
    assert_eq!(config.mcp.servers["proj-srv"].command, "python");
    assert_eq!(config.exclude, vec!["old-srv"]);
}

#[test]
#[serial]
fn resolve_effective_config_from_cwd_with_no_project_config() {
    let dir = TempDir::new().unwrap();
    let lorum_dir = dir.path().join("lorum");
    fs::create_dir_all(&lorum_dir).unwrap();
    let global_path = lorum_dir.join("config.yaml");
    write_yaml(
        &global_path,
        "mcp:\n  servers:\n    global-srv:\n      command: node\n",
    );

    let original = std::env::var_os("XDG_CONFIG_HOME");
    let result = panic::catch_unwind(|| {
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
        }
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        std::env::set_current_dir(&sub).unwrap();

        let effective = resolve_effective_config_from_cwd(None).unwrap();
        assert_eq!(effective.mcp.servers.len(), 1);
        assert_eq!(effective.mcp.servers["global-srv"].command, "node");
    });
    unsafe {
        match original {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(result.is_ok());
}

#[test]
#[serial]
fn resolve_effective_config_from_cwd_merges_project_config() {
    let dir = TempDir::new().unwrap();
    let lorum_dir = dir.path().join("lorum");
    fs::create_dir_all(&lorum_dir).unwrap();
    let global_path = lorum_dir.join("config.yaml");
    write_yaml(
        &global_path,
        "mcp:\n  servers:\n    global-srv:\n      command: node\n",
    );

    let project_dir = dir.path().join("project");
    let project_lorum_dir = project_dir.join(".lorum");
    fs::create_dir_all(&project_lorum_dir).unwrap();
    let project_path = project_lorum_dir.join("config.yaml");
    write_yaml(
        &project_path,
        "mcp:\n  servers:\n    project-srv:\n      command: python\n",
    );

    let original = std::env::var_os("XDG_CONFIG_HOME");
    let result = panic::catch_unwind(|| {
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.path());
        }
        std::env::set_current_dir(&project_dir).unwrap();

        let effective = resolve_effective_config_from_cwd(None).unwrap();
        assert_eq!(effective.mcp.servers.len(), 2);
        assert_eq!(effective.mcp.servers["global-srv"].command, "node");
        assert_eq!(effective.mcp.servers["project-srv"].command, "python");
    });
    unsafe {
        match original {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(result.is_ok());
}
