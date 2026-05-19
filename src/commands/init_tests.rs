//! Unit tests for the interactive `init` command.

use serial_test::serial;
use tempfile::TempDir;

use crate::commands::init::{git_repo_detected, is_git_repo, run_interactive_init};
use crate::config;

#[test]
#[serial]
fn rejects_non_tty_without_yes() {
    // cargo test runs with stdin as a pipe (non-TTY).
    let dir = TempDir::new().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = run_interactive_init(false, false);

    std::env::set_current_dir(&orig).unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("interactive init requires a TTY"));
}

#[test]
#[serial]
fn creates_local_config_and_gitignore() {
    let dir = TempDir::new().unwrap();
    let orig = std::env::current_dir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe {
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    std::env::set_current_dir(dir.path()).unwrap();

    run_interactive_init(true, true).unwrap();

    std::env::set_current_dir(&orig).unwrap();
    unsafe {
        if let Some(h) = orig_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }

    let config_path = dir.path().join(".lorum").join("config.yaml");
    let gitignore_path = dir.path().join(".lorum").join(".gitignore");

    assert!(config_path.exists());
    assert!(gitignore_path.exists());

    let cfg = config::load_config(&config_path).unwrap();
    assert!(cfg.mcp.servers.is_empty());

    let gitignore = std::fs::read_to_string(&gitignore_path).unwrap();
    assert_eq!(gitignore, "skills/\n");
}

#[test]
#[serial]
fn refuses_to_overwrite_existing() {
    let dir = TempDir::new().unwrap();

    // Create the local config first
    let local_config = dir.path().join(".lorum").join("config.yaml");
    std::fs::create_dir_all(local_config.parent().unwrap()).unwrap();
    std::fs::write(&local_config, "mcp:\n").unwrap();

    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = run_interactive_init(true, true);

    std::env::set_current_dir(&orig).unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("config already exists"));
}

#[test]
#[serial]
fn yes_mode_creates_without_prompts() {
    let dir = TempDir::new().unwrap();
    // Use a directory without .git so it routes to global config.
    let orig_home = std::env::var_os("HOME");
    unsafe {
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    run_interactive_init(false, true).unwrap();

    std::env::set_current_dir(&orig).unwrap();
    unsafe {
        if let Some(h) = orig_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }

    let config_path = dir.path().join(".config").join("lorum").join("config.yaml");
    assert!(config_path.exists());
}

// ---- Git detection tests ----

#[test]
fn is_git_repo_detects_git_directory() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    assert!(is_git_repo(dir.path()));
}

#[test]
fn is_git_repo_returns_false_without_git() {
    let dir = TempDir::new().unwrap();
    assert!(!is_git_repo(dir.path()));
}

#[test]
fn is_git_repo_detects_parent_git() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    let child = dir.path().join("subdir");
    std::fs::create_dir_all(&child).unwrap();
    assert!(is_git_repo(&child));
}

#[test]
#[serial]
fn git_repo_detected_uses_cwd() {
    let dir = TempDir::new().unwrap();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = git_repo_detected();
    assert!(!result);

    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    let result = git_repo_detected();
    assert!(result);

    std::env::set_current_dir(&orig).unwrap();
}

// ---- Dynamic detection test ----

#[test]
#[serial]
fn detect_installed_tools_uses_adapter_registry() {
    // Verify detect_installed_tools is driven by adapter config_paths.
    // On macOS dirs::home_dir() ignores $HOME, so we must create the
    // marker directory in the real home dir and clean it up afterwards.
    let home = dirs::home_dir().expect("home dir");
    let claude_dir = home.join(".claude");
    let already_existed = claude_dir.exists();

    // Ensure no .claude dir exists at start.
    if already_existed {
        std::fs::remove_dir_all(&claude_dir).unwrap();
    }

    let tools = super::detect_installed_tools();
    assert!(
        !tools.contains(&"claude-code".to_string()),
        "expected claude-code NOT detected before creating .claude, got: {tools:?}"
    );

    // Now create the Claude adapter's config file and verify it is detected.
    // config_paths returns ~/.claude/settings.json, so we need the file.
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(claude_dir.join("settings.json"), "{}").unwrap();
    let tools = super::detect_installed_tools();
    assert!(
        tools.contains(&"claude-code".to_string()),
        "expected claude-code detected after creating .claude/settings.json, got: {tools:?}"
    );

    // Cleanup: restore original state.
    if already_existed {
        std::fs::create_dir_all(&claude_dir).unwrap();
    } else {
        std::fs::remove_dir_all(&claude_dir).unwrap();
    }
}
