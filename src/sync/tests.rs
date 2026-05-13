//! Unit tests for the sync engine.

use super::*;
use crate::adapters::RulesAdapter;
use crate::adapters::test_utils::make_server;
use serial_test::serial;

#[allow(clippy::type_complexity)]
fn make_config(entries: &[(&str, &str, &[&str], &[(&str, &str)])]) -> McpConfig {
    McpConfig {
        servers: entries
            .iter()
            .map(|(name, cmd, args, env)| ((*name).into(), make_server(cmd, args, env)))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// MCP sync tests
// ---------------------------------------------------------------------------

#[test]
fn compute_diff_empty_to_empty() {
    let current = McpConfig::default();
    let target = McpConfig::default();
    let diff = compute_diff(&current, &target);
    assert!(diff.is_empty());
    assert_eq!(diff.change_count(), 0);
}

#[test]
fn compute_diff_detects_additions() {
    let current = McpConfig::default();
    let target = make_config(&[("new-server", "cmd", &[], &[])]);
    let diff = compute_diff(&current, &target);
    assert_eq!(diff.added, vec!["new-server"]);
    assert!(diff.removed.is_empty());
    assert!(diff.modified.is_empty());
    assert!(diff.unchanged.is_empty());
    assert_eq!(diff.change_count(), 1);
}

#[test]
fn compute_diff_detects_removals() {
    let current = make_config(&[("old-server", "cmd", &[], &[])]);
    let target = McpConfig::default();
    let diff = compute_diff(&current, &target);
    assert!(diff.added.is_empty());
    assert_eq!(diff.removed, vec!["old-server"]);
    assert!(diff.modified.is_empty());
    assert!(diff.unchanged.is_empty());
}

#[test]
fn compute_diff_detects_modifications() {
    let current = make_config(&[("server", "old-cmd", &[], &[])]);
    let target = make_config(&[("server", "new-cmd", &[], &[])]);
    let diff = compute_diff(&current, &target);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert_eq!(diff.modified, vec!["server"]);
    assert!(diff.unchanged.is_empty());
}

#[test]
fn compute_diff_detects_unchanged() {
    let config = make_config(&[("server", "cmd", &["a"], &[])]);
    let diff = compute_diff(&config, &config);
    assert!(diff.is_empty());
    assert_eq!(diff.unchanged, vec!["server"]);
}

#[test]
fn compute_diff_mixed_changes() {
    let current = make_config(&[
        ("kept", "cmd", &[], &[]),
        ("changed", "old", &[], &[]),
        ("removed", "cmd", &[], &[]),
    ]);
    let target = make_config(&[
        ("kept", "cmd", &[], &[]),
        ("changed", "new", &[], &[]),
        ("added", "cmd", &[], &[]),
    ]);
    let diff = compute_diff(&current, &target);
    assert_eq!(diff.added, vec!["added"]);
    assert_eq!(diff.removed, vec!["removed"]);
    assert_eq!(diff.modified, vec!["changed"]);
    assert_eq!(diff.unchanged, vec!["kept"]);
    assert!(!diff.is_empty());
    assert_eq!(diff.change_count(), 3);
}

#[test]
fn sync_tools_reports_unknown_adapter() {
    let config = McpConfig::default();
    let results = sync_tools(&config, &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(results[0].error.is_some());
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
fn dry_run_returns_results_for_all_adapters() {
    let config = McpConfig::default();
    let results = dry_run_all(&config);
    assert_eq!(results.len(), all_adapters().len());
    for result in &results {
        assert!(!result.tool.is_empty());
    }
}

#[test]
fn config_diff_is_empty_true_when_no_changes() {
    let diff = ConfigDiff {
        added: vec![],
        removed: vec![],
        modified: vec![],
        unchanged: vec!["x".into()],
    };
    assert!(diff.is_empty());
}

// ---------------------------------------------------------------------------
// Rules sync tests
// ---------------------------------------------------------------------------

#[test]
fn sync_rules_all_writes_to_all_adapters() {
    let dir = tempfile::tempdir().unwrap();
    let content = "Use 4-space indentation.\n";
    let results = sync_rules_all(dir.path(), content);

    // Every registered rules adapter should have a result.
    assert_eq!(results.len(), all_rules_adapters().len());

    // All should succeed (temp dir is writable).
    for result in &results {
        assert!(
            result.success,
            "tool {} failed: {:?}",
            result.tool, result.error
        );
        assert!(result.error.is_none());
    }

    // Verify files were actually written by reading via the adapters.
    for adapter in all_rules_adapters() {
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(
            read,
            Some(content.to_owned()),
            "content mismatch for {}",
            adapter.name()
        );
    }
}

#[test]
fn sync_rules_tools_only_writes_specified_tools() {
    let dir = tempfile::tempdir().unwrap();
    let content = "Prefer functional style.\n";

    let results = sync_rules_tools(dir.path(), content, &["cursor".into()]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool, "cursor");
    assert!(results[0].success);

    // Cursor should have the content.
    let cursor = crate::adapters::cursor::CursorRulesAdapter;
    let read = cursor.read_rules(dir.path()).unwrap();
    assert_eq!(read, Some(content.to_owned()));
}

#[test]
fn sync_rules_tools_unknown_tool_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let results = sync_rules_tools(dir.path(), "content", &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(results[0].error.is_some());
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
fn dry_run_rules_all_detects_needs_update() {
    let dir = tempfile::tempdir().unwrap();
    let content = "New rules content.\n";

    // No files exist yet -- every adapter should need an update.
    let results = dry_run_rules_all(dir.path(), content);
    assert_eq!(results.len(), all_rules_adapters().len());
    for result in &results {
        assert!(result.success);
        assert!(
            result.needs_update,
            "tool {} should need update",
            result.tool
        );
    }
}

#[test]
fn dry_run_rules_reports_no_update_when_content_matches() {
    let dir = tempfile::tempdir().unwrap();
    let content = "Existing rules content.\n";

    // Write content via the cursor adapter so one tool has matching content.
    let cursor = crate::adapters::cursor::CursorRulesAdapter;
    cursor.write_rules(dir.path(), content).unwrap();

    // Dry-run for just cursor -- should report no update needed.
    let results = dry_run_rules_tools(dir.path(), content, &["cursor".into()]);
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert!(!results[0].needs_update);
}

#[test]
fn dry_run_rules_detects_update_when_content_differs() {
    let dir = tempfile::tempdir().unwrap();

    // Write old content.
    let cursor = crate::adapters::cursor::CursorRulesAdapter;
    cursor.write_rules(dir.path(), "old content").unwrap();

    // Dry-run with different content -- should report update needed.
    let results = dry_run_rules_tools(dir.path(), "new content", &["cursor".into()]);
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert!(results[0].needs_update);
}

#[test]
fn dry_run_rules_tools_unknown_tool_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let results = dry_run_rules_tools(dir.path(), "content", &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(results[0].error.is_some());
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
#[serial]
fn sync_rules_creates_backup_when_file_exists() {
    let dir = tempfile::tempdir().unwrap();
    let cursor = crate::adapters::cursor::CursorRulesAdapter;
    let rules_path = cursor.rules_path(dir.path());

    // Write initial content.
    let old_content = "Old rules.\n";
    cursor.write_rules(dir.path(), old_content).unwrap();
    assert!(rules_path.exists());

    // Sync new content -- should create a backup.
    let new_content = "New rules.\n";
    let results = sync_rules_tools(dir.path(), new_content, &["cursor".into()]);
    assert_eq!(results.len(), 1);
    assert!(results[0].success);

    // File should now contain new content.
    let read = cursor.read_rules(dir.path()).unwrap();
    assert_eq!(read, Some(new_content.to_owned()));

    // A backup should have been created.
    let backups = crate::backup::list_backups("cursor").unwrap();
    assert!(
        !backups.is_empty(),
        "expected at least one backup for cursor"
    );

    // At least one backup should contain the old content.
    let found = backups
        .iter()
        .any(|b| std::fs::read_to_string(&b.path).unwrap() == old_content);
    assert!(found, "expected a backup with the old content");
}

// ---------------------------------------------------------------------------
// Hooks sync tests
// ---------------------------------------------------------------------------

use crate::config::{HookHandler, HooksConfig};

fn make_hooks_config(event: &str, matcher: &str, command: &str) -> HooksConfig {
    let mut config = HooksConfig::default();
    config.events.insert(
        event.into(),
        vec![HookHandler {
            matcher: matcher.into(),
            command: command.into(),
            timeout: None,
            handler_type: None,
        }],
    );
    config
}

#[test]
fn sync_hooks_tools_reports_unknown_adapter() {
    let config = make_hooks_config("pre-tool-use", "Bash", "check.sh");
    let results = sync_hooks_tools(&config, &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
fn dry_run_hooks_tools_unknown_tool_returns_error() {
    let config = HooksConfig::default();
    let results = dry_run_hooks_tools(&config, &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

// ---------------------------------------------------------------------------
// Skills sync tests
// ---------------------------------------------------------------------------

#[test]
fn dry_run_skills_all_returns_results_for_all_adapters() {
    let dir = tempfile::tempdir().unwrap();
    let results = dry_run_skills_all(dir.path());
    assert_eq!(results.len(), all_skills_adapters().len());
    for result in &results {
        assert!(!result.tool.is_empty());
    }
}

#[test]
fn dry_run_skills_tools_returns_results_for_specified_tools() {
    let dir = tempfile::tempdir().unwrap();
    let results = dry_run_skills_tools(dir.path(), &["claude-code".into()]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool, "claude-code");
}

#[test]
fn dry_run_skills_tools_unknown_tool_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let results = dry_run_skills_tools(dir.path(), &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
fn dry_run_skills_all_with_skills_detects_updates() {
    let dir = tempfile::tempdir().unwrap();
    let skill_dir = dir.path().join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\ndescription: \"Test\"\n---\n# Body\n",
    )
    .unwrap();

    let results = dry_run_skills_all(dir.path());
    assert_eq!(results.len(), all_skills_adapters().len());
    for result in &results {
        assert!(result.success);
        // Source has one skill that is not present in target, so it should
        // report one skill to update.
        assert_eq!(
            result.skills_to_update, 1,
            "tool {} should report 1 skill to update",
            result.tool
        );
    }
}

// ---------------------------------------------------------------------------
// Additional dry-run tests for MCP and hooks
// ---------------------------------------------------------------------------

#[test]
fn dry_run_tools_returns_results_for_specified_tools() {
    let config = McpConfig::default();
    let results = dry_run_tools(&config, &["claude-code".into()]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool, "claude-code");
}

#[test]
fn dry_run_tools_unknown_tool_returns_error() {
    let config = McpConfig::default();
    let results = dry_run_tools(&config, &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
fn dry_run_hooks_all_returns_results_for_all_adapters() {
    let config = HooksConfig::default();
    let results = dry_run_hooks_all(&config);
    assert_eq!(results.len(), all_hooks_adapters().len());
    for result in &results {
        assert!(!result.tool.is_empty());
    }
}
