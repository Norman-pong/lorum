//! Interactive `init` command implementation.

use std::io::{BufRead, IsTerminal, Write};

use crate::config::{self, LorumConfig};
use crate::error::LorumError;

/// Run interactive init, creating a new configuration file.
///
/// # Arguments
///
/// * `local` – Create `.lorum/config.yaml` in the current directory.
/// * `yes`   – Skip interactive prompts and auto-import detected tools.
///
/// # Errors
///
/// Returns `LorumError::Other` when run non-interactively without `--yes`,
/// or when the target config file already exists.
pub fn run_interactive_init(local: bool, yes: bool) -> Result<(), LorumError> {
    if !std::io::stdin().is_terminal() && !yes {
        return Err(LorumError::Other {
            message: "interactive init requires a TTY, use --yes for non-interactive mode".into(),
        });
    }

    let path = if local {
        std::env::current_dir()?.join(".lorum").join("config.yaml")
    } else {
        config::global_config_path()?
    };

    if path.exists() {
        return Err(LorumError::Other {
            message: format!("config already exists: {}", path.display()),
        });
    }

    let detected = super::detect_installed_tools();
    let mut cfg = LorumConfig::default();

    if yes {
        if !detected.is_empty() {
            for tool in &detected {
                if let Err(e) = import_tool(tool, &mut cfg) {
                    eprintln!("warning: failed to import from {}: {e}", tool);
                }
            }
        }
    } else if !detected.is_empty() {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for tool in &detected {
            write!(stdout, "Import configuration from {}? [y/N] ", tool)
                .map_err(|e| LorumError::Io { source: e })?;
            stdout.flush().map_err(|e| LorumError::Io { source: e })?;
            let mut line = String::new();
            stdin
                .lock()
                .read_line(&mut line)
                .map_err(|e| LorumError::Io { source: e })?;
            let trimmed = line.trim().to_lowercase();
            if trimmed == "y" || trimmed == "yes" {
                if let Err(e) = import_tool(tool, &mut cfg) {
                    eprintln!("warning: failed to import from {}: {e}", tool);
                }
            }
        }
    }

    config::save_config(&path, &cfg)?;
    println!("created config at: {}", path.display());

    if local {
        let parent = path.parent().ok_or_else(|| LorumError::Other {
            message: "cannot determine parent directory for .gitignore".into(),
        })?;
        let gitignore_path = parent.join(".gitignore");
        std::fs::write(&gitignore_path, "skills/\n").map_err(|e| LorumError::Io { source: e })?;
        println!("created .gitignore at: {}", gitignore_path.display());
    }

    Ok(())
}

fn import_tool(tool: &str, cfg: &mut LorumConfig) -> Result<(), LorumError> {
    let adapter = crate::adapters::find_adapter(tool)
        .ok_or_else(|| LorumError::AdapterNotFound { name: tool.into() })?;
    let mcp = adapter.read_mcp()?;
    for (name, server) in &mcp.servers {
        cfg.mcp.servers.insert(name.clone(), server.clone());
    }
    Ok(())
}
