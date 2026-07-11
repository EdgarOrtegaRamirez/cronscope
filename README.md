# cronscope

A comprehensive cron expression toolkit CLI — parse, validate, explain, compute run times, and detect schedule overlaps.

[![CI](https://github.com/EdgarOrtegaRamirez/cronscope/actions/workflows/ci.yml/badge.svg)](https://github.com/EdgarOrtegaRamirez/cronscope/actions/workflows/ci.yml)

## Why?

`crontab.guru` is web-only. Existing cron libraries are libraries, not CLIs. None of them detect **schedule overlaps** — a critical feature when managing dozens of cron jobs that might contend for resources at the same instant.

`cronscope` fills this gap with a single binary that works offline, integrates into CI pipelines, and handles the full range of cron syntax including special modifiers.

## Features

- **Three cron flavours**: standard 5-field, 6-field (with seconds), and 7-field Quartz (with year)
- **Special modifiers**: `L` (last day), `L-n` (last day minus offset), `nW` (nearest weekday), `nL` (last weekday), `n#m` (nth weekday), `?` (Quartz no-specific-value)
- **Named aliases**: `JAN`–`DEC` for months, `SUN`–`SAT` for days of week
- **Step values**: `*/15`, `1-10/2`, `5/10`
- **Timezone-aware**: any IANA timezone (UTC, America/New_York, Europe/London, …)
- **Human-readable explanations**: "At 02:30 on Monday through Friday"
- **Semantic validation**: detects impossible day/month combinations, degenerate steps, past years
- **Schedule overlap detection**: find time windows where multiple cron jobs fire simultaneously
- **CI-friendly exit codes**: `--strict` flag for validation
- **Calendar views**: monthly calendar with highlighted fire days, week view with 24-hour timeline, and full year overview
- **Multiple output formats**: text (default) and JSON (for CI/automation)

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/EdgarOrtegaRamirez/cronscope.git
cd cronscope
cargo build --release
# Binary at ./target/release/cronscope
```

## Usage

### Explain a cron expression

```bash
$ cronscope explain "*/5 * * * *"
Expression: */5 * * * *
Flavor:     5-field (standard)
Meaning:    Every 5 minutes.

$ cronscope explain "30 2 * * 1-5"
Expression: 30 2 * * 1-5
Flavor:     5-field (standard)
Meaning:    At 02:30 on Monday through Friday.

$ cronscope explain "0 0 L * *"
Expression: 0 0 L * *
Flavor:     5-field (standard)
Meaning:    At 00:00 on the last day of the month.

$ cronscope explain "0 0 * * 5#3"
Expression: 0 0 * * 5#3
Flavor:     5-field (standard)
Meaning:    At 00:00 on the third Friday of the month.
```

### Validate a cron expression

```bash
$ cronscope validate "0 0 30 2 *"
⚠ '0 0 30 2 *' is valid with warnings:
  ⚠ day 30 never occurs in February (max 29) — this combination will never match

$ cronscope validate "0 0 30 2 *" --strict
⚠ '0 0 30 2 *' is valid with warnings:
  ⚠ day 30 never occurs in February (max 29) — this combination will never match
# Exit code: 1 (use in CI to fail on warnings)
```

### Show next run times

```bash
$ cronscope next "*/15 * * * *" -c 5
  1  2026-07-10T21:30:00Z
  2  2026-07-10T21:45:00Z
  3  2026-07-10T22:00:00Z
  4  2026-07-10T22:15:00Z
  5  2026-07-10T22:30:00Z

$ cronscope next "0 0 * * *" -c 3 --timezone America/New_York
  1  2026-07-11T00:00:00-04:00
  2  2026-07-12T00:00:00-04:00
  3  2026-07-13T00:00:00-04:00
```

### Show previous run times

```bash
$ cronscope prev "0 12 * * *" -c 3
  1  2026-07-10T12:00:00Z
  2  2026-07-09T12:00:00Z
  3  2026-07-08T12:00:00Z
```

### Detect schedule overlaps

Create a file with one schedule per line (`name expression`):

```
# schedules.txt
backup    0 2 * * *
cleanup   0 2 * * *
report    0 2 1 * *
sync      */30 * * * *
```

```bash
$ cronscope overlap schedules.txt --hours 48
Found 2 overlap(s):

  2026-07-11T02:00:00Z  backup, cleanup, sync
  2026-07-12T02:00:00Z  backup, cleanup, sync
```

Use `-` to read from stdin:

```bash
$ printf 'job1 0 0 * * *\njob2 0 0 * * *\n' | cronscope overlap - --hours 24
Found 1 overlap(s):

  2026-07-11T00:00:00Z  job1, job2
```

### JSON output

All commands support `--format json`:

```bash
$ cronscope next "*/15 * * * *" -c 3 --format json
[
  {
    "index": 1,
    "timestamp": "2026-07-10T21:30:00Z"
  },
  {
    "index": 2,
    "timestamp": "2026-07-10T21:45:00Z"
  },
  {
    "index": 3,
    "timestamp": "2026-07-10T22:00:00Z"
  }
]
```

### Monthly calendar view

Show which days in a month a cron expression fires on, with fire days highlighted:

```bash
$ cronscope calendar "0 12 * * *" --year 2026 --month 7
July 2026
Expression: 0 12 * * *
Total fires: 31

Su Mo Tu We Th Fr Sa
          1  2  3  4
 5  6  7  8  9 10 11
12 13 14 15 16 17 18
19 20 21 22 23 24 25
26 27 28 29 30 31

█  = fire day
```

Show multiple months with `--months`:

```bash
$ cronscope calendar "0 0 * * 0" --year 2026 --month 7 --months 2
```

### Week view

Show fire times grouped by day of the week:

```bash
$ cronscope week "0 9,12,17 * * 1-5" --year 2026 --month 7
Week View — July 2026
Expression: 0 9,12,17 * * 1-5

Sun  —
Mon  09:00:00, 12:00:00, 17:00:00
Tue  09:00:00, 12:00:00, 17:00:00
Wed  09:00:00, 12:00:00, 17:00:00
Thu  09:00:00, 12:00:00, 17:00:00
Fri  09:00:00, 12:00:00, 17:00:00
Sat  —
```

### Year overview

Show fire days across all 12 months in a compact format:

```bash
$ cronscope year "0 0 1 * *" --year 2026
Year Overview — 2026
Expression: 0 0 1 * *
Total fires: 12

Jan 2026: ...............................  1 days, 3% busy
Feb 2026: ............................  1 days, 4% busy
Mar 2026: ...............................  1 days, 3% busy
...
```

## Cron Syntax Reference

### Field order

| Flavour | Fields |
|---------|--------|
| 5-field (standard) | `minute hour day-of-month month day-of-week` |
| 6-field (with seconds) | `second minute hour day-of-month month day-of-week` |
| 7-field (Quartz) | `second minute hour day-of-month month day-of-week year` |

### Special characters

| Character | Meaning | Fields |
|-----------|---------|--------|
| `*` | All values in range | All |
| `,` | List separator | All |
| `-` | Range | All |
| `/` | Step value | All |
| `L` | Last day of month | day-of-month |
| `L-n` | Last day minus n days | day-of-month |
| `nW` | Nearest weekday to day n | day-of-month |
| `nL` | Last occurrence of weekday n | day-of-week |
| `n#m` | m-th occurrence of weekday n | day-of-week |
| `?` | No specific value | day-of-month, day-of-week |

### DOM/DOW semantics

When both day-of-month and day-of-week are restricted (not `*` or `?`), `cronscope` uses **Vixie OR semantics**: the expression matches when *either* field matches. This is the standard cron behaviour.

When one is `?` (Quartz), only the other field is considered.

## Architecture

```
src/
├── main.rs       — CLI entry point, command dispatch
├── cli.rs        — clap argument definitions
├── lib.rs        — library re-exports
├── calendar.rs   — calendar, week, and year overview visualization
├── field.rs      — individual field parsing (minute, hour, DOM, etc.)
├── expr.rs       — full cron expression parsing
├── evaluator.rs  — next/previous run time computation
├── explain.rs    — human-readable description generation
├── validate.rs   — semantic validation (impossible days, etc.)
├── overlap.rs    — schedule overlap detection
└── output.rs     — text/JSON output formatting
```

The evaluator uses a **field-by-field advancement algorithm**: starting from a reference time, it walks forward (or backward) one field at a time — year, month, day, hour, minute, second — resetting smaller fields whenever a larger field is advanced. This is far more efficient than a naive second-by-second scan.

## Testing

```bash
cargo test          # 90 tests
cargo clippy        # lint
cargo fmt --check   # format check
```

## Security

- No network access required — all computation is local
- No hardcoded secrets or tokens
- File path input is validated; path traversal is not possible (paths are opened directly, not constructed from user data)
- All user input is validated with clear error messages

## License

MIT
