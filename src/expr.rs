//! The full cron expression: combining all fields into a [`CronExpr`].

use crate::field::{expand_aliases, parse_field, FieldKind, FieldSpec};

/// The format/flavour of a cron expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CronFlavor {
    /// Standard 5-field Vixie cron: `minute hour dom month dow`.
    Standard5,
    /// 6-field cron with a leading seconds field: `second minute hour dom month dow`.
    Seconds6,
    /// 7-field Quartz-style cron: `second minute hour dom month dow year`.
    Quartz7,
}

impl CronFlavor {
    pub fn field_count(self) -> usize {
        match self {
            CronFlavor::Standard5 => 5,
            CronFlavor::Seconds6 => 6,
            CronFlavor::Quartz7 => 7,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CronFlavor::Standard5 => "5-field (standard)",
            CronFlavor::Seconds6 => "6-field (with seconds)",
            CronFlavor::Quartz7 => "7-field (Quartz, with year)",
        }
    }
}

/// A fully parsed cron expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    pub raw: String,
    pub flavor: CronFlavor,
    pub second: FieldSpec,
    pub minute: FieldSpec,
    pub hour: FieldSpec,
    pub day_of_month: FieldSpec,
    pub month: FieldSpec,
    pub day_of_week: FieldSpec,
    pub year: FieldSpec,
}

impl CronExpr {
    /// Whether the seconds field is present (6/7-field expressions).
    pub fn has_seconds(&self) -> bool {
        self.flavor != CronFlavor::Standard5
    }

    /// Whether the year field is present (7-field expressions).
    pub fn has_year(&self) -> bool {
        self.flavor == CronFlavor::Quartz7
    }

    /// Returns `true` when the day-of-month field is restricted (not `*` and
    /// not `?`).
    pub fn dom_restricted(&self) -> bool {
        !self.day_of_month.is_wildcard() && !self.day_of_month.is_question()
    }

    /// Returns `true` when the day-of-week field is restricted (not `*` and
    /// not `?`).
    pub fn dow_restricted(&self) -> bool {
        !self.day_of_week.is_wildcard() && !self.day_of_week.is_question()
    }
}

/// Parse a cron expression string into a [`CronExpr`].
///
/// Automatically detects the flavour based on the number of whitespace-
/// separated fields:
/// - 5 fields → [`CronFlavor::Standard5`]
/// - 6 fields → [`CronFlavor::Seconds6`]
/// - 7 fields → [`CronFlavor::Quartz7`]
pub fn parse_cron(input: &str) -> Result<CronExpr, String> {
    let raw = input.trim().to_string();
    if raw.is_empty() {
        return Err("cron expression is empty".to_string());
    }

    let parts: Vec<&str> = raw.split_whitespace().collect();
    let flavor = match parts.len() {
        5 => CronFlavor::Standard5,
        6 => CronFlavor::Seconds6,
        7 => CronFlavor::Quartz7,
        n => {
            return Err(format!(
                "expected 5, 6, or 7 fields but found {n} in '{raw}'"
            ));
        }
    };

    // Map parts to fields depending on flavour.
    let (second, minute, hour, dom, month, dow, year) = match flavor {
        CronFlavor::Standard5 => {
            let minute = parse_field(parts[0], FieldKind::Minute)?;
            let hour = parse_field(parts[1], FieldKind::Hour)?;
            let dom = parse_field(parts[2], FieldKind::DayOfMonth)?;
            let month = parse_field(
                &expand_aliases(parts[3], FieldKind::Month),
                FieldKind::Month,
            )?;
            let dow = parse_field(
                &expand_aliases(parts[4], FieldKind::DayOfWeek),
                FieldKind::DayOfWeek,
            )?;
            (
                FieldSpec {
                    raw: "0".to_string(),
                    kind: FieldKind::Second,
                    terms: vec![crate::field::Term::Single(0)],
                },
                minute,
                hour,
                dom,
                month,
                dow,
                FieldSpec {
                    raw: "*".to_string(),
                    kind: FieldKind::Year,
                    terms: vec![crate::field::Term::Wildcard],
                },
            )
        }
        CronFlavor::Seconds6 => {
            let second = parse_field(parts[0], FieldKind::Second)?;
            let minute = parse_field(parts[1], FieldKind::Minute)?;
            let hour = parse_field(parts[2], FieldKind::Hour)?;
            let dom = parse_field(parts[3], FieldKind::DayOfMonth)?;
            let month = parse_field(
                &expand_aliases(parts[4], FieldKind::Month),
                FieldKind::Month,
            )?;
            let dow = parse_field(
                &expand_aliases(parts[5], FieldKind::DayOfWeek),
                FieldKind::DayOfWeek,
            )?;
            (
                second,
                minute,
                hour,
                dom,
                month,
                dow,
                FieldSpec {
                    raw: "*".to_string(),
                    kind: FieldKind::Year,
                    terms: vec![crate::field::Term::Wildcard],
                },
            )
        }
        CronFlavor::Quartz7 => {
            let second = parse_field(parts[0], FieldKind::Second)?;
            let minute = parse_field(parts[1], FieldKind::Minute)?;
            let hour = parse_field(parts[2], FieldKind::Hour)?;
            let dom = parse_field(parts[3], FieldKind::DayOfMonth)?;
            let month = parse_field(
                &expand_aliases(parts[4], FieldKind::Month),
                FieldKind::Month,
            )?;
            let dow = parse_field(
                &expand_aliases(parts[5], FieldKind::DayOfWeek),
                FieldKind::DayOfWeek,
            )?;
            let year = parse_field(parts[6], FieldKind::Year)?;
            (second, minute, hour, dom, month, dow, year)
        }
    };

    // Quartz `?` validation: when using `?` in one day field, the other day
    // field should be restricted (not `?` and not `*`).
    if dom.is_question() && dow.is_question() {
        return Err(
            "both day-of-month and day-of-week are '?' — at least one must be specified"
                .to_string(),
        );
    }

    Ok(CronExpr {
        raw,
        flavor,
        second,
        minute,
        hour,
        day_of_month: dom,
        month,
        day_of_week: dow,
        year,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard5() {
        let expr = parse_cron("*/5 * * * *").unwrap();
        assert_eq!(expr.flavor, CronFlavor::Standard5);
        assert!(!expr.has_seconds());
        assert!(!expr.has_year());
    }

    #[test]
    fn parse_seconds6() {
        let expr = parse_cron("0 */5 * * * *").unwrap();
        assert_eq!(expr.flavor, CronFlavor::Seconds6);
        assert!(expr.has_seconds());
        assert!(!expr.has_year());
    }

    #[test]
    fn parse_quartz7() {
        let expr = parse_cron("0 0 12 * * ? 2026").unwrap();
        assert_eq!(expr.flavor, CronFlavor::Quartz7);
        assert!(expr.has_seconds());
        assert!(expr.has_year());
    }

    #[test]
    fn parse_with_month_names() {
        let expr = parse_cron("0 0 1 JAN,JUL MON").unwrap();
        assert_eq!(expr.month.numeric_values(), vec![1, 7]);
        assert_eq!(expr.day_of_week.numeric_values(), vec![1]);
    }

    #[test]
    fn parse_invalid_field_count() {
        assert!(parse_cron("* * *").is_err());
        assert!(parse_cron("* * * * * * * *").is_err());
    }

    #[test]
    fn parse_empty() {
        assert!(parse_cron("").is_err());
        assert!(parse_cron("   ").is_err());
    }

    #[test]
    fn parse_double_question() {
        assert!(parse_cron("0 0 12 ? * ?").is_err());
    }

    #[test]
    fn dom_dow_restricted() {
        let expr = parse_cron("0 0 1 * *").unwrap();
        assert!(expr.dom_restricted());
        assert!(!expr.dow_restricted());

        let expr = parse_cron("0 0 * * 1").unwrap();
        assert!(!expr.dom_restricted());
        assert!(expr.dow_restricted());

        let expr = parse_cron("0 0 1 * 1").unwrap();
        assert!(expr.dom_restricted());
        assert!(expr.dow_restricted());
    }
}
