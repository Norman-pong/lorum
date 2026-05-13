//! Rules data model and parser for lorum's `RULES.md` files.
//!
//! This module defines the [`RulesSection`] and [`RulesFile`] data structures,
//! provides a Markdown-based parser ([`parse_rules`]) and renderer
//! ([`render_rules`]), and offers file I/O helpers ([`load_rules`],
//! [`save_rules`]) plus a project-root discovery function
//! ([`find_project_root`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::LorumError;

/// Default preamble text for newly created rules files.
pub const DEFAULT_PREAMBLE: &str = "\
# Project Rules

This file defines AI coding rules managed by lorum.
Each `##` heading defines a rule section that can be synced to target tools.";

/// A single rule section extracted from a `##` heading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulesSection {
    /// Section name (the text after `## `).
    pub name: String,
    /// Section body (lines between this heading and the next `##`).
    pub content: String,
}

/// Parsed representation of a `RULES.md` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RulesFile {
    /// Content before the first `##` heading (the file preamble).
    pub preamble: String,
    /// Ordered list of rule sections.
    pub sections: Vec<RulesSection>,
}

impl RulesFile {
    /// Returns the first section with the given name, or `None`.
    pub fn section(&self, name: &str) -> Option<&RulesSection> {
        self.sections.iter().find(|s| s.name == name)
    }
}

/// Parses a Markdown string into a [`RulesFile`].
///
/// # Parsing rules
///
/// - Everything before the first `## ` line is the preamble.
/// - Each line starting with `## ` (at column 0) begins a new section.
/// - Section content spans from the heading line to the next `## ` line
///   (exclusive on both ends).
/// - Lines starting with `###` or deeper headings are **not** section
///   delimiters.
/// - When two sections share the same name, the latter overwrites the
///   former.
/// - An empty input produces an empty [`RulesFile`].
///
/// # Examples
///
/// ```
/// use lorum::rules::{parse_rules, RulesFile};
///
/// let md = "## Style\nUse 4-space indentation.\n";
/// let parsed = parse_rules(md);
/// assert_eq!(parsed.preamble, "");
/// assert_eq!(parsed.sections.len(), 1);
/// ```
pub fn parse_rules(content: &str) -> RulesFile {
    let mut preamble_lines: Vec<&str> = Vec::new();
    let mut current_section: Option<(&str, Vec<&str>)> = None;

    // Deduplicate on the fly: latter sections with the same name overwrite
    // earlier ones, but we preserve the original order of first appearance.
    let mut seen = BTreeMap::<String, usize>::new();
    let mut sections: Vec<RulesSection> = Vec::new();

    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix("## ") {
            // Flush the previous section (if any) before starting a new one.
            if let Some((name, body_lines)) = current_section.take() {
                let content = body_lines.join("\n");
                let trimmed = content.trim_end();
                if let Some(&idx) = seen.get(name) {
                    sections[idx].content = trimmed.to_owned();
                } else {
                    seen.insert(name.to_owned(), sections.len());
                    sections.push(RulesSection {
                        name: name.to_owned(),
                        content: trimmed.to_owned(),
                    });
                }
            }
            let name = stripped.trim();
            current_section = Some((name, Vec::new()));
        } else if let Some((_name, ref mut body)) = current_section {
            body.push(line);
        } else {
            preamble_lines.push(line);
        }
    }

    // Flush the last section.
    if let Some((name, body_lines)) = current_section {
        let content = body_lines.join("\n");
        let trimmed = content.trim_end();
        if let Some(&idx) = seen.get(name) {
            sections[idx].content = trimmed.to_owned();
        } else {
            seen.insert(name.to_owned(), sections.len());
            sections.push(RulesSection {
                name: name.to_owned(),
                content: trimmed.to_owned(),
            });
        }
    }

    // Trim trailing blank lines from the preamble.
    let preamble = preamble_lines.join("\n");
    let preamble = preamble.trim_end().to_owned();

    RulesFile { preamble, sections }
}

/// Serialises a [`RulesFile`] back into a Markdown string.
///
/// # Rendering rules
///
/// - The preamble is emitted first.  If non-empty it is followed by two
///   newlines to separate it from the first section.
/// - Each section is rendered as `## {name}\n{content}\n\n`.
///
/// # Examples
///
/// ```
/// use lorum::rules::{parse_rules, render_rules};
///
/// let md = "## Style\nUse 4-space indentation.\n";
/// let parsed = parse_rules(md);
/// let rendered = render_rules(&parsed);
/// assert!(rendered.starts_with("## Style\n"));
/// ```
pub fn render_rules(rules: &RulesFile) -> String {
    let mut out = String::new();

    if !rules.preamble.is_empty() {
        out.push_str(&rules.preamble);
        out.push_str("\n\n");
    }

    for section in &rules.sections {
        out.push_str("## ");
        out.push_str(&section.name);
        out.push('\n');
        if !section.content.is_empty() {
            out.push_str(&section.content);
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

/// Returns the path to the project-level `RULES.md` file.
///
/// The file is located at `{project_root}/.lorum/RULES.md`.
fn rules_file_path(project_root: &Path) -> PathBuf {
    project_root.join(".lorum").join("RULES.md")
}

/// Loads a [`RulesFile`] from `{project_root}/.lorum/RULES.md`.
///
/// # Errors
///
/// - [`LorumError::ConfigNotFound`] if the `RULES.md` file does not exist.
/// - [`LorumError::Io`] for other I/O failures.
pub fn load_rules(project_root: &Path) -> Result<RulesFile, LorumError> {
    let path = rules_file_path(project_root);
    if !path.exists() {
        return Err(LorumError::ConfigNotFound { path });
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(parse_rules(&content))
}

/// Saves a [`RulesFile`] to `{project_root}/.lorum/RULES.md`.
///
/// Parent directories (`.lorum/`) are created automatically if they do not
/// exist.
///
/// # Errors
///
/// - [`LorumError::ConfigWrite`] if the file cannot be written.
pub fn save_rules(project_root: &Path, rules: &RulesFile) -> Result<(), LorumError> {
    let path = rules_file_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
            path: path.clone(),
            source: e,
        })?;
    }
    let content = render_rules(rules);
    std::fs::write(&path, content).map_err(|e| LorumError::ConfigWrite { path, source: e })?;
    Ok(())
}

/// Searches upward from `start_dir` for a directory containing `.lorum/`.
///
/// This follows the same discovery pattern as
/// [`config::find_project_config`](crate::config::find_project_config): it
/// walks parent directories until it finds one that contains a `.lorum/`
/// subdirectory.
///
/// Returns `Some(project_root)` when found, or `None` if no `.lorum/`
/// directory exists above `start_dir`.
pub fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir;
    loop {
        if dir.join(".lorum").is_dir() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_rules ----------------------------------------------------------

    #[test]
    fn parse_empty_file() {
        let rf = parse_rules("");
        assert_eq!(rf.preamble, "");
        assert!(rf.sections.is_empty());
    }

    #[test]
    fn parse_preamble_only() {
        let md = "This is a preamble.\nNo sections here.\n";
        let rf = parse_rules(md);
        assert_eq!(rf.preamble, "This is a preamble.\nNo sections here.");
        assert!(rf.sections.is_empty());
    }

    #[test]
    fn parse_multiple_sections() {
        let md = "\
## Style
Use 4-space indentation.

## Testing
Run `cargo test`.

";
        let rf = parse_rules(md);
        assert_eq!(rf.preamble, "");
        assert_eq!(rf.sections.len(), 2);
        assert_eq!(rf.sections[0].name, "Style");
        assert_eq!(rf.sections[0].content, "Use 4-space indentation.");
        assert_eq!(rf.sections[1].name, "Testing");
        assert_eq!(rf.sections[1].content, "Run `cargo test`.");
    }

    #[test]
    fn parse_duplicate_section_names_latter_wins() {
        let md = "\
## Style
Old style rule.

## Style
New style rule.
";
        let rf = parse_rules(md);
        assert_eq!(rf.sections.len(), 1);
        assert_eq!(rf.sections[0].name, "Style");
        assert_eq!(rf.sections[0].content, "New style rule.");
    }

    #[test]
    fn parse_h3_not_a_delimiter() {
        let md = "\
## Main
Some content
### Sub-heading
More content
";
        let rf = parse_rules(md);
        assert_eq!(rf.sections.len(), 1);
        assert_eq!(rf.sections[0].name, "Main");
        assert!(rf.sections[0].content.contains("### Sub-heading"));
    }

    #[test]
    fn parse_h2_not_at_line_start() {
        let md = "\
## Section
  ## Not a heading
Still section content
";
        let rf = parse_rules(md);
        assert_eq!(rf.sections.len(), 1);
        assert_eq!(rf.sections[0].name, "Section");
        assert!(rf.sections[0].content.contains("  ## Not a heading"));
    }

    #[test]
    fn parse_preamble_and_sections() {
        let md = "\
# Project Rules

Some preamble text.

## Style
Indent with 4 spaces.
";
        let rf = parse_rules(md);
        assert_eq!(rf.preamble, "# Project Rules\n\nSome preamble text.");
        assert_eq!(rf.sections.len(), 1);
        assert_eq!(rf.sections[0].name, "Style");
    }

    // --- roundtrip ------------------------------------------------------------

    #[test]
    fn roundtrip_parse_render_parse() {
        let original = "\
## Alpha
Alpha content line 1.
Alpha content line 2.

## Beta
Beta content.

";
        let parsed = parse_rules(original);
        let rendered = render_rules(&parsed);
        let reparsed = parse_rules(&rendered);
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn roundtrip_with_preamble() {
        let original = "\
# My Rules

Preamble here.

## Section A
Content A.
";
        let parsed = parse_rules(original);
        let rendered = render_rules(&parsed);
        let reparsed = parse_rules(&rendered);
        assert_eq!(parsed, reparsed);
    }

    // --- render_rules ---------------------------------------------------------

    #[test]
    fn render_empty_rules() {
        let rf = RulesFile {
            preamble: String::new(),
            sections: vec![],
        };
        assert_eq!(render_rules(&rf), "");
    }

    #[test]
    fn render_preamble_only() {
        let rf = RulesFile {
            preamble: "Hello world.".to_owned(),
            sections: vec![],
        };
        assert_eq!(render_rules(&rf), "Hello world.\n\n");
    }

    // --- file I/O -------------------------------------------------------------

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let rules = RulesFile {
            preamble: "# My Rules".to_owned(),
            sections: vec![
                RulesSection {
                    name: "Style".to_owned(),
                    content: "Use 4 spaces.".to_owned(),
                },
                RulesSection {
                    name: "Testing".to_owned(),
                    content: "Run `cargo test`.".to_owned(),
                },
            ],
        };

        save_rules(dir.path(), &rules).unwrap();
        let loaded = load_rules(dir.path()).unwrap();

        assert_eq!(loaded, rules);
    }

    #[test]
    fn load_nonexistent_returns_config_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = load_rules(dir.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            LorumError::ConfigNotFound { .. } => {}
            other => panic!("expected ConfigNotFound, got {other:?}"),
        }
    }

    // --- find_project_root ----------------------------------------------------

    #[test]
    fn find_project_root_finds_lorum_dir() {
        let dir = tempfile::tempdir().unwrap();
        let lorum_dir = dir.path().join(".lorum");
        std::fs::create_dir_all(&lorum_dir).unwrap();
        let nested = dir.path().join("sub").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_project_root(&nested), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn find_project_root_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_project_root(dir.path()), None);
    }
}
