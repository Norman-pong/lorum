//! Unit tests for the interactive `init` command.

use serial_test::serial;
use tempfile::TempDir;

use crate::commands::init::run_interactive_init;
use crate::config;

#[test]
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
    let orig_home = std::env::var_os("HOME");
    unsafe {
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    run_interactive_init(false, true).unwrap();

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
