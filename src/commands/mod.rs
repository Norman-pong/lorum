use crate::error::LorumError;

/// Run the `init` subcommand stub.
pub fn run_init(local: bool) -> Result<(), LorumError> {
    eprintln!("TODO: implement init (local={local})");
    Ok(())
}

/// Run the `import` subcommand stub.
pub fn run_import(from: &str) -> Result<(), LorumError> {
    eprintln!("TODO: implement import (from={from})");
    Ok(())
}

/// Run the `sync` subcommand stub.
pub fn run_sync(dry_run: bool, tools: &[String], expand_env: bool) -> Result<(), LorumError> {
    eprintln!(
        "TODO: implement sync (dry_run={}, tools={:?}, expand_env={})",
        dry_run, tools, expand_env
    );
    Ok(())
}

/// Run the `check` subcommand stub.
pub fn run_check() -> Result<(), LorumError> {
    eprintln!("TODO: implement check");
    Ok(())
}

/// Run the `status` subcommand stub.
pub fn run_status() -> Result<(), LorumError> {
    eprintln!("TODO: implement status");
    Ok(())
}

/// Run the `config` subcommand stub.
pub fn run_config(resolve_env: bool, local: bool, global: bool) -> Result<(), LorumError> {
    eprintln!(
        "TODO: implement config (resolve_env={}, local={}, global={})",
        resolve_env, local, global
    );
    Ok(())
}

/// Run the `backup list` subcommand stub.
pub fn run_backup_list() -> Result<(), LorumError> {
    eprintln!("TODO: implement backup list");
    Ok(())
}

/// Run the `backup restore` subcommand stub.
pub fn run_backup_restore(tool: &str) -> Result<(), LorumError> {
    eprintln!("TODO: implement backup restore (tool={tool})");
    Ok(())
}

/// Run the `mcp add` subcommand stub.
pub fn run_mcp_add() -> Result<(), LorumError> {
    eprintln!("TODO: implement mcp add");
    Ok(())
}

/// Run the `mcp remove` subcommand stub.
pub fn run_mcp_remove() -> Result<(), LorumError> {
    eprintln!("TODO: implement mcp remove");
    Ok(())
}

/// Run the `mcp list` subcommand stub.
pub fn run_mcp_list() -> Result<(), LorumError> {
    eprintln!("TODO: implement mcp list");
    Ok(())
}

/// Run the `mcp edit` subcommand stub.
pub fn run_mcp_edit() -> Result<(), LorumError> {
    eprintln!("TODO: implement mcp edit");
    Ok(())
}

/// Run the `hook` subcommand stub.
pub fn run_hook() -> Result<(), LorumError> {
    eprintln!("TODO: implement hook");
    Ok(())
}

/// Run the `skill` subcommand stub.
pub fn run_skill() -> Result<(), LorumError> {
    eprintln!("TODO: implement skill");
    Ok(())
}
