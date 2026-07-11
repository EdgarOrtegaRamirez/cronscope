//! Calendar visualization for cron expressions.
//!
//! Provides three views:
//! - **Monthly calendar**: shows which days a cron expression fires on in a
//!   given month, with fire days highlighted.
//! - **Week view**: 24-hour timeline of fire times by day of week.
//! - **Year overview**: compact view of fire days across all 12 months.

use chrono::{Datelike, NaiveDate, TimeZone};
use chrono_tz::Tz;

use crate::evaluator::next_run;
use crate::expr::CronExpr;

/// Configuration for calendar generation.
#[derive(Debug, Clone)]
pub struct CalendarConfig {
    /// The cron expression to visualize.
    pub expr: CronExpr,
    /// Timezone for date/time display.
    pub tz: Tz,
    /// Year for the calendar.
    pub year: i32,
    /// Month (1-12) for the calendar. If None, show all 12 months.
    pub month: Option<u32>,
    /// Number of months to show (for monthly calendar view).
    pub months: usize,
}

/// A single fire day entry for calendar display.
#[derive(Debug, Clone)]
pub struct FireDay {
    pub day: u32,
    pub times: Vec<String>,
}

/// A month's worth of calendar data.
#[derive(Debug, Clone)]
pub struct MonthCalendar {
    pub year: i32,
    pub month: u32,
    pub days_in_month: u32,
    pub first_weekday: u32, // 0=Sun, 1=Mon, ..., 6=Sat
    pub fire_days: Vec<FireDay>,
    pub total_fires: usize,
}

/// A week view entry: for a given day-of-week, the times when the cron fires.
#[derive(Debug, Clone)]
pub struct WeekDayEntry {
    pub weekday: usize, // 0=Sun..6=Sat
    pub weekday_name: &'static str,
    pub fire_times: Vec<String>,
    pub total_fires: usize,
}

/// A year overview: a compact representation of fire days across all months.
#[derive(Debug, Clone)]
pub struct YearOverview {
    pub year: i32,
    pub months: Vec<MonthCalendar>,
    pub total_fires: usize,
}

/// Build a calendar for a single month.
pub fn build_month_calendar(
    expr: &CronExpr,
    tz: &Tz,
    year: i32,
    month: u32,
) -> MonthCalendar {
    let days_in_month_count = days_in_month(year, month);
    let first_of_month = NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap_or_else(|| panic!("invalid date {year}-{month}-1"));
    let first_weekday = first_of_month.weekday().num_days_from_sunday();

    // Collect all fire times within this month.
    let mut fire_days: Vec<FireDay> = Vec::new();
    let mut total_fires = 0;

    // Start at the last moment of the previous month to include all
    // fire times in this month (next_run is exclusive of `after`).
    let start = if month == 1 {
        tz.with_ymd_and_hms(year - 1, 12, 31, 23, 59, 59)
            .single()
            .unwrap_or_else(|| {
                tz.with_ymd_and_hms(year, month, 1, 0, 0, 0)
                    .single()
                    .unwrap()
            })
    } else {
        tz.with_ymd_and_hms(year, month - 1, days_in_month(year, month - 1), 23, 59, 59)
            .single()
            .unwrap_or_else(|| {
                tz.with_ymd_and_hms(year, month, 1, 0, 0, 0)
                    .single()
                    .unwrap()
            })
    };

    // End of month (exclusive, start of next month).
    let end_naive = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    // We'll use a max iteration guard
    let max_iter = 31_536_000u32; // 1 year's worth of seconds — safety valve

    let mut current = start;
    let mut iteration_count = 0;

    // Find all fire times by repeatedly calling next_run and checking
    // if we're still within the month.
    loop {
        if iteration_count >= max_iter {
            break;
        }
        iteration_count += 1;

        let next = match next_run(expr, current) {
            Ok(t) => t,
            Err(_) => break,
        };

        // Check if we've moved past the end of the month.
        if next.naive_utc().date() >= end_naive {
            break;
        }

        let day = next.day();
        let time_str = next.format("%H:%M:%S").to_string();

        // Find or create the fire day entry.
        if let Some(fd) = fire_days.last_mut() {
            if fd.day == day {
                fd.times.push(time_str);
            } else {
                fire_days.push(FireDay {
                    day,
                    times: vec![time_str],
                });
            }
        } else {
            fire_days.push(FireDay {
                day,
                times: vec![time_str],
            });
        }

        total_fires += 1;
        current = next;
    }

    MonthCalendar {
        year,
        month,
        days_in_month: days_in_month_count,
        first_weekday,
        fire_days,
        total_fires,
    }
}

/// Build a week view showing fire times by day of week.
pub fn build_week_view(
    expr: &CronExpr,
    tz: &Tz,
    year: i32,
    month: u32,
) -> Vec<WeekDayEntry> {
    let cal = build_month_calendar(expr, tz, year, month);

    let mut week_entries = vec![
        WeekDayEntry { weekday: 0, weekday_name: "Sun", fire_times: Vec::new(), total_fires: 0 },
        WeekDayEntry { weekday: 1, weekday_name: "Mon", fire_times: Vec::new(), total_fires: 0 },
        WeekDayEntry { weekday: 2, weekday_name: "Tue", fire_times: Vec::new(), total_fires: 0 },
        WeekDayEntry { weekday: 3, weekday_name: "Wed", fire_times: Vec::new(), total_fires: 0 },
        WeekDayEntry { weekday: 4, weekday_name: "Thu", fire_times: Vec::new(), total_fires: 0 },
        WeekDayEntry { weekday: 5, weekday_name: "Fri", fire_times: Vec::new(), total_fires: 0 },
        WeekDayEntry { weekday: 6, weekday_name: "Sat", fire_times: Vec::new(), total_fires: 0 },
    ];

    // For each fire day, determine the weekday and collect unique fire times.
    for fd in &cal.fire_days {
        let date = NaiveDate::from_ymd_opt(year, month, fd.day)
            .unwrap_or_else(|| panic!("invalid date {year}-{month}-{}", fd.day));
        let dow = date.weekday().num_days_from_sunday() as usize;

        let entry = &mut week_entries[dow];
        for t in &fd.times {
            if !entry.fire_times.contains(t) {
                entry.fire_times.push(t.clone());
            }
        }
        entry.total_fires += fd.times.len();
    }

    // Sort fire times within each weekday.
    for entry in &mut week_entries {
        entry.fire_times.sort();
    }

    week_entries
}

/// Build a year overview.
pub fn build_year_overview(
    expr: &CronExpr,
    tz: &Tz,
    year: i32,
) -> YearOverview {
    let months: Vec<MonthCalendar> = (1..=12)
        .map(|m| build_month_calendar(expr, tz, year, m))
        .collect();

    let total_fires: usize = months.iter().map(|m| m.total_fires).sum();

    YearOverview {
        year,
        months,
        total_fires,
    }
}

/// Format a month calendar as a text string.
pub fn format_month_calendar(cal: &MonthCalendar, expr_raw: &str) -> String {
    let month_names = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let month_name = month_names[cal.month as usize - 1];

    let mut out = String::new();
    out.push_str(&format!(
        "{} {}\n",
        month_name,
        cal.year
    ));
    out.push_str(&format!("Expression: {}\n", expr_raw));
    out.push_str(&format!("Total fires: {}\n\n", cal.total_fires));

    // Calendar header.
    out.push_str("Su Mo Tu We Th Fr Sa\n");

    // Build a 2D grid of (day_number, is_fire_day) tuples.
    let mut grid: Vec<Vec<(Option<u32>, bool)>> = Vec::new();
    let mut current_week: Vec<(Option<u32>, bool)> = Vec::new();

    // Pad the first week with empty cells.
    for _ in 0..cal.first_weekday {
        current_week.push((None, false));
    }

    // Collect fire day numbers for quick lookup.
    let fire_day_nums: std::collections::HashSet<u32> =
        cal.fire_days.iter().map(|fd| fd.day).collect();

    for day in 1..=cal.days_in_month {
        let is_fire = fire_day_nums.contains(&day);
        current_week.push((Some(day), is_fire));

        if current_week.len() == 7 {
            grid.push(current_week);
            current_week = Vec::new();
        }
    }

    // Pad the last week.
    while current_week.len() < 7 {
        current_week.push((None, false));
    }
    grid.push(current_week);

    // Render the grid.
    for week in &grid {
        for cell in week {
            match cell {
                (Some(day), true) => {
                    // Highlight fire days with bold/inverse (ANSI).
                    out.push_str(&format!("\x1b[1;7m{:>2}\x1b[0m ", day));
                }
                (Some(day), false) => {
                    out.push_str(&format!("{:>2} ", day));
                }
                (None, _) => {
                    out.push_str("   ");
                }
            }
        }
        out.push('\n');
    }

    // Legend.
    out.push_str("\n\x1b[1;7m  \x1b[0m = fire day\n");

    out
}

/// Format a month calendar as JSON.
pub fn format_month_calendar_json(cal: &MonthCalendar, expr_raw: &str) -> String {
    #[derive(serde::Serialize)]
    struct CalendarJson {
        expression: String,
        year: i32,
        month: u32,
        month_name: String,
        total_fires: usize,
        days_in_month: u32,
        first_weekday: u32,
        fire_days: Vec<FireDayJson>,
    }

    #[derive(serde::Serialize)]
    struct FireDayJson {
        day: u32,
        times: Vec<String>,
    }

    let month_names = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];

    let json = CalendarJson {
        expression: expr_raw.to_string(),
        year: cal.year,
        month: cal.month,
        month_name: month_names[cal.month as usize - 1].to_string(),
        total_fires: cal.total_fires,
        days_in_month: cal.days_in_month,
        first_weekday: cal.first_weekday,
        fire_days: cal
            .fire_days
            .iter()
            .map(|fd| FireDayJson {
                day: fd.day,
                times: fd.times.clone(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
}

/// Format a week view as text.
pub fn format_week_view(entries: &[WeekDayEntry], expr_raw: &str, year: i32, month: u32) -> String {
    let month_names = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let month_name = month_names[month as usize - 1];

    let mut out = String::new();
    out.push_str(&format!(
        "Week View — {} {}\n",
        month_name, year
    ));
    out.push_str(&format!("Expression: {}\n\n", expr_raw));

    for entry in entries {
        let day_label = format!("{:>3}", entry.weekday_name);
        if entry.fire_times.is_empty() {
            out.push_str(&format!("{}  —\n", day_label));
        } else {
            out.push_str(&format!(
                "{}  {}\n",
                day_label,
                entry.fire_times.join(", ")
            ));
        }
    }

    out
}

/// Format a week view as JSON.
pub fn format_week_view_json(
    entries: &[WeekDayEntry],
    expr_raw: &str,
    year: i32,
    month: u32,
) -> String {
    #[derive(serde::Serialize)]
    struct WeekViewJson {
        expression: String,
        year: i32,
        month: u32,
        days: Vec<WeekDayJson>,
    }

    #[derive(serde::Serialize)]
    struct WeekDayJson {
        weekday: usize,
        name: String,
        fire_times: Vec<String>,
        total_fires: usize,
    }

    let json = WeekViewJson {
        expression: expr_raw.to_string(),
        year,
        month,
        days: entries
            .iter()
            .map(|e| WeekDayJson {
                weekday: e.weekday,
                name: e.weekday_name.to_string(),
                fire_times: e.fire_times.clone(),
                total_fires: e.total_fires,
            })
            .collect(),
    };

    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
}

/// Format a year overview as text.
pub fn format_year_overview(overview: &YearOverview, expr_raw: &str) -> String {
    let month_abbr = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let mut out = String::new();
    out.push_str(&format!(
        "Year Overview — {}\n",
        overview.year
    ));
    out.push_str(&format!("Expression: {}\n", expr_raw));
    out.push_str(&format!("Total fires: {}\n\n", overview.total_fires));

    // Render each month as a mini calendar row.
    for cal in &overview.months {
        let fire_count = cal.fire_days.len();
        let fire_pct = if cal.days_in_month > 0 {
            (fire_count as f64 / cal.days_in_month as f64 * 100.0).round() as u32
        } else {
            0
        };

        out.push_str(&format!("{} {:>2}: ", month_abbr[cal.month as usize - 1], cal.year));

        let fire_day_nums: std::collections::HashSet<u32> =
            cal.fire_days.iter().map(|fd| fd.day).collect();

        for day in 1..=cal.days_in_month {
            if fire_day_nums.contains(&day) {
                out.push_str("\x1b[1;7m.\x1b[0m");
            } else {
                out.push('.');
            }
        }

        out.push_str(&format!(
            "  {} days, {}% busy\n",
            fire_count, fire_pct
        ));
    }

    out
}

/// Format a year overview as JSON.
pub fn format_year_overview_json(overview: &YearOverview, expr_raw: &str) -> String {
    #[derive(serde::Serialize)]
    struct YearOverviewJson {
        expression: String,
        year: i32,
        total_fires: usize,
        months: Vec<MonthSummaryJson>,
    }

    #[derive(serde::Serialize)]
    struct MonthSummaryJson {
        month: u32,
        month_name: String,
        days_in_month: u32,
        fire_days: Vec<u32>,
        fire_count: usize,
        busy_pct: f64,
    }

    let month_names = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];

    let json = YearOverviewJson {
        expression: expr_raw.to_string(),
        year: overview.year,
        total_fires: overview.total_fires,
        months: overview
            .months
            .iter()
            .map(|cal| {
                let fire_day_nums: Vec<u32> = cal.fire_days.iter().map(|fd| fd.day).collect();
            let busy_pct = if cal.days_in_month > 0 {
                fire_day_nums.len() as f64 / cal.days_in_month as f64 * 100.0
            } else {
                0.0
            };
            let fire_count = fire_day_nums.len();
            MonthSummaryJson {
                month: cal.month,
                month_name: month_names[cal.month as usize - 1].to_string(),
                days_in_month: cal.days_in_month,
                fire_days: fire_day_nums,
                fire_count,
                busy_pct,
            }
            })
            .collect(),
    };

    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    #[test]
    fn test_build_month_calendar_daily() {
        let expr = crate::expr::parse_cron("0 0 * * *").unwrap();
        let tz = chrono_tz::UTC;
        let cal = build_month_calendar(&expr, &tz, 2026, 7);
        // Daily at midnight — should fire every day of July (31 days).
        assert_eq!(cal.fire_days.len(), 31);
        assert_eq!(cal.total_fires, 31);
        assert_eq!(cal.days_in_month, 31);
    }

    #[test]
    fn test_build_month_calendar_weekly() {
        let expr = crate::expr::parse_cron("0 0 * * 0").unwrap(); // Every Sunday
        let tz = chrono_tz::UTC;
        let cal = build_month_calendar(&expr, &tz, 2026, 7);
        // July 2026: Sundays are 5, 12, 19, 26
        assert!(cal.total_fires > 0);
        assert!(cal.total_fires <= 5);
        // Every fire day should be a Sunday
        for fd in &cal.fire_days {
            let date = NaiveDate::from_ymd_opt(2026, 7, fd.day).unwrap();
            assert_eq!(date.weekday(), Weekday::Sun);
        }
    }

    #[test]
    fn test_build_month_calendar_no_fires() {
        // Expression that fires only in February.
        let expr = crate::expr::parse_cron("0 0 29 2 *").unwrap(); // Feb 29
        let tz = chrono_tz::UTC;
        let cal = build_month_calendar(&expr, &tz, 2026, 7); // July — no fires
        assert_eq!(cal.fire_days.len(), 0);
        assert_eq!(cal.total_fires, 0);
    }

    #[test]
    fn test_build_week_view() {
        let expr = crate::expr::parse_cron("0 9 * * 1-5").unwrap(); // Weekdays at 9am
        let tz = chrono_tz::UTC;
        let entries = build_week_view(&expr, &tz, 2026, 7);
        // Weekdays (Mon-Fri) should have fire times, weekends should not.
        assert!(entries[1].total_fires > 0); // Mon
        assert!(entries[2].total_fires > 0); // Tue
        assert!(entries[3].total_fires > 0); // Wed
        assert!(entries[4].total_fires > 0); // Thu
        assert!(entries[5].total_fires > 0); // Fri
        assert_eq!(entries[0].fire_times, Vec::<String>::new()); // Sun
        assert_eq!(entries[6].fire_times, Vec::<String>::new()); // Sat
    }

    #[test]
    fn test_build_year_overview() {
        let expr = crate::expr::parse_cron("0 0 1 * *").unwrap(); // 1st of every month
        let tz = chrono_tz::UTC;
        let overview = build_year_overview(&expr, &tz, 2026);
        assert_eq!(overview.months.len(), 12);
        assert_eq!(overview.total_fires, 12);
        for cal in &overview.months {
            assert_eq!(cal.fire_days.len(), 1);
            assert_eq!(cal.fire_days[0].day, 1);
        }
    }

    #[test]
    fn test_format_month_calendar_text() {
        let expr = crate::expr::parse_cron("0 12 * * *").unwrap();
        let tz = chrono_tz::UTC;
        let cal = build_month_calendar(&expr, &tz, 2026, 7);
        let text = format_month_calendar(&cal, "0 12 * * *");
        assert!(text.contains("July 2026"));
        assert!(text.contains("0 12 * * *"));
        assert!(text.contains("Su Mo Tu We Th Fr Sa"));
        assert!(text.contains("fire day"));
    }

    #[test]
    fn test_format_month_calendar_json() {
        let expr = crate::expr::parse_cron("0 0 * * *").unwrap();
        let tz = chrono_tz::UTC;
        let cal = build_month_calendar(&expr, &tz, 2026, 7);
        let json = format_month_calendar_json(&cal, "0 0 * * *");
        assert!(json.contains("fire_days"));
        assert!(json.contains("expression"));
    }

    #[test]
    fn test_format_week_view() {
        let expr = crate::expr::parse_cron("0 9 * * 1-5").unwrap();
        let tz = chrono_tz::UTC;
        let entries = build_week_view(&expr, &tz, 2026, 7);
        let text = format_week_view(&entries, "0 9 * * 1-5", 2026, 7);
        assert!(text.contains("Week View"));
        assert!(text.contains("Mon"));
        assert!(text.contains("Fri"));
    }

    #[test]
    fn test_format_year_overview() {
        let expr = crate::expr::parse_cron("0 0 1 * *").unwrap();
        let tz = chrono_tz::UTC;
        let overview = build_year_overview(&expr, &tz, 2026);
        let text = format_year_overview(&overview, "0 0 1 * *");
        assert!(text.contains("Year Overview"));
        assert!(text.contains("2026"));
        assert!(text.contains("12"));
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29); // Leap year
        assert_eq!(days_in_month(2026, 4), 30);
    }
}