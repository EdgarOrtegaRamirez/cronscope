//! Human-readable description of cron expressions.
//!
//! Converts a parsed [`CronExpr`] into a plain-English sentence, e.g.
//! "At 02:30 on every Monday in January".

use crate::expr::CronExpr;
use crate::field::{FieldKind, FieldSpec, Term};

const MONTH_NAMES: [&str; 12] = [
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
];

const DOW_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Produce a human-readable description of the cron expression.
pub fn explain(expr: &CronExpr) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Time portion.
    parts.push(time_part(expr));

    // Day portion.
    let day_desc = day_part(expr);
    if !day_desc.is_empty() {
        parts.push(day_desc);
    }

    // Month portion.
    let month_desc = month_part(expr);
    if !month_desc.is_empty() {
        parts.push(month_desc);
    }

    // Year portion.
    if expr.has_year() && !expr.year.is_wildcard() {
        parts.push(year_part(expr));
    }

    let mut result = parts.join(" ");
    // Capitalise first letter.
    if let Some(c) = result.chars().next() {
        let mut upper = c.to_uppercase().to_string();
        upper.push_str(&result[c.len_utf8()..]);
        result = upper;
    }
    result.push('.');
    result
}

fn time_part(expr: &CronExpr) -> String {
    let sec = &expr.second;
    let min = &expr.minute;
    let hr = &expr.hour;

    let has_seconds = expr.has_seconds() && !sec.is_wildcard() && !all_zero(sec);
    let min_wild = min.is_wildcard() || all_same_every(min, 0);
    let hr_wild = hr.is_wildcard() || all_same_every(hr, 0);

    if has_seconds {
        // Seconds-level description.
        let sec_desc = describe_field(sec, FieldKind::Second, "second");
        if min_wild && hr_wild {
            format!("At {sec_desc}")
        } else if hr_wild {
            format!(
                "At {sec_desc} {}",
                describe_field(min, FieldKind::Minute, "minute")
            )
        } else if min_wild {
            format!(
                "At {sec_desc} {}",
                describe_field(hr, FieldKind::Hour, "hour")
            )
        } else {
            // Specific minute and hour — build a time string with seconds.
            let times = build_times(expr);
            if times.len() == 1 {
                format!("At {}", times[0])
            } else {
                format!("At {}", join_list(&times))
            }
        }
    } else if min_wild && hr_wild {
        "Every minute".to_string()
    } else if hr_wild {
        describe_field(min, FieldKind::Minute, "minute")
    } else if min_wild {
        format!(
            "At {} every hour",
            describe_field(hr, FieldKind::Hour, "hour")
        )
    } else {
        // Specific time(s).
        let times = build_times(expr);
        if times.len() == 1 {
            format!("At {}", times[0])
        } else {
            format!("At {}", join_list(&times))
        }
    }
}

/// Build a list of "HH:MM" (or "HH:MM:SS") strings for specific time-of-day
/// matches, capped at a reasonable number.
fn build_times(expr: &CronExpr) -> Vec<String> {
    let hours = expr.hour.numeric_values();
    let minutes = expr.minute.numeric_values();
    let seconds = if expr.has_seconds() {
        expr.second.numeric_values()
    } else {
        vec![0]
    };

    let mut times = Vec::new();
    for &h in &hours {
        for &m in &minutes {
            if expr.has_seconds() && !expr.second.is_wildcard() {
                for &s in &seconds {
                    times.push(format!("{:02}:{:02}:{:02}", h, m, s));
                    if times.len() >= 12 {
                        return times;
                    }
                }
            } else {
                times.push(format!("{:02}:{:02}", h, m));
                if times.len() >= 12 {
                    return times;
                }
            }
        }
    }
    times
}

fn day_part(expr: &CronExpr) -> String {
    let dom = &expr.day_of_month;
    let dow = &expr.day_of_week;
    let dom_wild = dom.is_wildcard() || dom.is_question();
    let dow_wild = dow.is_wildcard() || dow.is_question();

    if dom_wild && dow_wild {
        return String::new(); // every day — implied
    }
    if dom_wild {
        return describe_dow(dow);
    }
    if dow_wild {
        return describe_dom(dom);
    }
    // Both restricted — Vixie OR semantics.
    format!("on {} or on {}", describe_dom(dom), describe_dow(dow))
}

fn month_part(expr: &CronExpr) -> String {
    let m = &expr.month;
    if m.is_wildcard() {
        return String::new();
    }
    let values = m.numeric_values();
    let names: Vec<&str> = values
        .iter()
        .map(|&v| {
            MONTH_NAMES
                .get((v - 1) as usize)
                .copied()
                .unwrap_or("Unknown")
        })
        .collect();
    format!("in {}", join_list_str(&names))
}

fn year_part(expr: &CronExpr) -> String {
    let values = expr.year.numeric_values();
    let strs: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    format!(
        "in {}",
        join_list_str(&strs.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    )
}

fn describe_dom(field: &FieldSpec) -> String {
    let mut descs = Vec::new();
    for term in &field.terms {
        match term {
            Term::Wildcard | Term::Question => {}
            Term::Last => descs.push("the last day of the month".to_string()),
            Term::LastOffset(n) => descs.push(format!("the last day of the month minus {n} days")),
            Term::NearestWeekday(d) => {
                descs.push(format!("the nearest weekday to day {d} of the month"))
            }
            Term::Single(v) => descs.push(format!("day {v} of the month")),
            Term::Range(a, b) => descs.push(format!("days {a}-{b} of the month")),
            Term::Step { from, step } => {
                if *from == 0 || *from == 1 {
                    descs.push(format!("every {step} days"))
                } else {
                    descs.push(format!("every {step} days starting from day {from}"))
                }
            }
            Term::RangeStep { from, to, step } => {
                descs.push(format!("every {step} days from day {from} to day {to}"))
            }
            _ => {}
        }
    }
    if descs.is_empty() {
        "every day".to_string()
    } else if descs.len() == 1 {
        format!("on {}", descs[0])
    } else {
        format!("on {}", join_list(&descs))
    }
}

fn describe_dow(field: &FieldSpec) -> String {
    let mut descs = Vec::new();
    for term in &field.terms {
        match term {
            Term::Wildcard | Term::Question => {}
            Term::Single(v) => descs.push(dow_name(*v).to_string()),
            Term::Range(a, b) => {
                if *a == 1 && *b == 5 {
                    descs.push("Monday through Friday".to_string());
                } else if *a == 0 && *b == 6 {
                    // every day — skip
                } else {
                    descs.push(format!("{} through {}", dow_name(*a), dow_name(*b)));
                }
            }
            Term::Step { from, step } => descs.push(format!(
                "every {step} days starting from {}",
                dow_name(*from)
            )),
            Term::RangeStep { from, to, step } => descs.push(format!(
                "every {step} days from {} to {}",
                dow_name(*from),
                dow_name(*to)
            )),
            Term::LastWeekday(v) => descs.push(format!("the last {} of the month", dow_name(*v))),
            Term::NthWeekday {
                weekday,
                occurrence,
            } => descs.push(format!(
                "the {} {} of the month",
                ordinal(*occurrence),
                dow_name(*weekday)
            )),
            _ => {}
        }
    }
    if descs.is_empty() {
        "every day".to_string()
    } else if descs.len() == 1 {
        format!("on {}", descs[0])
    } else {
        format!("on {}", join_list(&descs))
    }
}

fn describe_field(field: &FieldSpec, _kind: FieldKind, unit: &str) -> String {
    if field.is_wildcard() {
        return format!("every {unit}");
    }
    let values = field.numeric_values();
    if values.len() == 1 {
        return format!("{unit} {}", values[0]);
    }
    // Check if it's a uniform step.
    if let Some(step) = uniform_step(field) {
        if step == 1 {
            return format!("every {unit}");
        }
        return format!("every {step} {unit}s");
    }
    let strs: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    format!("{} {}", unit, join_list(&strs))
}

/// Detect if a field is a uniform step (e.g. `*/15` → 15).
fn uniform_step(field: &FieldSpec) -> Option<u32> {
    if field.terms.len() == 1 {
        if let Term::Step { step, .. } = &field.terms[0] {
            return Some(*step);
        }
        if let Term::RangeStep { step, .. } = &field.terms[0] {
            return Some(*step);
        }
    }
    None
}

fn all_same_every(field: &FieldSpec, _val: u32) -> bool {
    // True if the field effectively matches everything (e.g. `0-59` for
    // minutes, or `*`).
    if field.is_wildcard() {
        return true;
    }
    let (min, max) = field.kind.numeric_range();
    let values = field.numeric_values();
    values.len() == (max - min + 1) as usize
}

fn all_zero(field: &FieldSpec) -> bool {
    let values = field.numeric_values();
    values.len() == 1 && values[0] == 0
}

fn dow_name(n: u32) -> &'static str {
    DOW_NAMES.get(n as usize).copied().unwrap_or("Unknown")
}

fn ordinal(n: u32) -> &'static str {
    match n {
        1 => "first",
        2 => "second",
        3 => "third",
        4 => "fourth",
        5 => "fifth",
        _ => "nth",
    }
}

fn join_list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} and {}", items[0], items[1]),
        _ => {
            let last = items.len() - 1;
            format!("{}, and {}", items[..last].join(", "), items[last])
        }
    }
}

fn join_list_str(items: &[&str]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        2 => format!("{} and {}", items[0], items[1]),
        _ => {
            let last = items.len() - 1;
            format!("{}, and {}", items[..last].join(", "), items[last])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> CronExpr {
        crate::expr::parse_cron(input).unwrap()
    }

    #[test]
    fn explain_every_minute() {
        let expr = parse("* * * * *");
        let desc = explain(&expr);
        assert!(desc.contains("Every minute"), "got: {desc}");
    }

    #[test]
    fn explain_specific_time() {
        let expr = parse("30 2 * * *");
        let desc = explain(&expr);
        assert!(desc.contains("02:30"), "got: {desc}");
    }

    #[test]
    fn explain_every_5_minutes() {
        let expr = parse("*/5 * * * *");
        let desc = explain(&expr);
        assert!(desc.contains("5 minutes"), "got: {desc}");
    }

    #[test]
    fn explain_specific_day() {
        let expr = parse("0 0 1 * *");
        let desc = explain(&expr);
        assert!(desc.contains("day 1"), "got: {desc}");
    }

    #[test]
    fn explain_dow() {
        let expr = parse("0 0 * * 5");
        let desc = explain(&expr);
        assert!(desc.contains("Friday"), "got: {desc}");
    }

    #[test]
    fn explain_last_day() {
        let expr = parse("0 0 L * *");
        let desc = explain(&expr);
        assert!(desc.contains("last day"), "got: {desc}");
    }

    #[test]
    fn explain_last_friday() {
        let expr = parse("0 0 * * 5L");
        let desc = explain(&expr);
        assert!(desc.contains("last Friday"), "got: {desc}");
    }

    #[test]
    fn explain_nth_friday() {
        let expr = parse("0 0 * * 5#3");
        let desc = explain(&expr);
        assert!(desc.contains("third Friday"), "got: {desc}");
    }

    #[test]
    fn explain_month() {
        let expr = parse("0 0 1 6 *");
        let desc = explain(&expr);
        assert!(desc.contains("June"), "got: {desc}");
    }

    #[test]
    fn explain_nearest_weekday() {
        let expr = parse("0 0 15W * *");
        let desc = explain(&expr);
        assert!(desc.contains("nearest weekday"), "got: {desc}");
    }

    #[test]
    fn explain_mon_fri() {
        let expr = parse("0 0 * * 1-5");
        let desc = explain(&expr);
        assert!(desc.contains("Monday through Friday"), "got: {desc}");
    }
}
