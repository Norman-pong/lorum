//! Configuration schema, loading, merging, and resolution for lorum.
//!
//! This module defines the unified configuration structs, handles reading
//! global (`~/.config/lorum/config.yaml`) and project-level (`.lorum/config.yaml`)
//! configuration files, merging them, and resolving the effective configuration
//! based on command-line arguments.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::LorumError;

/// Single MCP server configuration entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct McpServer {
    /// Command to start the MCP server (e.g. `"npx"` or `"python"`).
    pub command: String,
    /// Arguments passed to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables set for the server process.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// MCP server collection.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct McpConfig {
    /// Map of server name to server configuration.
    #[serde(default)]
    pub servers: BTreeMap<String, McpServer>,
}

/// Lorum unified global configuration file structure.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LorumConfig {
    /// MCP server configurations.
    #[serde(default)]
    pub mcp: McpConfig,
}

/// Project-level configuration with additional fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ProjectConfig {
    /// MCP server configurations (overrides global on name collision).
    #[serde(default)]
    pub mcp: McpConfig,
    /// Server names to exclude from the merged result.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Returns the global configuration file path: `~/.config/lorum/config.yaml`.
///
/// Uses the XDG configuration directory as reported by the `dirs` crate.
///
/// # Errors
///
/// Returns [`LorumError::Other`] if the system configuration directory
/// cannot be determined.
pub fn global_config_path() -> Result<PathBuf, LorumError> {
    let config_dir = dirs::config_dir().ok_or_else(|| LorumError::Other {
        message: "cannot determine system config directory".into(),
    })?;
    Ok(config_dir.join("lorum").join("config.yaml"))
}

/// Reads a [`LorumConfig`] from the file at `path`.
///
/// The file must be valid YAML.
///
/// # Errors
///
/// - [`LorumError::ConfigNotFound`] if the file does not exist.
/// - [`LorumError::ConfigParse`] if the file cannot be parsed as YAML.
/// - [`LorumError::Io`] for other I/O failures.
pub fn load_config(path: &Path) -> Result<LorumConfig, LorumError> {
    if !path.exists() {
        return Err(LorumError::ConfigNotFound {
            path: path.to_path_buf(),
        });
    }
    let contents = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&contents).map_err(|e| LorumError::ConfigParse {
        format: "yaml".into(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

/// Writes a [`LorumConfig`] to the file at `path`.
///
/// Parent directories are created automatically if they do not exist.
///
/// # Errors
///
/// - [`LorumError::ConfigWrite`] if the file cannot be written.
pub fn save_config(path: &Path, config: &LorumConfig) -> Result<(), LorumError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    let yaml = serde_yaml::to_string(config).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    std::fs::write(path, yaml).map_err(|e| LorumError::ConfigWrite {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Recursively searches for `.lorum/config.yaml` starting from `start_dir`.
///
/// Traverses upward through parent directories until the file is found or
/// the filesystem root is reached (similar to how Git discovers `.git`).
///
/// Returns `Some(path)` when a `.lorum/config.yaml` is found, or `None` if
/// no project configuration exists above `start_dir`.
pub fn find_project_config(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir;
    loop {
        let candidate = dir.join(".lorum").join("config.yaml");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

/// Reads a [`ProjectConfig`] from the file at `path`.
///
/// The file must be valid YAML.
///
/// # Errors
///
/// - [`LorumError::ConfigNotFound`] if the file does not exist.
/// - [`LorumError::ConfigParse`] if the file cannot be parsed as YAML.
/// - [`LorumError::Io`] for other I/O failures.
pub fn load_project_config(path: &Path) -> Result<ProjectConfig, LorumError> {
    if !path.exists() {
        return Err(LorumError::ConfigNotFound {
            path: path.to_path_buf(),
        });
    }
    let contents = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&contents).map_err(|e| LorumError::ConfigParse {
        format: "yaml".into(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

/// Merges global and project-level configurations.
///
/// # Merge rules
///
/// 1. Project-level servers with the same name **override** global servers
///    (no deep merge -- the project entry replaces the global one entirely).
/// 2. Servers listed in the project's `exclude` field are removed from the
///    merged result.
pub fn merge_configs(global: &LorumConfig, project: Option<&ProjectConfig>) -> LorumConfig {
    let Some(project) = project else {
        return global.clone();
    };

    let mut servers = global.mcp.servers.clone();

    // Project-level servers override global ones (full replacement).
    for (name, server) in &project.mcp.servers {
        servers.insert(name.clone(), server.clone());
    }

    // Remove excluded servers.
    for name in &project.exclude {
        servers.remove(name);
    }

    LorumConfig {
        mcp: McpConfig { servers },
    }
}

/// Resolves the effective configuration based on command-line arguments.
///
/// - If `config_path` is provided, only that file is loaded (no global or
///   project-level lookup is performed).
/// - Otherwise the global config is loaded, any project config is discovered
///   from `search_start`, and the two are merged via [`merge_configs`].
///
/// # Errors
///
/// Propagates any error from [`load_config`], [`load_project_config`], or
/// I/O operations.
pub fn resolve_effective_config(
    config_path: Option<&Path>,
    search_start: &Path,
) -> Result<LorumConfig, LorumError> {
    if let Some(path) = config_path {
        return load_config(path);
    }

    let global = match load_config(&global_config_path()?) {
        Ok(cfg) => cfg,
        Err(LorumError::ConfigNotFound { .. }) => LorumConfig::default(),
        Err(e) => return Err(e),
    };

    let project = find_project_config(search_start);

    let project_config = project
        .as_ref()
        .map(|p| load_project_config(p))
        .transpose()?;

    Ok(merge_configs(&global, project_config.as_ref()))
}

/// Resolves the effective configuration starting from the current working directory.
///
/// Convenience wrapper around [`resolve_effective_config`] that uses
/// `std::env::current_dir()` as the search start point.
///
/// # Errors
///
/// Propagates any error from [`resolve_effective_config`] or if the current
/// working directory cannot be determined.
pub fn resolve_effective_config_from_cwd(
    config_path: Option<&Path>,
) -> Result<LorumConfig, LorumError> {
    let cwd = std::env::current_dir().map_err(|e| LorumError::Io { source: e })?;
    resolve_effective_config(config_path, &cwd)
}

#[cfg(test)]
mod tests;
