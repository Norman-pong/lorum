//! Tool adapter framework for reading/writing MCP configurations.
//!
//! Each AI coding tool has its own configuration file format and location.
//! The [`ToolAdapter`](crate::adapters::ToolAdapter) trait provides a uniform interface for reading and
//! writing MCP server configurations across these tools.
//!
//! The [`RulesAdapter`](crate::adapters::RulesAdapter) trait provides a uniform interface for reading and
//! writing rules files across tools that support them.

use std::path::{Path, PathBuf};

use crate::config::McpConfig;
use crate::error::LorumError;

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod json_utils;
pub mod kimi;
pub mod proma;
pub mod toml_utils;
pub mod trae;
pub mod windsurf;

/// Per-tool adapter that can read/write MCP configuration.
///
/// Implementors define how to interact with a specific AI coding tool's
/// configuration file, including its format (JSON/TOML), location, and
/// the field name used for MCP servers.
pub trait ToolAdapter {
    /// Human-readable name of the tool (e.g. "claude-code").
    fn name(&self) -> &str;

    /// Paths where this tool stores configuration.
    ///
    /// Returns multiple paths for tools with global + project-level config.
    fn config_paths(&self) -> Vec<PathBuf>;

    /// Read MCP servers from the tool's configuration.
    ///
    /// Returns an empty [`McpConfig`] when the configuration file does not
    /// exist, rather than an error.
    fn read_mcp(&self) -> Result<McpConfig, LorumError>;

    /// Write MCP servers to the tool's configuration.
    ///
    /// Must preserve non-MCP fields in the existing config file.
    fn write_mcp(&self, config: &McpConfig) -> Result<(), LorumError>;
}

/// Return all registered adapters.
pub fn all_adapters() -> Vec<Box<dyn ToolAdapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(proma::PromaAdapter),
        Box::new(kimi::KimiAdapter),
        Box::new(trae::TraeAdapter),
    ]
}

/// Find an adapter by name.
pub fn find_adapter(name: &str) -> Option<Box<dyn ToolAdapter>> {
    all_adapters().into_iter().find(|a| a.name() == name)
}

/// Per-tool adapter for reading/writing rules files.
///
/// Implementors define how to interact with a specific AI coding tool's
/// rules file, including its location on disk.
pub trait RulesAdapter {
    /// Human-readable name of the tool.
    fn name(&self) -> &str;

    /// Path where this tool stores its rules file for the given project.
    fn rules_path(&self, project_root: &Path) -> PathBuf;

    /// Read existing rules content from the tool's file.
    ///
    /// Returns `None` if the file does not exist.
    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError>;

    /// Write rules content to the tool's file.
    ///
    /// Creates parent directories if needed.
    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError>;
}

/// Return all registered rules adapters.
pub fn all_rules_adapters() -> Vec<Box<dyn RulesAdapter>> {
    vec![
        Box::new(cursor::CursorRulesAdapter),
        Box::new(windsurf::WindsurfRulesAdapter),
        Box::new(codex::CodexRulesAdapter),
    ]
}

/// Find a rules adapter by name.
pub fn find_rules_adapter(name: &str) -> Option<Box<dyn RulesAdapter>> {
    all_rules_adapters().into_iter().find(|a| a.name() == name)
}

/// Shared test utilities for adapter tests.
#[cfg(test)]
pub(crate) mod test_utils {
    use crate::config::McpServer;

    /// Create a test [`McpServer`] with the given command, args, and env.
    pub fn make_server(command: &str, args: &[&str], env: &[(&str, &str)]) -> McpServer {
        McpServer {
            command: command.into(),
            args: args.iter().map(|s| (*s).into()).collect(),
            env: env
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        }
    }
}
