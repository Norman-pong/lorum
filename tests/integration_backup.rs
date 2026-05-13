//! Backup integration tests.

use serial_test::serial;
use std::fs;

// ---------------------------------------------------------------------------
// backup create produces a backup with the new timestamp format
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn backup_create_produces_new_timestamp_format() {
    let dir = tempfile::tempdir().unwrap();

    // Create a mock config file in the temp home directory
    let config_dir = dir.path().join(".claude");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("settings.json"), r#"{"mcpServers": {}}"#).unwrap();

    let orig_home = std::env::var_os("HOME");
    let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", dir.path());
    }

    let result = std::panic::catch_unwind(|| {
        lorum::commands::run_backup_create(&["claude-code".to_string()], false, None).unwrap();
    });

    if let Some(x) = orig_xdg {
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", x);
        }
    }
    if let Some(h) = orig_home {
        unsafe {
            std::env::set_var("HOME", h);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }

    result.unwrap();

    // Verify backup was created with YYYYMMDD-HHMMSS format
    let backup_dir = dir.path().join(".config").join("lorum").join("backups");
    let entries: Vec<_> = fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty());

    let name = entries[0].file_name().to_str().unwrap().to_string();
    // Name should be like "claude-code-20240512-223000.json"
    let parts: Vec<_> = name.split('-').collect();
    assert_eq!(parts[0], "claude");
    assert_eq!(parts[1], "code");
    // parts[2] = YYYYMMDD, parts[3] = HHMMSS.ext
    assert_eq!(parts[2].len(), 8); // YYYYMMDD
    assert!(parts[3].contains('.')); // HHMMSS.json
}

// ---------------------------------------------------------------------------
// backup list shows backups with time and size
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn backup_list_shows_backup_details() {
    let dir = tempfile::tempdir().unwrap();
    let backup_dir = dir.path().join("lorum").join("backups");
    fs::create_dir_all(&backup_dir).unwrap();

    // Create two backups in new format
    fs::write(
        backup_dir.join("codex-20241231-235959.toml"),
        "newer content here",
    )
    .unwrap();
    fs::write(backup_dir.join("codex-20240101-000000.toml"), "older").unwrap();

    // Patch backup_dir to our temp dir by setting config dir
    let orig_config = std::env::var_os("XDG_CONFIG_HOME");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }

    let result = std::panic::catch_unwind(|| {
        lorum::commands::run_backup_list(None).unwrap();
    });

    if let Some(c) = orig_config {
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", c);
        }
    } else {
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    result.unwrap();
}

// ---------------------------------------------------------------------------
// backup restore from latest picks the newest backup
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn backup_restore_from_latest_picks_newest() {
    let dir = tempfile::tempdir().unwrap();
    let backup_dir = dir.path().join("lorum").join("backups");
    fs::create_dir_all(&backup_dir).unwrap();

    let old_path = backup_dir.join("test-tool-20240101-000000.json");
    let new_path = backup_dir.join("test-tool-20241231-235959.json");
    fs::write(&old_path, "old").unwrap();
    fs::write(&new_path, "new").unwrap();

    let target = dir.path().join("restored.json");

    let orig_config = std::env::var_os("XDG_CONFIG_HOME");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }

    let result = std::panic::catch_unwind(|| {
        lorum::backup::restore_backup("test-tool", &target).unwrap();
    });

    if let Some(c) = orig_config {
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", c);
        }
    } else {
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    result.unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
}

// ---------------------------------------------------------------------------
// backup restore from specific file
// ---------------------------------------------------------------------------

#[test]
fn backup_restore_from_specific_file() {
    let dir = tempfile::tempdir().unwrap();
    let backup_dir = dir.path().join("lorum").join("backups");
    fs::create_dir_all(&backup_dir).unwrap();

    let specific = backup_dir.join("test-tool-20240615-120000.json");
    fs::write(&specific, "specific content").unwrap();

    let target = dir.path().join("restored.json");

    lorum::backup::restore_backup_from_path(&specific, &target).unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "specific content");
}

// ---------------------------------------------------------------------------
// backup create --all creates backups for all tools with config
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn backup_create_all_creates_for_all_tools() {
    let dir = tempfile::tempdir().unwrap();

    // Create mock configs for two tools
    let claude_dir = dir.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(claude_dir.join("settings.json"), "{}").unwrap();

    let codex_dir = dir.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(codex_dir.join("config.toml"), "{}").unwrap();

    let orig_home = std::env::var_os("HOME");
    let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", dir.path());
    }

    let result = std::panic::catch_unwind(|| {
        lorum::commands::run_backup_create(&[], true, None).unwrap();
    });

    if let Some(x) = orig_xdg {
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", x);
        }
    }
    if let Some(h) = orig_home {
        unsafe {
            std::env::set_var("HOME", h);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }

    result.unwrap();

    let backup_dir = dir.path().join(".config").join("lorum").join("backups");
    let entries: Vec<_> = fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    // Should have at least 2 backups (claude-code and codex)
    assert!(
        entries.len() >= 2,
        "expected at least 2 backups, got {}",
        entries.len()
    );
}
