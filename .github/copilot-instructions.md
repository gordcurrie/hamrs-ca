# Copilot Instructions

## Project

`hamrs-ca` is a Rust CLI study tool for the Canadian Amateur Radio Basic Qualification exam (ISED). It has two distinct UX modes:

- **Concept mode** (`hamrs concept`) — plain readline I/O, no ratatui. Displays pre-generated or AI-generated explanations per topic, tracks progress in SQLite, supports follow-up questions via Anthropic or Ollama.
- **Quiz / Exam mode** (`hamrs quiz`, `hamrs exam`) — full ratatui TUI, weighted question sampling from a 984-question SQLite-backed bank.

Key dependencies: `tokio` (async runtime), `rusqlite` (bundled SQLite), `ratatui` + `crossterm` (quiz TUI), `reqwest` (AI backends), `clap` (CLI), `termimad` (markdown rendering), `dirs` (platform paths), `toml` (config).

## What to focus on

- **Correctness**: wrong assumptions about data ordering, off-by-one errors, missed edge cases in user input handling
- **Concurrency / async**: unnecessary blocking in async context, missing cancellation, futures held across await points
- **SQLite safety**: SQL injection via string interpolation, missing `IF NOT EXISTS`, schema migrations that would break existing DBs
- **Resource usage**: allocations inside loops or closures that run per-element (e.g. `format!` inside `retain`), redundant clones
- **Platform correctness**: paths built with string concat instead of `PathBuf`, Unix-only code not gated behind `#[cfg(unix)]`
- **TOCTOU**: existence checks followed by open — flag and suggest `create_new(true)` or equivalent atomic alternatives
- **Test coverage**: new public DB methods or non-trivial logic without tests

## What to skip

- Suggestions to add comments, doc comments, or logging — the codebase is intentionally terse
- Suggestions to add error handling for internal invariants that cannot fail in practice
- Abstractions or traits for hypothetical future use cases
- Style nitpicks that `rustfmt` and `clippy` already enforce

## Code conventions

- No comments unless the why is non-obvious (a hidden constraint, a workaround, a surprising invariant)
- No docstrings on internal functions
- Errors propagated with `anyhow` — `?` everywhere, `anyhow!(...)` for contextual messages
- DB tests use `Db::open_in_memory()` (defined under `#[cfg(test)]`), never a real file
- File-system tests use `tempfile::tempdir()` for isolation
- Config and data paths resolved via `dirs::config_local_dir()` / `dirs::data_local_dir()` — never hardcoded strings
- The question bank is compiled into the binary via `build.rs` — it is read-only and always present at runtime
