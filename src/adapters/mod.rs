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

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

/// Severity level of a configuration validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A critical issue that prevents the configuration from being usable.
    Error,
    /// A non-critical issue that should be addressed but does not block usage.
    Warning,
}

/// A single issue discovered during configuration validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationIssue {
    /// Severity of the issue.
    pub severity: Severity,
    /// Human-readable description of the issue.
    pub message: String,
    /// Path to the file where the issue was found, if applicable.
    pub path: Option<PathBuf>,
    /// Line number where the issue was found, if applicable.
    pub line: Option<usize>,
}

/// Trait for validating tool configuration files.
///
/// Implementors can check configuration files for syntax errors,
/// structural problems, or other issues.
pub trait ConfigValidator: Send + Sync {
    /// Human-readable name of the tool (e.g. "claude-code").
    fn name(&self) -> &str;

    /// Validate all configuration files for this tool.
    ///
    /// Returns a list of issues found. An empty vector means no issues.
    fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError>;
}

/// Validate syntax for a single configuration file based on its extension.
///
/// Checks that the file exists, is not a directory, is not empty, and that
/// its contents are valid JSON, TOML, or YAML depending on the extension.
/// Returns a list of issues found (empty if no issues).
pub fn validate_syntax(path: &Path) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if !path.exists() {
        return issues;
    }

    // Check if path is a directory
    if path.is_dir() {
        issues.push(ValidationIssue {
            severity: Severity::Error,
            message: "expected file, found directory".into(),
            path: Some(path.to_path_buf()),
            line: None,
        });
        return issues;
    }

    // Get metadata to check file size
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("failed to read metadata: {e}"),
                path: Some(path.to_path_buf()),
                line: None,
            });
            return issues;
        }
    };

    if metadata.len() == 0 {
        issues.push(ValidationIssue {
            severity: Severity::Warning,
            message: "file is empty".into(),
            path: Some(path.to_path_buf()),
            line: None,
        });
        return issues;
    }

    // Read file content
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("failed to read file: {e}"),
                path: Some(path.to_path_buf()),
                line: None,
            });
            return issues;
        }
    };

    // Validate based on extension
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "json" => {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("invalid JSON: {e}"),
                    path: Some(path.to_path_buf()),
                    line: None,
                });
            }
        }
        "toml" => {
            if let Err(e) = toml::from_str::<toml::Value>(&content) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("invalid TOML: {e}"),
                    path: Some(path.to_path_buf()),
                    line: None,
                });
            }
        }
        "yaml" | "yml" => {
            if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("invalid YAML: {e}"),
                    path: Some(path.to_path_buf()),
                    line: None,
                });
            }
        }
        _ => {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!("unknown file extension '{ext}', skipping syntax validation"),
                path: Some(path.to_path_buf()),
                line: None,
            });
        }
    }

    issues
}

/// Validate syntax for all configuration files returned by `config_paths`.
///
/// This is a convenience wrapper around [`validate_syntax`] that iterates
/// over all paths and aggregates issues.
pub fn validate_all_syntax(config_paths: &[PathBuf]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    for path in config_paths {
        issues.extend(validate_syntax(path));
    }
    issues
}

/// Default implementation helper for `ConfigValidator`.
///
/// Adapters that do not need custom validation logic can call this
/// function from their `validate_config` implementation to perform
/// standard syntax validation on all config files.
pub fn default_validate_config(
    adapter: &dyn ToolAdapter,
) -> Result<Vec<ValidationIssue>, LorumError> {
    let paths: Vec<PathBuf> = adapter.config_paths();
    Ok(validate_all_syntax(&paths))
}

pub mod claude;
pub mod codex;
pub mod continue_dev;
pub mod cursor;
pub mod json_utils;
pub mod kimi;
pub mod opencode;
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
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] or [`LorumError::ConfigParse`] if the
    /// configuration file exists but cannot be read or parsed.
    fn read_mcp(&self) -> Result<McpConfig, LorumError>;

    /// Write MCP servers to the tool's configuration.
    ///
    /// Must preserve non-MCP fields in the existing config file.
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] or [`LorumError::ConfigWrite`] if the
    /// configuration file cannot be written.
    fn write_mcp(&self, config: &McpConfig) -> Result<(), LorumError>;
}

static ALL_ADAPTERS: LazyLock<Vec<Box<dyn ToolAdapter>>> = LazyLock::new(|| {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(continue_dev::ContinueDevAdapter::new()),
        Box::new(cursor::CursorAdapter::new()),
        Box::new(proma::PromaAdapter),
        Box::new(kimi::KimiAdapter),
        Box::new(opencode::OpencodeAdapter::new()),
        Box::new(trae::TraeAdapter::new()),
        Box::new(windsurf::WindsurfAdapter),
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
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] if the file exists but cannot be read.
    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError>;

    /// Write rules content to the tool's file.
    ///
    /// Creates parent directories if needed.
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] or [`LorumError::ConfigWrite`] if the file
    /// cannot be written.
    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError>;
}

static ALL_RULES_ADAPTERS: LazyLock<Vec<Box<dyn RulesAdapter>>> = LazyLock::new(|| {
    vec![
        Box::new(claude::ClaudeRulesAdapter),
        Box::new(cursor::CursorRulesAdapter),
        Box::new(windsurf::WindsurfRulesAdapter),
        Box::new(codex::CodexRulesAdapter),
        Box::new(kimi::KimiRulesAdapter),
        Box::new(opencode::OpenCodeRulesAdapter),
        Box::new(trae::TraeRulesAdapter),
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
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
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
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] or [`LorumError::ConfigParse`] if the
    /// configuration file exists but cannot be read or parsed.
    fn read_hooks(&self) -> Result<HooksConfig, LorumError>;

    /// Write hooks to the tool's configuration.
    ///
    /// Must preserve non-hooks fields in the existing config file.
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] or [`LorumError::ConfigWrite`] if the
    /// configuration file cannot be written.
    fn write_hooks(&self, config: &HooksConfig) -> Result<(), LorumError>;

    /// Convert a lorum unified event name (kebab-case) to this tool's
    /// specific event name format.
    ///
    /// Returns `None` when the tool does not support the given event.
    fn lorum_to_tool_event(&self, lorum_event: &str) -> Option<String>;

    /// Convert this tool's event name format to a lorum unified event name
    /// (kebab-case).
    ///
    /// Returns `None` when the event name is not recognised by lorum.
    fn tool_to_lorum_event(&self, tool_event: &str) -> Option<String>;
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
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] if the skills directory cannot be read.
    fn read_skills(&self) -> Result<Vec<SkillEntry>, LorumError>;

    /// Write a single skill (full directory copy) to the tool's skills dir.
    ///
    /// If a skill with the same name already exists, it is removed without
    /// backup before the new content is copied.
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] if the source directory cannot be read or
    /// the destination cannot be written.
    fn write_skill(&self, name: &str, source_dir: &Path) -> Result<(), LorumError>;

    /// Remove a skill directory from the tool's skills dir.
    ///
    /// # Errors
    ///
    /// Returns [`LorumError::Io`] if the skill directory cannot be removed.
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

/// All registered config validators, derived from the MCP adapters.
///
/// Each [`ToolAdapter`] also implements [`ConfigValidator`] via the blanket
/// implementation, so this list contains a validator for every MCP adapter.
static ALL_CONFIG_VALIDATORS: LazyLock<Vec<Box<dyn ConfigValidator>>> = LazyLock::new(|| {
    vec![
        Box::new(claude::ClaudeAdapter) as Box<dyn ConfigValidator>,
        Box::new(codex::CodexAdapter) as Box<dyn ConfigValidator>,
        Box::new(continue_dev::ContinueDevAdapter::new()) as Box<dyn ConfigValidator>,
        Box::new(cursor::CursorAdapter::new()) as Box<dyn ConfigValidator>,
        Box::new(proma::PromaAdapter) as Box<dyn ConfigValidator>,
        Box::new(kimi::KimiAdapter) as Box<dyn ConfigValidator>,
        Box::new(opencode::OpencodeAdapter::new()) as Box<dyn ConfigValidator>,
        Box::new(trae::TraeAdapter::new()) as Box<dyn ConfigValidator>,
        Box::new(windsurf::WindsurfAdapter) as Box<dyn ConfigValidator>,
    ]
});

/// Return all registered config validators.
pub fn all_config_validators() -> &'static [Box<dyn ConfigValidator>] {
    &ALL_CONFIG_VALIDATORS
}

/// Find a config validator by name.
pub fn find_config_validator(name: &str) -> Option<&'static dyn ConfigValidator> {
    ALL_CONFIG_VALIDATORS
        .iter()
        .find(|v| v.name() == name)
        .map(|v| v.as_ref())
}

/// Return the union of all tool names registered across all four adapter dimensions.
///
/// Each tool name appears at most once in the returned vector.
///
/// **Note:** Names are currently returned in lexicographic order because a
/// `BTreeSet` is used for deduplication. If insertion order is required,
/// switch to `IndexSet` (requires the `indexmap` crate).
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
    let mut result = String::with_capacity(s.len());
    let mut upper_next = true;
    for c in s.chars() {
        if c == '-' {
            upper_next = true;
        } else if upper_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            upper_next = false;
        } else {
            for lc in c.to_lowercase() {
                result.push(lc);
            }
        }
    }
    result
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
    let mut result = String::with_capacity(s.len() * 2);
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
        assert_eq!(adapters.len(), 9);
        let names: Vec<_> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"continue"));
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"proma"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"opencode"));
        assert!(names.contains(&"trae"));
        assert!(names.contains(&"windsurf"));
    }

    #[test]
    fn find_adapter_finds_known() {
        assert_eq!(find_adapter("claude-code").unwrap().name(), "claude-code");
        assert_eq!(find_adapter("codex").unwrap().name(), "codex");
        assert_eq!(find_adapter("continue").unwrap().name(), "continue");
        assert_eq!(find_adapter("cursor").unwrap().name(), "cursor");
        assert_eq!(find_adapter("proma").unwrap().name(), "proma");
        assert_eq!(find_adapter("kimi").unwrap().name(), "kimi");
        assert_eq!(find_adapter("opencode").unwrap().name(), "opencode");
        assert_eq!(find_adapter("trae").unwrap().name(), "trae");
        assert_eq!(find_adapter("windsurf").unwrap().name(), "windsurf");
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
    fn all_rules_adapters_returns_expected_count() {
        let adapters = all_rules_adapters();
        assert_eq!(adapters.len(), 7);
        let names: Vec<_> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"windsurf"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"opencode"));
        assert!(names.contains(&"trae"));
    }

    #[test]
    fn find_rules_adapter_finds_known() {
        assert_eq!(
            find_rules_adapter("claude-code").unwrap().name(),
            "claude-code"
        );
        assert_eq!(find_rules_adapter("cursor").unwrap().name(), "cursor");
        assert_eq!(find_rules_adapter("windsurf").unwrap().name(), "windsurf");
        assert_eq!(find_rules_adapter("codex").unwrap().name(), "codex");
        assert_eq!(find_rules_adapter("kimi").unwrap().name(), "kimi");
        assert_eq!(find_rules_adapter("opencode").unwrap().name(), "opencode");
        assert_eq!(find_rules_adapter("trae").unwrap().name(), "trae");
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
    fn test_hooks_event_mapping_claude() {
        let adapter = claude::ClaudeAdapter;
        // kebab-case -> PascalCase
        assert_eq!(
            adapter.lorum_to_tool_event("pre-tool-use"),
            Some("PreToolUse".to_string())
        );
        assert_eq!(
            adapter.lorum_to_tool_event("session-start"),
            Some("SessionStart".to_string())
        );
        // PascalCase -> kebab-case
        assert_eq!(
            adapter.tool_to_lorum_event("PreToolUse"),
            Some("pre-tool-use".to_string())
        );
        assert_eq!(
            adapter.tool_to_lorum_event("SessionStart"),
            Some("session-start".to_string())
        );
    }

    #[test]
    fn test_hooks_event_mapping_kimi() {
        let adapter = kimi::KimiAdapter;
        // kebab-case -> PascalCase
        assert_eq!(
            adapter.lorum_to_tool_event("post-tool-use"),
            Some("PostToolUse".to_string())
        );
        // PascalCase -> kebab-case
        assert_eq!(
            adapter.tool_to_lorum_event("PostToolUse"),
            Some("post-tool-use".to_string())
        );
    }

    #[test]
    fn test_hooks_event_mapping_roundtrip() {
        let claude = claude::ClaudeAdapter;
        let events = [
            "pre-tool-use",
            "post-tool-use",
            "session-start",
            "session-end",
        ];
        for event in &events {
            let tool = claude.lorum_to_tool_event(event).unwrap();
            let back = claude.tool_to_lorum_event(&tool).unwrap();
            assert_eq!(back, *event, "roundtrip failed for {event}");
        }
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

    // ---- ConfigValidator blanket impl via validate_syntax ------------------

    #[test]
    fn test_config_validator_blanket_impl_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"key": "value"}"#).unwrap();
        let issues = validate_syntax(&path);
        assert!(
            issues.is_empty(),
            "expected no issues for valid JSON, got: {:?}",
            issues
        );
    }

    #[test]
    fn test_config_validator_blanket_impl_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "key = \"value\"\n").unwrap();
        let issues = validate_syntax(&path);
        assert!(
            issues.is_empty(),
            "expected no issues for valid TOML, got: {:?}",
            issues
        );
    }

    #[test]
    fn test_config_validator_blanket_impl_valid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "key: value\n").unwrap();
        let issues = validate_syntax(&path);
        assert!(
            issues.is_empty(),
            "expected no issues for valid YAML, got: {:?}",
            issues
        );
    }

    #[test]
    fn test_config_validator_blanket_impl_broken_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"key": "value" "missing": "comma"}"#).unwrap();
        let issues = validate_syntax(&path);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert!(issues[0].message.contains("invalid JSON"));
    }

    #[test]
    fn test_config_validator_blanket_impl_broken_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "key = \n").unwrap();
        let issues = validate_syntax(&path);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert!(issues[0].message.contains("invalid TOML"));
    }

    #[test]
    fn test_config_validator_blanket_impl_broken_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "key: [unclosed").unwrap();
        let issues = validate_syntax(&path);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert!(issues[0].message.contains("invalid YAML"));
    }

    #[test]
    fn test_config_validator_blanket_impl_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "").unwrap();
        let issues = validate_syntax(&path);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert!(issues[0].message.contains("empty"));
    }

    #[test]
    fn test_config_validator_blanket_impl_directory_not_file() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("config.json");
        fs::create_dir(&subdir).unwrap();
        let issues = validate_syntax(&subdir);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert!(issues[0].message.contains("directory"));
    }

    #[test]
    fn test_config_validator_blanket_impl_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.txt");
        fs::write(&path, "some text").unwrap();
        let issues = validate_syntax(&path);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert!(issues[0].message.contains("unknown file extension"));
    }

    #[test]
    fn test_all_config_validators_returns_expected_count() {
        let validators = all_config_validators();
        assert_eq!(validators.len(), 9);
    }

    #[test]
    fn test_find_config_validator_finds_known() {
        assert_eq!(
            find_config_validator("claude-code").unwrap().name(),
            "claude-code"
        );
        assert_eq!(find_config_validator("codex").unwrap().name(), "codex");
        assert_eq!(
            find_config_validator("continue").unwrap().name(),
            "continue"
        );
        assert_eq!(find_config_validator("cursor").unwrap().name(), "cursor");
        assert_eq!(find_config_validator("proma").unwrap().name(), "proma");
        assert_eq!(find_config_validator("kimi").unwrap().name(), "kimi");
        assert_eq!(
            find_config_validator("opencode").unwrap().name(),
            "opencode"
        );
        assert_eq!(find_config_validator("trae").unwrap().name(), "trae");
        assert_eq!(
            find_config_validator("windsurf").unwrap().name(),
            "windsurf"
        );
    }

    #[test]
    fn test_find_config_validator_returns_none_for_unknown() {
        assert!(find_config_validator("nonexistent-tool").is_none());
    }

    #[test]
    fn test_all_adapters_and_validators_are_consistent() {
        // Ensure that every adapter has a corresponding validator and vice versa.
        let adapters = all_adapters();
        let validators = all_config_validators();
        assert_eq!(
            adapters.len(),
            validators.len(),
            "ALL_ADAPTERS and ALL_CONFIG_VALIDATORS must have the same length"
        );
        for adapter in adapters {
            assert!(
                find_config_validator(adapter.name()).is_some(),
                "adapter '{}' has no corresponding config validator",
                adapter.name()
            );
        }
    }
}
