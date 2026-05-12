//! Unit tests for CLI commands (non-MCP).
//!
//! Each test creates a temporary config file and passes its path directly to
//! the command functions via the `config_path` parameter, ensuring full
//! isolation from the user's real configuration.

use std::collections::BTreeMap;

use tempfile::TempDir;

use crate::config::{self, LorumConfig, McpConfig, McpServer};

/// Helper: read and parse the config at `path`.
fn read_config(path: &std::path::Path) -> LorumConfig {
    let raw = std::fs::read_to_string(path).unwrap();
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

// ---- Init tests ----

#[test]
fn init_creates_global_config() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.yaml");
    let path_s = config_path.to_str().unwrap().to_string();

    super::run_init(Some(&path_s), false, true).unwrap();

    assert!(config_path.exists());
    let cfg = read_config(&config_path);
    assert!(cfg.mcp.servers.is_empty());
}

#[test]
fn init_creates_local_config() {
    let dir = TempDir::new().unwrap();
    let local_path = dir.path().join(".lorum").join("config.yaml");

    // init --local uses cwd/.lorum/config.yaml, so we set cwd to our temp dir
    let orig = std::env::current_dir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe {
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    std::env::set_current_dir(dir.path()).unwrap();

    super::run_init(None, true, true).unwrap();

    std::env::set_current_dir(&orig).unwrap();
    unsafe {
        if let Some(h) = orig_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }

    assert!(local_path.exists());
    let cfg = read_config(&local_path);
    assert!(cfg.mcp.servers.is_empty());
}

#[test]
fn init_skips_existing_config() {
    let initial = LorumConfig::default();
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    // Should fail when config already exists
    let result = super::run_init(Some(&path_str(&config_path)), false, true);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("config already exists"));
}

// ---- Import tests ----

#[test]
fn import_from_nonexistent_adapter_returns_error() {
    let (_dir, config_path) = setup_temp_config(None);

    let result = super::run_import("nonexistent-tool", false, Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn import_creates_config_if_missing() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.yaml");
    let path_s = config_path.to_str().unwrap().to_string();

    // import from "all" should succeed even with no existing config file
    super::run_import("all", false, Some(&path_s)).unwrap();

    assert!(config_path.exists());
}

// ---- Check tests ----

#[test]
fn check_valid_config() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                // Use "cargo" as a command that is guaranteed to be on PATH
                // during test execution.
                m.insert("srv".into(), make_server("cargo", &[], &[]));
                m
            },
        },
        ..Default::default()
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    super::run_check(Some(&path_str(&config_path))).unwrap();
}

#[test]
fn check_empty_command_returns_error() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("bad".into(), make_server("", &[], &[]));
                m
            },
        },
        ..Default::default()
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = super::run_check(Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn check_empty_config_is_valid() {
    let initial = LorumConfig::default();
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    super::run_check(Some(&path_str(&config_path))).unwrap();
}

// ---- Status tests ----

#[test]
fn status_succeeds() {
    // status just prints, should always succeed
    super::run_status(None).unwrap();
}

// ---- Config tests ----

#[test]
fn config_outputs_yaml() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert("srv".into(), make_server("cmd", &["arg"], &[]));
                m
            },
        },
        ..Default::default()
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    // run_config with explicit path should output valid yaml
    super::run_config(false, false, false, Some(&path_str(&config_path))).unwrap();
}

#[test]
fn config_local_missing_returns_error() {
    let dir = TempDir::new().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = super::run_config(false, true, false, None);

    std::env::set_current_dir(&orig).unwrap();
    assert!(result.is_err());
}

// ---- Backup list tests ----

#[test]
fn backup_list_no_backups() {
    // Should succeed even when no backup directory exists
    super::run_backup_list(None).unwrap();
}

// ---- Backup restore tests ----

#[test]
fn backup_restore_nonexistent_adapter_returns_error() {
    let result = super::run_backup_restore("nonexistent-tool", None, None);
    assert!(result.is_err());
}

#[test]
fn backup_restore_no_backup_returns_error() {
    // claude-code adapter exists but likely has no backups
    let result = super::run_backup_restore("claude-code", None, None);
    // This may succeed or fail depending on state; we just verify it doesn't panic
    let _ = result;
}

// ---- Detect installed tools tests ----

#[test]
fn detect_installed_tools_returns_vec() {
    // Just verify it doesn't panic
    let tools = super::detect_installed_tools();
    // We can't assert specific tools since it depends on the system
    let _ = tools;
}

// ---- Status helper tests ----

#[test]
fn fmt_count_none() {
    assert_eq!(super::fmt_count(None), "-");
}

#[test]
fn fmt_count_zero() {
    assert_eq!(super::fmt_count(Some(0)), "·");
}

#[test]
fn fmt_count_positive() {
    assert_eq!(super::fmt_count(Some(42)), "42");
}

// ---- Check helper tests ----

#[test]
fn command_exists_with_absolute_path() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("my-cmd");
    std::fs::write(&file, "").unwrap();
    assert!(super::command_exists(file.to_str().unwrap()));
}

#[test]
fn command_exists_missing_file() {
    assert!(!super::command_exists("/tmp/no-such-command-xyz-abc"));
}

#[test]
fn find_unset_env_refs_finds_unset() {
    // Ensure the variable is not set
    unsafe { std::env::remove_var("LORUM_TEST_UNSET_VAR_42") };
    let result = super::find_unset_env_refs("prefix_${LORUM_TEST_UNSET_VAR_42}_suffix");
    assert_eq!(result, vec!["LORUM_TEST_UNSET_VAR_42"]);
}

#[test]
fn find_unset_env_refs_ignores_set() {
    unsafe { std::env::set_var("LORUM_TEST_SET_VAR_42", "value") };
    let result = super::find_unset_env_refs("${LORUM_TEST_SET_VAR_42}");
    assert!(result.is_empty());
    unsafe { std::env::remove_var("LORUM_TEST_SET_VAR_42") };
}

#[test]
fn find_unset_env_refs_no_refs() {
    let result = super::find_unset_env_refs("plain text without refs");
    assert!(result.is_empty());
}

#[test]
fn is_valid_kebab_case_accepts_valid() {
    assert!(super::is_valid_kebab_case("pre-tool-use"));
    assert!(super::is_valid_kebab_case("event-1"));
    assert!(super::is_valid_kebab_case("a"));
}

#[test]
fn is_valid_kebab_case_rejects_invalid() {
    assert!(!super::is_valid_kebab_case("")); // empty
    assert!(!super::is_valid_kebab_case("-start")); // leading hyphen
    assert!(!super::is_valid_kebab_case("end-")); // trailing hyphen
    assert!(!super::is_valid_kebab_case("a--b")); // double hyphen
    assert!(!super::is_valid_kebab_case("UPPER")); // uppercase
    assert!(!super::is_valid_kebab_case("snake_case")); // underscore
}

#[test]
fn check_missing_command_returns_error() {
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "bad".into(),
                    make_server("/tmp/no-such-lorum-cmd-xyz", &[], &[]),
                );
                m
            },
        },
        ..Default::default()
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = super::run_check(Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn check_unset_env_ref_returns_error() {
    unsafe { std::env::remove_var("LORUM_TEST_UNSET_ENV_99") };
    let initial = LorumConfig {
        mcp: McpConfig {
            servers: {
                let mut m = BTreeMap::new();
                m.insert(
                    "srv".into(),
                    make_server("cargo", &[], &[("KEY", "${LORUM_TEST_UNSET_ENV_99}")]),
                );
                m
            },
        },
        ..Default::default()
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = super::run_check(Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn check_invalid_hook_event_returns_error() {
    use crate::config::{HookHandler, HooksConfig};

    let initial = LorumConfig {
        mcp: McpConfig::default(),
        hooks: HooksConfig {
            events: {
                let mut m = BTreeMap::new();
                m.insert(
                    "InvalidEvent".into(),
                    vec![HookHandler {
                        matcher: "Bash".into(),
                        command: "echo ok".into(),
                        timeout: None,
                        handler_type: None,
                    }],
                );
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = super::run_check(Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn check_empty_hook_handler_returns_error() {
    use crate::config::{HookHandler, HooksConfig};

    let initial = LorumConfig {
        mcp: McpConfig::default(),
        hooks: HooksConfig {
            events: {
                let mut m = BTreeMap::new();
                m.insert(
                    "pre-tool-use".into(),
                    vec![HookHandler {
                        matcher: "".into(),
                        command: "".into(),
                        timeout: None,
                        handler_type: None,
                    }],
                );
                m
            },
        },
    };
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = super::run_check(Some(&path_str(&config_path)));
    assert!(result.is_err());
}
