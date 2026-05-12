//! Cursor rules adapter for reading/writing rules files.
//!
//! Rules file: `{project_root}/.cursorrules`

use std::path::{Path, PathBuf};

use crate::adapters::RulesAdapter;
use crate::error::LorumError;

/// Adapter for Cursor.
///
/// Reads and writes rules content from Cursor's `.cursorrules` file
/// located at the project root.
pub struct CursorRulesAdapter;

impl RulesAdapter for CursorRulesAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn rules_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".cursorrules")
    }

    fn read_rules(&self, project_root: &Path) -> Result<Option<String>, LorumError> {
        let path = self.rules_path(project_root);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(Some(content))
    }

    fn write_rules(&self, project_root: &Path, content: &str) -> Result<(), LorumError> {
        let path = self.rules_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| LorumError::ConfigWrite {
                path: path.clone(),
                source: e,
            })?;
        }
        std::fs::write(&path, content).map_err(|e| LorumError::ConfigWrite { path, source: e })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_path_returns_cursorrules() {
        let adapter = CursorRulesAdapter;
        let path = adapter.rules_path(Path::new("/tmp/myproject"));
        assert_eq!(path, PathBuf::from("/tmp/myproject/.cursorrules"));
    }

    #[test]
    fn read_rules_returns_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorRulesAdapter;
        let result = adapter.read_rules(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_rules_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorRulesAdapter;
        let path = adapter.rules_path(dir.path());
        assert!(!path.exists());

        adapter
            .write_rules(dir.path(), "Use 4-space indentation.")
            .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = CursorRulesAdapter;
        let content = "## Style\nUse 4-space indentation.\n";

        adapter.write_rules(dir.path(), content).unwrap();
        let read = adapter.read_rules(dir.path()).unwrap();
        assert_eq!(read, Some(content.to_owned()));
    }

    #[test]
    fn adapter_name() {
        let adapter = CursorRulesAdapter;
        assert_eq!(adapter.name(), "cursor");
    }
}
