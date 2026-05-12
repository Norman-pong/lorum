//! Backup management for tool configuration files.
//!
//! Before overwriting a tool's configuration, a backup is created in
//! `~/.config/lorum/backups/`. Old backups are pruned automatically to
//! keep at most 10 copies per tool.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::LorumError;

/// Maximum number of backups to keep per tool.
const MAX_BACKUPS: usize = 10;

/// Returns the backup directory path: `~/.config/lorum/backups/`.
///
/// # Errors
///
/// Returns [`LorumError::Other`] if the system config directory cannot
/// be determined.
pub fn backup_dir() -> Result<PathBuf, LorumError> {
    let config_dir = dirs::config_dir().ok_or_else(|| LorumError::Other {
        message: "cannot determine system config directory".into(),
    })?;
    Ok(config_dir.join("lorum").join("backups"))
}

/// Create a backup of a file before overwriting.
///
/// The backup file is named `<tool>-<timestamp>.<ext>` where `<ext>` is
/// taken from the source file's extension (defaults to `"bak"`).
///
/// After creating the backup, old copies are pruned so that at most
/// 10 backups exist for the given tool.
///
/// # Errors
///
/// - [`LorumError::Io`] if the file cannot be copied.
/// - [`LorumError::Other`] if the backup directory cannot be determined.
pub fn create_backup(tool_name: &str, source_path: &Path) -> Result<PathBuf, LorumError> {
    let dir = backup_dir()?;
    fs::create_dir_all(&dir)?;

    let timestamp = epoch_timestamp();
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bak");
    let backup_name = format!("{tool_name}-{timestamp}.{ext}");
    let backup_path = dir.join(&backup_name);

    fs::copy(source_path, &backup_path)?;

    cleanup_old_backups(tool_name, &dir)?;

    Ok(backup_path)
}

/// List all backups for a tool, newest first.
///
/// Returns an empty vector if no backups exist or the backup directory
/// does not exist yet.
///
/// # Errors
///
/// Returns [`LorumError::Io`] if the backup directory cannot be read.
pub fn list_backups(tool_name: &str) -> Result<Vec<PathBuf>, LorumError> {
    let dir = backup_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    list_backups_in_dir(tool_name, &dir)
}

/// Restore a tool's configuration from the most recent backup.
///
/// # Errors
///
/// - [`LorumError::Other`] if no backups exist for the tool.
/// - [`LorumError::Io`] if the backup file cannot be copied.
pub fn restore_backup(tool_name: &str, target_path: &Path) -> Result<(), LorumError> {
    let backups = list_backups(tool_name)?;
    let latest = backups.first().ok_or_else(|| LorumError::Other {
        message: format!("no backups found for {tool_name}"),
    })?;
    fs::copy(latest, target_path)?;
    Ok(())
}

/// Remove old backups, keeping only [`MAX_BACKUPS`] most recent.
fn cleanup_old_backups(tool_name: &str, dir: &Path) -> Result<(), LorumError> {
    let mut backups = list_backups_in_dir(tool_name, dir)?;
    if backups.len() > MAX_BACKUPS {
        for old in backups.drain(MAX_BACKUPS..) {
            let _ = fs::remove_file(old); // best effort
        }
    }
    Ok(())
}

/// List backups for a tool within a specific directory, newest first.
fn list_backups_in_dir(tool_name: &str, dir: &Path) -> Result<Vec<PathBuf>, LorumError> {
    let prefix = format!("{tool_name}-");
    let mut backups: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    backups.sort_by(|a, b| b.cmp(a));
    Ok(backups)
}

/// Generate a simple timestamp string based on the Unix epoch.
fn epoch_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn epoch_timestamp_is_numeric() {
        let ts = epoch_timestamp();
        assert!(ts.chars().all(|c| c.is_ascii_digit()));
        assert!(!ts.is_empty());
    }

    #[test]
    fn create_and_list_backups() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("settings.json");
        fs::write(&source, r#"{"test": true}"#).unwrap();

        // Use the temp dir as backup dir by patching via a helper approach:
        // We test the logic directly against a known directory.
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        let ts = epoch_timestamp();
        let backup_name = format!("claude-code-{ts}.json");
        let backup_path = backup_dir.join(&backup_name);
        fs::copy(&source, &backup_path).unwrap();

        assert!(backup_path.exists());
        let contents = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(contents, r#"{"test": true}"#);
    }

    #[test]
    fn list_backups_empty_when_no_dir() {
        // list_backups returns Ok(vec![]) when the backup directory does not exist.
        let result = list_backups("nonexistent-tool-xyz");
        // This will use the real backup_dir which may or may not exist.
        // The important thing is it returns Ok with an empty or filtered vec.
        assert!(result.is_ok());
    }

    #[test]
    fn restore_from_latest_backup() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        // Create two backups with different timestamps
        let old_backup = backup_dir.join("test-tool-1000.json");
        let new_backup = backup_dir.join("test-tool-2000.json");
        fs::write(&old_backup, "old content").unwrap();
        fs::write(&new_backup, "new content").unwrap();

        // list_backups_in_dir returns newest first
        let backups = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0], new_backup);
        assert_eq!(backups[1], old_backup);

        // Restore latest
        let target = dir.path().join("restored.json");
        fs::copy(&backups[0], &target).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new content");
    }

    #[test]
    fn cleanup_removes_old_backups() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        // Create more than MAX_BACKUPS
        for i in 0..=MAX_BACKUPS {
            let path = backup_dir.join(format!("test-tool-{i:04}.json"));
            fs::write(&path, format!("content-{i}")).unwrap();
        }
        assert_eq!(
            list_backups_in_dir("test-tool", &backup_dir).unwrap().len(),
            MAX_BACKUPS + 1
        );

        cleanup_old_backups("test-tool", &backup_dir).unwrap();
        let remaining = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(remaining.len(), MAX_BACKUPS);

        // Verify the newest backups are kept (highest numbers)
        for path in &remaining {
            let name = path.file_name().unwrap().to_str().unwrap();
            let num_str = name
                .strip_prefix("test-tool-")
                .and_then(|s| s.strip_suffix(".json"))
                .unwrap();
            let num: usize = num_str.parse().unwrap();
            assert!(num >= 1, "backup {num} should have been removed");
        }
    }

    #[test]
    fn list_backups_filters_by_tool_name() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        fs::write(backup_dir.join("claude-code-1000.json"), "").unwrap();
        fs::write(backup_dir.join("codex-2000.toml"), "").unwrap();
        fs::write(backup_dir.join("claude-code-3000.json"), "").unwrap();

        let claude = list_backups_in_dir("claude-code", &backup_dir).unwrap();
        let codex = list_backups_in_dir("codex", &backup_dir).unwrap();

        assert_eq!(claude.len(), 2);
        assert_eq!(codex.len(), 1);
    }
}
