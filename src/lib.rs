//! Tool adapter framework for reading/writing MCP configurations.
pub mod adapters;
pub mod backup;
pub mod commands;
pub mod config;
pub mod env_interpolate;
pub mod error;
pub mod rules;
pub mod skills;
pub mod sync;

/// Dimensions supported by the unified `sync` command.
///
/// Each variant maps to a category of configuration that can be synchronised
/// across registered tool adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDimension {
    /// MCP server configurations.
    Mcp,
    /// Rules files (`.lorum/RULES.md` → tool-specific paths).
    Rules,
    /// Lifecycle hooks.
    Hooks,
    /// Skills directories.
    Skills,
}
