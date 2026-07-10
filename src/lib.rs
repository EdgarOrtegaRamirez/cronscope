//! cronscope — a comprehensive cron expression toolkit.
//!
//! Parse, validate, explain, and compute run times for cron expressions.
//! Supports standard 5-field, 6-field (with seconds), and 7-field
//! (Quartz, with year) cron syntax, including special modifiers `L`, `W`,
//! `#`, `?`, named months/days, and step values.

pub mod cli;
pub mod evaluator;
pub mod explain;
pub mod expr;
pub mod field;
pub mod output;
pub mod overlap;
pub mod validate;

pub use evaluator::{next_run, next_runs, prev_run};
pub use explain::explain;
pub use expr::{parse_cron, CronExpr, CronFlavor};
pub use output::OutputFormat;
pub use overlap::{find_overlaps, Overlap, Schedule};
pub use validate::{validate, ValidationIssue};
