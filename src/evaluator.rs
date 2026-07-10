//! Cron expression evaluation: computing next and previous run times.
//!
//! The evaluator uses a field-by-field advancement algorithm. Starting from
//! a reference time it walks forward (or backward) one field at a time —
//! year, month, day, hour, minute, second — resetting the smaller fields
//! whenever a larger field is advanced. This is far more efficient than a
//! naive second-by-second scan.

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Weekday};

use crate::expr::CronExpr;
use crate::field::{FieldSpec, Term};

/// Maximum number of iterations before we give up searching (safety valve
/// against pathological expressions or infinite loops).
const MAX_ITERATIONS: u32 = 366 * 100;

/// Result of matching a specific calendar day against the day-of-month and
/// day-of-week fields.
enum DayMatch {
    /// The day matches.
    Yes,
    /// The day does not match.
    No,
    /// The day-of-month value is out of range for this month (e.g. day 30 in
    /// February). The caller should skip to the next month.
    Invalid,
}

/// Compute the next run time at or after `after` (exclusive — the returned
/// time is strictly greater than `after`).
pub fn next_run<Tz: TimeZone>(expr: &CronExpr, after: DateTime<Tz>) -> Result<DateTime<Tz>, String>
where
    Tz::Offset: std::fmt::Display,
{
    let mut candidate = after + Duration::seconds(1);
    // Truncate to whole seconds.
    candidate = candidate
        .with_nanosecond(0)
        .ok_or("failed to truncate nanoseconds")?;

    for _ in 0..MAX_ITERATIONS {
        // --- Year ---
        if expr.has_year() && !year_matches(expr, candidate.year() as u32) {
            let next_year = next_year_value(expr, candidate.year() as u32)
                .ok_or_else(|| "no matching year within range".to_string())?;
            if next_year as i32 <= candidate.year() {
                return Err("could not find next run time".to_string());
            }
            candidate = make_dt(&candidate, next_year as i32, 1, 1, 0, 0, 0)?;
            continue;
        }

        // --- Month ---
        if !month_matches(expr, candidate.month()) {
            let next_month = next_month_value(expr, candidate.month(), candidate.year() as u32);
            match next_month {
                Some((m, y)) => {
                    candidate = make_dt(&candidate, y as i32, m, 1, 0, 0, 0)?;
                    continue;
                }
                None => {
                    // No matching month this year — advance to next year.
                    let ny = candidate.year() + 1;
                    if expr.has_year() && !year_matches(expr, ny as u32) {
                        let next_y = next_year_value(expr, ny as u32)
                            .ok_or_else(|| "no matching year within range".to_string())?;
                        candidate = make_dt(&candidate, next_y as i32, 1, 1, 0, 0, 0)?;
                    } else {
                        candidate = make_dt(&candidate, ny, 1, 1, 0, 0, 0)?;
                    }
                    continue;
                }
            }
        }

        // --- Day ---
        match day_matches(expr, &candidate) {
            DayMatch::No => {
                let next_date = candidate
                    .date_naive()
                    .succ_opt()
                    .ok_or("date out of range")?;
                candidate = from_naive(&candidate, next_date, 0, 0, 0)?;
                continue;
            }
            DayMatch::Invalid => {
                let (m, y) = if candidate.month() == 12 {
                    (1u32, candidate.year() + 1)
                } else {
                    (candidate.month() + 1, candidate.year())
                };
                candidate = make_dt(&candidate, y, m, 1, 0, 0, 0)?;
                continue;
            }
            DayMatch::Yes => {}
        }

        // --- Hour ---
        if !hour_matches(expr, candidate.hour()) {
            let next_h = next_value_in_set(&expr.hour, candidate.hour());
            match next_h {
                Some(h) => {
                    candidate = candidate
                        .with_hour(h)
                        .and_then(|c| c.with_minute(0))
                        .and_then(|c| c.with_second(0))
                        .ok_or("failed to set hour")?;
                    continue;
                }
                None => {
                    let next_date = candidate
                        .date_naive()
                        .succ_opt()
                        .ok_or("date out of range")?;
                    candidate = from_naive(&candidate, next_date, 0, 0, 0)?;
                    continue;
                }
            }
        }

        // --- Minute ---
        if !minute_matches(expr, candidate.minute()) {
            let next_m = next_value_in_set(&expr.minute, candidate.minute());
            match next_m {
                Some(m) => {
                    candidate = candidate
                        .with_minute(m)
                        .and_then(|c| c.with_second(0))
                        .ok_or("failed to set minute")?;
                    continue;
                }
                None => {
                    // No matching minute this hour — advance to next hour at minute 0, second 0.
                    let next_dt = candidate.naive_local() + Duration::hours(1);
                    candidate = from_naive(&candidate, next_dt.date(), next_dt.hour(), 0, 0)?;
                    continue;
                }
            }
        }

        // --- Second ---
        // Always check seconds — for 5-field expressions the second field is
        // pinned to 0, so this ensures they only fire at second 0.
        if !second_matches(expr, candidate.second()) {
            let next_s = next_value_in_set(&expr.second, candidate.second());
            match next_s {
                Some(s) => {
                    candidate = candidate.with_second(s).ok_or("failed to set second")?;
                    continue;
                }
                None => {
                    let next_dt = candidate.naive_local() + Duration::minutes(1);
                    candidate = from_naive(
                        &candidate,
                        next_dt.date(),
                        next_dt.hour(),
                        next_dt.minute(),
                        0,
                    )?;
                    continue;
                }
            }
        }

        // All fields match.
        return Ok(candidate);
    }

    Err("exceeded maximum iterations while searching for next run time".to_string())
}

/// Compute the previous run time strictly before `before`.
pub fn prev_run<Tz: TimeZone>(expr: &CronExpr, before: DateTime<Tz>) -> Result<DateTime<Tz>, String>
where
    Tz::Offset: std::fmt::Display,
{
    let mut candidate = before - Duration::seconds(1);
    candidate = candidate
        .with_nanosecond(0)
        .ok_or("failed to truncate nanoseconds")?;

    for _ in 0..MAX_ITERATIONS {
        // --- Year ---
        if expr.has_year() && !year_matches(expr, candidate.year() as u32) {
            let prev_year = prev_year_value(expr, candidate.year() as u32)
                .ok_or_else(|| "no matching year within range".to_string())?;
            if prev_year as i32 >= candidate.year() {
                return Err("could not find previous run time".to_string());
            }
            candidate = make_dt(&candidate, prev_year as i32, 12, 31, 23, 59, 59)?;
            continue;
        }

        // --- Month ---
        if !month_matches(expr, candidate.month()) {
            let prev_month = prev_month_value(expr, candidate.month(), candidate.year() as u32);
            match prev_month {
                Some((m, y)) => {
                    let last_day = days_in_month(y as i32, m);
                    candidate = make_dt(&candidate, y as i32, m, last_day, 23, 59, 59)?;
                    continue;
                }
                None => {
                    let py = candidate.year() - 1;
                    if expr.has_year() && !year_matches(expr, py as u32) {
                        let prev_y = prev_year_value(expr, py as u32)
                            .ok_or_else(|| "no matching year within range".to_string())?;
                        candidate = make_dt(&candidate, prev_y as i32, 12, 31, 23, 59, 59)?;
                    } else {
                        candidate = make_dt(&candidate, py, 12, 31, 23, 59, 59)?;
                    }
                    continue;
                }
            }
        }

        // --- Day ---
        match day_matches(expr, &candidate) {
            DayMatch::No | DayMatch::Invalid => {
                let prev_date = candidate
                    .date_naive()
                    .pred_opt()
                    .ok_or("date out of range")?;
                candidate = from_naive(&candidate, prev_date, 23, 59, 59)?;
                continue;
            }
            DayMatch::Yes => {}
        }

        // --- Hour ---
        if !hour_matches(expr, candidate.hour()) {
            let prev_h = prev_value_in_set(&expr.hour, candidate.hour());
            match prev_h {
                Some(h) => {
                    candidate = candidate
                        .with_hour(h)
                        .and_then(|c| c.with_minute(59))
                        .and_then(|c| c.with_second(59))
                        .ok_or("failed to set hour")?;
                    continue;
                }
                None => {
                    let prev_date = candidate
                        .date_naive()
                        .pred_opt()
                        .ok_or("date out of range")?;
                    candidate = from_naive(&candidate, prev_date, 23, 59, 59)?;
                    continue;
                }
            }
        }

        // --- Minute ---
        if !minute_matches(expr, candidate.minute()) {
            let prev_m = prev_value_in_set(&expr.minute, candidate.minute());
            match prev_m {
                Some(m) => {
                    candidate = candidate
                        .with_minute(m)
                        .and_then(|c| c.with_second(59))
                        .ok_or("failed to set minute")?;
                    continue;
                }
                None => {
                    let prev_dt = candidate.naive_local() - Duration::hours(1);
                    candidate = from_naive(&candidate, prev_dt.date(), prev_dt.hour(), 59, 59)?;
                    continue;
                }
            }
        }

        // --- Second ---
        // Always check seconds — for 5-field expressions the second field is
        // pinned to 0, so this ensures they only fire at second 0.
        if !second_matches(expr, candidate.second()) {
            let prev_s = prev_value_in_set(&expr.second, candidate.second());
            match prev_s {
                Some(s) => {
                    candidate = candidate.with_second(s).ok_or("failed to set second")?;
                    continue;
                }
                None => {
                    let prev_dt = candidate.naive_local() - Duration::minutes(1);
                    candidate = from_naive(
                        &candidate,
                        prev_dt.date(),
                        prev_dt.hour(),
                        prev_dt.minute(),
                        59,
                    )?;
                    continue;
                }
            }
        }

        return Ok(candidate);
    }

    Err("exceeded maximum iterations while searching for previous run time".to_string())
}

/// Compute the next `count` run times after `after`.
pub fn next_runs<Tz: TimeZone>(
    expr: &CronExpr,
    after: DateTime<Tz>,
    count: usize,
) -> Result<Vec<DateTime<Tz>>, String>
where
    Tz::Offset: std::fmt::Display,
{
    let mut results = Vec::with_capacity(count);
    let mut current = after;
    for _ in 0..count {
        let next = next_run(expr, current)?;
        current = next.clone();
        results.push(next);
    }
    Ok(results)
}

// ---------------------------------------------------------------------------
// DateTime construction helpers (avoid moving `candidate` into closures)
// ---------------------------------------------------------------------------

/// Construct a `DateTime` from a reference datetime's timezone and explicit
/// Y/M/D/H/M/S components.
fn make_dt<Tz: TimeZone>(
    ref_dt: &DateTime<Tz>,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
) -> Result<DateTime<Tz>, String> {
    let tz = ref_dt.timezone();
    tz.with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()
        .ok_or_else(|| format!("invalid date constructed: {year}-{month}-{day} {hour}:{min}:{sec}"))
}

/// Construct a `DateTime` from a reference datetime's timezone, a `NaiveDate`,
/// and explicit H/M/S.
fn from_naive<Tz: TimeZone>(
    ref_dt: &DateTime<Tz>,
    date: NaiveDate,
    hour: u32,
    min: u32,
    sec: u32,
) -> Result<DateTime<Tz>, String> {
    let tz = ref_dt.timezone();
    let ndt = date
        .and_hms_opt(hour, min, sec)
        .ok_or_else(|| format!("invalid time: {hour}:{min}:{sec}"))?;
    tz.from_local_datetime(&ndt)
        .single()
        .ok_or_else(|| format!("invalid local datetime: {ndt}"))
}

// ---------------------------------------------------------------------------
// Field matching helpers
// ---------------------------------------------------------------------------

fn year_matches(expr: &CronExpr, year: u32) -> bool {
    if !expr.has_year() {
        return true;
    }
    value_matches(&expr.year, year)
}

fn month_matches(expr: &CronExpr, month: u32) -> bool {
    value_matches(&expr.month, month)
}

fn hour_matches(expr: &CronExpr, hour: u32) -> bool {
    value_matches(&expr.hour, hour)
}

fn minute_matches(expr: &CronExpr, minute: u32) -> bool {
    value_matches(&expr.minute, minute)
}

fn second_matches(expr: &CronExpr, second: u32) -> bool {
    value_matches(&expr.second, second)
}

/// Check whether a plain numeric value matches a field's terms (ignoring
/// special modifiers, which are handled separately for day fields).
fn value_matches(field: &FieldSpec, value: u32) -> bool {
    for term in &field.terms {
        match term {
            Term::Wildcard => return true,
            Term::Single(v) if *v == value => return true,
            Term::Range(a, b) if *a <= value && value <= *b => return true,
            Term::Step { from, step } => {
                if value >= *from && (value - from).is_multiple_of(*step) {
                    return true;
                }
            }
            Term::RangeStep { from, to, step }
                if *from <= value && value <= *to && (value - from).is_multiple_of(*step) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// The day-matching logic, accounting for special modifiers and the
/// Vixie/Quartz DOM-DOW semantics.
fn day_matches<Tz: TimeZone>(expr: &CronExpr, dt: &DateTime<Tz>) -> DayMatch {
    let year = dt.year();
    let month = dt.month();
    let day = dt.day();
    let weekday = dt.weekday();

    let days_this_month = days_in_month(year, month);
    if day > days_this_month {
        return DayMatch::Invalid;
    }

    let dom_match = dom_matches(&expr.day_of_month, year, month, day);
    let dow_match = dow_matches(&expr.day_of_week, year, month, day, weekday);

    let dom_q = expr.day_of_month.is_question();
    let dow_q = expr.day_of_week.is_question();

    if dom_q && dow_q {
        return DayMatch::No;
    }
    if dom_q {
        return if dow_match {
            DayMatch::Yes
        } else {
            DayMatch::No
        };
    }
    if dow_q {
        return if dom_match {
            DayMatch::Yes
        } else {
            DayMatch::No
        };
    }

    let dom_restricted = expr.dom_restricted();
    let dow_restricted = expr.dow_restricted();

    if dom_restricted && dow_restricted {
        if dom_match || dow_match {
            DayMatch::Yes
        } else {
            DayMatch::No
        }
    } else if dom_restricted {
        if dom_match {
            DayMatch::Yes
        } else {
            DayMatch::No
        }
    } else if dow_restricted {
        if dow_match {
            DayMatch::Yes
        } else {
            DayMatch::No
        }
    } else {
        DayMatch::Yes
    }
}

/// Match the day-of-month field, including special modifiers `L`, `L-n`, `nW`.
fn dom_matches(field: &FieldSpec, year: i32, month: u32, day: u32) -> bool {
    let last_day = days_in_month(year, month);
    for term in &field.terms {
        match term {
            Term::Wildcard => return true,
            Term::Single(v) if *v == day => return true,
            Term::Range(a, b) if *a <= day && day <= *b => return true,
            Term::Step { from, step } => {
                if day >= *from && (day - from).is_multiple_of(*step) {
                    return true;
                }
            }
            Term::RangeStep { from, to, step } => {
                if *from <= day && day <= *to && (day - from).is_multiple_of(*step) {
                    return true;
                }
            }
            Term::Last if day == last_day => return true,
            Term::LastOffset(n) => {
                let target = last_day.saturating_sub(*n);
                if day == target && target >= 1 {
                    return true;
                }
            }
            Term::NearestWeekday(target) => {
                if let Some(nearest) = nearest_weekday(year, month, *target) {
                    if day == nearest {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Match the day-of-week field, including special modifiers `nL` and `n#m`.
fn dow_matches(field: &FieldSpec, year: i32, month: u32, day: u32, weekday: Weekday) -> bool {
    let dow_num = weekday_to_num(weekday);
    let last_day = days_in_month(year, month);

    for term in &field.terms {
        match term {
            Term::Wildcard => return true,
            Term::Question => {}
            Term::Single(v) if *v == dow_num => return true,
            Term::Range(a, b) if *a <= dow_num && dow_num <= *b => return true,
            Term::Step { from, step } => {
                if dow_num >= *from && (dow_num - from).is_multiple_of(*step) {
                    return true;
                }
            }
            Term::RangeStep { from, to, step } => {
                if *from <= dow_num && dow_num <= *to && (dow_num - from).is_multiple_of(*step) {
                    return true;
                }
            }
            Term::LastWeekday(w) => {
                if *w == dow_num && day + 7 > last_day {
                    return true;
                }
            }
            Term::NthWeekday {
                weekday,
                occurrence,
            } if *weekday == dow_num => {
                let occ = (day - 1) / 7 + 1;
                if occ == *occurrence {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

fn weekday_to_num(w: Weekday) -> u32 {
    match w {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    }
}

/// Number of days in a given month (handles leap years).
fn days_in_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(
        year + if month == 12 { 1 } else { 0 },
        if month == 12 { 1 } else { month + 1 },
        1,
    )
    .and_then(|first_next| first_next.pred_opt())
    .map(|d| d.day())
    .unwrap_or(28)
}

/// Find the nearest weekday (Mon-Fri) to `target` day in the given month.
fn nearest_weekday(year: i32, month: u32, target: u32) -> Option<u32> {
    let last_day = days_in_month(year, month);
    if target > last_day {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year, month, target)?;
    match date.weekday() {
        Weekday::Sat => {
            if target == 1 {
                Some(3)
            } else {
                Some(target - 1)
            }
        }
        Weekday::Sun => {
            if target == last_day {
                Some(target - 2)
            } else {
                Some(target + 1)
            }
        }
        _ => Some(target),
    }
}

/// Find the next value >= `current` in a field's numeric value set.
fn next_value_in_set(field: &FieldSpec, current: u32) -> Option<u32> {
    let values = field.numeric_values();
    values.into_iter().find(|&v| v >= current)
}

/// Find the previous value <= `current` in a field's numeric value set.
fn prev_value_in_set(field: &FieldSpec, current: u32) -> Option<u32> {
    let values = field.numeric_values();
    values.into_iter().rev().find(|&v| v <= current)
}

fn next_year_value(expr: &CronExpr, current: u32) -> Option<u32> {
    let values = expr.year.numeric_values();
    values.into_iter().find(|&v| v >= current)
}

fn prev_year_value(expr: &CronExpr, current: u32) -> Option<u32> {
    let values = expr.year.numeric_values();
    values.into_iter().rev().find(|&v| v <= current)
}

fn next_month_value(expr: &CronExpr, current: u32, year: u32) -> Option<(u32, u32)> {
    let values = expr.month.numeric_values();
    values
        .into_iter()
        .find(|&v| v >= current)
        .map(|m| (m, year))
}

fn prev_month_value(expr: &CronExpr, current: u32, year: u32) -> Option<(u32, u32)> {
    let values = expr.month.numeric_values();
    values
        .into_iter()
        .rev()
        .find(|&v| v <= current)
        .map(|m| (m, year))
}

// Keep FieldKind imported for potential future use.
#[allow(unused_imports)]
use field_kind_marker::FieldKind as _FieldKind;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn parse(input: &str) -> CronExpr {
        crate::expr::parse_cron(input).unwrap()
    }

    #[test]
    fn next_every_minute() {
        let expr = parse("* * * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // 5-field fires at second 0, so next run is the next minute boundary.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 1, 12, 1, 0).unwrap());
    }

    #[test]
    fn next_every_5_minutes() {
        let expr = parse("*/5 * * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 3, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 1, 12, 5, 0).unwrap());
    }

    #[test]
    fn next_specific_time() {
        let expr = parse("30 2 * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 2, 2, 30, 0).unwrap());
    }

    #[test]
    fn next_specific_day() {
        let expr = parse("0 0 1 * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_specific_month() {
        let expr = parse("0 0 1 6 *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_with_seconds() {
        let expr = parse("*/30 * * * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 10).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 30).unwrap());
    }

    #[test]
    fn next_dow_friday() {
        let expr = parse("0 0 * * 5");
        // 2026-01-01 is a Thursday.
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // Next Friday is 2026-01-02.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_dom_and_dow_or() {
        // Vixie OR: runs on the 1st OR on Monday.
        let expr = parse("0 0 1 * 1");
        // 2026-01-01 is Thursday. Day 1 matches DOM → fires Jan 1.
        // But we start AFTER Jan 1 00:00, so next is Jan 5 (Monday).
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_last_day_of_month() {
        let expr = parse("0 0 L * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 31, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_last_friday() {
        let expr = parse("0 0 * * 5L");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // Last Friday of January 2026 is the 30th.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 30, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_nth_friday() {
        // Third Friday of the month.
        let expr = parse("0 0 * * 5#3");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // Jan 2026: Fridays are 2, 9, 16, 23, 30. Third is the 16th.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 16, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_nearest_weekday() {
        // 15W — nearest weekday to the 15th.
        let expr = parse("0 0 15W * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // Jan 15 2026 is a Thursday — so the 15th itself.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_nearest_weekday_sunday() {
        // 1W — nearest weekday to the 1st.
        // Feb 1 2026 is Sunday → nearest weekday is Mon Feb 2.
        let expr = parse("0 0 1W * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 31, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 2, 2, 0, 0, 0).unwrap());
    }

    #[test]
    fn prev_every_minute() {
        let expr = parse("* * * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 30).unwrap();
        let prev = prev_run(&expr, start).unwrap();
        // 5-field fires at second 0, so previous run is the current minute boundary.
        assert_eq!(prev, Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap());
    }

    #[test]
    fn prev_specific_time() {
        let expr = parse("30 2 * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
        let prev = prev_run(&expr, start).unwrap();
        assert_eq!(prev, Utc.with_ymd_and_hms(2026, 1, 2, 2, 30, 0).unwrap());
    }

    #[test]
    fn next_runs_multiple() {
        let expr = parse("*/15 * * * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let runs = next_runs(&expr, start, 4).unwrap();
        assert_eq!(runs.len(), 4);
        assert_eq!(
            runs[0],
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 15, 0).unwrap()
        );
        assert_eq!(
            runs[1],
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 30, 0).unwrap()
        );
        assert_eq!(
            runs[2],
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 45, 0).unwrap()
        );
        assert_eq!(runs[3], Utc.with_ymd_and_hms(2026, 1, 1, 13, 0, 0).unwrap());
    }

    #[test]
    fn days_in_month_leap() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
    }

    #[test]
    fn weekday_to_num_correct() {
        assert_eq!(weekday_to_num(Weekday::Sun), 0);
        assert_eq!(weekday_to_num(Weekday::Mon), 1);
        assert_eq!(weekday_to_num(Weekday::Sat), 6);
    }

    #[test]
    fn next_with_year() {
        let expr = parse("0 0 0 1 1 * 2027");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_last_day_offset() {
        let expr = parse("0 0 L-3 * *");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // Jan has 31 days, L-3 = 28.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 28, 0, 0, 0).unwrap());
    }

    #[test]
    fn next_month_names() {
        let expr = parse("0 0 1 JAN,JUL MON-FRI");
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let next = next_run(&expr, start).unwrap();
        // Jan 1 2026 is Thursday (a weekday), DOM=1 matches, so fires Jan 1.
        // But we start AFTER Jan 1 00:00, so next is Jan 2 (Fri).
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap());
    }
}

// Dummy module to allow the unused-import marker.
mod field_kind_marker {
    pub use crate::field::FieldKind;
}
