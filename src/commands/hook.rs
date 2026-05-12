//! Hook CRUD command handlers.
//!
//! Functions for adding, removing, listing, and syncing hooks in the
//! lorum configuration file.

use crate::config;
use crate::config::HookHandler;
use crate::error::LorumError;

use super::resolve_path;

/// Run the `hook add` subcommand.
///
/// Adds a new handler for the given event. If the event does not exist, it
/// is created. If a handler with the same matcher already exists for this
/// event, it is replaced.
pub fn run_hook_add(
    event: &str,
    matcher: &str,
    command: &str,
    timeout: Option<u64>,
    handler_type: Option<&str>,
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    if event.chars().any(|c| c.is_uppercase()) {
        return Err(LorumError::Other {
            message: format!("event names must be kebab-case (e.g. 'pre-tool-use'), got: {event}"),
        });
    }

    let path = resolve_path(config_path)?;
    let mut cfg = super::load_config_or_default(&path)?;

    let handlers = cfg.hooks.events.entry(event.to_string()).or_default();
    // Remove any existing handler with the same matcher for this event.
    handlers.retain(|h| h.matcher != matcher);
    handlers.push(HookHandler {
        matcher: matcher.to_string(),
        command: command.to_string(),
        timeout,
        handler_type: handler_type.map(String::from),
    });

    config::save_config(&path, &cfg)?;
    println!("added hook: {event} -> {matcher}");
    Ok(())
}

/// Run the `hook remove` subcommand.
///
/// Removes the handler matching `matcher` from the given event. If no
/// matcher is provided, the entire event (and all its handlers) is removed.
/// Returns an error if the event or matcher does not exist.
pub fn run_hook_remove(
    event: &str,
    matcher: Option<&str>,
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    let path = resolve_path(config_path)?;
    let mut cfg = super::load_config_or_default(&path)?;

    if let Some(m) = matcher {
        let handlers = cfg
            .hooks
            .events
            .get_mut(event)
            .ok_or_else(|| LorumError::Other {
                message: format!("event not found: {event}"),
            })?;
        let before = handlers.len();
        handlers.retain(|h| h.matcher != m);
        if handlers.len() == before {
            return Err(LorumError::Other {
                message: format!("matcher not found: {m}"),
            });
        }
        if handlers.is_empty() {
            cfg.hooks.events.remove(event);
        }
        println!("removed hook: {event} -> {m}");
    } else {
        if cfg.hooks.events.remove(event).is_none() {
            return Err(LorumError::Other {
                message: format!("event not found: {event}"),
            });
        }
        println!("removed event: {event}");
    }

    config::save_config(&path, &cfg)?;
    Ok(())
}

/// Run the `hook list` subcommand.
///
/// Prints all configured hooks in an aligned table.
pub fn run_hook_list(config_path: Option<&str>) -> Result<(), LorumError> {
    let path = resolve_path(config_path)?;
    let cfg = super::load_config_or_default(&path)?;

    if cfg.hooks.events.is_empty() {
        println!("no hooks configured");
        return Ok(());
    }

    println!("{:<20} {:<15} COMMAND", "EVENT", "MATCHER");
    for (event, handlers) in &cfg.hooks.events {
        for h in handlers {
            println!("{:<20} {:<15} {}", event, h.matcher, h.command);
        }
    }
    Ok(())
}

/// Run the `hook sync` subcommand.
///
/// Syncs hooks configuration to target tools. When `dry_run` is true, only
/// previews what would change. When `tools` is non-empty, only the specified
/// tools are synced; otherwise all registered hooks adapters are synced.
pub fn run_hook_sync(
    dry_run: bool,
    tools: &[String],
    config_path: Option<&str>,
) -> Result<(), LorumError> {
    let cfg = if let Some(p) = config_path {
        config::load_config(std::path::Path::new(p))?
    } else {
        config::resolve_effective_config_from_cwd(None)?
    };

    if dry_run {
        let results = if tools.is_empty() {
            crate::sync::dry_run_hooks_all(&cfg.hooks)
        } else {
            crate::sync::dry_run_hooks_tools(&cfg.hooks, tools)
        };
        print_hooks_dry_run_results(&results);
    } else {
        let results = if tools.is_empty() {
            crate::sync::sync_hooks_all(&cfg.hooks)
        } else {
            crate::sync::sync_hooks_tools(&cfg.hooks, tools)
        };
        let failed = print_hooks_sync_results(&results);
        if failed > 0 {
            eprintln!("{failed} tool(s) failed to sync");
        }
    }
    Ok(())
}

/// Print dry-run results for hooks sync in an aligned table.
fn print_hooks_dry_run_results(results: &[crate::sync::HooksDryRunResult]) {
    println!("{:<15} {:<8} NEEDS UPDATE", "TOOL", "STATUS");
    for r in results {
        let status = if r.success { "OK" } else { "FAIL" };
        let update = if r.success {
            if r.needs_update { "yes" } else { "no" }
        } else {
            "-"
        };
        println!("{:<15} {:<8} {update}", r.tool, status);
        if let Some(err) = &r.error {
            println!("  error: {err}");
        }
    }
}

/// Print sync results for hooks and return the number of failures.
fn print_hooks_sync_results(results: &[crate::sync::HooksSyncResult]) -> usize {
    println!("{:<15} {:<6}", "TOOL", "STATUS");
    for r in results {
        let status = if r.success { "OK" } else { "FAIL" };
        println!("{:<15} {status}", r.tool);
        if let Some(err) = &r.error {
            println!("  error: {err}");
        }
    }
    results.iter().filter(|r| !r.success).count()
}
