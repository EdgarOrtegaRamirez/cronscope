//! Entry point for the cronscope CLI.

use std::io::{self, Read};
use std::process::ExitCode;

use anyhow::{anyhow, Result};
use chrono::Datelike;
use chrono::Utc;
use clap::Parser;

use cronscope::cli::{Cli, Command};
use cronscope::calendar::{
    build_month_calendar, build_week_view, build_year_overview, format_month_calendar,
    format_month_calendar_json, format_week_view, format_week_view_json, format_year_overview,
    format_year_overview_json,
};
use cronscope::evaluator::{next_runs, prev_run};
use cronscope::explain::explain;
use cronscope::expr::parse_cron;
use cronscope::output::{
    format_explain_json, format_explain_text, format_overlaps_json, format_overlaps_text,
    format_run_times_json, format_run_times_text, format_validation_json, format_validation_text,
    OutputFormat,
};
use cronscope::overlap::{find_overlaps, Schedule};
use cronscope::validate::{is_valid, validate};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    match cli.command {
        Command::Explain { expression, format } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let desc = explain(&expr);
            let fmt = parse_format(&format)?;
            let out = match fmt {
                OutputFormat::Text => format_explain_text(&expr, &desc),
                OutputFormat::Json => format_explain_json(&expr, &desc),
            };
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }

        Command::Validate {
            expression,
            format,
            strict,
        } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let issues = validate(&expr);
            let fmt = parse_format(&format)?;
            let out = match fmt {
                OutputFormat::Text => format_validation_text(&expr, &issues),
                OutputFormat::Json => format_validation_json(&expr, &issues),
            };
            print!("{out}");
            if strict && !issues.is_empty() {
                // Exit 1 if there are any issues (errors or warnings).
                if !is_valid(&expr) || !issues.is_empty() {
                    return Ok(ExitCode::from(1));
                }
            }
            if !is_valid(&expr) {
                return Ok(ExitCode::from(1));
            }
            Ok(ExitCode::SUCCESS)
        }

        Command::Next {
            expression,
            count,
            timezone,
            format,
        } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let tz = parse_timezone(&timezone)?;
            let now = Utc::now().with_timezone(&tz);
            let times = next_runs(&expr, now, count).map_err(|e| anyhow!(e))?;
            let fmt = parse_format(&format)?;
            let out = match fmt {
                OutputFormat::Text => format_run_times_text(&times),
                OutputFormat::Json => format_run_times_json(&times),
            };
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }

        Command::Prev {
            expression,
            count,
            timezone,
            format,
        } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let tz = parse_timezone(&timezone)?;
            let now = Utc::now().with_timezone(&tz);
            let mut times = Vec::with_capacity(count);
            let mut current = now;
            for _ in 0..count {
                let prev = prev_run(&expr, current).map_err(|e| anyhow!(e))?;
                times.push(prev);
                current = prev;
            }
            let fmt = parse_format(&format)?;
            let out = match fmt {
                OutputFormat::Text => format_run_times_text(&times),
                OutputFormat::Json => format_run_times_json(&times),
            };
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }

        Command::Overlap {
            file,
            hours,
            timezone,
            format,
        } => {
            let tz = parse_timezone(&timezone)?;
            let content = if file == "-" {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf)?;
                buf
            } else {
                std::fs::read_to_string(&file)
                    .map_err(|e| anyhow!("failed to read file '{file}': {e}"))?
            };

            let schedules = parse_schedules(&content)?;
            if schedules.len() < 2 {
                eprintln!("warning: need at least 2 schedules to detect overlaps");
            }
            let now = Utc::now().with_timezone(&tz);
            let window = hours * 3600;
            let overlaps = find_overlaps(&schedules, now, window).map_err(|e| anyhow!(e))?;
            let fmt = parse_format(&format)?;
            let out = match fmt {
                OutputFormat::Text => format_overlaps_text(&overlaps),
                OutputFormat::Json => format_overlaps_json(&overlaps),
            };
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }

        Command::Calendar {
            expression,
            year,
            month,
            months,
            timezone,
            format,
        } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let tz = parse_timezone(&timezone)?;
            let now = Utc::now().with_timezone(&tz);
            let cal_year = year.unwrap_or(now.year());
            let cal_month = month.unwrap_or(now.month());
            let fmt = parse_format(&format)?;

            let mut output = String::new();
            for i in 0..months {
                let m = u32::try_from((cal_month as usize - 1 + i) % 12 + 1).unwrap_or(1);
                let y = cal_year + ((cal_month as usize - 1 + i) / 12) as i32;
                let cal = build_month_calendar(&expr, &tz, y, m);
                let out = match fmt {
                    OutputFormat::Text => format_month_calendar(&cal, &expression),
                    OutputFormat::Json => format_month_calendar_json(&cal, &expression),
                };
                output.push_str(&out);
                if fmt == OutputFormat::Text && i < months - 1 {
                    output.push('\n');
                }
            }
            print!("{output}");
            Ok(ExitCode::SUCCESS)
        }

        Command::Week {
            expression,
            year,
            month,
            timezone,
            format,
        } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let tz = parse_timezone(&timezone)?;
            let now = Utc::now().with_timezone(&tz);
            let w_year = year.unwrap_or(now.year());
            let w_month = month.unwrap_or(now.month());
            let fmt = parse_format(&format)?;

            let entries = build_week_view(&expr, &tz, w_year, w_month);
            let out = match fmt {
                OutputFormat::Text => format_week_view(&entries, &expression, w_year, w_month),
                OutputFormat::Json => {
                    format_week_view_json(&entries, &expression, w_year, w_month)
                }
            };
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }

        Command::Year {
            expression,
            year,
            timezone,
            format,
        } => {
            let expr = parse_cron(&expression).map_err(|e| anyhow!(e))?;
            let tz = parse_timezone(&timezone)?;
            let now = Utc::now().with_timezone(&tz);
            let y_year = year.unwrap_or(now.year());
            let fmt = parse_format(&format)?;

            let overview = build_year_overview(&expr, &tz, y_year);
            let out = match fmt {
                OutputFormat::Text => format_year_overview(&overview, &expression),
                OutputFormat::Json => format_year_overview_json(&overview, &expression),
            };
            print!("{out}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn parse_format(s: &str) -> Result<OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "text" | "txt" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(anyhow!(
            "unknown output format '{s}' (use 'text' or 'json')"
        )),
    }
}

fn parse_timezone(s: &str) -> Result<chrono_tz::Tz> {
    s.parse::<chrono_tz::Tz>()
        .map_err(|e| anyhow!("invalid timezone '{s}': {e}"))
}

/// Parse a schedules file. Each line is "name expression" where the name
/// is the first whitespace-delimited token and the rest is the cron
/// expression. Blank lines and lines starting with # are ignored.
fn parse_schedules(content: &str) -> Result<Vec<Schedule>> {
    let mut schedules = Vec::new();
    for (lineno, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let name = parts
            .next()
            .ok_or_else(|| anyhow!("line {}: missing schedule name", lineno + 1))?
            .trim()
            .to_string();
        let expr_str = parts
            .next()
            .ok_or_else(|| anyhow!("line {}: missing cron expression", lineno + 1))?
            .trim()
            .to_string();
        if expr_str.is_empty() {
            return Err(anyhow!("line {}: missing cron expression", lineno + 1));
        }
        let expr = parse_cron(&expr_str).map_err(|e| {
            anyhow!(
                "line {}: invalid cron expression '{expr_str}': {e}",
                lineno + 1
            )
        })?;
        schedules.push(Schedule::new(name, expr));
    }
    Ok(schedules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_schedules_basic() {
        let content = "# comment\njob-a 0 0 * * *\njob-b */30 * * * *\n";
        let schedules = parse_schedules(content).unwrap();
        assert_eq!(schedules.len(), 2);
        assert_eq!(schedules[0].name, "job-a");
        assert_eq!(schedules[1].name, "job-b");
    }

    #[test]
    fn parse_schedules_blank_lines() {
        let content = "\n\njob-a 0 0 * * *\n\n";
        let schedules = parse_schedules(content).unwrap();
        assert_eq!(schedules.len(), 1);
    }

    #[test]
    fn parse_schedules_invalid_expr() {
        let content = "job-a not a cron\n";
        assert!(parse_schedules(content).is_err());
    }

    #[test]
    fn parse_format_text() {
        assert_eq!(parse_format("text").unwrap(), OutputFormat::Text);
        assert_eq!(parse_format("json").unwrap(), OutputFormat::Json);
        assert!(parse_format("xml").is_err());
    }

    #[test]
    fn parse_timezone_valid() {
        assert!(parse_timezone("UTC").is_ok());
        assert!(parse_timezone("America/New_York").is_ok());
        assert!(parse_timezone("Europe/London").is_ok());
    }

    #[test]
    fn parse_timezone_invalid() {
        assert!(parse_timezone("Mars/Olympus").is_err());
    }
}
