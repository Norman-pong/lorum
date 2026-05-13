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

/// Information about a single backup file.
#[derive(Debug, Clone)]
pub struct BackupInfo {
    /// Full path to the backup file.
    pub path: PathBuf,
    /// File name of the backup.
    pub name: String,
    /// Raw timestamp string extracted from the filename.
    pub timestamp: String,
    /// Human-readable time display.
    pub time_display: String,
    /// File size in bytes.
    pub size: u64,
}

/// Returns the backup directory path: `~/.config/lorum/backups/`.
///
/// Uses `$XDG_CONFIG_HOME/lorum/backups` if that environment variable is set.
/// Otherwise falls back to `$HOME/.config/lorum/backups` on all platforms.
///
/// # Errors
///
/// Returns [`LorumError::Other`] if the config directory cannot be determined.
pub fn backup_dir() -> Result<PathBuf, LorumError> {
    let config_dir = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = dirs::home_dir().ok_or_else(|| LorumError::Other {
            message: "cannot determine home directory".into(),
        })?;
        home.join(".config")
    };
    Ok(config_dir.join("lorum").join("backups"))
}

/// Create a backup of a file before overwriting.
///
/// The backup file is named `<tool>-<timestamp>.<ext>` where `<timestamp>`
/// uses the `YYYYMMDD-HHMMSS` format and `<ext>` is taken from the source
/// file's extension (defaults to `"bak"`).
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

    let timestamp = timestamp();
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
pub fn list_backups(tool_name: &str) -> Result<Vec<BackupInfo>, LorumError> {
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
    fs::copy(&latest.path, target_path)?;
    Ok(())
}

/// Restore a tool's configuration from a specific backup file.
///
/// # Errors
///
/// - [`LorumError::Io`] if the backup file cannot be copied.
pub fn restore_backup_from_path(backup_path: &Path, target_path: &Path) -> Result<(), LorumError> {
    fs::copy(backup_path, target_path)?;
    Ok(())
}

/// Remove old backups, keeping only [`MAX_BACKUPS`] most recent.
fn cleanup_old_backups(tool_name: &str, dir: &Path) -> Result<(), LorumError> {
    let mut backups = list_backups_in_dir(tool_name, dir)?;
    if backups.len() > MAX_BACKUPS {
        for old in backups.drain(MAX_BACKUPS..) {
            let _ = fs::remove_file(old.path); // best effort
        }
    }
    Ok(())
}

/// List backups for a tool within a specific directory, newest first.
fn list_backups_in_dir(tool_name: &str, dir: &Path) -> Result<Vec<BackupInfo>, LorumError> {
    let prefix = format!("{tool_name}-");
    let mut backups: Vec<BackupInfo> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?.to_string();
            if !name.starts_with(&prefix) {
                return None;
            }

            let metadata = fs::metadata(&path).ok()?;
            let size = metadata.len();

            // Extract timestamp: {tool}-{timestamp}.{ext}
            let rest = name.strip_prefix(&prefix)?;
            let timestamp = rest.rsplit_once('.')?.0;
            let (timestamp, time_display) = parse_timestamp(timestamp);

            Some(BackupInfo {
                path,
                name,
                timestamp,
                time_display,
                size,
            })
        })
        .collect();

    // Sort by timestamp string descending (YYYYMMDD-HHMMSS sorts correctly).
    // Mixed old/new: YYYYMMDD... > epoch numbers because '2' > '1'.
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(backups)
}

/// Generate a human-readable timestamp in `YYYYMMDD-HHMMSS` format.
fn timestamp() -> String {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, hour, minute, second) = epoch_to_utc(secs);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        year, month, day, hour, minute, second
    )
}

/// Convert a Unix epoch timestamp to UTC date/time components.
fn epoch_to_utc(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    const SECS_PER_MINUTE: u64 = 60;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_DAY: u64 = 86400;

    let mut days = secs / SECS_PER_DAY;
    let rem = secs % SECS_PER_DAY;

    let hour = (rem / SECS_PER_HOUR) as u32;
    let rem = rem % SECS_PER_HOUR;

    let minute = (rem / SECS_PER_MINUTE) as u32;
    let second = (rem % SECS_PER_MINUTE) as u32;

    let mut year = 1970u32;
    loop {
        let dim = if is_leap_year(year) { 366 } else { 365 };
        if days < dim {
            break;
        }
        days -= dim;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for (i, &dim) in month_days.iter().enumerate() {
        if days < dim {
            month = (i + 1) as u32;
            break;
        }
        days -= dim;
    }

    let day = (days + 1) as u32;

    (year, month, day, hour, minute, second)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Parse a timestamp string from a backup filename.
///
/// Supports both the new `YYYYMMDD-HHMMSS` format and the legacy epoch
/// (numeric seconds) format.
fn parse_timestamp(ts: &str) -> (String, String) {
    if ts.len() == 15 && ts.as_bytes()[8] == b'-' {
        // YYYYMMDD-HHMMSS
        let formatted = format!(
            "{}-{}-{} {}:{}:{}",
            &ts[0..4],
            &ts[4..6],
            &ts[6..8],
            &ts[9..11],
            &ts[11..13],
            &ts[13..15]
        );
        (ts.to_string(), formatted)
    } else if ts.parse::<u64>().is_ok() {
        // Legacy epoch format
        (ts.to_string(), format!("epoch: {ts}"))
    } else {
        (ts.to_string(), ts.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::panic;

    #[test]
    #[serial]
    fn backup_dir_uses_xdg_config_home() {
        let tmp = tempfile::tempdir().unwrap();
        let xdg = tmp.path().join("xdg_config");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &xdg);
        }

        let result = panic::catch_unwind(|| {
            let dir = backup_dir().unwrap();
            assert_eq!(dir, xdg.join("lorum").join("backups"));
        });

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        result.unwrap();
    }

    #[test]
    #[serial]
    fn backup_dir_falls_back_to_home_dot_config() {
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let result = panic::catch_unwind(|| {
            let dir = backup_dir().unwrap();
            let home = dirs::home_dir().expect("home dir");
            assert_eq!(dir, home.join(".config").join("lorum").join("backups"));
        });

        result.unwrap();
    }

    #[test]
    #[serial]
    fn create_backup_creates_file_and_prunes() {
        let tmp = tempfile::tempdir().unwrap();
        let xdg = tmp.path().join("xdg_config");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &xdg);
        }

        let result = panic::catch_unwind(|| {
            let source = tmp.path().join("settings.json");
            fs::write(&source, r#"{"test": true}"#).unwrap();

            let backup_path = create_backup("test-tool", &source).unwrap();
            assert!(backup_path.exists());
            assert_eq!(
                fs::read_to_string(&backup_path).unwrap(),
                r#"{"test": true}"#
            );
            assert!(backup_path.to_string_lossy().contains("test-tool-"));
            assert_eq!(backup_path.extension().unwrap(), "json");
        });

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        result.unwrap();
    }

    #[test]
    #[serial]
    fn restore_backup_restores_latest() {
        let tmp = tempfile::tempdir().unwrap();
        let xdg = tmp.path().join("xdg_config");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &xdg);
        }

        let result = panic::catch_unwind(|| {
            let source = tmp.path().join("settings.json");
            fs::write(&source, "original").unwrap();

            // Create first backup
            let _ = create_backup("restore-tool", &source).unwrap();
            // Modify source
            fs::write(&source, "modified").unwrap();
            // Create second backup
            let _ = create_backup("restore-tool", &source).unwrap();

            // Restore to a new target
            let target = tmp.path().join("restored.json");
            restore_backup("restore-tool", &target).unwrap();
            assert_eq!(fs::read_to_string(&target).unwrap(), "modified");
        });

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        result.unwrap();
    }

    #[test]
    #[serial]
    fn restore_backup_from_path_works() {
        let tmp = tempfile::tempdir().unwrap();
        let xdg = tmp.path().join("xdg_config");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &xdg);
        }

        let result = panic::catch_unwind(|| {
            let source = tmp.path().join("settings.json");
            fs::write(&source, "specific backup").unwrap();

            let backup_path = create_backup("specific-tool", &source).unwrap();

            let target = tmp.path().join("restored.json");
            restore_backup_from_path(&backup_path, &target).unwrap();
            assert_eq!(fs::read_to_string(&target).unwrap(), "specific backup");
        });

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        result.unwrap();
    }

    #[test]
    fn timestamp_format_is_yyyymmdd_hhmmss() {
        let ts = timestamp();
        assert_eq!(ts.len(), 15);
        assert_eq!(ts.as_bytes()[8], b'-');
        assert!(ts.chars().all(|c| c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn epoch_to_utc_known_values() {
        // 1970-01-01 00:00:00 UTC
        assert_eq!(epoch_to_utc(0), (1970, 1, 1, 0, 0, 0));
        // 2000-01-01 00:00:00 UTC = 946684800
        assert_eq!(epoch_to_utc(946_684_800), (2000, 1, 1, 0, 0, 0));
    }

    #[test]
    fn create_and_list_backups() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("settings.json");
        fs::write(&source, r#"{"test": true}"#).unwrap();

        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        let ts = timestamp();
        let backup_name = format!("claude-code-{ts}.json");
        let backup_path = backup_dir.join(&backup_name);
        fs::copy(&source, &backup_path).unwrap();

        assert!(backup_path.exists());
        let contents = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(contents, r#"{"test": true}"#);
    }

    #[test]
    fn list_backups_empty_when_no_dir() {
        let result = list_backups("nonexistent-tool-xyz");
        assert!(result.is_ok());
        let backups = result.unwrap();
        assert!(backups.is_empty());
    }

    #[test]
    fn restore_from_latest_backup() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        // Create two backups with different timestamps
        let old_backup = backup_dir.join("test-tool-20240101-000000.json");
        let new_backup = backup_dir.join("test-tool-20241231-235959.json");
        fs::write(&old_backup, "old content").unwrap();
        fs::write(&new_backup, "new content").unwrap();

        let backups = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0].path, new_backup);
        assert_eq!(backups[1].path, old_backup);

        let target = dir.path().join("restored.json");
        fs::copy(&backups[0].path, &target).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new content");
    }

    #[test]
    fn restore_from_legacy_epoch_backup() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        // Legacy epoch-format backup
        let epoch_backup = backup_dir.join("test-tool-2000.json");
        fs::write(&epoch_backup, "epoch content").unwrap();

        let backups = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].time_display, "epoch: 2000");

        let target = dir.path().join("restored.json");
        fs::copy(&backups[0].path, &target).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "epoch content");
    }

    #[test]
    fn mixed_new_and_legacy_backups_sort_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        // Legacy epoch backup (small number = old)
        fs::write(backup_dir.join("test-tool-1000.json"), "old").unwrap();
        // New format backup (newer)
        fs::write(backup_dir.join("test-tool-20241231-235959.json"), "new").unwrap();

        let backups = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(backups.len(), 2);
        // New format should sort before old epoch format ('2' > '1')
        assert!(backups[0].name.contains("20241231"));
        assert!(backups[1].name.contains("1000"));
    }

    #[test]
    fn cleanup_removes_old_backups() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        for i in 0..=MAX_BACKUPS {
            let ts = format!("2024{:02}{:02}-{:02}{:02}{:02}", 1, i + 1, 0, 0, 0);
            let path = backup_dir.join(format!("test-tool-{ts}.json"));
            fs::write(&path, format!("content-{i}")).unwrap();
        }
        assert_eq!(
            list_backups_in_dir("test-tool", &backup_dir).unwrap().len(),
            MAX_BACKUPS + 1
        );

        cleanup_old_backups("test-tool", &backup_dir).unwrap();
        let remaining = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(remaining.len(), MAX_BACKUPS);
    }

    #[test]
    fn list_backups_filters_by_tool_name() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        fs::write(backup_dir.join("claude-code-20240101-000000.json"), "").unwrap();
        fs::write(backup_dir.join("codex-20240101-000000.toml"), "").unwrap();
        fs::write(backup_dir.join("claude-code-20240102-000000.json"), "").unwrap();

        let claude = list_backups_in_dir("claude-code", &backup_dir).unwrap();
        let codex = list_backups_in_dir("codex", &backup_dir).unwrap();

        assert_eq!(claude.len(), 2);
        assert_eq!(codex.len(), 1);
    }

    #[test]
    fn backup_info_contains_size() {
        let dir = tempfile::tempdir().unwrap();
        let backup_dir = dir.path().join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        let path = backup_dir.join("test-tool-20240101-000000.json");
        fs::write(&path, "hello world").unwrap();

        let backups = list_backups_in_dir("test-tool", &backup_dir).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].size, 11);
    }
}
