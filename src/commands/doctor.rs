//! Doctor command: comprehensive health check for tool configurations.
//!
//! The doctor command validates configuration files across all registered
//! tools and reports issues. It can also check consistency between the
//! unified lorum configuration and individual tool configurations.

use std::collections::BTreeSet;

use crate::adapters::{ConfigValidator, Severity, ValidationIssue, all_config_validators, find_config_validator};
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
    println!("{:<15} {:>8} {:>8} {:>8}", "TOOL", "STATUS", "ERRORS", "WARNINGS");

    for result in results {
        let status = if result.healthy { "OK" } else { "FAIL" };
        let errors = result.issues.iter().filter(|i| i.severity == Severity::Error).count();
        let warnings = result.issues.iter().filter(|i| i.severity == Severity::Warning).count();
        println!(
            "{:<15} {:>8} {:>8} {:>8}",
            result.tool,
            status,
            errors,
            warnings,
        );

        // Print each issue with indentation
        for issue in &result.issues {
            let severity_label = match issue.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            if let Some(ref path) = issue.path {
                println!("  {}: {} ({})", severity_label, issue.message, path.display());
            } else {
                println!("  {}: {}", severity_label, issue.message);
            }
        }
    }

    let total_errors: usize = results
        .iter()
        .map(|r| r.issues.iter().filter(|i| i.severity == Severity::Error).count())
        .sum();
    let total_warnings: usize = results
        .iter()
        .map(|r| r.issues.iter().filter(|i| i.severity == Severity::Warning).count())
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
    use crate::adapters::{find_adapter, all_adapters};
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
