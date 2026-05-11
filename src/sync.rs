//! Synchronisation engine for MCP configurations.
//!
//! The sync engine copies the unified MCP configuration to every registered
//! tool adapter. Each adapter's [`write_mcp`](crate::adapters::ToolAdapter::write_mcp)
//! method is called, and a [`SyncResult`] is produced per tool so that a
//! single failure does not block the others.
//!
//! # Dry-run mode
//!
//! [`dry_run_all`] previews what would change without writing anything.
//! It compares each tool's current configuration against the target and
//! reports the diff via [`ConfigDiff`].

use crate::adapters::{ToolAdapter, all_adapters, find_adapter};
use crate::config::McpConfig;
use crate::error::LorumError;

/// Result of syncing a single tool.
#[derive(Debug)]
pub struct SyncResult {
    /// Name of the tool that was synced.
    pub tool: String,
    /// Whether the sync succeeded.
    pub success: bool,
    /// Number of MCP servers that were synced.
    pub servers_synced: usize,
    /// Error message if the sync failed.
    pub error: Option<String>,
}

/// Diff between current and target MCP configurations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiff {
    /// Server names present in target but not in current.
    pub added: Vec<String>,
    /// Server names present in current but not in target.
    pub removed: Vec<String>,
    /// Server names present in both but with different configurations.
    pub modified: Vec<String>,
    /// Server names identical in both.
    pub unchanged: Vec<String>,
}

impl ConfigDiff {
    /// Returns `true` if there are no changes (added, removed, or modified).
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    /// Returns the total number of changes (added + removed + modified).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

/// Result of a dry-run preview for a single tool.
#[derive(Debug)]
pub struct DryRunResult {
    /// Name of the tool.
    pub tool: String,
    /// Whether the current config could be read successfully.
    pub success: bool,
    /// Diff between current and target configurations.
    pub diff: Option<ConfigDiff>,
    /// Error message if the current config could not be read.
    pub error: Option<String>,
}

/// Sync MCP configuration to all registered adapters.
///
/// Each adapter is synced independently; a failure for one tool does not
/// affect the others.
pub fn sync_all(mcp_config: &McpConfig) -> Vec<SyncResult> {
    let mut results = Vec::new();
    for adapter in all_adapters() {
        let result = sync_tool(&*adapter, mcp_config);
        results.push(result);
    }
    results
}

/// Sync MCP configuration to specified tools only.
///
/// Tools that are not found in the registered adapters produce a failed
/// [`SyncResult`] with an appropriate error message.
pub fn sync_tools(mcp_config: &McpConfig, tool_names: &[String]) -> Vec<SyncResult> {
    let mut results = Vec::new();
    for name in tool_names {
        match find_adapter(name) {
            Some(adapter) => results.push(sync_tool(&*adapter, mcp_config)),
            None => {
                let err = LorumError::AdapterNotFound { name: name.clone() };
                results.push(SyncResult {
                    tool: name.clone(),
                    success: false,
                    servers_synced: 0,
                    error: Some(err.to_string()),
                })
            }
        }
    }
    results
}

/// Sync a single adapter.
fn sync_tool(adapter: &dyn ToolAdapter, mcp_config: &McpConfig) -> SyncResult {
    let name = adapter.name().to_string();

    // Backup existing configuration before overwriting.
    for path in adapter.config_paths() {
        if path.exists() {
            if let Err(e) = crate::backup::create_backup(&name, &path) {
                // Backup failure should not block the sync, but log a warning.
                eprintln!("warning: failed to backup {}: {e}", path.display());
            }
        }
    }

    match adapter.write_mcp(mcp_config) {
        Ok(()) => SyncResult {
            tool: name,
            success: true,
            servers_synced: mcp_config.servers.len(),
            error: None,
        },
        Err(e) => SyncResult {
            tool: name,
            success: false,
            servers_synced: 0,
            error: Some(e.to_string()),
        },
    }
}

/// Preview sync results without writing anything.
///
/// For each registered adapter, reads the current configuration and
/// computes a [`ConfigDiff`] against the target. No files are modified.
pub fn dry_run_all(mcp_config: &McpConfig) -> Vec<DryRunResult> {
    let mut results = Vec::new();
    for adapter in all_adapters() {
        let name = adapter.name().to_string();
        match adapter.read_mcp() {
            Ok(current) => results.push(DryRunResult {
                tool: name,
                success: true,
                diff: Some(compute_diff(&current, mcp_config)),
                error: None,
            }),
            Err(e) => results.push(DryRunResult {
                tool: name,
                success: false,
                diff: None,
                error: Some(e.to_string()),
            }),
        }
    }
    results
}

/// Compute the diff between current and target MCP configs.
///
/// Servers are compared by name and by value. A server is "modified" if
/// it exists in both but with different command, args, or env.
pub fn compute_diff(current: &McpConfig, target: &McpConfig) -> ConfigDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged = Vec::new();

    for name in current.servers.keys() {
        if !target.servers.contains_key(name) {
            removed.push(name.clone());
        } else if current.servers.get(name) != target.servers.get(name) {
            modified.push(name.clone());
        } else {
            unchanged.push(name.clone());
        }
    }
    for name in target.servers.keys() {
        if !current.servers.contains_key(name) {
            added.push(name.clone());
        }
    }

    ConfigDiff {
        added,
        removed,
        modified,
        unchanged,
    }
}

#[cfg(test)]
mod tests {
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
}
