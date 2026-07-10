//! Validation of cron expressions beyond what the parser checks.
//!
//! The parser rejects syntactically invalid expressions. This module
//! performs *semantic* validation: detecting impossible day/month
//! combinations, unreachable expressions, and other issues that would
//! cause a schedule to never fire.

use crate::expr::CronExpr;
use crate::field::{FieldKind, Term};

/// A single validation issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    /// The expression will never match — it is dead code.
    Error,
    /// The expression is valid but likely unintended.
    Warning,
}

/// Validate a cron expression, returning a list of issues. An empty list
/// means the expression is fully valid with no warnings.
pub fn validate(expr: &CronExpr) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // Check for impossible day-of-month values (e.g. day 31 in February).
    check_impossible_days(expr, &mut issues);

    // Check for day-of-week values that never occur with the given month/day
    // constraints.
    check_dow_feasibility(expr, &mut issues);

    // Check for `nW` where n is beyond the month's days.
    check_nearest_weekday_feasibility(expr, &mut issues);

    // Check for `n#m` where m > 4 and the weekday never has a 5th occurrence.
    check_nth_weekday_feasibility(expr, &mut issues);

    // Check for year ranges that are entirely in the past.
    check_year_past(expr, &mut issues);

    // Check for step values that produce a single value (degenerate).
    check_degenerate_steps(expr, &mut issues);

    issues
}

/// Whether the expression is valid (no Error-severity issues).
pub fn is_valid(expr: &CronExpr) -> bool {
    validate(expr)
        .iter()
        .all(|i| i.severity != IssueSeverity::Error)
}

fn check_impossible_days(expr: &CronExpr, issues: &mut Vec<ValidationIssue>) {
    // Skip when day-of-month is unrestricted — the evaluator handles
    // per-month day limits correctly at runtime.
    if expr.day_of_month.is_wildcard() || expr.day_of_month.is_question() {
        return;
    }
    let dom_values = expr.day_of_month.numeric_values();
    let month_values = expr.month.numeric_values();

    for &month in &month_values {
        let max_day = max_days_in_month(month);
        for &day in &dom_values {
            if day > max_day {
                // Check if this month is actually reachable.
                let month_name = month_name(month);
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    message: format!(
                        "day {day} never occurs in {month_name} (max {max_day}) — this combination will never match"
                    ),
                });
            }
        }
    }
}

fn check_dow_feasibility(expr: &CronExpr, issues: &mut Vec<ValidationIssue>) {
    // If DOM is restricted to specific days and DOW is restricted, the OR
    // semantics mean the expression still fires — so no issue. Only flag
    // if BOTH are restricted and neither can ever match.
    // This is a minor check; skip for now as OR semantics handle it.
    let _ = (expr, issues);
}

fn check_nearest_weekday_feasibility(expr: &CronExpr, issues: &mut Vec<ValidationIssue>) {
    for term in &expr.day_of_month.terms {
        if let Term::NearestWeekday(n) = term {
            let month_values = expr.month.numeric_values();
            for &month in &month_values {
                if *n > max_days_in_month(month) {
                    issues.push(ValidationIssue {
                        severity: IssueSeverity::Warning,
                        message: format!(
                            "{n}W: day {n} does not exist in {} — this modifier will never match for that month",
                            month_name(month)
                        ),
                    });
                }
            }
        }
    }
}

fn check_nth_weekday_feasibility(expr: &CronExpr, issues: &mut Vec<ValidationIssue>) {
    for term in &expr.day_of_week.terms {
        if let Term::NthWeekday {
            weekday,
            occurrence,
        } = term
        {
            if *occurrence == 5 {
                // A 5th occurrence only happens in months long enough.
                // This is just a warning, not an error.
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    message: format!(
                        "{weekday}#5: the fifth {} of the month does not occur in every month — this will skip some months",
                        dow_name(*weekday)
                    ),
                });
            }
        }
    }
}

fn check_year_past(expr: &CronExpr, issues: &mut Vec<ValidationIssue>) {
    if !expr.has_year() || expr.year.is_wildcard() {
        return;
    }
    let now = chrono::Utc::now().year();
    let year_values = expr.year.numeric_values();
    let all_past = year_values.iter().all(|&y| (y as i32) < now);
    if all_past && !year_values.is_empty() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            message: format!(
                "all specified years ({}) are in the past — this expression will never fire again",
                year_values
                    .iter()
                    .map(|y| y.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
}

fn check_degenerate_steps(expr: &CronExpr, issues: &mut Vec<ValidationIssue>) {
    for (field, kind) in [
        (&expr.second, FieldKind::Second),
        (&expr.minute, FieldKind::Minute),
        (&expr.hour, FieldKind::Hour),
    ] {
        for term in &field.terms {
            if let Term::Step { from, step } = term {
                let (min, max) = kind.numeric_range();
                if *step > max - min {
                    issues.push(ValidationIssue {
                        severity: IssueSeverity::Warning,
                        message: format!(
                            "{} step value {step} is larger than the field range ({min}-{max}) — only {from} will ever match",
                            kind.name()
                        ),
                    });
                }
            }
        }
    }
}

fn max_days_in_month(month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 29, // worst case (leap year)
        _ => 28,
    }
}

fn month_name(m: u32) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][(m - 1) as usize]
}

fn dow_name(d: u32) -> &'static str {
    [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ][d as usize]
}

// chrono::Datelike is needed for .year()
use chrono::Datelike;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> CronExpr {
        crate::expr::parse_cron(input).unwrap()
    }

    #[test]
    fn valid_simple() {
        let expr = parse("*/5 * * * *");
        assert!(validate(&expr).is_empty());
    }

    #[test]
    fn warn_feb_30() {
        let expr = parse("0 0 30 2 *");
        let issues = validate(&expr);
        assert!(
            issues.iter().any(|i| i.message.contains("February")),
            "{issues:?}"
        );
    }

    #[test]
    fn warn_feb_31() {
        let expr = parse("0 0 31 2 *");
        let issues = validate(&expr);
        assert!(issues.iter().any(|i| i.message.contains("31")));
    }

    #[test]
    fn warn_nth_5() {
        let expr = parse("0 0 * * 5#5");
        let issues = validate(&expr);
        assert!(issues.iter().any(|i| i.message.contains("fifth")));
    }

    #[test]
    fn warn_degenerate_step() {
        let expr = parse("0/120 * * * *");
        let issues = validate(&expr);
        assert!(issues.iter().any(|i| i.message.contains("step")));
    }

    #[test]
    fn is_valid_true() {
        let expr = parse("*/5 * * * *");
        assert!(is_valid(&expr));
    }
}
