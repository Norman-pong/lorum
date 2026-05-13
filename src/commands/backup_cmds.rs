//! Backup command handlers.

use std::collections::HashSet;

use crate::error::LorumError;

/// Run the `backup list` subcommand: lists all backup files with details.
pub fn run_backup_list(_config_path: Option<&str>) -> Result<(), LorumError> {
    let dir = crate::backup::backup_dir()?;
    if !dir.exists() {
        println!("no backups found");
        return Ok(());
    }

    // Collect unique tool names first, then list backups per tool once.
    let mut tool_names: HashSet<String> = HashSet::new();
    for item in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
        let name = match item.path().file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if let Some((tool, _)) = name.split_once('-') {
            tool_names.insert(tool.to_string());
        }
    }

    let mut entries: Vec<(String, crate::backup::BackupInfo)> = Vec::new();
    for tool_name in &tool_names {
        match crate::backup::list_backups(tool_name) {
            Ok(list) => {
                for info in list {
                    entries.push((tool_name.clone(), info));
                }
            }
            Err(_) => continue,
        }
    }

    if entries.is_empty() {
        println!("no backups found");
        return Ok(());
    }

    // Sort by timestamp descending (newest first)
    entries.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));

    println!("{:<15} {:<20} {:>10} FILE", "TOOL", "TIME", "SIZE");
    for (tool, b) in &entries {
        let size = format_size(b.size);
        println!(
            "{:<15} {:<20} {:>10} {}",
            tool, b.time_display, size, b.name
        );
    }
    Ok(())
}

fn format_size(size: u64) -> String {
    if size == 0 {
        return "0B".to_string();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut s = size as f64;
    let mut idx = 0;
    while s >= 1024.0 && idx < UNITS.len() - 1 {
        s /= 1024.0;
        idx += 1;
    }
    if s.fract() < 0.05 {
        format!("{:.0}{}", s, UNITS[idx])
    } else {
        format!("{:.1}{}", s, UNITS[idx])
    }
}

/// Run the `backup create` subcommand: creates backups for specified tools.
pub fn run_backup_create(
    tools: &[String],
    all: bool,
    _config_path: Option<&str>,
) -> Result<(), LorumError> {
    let tool_names: Vec<String> = if all || tools.is_empty() {
        crate::adapters::all_adapters()
            .iter()
            .map(|a| a.name().to_string())
            .collect()
    } else {
        tools.to_vec()
    };

    let mut created = 0;
    for tool_name in &tool_names {
        let adapter = match crate::adapters::find_adapter(tool_name) {
            Some(a) => a,
            None => {
                eprintln!("warning: unknown tool '{tool_name}'");
                continue;
            }
        };

        for path in adapter.config_paths() {
            if path.exists() {
                match crate::backup::create_backup(tool_name, &path) {
                    Ok(backup_path) => {
                        println!("created backup: {}", backup_path.display());
                        created += 1;
                    }
                    Err(e) => {
                        eprintln!("warning: failed to backup {}: {e}", path.display());
                    }
                }
            }
        }
    }

    println!("created {created} backup(s)");
    Ok(())
}

/// Run the `backup restore` subcommand: restores a tool config from backup.
pub fn run_backup_restore(
    tool: &str,
    backup: Option<&str>,
    _config_path: Option<&str>,
) -> Result<(), LorumError> {
    let adapter = crate::adapters::find_adapter(tool)
        .ok_or_else(|| LorumError::AdapterNotFound { name: tool.into() })?;

    let paths = adapter.config_paths();
    if paths.is_empty() {
        return Err(LorumError::Other {
            message: format!("no config path for {tool}"),
        });
    }

    if let Some(backup_file) = backup {
        let backup_path = std::path::Path::new(backup_file);
        let backup_path = if backup_path.is_relative() {
            crate::backup::backup_dir()?.join(backup_file)
        } else {
            backup_path.to_path_buf()
        };
        if !backup_path.exists() {
            return Err(LorumError::ConfigNotFound { path: backup_path });
        }
        // Validate that the backup path is within the backup directory.
        let canonical_backup =
            std::fs::canonicalize(&backup_path).map_err(|e| LorumError::Io { source: e })?;
        let canonical_backup_dir = std::fs::canonicalize(crate::backup::backup_dir()?)
            .map_err(|e| LorumError::Io { source: e })?;
        if !canonical_backup.starts_with(&canonical_backup_dir) {
            return Err(LorumError::Other {
                message: format!(
                    "backup path '{}' is outside the backup directory",
                    backup_path.display()
                ),
            });
        }
        crate::backup::restore_backup_from_path(&backup_path, &paths[0])?;
        println!("restored {tool} from {}", backup_path.display());
    } else {
        crate::backup::restore_backup(tool, &paths[0])?;
        println!("restored {tool} from latest backup");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_backup_create_empty_tools_all_false() {
        // empty tools and all=false means all adapters are backed up
        let result = run_backup_create(&[], false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn run_backup_create_with_specific_tools() {
        // Pass a nonexistent tool name — should warn but still return Ok
        let result = run_backup_create(&["nonexistent-tool-xyz".into()], false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn format_size_zero() {
        assert_eq!(format_size(0), "0B");
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(512), "512B");
    }

    #[test]
    fn format_size_kb() {
        assert_eq!(format_size(1024), "1KB");
    }

    #[test]
    fn format_size_mb() {
        assert_eq!(format_size(1_048_576), "1MB");
    }

    #[test]
    fn format_size_gb() {
        assert_eq!(format_size(1_073_741_824), "1GB");
    }

    #[test]
    fn format_size_fractional() {
        // 1.5 KB
        assert_eq!(format_size(1536), "1.5KB");
    }

    #[test]
    fn format_size_large_gb() {
        assert_eq!(format_size(2_147_483_648), "2GB");
    }
}
