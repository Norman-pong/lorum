//! Skill management command handlers.
//!
//! Functions for listing, showing, adding, removing, and syncing skills
//! in the unified skills directory (`~/.config/lorum/skills/` or
//! `.lorum/skills/`).

use std::path::{Path, PathBuf};

use crate::error::LorumError;
use crate::skills::{copy_dir_recursive, global_skills_dir, scan_skills_dir};
use crate::sync::{
    SkillsDryRunResult, SkillsSyncResult, dry_run_skills_all, dry_run_skills_tools,
    sync_skills_all, sync_skills_tools,
};

/// Resolve the unified skills directory.
///
/// Returns the global skills directory by default. When `project_root` is
/// provided, uses the project-level `.lorum/skills/` directory.
fn resolve_skills_dir(project_root: Option<&Path>) -> Result<PathBuf, LorumError> {
    match project_root {
        Some(root) => Ok(crate::skills::project_skills_dir(root)),
        None => global_skills_dir(),
    }
}

/// Run the `skill list` subcommand.
///
/// Prints all skills found in the unified skills directory.
pub fn run_skill_list(project_root: Option<&Path>) -> Result<(), LorumError> {
    let dir = resolve_skills_dir(project_root)?;
    let skills = scan_skills_dir(&dir)?;

    if skills.is_empty() {
        println!("no skills found in {}", dir.display());
        return Ok(());
    }

    println!("{:<20} DESCRIPTION", "NAME");
    for skill in &skills {
        println!("{:<20} {}", skill.manifest.name, skill.manifest.description);
    }
    Ok(())
}

/// Run the `skill show` subcommand.
///
/// Prints the full content of a skill's SKILL.md.
pub fn run_skill_show(name: &str, project_root: Option<&Path>) -> Result<(), LorumError> {
    let dir = resolve_skills_dir(project_root)?;
    let skills = scan_skills_dir(&dir)?;

    let skill = skills
        .iter()
        .find(|s| s.manifest.name == name)
        .ok_or_else(|| LorumError::Other {
            message: format!("skill not found: {name}"),
        })?;

    print!("{}", skill.content);
    Ok(())
}

/// Validate that a skill name is safe to use as a directory name.
///
/// Rejects empty names, path separators, and special path components
/// that could lead to directory traversal.
fn validate_skill_name(name: &str) -> Result<(), LorumError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name.contains("..")
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(LorumError::Other {
            message: format!("invalid skill name: {name}"),
        });
    }
    Ok(())
}

/// Run the `skill add` subcommand.
///
/// Imports a skill directory into the unified skills directory.
/// The source directory must contain a SKILL.md file.
pub fn run_skill_add(
    name: &str,
    from: &str,
    project_root: Option<&Path>,
) -> Result<(), LorumError> {
    validate_skill_name(name)?;

    let source = Path::new(from);

    // Verify source directory name matches skill name.
    if let Some(src_name) = source.file_name().and_then(|n| n.to_str()) {
        if src_name != name {
            return Err(LorumError::Other {
                message: format!(
                    "source directory name '{}' does not match skill name '{}'",
                    src_name, name
                ),
            });
        }
    }

    let skill_md = source.join("SKILL.md");
    if !skill_md.exists() {
        return Err(LorumError::Other {
            message: format!(
                "source directory does not contain SKILL.md: {}",
                source.display()
            ),
        });
    }

    // Validate frontmatter by parsing.
    let content = std::fs::read_to_string(&skill_md)?;
    let manifest = crate::skills::parse_skill_manifest(&content, &skill_md)?;
    if manifest.name != name {
        return Err(LorumError::Other {
            message: format!(
                "skill name mismatch: frontmatter says '{}' but requested '{}'",
                manifest.name, name
            ),
        });
    }

    let target_dir = resolve_skills_dir(project_root)?;
    let target = target_dir.join(name);

    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::create_dir_all(&target_dir)?;
    copy_dir_recursive(source, &target)?;

    println!("added skill: {name}");
    Ok(())
}

/// Run the `skill remove` subcommand.
///
/// Removes a skill directory from the unified skills directory.
pub fn run_skill_remove(name: &str, project_root: Option<&Path>) -> Result<(), LorumError> {
    validate_skill_name(name)?;

    let dir = resolve_skills_dir(project_root)?;
    let target = dir.join(name);

    if !target.exists() {
        return Err(LorumError::Other {
            message: format!("skill not found: {name}"),
        });
    }

    std::fs::remove_dir_all(&target)?;
    println!("removed skill: {name}");
    Ok(())
}

/// Run the `skill sync` subcommand.
///
/// Syncs skills from the unified directory to target tools.
pub fn run_skill_sync(
    dry_run: bool,
    tools: &[String],
    project_root: Option<&Path>,
) -> Result<(), LorumError> {
    let skills_dir = resolve_skills_dir(project_root)?;

    if dry_run {
        let results = if tools.is_empty() {
            dry_run_skills_all(&skills_dir)
        } else {
            dry_run_skills_tools(&skills_dir, tools)
        };
        print_skills_dry_run_results(&results);
    } else {
        let results = if tools.is_empty() {
            sync_skills_all(&skills_dir)
        } else {
            sync_skills_tools(&skills_dir, tools)
        };
        print_skills_sync_results(&results);
        let failed = results.iter().filter(|r| !r.success).count();
        if failed > 0 {
            eprintln!("{failed} tool(s) failed to sync");
        }
    }
    Ok(())
}

/// Print dry-run results for skills sync in an aligned table.
fn print_skills_dry_run_results(results: &[SkillsDryRunResult]) {
    println!(
        "{:<15} {:<8} {:<10} {:<10} TO REMOVE",
        "TOOL", "STATUS", "TO UPDATE", "UP TO DATE"
    );
    for r in results {
        let status = if r.success { "OK" } else { "FAIL" };
        println!(
            "{:<15} {:<8} {:<10} {:<10} {}",
            r.tool, status, r.skills_to_update, r.skills_up_to_date, r.skills_to_remove
        );
        if let Some(err) = &r.error {
            println!("  error: {err}");
        }
    }
}

/// Print sync results for skills and return the number of failures.
fn print_skills_sync_results(results: &[SkillsSyncResult]) {
    println!("{:<15} {:<6} SKILLS SYNCED", "TOOL", "STATUS");
    for r in results {
        let status = if r.success { "OK" } else { "FAIL" };
        println!("{:<15} {:<6} {}", r.tool, status, r.skills_synced);
        if let Some(err) = &r.error {
            println!("  error: {err}");
        }
    }
}
