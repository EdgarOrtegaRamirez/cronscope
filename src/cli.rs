//! CLI argument definitions using clap.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "cronscope",
    version,
    about = "A comprehensive cron expression toolkit — parse, validate, explain, and compute run times",
    long_about = "cronscope parses, validates, explains, and computes run times for cron \
expressions. Supports standard 5-field, 6-field (with seconds), and 7-field \
(Quartz, with year) syntax, including L, W, #, ?, named months/days, and \
step values. Includes schedule overlap detection for managing multiple \
cron jobs."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Explain a cron expression in plain English.
    Explain {
        /// The cron expression to explain.
        expression: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },

    /// Validate a cron expression and report issues.
    Validate {
        /// The cron expression to validate.
        expression: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Exit with code 1 if any issues are found (for CI).
        #[arg(long)]
        strict: bool,
    },

    /// Show the next N run times for a cron expression.
    Next {
        /// The cron expression.
        expression: String,

        /// Number of run times to show.
        #[arg(short, long, default_value = "5")]
        count: usize,

        /// Timezone (IANA name, e.g. UTC, America/New_York, Europe/London).
        #[arg(short, long, default_value = "UTC")]
        timezone: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },

    /// Show the previous N run times for a cron expression.
    Prev {
        /// The cron expression.
        expression: String,

        /// Number of run times to show.
        #[arg(short, long, default_value = "5")]
        count: usize,

        /// Timezone (IANA name, e.g. UTC, America/New_York, Europe/London).
        #[arg(short, long, default_value = "UTC")]
        timezone: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },

    /// Find overlapping run times between multiple cron expressions.
    Overlap {
        /// File containing schedules (one per line: "name expression").
        /// Use "-" for stdin.
        file: String,

        /// Number of hours to scan ahead.
        #[arg(short, long, default_value = "24")]
        hours: i64,

        /// Timezone (IANA name).
        #[arg(short, long, default_value = "UTC")]
        timezone: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },

    /// Show a monthly calendar with fire days highlighted.
    Calendar {
        /// The cron expression.
        expression: String,

        /// Year for the calendar (default: current year).
        #[arg(short, long)]
        year: Option<i32>,

        /// Month (1-12). If not specified, uses current month.
        #[arg(short, long)]
        month: Option<u32>,

        /// Number of months to show.
        #[arg(long, default_value = "1")]
        months: usize,

        /// Timezone (IANA name).
        #[arg(short, long, default_value = "UTC")]
        timezone: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },

    /// Show a 24-hour timeline of fire times by day of week.
    Week {
        /// The cron expression.
        expression: String,

        /// Year for the week view (default: current year).
        #[arg(short, long)]
        year: Option<i32>,

        /// Month (1-12). If not specified, uses current month.
        #[arg(short, long)]
        month: Option<u32>,

        /// Timezone (IANA name).
        #[arg(short, long, default_value = "UTC")]
        timezone: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },

    /// Show a year overview of fire days across all 12 months.
    Year {
        /// The cron expression.
        expression: String,

        /// Year for the overview (default: current year).
        #[arg(short, long)]
        year: Option<i32>,

        /// Timezone (IANA name).
        #[arg(short, long, default_value = "UTC")]
        timezone: String,

        /// Output format: text or json.
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,
    },
}
