# Security Policy

## Supported Versions

cronscope is a CLI tool with no network access and no persistent state. Security updates apply to the latest release only.

## Reporting a Vulnerability

If you discover a security vulnerability, please **do not** open a public issue. Instead, email the maintainer directly or open a private security advisory on GitHub.

## Security Characteristics

- **No network access**: All computation is performed locally. The tool makes zero network calls.
- **No secrets**: No API keys, tokens, or credentials are required or stored.
- **Input validation**: All user input (cron expressions, file paths, timezone names) is validated. Malformed input produces a clear error message and a non-zero exit code — never a panic.
- **File access**: The `overlap` command reads schedule files. Paths are opened directly via the standard library; no user-supplied path components are used to construct filesystem paths, so path traversal is not possible.
- **No `unsafe` code**: The codebase contains zero `unsafe` blocks.
- **No shell execution**: The tool never invokes a shell or executes external commands.

## Dependencies

cronscope depends on well-maintained, widely-audited crates:
- `clap` — CLI argument parsing
- `chrono` / `chrono-tz` — date/time and timezone handling
- `serde` / `serde_json` — JSON serialization
- `anyhow` — error handling

Keep dependencies updated with `cargo update` and review `Cargo.lock` for advisories via `cargo audit`.
