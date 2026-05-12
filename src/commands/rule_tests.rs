//! Unit tests for rule CRUD commands.
//!
//! Each test creates a temporary project directory with a `.lorum/` folder
//! and calls the internal rule functions with the project root path,
//! ensuring full isolation from the user's real configuration.

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::error::LorumError;
use crate::rules::{self, RulesFile, RulesSection};

/// Helper: create a temp dir with a `.lorum/` subdirectory and return it.
fn setup_project_dir() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    let lorum_dir = root.join(".lorum");
    fs::create_dir_all(&lorum_dir).unwrap();
    (dir, root)
}

/// Helper: create a temp dir with `.lorum/` and pre-populated RULES.md.
fn setup_with_rules(rules: &RulesFile) -> (TempDir, PathBuf) {
    let (dir, root) = setup_project_dir();
    rules::save_rules(&root, rules).unwrap();
    (dir, root)
}

/// Helper: read the rules file back from disk.
fn read_rules(root: &PathBuf) -> RulesFile {
    rules::load_rules(root).unwrap()
}

// ---------------------------------------------------------------------------
// rule_init
// ---------------------------------------------------------------------------

#[test]
fn rule_init_creates_file() {
    let (dir, root) = setup_project_dir();
    super::rule::rule_init(&root).unwrap();

    let loaded = read_rules(&root);
    assert!(!loaded.sections.is_empty());
    assert_eq!(loaded.sections[0].name, "Code Style");
    let path = root.join(".lorum").join("RULES.md");
    assert!(path.exists());
    let _ = dir;
}

#[test]
fn rule_init_skips_when_exists() {
    let existing = RulesFile {
        preamble: "# Custom".to_owned(),
        sections: vec![RulesSection {
            name: "Existing".to_owned(),
            content: "Do not overwrite.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_init(&root).unwrap();

    let loaded = read_rules(&root);
    assert_eq!(loaded.sections.len(), 1);
    assert_eq!(loaded.sections[0].name, "Existing");
    let _ = dir;
}

// ---------------------------------------------------------------------------
// rule_add
// ---------------------------------------------------------------------------

#[test]
fn rule_add_appends_section() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use 4 spaces.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_add(&root, "Testing", "Run cargo test.").unwrap();

    let loaded = read_rules(&root);
    assert_eq!(loaded.sections.len(), 2);
    assert_eq!(loaded.sections[1].name, "Testing");
    assert_eq!(loaded.sections[1].content, "Run cargo test.");
    let _ = dir;
}

#[test]
fn rule_add_duplicate_name_returns_error() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use 4 spaces.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    let result = super::rule::rule_add(&root, "Style", "Duplicate.");
    assert!(result.is_err());
    match result.unwrap_err() {
        LorumError::Other { message } => {
            assert!(message.contains("already exists"));
        }
        other => panic!("expected Other error, got {other:?}"),
    }

    // Original should be unchanged.
    let loaded = read_rules(&root);
    assert_eq!(loaded.sections.len(), 1);
    let _ = dir;
}

#[test]
fn rule_add_without_init_returns_error() {
    let dir = TempDir::new().unwrap();
    // No .lorum directory at all — load_rules will fail.
    let result = super::rule::rule_add(dir.path(), "Foo", "Bar");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// rule_remove
// ---------------------------------------------------------------------------

#[test]
fn rule_remove_deletes_section() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![
            RulesSection {
                name: "Style".to_owned(),
                content: "Use 4 spaces.".to_owned(),
            },
            RulesSection {
                name: "Testing".to_owned(),
                content: "Run cargo test.".to_owned(),
            },
        ],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_remove(&root, "Testing").unwrap();

    let loaded = read_rules(&root);
    assert_eq!(loaded.sections.len(), 1);
    assert_eq!(loaded.sections[0].name, "Style");
    let _ = dir;
}

#[test]
fn rule_remove_nonexistent_returns_error() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use 4 spaces.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    let result = super::rule::rule_remove(&root, "Nope");
    assert!(result.is_err());
    match result.unwrap_err() {
        LorumError::Other { message } => {
            assert!(message.contains("not found"));
        }
        other => panic!("expected Other error, got {other:?}"),
    }

    // Original should be unchanged.
    let loaded = read_rules(&root);
    assert_eq!(loaded.sections.len(), 1);
    let _ = dir;
}

// ---------------------------------------------------------------------------
// rule_edit
// ---------------------------------------------------------------------------

#[test]
fn rule_edit_replaces_content() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Old content.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_edit(&root, "Style", "New content.").unwrap();

    let loaded = read_rules(&root);
    assert_eq!(loaded.sections[0].content, "New content.");
    let _ = dir;
}

#[test]
fn rule_edit_nonexistent_returns_error() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Content.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    let result = super::rule::rule_edit(&root, "Nope", "Content.");
    assert!(result.is_err());
    match result.unwrap_err() {
        LorumError::Other { message } => {
            assert!(message.contains("not found"));
        }
        other => panic!("expected Other error, got {other:?}"),
    }

    let loaded = read_rules(&root);
    assert_eq!(loaded.sections[0].content, "Content.");
    let _ = dir;
}

// ---------------------------------------------------------------------------
// rule_list
// ---------------------------------------------------------------------------

#[test]
fn rule_list_outputs_sections() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![
            RulesSection {
                name: "Style".to_owned(),
                content: "Use 4 spaces.\nNo tabs.".to_owned(),
            },
            RulesSection {
                name: "Testing".to_owned(),
                content: "Run cargo test.".to_owned(),
            },
        ],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_list(&root).unwrap();

    let _ = dir;
}

#[test]
fn rule_list_empty_sections() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_list(&root).unwrap();

    let _ = dir;
}

// ---------------------------------------------------------------------------
// rule_show
// ---------------------------------------------------------------------------

#[test]
fn rule_show_section() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use 4 spaces.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_show(&root, Some("Style")).unwrap();

    let _ = dir;
}

#[test]
fn rule_show_all() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use 4 spaces.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    super::rule::rule_show(&root, None).unwrap();

    let _ = dir;
}

#[test]
fn rule_show_nonexistent_section_returns_error() {
    let existing = RulesFile {
        preamble: "# Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Content.".to_owned(),
        }],
    };
    let (dir, root) = setup_with_rules(&existing);

    let result = super::rule::rule_show(&root, Some("Nope"));
    assert!(result.is_err());

    let _ = dir;
}
