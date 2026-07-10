# AGENTS.md — cronscope

## Project Overview
cronscope is a Rust CLI tool for cron expression parsing, validation, explanation, run-time computation, and schedule overlap detection.

## Build & Test
```bash
cargo build              # Debug build
cargo build --release    # Release build (optimized)
cargo test               # Run all 80 tests
cargo clippy --all-targets  # Lint (must be clean)
cargo fmt --check        # Format check
```

## Architecture
- `field.rs` — parses individual cron fields into `Term` enums (Wildcard, Single, Range, Step, Last, NearestWeekday, NthWeekday, etc.)
- `expr.rs` — combines fields into `CronExpr`, auto-detects flavour (5/6/7-field)
- `evaluator.rs` — field-by-field advancement algorithm for next/prev run times
- `explain.rs` — generates human-readable descriptions
- `validate.rs` — semantic validation (impossible days, degenerate steps, past years)
- `overlap.rs` — collects fire times from multiple schedules and groups simultaneous ones
- `output.rs` — text and JSON formatting
- `cli.rs` — clap derive CLI definitions
- `main.rs` — command dispatch and file/stdin handling

## Key Design Decisions
- 5-field expressions have an implicit second field pinned to 0 (fires only at second 0)
- Vixie OR semantics for DOM/DOW when both are restricted
- `?` (Quartz) in one day field means only the other matters
- `7` in day-of-week is an alias for `0` (Sunday)
- Named aliases (JAN-DEC, SUN-SAT) are expanded to numbers before numeric parsing

## Conventions
- Commit messages: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `ci:`, `security:`
- All public functions have doc comments
- Tests are inline in each module under `#[cfg(test)]`
- Clippy must pass with zero warnings
