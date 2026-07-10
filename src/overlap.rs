//! Schedule overlap detection.
//!
//! Given multiple cron expressions, find time windows where two or more
//! schedules fire at the same instant. This is useful for detecting
//! resource contention between scheduled jobs.

use chrono::{DateTime, TimeZone};

use crate::evaluator::next_run;
use crate::expr::CronExpr;

/// A named cron schedule used for overlap analysis.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub name: String,
    pub expr: CronExpr,
}

impl Schedule {
    pub fn new(name: impl Into<String>, expr: CronExpr) -> Self {
        Self {
            name: name.into(),
            expr,
        }
    }
}

/// A detected overlap between two or more schedules at a specific time.
#[derive(Debug, Clone)]
pub struct Overlap {
    /// The time at which the overlap occurs.
    pub time: chrono::DateTime<chrono_tz::Tz>,
    /// The names of all schedules that fire at this time.
    pub schedules: Vec<String>,
}

/// Find overlaps between the given schedules within a time window.
///
/// Scans from `start` for up to `window` seconds, collecting all times at
/// which two or more schedules fire simultaneously. Returns overlaps
/// sorted by time.
pub fn find_overlaps<Tz: TimeZone>(
    schedules: &[Schedule],
    start: DateTime<Tz>,
    window_seconds: i64,
) -> Result<Vec<Overlap>, String>
where
    Tz::Offset: std::fmt::Display,
{
    if schedules.len() < 2 {
        return Ok(Vec::new());
    }

    let mut events: Vec<(DateTime<Tz>, usize)> = Vec::new();

    let deadline = start.clone() + chrono::Duration::seconds(window_seconds);
    for (idx, sched) in schedules.iter().enumerate() {
        let mut current = start.clone();
        let mut guard = 0u32;
        loop {
            guard += 1;
            if guard > 100_000 {
                break;
            }
            let next = match next_run(&sched.expr, current) {
                Ok(t) => t,
                Err(_) => break,
            };
            if next > deadline {
                break;
            }
            current = next.clone();
            events.push((next, idx));
        }
    }

    // Sort by time.
    events.sort_by_key(|(t, _)| t.clone());

    // Group events that occur at the same instant.
    let mut overlaps = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let time = &events[i].0;
        let mut group = vec![events[i].1];
        let mut j = i + 1;
        while j < events.len() && &events[j].0 == time {
            group.push(events[j].1);
            j += 1;
        }
        if group.len() >= 2 {
            let mut names: Vec<String> = group
                .iter()
                .map(|&idx| schedules[idx].name.clone())
                .collect();
            names.sort();
            names.dedup();
            overlaps.push(Overlap {
                time: time.clone().with_timezone(&chrono_tz::UTC),
                schedules: names,
            });
        }
        i = j;
    }

    Ok(overlaps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn parse(input: &str) -> CronExpr {
        crate::expr::parse_cron(input).unwrap()
    }

    #[test]
    fn overlap_two_schedules() {
        // Both fire at 00:00 every day.
        let s1 = Schedule::new("job-a", parse("0 0 * * *"));
        let s2 = Schedule::new("job-b", parse("0 0 * * *"));
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let overlaps = find_overlaps(&[s1, s2], start, 7 * 24 * 3600).unwrap();
        // 7 days → 7 overlaps at midnight.
        assert_eq!(overlaps.len(), 7);
        assert!(overlaps[0].schedules.contains(&"job-a".to_string()));
        assert!(overlaps[0].schedules.contains(&"job-b".to_string()));
    }

    #[test]
    fn no_overlap() {
        let s1 = Schedule::new("job-a", parse("0 0 * * *")); // midnight
        let s2 = Schedule::new("job-b", parse("0 12 * * *")); // noon
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let overlaps = find_overlaps(&[s1, s2], start, 7 * 24 * 3600).unwrap();
        assert!(overlaps.is_empty());
    }

    #[test]
    fn overlap_partial() {
        // job-a fires every 30 min, job-b fires every 60 min → overlap at :00.
        let s1 = Schedule::new("frequent", parse("*/30 * * * *"));
        let s2 = Schedule::new("hourly", parse("0 * * * *"));
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let overlaps = find_overlaps(&[s1, s2], start, 3 * 3600).unwrap();
        // 3 hours → 3 overlaps (00:00, 01:00, 02:00).
        assert_eq!(overlaps.len(), 3);
    }

    #[test]
    fn single_schedule_no_overlap() {
        let s1 = Schedule::new("only", parse("0 0 * * *"));
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let overlaps = find_overlaps(&[s1], start, 3600).unwrap();
        assert!(overlaps.is_empty());
    }
}
