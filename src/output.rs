//! Output formatting for CLI results (text and JSON).

use chrono::{DateTime, SecondsFormat};
use serde::Serialize;

use crate::expr::{CronExpr, CronFlavor};
use crate::validate::{IssueSeverity, ValidationIssue};

/// The output format requested by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

/// A run-time entry for JSON output.
#[derive(Serialize)]
pub struct RunTimeEntry {
    pub index: usize,
    pub timestamp: String,
}

/// An overlap entry for JSON output.
#[derive(Serialize)]
pub struct OverlapEntry {
    pub timestamp: String,
    pub schedules: Vec<String>,
}

/// A validation result for JSON output.
#[derive(Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub issues: Vec<ValidationIssueJson>,
}

#[derive(Serialize)]
pub struct ValidationIssueJson {
    pub severity: String,
    pub message: String,
}

/// An explanation result for JSON output.
#[derive(Serialize)]
pub struct ExplainResult {
    pub expression: String,
    pub flavor: String,
    pub description: String,
}

/// Format a list of run times as text.
pub fn format_run_times_text(times: &[DateTime<chrono_tz::Tz>]) -> String {
    let mut out = String::new();
    for (i, t) in times.iter().enumerate() {
        out.push_str(&format!("{:>3}  {}\n", i + 1, format_dt(t)));
    }
    out
}

/// Format a list of run times as JSON.
pub fn format_run_times_json(times: &[DateTime<chrono_tz::Tz>]) -> String {
    let entries: Vec<RunTimeEntry> = times
        .iter()
        .enumerate()
        .map(|(i, t)| RunTimeEntry {
            index: i + 1,
            timestamp: format_dt(t),
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
}

/// Format overlaps as text.
pub fn format_overlaps_text(overlaps: &[crate::overlap::Overlap]) -> String {
    if overlaps.is_empty() {
        return "No overlaps found.\n".to_string();
    }
    let mut out = String::new();
    out.push_str(&format!("Found {} overlap(s):\n\n", overlaps.len()));
    for o in overlaps {
        out.push_str(&format!(
            "  {}  {}\n",
            format_dt(&o.time),
            o.schedules.join(", ")
        ));
    }
    out
}

/// Format overlaps as JSON.
pub fn format_overlaps_json(overlaps: &[crate::overlap::Overlap]) -> String {
    let entries: Vec<OverlapEntry> = overlaps
        .iter()
        .map(|o| OverlapEntry {
            timestamp: format_dt(&o.time),
            schedules: o.schedules.clone(),
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
}

/// Format validation issues as text.
pub fn format_validation_text(expr: &CronExpr, issues: &[ValidationIssue]) -> String {
    let mut out = String::new();
    let has_errors = issues.iter().any(|i| i.severity == IssueSeverity::Error);
    if issues.is_empty() {
        out.push_str(&format!("✓ '{}' is valid.\n", expr.raw));
    } else if has_errors {
        out.push_str(&format!("✗ '{}' is INVALID:\n", expr.raw));
    } else {
        out.push_str(&format!("⚠ '{}' is valid with warnings:\n", expr.raw));
    }
    for issue in issues {
        let marker = match issue.severity {
            IssueSeverity::Error => "✗",
            IssueSeverity::Warning => "⚠",
        };
        out.push_str(&format!("  {marker} {}\n", issue.message));
    }
    out
}

/// Format validation issues as JSON.
pub fn format_validation_json(expr: &CronExpr, issues: &[ValidationIssue]) -> String {
    let valid = !issues.iter().any(|i| i.severity == IssueSeverity::Error);
    let result = ValidationResult {
        valid,
        issues: issues
            .iter()
            .map(|i| ValidationIssueJson {
                severity: match i.severity {
                    IssueSeverity::Error => "error".to_string(),
                    IssueSeverity::Warning => "warning".to_string(),
                },
                message: i.message.clone(),
            })
            .collect(),
    };
    let _ = expr;
    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
}

/// Format an explanation as text.
pub fn format_explain_text(expr: &CronExpr, description: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("Expression: {}\n", expr.raw));
    out.push_str(&format!("Flavor:     {}\n", expr.flavor.name()));
    out.push_str(&format!("Meaning:    {}\n", description));
    out
}

/// Format an explanation as JSON.
pub fn format_explain_json(expr: &CronExpr, description: &str) -> String {
    let result = ExplainResult {
        expression: expr.raw.clone(),
        flavor: flavor_string(expr.flavor),
        description: description.to_string(),
    };
    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
}

fn flavor_string(f: CronFlavor) -> String {
    f.name().to_string()
}

/// Format a datetime in ISO 8601 with timezone.
pub fn format_dt<Tz: chrono::TimeZone>(dt: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_times_text() {
        let tz = chrono_tz::UTC;
        let t = chrono::Utc::now().with_timezone(&tz);
        let out = format_run_times_text(&[t]);
        assert!(out.contains("1"));
    }

    #[test]
    fn run_times_json() {
        let tz = chrono_tz::UTC;
        let t = chrono::Utc::now().with_timezone(&tz);
        let out = format_run_times_json(&[t]);
        assert!(out.contains("index"));
    }

    #[test]
    fn overlaps_empty_text() {
        let out = format_overlaps_text(&[]);
        assert!(out.contains("No overlaps"));
    }
}
