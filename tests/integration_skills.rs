//! End-to-end integration tests for skills lifecycle.

use std::path::Path;

use lorum::skills::{copy_dir_recursive, scan_skills_dir};
use lorum::sync::sync_skills_tools;

fn make_skill_dir(parent: &Path, name: &str) -> std::path::PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: \"Test skill\"\n---\n# {name}\n"),
    )
    .unwrap();
    dir
}

#[test]
fn scan_skills_finds_all_directories() {
    let dir = tempfile::tempdir().unwrap();
    make_skill_dir(dir.path(), "alpha");
    make_skill_dir(dir.path(), "beta");

    let skills = scan_skills_dir(dir.path()).unwrap();
    assert_eq!(skills.len(), 2);
    assert_eq!(skills[0].manifest.name, "alpha");
    assert_eq!(skills[1].manifest.name, "beta");
}

#[test]
fn copy_dir_recursive_preserves_structure() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();

    make_skill_dir(src.path(), "test-skill");
    std::fs::create_dir_all(src.path().join("test-skill").join("scripts")).unwrap();
    std::fs::write(
        src.path().join("test-skill").join("scripts").join("run.sh"),
        "#!/bin/sh\n",
    )
    .unwrap();

    let target = dst.path().join("test-skill");
    copy_dir_recursive(src.path().join("test-skill").as_path(), &target).unwrap();

    assert!(target.join("SKILL.md").exists());
    assert!(target.join("scripts/run.sh").exists());
}

#[test]
fn sync_skills_tools_reports_unknown_adapter() {
    let dir = tempfile::tempdir().unwrap();
    make_skill_dir(dir.path(), "test-skill");

    let results = sync_skills_tools(dir.path(), &["nonexistent-tool".into()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("adapter not found")
    );
}

#[test]
fn sync_skills_to_mock_adapter_via_temp_dir() {
    let src = tempfile::tempdir().unwrap();
    make_skill_dir(src.path(), "my-skill");

    // Use a temp dir as the mock target and manually verify copy works.
    let dst = tempfile::tempdir().unwrap();
    let target = dst.path().join("my-skill");
    copy_dir_recursive(src.path().join("my-skill").as_path(), &target).unwrap();

    assert!(target.join("SKILL.md").exists());
    let skills = scan_skills_dir(dst.path()).unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].manifest.name, "my-skill");
}

#[test]
fn skills_manifest_parses_frontmatter_correctly() {
    let dir = tempfile::tempdir().unwrap();
    make_skill_dir(dir.path(), "parse-test");

    let skills = scan_skills_dir(dir.path()).unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].manifest.name, "parse-test");
    assert_eq!(skills[0].manifest.description, "Test skill");
}
