//! End-to-end integration tests for the `doctor` command.
//!
//! These tests exercise `run_doctor` (syntax validation) and
//! `run_doctor_consistency` (consistency checks) against temporary
//! configurations so no real user files are touched.

use lorum::commands::doctor::{run_doctor, run_doctor_consistency};
use lorum::commands::run_check;

// ---------------------------------------------------------------------------
// F3.1 – doctor detects a broken tool config file
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_doctor_command_detects_broken_tool_config() {
    let dir = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");
    let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let original_cwd = std::env::current_dir().unwrap();

    let result = std::panic::catch_unwind(|| {
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        // Create a broken cursor config file in the current directory.
        std::env::set_current_dir(dir.path()).unwrap();
        let cursor_dir = dir.path().join(".cursor");
        std::fs::create_dir_all(&cursor_dir).unwrap();
        std::fs::write(cursor_dir.join("mcp.json"), r#"{"broken": json}"#).unwrap();

        let results = run_doctor(&["cursor".into()]).unwrap();
        assert_eq!(results.len(), 1);
        let report = &results[0];
        assert_eq!(report.tool, "cursor");
        assert!(!report.healthy, "expected broken config to be unhealthy");
        let has_errors = report
            .issues
            .iter()
            .any(|i| i.severity == lorum::adapters::Severity::Error);
        assert!(has_errors, "expected at least one Error-level issue");
    });

    unsafe {
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
    match original_xdg {
        Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
    std::env::set_current_dir(original_cwd).unwrap();

    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// F3.2 – doctor consistency detects drift between lorum and tool configs
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_doctor_command_detects_consistency_drift() {
    let dir = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");
    let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let original_cwd = std::env::current_dir().unwrap();

    let result = std::panic::catch_unwind(|| {
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        // Lorum global config has server "test-srv".
        let lorum_dir = dir.path().join(".config").join("lorum");
        std::fs::create_dir_all(&lorum_dir).unwrap();
        std::fs::write(
            lorum_dir.join("config.yaml"),
            "mcp:\n  servers:\n    test-srv:\n      command: echo\n      args: []\n      env: {}\n",
        )
        .unwrap();

        // Change cwd to a subdir (no project-level config).
        let cwd_dir = dir.path().join("workspace");
        std::fs::create_dir_all(&cwd_dir).unwrap();
        std::env::set_current_dir(&cwd_dir).unwrap();

        // Cursor config has a different server "other-srv".
        let cursor_dir = cwd_dir.join(".cursor");
        std::fs::create_dir_all(&cursor_dir).unwrap();
        std::fs::write(
            cursor_dir.join("mcp.json"),
            r#"{"mcpServers":{"other-srv":{"command":"node","args":["server.js"]}}}"#,
        )
        .unwrap();

        let reports = run_doctor_consistency(&["cursor".into()]).unwrap();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(report.tool, "cursor");
        assert!(!report.consistent, "expected drift to be detected");
        assert!(report.error.is_none());
        let diff = report.diff.as_ref().expect("diff should be present");
        assert_eq!(diff.added, vec!["test-srv"]);
        assert_eq!(diff.removed, vec!["other-srv"]);
        assert!(diff.modified.is_empty());
    });

    unsafe {
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
    match original_xdg {
        Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
    std::env::set_current_dir(original_cwd).unwrap();

    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// F3.3 – doctor filters tools with --tools
// ---------------------------------------------------------------------------

#[test]
fn test_doctor_command_filters_tools() {
    // run_doctor with explicit tool list returns only those tools.
    let results = run_doctor(&["cursor".into(), "kimi".into()]).unwrap();
    assert_eq!(results.len(), 2);
    let names: Vec<&str> = results.iter().map(|r| r.tool.as_str()).collect();
    assert!(names.contains(&"cursor"));
    assert!(names.contains(&"kimi"));
    assert!(!names.contains(&"claude-code"));

    // Same for consistency.
    let reports = run_doctor_consistency(&["cursor".into(), "kimi".into()]).unwrap();
    assert_eq!(reports.len(), 2);
    let names: Vec<&str> = reports.iter().map(|r| r.tool.as_str()).collect();
    assert!(names.contains(&"cursor"));
    assert!(names.contains(&"kimi"));
}

// ---------------------------------------------------------------------------
// F3.4 – check command is backward-compatible
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_check_command_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");
    let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let original_cwd = std::env::current_dir().unwrap();

    let result = std::panic::catch_unwind(|| {
        unsafe {
            std::env::set_var("HOME", dir.path());
        }
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        // Create a valid global lorum config with a real command.
        let lorum_dir = dir.path().join(".config").join("lorum");
        std::fs::create_dir_all(&lorum_dir).unwrap();
        std::fs::write(
            lorum_dir.join("config.yaml"),
            "mcp:\n  servers:\n    test-srv:\n      command: echo\n      args: [hello]\n      env: {}\n",
        )
        .unwrap();

        // Change cwd to a subdir (no project-level config).
        let cwd_dir = dir.path().join("workspace");
        std::fs::create_dir_all(&cwd_dir).unwrap();
        std::env::set_current_dir(&cwd_dir).unwrap();

        // run_check should succeed for a valid config.
        let check_result = run_check(None);
        assert!(
            check_result.is_ok(),
            "check command should succeed for valid config: {:?}",
            check_result.err()
        );
    });

    unsafe {
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
    match original_xdg {
        Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
    std::env::set_current_dir(original_cwd).unwrap();

    assert!(result.is_ok());
}
