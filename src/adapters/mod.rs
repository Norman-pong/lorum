//! Tool adapter framework for reading/writing MCP configurations.
//!
//! Each AI coding tool has its own configuration file format and location.
//! The [`ToolAdapter`](crate::adapters::ToolAdapter) trait provides a uniform interface for reading and
//! writing MCP server configurations across these tools.
//!
//! The [`RulesAdapter`](crate::adapters::RulesAdapter) trait provides a uniform interface for reading and
//! writing rules files across tools that support them.

use std::path::{Path, PathBuf};

use crate::config::{HooksConfig, McpConfig};
use crate::error::LorumError;
use crate::skills::SkillEntry;

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

/// Read a rules file at `path`, returning `None` if it does not exist.
pub(crate) fn read_rules_file(path: &Path) -> Result<Option<String>, LorumError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    Ok(Some(content))
}

/// Write rules content to `path`, creating parent directories if needed.
pub(crate) fn write_rules_file(path: &Path, content: &str) -> Result<(), LorumError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    std::fs::write(path, content).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Per-tool adapter for reading/writing hooks configurations.
///
/// Implementors define how to interact with a specific AI coding tool's
/// hooks configuration, including its format (JSON/TOML), location, and
/// the field structure used for hooks.
pub trait HooksAdapter {
    /// Human-readable name of the tool (e.g. "claude-code").
    fn name(&self) -> &str;

    /// Paths where this tool stores configuration.
    fn config_paths(&self) -> Vec<PathBuf>;

    /// Read hooks from the tool's configuration.
    ///
    /// Returns an empty [`HooksConfig`] when the configuration file does not
    /// exist or contains no hooks, rather than an error.
    fn read_hooks(&self) -> Result<HooksConfig, LorumError>;

    /// Write hooks to the tool's configuration.
    ///
    /// Must preserve non-hooks fields in the existing config file.
    fn write_hooks(&self, config: &HooksConfig) -> Result<(), LorumError>;
}

/// Return all registered hooks adapters.
pub fn all_hooks_adapters() -> Vec<Box<dyn HooksAdapter>> {
    vec![Box::new(claude::ClaudeAdapter), Box::new(kimi::KimiAdapter)]
}

/// Find a hooks adapter by name.
pub fn find_hooks_adapter(name: &str) -> Option<Box<dyn HooksAdapter>> {
    all_hooks_adapters().into_iter().find(|a| a.name() == name)
}

/// Per-tool adapter for reading/writing skills directories.
///
/// Skills are directory-based entities (each skill is a folder containing
/// SKILL.md and optional auxiliary files). Adapters define where each tool
/// stores its skills.
pub trait SkillsAdapter {
    /// Human-readable name of the tool (e.g. "claude-code").
    fn name(&self) -> &str;

    /// Base directory where this tool stores skills.
    fn skills_base_dir(&self) -> Option<PathBuf>;

    /// Read all skills from the tool's skills directory.
    fn read_skills(&self) -> Result<Vec<SkillEntry>, LorumError>;

    /// Write a single skill (full directory copy) to the tool's skills dir.
    fn write_skill(&self, name: &str, source_dir: &Path) -> Result<(), LorumError>;

    /// Remove a skill directory from the tool's skills dir.
    fn remove_skill(&self, name: &str) -> Result<(), LorumError>;
}

/// Return all registered skills adapters.
pub fn all_skills_adapters() -> Vec<Box<dyn SkillsAdapter>> {
    vec![
        Box::new(claude::ClaudeSkillsAdapter),
        Box::new(proma::PromaSkillsAdapter),
    ]
}

/// Find a skills adapter by name.
pub fn find_skills_adapter(name: &str) -> Option<Box<dyn SkillsAdapter>> {
    all_skills_adapters().into_iter().find(|a| a.name() == name)
}

/// Return the union of all tool names registered across all four adapter dimensions.
///
/// Each tool name appears at most once in the returned vector.
pub fn all_adapter_tool_names() -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    for a in all_adapters() {
        names.insert(a.name().to_string());
    }
    for a in all_rules_adapters() {
        names.insert(a.name().to_string());
    }
    for a in all_hooks_adapters() {
        names.insert(a.name().to_string());
    }
    for a in all_skills_adapters() {
        names.insert(a.name().to_string());
    }
    names.into_iter().collect()
}

/// Convert a kebab-case string to PascalCase.
///
/// # Examples
///
/// ```
/// use lorum::adapters::kebab_to_pascal;
/// assert_eq!(kebab_to_pascal("pre-tool-use"), "PreToolUse");
/// assert_eq!(kebab_to_pascal("session-start"), "SessionStart");
/// ```
pub fn kebab_to_pascal(s: &str) -> String {
    s.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

/// Convert a PascalCase string to kebab-case.
///
/// # Examples
///
/// ```
/// use lorum::adapters::pascal_to_kebab;
/// assert_eq!(pascal_to_kebab("PreToolUse"), "pre-tool-use");
/// assert_eq!(pascal_to_kebab("SessionStart"), "session-start");
/// ```
pub fn pascal_to_kebab(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('-');
        }
        result.extend(c.to_lowercase());
    }
    result
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
