//! End-to-end integration tests for the rules subsystem (Phase 2).
//!
//! These tests exercise the public API to replicate the full workflows that the
//! CLI commands perform: init, add, edit, remove, sync, import, and backup.

use std::fs;

use lorum::adapters::{all_rules_adapters, find_rules_adapter};
use lorum::rules::{self, RulesFile, RulesSection};
use lorum::sync;

// ---------------------------------------------------------------------------
// T5.1 – Full workflow: init -> add -> sync -> verify tool files
// ---------------------------------------------------------------------------

#[test]
fn test_rule_full_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Step 1: rule_init — create .lorum/RULES.md with a default template.
    let init_rules = RulesFile {
        preamble: "# Project Rules\n\nThis file defines AI coding rules managed by lorum.\n\
             Each `##` heading defines a rule section that can be synced to target tools."
            .to_owned(),
        sections: vec![RulesSection {
            name: "Code Style".to_owned(),
            content: "Add your code style rules here.".to_owned(),
        }],
    };
    rules::save_rules(root, &init_rules).unwrap();
    assert!(root.join(".lorum/RULES.md").exists());

    // Step 2: rule_add("Testing", "Run cargo test") — add a section.
    let mut rules = rules::load_rules(root).unwrap();
    rules.sections.push(RulesSection {
        name: "Testing".to_owned(),
        content: "Run cargo test".to_owned(),
    });
    rules::save_rules(root, &rules).unwrap();

    // Step 3: rule_add("Style", "Use 4 spaces") — add another section.
    let mut rules = rules::load_rules(root).unwrap();
    rules.sections.push(RulesSection {
        name: "Style".to_owned(),
        content: "Use 4 spaces".to_owned(),
    });
    rules::save_rules(root, &rules).unwrap();

    // Verify 3 sections now exist.
    let rules = rules::load_rules(root).unwrap();
    assert_eq!(rules.sections.len(), 3);

    // Step 4: rule_sync(root, false, &[]) — sync to all tools.
    let content = rules::render_rules(&rules);
    let results = sync::sync_rules_all(root, &content);
    assert_eq!(results.len(), 3); // cursor, windsurf, codex
    for r in &results {
        assert!(r.success, "sync failed for {}: {:?}", r.tool, r.error);
    }

    // Step 5: verify .cursorrules exists and contains the rendered content.
    let cursor_path = root.join(".cursorrules");
    assert!(cursor_path.exists(), ".cursorrules should exist");
    assert_eq!(fs::read_to_string(&cursor_path).unwrap(), content);

    // Step 6: verify .windsurfrules exists and contains the rendered content.
    let windsurf_path = root.join(".windsurfrules");
    assert!(windsurf_path.exists(), ".windsurfrules should exist");
    assert_eq!(fs::read_to_string(&windsurf_path).unwrap(), content);

    // Step 7: verify .codex/rules.md exists and contains the rendered content.
    let codex_path = root.join(".codex").join("rules.md");
    assert!(codex_path.exists(), ".codex/rules.md should exist");
    assert_eq!(fs::read_to_string(&codex_path).unwrap(), content);
}

// ---------------------------------------------------------------------------
// T5.1 – Dry run does not create files
// ---------------------------------------------------------------------------

#[test]
fn test_rule_sync_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // rule_init
    let init_rules = RulesFile {
        preamble: "# Project Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use tabs".to_owned(),
        }],
    };
    rules::save_rules(root, &init_rules).unwrap();

    // rule_sync(root, true, &[]) — dry run
    let content = rules::render_rules(&init_rules);
    let results = sync::dry_run_rules_all(root, &content);
    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(
            r.success,
            "dry_run read failed for {}: {:?}",
            r.tool, r.error
        );
        assert!(r.needs_update, "{} should need update", r.tool);
    }

    // Verify no tool files were created.
    assert!(!root.join(".cursorrules").exists());
    assert!(!root.join(".windsurfrules").exists());
    assert!(!root.join(".codex").join("rules.md").exists());
}

// ---------------------------------------------------------------------------
// T5.1 – Filtered sync: only sync specified tools
// ---------------------------------------------------------------------------

#[test]
fn test_rule_sync_filtered_tools() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // rule_init + rule_add
    let init_rules = RulesFile {
        preamble: "# Project Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Testing".to_owned(),
            content: "Run cargo test".to_owned(),
        }],
    };
    rules::save_rules(root, &init_rules).unwrap();

    // rule_sync(root, false, &["cursor"]) — only sync cursor
    let content = rules::render_rules(&init_rules);
    let tools: Vec<String> = vec!["cursor".to_owned()];
    let results = sync::sync_rules_tools(root, &content, &tools);
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert_eq!(results[0].tool, "cursor");

    // .cursorrules should exist.
    assert!(
        root.join(".cursorrules").exists(),
        ".cursorrules should exist"
    );

    // .windsurfrules should NOT exist.
    assert!(
        !root.join(".windsurfrules").exists(),
        ".windsurfrules should not exist"
    );

    // .codex/rules.md should NOT exist.
    assert!(
        !root.join(".codex").join("rules.md").exists(),
        ".codex/rules.md should not exist"
    );
}

// ---------------------------------------------------------------------------
// T5.2 – Import from cursor
// ---------------------------------------------------------------------------

#[test]
fn test_rule_import_from_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create a .cursorrules file in the tempdir.
    let original_content = "Always use meaningful variable names.\nPrefer iterators over loops.";
    let cursor_path = root.join(".cursorrules");
    fs::write(&cursor_path, original_content).unwrap();

    // rule_import(root, "cursor") — read from cursor adapter and create RULES.md.
    let adapter = find_rules_adapter("cursor").expect("cursor adapter should exist");
    let imported_content = adapter
        .read_rules(root)
        .unwrap()
        .expect("should read .cursorrules");

    let section_name = "Imported from cursor";
    let imported_rules = RulesFile {
        preamble: "# Project Rules\n\nThis file defines AI coding rules managed by lorum.\n\
             Each `##` heading defines a rule section that can be synced to target tools."
            .to_owned(),
        sections: vec![RulesSection {
            name: section_name.to_owned(),
            content: imported_content.clone(),
        }],
    };
    rules::save_rules(root, &imported_rules).unwrap();

    // Verify .lorum/RULES.md created.
    assert!(root.join(".lorum/RULES.md").exists());

    // Verify the imported section exists with correct content.
    let loaded = rules::load_rules(root).unwrap();
    let section = loaded
        .section(section_name)
        .expect("imported section should exist");
    assert_eq!(section.content, original_content);
}

// ---------------------------------------------------------------------------
// T5.2 – Import into existing rules file appends section
// ---------------------------------------------------------------------------

#[test]
fn test_rule_import_into_existing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // rule_init + rule_add — create existing RULES.md with a section.
    let existing = RulesFile {
        preamble: "# Project Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Existing Section".to_owned(),
            content: "Do not modify.".to_owned(),
        }],
    };
    rules::save_rules(root, &existing).unwrap();

    // Create a .windsurfrules file.
    let windsurf_content = "Use TypeScript strict mode.\nNo any types.";
    let windsurf_path = root.join(".windsurfrules");
    fs::write(&windsurf_path, windsurf_content).unwrap();

    // rule_import(root, "windsurf") — import from windsurf adapter.
    let adapter = find_rules_adapter("windsurf").expect("windsurf adapter should exist");
    let imported = adapter
        .read_rules(root)
        .unwrap()
        .expect("should read .windsurfrules");

    let section_name = "Imported from windsurf";
    let mut rules = rules::load_rules(root).unwrap();
    // Remove any existing import section with the same name (re-import replaces).
    rules.sections.retain(|s| s.name != section_name);
    rules.sections.push(RulesSection {
        name: section_name.to_owned(),
        content: imported,
    });
    rules::save_rules(root, &rules).unwrap();

    // Verify the existing section is still there.
    let loaded = rules::load_rules(root).unwrap();
    assert_eq!(loaded.sections.len(), 2);
    assert_eq!(
        loaded.section("Existing Section").unwrap().content,
        "Do not modify."
    );

    // Verify the imported section was appended.
    let imported_section = loaded
        .section(section_name)
        .expect("imported section should exist");
    assert_eq!(imported_section.content, windsurf_content);
}

// ---------------------------------------------------------------------------
// T5.3 – Sync creates backup of existing files
// ---------------------------------------------------------------------------

#[test]
fn test_rule_sync_creates_backup() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create an existing .cursorrules file with old content.
    let old_content = "Old cursor rules content.";
    let cursor_path = root.join(".cursorrules");
    fs::write(&cursor_path, old_content).unwrap();

    // rule_init + rule_add
    let rules = RulesFile {
        preamble: "# Project Rules".to_owned(),
        sections: vec![RulesSection {
            name: "Style".to_owned(),
            content: "Use 4 spaces.".to_owned(),
        }],
    };
    rules::save_rules(root, &rules).unwrap();

    // rule_sync
    let content = rules::render_rules(&rules);
    let results = sync::sync_rules_all(root, &content);
    for r in &results {
        assert!(r.success, "sync failed for {}: {:?}", r.tool, r.error);
    }

    // Verify backup was created for cursor.
    let backups = lorum::backup::list_backups("cursor").unwrap();
    assert!(
        !backups.is_empty(),
        "at least one backup should exist for cursor"
    );

    // Verify the backup contains the old content.
    let latest_backup = &backups[0];
    let backup_content = fs::read_to_string(latest_backup).unwrap();
    assert_eq!(backup_content, old_content);

    // Verify the new content overwrote the old file.
    let new_content = fs::read_to_string(&cursor_path).unwrap();
    assert_eq!(new_content, content);
    assert_ne!(new_content, old_content);
}

// ---------------------------------------------------------------------------
// T5.4 – CRUD sequence: add -> add -> edit -> remove -> list
// ---------------------------------------------------------------------------

#[test]
fn test_rule_crud_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Step 1: rule_init — create empty rules file.
    let mut rules = RulesFile {
        preamble: "# Project Rules".to_owned(),
        sections: vec![],
    };
    rules::save_rules(root, &rules).unwrap();
    assert!(root.join(".lorum/RULES.md").exists());

    // Step 2: rule_add("A", "content A")
    rules.sections.push(RulesSection {
        name: "A".to_owned(),
        content: "content A".to_owned(),
    });
    rules::save_rules(root, &rules).unwrap();

    let loaded = rules::load_rules(root).unwrap();
    assert_eq!(loaded.sections.len(), 1);
    assert_eq!(loaded.sections[0].name, "A");
    assert_eq!(loaded.sections[0].content, "content A");

    // Step 3: rule_add("B", "content B")
    rules.sections.push(RulesSection {
        name: "B".to_owned(),
        content: "content B".to_owned(),
    });
    rules::save_rules(root, &rules).unwrap();

    let loaded = rules::load_rules(root).unwrap();
    assert_eq!(loaded.sections.len(), 2);
    assert_eq!(loaded.sections[0].name, "A");
    assert_eq!(loaded.sections[1].name, "B");

    // Step 4: rule_edit("A", "updated A") — update A, B stays unchanged.
    let mut loaded = rules::load_rules(root).unwrap();
    let section_a = loaded
        .sections
        .iter_mut()
        .find(|s| s.name == "A")
        .expect("section A should exist");
    section_a.content = "updated A".to_owned();
    rules::save_rules(root, &loaded).unwrap();

    let loaded = rules::load_rules(root).unwrap();
    assert_eq!(loaded.sections.len(), 2);
    assert_eq!(loaded.sections[0].content, "updated A");
    assert_eq!(loaded.sections[1].name, "B");
    assert_eq!(loaded.sections[1].content, "content B");

    // Step 5: rule_remove("B") — remove B, only A remains.
    let mut loaded = rules::load_rules(root).unwrap();
    loaded.sections.retain(|s| s.name != "B");
    rules::save_rules(root, &loaded).unwrap();

    let loaded = rules::load_rules(root).unwrap();
    assert_eq!(loaded.sections.len(), 1);
    assert_eq!(loaded.sections[0].name, "A");
    assert_eq!(loaded.sections[0].content, "updated A");

    // Step 6: rule_list — verify only "A" section remains.
    let names: Vec<&str> = loaded.sections.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["A"]);
}

// ---------------------------------------------------------------------------
// T5.1 – Verify all_rules_adapters returns the 3 registered rules adapters
// ---------------------------------------------------------------------------

#[test]
fn test_all_rules_adapters_registered() {
    let adapters = all_rules_adapters();
    assert_eq!(adapters.len(), 3);

    let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
    assert!(names.contains(&"cursor"));
    assert!(names.contains(&"windsurf"));
    assert!(names.contains(&"codex"));
}

// ---------------------------------------------------------------------------
// T5.1 – Verify find_rules_adapter works for known names
// ---------------------------------------------------------------------------

#[test]
fn test_find_rules_adapter_by_name() {
    assert!(find_rules_adapter("cursor").is_some());
    assert!(find_rules_adapter("windsurf").is_some());
    assert!(find_rules_adapter("codex").is_some());
    assert!(find_rules_adapter("nonexistent").is_none());
}

// ---------------------------------------------------------------------------
// T5.1 – Parse -> render -> parse roundtrip through file I/O
// ---------------------------------------------------------------------------

#[test]
fn test_rules_file_roundtrip_via_sync() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create a rules file.
    let original = RulesFile {
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
    rules::save_rules(root, &original).unwrap();

    // Sync to all tools.
    let content = rules::render_rules(&original);
    let results = sync::sync_rules_all(root, &content);
    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(r.success, "sync failed for {}: {:?}", r.tool, r.error);
    }

    // Read back from each adapter and verify content matches.
    for adapter in all_rules_adapters() {
        let read = adapter
            .read_rules(root)
            .unwrap()
            .expect(&format!("{} should have rules file", adapter.name()));
        assert_eq!(
            read,
            content,
            "content mismatch for adapter {}",
            adapter.name()
        );
    }
}
