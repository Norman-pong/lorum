//! Unit tests for hook CRUD commands.

use tempfile::TempDir;

use crate::commands::hook;
use crate::config::{self, HookHandler, LorumConfig};

/// Helper: read and parse the config at `path`.
fn read_config(path: &std::path::Path) -> LorumConfig {
    let raw = std::fs::read_to_string(path).unwrap();
    serde_yaml::from_str(&raw).unwrap()
}

/// Helper: create a temp config file, optionally pre-populated, and return
/// the TempDir and the config file path.
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

#[test]
fn add_new_hook_to_empty_config() {
    let (_dir, config_path) = setup_temp_config(None);

    hook::run_hook_add(
        "pre-tool-use",
        "Bash",
        "check.sh",
        Some(30),
        Some("command"),
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    assert_eq!(reloaded.hooks.events.len(), 1);
    let handlers = &reloaded.hooks.events["pre-tool-use"];
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].matcher, "Bash");
    assert_eq!(handlers[0].command, "check.sh");
    assert_eq!(handlers[0].timeout, Some(30));
    assert_eq!(handlers[0].handler_type, Some("command".into()));
}

#[test]
fn add_replaces_existing_matcher() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "old.sh".into(),
            timeout: None,
            handler_type: None,
        }],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    hook::run_hook_add(
        "pre-tool-use",
        "Bash",
        "new.sh",
        Some(60),
        None,
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    let handlers = &reloaded.hooks.events["pre-tool-use"];
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].command, "new.sh");
    assert_eq!(handlers[0].timeout, Some(60));
}

#[test]
fn add_second_handler_to_same_event() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "bash.sh".into(),
            timeout: None,
            handler_type: None,
        }],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    hook::run_hook_add(
        "pre-tool-use",
        "Write",
        "write.sh",
        None,
        None,
        Some(&path_str(&config_path)),
    )
    .unwrap();

    let reloaded = read_config(&config_path);
    let handlers = &reloaded.hooks.events["pre-tool-use"];
    assert_eq!(handlers.len(), 2);
}

#[test]
fn remove_existing_handler() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![
            HookHandler {
                matcher: "Bash".into(),
                command: "bash.sh".into(),
                timeout: None,
                handler_type: None,
            },
            HookHandler {
                matcher: "Write".into(),
                command: "write.sh".into(),
                timeout: None,
                handler_type: None,
            },
        ],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    hook::run_hook_remove("pre-tool-use", Some("Bash"), Some(&path_str(&config_path))).unwrap();

    let reloaded = read_config(&config_path);
    let handlers = &reloaded.hooks.events["pre-tool-use"];
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].matcher, "Write");
}

#[test]
fn remove_last_handler_removes_event() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "bash.sh".into(),
            timeout: None,
            handler_type: None,
        }],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    hook::run_hook_remove("pre-tool-use", Some("Bash"), Some(&path_str(&config_path))).unwrap();

    let reloaded = read_config(&config_path);
    assert!(!reloaded.hooks.events.contains_key("pre-tool-use"));
}

#[test]
fn remove_entire_event() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "bash.sh".into(),
            timeout: None,
            handler_type: None,
        }],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    hook::run_hook_remove("pre-tool-use", None, Some(&path_str(&config_path))).unwrap();

    let reloaded = read_config(&config_path);
    assert!(!reloaded.hooks.events.contains_key("pre-tool-use"));
}

#[test]
fn remove_nonexistent_event_returns_error() {
    let (_dir, config_path) = setup_temp_config(None);

    let result = hook::run_hook_remove("nope", None, Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn remove_nonexistent_matcher_returns_error() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "bash.sh".into(),
            timeout: None,
            handler_type: None,
        }],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    let result = hook::run_hook_remove("pre-tool-use", Some("Nope"), Some(&path_str(&config_path)));
    assert!(result.is_err());
}

#[test]
fn list_empty_config_outputs_no_hooks() {
    let (_dir, config_path) = setup_temp_config(None);

    hook::run_hook_list(Some(&path_str(&config_path))).unwrap();
}

#[test]
fn run_hook_sync_dry_run_empty_config() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.yaml");
    // Create an empty config file so load_config succeeds
    config::save_config(&config_path, &config::LorumConfig::default()).unwrap();
    // dry_run with empty config should not panic and return Ok
    hook::run_hook_sync(true, &[], Some(config_path.to_str().unwrap())).unwrap();
}

#[test]
fn list_outputs_hooks() {
    let mut initial = LorumConfig::default();
    initial.hooks.events.insert(
        "pre-tool-use".into(),
        vec![HookHandler {
            matcher: "Bash".into(),
            command: "check.sh".into(),
            timeout: Some(30),
            handler_type: None,
        }],
    );
    let (_dir, config_path) = setup_temp_config(Some(&initial));

    hook::run_hook_list(Some(&path_str(&config_path))).unwrap();
}
