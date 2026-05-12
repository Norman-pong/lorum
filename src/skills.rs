//! Skills data model, scanning, and import.
//!
//! A *skill* is a directory containing a `SKILL.md` file (Markdown with YAML
//! frontmatter) and optional auxiliary files (scripts, references, etc.).
//!
//! lorum stores unified skills under `~/.config/lorum/skills/<name>/` (global)
//! or `.lorum/skills/<name>/` (project-level).

use std::path::{Path, PathBuf};

use crate::error::LorumError;

/// Parsed YAML frontmatter from a SKILL.md file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillManifest {
    /// Skill name (must match directory name).
    pub name: String,
    /// Short description of the skill.
    pub description: String,
}

/// A fully resolved skill entry.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    /// Parsed frontmatter.
    pub manifest: SkillManifest,
    /// Full text of SKILL.md (including frontmatter).
    pub content: String,
    /// Absolute path to the skill directory.
    pub dir_path: PathBuf,
}

/// Returns the global skills directory: `~/.config/lorum/skills/`.
pub fn global_skills_dir() -> Result<PathBuf, LorumError> {
    let config_dir = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        let home = dirs::home_dir().ok_or_else(|| LorumError::Other {
            message: "cannot determine home directory".into(),
        })?;
        home.join(".config")
    };
    Ok(config_dir.join("lorum").join("skills"))
}

/// Returns the project-level skills directory: `<project_root>/.lorum/skills/`.
pub fn project_skills_dir(project_root: &Path) -> PathBuf {
    project_root.join(".lorum").join("skills")
}

/// Parse the YAML frontmatter from a SKILL.md string.
///
/// Expects the content to start with `---\n`, ending with `---\n`.
/// Returns the parsed manifest and the full raw content.
pub fn parse_skill_manifest(content: &str, path: &Path) -> Result<SkillManifest, LorumError> {
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| LorumError::Other {
            message: format!(
                "{} must start with '---' frontmatter delimiter",
                path.display()
            ),
        })?;

    // Trim a single leading newline after the opening ---.
    let rest = rest.strip_prefix('\n').unwrap_or(rest);

    let end = rest.find("\n---").ok_or_else(|| LorumError::Other {
        message: format!(
            "{} frontmatter must be closed with '---'",
            path.display()
        ),
    })?;

    let yaml_str = &rest[..end];
    serde_yaml::from_str(yaml_str).map_err(|e| LorumError::ConfigParse {
        format: "yaml".into(),
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

/// Scan a skills directory for all subdirectories containing a SKILL.md.
///
/// Returns entries sorted by name. Directories without SKILL.md are skipped.
pub fn scan_skills_dir(dir: &Path) -> Result<Vec<SkillEntry>, LorumError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<SkillEntry> = Vec::new();
    let read_dir = std::fs::read_dir(dir).map_err(|e| LorumError::Io { source: e })?;

    for entry in read_dir {
        let entry = entry.map_err(|e| LorumError::Io { source: e })?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&skill_md)?;
        let manifest = parse_skill_manifest(&content, &skill_md)?;

        entries.push(SkillEntry {
            manifest,
            content,
            dir_path: path,
        });
    }

    entries.sort_by(|a, b| {
        let a_name = a.dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.dir_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.cmp(b_name)
    });
    Ok(entries)
}

/// Recursively copy a directory tree from `src` to `dst`.
///
/// Creates `dst` if it does not exist. Existing files are overwritten.
/// Symlinks are skipped to avoid infinite recursion.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), LorumError> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src).map_err(|e| LorumError::Io { source: e })? {
        let entry = entry.map_err(|e| LorumError::Io { source: e })?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let metadata = std::fs::symlink_metadata(&src_path)
            .map_err(|e| LorumError::Io { source: e })?;

        if metadata.file_type().is_symlink() {
            continue; // Skip symlinks to avoid infinite recursion
        }

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
            // Also preserve permissions
            let perms = metadata.permissions();
            std::fs::set_permissions(&dst_path, perms)
                .map_err(|e| LorumError::Io { source: e })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_frontmatter() {
        let content =
            "---\nname: my-skill\ndescription: \"A test skill\"\n---\n# My Skill\nBody here.\n";
        let manifest = parse_skill_manifest(content, Path::new("SKILL.md")).unwrap();
        assert_eq!(manifest.name, "my-skill");
        assert_eq!(manifest.description, "A test skill");
    }

    #[test]
    fn parse_frontmatter_missing_open() {
        let content = "name: my-skill\n---\nBody\n";
        assert!(parse_skill_manifest(content, Path::new("SKILL.md")).is_err());
    }

    #[test]
    fn parse_frontmatter_missing_close() {
        let content = "---\nname: my-skill\nBody\n";
        assert!(parse_skill_manifest(content, Path::new("SKILL.md")).is_err());
    }

    #[test]
    fn scan_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let skills = scan_skills_dir(dir.path()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn scan_dir_with_skills() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: \"Test\"\n---\n# Body\n",
        )
        .unwrap();

        let skills = scan_skills_dir(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].manifest.name, "my-skill");
        assert_eq!(skills[0].dir_path, skill_dir);
    }

    #[test]
    fn scan_dir_skips_non_skill_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("no-skill-md");
        std::fs::create_dir_all(&sub).unwrap();
        // No SKILL.md in this subdirectory.

        let skills = scan_skills_dir(dir.path()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn scan_nonexistent_dir_returns_empty() {
        let skills = scan_skills_dir(Path::new("/tmp/no-such-lorum-dir-xyz")).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn copy_dir_recursive_copies_all_files() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        std::fs::write(src.path().join("SKILL.md"), "---\nname: t\n---\n").unwrap();
        std::fs::create_dir_all(src.path().join("scripts")).unwrap();
        std::fs::write(src.path().join("scripts/run.sh"), "#!/bin/sh\necho hi\n").unwrap();

        let dst_skill = dst.path().join("t");
        copy_dir_recursive(src.path(), &dst_skill).unwrap();

        assert!(dst_skill.join("SKILL.md").exists());
        assert!(dst_skill.join("scripts/run.sh").exists());
        let content = std::fs::read_to_string(dst_skill.join("scripts/run.sh")).unwrap();
        assert_eq!(content, "#!/bin/sh\necho hi\n");
    }

    #[test]
    #[cfg(unix)]
    fn copy_dir_recursive_skips_symlinks() {
        use std::os::unix::fs::symlink;

        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        std::fs::write(src.path().join("SKILL.md"), "---\nname: t\n---\n").unwrap();
        // Create a symlink pointing back to the parent directory (would cause infinite recursion)
        symlink(src.path(), src.path().join("loop")).unwrap();

        let dst_skill = dst.path().join("t");
        copy_dir_recursive(src.path(), &dst_skill).unwrap();

        assert!(dst_skill.join("SKILL.md").exists());
        assert!(!dst_skill.join("loop").exists());
    }
}
