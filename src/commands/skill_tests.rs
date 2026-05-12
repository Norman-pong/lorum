//! Unit tests for skill CRUD commands.

use tempfile::TempDir;

use crate::commands::skill;

/// Create a temporary skill directory under `.lorum/skills/<name>` with a SKILL.md.
fn make_skill_dir(parent: &std::path::Path, name: &str) -> std::path::PathBuf {
    let dir = parent.join(".lorum").join("skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: \"Test skill\"\n---\n# {name}\n"),
    )
    .unwrap();
    dir
}

#[test]
fn list_empty_dir_outputs_message() {
    let dir = TempDir::new().unwrap();
    skill::run_skill_list(Some(dir.path())).unwrap();
}

#[test]
fn list_outputs_skills() {
    let dir = TempDir::new().unwrap();
    make_skill_dir(dir.path(), "alpha");
    make_skill_dir(dir.path(), "beta");
    skill::run_skill_list(Some(dir.path())).unwrap();
}

#[test]
fn show_existing_skill() {
    let dir = TempDir::new().unwrap();
    make_skill_dir(dir.path(), "my-skill");
    skill::run_skill_show("my-skill", Some(dir.path())).unwrap();
}

#[test]
fn show_missing_skill_returns_error() {
    let dir = TempDir::new().unwrap();
    let result = skill::run_skill_show("missing", Some(dir.path()));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("skill not found"));
}

#[test]
fn add_imports_skill_directory() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let skill_src = make_skill_dir(src.path(), "import-me");

    skill::run_skill_add("import-me", skill_src.to_str().unwrap(), Some(dst.path())).unwrap();

    let imported = dst
        .path()
        .join(".lorum")
        .join("skills")
        .join("import-me")
        .join("SKILL.md");
    assert!(imported.exists());
}

#[test]
fn add_rejects_missing_skill_md() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let bad_dir = src.path().join("no-md");
    std::fs::create_dir_all(&bad_dir).unwrap();

    let result = skill::run_skill_add("no-md", bad_dir.to_str().unwrap(), Some(dst.path()));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("SKILL.md"));
}

#[test]
fn add_rejects_name_mismatch() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let skill_src = make_skill_dir(src.path(), "real-name");

    let result = skill::run_skill_add("wrong-name", skill_src.to_str().unwrap(), Some(dst.path()));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("mismatch"));
}

#[test]
fn remove_existing_skill() {
    let dir = TempDir::new().unwrap();
    make_skill_dir(dir.path(), "to-remove");

    skill::run_skill_remove("to-remove", Some(dir.path())).unwrap();
    assert!(
        !dir.path()
            .join(".lorum")
            .join("skills")
            .join("to-remove")
            .exists()
    );
}

#[test]
fn remove_missing_skill_returns_error() {
    let dir = TempDir::new().unwrap();
    let result = skill::run_skill_remove("missing", Some(dir.path()));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("skill not found"));
}
