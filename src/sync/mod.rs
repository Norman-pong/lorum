//! Synchronisation engine for MCP configurations and rules files.
//!
//! The sync engine copies the unified MCP configuration to every registered
//! tool adapter. Each adapter's [`write_mcp`](crate::adapters::ToolAdapter::write_mcp)
//! method is called, and a [`SyncResult`] is produced per tool so that a
//! single failure does not block the others.
//!
//! # Rules sync
//!
//! The engine also supports syncing rules content to all registered
//! [`RulesAdapter`](crate::adapters::RulesAdapter) instances via
//! [`sync_rules_all`] and [`sync_rules_tools`]. The dry-run counterparts
//! [`dry_run_rules_all`] and [`dry_run_rules_tools`] preview which tools
//! need an update without writing anything.
//!
//! # Dry-run mode
//!
//! [`dry_run_all`] previews what would change without writing anything.
//! It compares each tool's current configuration against the target and
//! reports the diff via [`ConfigDiff`].

use std::path::Path;

use crate::adapters::{
    ToolAdapter, all_adapters, all_rules_adapters, find_adapter, find_rules_adapter,
};
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

/// Preview sync results for specified tools only.
///
/// Tools that are not found in the registered adapters produce a failed
/// [`DryRunResult`] with an appropriate error message.
pub fn dry_run_tools(mcp_config: &McpConfig, tool_names: &[String]) -> Vec<DryRunResult> {
    let mut results = Vec::new();
    for name in tool_names {
        match find_adapter(name) {
            Some(adapter) => {
                let adapter_name = adapter.name().to_string();
                match adapter.read_mcp() {
                    Ok(current) => results.push(DryRunResult {
                        tool: adapter_name,
                        success: true,
                        diff: Some(compute_diff(&current, mcp_config)),
                        error: None,
                    }),
                    Err(e) => results.push(DryRunResult {
                        tool: adapter_name,
                        success: false,
                        diff: None,
                        error: Some(e.to_string()),
                    }),
                }
            }
            None => {
                let err = LorumError::AdapterNotFound { name: name.clone() };
                results.push(DryRunResult {
                    tool: name.clone(),
                    success: false,
                    diff: None,
                    error: Some(err.to_string()),
                })
            }
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

// ---------------------------------------------------------------------------
// Rules sync
// ---------------------------------------------------------------------------

/// Result of syncing rules to a single tool.
#[derive(Debug)]
pub struct RulesSyncResult {
    /// Name of the tool that was synced.
    pub tool: String,
    /// Whether the sync succeeded.
    pub success: bool,
    /// Error message if the sync failed.
    pub error: Option<String>,
}

/// Result of a dry-run preview for rules syncing.
#[derive(Debug)]
pub struct RulesDryRunResult {
    /// Name of the tool.
    pub tool: String,
    /// Whether the current rules could be read successfully.
    pub success: bool,
    /// Whether the current content differs from the target content.
    pub needs_update: bool,
    /// Error message if the current rules could not be read.
    pub error: Option<String>,
}

/// Sync rules content to all registered rules adapters.
///
/// Each adapter is synced independently; a failure for one tool does not
/// affect the others. Before writing, the existing file (if any) is backed
/// up via [`crate::backup::create_backup`].
pub fn sync_rules_all(project_root: &Path, content: &str) -> Vec<RulesSyncResult> {
    let mut results = Vec::new();
    for adapter in all_rules_adapters() {
        let result = sync_rules_adapter(&*adapter, project_root, content);
        results.push(result);
    }
    results
}

/// Sync rules content to specified tools only.
///
/// Tools that are not found in the registered rules adapters produce a failed
/// [`RulesSyncResult`] with an appropriate error message.
pub fn sync_rules_tools(
    project_root: &Path,
    content: &str,
    tool_names: &[String],
) -> Vec<RulesSyncResult> {
    let mut results = Vec::new();
    for name in tool_names {
        match find_rules_adapter(name) {
            Some(adapter) => {
                let result = sync_rules_adapter(&*adapter, project_root, content);
                results.push(result);
            }
            None => {
                let err = LorumError::AdapterNotFound { name: name.clone() };
                results.push(RulesSyncResult {
                    tool: name.clone(),
                    success: false,
                    error: Some(err.to_string()),
                });
            }
        }
    }
    results
}

/// Sync a single rules adapter.
///
/// Backs up the existing file (if present) before writing.
fn sync_rules_adapter(
    adapter: &dyn crate::adapters::RulesAdapter,
    project_root: &Path,
    content: &str,
) -> RulesSyncResult {
    let name = adapter.name().to_string();
    let path = adapter.rules_path(project_root);

    // Backup existing file before overwriting.
    if path.exists() {
        if let Err(e) = crate::backup::create_backup(&name, &path) {
            // Backup failure should not block the sync, but log a warning.
            eprintln!("warning: failed to backup {}: {e}", path.display());
        }
    }

    match adapter.write_rules(project_root, content) {
        Ok(()) => RulesSyncResult {
            tool: name,
            success: true,
            error: None,
        },
        Err(e) => RulesSyncResult {
            tool: name,
            success: false,
            error: Some(e.to_string()),
        },
    }
}

/// Preview rules sync results without writing anything.
///
/// For each registered rules adapter, reads the current rules file and
/// compares it against the target content. No files are modified.
pub fn dry_run_rules_all(project_root: &Path, content: &str) -> Vec<RulesDryRunResult> {
    let mut results = Vec::new();
    for adapter in all_rules_adapters() {
        let name = adapter.name().to_string();
        match adapter.read_rules(project_root) {
            Ok(current) => {
                let needs_update = current.as_deref() != Some(content);
                results.push(RulesDryRunResult {
                    tool: name,
                    success: true,
                    needs_update,
                    error: None,
                });
            }
            Err(e) => results.push(RulesDryRunResult {
                tool: name,
                success: false,
                needs_update: false,
                error: Some(e.to_string()),
            }),
        }
    }
    results
}

/// Preview rules sync results for specified tools only.
///
/// Tools that are not found in the registered rules adapters produce a failed
/// [`RulesDryRunResult`] with an appropriate error message.
pub fn dry_run_rules_tools(
    project_root: &Path,
    content: &str,
    tool_names: &[String],
) -> Vec<RulesDryRunResult> {
    let mut results = Vec::new();
    for name in tool_names {
        match find_rules_adapter(name) {
            Some(adapter) => {
                let adapter_name = adapter.name().to_string();
                match adapter.read_rules(project_root) {
                    Ok(current) => {
                        let needs_update = current.as_deref() != Some(content);
                        results.push(RulesDryRunResult {
                            tool: adapter_name,
                            success: true,
                            needs_update,
                            error: None,
                        });
                    }
                    Err(e) => results.push(RulesDryRunResult {
                        tool: adapter_name,
                        success: false,
                        needs_update: false,
                        error: Some(e.to_string()),
                    }),
                }
            }
            None => {
                let err = LorumError::AdapterNotFound { name: name.clone() };
                results.push(RulesDryRunResult {
                    tool: name.clone(),
                    success: false,
                    needs_update: false,
                    error: Some(err.to_string()),
                });
            }
        }
    }
    results
}

#[cfg(test)]
mod tests;
