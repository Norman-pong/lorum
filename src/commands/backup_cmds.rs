//! Backup command handlers.

use crate::error::LorumError;

/// Run the `backup list` subcommand: lists all backup files.
pub fn run_backup_list(_config_path: Option<&str>) -> Result<(), LorumError> {
    let dir = crate::backup::backup_dir()?;
    if !dir.exists() {
        println!("no backups found");
        return Ok(());
    }

    let mut entries: Vec<String> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
        .collect();

    if entries.is_empty() {
        println!("no backups found");
    } else {
        entries.sort();
        for name in entries {
            println!("{name}");
        }
    }
    Ok(())
}

/// Run the `backup restore` subcommand: restores a tool config from backup.
pub fn run_backup_restore(tool: &str, _config_path: Option<&str>) -> Result<(), LorumError> {
    let adapter = crate::adapters::find_adapter(tool)
        .ok_or_else(|| LorumError::AdapterNotFound { name: tool.into() })?;

    let paths = adapter.config_paths();
    if paths.is_empty() {
        return Err(LorumError::Other {
            message: format!("no config path for {tool}"),
        });
    }

    crate::backup::restore_backup(tool, &paths[0])?;
    println!("restored {tool} from backup");
    Ok(())
}
