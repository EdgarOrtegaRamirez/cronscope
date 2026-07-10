//! Individual cron field parsing.
//!
//! Each field in a cron expression (second, minute, hour, day-of-month,
//! month, day-of-week, year) is parsed into a [`FieldSpec`] containing one
//! or more [`Term`]s.

use std::fmt;

/// The kind of a cron field, used to determine valid value ranges and
/// which special modifiers are permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Second,
    Minute,
    Hour,
    DayOfMonth,
    Month,
    DayOfWeek,
    Year,
}

impl FieldKind {
    /// Returns the inclusive `(min, max)` range of plain numeric values for
    /// this field kind (special modifiers like `L`/`W`/`#` are excluded).
    pub fn numeric_range(self) -> (u32, u32) {
        match self {
            FieldKind::Second => (0, 59),
            FieldKind::Minute => (0, 59),
            FieldKind::Hour => (0, 23),
            FieldKind::DayOfMonth => (1, 31),
            FieldKind::Month => (1, 12),
            FieldKind::DayOfWeek => (0, 6),
            FieldKind::Year => (1970, 2100),
        }
    }

    /// Human-readable name of the field.
    pub fn name(self) -> &'static str {
        match self {
            FieldKind::Second => "second",
            FieldKind::Minute => "minute",
            FieldKind::Hour => "hour",
            FieldKind::DayOfMonth => "day-of-month",
            FieldKind::Month => "month",
            FieldKind::DayOfWeek => "day-of-week",
            FieldKind::Year => "year",
        }
    }
}

/// A single term within a cron field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// `*` — matches every value in the field's range.
    Wildcard,
    /// A single value, e.g. `5`.
    Single(u32),
    /// An inclusive range, e.g. `1-5`.
    Range(u32, u32),
    /// A step from a starting value, e.g. `*/15` or `0/15`.
    Step { from: u32, step: u32 },
    /// A stepped range, e.g. `1-10/2`.
    RangeStep { from: u32, to: u32, step: u32 },
    /// `L` — last day of the month (day-of-month only).
    Last,
    /// `nL` — last occurrence of weekday `n` in the month (day-of-week only).
    LastWeekday(u32),
    /// `L-n` — last day of the month minus `n` days (day-of-month only).
    LastOffset(u32),
    /// `nW` — nearest weekday to day `n` (day-of-month only).
    NearestWeekday(u32),
    /// `n#m` — the `m`-th occurrence of weekday `n` in the month (day-of-week only).
    NthWeekday { weekday: u32, occurrence: u32 },
    /// `?` — no specific value (Quartz, day-of-month/day-of-week only).
    Question,
}

impl Term {
    /// Whether this term is a wildcard (`*`) — meaning the field is
    /// "unrestricted".
    pub fn is_wildcard(&self) -> bool {
        matches!(self, Term::Wildcard)
    }

    /// Whether this term is a `?` (no specific value).
    pub fn is_question(&self) -> bool {
        matches!(self, Term::Question)
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Wildcard => write!(f, "*"),
            Term::Single(v) => write!(f, "{v}"),
            Term::Range(a, b) => write!(f, "{a}-{b}"),
            Term::Step { from, step } => write!(f, "{from}/{step}"),
            Term::RangeStep { from, to, step } => write!(f, "{from}-{to}/{step}"),
            Term::Last => write!(f, "L"),
            Term::LastWeekday(v) => write!(f, "{v}L"),
            Term::LastOffset(n) => write!(f, "L-{n}"),
            Term::NearestWeekday(v) => write!(f, "{v}W"),
            Term::NthWeekday {
                weekday,
                occurrence,
            } => write!(f, "{weekday}#{occurrence}"),
            Term::Question => write!(f, "?"),
        }
    }
}

/// A parsed cron field: the raw text plus the list of terms it contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSpec {
    pub raw: String,
    pub kind: FieldKind,
    pub terms: Vec<Term>,
}

impl FieldSpec {
    /// Returns `true` when the field is fully unrestricted — either `*` or
    /// a single `*` term.
    pub fn is_wildcard(&self) -> bool {
        self.terms.len() == 1 && self.terms[0].is_wildcard()
    }

    /// Returns `true` when the field is `?` (Quartz no-specific-value).
    pub fn is_question(&self) -> bool {
        self.terms.len() == 1 && self.terms[0].is_question()
    }

    /// Returns `true` when the field contains any special modifier
    /// (`L`, `W`, `#`).
    pub fn has_special(&self) -> bool {
        self.terms.iter().any(|t| {
            matches!(
                t,
                Term::Last
                    | Term::LastWeekday(_)
                    | Term::LastOffset(_)
                    | Term::NearestWeekday(_)
                    | Term::NthWeekday { .. }
            )
        })
    }

    /// Expand the plain-numeric terms of this field into the set of values
    /// they represent within the field's numeric range.
    ///
    /// Special modifiers (`L`, `W`, `#`) are *not* expanded here — they are
    /// context-dependent and handled by the evaluator.
    pub fn numeric_values(&self) -> Vec<u32> {
        let (min, max) = self.kind.numeric_range();
        let mut values = Vec::new();
        for term in &self.terms {
            match term {
                Term::Wildcard => values.extend(min..=max),
                Term::Single(v) => values.push(*v),
                Term::Range(a, b) => values.extend(*a..=*b),
                Term::Step { from, step } => {
                    let mut v = *from;
                    while v <= max {
                        values.push(v);
                        v = v.saturating_add(*step);
                    }
                }
                Term::RangeStep { from, to, step } => {
                    let mut v = *from;
                    while v <= *to {
                        values.push(v);
                        v = v.saturating_add(*step);
                    }
                }
                // Special modifiers are not numeric-expandable.
                _ => {}
            }
        }
        values.sort_unstable();
        values.dedup();
        values
    }
}

/// Parse a single cron field given its raw text and [`FieldKind`].
pub fn parse_field(raw: &str, kind: FieldKind) -> Result<FieldSpec, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(format!("{} field is empty", kind.name()));
    }

    let mut terms = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!(
                "{} field has an empty list element in '{}'",
                kind.name(),
                raw
            ));
        }
        terms.push(parse_term(part, kind)?);
    }

    Ok(FieldSpec {
        raw: raw.to_string(),
        kind,
        terms,
    })
}

/// Parse a single comma-separated term of a cron field.
fn parse_term(text: &str, kind: FieldKind) -> Result<Term, String> {
    let text = text.trim();
    let (min, _max) = kind.numeric_range();

    // Wildcard.
    if text == "*" {
        return Ok(Term::Wildcard);
    }
    // Quartz no-specific-value.
    if text == "?" {
        if !matches!(kind, FieldKind::DayOfMonth | FieldKind::DayOfWeek) {
            return Err(format!(
                "'?' is only valid in day-of-month or day-of-week fields, not {}",
                kind.name()
            ));
        }
        return Ok(Term::Question);
    }

    // `L` — last day of month (day-of-month only).
    if text == "L" {
        if kind != FieldKind::DayOfMonth {
            return Err(format!(
                "'L' is only valid in the day-of-month field, not {}",
                kind.name()
            ));
        }
        return Ok(Term::Last);
    }

    // `L-n` — last day minus offset (day-of-month only).
    if let Some(rest) = text.strip_prefix("L-") {
        if kind != FieldKind::DayOfMonth {
            return Err("'L-n' is only valid in the day-of-month field".to_string());
        }
        let n: u32 = rest
            .parse()
            .map_err(|_| format!("invalid offset in '{}'", text))?;
        if n == 0 {
            return Err(format!("'L-n' offset must be >= 1, got 0 in '{}'", text));
        }
        return Ok(Term::LastOffset(n));
    }

    // `nL` — last occurrence of weekday n (day-of-week only).
    if text.ends_with('L') && text.len() >= 2 {
        if kind != FieldKind::DayOfWeek {
            return Err("'nL' is only valid in the day-of-week field".to_string());
        }
        let n: u32 = text[..text.len() - 1]
            .parse()
            .map_err(|_| format!("invalid weekday in '{}'", text))?;
        // Day-of-week: 0-6 (0=Sunday). Some systems use 1-7 (7=Sunday).
        let n = if n == 7 { 0 } else { n };
        if n > 6 {
            return Err(format!("'nL' weekday must be 0-7, got {n} in '{}'", text));
        }
        return Ok(Term::LastWeekday(n));
    }

    // `nW` — nearest weekday to day n (day-of-month only).
    if text.ends_with('W') && text.len() >= 2 {
        if kind != FieldKind::DayOfMonth {
            return Err("'nW' is only valid in the day-of-month field".to_string());
        }
        let n: u32 = text[..text.len() - 1]
            .parse()
            .map_err(|_| format!("invalid day in '{}'", text))?;
        if !(1..=31).contains(&n) {
            return Err(format!("'nW' day must be 1-31, got {n} in '{}'", text));
        }
        return Ok(Term::NearestWeekday(n));
    }

    // `n#m` — nth occurrence of weekday n (day-of-week only).
    if let Some(hash_pos) = text.find('#') {
        if kind != FieldKind::DayOfWeek {
            return Err("'#' is only valid in the day-of-week field".to_string());
        }
        let weekday_str = &text[..hash_pos];
        let occ_str = &text[hash_pos + 1..];
        let weekday: u32 = weekday_str
            .parse()
            .map_err(|_| format!("invalid weekday in '{}'", text))?;
        let occurrence: u32 = occ_str
            .parse()
            .map_err(|_| format!("invalid occurrence in '{}'", text))?;
        let weekday = if weekday == 7 { 0 } else { weekday };
        if weekday > 6 {
            return Err(format!(
                "'#' weekday must be 0-7, got {weekday} in '{}'",
                text
            ));
        }
        if !(1..=5).contains(&occurrence) {
            return Err(format!(
                "'#' occurrence must be 1-5, got {occurrence} in '{}'",
                text
            ));
        }
        return Ok(Term::NthWeekday {
            weekday,
            occurrence,
        });
    }

    // Step expressions: `from/step` or `from-to/step`.
    if let Some(slash_pos) = text.find('/') {
        let base = &text[..slash_pos];
        let step_str = &text[slash_pos + 1..];
        let step: u32 = step_str
            .parse()
            .map_err(|_| format!("invalid step value in '{}'", text))?;
        if step == 0 {
            return Err(format!("step value must be >= 1, got 0 in '{}'", text));
        }

        if base == "*" {
            return Ok(Term::Step { from: min, step });
        }
        if let Some(dash_pos) = base.find('-') {
            let from: u32 = base[..dash_pos]
                .parse()
                .map_err(|_| format!("invalid range start in '{}'", text))?;
            let to: u32 = base[dash_pos + 1..]
                .parse()
                .map_err(|_| format!("invalid range end in '{}'", text))?;
            let from = resolve_alias(from, kind)?;
            let to = resolve_alias(to, kind)?;
            if from > to {
                return Err(format!("range start > end in '{}'", text));
            }
            check_range(from, to, kind, text)?;
            return Ok(Term::RangeStep { from, to, step });
        }
        let from: u32 = base
            .parse()
            .map_err(|_| format!("invalid value in '{}'", text))?;
        let from = resolve_alias(from, kind)?;
        check_single(from, kind, text)?;
        return Ok(Term::Step { from, step });
    }

    // Plain range: `from-to`.
    if let Some(dash_pos) = text.find('-') {
        // Guard against negative-looking values; cron has none.
        let from_str = &text[..dash_pos];
        let to_str = &text[dash_pos + 1..];
        let from: u32 = from_str
            .parse()
            .map_err(|_| format!("invalid range start in '{}'", text))?;
        let to: u32 = to_str
            .parse()
            .map_err(|_| format!("invalid range end in '{}'", text))?;
        let from = resolve_alias(from, kind)?;
        let to = resolve_alias(to, kind)?;
        if from > to {
            return Err(format!("range start > end in '{}'", text));
        }
        check_range(from, to, kind, text)?;
        return Ok(Term::Range(from, to));
    }

    // Single value.
    let v: u32 = text
        .parse()
        .map_err(|_| format!("invalid value '{}' in {} field", text, kind.name()))?;
    let v = resolve_alias(v, kind)?;
    check_single(v, kind, text)?;
    Ok(Term::Single(v))
}

/// Resolve day-of-week / month name aliases to numeric values.
///
/// Supports `SUN`..`SAT` (0-6, 7→0) and `JAN`..`DEC` (1-12), case-insensitive.
fn resolve_alias(v: u32, kind: FieldKind) -> Result<u32, String> {
    match kind {
        FieldKind::DayOfWeek => {
            // 7 is an alias for 0 (Sunday) in many cron implementations.
            if v == 7 {
                Ok(0)
            } else if v <= 6 {
                Ok(v)
            } else {
                Err(format!("day-of-week value must be 0-7, got {v}"))
            }
        }
        _ => Ok(v),
    }
}

fn check_single(v: u32, kind: FieldKind, ctx: &str) -> Result<(), String> {
    let (min, max) = kind.numeric_range();
    if v < min || v > max {
        return Err(format!(
            "{} value must be {min}-{max}, got {v} in '{}'",
            kind.name(),
            ctx
        ));
    }
    Ok(())
}

fn check_range(from: u32, to: u32, kind: FieldKind, ctx: &str) -> Result<(), String> {
    let (min, max) = kind.numeric_range();
    if from < min || from > max {
        return Err(format!(
            "{} range start must be {min}-{max}, got {from} in '{}'",
            kind.name(),
            ctx
        ));
    }
    if to < min || to > max {
        return Err(format!(
            "{} range end must be {min}-{max}, got {to} in '{}'",
            kind.name(),
            ctx
        ));
    }
    Ok(())
}

/// Parse a month name (`JAN`..`DEC`) or day-of-week name (`SUN`..`SAT`) into
/// its numeric value. Returns `None` if the text is not a recognised alias.
pub fn parse_name_alias(text: &str) -> Option<u32> {
    let upper = text.trim().to_ascii_uppercase();
    let months = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    let days = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
    if let Some(idx) = months.iter().position(|m| *m == upper) {
        return Some(idx as u32 + 1);
    }
    if let Some(idx) = days.iter().position(|d| *d == upper) {
        return Some(idx as u32);
    }
    None
}

/// Pre-process a field string, replacing named aliases (JAN-DEC, SUN-SAT)
/// with their numeric equivalents so the numeric parser can handle them.
pub fn expand_aliases(raw: &str, kind: FieldKind) -> String {
    if !matches!(kind, FieldKind::Month | FieldKind::DayOfWeek) {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut token = String::new();
    let flush_token = |token: &mut String, out: &mut String, _kind: FieldKind| {
        if token.is_empty() {
            return;
        }
        if let Some(n) = parse_name_alias(token) {
            // For day-of-week, 7 maps to 0 — but aliases never produce 7.
            out.push_str(&n.to_string());
        } else {
            // Not an alias — keep as-is (may be a number or special char).
            out.push_str(token);
        }
        token.clear();
    };
    for ch in raw.chars() {
        if ch.is_ascii_alphabetic() {
            token.push(ch);
        } else {
            flush_token(&mut token, &mut out, kind);
            out.push(ch);
        }
    }
    flush_token(&mut token, &mut out, kind);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wildcard() {
        let f = parse_field("*", FieldKind::Minute).unwrap();
        assert!(f.is_wildcard());
        assert_eq!(f.numeric_values().len(), 60);
    }

    #[test]
    fn parse_single() {
        let f = parse_field("5", FieldKind::Minute).unwrap();
        assert_eq!(f.terms, vec![Term::Single(5)]);
        assert_eq!(f.numeric_values(), vec![5]);
    }

    #[test]
    fn parse_range() {
        let f = parse_field("1-5", FieldKind::Hour).unwrap();
        assert_eq!(f.terms, vec![Term::Range(1, 5)]);
        assert_eq!(f.numeric_values(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn parse_list() {
        let f = parse_field("1,3,5", FieldKind::Minute).unwrap();
        assert_eq!(f.terms.len(), 3);
        assert_eq!(f.numeric_values(), vec![1, 3, 5]);
    }

    #[test]
    fn parse_step_wildcard() {
        let f = parse_field("*/15", FieldKind::Minute).unwrap();
        assert_eq!(f.terms, vec![Term::Step { from: 0, step: 15 }]);
        assert_eq!(f.numeric_values(), vec![0, 15, 30, 45]);
    }

    #[test]
    fn parse_range_step() {
        let f = parse_field("1-10/2", FieldKind::Minute).unwrap();
        assert_eq!(
            f.terms,
            vec![Term::RangeStep {
                from: 1,
                to: 10,
                step: 2
            }]
        );
        assert_eq!(f.numeric_values(), vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn parse_last_day() {
        let f = parse_field("L", FieldKind::DayOfMonth).unwrap();
        assert_eq!(f.terms, vec![Term::Last]);
        assert!(f.has_special());
    }

    #[test]
    fn parse_last_weekday() {
        let f = parse_field("5L", FieldKind::DayOfWeek).unwrap();
        assert_eq!(f.terms, vec![Term::LastWeekday(5)]);
    }

    #[test]
    fn parse_nearest_weekday() {
        let f = parse_field("15W", FieldKind::DayOfMonth).unwrap();
        assert_eq!(f.terms, vec![Term::NearestWeekday(15)]);
    }

    #[test]
    fn parse_nth_weekday() {
        let f = parse_field("5#3", FieldKind::DayOfWeek).unwrap();
        assert_eq!(
            f.terms,
            vec![Term::NthWeekday {
                weekday: 5,
                occurrence: 3
            }]
        );
    }

    #[test]
    fn parse_question() {
        let f = parse_field("?", FieldKind::DayOfMonth).unwrap();
        assert!(f.is_question());
    }

    #[test]
    fn parse_question_invalid_field() {
        assert!(parse_field("?", FieldKind::Minute).is_err());
    }

    #[test]
    fn parse_month_aliases() {
        let expanded = expand_aliases("JAN-JUN", FieldKind::Month);
        assert_eq!(expanded, "1-6");
        let f = parse_field(&expanded, FieldKind::Month).unwrap();
        assert_eq!(f.numeric_values(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn parse_dow_aliases() {
        let expanded = expand_aliases("MON-FRI", FieldKind::DayOfWeek);
        assert_eq!(expanded, "1-5");
        let f = parse_field(&expanded, FieldKind::DayOfWeek).unwrap();
        assert_eq!(f.numeric_values(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn parse_dow_seven_is_sunday() {
        let f = parse_field("7", FieldKind::DayOfWeek).unwrap();
        assert_eq!(f.terms, vec![Term::Single(0)]);
    }

    #[test]
    fn parse_invalid_value() {
        assert!(parse_field("60", FieldKind::Minute).is_err());
        assert!(parse_field("25", FieldKind::Hour).is_err());
        assert!(parse_field("0", FieldKind::DayOfMonth).is_err());
        assert!(parse_field("13", FieldKind::Month).is_err());
    }

    #[test]
    fn parse_invalid_range() {
        assert!(parse_field("5-3", FieldKind::Minute).is_err());
    }

    #[test]
    fn parse_empty_field() {
        assert!(parse_field("", FieldKind::Minute).is_err());
    }

    #[test]
    fn parse_empty_list_element() {
        assert!(parse_field("1,,3", FieldKind::Minute).is_err());
    }

    #[test]
    fn parse_last_offset() {
        let f = parse_field("L-3", FieldKind::DayOfMonth).unwrap();
        assert_eq!(f.terms, vec![Term::LastOffset(3)]);
    }

    #[test]
    fn parse_step_from_value() {
        let f = parse_field("5/10", FieldKind::Minute).unwrap();
        assert_eq!(f.terms, vec![Term::Step { from: 5, step: 10 }]);
        assert_eq!(f.numeric_values(), vec![5, 15, 25, 35, 45, 55]);
    }
}
