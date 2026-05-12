//! Rule CRUD command handlers.
//!
//! Functions for initialising, adding, removing, editing, listing, showing,
//! syncing, and importing rule sections in the `.lorum/RULES.md` file.

use std::path::Path;

use crate::adapters::find_rules_adapter;
use crate::error::LorumError;
use crate::rules::{self, RulesFile, RulesSection};
use crate::sync::{self, RulesDryRunResult, RulesSyncResult};

/// Resolve the project root directory.
///
/// Uses [`rules::find_project_root`] starting from the current working
/// directory. Falls back to the current working directory when no `.lorum/`
/// directory is found.
fn resolve_project_root() -> Result<std::path::PathBuf, LorumError> {
    let cwd = std::env::current_dir()?;
    Ok(rules::find_project_root(&cwd).unwrap_or(cwd))
}

/// Run the `rule init` subcommand.
///
/// Creates an empty `.lorum/RULES.md` template at the project root. If the
/// file already exists, prints a message and returns `Ok(())` without
/// overwriting.
pub fn run_rule_init() -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_init(&root)
}

/// Internal implementation of [`run_rule_init`] that accepts an explicit
/// project root. Used by both the CLI dispatcher and unit tests.
pub(crate) fn rule_init(root: &Path) -> Result<(), LorumError> {
    let path = root.join(".lorum").join("RULES.md");

    if path.exists() {
        println!("rules file already exists: {}", path.display());
        return Ok(());
    }

    let rules = RulesFile {
        preamble: crate::rules::DEFAULT_PREAMBLE.to_owned(),
        sections: vec![RulesSection {
            name: "Code Style".to_owned(),
            content: "Add your code style rules here.".to_owned(),
        }],
    };

    rules::save_rules(root, &rules)?;
    println!("created rules file: {}", path.display());
    Ok(())
}

/// Run the `rule add` subcommand.
///
/// Appends a new section with the given `name` and `content` to the rules
/// file. Returns an error if a section with the same name already exists.
pub fn run_rule_add(name: &str, content: &str) -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_add(&root, name, content)
}

/// Internal implementation of [`run_rule_add`] that accepts an explicit
/// project root.
pub(crate) fn rule_add(root: &Path, name: &str, content: &str) -> Result<(), LorumError> {
    let mut rules = rules::load_rules(root)?;

    if rules.section(name).is_some() {
        return Err(LorumError::Other {
            message: format!("section already exists: {name}"),
        });
    }

    rules.sections.push(RulesSection {
        name: name.to_owned(),
        content: content.to_owned(),
    });

    rules::save_rules(root, &rules)?;
    println!("added section: {name}");
    Ok(())
}

/// Run the `rule remove` subcommand.
///
/// Removes the section with the given `name` from the rules file. Returns an
/// error if no such section exists.
pub fn run_rule_remove(name: &str) -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_remove(&root, name)
}

/// Internal implementation of [`run_rule_remove`] that accepts an explicit
/// project root.
pub(crate) fn rule_remove(root: &Path, name: &str) -> Result<(), LorumError> {
    let mut rules = rules::load_rules(root)?;

    let before = rules.sections.len();
    rules.sections.retain(|s| s.name != name);

    if rules.sections.len() == before {
        return Err(LorumError::Other {
            message: format!("section not found: {name}"),
        });
    }

    rules::save_rules(root, &rules)?;
    println!("removed section: {name}");
    Ok(())
}

/// Run the `rule edit` subcommand.
///
/// Replaces the content of the section with the given `name`. Returns an
/// error if no such section exists.
pub fn run_rule_edit(name: &str, content: &str) -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_edit(&root, name, content)
}

/// Internal implementation of [`run_rule_edit`] that accepts an explicit
/// project root.
pub(crate) fn rule_edit(root: &Path, name: &str, content: &str) -> Result<(), LorumError> {
    let mut rules = rules::load_rules(root)?;

    let section = rules
        .sections
        .iter_mut()
        .find(|s| s.name == name)
        .ok_or_else(|| LorumError::Other {
            message: format!("section not found: {name}"),
        })?;

    section.content = content.to_owned();

    rules::save_rules(root, &rules)?;
    println!("updated section: {name}");
    Ok(())
}

/// Run the `rule list` subcommand.
///
/// Prints all section names with their content line counts in an aligned
/// table. Format: `{name:<20} {lines:>6} lines`.
pub fn run_rule_list() -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_list(&root)
}

/// Internal implementation of [`run_rule_list`] that accepts an explicit
/// project root.
pub(crate) fn rule_list(root: &Path) -> Result<(), LorumError> {
    let rules = rules::load_rules(root)?;

    if rules.sections.is_empty() {
        println!("no rule sections defined");
        return Ok(());
    }

    println!("{:<20} {:>6}", "SECTION", "LINES");
    for section in &rules.sections {
        let line_count = if section.content.is_empty() {
            0
        } else {
            section.content.lines().count()
        };
        println!("{:<20} {:>6} lines", section.name, line_count);
    }
    Ok(())
}

/// Run the `rule show` subcommand.
///
/// When `name` is `Some`, prints the content of that section. When `name` is
/// `None`, prints the entire rules file.
pub fn run_rule_show(name: Option<&str>) -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_show(&root, name)
}

/// Internal implementation of [`run_rule_show`] that accepts an explicit
/// project root.
pub(crate) fn rule_show(root: &Path, name: Option<&str>) -> Result<(), LorumError> {
    let rules = rules::load_rules(root)?;

    match name {
        Some(section_name) => {
            let section = rules
                .section(section_name)
                .ok_or_else(|| LorumError::Other {
                    message: format!("section not found: {section_name}"),
                })?;
            if section.content.is_empty() {
                println!("(empty section)");
            } else {
                println!("{}", section.content);
            }
        }
        None => {
            let content = rules::render_rules(&rules);
            if content.is_empty() {
                println!("(empty rules file)");
            } else {
                print!("{content}");
            }
        }
    }
    Ok(())
}

/// Run the `rule sync` subcommand.
///
/// Syncs rules content to target tools. When `dry_run` is true, only previews
/// what would change. When `tools` is non-empty, only the specified tools are
/// synced; otherwise all registered rules adapters are synced.
pub fn run_rule_sync(dry_run: bool, tools: &[String]) -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_sync(&root, dry_run, tools)
}

/// Internal implementation of [`run_rule_sync`] that accepts an explicit
/// project root.
pub(crate) fn rule_sync(root: &Path, dry_run: bool, tools: &[String]) -> Result<(), LorumError> {
    let rules = rules::load_rules(root)?;
    let content = rules::render_rules(&rules);

    if dry_run {
        let results = if tools.is_empty() {
            sync::dry_run_rules_all(root, &content)
        } else {
            sync::dry_run_rules_tools(root, &content, tools)
        };
        print_rules_dry_run_results(&results);
    } else {
        let results = if tools.is_empty() {
            sync::sync_rules_all(root, &content)
        } else {
            sync::sync_rules_tools(root, &content, tools)
        };
        let failed = print_rules_sync_results(&results);
        if failed > 0 {
            eprintln!("{failed} tool(s) failed to sync");
        }
    }
    Ok(())
}

/// Run the `rule import` subcommand.
///
/// Reads rules content from the specified tool's adapter and either creates a
/// new `.lorum/RULES.md` or appends an `## Imported from {tool}` section to
/// the existing file.
pub fn run_rule_import(from: &str) -> Result<(), LorumError> {
    let root = resolve_project_root()?;
    rule_import(&root, from)
}

/// Internal implementation of [`run_rule_import`] that accepts an explicit
/// project root.
pub(crate) fn rule_import(root: &Path, from: &str) -> Result<(), LorumError> {
    let adapter = find_rules_adapter(from).ok_or_else(|| LorumError::AdapterNotFound {
        name: from.to_owned(),
    })?;

    let imported_content = adapter.read_rules(root)?.ok_or_else(|| LorumError::Other {
        message: format!("no rules file found for {from}"),
    })?;

    let path = root.join(".lorum").join("RULES.md");
    if path.exists() {
        let mut rules = rules::load_rules(root)?;
        let section_name = format!("Imported from {from}");
        // Remove any existing import section with the same name so re-import
        // replaces it.
        rules.sections.retain(|s| s.name != section_name);
        rules.sections.push(RulesSection {
            name: section_name.clone(),
            content: imported_content,
        });
        rules::save_rules(root, &rules)?;
        println!("imported rules from {from} (appended as section: {section_name})");
    } else {
        let rules = RulesFile {
            preamble: crate::rules::DEFAULT_PREAMBLE.to_owned(),
            sections: vec![RulesSection {
                name: format!("Imported from {from}"),
                content: imported_content,
            }],
        };
        rules::save_rules(root, &rules)?;
        println!("created rules file with imported content from {from}");
    }
    Ok(())
}

/// Print dry-run results for rules sync in an aligned table.
fn print_rules_dry_run_results(results: &[RulesDryRunResult]) {
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

/// Print sync results for rules and return the number of failures.
fn print_rules_sync_results(results: &[RulesSyncResult]) -> usize {
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
