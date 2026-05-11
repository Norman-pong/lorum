//! Unit tests for the sync engine.

use super::*;
use crate::adapters::test_utils::make_server;

#[allow(clippy::type_complexity)]
fn make_config(entries: &[(&str, &str, &[&str], &[(&str, &str)])]) -> McpConfig {
    McpConfig {
        servers: entries
            .iter()
            .map(|(name, cmd, args, env)| ((*name).into(), make_server(cmd, args, env)))
            .collect(),
    }
}

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
