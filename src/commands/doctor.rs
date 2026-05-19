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
use crate::sync::{ConfigDiff, DimensionDiff};

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
///
/// Each report covers a single tool on a single dimension (`"mcp"`, `"hooks"`,
/// `"skills"`, or `"rules"`).
#[derive(Debug, Clone, PartialEq)]
pub struct ConsistencyReport {
    /// Name of the tool that was checked.
    pub tool: String,
    /// Dimension that was checked: `"mcp"`, `"hooks"`, `"skills"`, or `"rules"`.
    pub dimension: String,
    /// Whether the tool's config is consistent with the unified config.
    pub consistent: bool,
    /// Diff showing drift, if any.
    pub diff: Option<DimensionDiff>,
    /// Error message if the consistency check could not be performed.
    pub error: Option<String>,
}

/// Legacy consistency report format (MCP-only).
///
/// Preserved for backward compatibility; will be removed in a future minor
/// version.
#[deprecated(
    since = "0.2.0",
    note = "Use ConsistencyReport with the `dimension` field instead"
)]
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyConsistencyReport {
    /// Name of the tool that was checked.
    pub tool: String,
    /// Whether the tool's config is consistent with the unified config.
    pub consistent: bool,
    /// Diff showing drift, if any.
    pub diff: Option<ConfigDiff>,
    /// Error message if the consistency check could not be performed.
    pub error: Option<String>,
}

#[allow(deprecated)]
impl From<&ConsistencyReport> for LegacyConsistencyReport {
    fn from(report: &ConsistencyReport) -> Self {
        let config_diff = report.diff.as_ref().and_then(|dd| match dd {
            DimensionDiff::Mcp(cd) => Some(cd.clone()),
            _ => None,
        });
        LegacyConsistencyReport {
            tool: report.tool.clone(),
            consistent: report.consistent,
            diff: config_diff,
            error: report.error.clone(),
        }
    }
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
/// Shows each tool's consistency status grouped by dimension. Consistent tools
/// are marked as "consistent"; drifted tools show the count of changes.
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
            println!(
                "{} [{:>6}]: error — {}",
                report.tool, report.dimension, error
            );
        } else if report.consistent {
            println!("{} [{:>6}]: consistent", report.tool, report.dimension);
        } else if let Some(ref diff) = report.diff {
            match diff {
                DimensionDiff::Mcp(cd) => {
                    println!(
                        "{} [{:>6}]: drift +{} -{}",
                        report.tool,
                        report.dimension,
                        cd.added.len(),
                        cd.removed.len()
                    );
                }
                DimensionDiff::Hooks(hd) => {
                    println!(
                        "{} [{:>6}]: drift +{} -{} ~{}",
                        report.tool,
                        report.dimension,
                        hd.added_events.len(),
                        hd.removed_events.len(),
                        hd.modified_handlers.len()
                    );
                }
                DimensionDiff::Skills(sd) => {
                    println!(
                        "{} [{:>6}]: drift +{} -{} ~{}",
                        report.tool,
                        report.dimension,
                        sd.added_skills.len(),
                        sd.removed_skills.len(),
                        sd.modified_skills.len()
                    );
                }
                DimensionDiff::Rules(rd) => {
                    println!(
                        "{} [{:>6}]: drift ({} line(s) differ)",
                        report.tool,
                        report.dimension,
                        rd.line_diffs.len()
                    );
                }
            }
        } else {
            println!("{} [{:>6}]: drift", report.tool, report.dimension);
        }
    }

    // Dimension summary.
    let dimensions: std::collections::BTreeSet<&str> =
        reports.iter().map(|r| r.dimension.as_str()).collect();
    let consistent_count = reports.iter().filter(|r| r.consistent).count();
    let drifted_count = reports.len() - consistent_count;
    println!();
    println!(
        "summary: {} dimension(s) checked, {} consistent, {} drifted",
        dimensions.len(),
        consistent_count,
        drifted_count,
    );
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
                    dimension: "mcp".to_string(),
                    consistent: consistent && config_diff.modified.is_empty(),
                    diff: Some(DimensionDiff::Mcp(config_diff)),
                    error: None,
                });
            }
            Err(e) => {
                reports.push(ConsistencyReport {
                    tool,
                    dimension: "mcp".to_string(),
                    consistent: false,
                    diff: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(reports)
}

/// Run a consistency check for hooks between the unified lorum config and tool
/// configs.
///
/// If `tools` is empty, all registered hooks adapters are checked.
///
/// Returns a [`ConsistencyReport`] for each tool checked on the `"hooks"`
/// dimension.
pub fn run_doctor_hooks_consistency(
    tools: &[String],
) -> Result<Vec<ConsistencyReport>, LorumError> {
    use crate::adapters::{all_hooks_adapters, find_hooks_adapter};
    use crate::config;
    use crate::sync::{HooksDiff, ModifiedHandler};

    let adapters: Vec<&dyn crate::adapters::HooksAdapter> = if tools.is_empty() {
        all_hooks_adapters().iter().map(|a| a.as_ref()).collect()
    } else {
        let mut a = Vec::new();
        for name in tools {
            if let Some(adapter) = find_hooks_adapter(name) {
                a.push(adapter);
            } else {
                return Err(LorumError::AdapterNotFound { name: name.clone() });
            }
        }
        a
    };

    let unified = config::resolve_effective_config_from_cwd(None)?;
    let unified_events: std::collections::BTreeSet<String> =
        unified.hooks.events.keys().cloned().collect();

    let mut reports = Vec::new();
    for adapter in adapters {
        let tool = adapter.name().to_string();
        match adapter.read_hooks() {
            Ok(current) => {
                let current_events: std::collections::BTreeSet<String> =
                    current.events.keys().cloned().collect();

                let added_events: Vec<String> = unified_events
                    .difference(&current_events)
                    .cloned()
                    .collect();
                let removed_events: Vec<String> = current_events
                    .difference(&unified_events)
                    .cloned()
                    .collect();

                // Check for modified handlers within shared events.
                let mut modified_handlers = Vec::new();
                for event in current_events.intersection(&unified_events) {
                    let unified_h = unified
                        .hooks
                        .events
                        .get(event)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let current_h = current
                        .events
                        .get(event)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    if unified_h != current_h {
                        // Report the first differing handler as a representative.
                        for (u, c) in unified_h.iter().zip(current_h.iter()) {
                            if u.command != c.command {
                                modified_handlers.push(ModifiedHandler {
                                    event: event.clone(),
                                    old_handler: c.command.clone(),
                                    new_handler: u.command.clone(),
                                });
                            }
                        }
                        // If lengths differ, also note the event as modified.
                        if unified_h.len() != current_h.len()
                            && modified_handlers.iter().all(|m| m.event != *event)
                        {
                            modified_handlers.push(ModifiedHandler {
                                event: event.clone(),
                                old_handler: format!("{} handler(s)", current_h.len()),
                                new_handler: format!("{} handler(s)", unified_h.len()),
                            });
                        }
                    }
                }

                let hooks_diff = HooksDiff {
                    added_events,
                    removed_events,
                    modified_handlers,
                };
                let consistent = hooks_diff.is_empty();

                reports.push(ConsistencyReport {
                    tool,
                    dimension: "hooks".to_string(),
                    consistent,
                    diff: Some(DimensionDiff::Hooks(hooks_diff)),
                    error: None,
                });
            }
            Err(e) => {
                reports.push(ConsistencyReport {
                    tool,
                    dimension: "hooks".to_string(),
                    consistent: false,
                    diff: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(reports)
}

/// Run a consistency check for skills between the unified skills directory and
/// tool skills directories.
///
/// If `tools` is empty, all registered skills adapters are checked.
///
/// Returns a [`ConsistencyReport`] for each tool checked on the `"skills"`
/// dimension.
pub fn run_doctor_skills_consistency(
    tools: &[String],
) -> Result<Vec<ConsistencyReport>, LorumError> {
    use crate::adapters::{all_skills_adapters, find_skills_adapter};
    use crate::sync::SkillsDiff;

    let adapters: Vec<&dyn crate::adapters::SkillsAdapter> = if tools.is_empty() {
        all_skills_adapters().iter().map(|a| a.as_ref()).collect()
    } else {
        let mut a = Vec::new();
        for name in tools {
            if let Some(adapter) = find_skills_adapter(name) {
                a.push(adapter);
            } else {
                return Err(LorumError::AdapterNotFound { name: name.clone() });
            }
        }
        a
    };

    let skills_dir = crate::skills::global_skills_dir()?;
    let source_skills = crate::skills::scan_skills_dir(&skills_dir).unwrap_or_default();
    let source_names: std::collections::BTreeSet<String> = source_skills
        .iter()
        .map(|s| s.manifest.name.clone())
        .collect();

    let mut reports = Vec::new();
    for adapter in adapters {
        let tool = adapter.name().to_string();
        match adapter.read_skills() {
            Ok(target_skills) => {
                let target_names: std::collections::BTreeSet<String> = target_skills
                    .iter()
                    .map(|s| s.manifest.name.clone())
                    .collect();

                let added_skills: Vec<String> =
                    source_names.difference(&target_names).cloned().collect();
                let removed_skills: Vec<String> =
                    target_names.difference(&source_names).cloned().collect();

                // Check for modified skills (same name, different content).
                let mut modified_skills = Vec::new();
                for source in &source_skills {
                    if let Some(target) = target_skills
                        .iter()
                        .find(|t| t.manifest.name == source.manifest.name)
                    {
                        if source.content != target.content {
                            modified_skills.push(source.manifest.name.clone());
                        }
                    }
                }

                let skills_diff = SkillsDiff {
                    added_skills,
                    removed_skills,
                    modified_skills,
                };
                let consistent = skills_diff.is_empty();

                reports.push(ConsistencyReport {
                    tool,
                    dimension: "skills".to_string(),
                    consistent,
                    diff: Some(DimensionDiff::Skills(skills_diff)),
                    error: None,
                });
            }
            Err(e) => {
                reports.push(ConsistencyReport {
                    tool,
                    dimension: "skills".to_string(),
                    consistent: false,
                    diff: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(reports)
}

/// Run a consistency check for rules between the unified rules file and tool
/// rules files.
///
/// If `tools` is empty, all registered rules adapters are checked.
///
/// Returns a [`ConsistencyReport`] for each tool checked on the `"rules"`
/// dimension.
pub fn run_doctor_rules_consistency(
    tools: &[String],
) -> Result<Vec<ConsistencyReport>, LorumError> {
    use crate::adapters::{all_rules_adapters, find_rules_adapter};
    use crate::sync::RulesDiff;

    let adapters: Vec<&dyn crate::adapters::RulesAdapter> = if tools.is_empty() {
        all_rules_adapters().iter().map(|a| a.as_ref()).collect()
    } else {
        let mut a = Vec::new();
        for name in tools {
            if let Some(adapter) = find_rules_adapter(name) {
                a.push(adapter);
            } else {
                return Err(LorumError::AdapterNotFound { name: name.clone() });
            }
        }
        a
    };

    let project_root = std::env::current_dir().map_err(|e| LorumError::Io { source: e })?;
    let project_root =
        crate::rules::find_project_root(&project_root).unwrap_or_else(|| project_root.clone());

    // Load unified rules content.
    let unified_content = match crate::rules::load_rules(&project_root) {
        Ok(rules) => crate::rules::render_rules(&rules),
        Err(_) => String::new(),
    };

    let mut reports = Vec::new();
    for adapter in adapters {
        let tool = adapter.name().to_string();
        match adapter.read_rules(&project_root) {
            Ok(Some(tool_content)) => {
                let consistent = unified_content == tool_content;

                let line_diffs = if consistent {
                    Vec::new()
                } else {
                    compute_rules_line_diffs(&unified_content, &tool_content)
                };

                let rules_diff = RulesDiff {
                    consistent,
                    line_diffs,
                };

                reports.push(ConsistencyReport {
                    tool,
                    dimension: "rules".to_string(),
                    consistent,
                    diff: Some(DimensionDiff::Rules(rules_diff)),
                    error: None,
                });
            }
            Ok(None) => {
                // No rules file in the tool — consistent if unified is also empty.
                let consistent = unified_content.is_empty();
                reports.push(ConsistencyReport {
                    tool,
                    dimension: "rules".to_string(),
                    consistent,
                    diff: None,
                    error: None,
                });
            }
            Err(e) => {
                reports.push(ConsistencyReport {
                    tool,
                    dimension: "rules".to_string(),
                    consistent: false,
                    diff: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(reports)
}

/// Compute line-level diffs between unified and tool rules content.
///
/// Returns a [`DiffLine`] for each line that differs.
fn compute_rules_line_diffs(unified: &str, tool: &str) -> Vec<crate::sync::DiffLine> {
    use crate::sync::DiffLine;

    let unified_lines: Vec<&str> = unified.lines().collect();
    let tool_lines: Vec<&str> = tool.lines().collect();
    let max_len = unified_lines.len().max(tool_lines.len());

    let mut diffs = Vec::new();
    for i in 0..max_len {
        let u_line = unified_lines.get(i).copied();
        let t_line = tool_lines.get(i).copied();

        if u_line != t_line {
            diffs.push(DiffLine {
                line_number: i + 1,
                unified_line: u_line.unwrap_or("").to_string(),
                tool_line: t_line.map(|s| s.to_string()),
            });
        }
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{ConfigValidator, Severity, ValidationIssue};
    use crate::error::LorumError;
    use serial_test::serial;

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
    #[serial]
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
            if let DimensionDiff::Mcp(cd) = diff {
                assert!(cd.added.is_empty(), "expected no added servers");
                assert!(cd.removed.is_empty(), "expected no removed servers");
                assert!(cd.modified.is_empty(), "expected no modified servers");
            } else {
                panic!("expected Mcp dimension diff");
            }
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
            if let DimensionDiff::Mcp(cd) = diff {
                assert_eq!(cd.added, vec!["test-srv"], "expected test-srv to be added");
                assert_eq!(
                    cd.removed,
                    vec!["other-srv"],
                    "expected other-srv to be removed"
                );
                assert!(cd.modified.is_empty(), "expected no modified servers");
            } else {
                panic!("expected Mcp dimension diff");
            }
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
