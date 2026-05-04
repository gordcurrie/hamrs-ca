# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                        # debug build
cargo build --release              # release build (binary at target/release/hamrs)
cargo test                         # run all tests
cargo test <test_name>             # run a single test by name
cargo clippy -- -D warnings        # lint (CI enforces warnings-as-errors)
cargo fmt --check                  # format check (CI enforces this)
cargo fmt                          # auto-format
```

## After every code change

Always run all three CI checks before declaring work done or opening a PR:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Do not skip these steps — CI enforces all three and they must pass before pushing.

## Architecture

### Compile-time data embedding

`build.rs` runs before compilation and does two things:

1. Parses `amat_basic_quest/amat_basic_quest_delim.txt` (semicolon-delimited, 984 rows) into `$OUT_DIR/questions.json`.
2. Reads every `content/*.md` file and generates `$OUT_DIR/content_map.rs` — a `get_pregenerated_content(key: &str) -> Option<&'static str>` function with all markdown content as static strings.

The question bank is embedded via `include_str!`; the content map is embedded via `include!` (it's generated Rust source, not raw text). No data files ship with the binary. Changing any file in `content/` or the question bank triggers a rebuild.

### Module responsibilities

- **`src/questions/mod.rs`** — `QuestionBank` loads the embedded JSON and exposes iterators by section and subsection. Question IDs use the format `B-SSS-SSS-QQQ`.
- **`src/db/mod.rs`** — SQLite via rusqlite. Database path resolves as: `$XDG_DATA_HOME/hamrs-ca/progress.db` → `~/.local/share/hamrs-ca/progress.db` → `.` (if home unavailable). On first run with a new path, if the old `dirs::data_local_dir()` path exists, it keeps using the old location to avoid silently losing progress. Three tables: `sessions`, `attempts`, `concept_progress`. `QuestionStats::weight()` is the spaced-repetition core: unseen → 3, ≥90% → 1, ≥60% → 2, <60% → 4.
- **`src/ai/mod.rs`** — `ConceptClient` abstracts two backends: Anthropic API and Ollama. Anthropic takes priority when `anthropic_api_key` is set in the config file or via `$HAMRS_ANTHROPIC_API_KEY`. Falls back to Ollama at `http://localhost:11434`. Config file is written on first run (commented-out template, `chmod 600`). Config path resolves as: `$XDG_CONFIG_HOME/hamrs-ca/config.toml` → `~/.config/hamrs-ca/config.toml` → `.`.
- **`src/modes/exam.rs`** — Builds `QuizSession` values. `weighted_sample` shuffles a weight-expanded index pool then deduplicates, giving higher-weight questions a proportionally better chance of appearing.
- **`src/modes/concept.rs`** — Interactive learn mode. For each subsection key (e.g., `B-005-002`), it checks `get_pregenerated_content` first. If found, content is printed immediately with no API call; the pre-generated text is also injected as the assistant turn so follow-up questions have context. Falls back to a live AI call if no pre-generated content exists.
- **`src/tui/quiz.rs`** — ratatui + crossterm terminal UI for quiz and exam sessions.
- **`src/content.rs`** — includes the generated `content_map.rs` from `$OUT_DIR`.

### Pre-generated concept content

Files in `content/` are named by subsection key (e.g., `content/B-005-002.md`) and committed to the repo. To add or regenerate content for a subsection, create or update the corresponding `.md` file and rebuild. The Makefile comment says: "open Claude Code and ask it to regenerate content for the desired sections."

### UX mode split

Concept mode (`hamrs concept`) uses plain `stdin.read_line()` throughout — no ratatui. Quiz and exam modes use the ratatui TUI for the question screen, but the section-picker prompt in `modes/exam::pick_sections()` also uses plain `stdin.read_line()` before the TUI starts. Both I/O paths need to stay correct when touching input handling.

### Code conventions

- Errors propagated with `anyhow` — `?` everywhere, `anyhow!(...)` for contextual messages.
- No comments unless the why is non-obvious. No docstrings on internal functions.
- Config and data paths resolve via XDG env vars first (`$XDG_CONFIG_HOME`, `$XDG_DATA_HOME`), then home-based fallbacks (`~/.config`, `~/.local/share`), then `.` as a last resort.
- Paths built with `PathBuf` joins, never string concatenation.
- Unix-only code gated behind `#[cfg(unix)]`.
- Atomic file creation uses `OpenOptions::new().create_new(true)` to avoid TOCTOU races.

### What to watch for

- SQL injection via string interpolation (use `params!` / `params_from_iter`).
- Schema changes that would break existing databases (no migration system exists).
- Blocking calls in async context.
- `format!` or allocations inside per-element closures/loops.
- New public DB methods or non-trivial logic without tests.

### Test conventions

- Tests that mutate environment variables must hold `crate::ENV_LOCK` (defined in `main.rs`) for the duration of the test — it's a crate-wide mutex that prevents cross-module env races.
- Database tests use `Db::open_in_memory()` for isolation.
- The `EnvGuard` helper (duplicated in `src/ai/mod.rs` and `src/db/mod.rs`) restores env vars on drop.
