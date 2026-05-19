//! End-to-end integration tests for the `doctor` command.
//!
//! These tests exercise `run_doctor` (syntax validation) and
//! `run_doctor_consistency` (consistency checks) against temporary
//! configurations so no real user files are touched.
//!
//! # Dimensions covered
//!
//! - MCP doctor (original tests)
//! - Hooks adapters validation (F3)
//! - Skills adapters validation (F3)
//! - New adapters coverage: opencode hooks, codex skills, kimi skills (F3)

use lorum::adapters::{
    all_hooks_adapters, all_skills_adapters, find_hooks_adapter, find_skills_adapter,
};
use lorum::commands::doctor::{run_doctor, run_doctor_consistency};
use lorum::commands::run_check;
use lorum::config::{HookHandler, HooksConfig};

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
        if let lorum::sync::DimensionDiff::Mcp(d) = diff {
            assert_eq!(d.added, vec!["test-srv"]);
            assert_eq!(d.removed, vec!["other-srv"]);
            assert!(d.modified.is_empty());
        } else {
            panic!("expected Mcp variant in DimensionDiff");
        }
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

// ---------------------------------------------------------------------------
// F3.5 – Verify all hooks adapters are registered (6 adapters)
// ---------------------------------------------------------------------------

#[test]
fn test_all_hooks_adapters_registered_in_doctor() {
    let adapters = all_hooks_adapters();
    assert_eq!(adapters.len(), 6, "expected 6 hooks adapters");

    let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"kimi"));
    assert!(names.contains(&"cursor"));
    assert!(names.contains(&"codex"));
    assert!(names.contains(&"windsurf"));
    assert!(names.contains(&"opencode"));
}

// ---------------------------------------------------------------------------
// F3.6 – Verify all skills adapters are registered (8 adapters)
// ---------------------------------------------------------------------------

#[test]
fn test_all_skills_adapters_registered_in_doctor() {
    let adapters = all_skills_adapters();
    assert_eq!(adapters.len(), 8, "expected 8 skills adapters");

    let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"codex"));
    assert!(names.contains(&"cursor"));
    assert!(names.contains(&"kimi"));
    assert!(names.contains(&"opencode"));
    assert!(names.contains(&"proma"));
    assert!(names.contains(&"trae"));
    assert!(names.contains(&"windsurf"));
}

// ---------------------------------------------------------------------------
// F3.7 – Find hooks adapter for Phase D adapters (opencode)
// ---------------------------------------------------------------------------

#[test]
fn test_find_hooks_adapter_phase_d_adapters() {
    // OpenCode HooksAdapter (Phase D1)
    let opencode = find_hooks_adapter("opencode");
    assert!(
        opencode.is_some(),
        "opencode hooks adapter should be registered"
    );
    assert_eq!(opencode.unwrap().name(), "opencode");

    // Codex HooksAdapter (already existed, verify it is present)
    let codex = find_hooks_adapter("codex");
    assert!(codex.is_some(), "codex hooks adapter should be registered");
    assert_eq!(codex.unwrap().name(), "codex");
}

// ---------------------------------------------------------------------------
// F3.8 – Find skills adapter for Phase E adapters (codex, kimi)
// ---------------------------------------------------------------------------

#[test]
fn test_find_skills_adapter_phase_e_adapters() {
    // Codex SkillsAdapter (Phase E1)
    let codex = find_skills_adapter("codex");
    assert!(codex.is_some(), "codex skills adapter should be registered");
    assert_eq!(codex.unwrap().name(), "codex");

    // Kimi SkillsAdapter (Phase E2)
    let kimi = find_skills_adapter("kimi");
    assert!(kimi.is_some(), "kimi skills adapter should be registered");
    assert_eq!(kimi.unwrap().name(), "kimi");
}

// ---------------------------------------------------------------------------
// F3.9 – OpenCode hooks read/write roundtrip via temp dir
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_opencode_hooks_read_write_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");

    let result = std::panic::catch_unwind(|| {
        unsafe { std::env::set_var("HOME", dir.path()) };

        let adapter = find_hooks_adapter("opencode").expect("opencode hooks adapter");

        // Write hooks config.
        let mut hooks = HooksConfig::default();
        hooks.events.insert(
            "pre-tool-use".into(),
            vec![HookHandler {
                matcher: "Bash".into(),
                command: "check.sh".into(),
                timeout: Some(30),
                handler_type: Some("command".into()),
            }],
        );
        hooks.events.insert(
            "session-start".into(),
            vec![HookHandler {
                matcher: "*".into(),
                command: "init.sh".into(),
                timeout: None,
                handler_type: None,
            }],
        );
        adapter.write_hooks(&hooks).unwrap();

        // Read back and verify.
        let read = adapter.read_hooks().unwrap();
        assert_eq!(read.events.len(), 2);
        assert_eq!(read.events["pre-tool-use"].len(), 1);
        assert_eq!(read.events["pre-tool-use"][0].command, "check.sh");
        assert_eq!(read.events["pre-tool-use"][0].timeout, Some(30));
        assert_eq!(read.events["session-start"].len(), 1);
        assert_eq!(read.events["session-start"][0].command, "init.sh");
    });

    unsafe {
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// F3.10 – Codex skills read/write roundtrip via temp dir
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_codex_skills_read_write_roundtrip() {
    let home = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");

    let result = std::panic::catch_unwind(|| {
        unsafe { std::env::set_var("HOME", home.path()) };

        let adapter = find_skills_adapter("codex").expect("codex skills adapter");

        // Create a source skill directory.
        let src = tempfile::tempdir().unwrap();
        std::fs::write(
            src.path().join("SKILL.md"),
            "---\nname: integration-test\ndescription: \"Integration test skill\"\n---\n",
        )
        .unwrap();

        // Write skill.
        adapter.write_skill("integration-test", src.path()).unwrap();

        // Read back.
        let skills = adapter.read_skills().unwrap();
        assert!(
            skills.iter().any(|s| s.manifest.name == "integration-test"),
            "codex should have integration-test skill"
        );

        // Remove skill.
        adapter.remove_skill("integration-test").unwrap();
        let skills = adapter.read_skills().unwrap();
        assert!(
            !skills.iter().any(|s| s.manifest.name == "integration-test"),
            "codex should no longer have integration-test skill"
        );
    });

    unsafe {
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// F3.11 – Kimi skills read/write roundtrip via temp dir
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_kimi_skills_read_write_roundtrip() {
    let home = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");

    let result = std::panic::catch_unwind(|| {
        unsafe { std::env::set_var("HOME", home.path()) };

        let adapter = find_skills_adapter("kimi").expect("kimi skills adapter");

        // Create a source skill directory.
        let src = tempfile::tempdir().unwrap();
        std::fs::write(
            src.path().join("SKILL.md"),
            "---\nname: kimi-test\ndescription: \"Kimi integration test\"\n---\n",
        )
        .unwrap();

        // Write skill.
        adapter.write_skill("kimi-test", src.path()).unwrap();

        // Read back.
        let skills = adapter.read_skills().unwrap();
        assert!(
            skills.iter().any(|s| s.manifest.name == "kimi-test"),
            "kimi should have kimi-test skill"
        );

        // Remove skill.
        adapter.remove_skill("kimi-test").unwrap();
        let skills = adapter.read_skills().unwrap();
        assert!(
            !skills.iter().any(|s| s.manifest.name == "kimi-test"),
            "kimi should no longer have kimi-test skill"
        );
    });

    unsafe {
        match original_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// F3.12 – Doctor detects broken config for new adapters
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_doctor_detects_broken_opencode_config() {
    let dir = tempfile::tempdir().unwrap();
    let original_home = std::env::var_os("HOME");
    let original_xdg = std::env::var_os("XDG_CONFIG_HOME");

    let result = std::panic::catch_unwind(|| {
        unsafe { std::env::set_var("HOME", dir.path()) };
        unsafe { std::env::remove_var("XDG_CONFIG_HOME") };

        // Create a broken opencode config.
        let opencode_dir = dir.path().join(".config").join("opencode");
        std::fs::create_dir_all(&opencode_dir).unwrap();
        std::fs::write(opencode_dir.join("opencode.json"), r#"{"broken": json}"#).unwrap();

        let results = run_doctor(&["opencode".into()]).unwrap();
        assert_eq!(results.len(), 1);
        let report = &results[0];
        assert_eq!(report.tool, "opencode");
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
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// F3.13 – Hooks event mapping roundtrip for Phase D/E adapters
// ---------------------------------------------------------------------------

#[test]
fn test_hooks_event_mapping_opencode() {
    let adapter = find_hooks_adapter("opencode").expect("opencode hooks adapter");

    // kebab -> camelCase
    assert_eq!(
        adapter.lorum_to_tool_event("pre-tool-use"),
        Some("preToolUse".to_string())
    );
    assert_eq!(
        adapter.lorum_to_tool_event("session-start"),
        Some("sessionStart".to_string())
    );

    // camelCase -> kebab
    assert_eq!(
        adapter.tool_to_lorum_event("preToolUse"),
        Some("pre-tool-use".to_string())
    );
    assert_eq!(
        adapter.tool_to_lorum_event("sessionStart"),
        Some("session-start".to_string())
    );

    // Roundtrip
    for event in &["pre-tool-use", "post-tool-use", "session-start"] {
        let tool_event = adapter.lorum_to_tool_event(event).unwrap();
        let back = adapter.tool_to_lorum_event(&tool_event).unwrap();
        assert_eq!(back, *event, "roundtrip failed for {event}");
    }
}

#[test]
fn test_hooks_event_mapping_codex() {
    let adapter = find_hooks_adapter("codex").expect("codex hooks adapter");

    // Codex uses PascalCase with special "Stop" mapping.
    assert_eq!(
        adapter.lorum_to_tool_event("session-end"),
        Some("Stop".to_string())
    );
    assert_eq!(
        adapter.tool_to_lorum_event("Stop"),
        Some("session-end".to_string())
    );

    // Roundtrip for standard events.
    for event in &["pre-tool-use", "post-tool-use", "session-start"] {
        let tool_event = adapter.lorum_to_tool_event(event).unwrap();
        let back = adapter.tool_to_lorum_event(&tool_event).unwrap();
        assert_eq!(back, *event, "roundtrip failed for {event}");
    }
}
