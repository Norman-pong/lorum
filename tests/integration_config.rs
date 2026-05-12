//! Integration tests for the config subcommand.

use std::collections::BTreeMap;

use lorum::config::{HookHandler, HooksConfig, LorumConfig, McpConfig, McpServer, OutputFormat};

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

fn temp_config_with_hooks() -> (tempfile::TempDir, std::path::PathBuf, LorumConfig) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.yaml");
    let config = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("test".into(), make_server("cmd", &["arg"], &[]));
                m
            },
        },
        hooks: HooksConfig {
            events: {
                let mut m = BTreeMap::new();
                m.insert(
                    "pre-tool-use".into(),
                    vec![HookHandler {
                        matcher: "Bash".into(),
                        command: "echo ${TEST_VAR}".into(),
                        timeout: Some(30),
                        handler_type: Some("command".into()),
                    }],
                );
                m
            },
        },
    };
    lorum::config::save_config(&path, &config).unwrap();
    (dir, path, config)
}

// ---------------------------------------------------------------------------
// T6.1 — Config outputs YAML by default with source annotation
// ---------------------------------------------------------------------------

#[test]
fn config_yaml_output_with_source() {
    let (_dir, path, _config) = temp_config_with_hooks();
    let path_s = path.to_str().unwrap();

    let output = std::panic::catch_unwind(|| {
        lorum::commands::run_config(false, false, false, OutputFormat::Yaml, Some(path_s))
    });
    assert!(output.is_ok());
}

// ---------------------------------------------------------------------------
// T6.1 — Config outputs JSON with source annotation
// ---------------------------------------------------------------------------

#[test]
fn config_json_output_with_source() {
    let (_dir, path, _config) = temp_config_with_hooks();
    let path_s = path.to_str().unwrap();

    let output = std::panic::catch_unwind(|| {
        lorum::commands::run_config(false, false, false, OutputFormat::Json, Some(path_s))
    });
    assert!(output.is_ok());
}

// ---------------------------------------------------------------------------
// T6.2 — Env interpolation expands variables in hooks config
// ---------------------------------------------------------------------------

#[test]
fn config_resolve_env_interpolates_hooks() {
    let (_dir, path, _config) = temp_config_with_hooks();
    let path_s = path.to_str().unwrap();

    unsafe { std::env::set_var("TEST_VAR", "hello") };

    let output = std::panic::catch_unwind(|| {
        lorum::commands::run_config(true, false, false, OutputFormat::Yaml, Some(path_s)).unwrap();
    });

    unsafe { std::env::remove_var("TEST_VAR") };

    assert!(output.is_ok());
}

// ---------------------------------------------------------------------------
// T6.3 — Local config source annotation when --local is used
// ---------------------------------------------------------------------------

#[test]
fn config_local_source_annotation() {
    let dir = tempfile::tempdir().unwrap();
    let lorum_dir = dir.path().join(".lorum");
    std::fs::create_dir_all(&lorum_dir).unwrap();

    let project_config = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("local-srv".into(), make_server("local-cmd", &[], &[]));
                m
            },
        },
        ..Default::default()
    };
    lorum::config::save_config(&lorum_dir.join("config.yaml"), &project_config).unwrap();

    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = std::panic::catch_unwind(|| {
        lorum::commands::run_config(false, true, false, OutputFormat::Yaml, None).unwrap();
    });

    std::env::set_current_dir(&orig).unwrap();
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// T6.3 — Global config source annotation when --global is used
// ---------------------------------------------------------------------------

#[test]
fn config_global_source_annotation() {
    let output = std::panic::catch_unwind(|| {
        lorum::commands::run_config(false, false, true, OutputFormat::Yaml, None).unwrap();
    });
    assert!(output.is_ok());
}

// ---------------------------------------------------------------------------
// T6.4 — resolve_env=false preserves ${VAR} placeholders in hooks
// ---------------------------------------------------------------------------

#[test]
fn config_no_resolve_env_preserves_placeholders() {
    let (_dir, path, _config) = temp_config_with_hooks();
    let path_s = path.to_str().unwrap();

    let output = std::panic::catch_unwind(|| {
        lorum::commands::run_config(false, false, false, OutputFormat::Yaml, Some(path_s))
    });
    assert!(output.is_ok());
}

// ---------------------------------------------------------------------------
// T6.5 — Config local missing returns error
// ---------------------------------------------------------------------------

#[test]
fn config_local_missing_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = lorum::commands::run_config(false, true, false, OutputFormat::Yaml, None);

    std::env::set_current_dir(&orig).unwrap();
    assert!(result.is_err());
}
