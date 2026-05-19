//! Interactive `init` command implementation.

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

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

    // Detect .git/ directory and suggest --local when appropriate.
    let local = if !local && git_repo_detected() {
        if yes {
            true
        } else {
            let stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            print!(
                "Git repository detected. Create a project-level config (.lorum/config.yaml)? [Y/n] "
            );
            stdout.flush().map_err(|e| LorumError::Io { source: e })?;
            let mut line = String::new();
            stdin
                .lock()
                .read_line(&mut line)
                .map_err(|e| LorumError::Io { source: e })?;
            let trimmed = line.trim().to_lowercase();
            trimmed != "n" && trimmed != "no"
        }
    } else {
        local
    };

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

    // Step 1: list detected tools.
    if !detected.is_empty() {
        println!("Detected installed tools:");
        for tool in &detected {
            println!("  - {tool}");
        }
        println!();
    }

    let mut cfg = LorumConfig::default();
    let selected: Vec<String> = if yes {
        detected.clone()
    } else if !detected.is_empty() {
        select_tools_interactive(&detected)?
    } else {
        Vec::new()
    };

    // Step 2: import configuration from selected tools.
    for tool in &selected {
        if let Err(e) = import_tool(tool, &mut cfg) {
            eprintln!("warning: failed to import from {}: {e}", tool);
        }
    }

    config::save_config(&path, &cfg)?;

    // Step 3: summary.
    let server_count = cfg.mcp.servers.len();
    println!("created config at: {}", path.display());
    if server_count > 0 {
        println!("  imported {server_count} MCP server(s)");
    }

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

/// Check whether the current working directory is inside a Git repository.
pub(crate) fn git_repo_detected() -> bool {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };
    is_git_repo(&cwd)
}

/// Walk upward from `dir` looking for a `.git` entry.
pub(crate) fn is_git_repo(dir: &Path) -> bool {
    let mut current = Some(dir);
    while let Some(d) = current {
        if d.join(".git").exists() {
            return true;
        }
        current = d.parent();
    }
    false
}

/// Interactively select which detected tools to import from.
///
/// The user can type comma-separated indices, "all", or press Enter to skip.
fn select_tools_interactive(detected: &[String]) -> Result<Vec<String>, LorumError> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    println!("Which tools should lorum manage? (comma-separated indices, or \"all\")");
    for (i, tool) in detected.iter().enumerate() {
        println!("  {}) {tool}", i + 1);
    }
    print!("Selection [all]: ");
    stdout.flush().map_err(|e| LorumError::Io { source: e })?;

    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| LorumError::Io { source: e })?;

    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return Ok(detected.to_vec());
    }

    let mut selected = Vec::new();
    for part in trimmed.split(',') {
        let part = part.trim();
        if let Ok(idx) = part.parse::<usize>() {
            if idx >= 1 && idx <= detected.len() {
                selected.push(detected[idx - 1].clone());
            }
        }
    }
    Ok(selected)
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
