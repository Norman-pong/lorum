//! Doctor command: comprehensive health check for tool configurations.
//!
//! The doctor command validates configuration files across all registered
//! tools and reports issues. It can also check consistency between the
//! unified lorum configuration and individual tool configurations.

use std::collections::BTreeSet;

use crate::adapters::{
    ConfigValidator, Severity, ValidationIssue, all_config_validators, find_config_validator,
};
use crate::error::LorumError;
use crate::sync::ConfigDiff;

/// Result of running the doctor check on a single tool.
#[derive(Debug, Clone, PartialEq)]
pub struct DoctorResult {
    /// Name of the tool that was checked.
    pub tool: String,
    /// Whether the tool's configuration is healthy (no errors).
    pub healthy: bool,
    /// All validation issues found for this tool.
    pub issues: Vec<ValidationIssue>,
}

/// Report of a consistency check between the unified config and a tool's config.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsistencyReport {
    /// Name of the tool that was checked.
    pub tool: String,
    /// Whether the tool's config is consistent with the unified config.
    pub consistent: bool,
    /// Diff showing drift, if any.
    pub diff: Option<ConfigDiff>,
    /// Error message if the consistency check could not be performed.
    pub error: Option<String>,
}

/// Run the doctor command on the specified tools.
///
/// If `tools` is empty, all registered tools are checked.
///
/// Returns a [`DoctorResult`] for each tool checked.
pub fn run_doctor(tools: &[String]) -> Result<Vec<DoctorResult>, LorumError> {
    let validators: Vec<&dyn ConfigValidator> = if tools.is_empty() {
        all_config_validators().iter().map(|v| v.as_ref()).collect()
    } else {
        let mut v = Vec::new();
        for name in tools {
            if let Some(validator) = find_config_validator(name) {
                v.push(validator);
            } else {
                // Unknown tool — report as an error result
                return Err(LorumError::AdapterNotFound { name: name.clone() });
            }
        }
        v
    };

    let mut results = Vec::new();
    for validator in validators {
        let tool = validator.name().to_string();
        let issues = validator.validate_config()?;
        let has_errors = issues.iter().any(|i| i.severity == Severity::Error);

        results.push(DoctorResult {
            tool,
            healthy: !has_errors,
            issues,
        });
    }

    Ok(results)
}

/// Print doctor results in a table format.
///
/// Shows tool name, health status, issue counts by severity, and a summary.
/// If no issues are found across all tools, prints "all clear".
pub fn print_doctor_results(results: &[DoctorResult]) {
    if results.is_empty() {
        println!("no tools to check");
        return;
    }

    let total_issues: usize = results.iter().map(|r| r.issues.len()).sum();
    if total_issues == 0 {
        println!("all clear — no issues found in {} tool(s)", results.len());
        return;
    }

    // Header
    println!(
        "{:<15} {:>8} {:>8} {:>8}",
        "TOOL", "STATUS", "ERRORS", "WARNINGS"
    );

    for result in results {
        let status = if result.healthy { "OK" } else { "FAIL" };
        let errors = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warnings = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        println!(
            "{:<15} {:>8} {:>8} {:>8}",
            result.tool, status, errors, warnings,
        );

        // Print each issue with indentation
        for issue in &result.issues {
            let severity_label = match issue.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            if let Some(ref path) = issue.path {
                println!(
                    "  {}: {} ({})",
                    severity_label,
                    issue.message,
                    path.display()
                );
            } else {
                println!("  {}: {}", severity_label, issue.message);
            }
        }
    }

    let total_errors: usize = results
        .iter()
        .map(|r| {
            r.issues
                .iter()
                .filter(|i| i.severity == Severity::Error)
                .count()
        })
        .sum();
    let total_warnings: usize = results
        .iter()
        .map(|r| {
            r.issues
                .iter()
                .filter(|i| i.severity == Severity::Warning)
                .count()
        })
        .sum();

    println!();
    println!(
        "summary: {} error(s), {} warning(s) across {} tool(s)",
        total_errors,
        total_warnings,
        results.len()
    );
}

/// Print consistency check results.
///
/// Shows each tool's consistency status. Consistent tools are marked as
/// "consistent"; drifted tools show the count of added and removed servers.
pub fn print_consistency_reports(reports: &[ConsistencyReport]) {
    if reports.is_empty() {
        println!("no tools to check");
        return;
    }

    let all_consistent = reports.iter().all(|r| r.consistent);
    if all_consistent && reports.iter().all(|r| r.error.is_none()) {
        println!("all clear — all {} tool(s) are consistent", reports.len());
        return;
    }

    for report in reports {
        if let Some(ref error) = report.error {
            println!("{}: error — {}", report.tool, error);
        } else if report.consistent {
            println!("{}: consistent", report.tool);
        } else if let Some(ref diff) = report.diff {
            println!(
                "{}: drift +{} -{}",
                report.tool,
                diff.added.len(),
                diff.removed.len()
            );
        } else {
            println!("{}: drift", report.tool);
        }
    }
}

/// Run a consistency check between the unified lorum config and tool configs.
///
/// If `tools` is empty, all registered MCP adapters are checked.
///
/// Returns a [`ConsistencyReport`] for each tool checked.
pub fn run_doctor_consistency(tools: &[String]) -> Result<Vec<ConsistencyReport>, LorumError> {
    use crate::adapters::{all_adapters, find_adapter};
    use crate::config;

    let adapters: Vec<&dyn crate::adapters::ToolAdapter> = if tools.is_empty() {
        all_adapters().iter().map(|a| a.as_ref()).collect()
    } else {
        let mut a = Vec::new();
        for name in tools {
            if let Some(adapter) = find_adapter(name) {
                a.push(adapter);
            } else {
                return Err(LorumError::AdapterNotFound { name: name.clone() });
            }
        }
        a
    };

    let unified = config::resolve_effective_config_from_cwd(None)?;
    let unified_servers: BTreeSet<String> = unified.mcp.servers.keys().cloned().collect();

    let mut reports = Vec::new();
    for adapter in adapters {
        let tool = adapter.name().to_string();
        match adapter.read_mcp() {
            Ok(current) => {
                let current_servers: BTreeSet<String> = current.servers.keys().cloned().collect();

                // Use symmetric_difference to find servers that differ
                let diff: Vec<String> = unified_servers
                    .symmetric_difference(&current_servers)
                    .cloned()
                    .collect();

                let consistent = diff.is_empty();

                // Build a ConfigDiff for detailed reporting
                let added: Vec<String> = unified_servers
                    .difference(&current_servers)
                    .cloned()
                    .collect();
                let removed: Vec<String> = current_servers
                    .difference(&unified_servers)
                    .cloned()
                    .collect();

                // Check for modified servers (present in both but different)
                let mut modified = Vec::new();
                for name in current_servers.intersection(&unified_servers) {
                    if current.servers.get(name) != unified.mcp.servers.get(name) {
                        modified.push(name.clone());
                    }
                }

                let config_diff = ConfigDiff {
                    added,
                    removed,
                    modified,
                    unchanged: Vec::new(),
                };

                reports.push(ConsistencyReport {
                    tool,
                    consistent: consistent && config_diff.modified.is_empty(),
                    diff: Some(config_diff),
                    error: None,
                });
            }
            Err(e) => {
                reports.push(ConsistencyReport {
                    tool,
                    consistent: false,
                    diff: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{ConfigValidator, Severity, ValidationIssue};
    use crate::error::LorumError;

    /// A mock validator for testing doctor behaviour without touching real adapters.
    struct MockValidator {
        name: &'static str,
        issues: Vec<ValidationIssue>,
    }

    impl ConfigValidator for MockValidator {
        fn name(&self) -> &str {
            self.name
        }

        fn validate_config(&self) -> Result<Vec<ValidationIssue>, LorumError> {
            Ok(self.issues.clone())
        }
    }

    #[test]
    fn test_doctor_runs_all_validators() {
        // When tools list is empty, run_doctor should iterate all registered validators.
        let results = run_doctor(&[]).unwrap();
        assert_eq!(results.len(), 9, "expected 9 registered validators");

        let names: Vec<&str> = results.iter().map(|r| r.tool.as_str()).collect();
        assert!(names.contains(&"claude-code"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"continue"));
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"proma"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"opencode"));
        assert!(names.contains(&"trae"));
        assert!(names.contains(&"windsurf"));
    }

    #[test]
    fn test_doctor_filters_by_tools() {
        let results = run_doctor(&["cursor".into(), "kimi".into()]).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].tool, "cursor");
        assert_eq!(results[1].tool, "kimi");
    }

    #[test]
    fn test_doctor_reports_no_issues_for_valid_configs() {
        // Most real adapters return no issues when their config files don't exist.
        let results = run_doctor(&[]).unwrap();
        for result in &results {
            // No errors should be present (warnings are okay, but typically there are none).
            let has_errors = result.issues.iter().any(|i| i.severity == Severity::Error);
            assert!(
                !has_errors,
                "tool '{}' should have no errors when config files are absent",
                result.tool
            );
        }
    }

    #[test]
    fn test_doctor_reports_issues_for_broken_configs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, r#"{"broken": json}"#).unwrap();

        let validator = MockValidator {
            name: "test-tool",
            issues: vec![ValidationIssue {
                severity: Severity::Error,
                message: "invalid JSON".into(),
                path: Some(path),
                line: None,
            }],
        };

        let tool = validator.name().to_string();
        let issues = validator.validate_config().unwrap();
        let has_errors = issues.iter().any(|i| i.severity == Severity::Error);
        assert!(has_errors);
        assert_eq!(tool, "test-tool");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn test_doctor_invalid_tool_name() {
        let result = run_doctor(&["nonexistent-tool-xyz".into()]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nonexistent-tool-xyz"));
    }

    #[test]
    #[serial_test::serial]
    fn test_consistency_reports_consistent_when_synced() {
        let dir = tempfile::tempdir().unwrap();
        let original_home = std::env::var_os("HOME");
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let original_cwd = std::env::current_dir().unwrap();

        let result = std::panic::catch_unwind(|| {
            // Set HOME to temp dir so global config resolves to temp/.config/lorum/config.yaml
            unsafe {
                std::env::set_var("HOME", dir.path());
            }
            // Clear XDG_CONFIG_HOME so HOME is used
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }

            // Create empty global lorum config
            let lorum_dir = dir.path().join(".config").join("lorum");
            std::fs::create_dir_all(&lorum_dir).unwrap();
            std::fs::write(lorum_dir.join("config.yaml"), "mcp:\n  servers: {}\n").unwrap();

            // Change cwd to a subdir (no .lorum/config.yaml, no .cursor/mcp.json)
            let cwd_dir = dir.path().join("workspace");
            std::fs::create_dir_all(&cwd_dir).unwrap();
            std::env::set_current_dir(&cwd_dir).unwrap();

            let reports = run_doctor_consistency(&["cursor".into()]).unwrap();
            assert_eq!(reports.len(), 1);
            let report = &reports[0];
            assert_eq!(report.tool, "cursor");
            assert!(
                report.consistent,
                "expected consistent when both configs are empty"
            );
            assert!(report.error.is_none());
            let diff = report.diff.as_ref().expect("diff should be present");
            assert!(diff.added.is_empty(), "expected no added servers");
            assert!(diff.removed.is_empty(), "expected no removed servers");
            assert!(diff.modified.is_empty(), "expected no modified servers");
        });

        // Restore environment
        unsafe {
            match original_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
        match original_xdg {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        std::env::set_current_dir(original_cwd).unwrap();

        assert!(result.is_ok());
    }

    #[test]
    #[serial_test::serial]
    fn test_consistency_detects_drift() {
        let dir = tempfile::tempdir().unwrap();
        let original_home = std::env::var_os("HOME");
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let original_cwd = std::env::current_dir().unwrap();

        let result = std::panic::catch_unwind(|| {
            // Set HOME to temp dir so global config resolves to temp/.config/lorum/config.yaml
            unsafe {
                std::env::set_var("HOME", dir.path());
            }
            // Clear XDG_CONFIG_HOME so HOME is used
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }

            // Create global lorum config with test-srv
            let lorum_dir = dir.path().join(".config").join("lorum");
            std::fs::create_dir_all(&lorum_dir).unwrap();
            std::fs::write(
                lorum_dir.join("config.yaml"),
                "mcp:\n  servers:\n    test-srv:\n      command: echo\n      args: []\n      env: {}\n",
            )
            .unwrap();

            // Change cwd to a subdir
            let cwd_dir = dir.path().join("workspace");
            std::fs::create_dir_all(&cwd_dir).unwrap();
            std::env::set_current_dir(&cwd_dir).unwrap();

            // Create cursor config with a different server
            let cursor_dir = cwd_dir.join(".cursor");
            std::fs::create_dir_all(&cursor_dir).unwrap();
            std::fs::write(
                cursor_dir.join("mcp.json"),
                r#"{"mcpServers":{"other-srv":{"command":"node","args":["server.js"]}}}"#,
            )
            .unwrap();

            let reports = run_doctor_consistency(&["cursor".into()]).unwrap();
            assert_eq!(reports.len(), 1);
            let report = &reports[0];
            assert_eq!(report.tool, "cursor");
            assert!(!report.consistent, "expected drift to be detected");
            assert!(report.error.is_none());
            let diff = report.diff.as_ref().expect("diff should be present");
            assert_eq!(
                diff.added,
                vec!["test-srv"],
                "expected test-srv to be added"
            );
            assert_eq!(
                diff.removed,
                vec!["other-srv"],
                "expected other-srv to be removed"
            );
            assert!(diff.modified.is_empty(), "expected no modified servers");
        });

        // Restore environment
        unsafe {
            match original_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
        match original_xdg {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        std::env::set_current_dir(original_cwd).unwrap();

        assert!(result.is_ok());
    }
}
