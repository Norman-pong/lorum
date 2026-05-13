//! Tool adapter framework for reading/writing MCP configurations.
//!
//! Each AI coding tool has its own configuration file format and location.
//! The [`ToolAdapter`](crate::adapters::ToolAdapter) trait provides a uniform interface for reading and
//! writing MCP server configurations across these tools.
//!
//! The [`RulesAdapter`](crate::adapters::RulesAdapter) trait provides a uniform interface for reading and
//! writing rules files across tools that support them.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

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
pub trait ToolAdapter: Send + Sync {
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

static ALL_ADAPTERS: LazyLock<Vec<Box<dyn ToolAdapter>>> = LazyLock::new(|| {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(proma::PromaAdapter),
        Box::new(kimi::KimiAdapter),
        Box::new(trae::TraeAdapter::new()),
    ]
});

/// Return all registered adapters.
pub fn all_adapters() -> &'static [Box<dyn ToolAdapter>] {
    &ALL_ADAPTERS
}

/// Find an adapter by name.
pub fn find_adapter(name: &str) -> Option<&'static dyn ToolAdapter> {
    ALL_ADAPTERS
        .iter()
        .find(|a| a.name() == name)
        .map(|a| a.as_ref())
}

/// Per-tool adapter for reading/writing rules files.
///
/// Implementors define how to interact with a specific AI coding tool's
/// rules file, including its location on disk.
pub trait RulesAdapter: Send + Sync {
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

static ALL_RULES_ADAPTERS: LazyLock<Vec<Box<dyn RulesAdapter>>> = LazyLock::new(|| {
    vec![
        Box::new(cursor::CursorRulesAdapter),
        Box::new(windsurf::WindsurfRulesAdapter),
        Box::new(codex::CodexRulesAdapter),
    ]
});

/// Return all registered rules adapters.
pub fn all_rules_adapters() -> &'static [Box<dyn RulesAdapter>] {
    &ALL_RULES_ADAPTERS
}

/// Find a rules adapter by name.
pub fn find_rules_adapter(name: &str) -> Option<&'static dyn RulesAdapter> {
    ALL_RULES_ADAPTERS
        .iter()
        .find(|a| a.name() == name)
        .map(|a| a.as_ref())
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
pub trait HooksAdapter: Send + Sync {
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

static ALL_HOOKS_ADAPTERS: LazyLock<Vec<Box<dyn HooksAdapter>>> =
    LazyLock::new(|| vec![Box::new(claude::ClaudeAdapter), Box::new(kimi::KimiAdapter)]);

/// Return all registered hooks adapters.
pub fn all_hooks_adapters() -> &'static [Box<dyn HooksAdapter>] {
    &ALL_HOOKS_ADAPTERS
}

/// Find a hooks adapter by name.
pub fn find_hooks_adapter(name: &str) -> Option<&'static dyn HooksAdapter> {
    ALL_HOOKS_ADAPTERS
        .iter()
        .find(|a| a.name() == name)
        .map(|a| a.as_ref())
}

/// Per-tool adapter for reading/writing skills directories.
///
/// Skills are directory-based entities (each skill is a folder containing
/// SKILL.md and optional auxiliary files). Adapters define where each tool
/// stores its skills.
pub trait SkillsAdapter: Send + Sync {
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

static ALL_SKILLS_ADAPTERS: LazyLock<Vec<Box<dyn SkillsAdapter>>> = LazyLock::new(|| {
    vec![
        Box::new(claude::ClaudeSkillsAdapter),
        Box::new(proma::PromaSkillsAdapter),
    ]
});

/// Return all registered skills adapters.
pub fn all_skills_adapters() -> &'static [Box<dyn SkillsAdapter>] {
    &ALL_SKILLS_ADAPTERS
}

/// Find a skills adapter by name.
pub fn find_skills_adapter(name: &str) -> Option<&'static dyn SkillsAdapter> {
    ALL_SKILLS_ADAPTERS
        .iter()
        .find(|a| a.name() == name)
        .map(|a| a.as_ref())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn all_adapters_returns_known_adapters() {
        let adapters = all_adapters();
        assert!(!adapters.is_empty());
        let names: Vec<_> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"proma"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"trae"));
    }

    #[test]
    fn find_adapter_finds_known() {
        assert_eq!(find_adapter("claude-code").unwrap().name(), "claude-code");
        assert_eq!(find_adapter("codex").unwrap().name(), "codex");
        assert_eq!(find_adapter("proma").unwrap().name(), "proma");
        assert_eq!(find_adapter("kimi").unwrap().name(), "kimi");
        assert_eq!(find_adapter("trae").unwrap().name(), "trae");
    }

    #[test]
    fn find_adapter_returns_none_for_unknown() {
        assert!(find_adapter("nonexistent-tool").is_none());
    }

    #[test]
    fn find_adapter_returns_static_ref() {
        let a = find_adapter("claude-code");
        let b = find_adapter("claude-code");
        assert!(a.is_some());
        // Both should point to the same cached instance.
        assert_eq!(a.unwrap().name(), b.unwrap().name());
    }

    #[test]
    fn all_rules_adapters_returns_three() {
        let adapters = all_rules_adapters();
        assert_eq!(adapters.len(), 3);
        let names: Vec<_> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"windsurf"));
        assert!(names.contains(&"codex"));
    }

    #[test]
    fn find_rules_adapter_finds_known() {
        assert_eq!(find_rules_adapter("cursor").unwrap().name(), "cursor");
        assert_eq!(find_rules_adapter("windsurf").unwrap().name(), "windsurf");
        assert_eq!(find_rules_adapter("codex").unwrap().name(), "codex");
    }

    #[test]
    fn find_rules_adapter_returns_none_for_unknown() {
        assert!(find_rules_adapter("nonexistent").is_none());
    }

    #[test]
    fn all_hooks_adapters_returns_two() {
        let adapters = all_hooks_adapters();
        assert_eq!(adapters.len(), 2);
        let names: Vec<_> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"kimi"));
    }

    #[test]
    fn find_hooks_adapter_finds_known() {
        assert_eq!(
            find_hooks_adapter("claude-code").unwrap().name(),
            "claude-code"
        );
        assert_eq!(find_hooks_adapter("kimi").unwrap().name(), "kimi");
    }

    #[test]
    fn find_hooks_adapter_returns_none_for_unknown() {
        assert!(find_hooks_adapter("nonexistent").is_none());
    }

    #[test]
    fn all_skills_adapters_returns_two() {
        let adapters = all_skills_adapters();
        assert_eq!(adapters.len(), 2);
        let names: Vec<_> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"proma"));
    }

    #[test]
    fn find_skills_adapter_finds_known() {
        assert_eq!(
            find_skills_adapter("claude-code").unwrap().name(),
            "claude-code"
        );
        assert_eq!(find_skills_adapter("proma").unwrap().name(), "proma");
    }

    #[test]
    fn find_skills_adapter_returns_none_for_unknown() {
        assert!(find_skills_adapter("nonexistent").is_none());
    }

    #[test]
    fn read_rules_file_reads_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.md");
        fs::write(&path, "# Rules\n").unwrap();
        let result = read_rules_file(&path).unwrap();
        assert_eq!(result, Some("# Rules\n".to_string()));
    }

    #[test]
    fn read_rules_file_returns_none_for_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.md");
        let result = read_rules_file(&path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn write_rules_file_creates_file_and_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("rules.md");
        assert!(!path.exists());
        write_rules_file(&path, "# New Rules\n").unwrap();
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "# New Rules\n");
    }
}
