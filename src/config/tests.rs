use super::*;
use std::fs;
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
    };

    let project = ProjectConfig {
        mcp: McpConfig {
            servers: BTreeMap::new(),
        },
        exclude: vec!["remove-me".into()],
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
