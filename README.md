# hamrs-ca

A Rust CLI study tool for the **Canadian Amateur Radio Basic Qualification** exam (ISED).

Built for people who want to actually understand the RF engineering — not just memorize answers and forget them. The tool has two distinct modes that reflect the nature of the material: conceptual depth for technical topics, and efficient drilling for regulations and procedures.

---

## Features

**Learn Mode** (AI powered)

![concept mode demo](assets/demo-concept.gif)

- Browse all 8 sections by topic
- Engineering-depth explanations — signal physics, circuit behaviour, propagation mechanics
- Regulations covered with context and rationale, not just rules
- Related exam questions shown after each explanation with correct answers marked
- Follow-up questions supported — it's a conversation, not a page

**Frequency Band Reference**

![bands command demo](assets/demo-bands.gif)

- `bands` — log-scale spectrum chart (100 kHz–1 GHz) showing all 15 Canadian amateur allocations
- Green = primary (protected), yellow = secondary (must not interfere)
- Exam key facts and question-bank cross-references for every band

**Morse Code Practice**

![morse mode demo](assets/demo-morse.gif)

- `morse` — interactive trainer for both decoding and encoding
- **Receive mode**: Morse elements animate one at a time at your target WPM — watch the pattern, type the character
- **Transmit mode**: A character is shown; type the Morse (`.` and `-`) and hit Enter — your actual WPM is calculated from response times
- **Both**: alternates receive and transmit items in the same session
- Prompts for mode, WPM (5 / 10 / 13 / 15 / 20 / custom), character set (letters, numbers, or both), and session length
- Flags (`--mode`, `--wpm`, `--count`) skip individual prompts for scripted use

**Quiz & Exam Mode**

![quiz mode demo](assets/demo.gif)

- `quiz` — weighted practice across all sections; questions you miss appear more often
  - Interactive section picker, or target with `-s` and control length with `-c`
- `exam` — full 100-question timed exam simulation with pass/honours feedback
- Progress tracked in a local SQLite database
- Pass threshold: 70% | Honours threshold: 80% (honours unlocks HF privileges)

---

## Installation

### Prebuilt binary (no Rust required)

Download the latest release for your platform from the [Releases page](https://github.com/gordcurrie/hamrs-ca/releases) and place the `hamrs` binary somewhere on your PATH.

### Install with Cargo

```bash
cargo install --git https://github.com/gordcurrie/hamrs-ca
```

Requires the [Rust toolchain](https://rustup.rs). The binary installs to `~/.cargo/bin/hamrs`.

### Build from source

```bash
git clone https://github.com/gordcurrie/hamrs-ca
cd hamrs-ca
cargo build --release
```

The binary is at `target/release/hamrs`.

---

The question bank (984 questions from the ISED public-domain database) is compiled into the binary — no data files to manage.

---

## Usage

```bash
hamrs concept              # Learn any section with AI explanations — start here
hamrs bands                # Frequency band reference — log-scale spectrum chart with exam key facts
hamrs quiz                 # Weighted practice quiz, with interactive section picker
hamrs quiz -s 5            # Practice section 5 only (Electrical Principles)
hamrs quiz -s 5,6          # Practice sections 5 and 6
hamrs quiz -s 5 -c 10      # 10 questions from section 5
hamrs exam                 # Full 100-question timed exam (90 min)
hamrs stats                # Recent session history
hamrs morse                # Morse code practice — prompts for all options
hamrs morse --mode receive --wpm 13  # Skip prompts: receive at 13 WPM
```

### Recommended study flow

1. `hamrs concept` — read and understand a section before drilling it
2. `hamrs quiz -s <N>` — drill a section under repetition pressure while it's fresh
3. `hamrs quiz` — mixed practice across all sections as you progress
4. `hamrs exam` — full timed simulation when you're ready to assess

### Quiz & Exam controls

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Navigate answers |
| `1` – `4` | Jump directly to an answer |
| `Enter` / `Space` | Confirm answer / advance |
| `q` | Quit |

### Morse controls

| Key | Action |
|-----|--------|
| Letters / digits | Type decoded character (receive mode) |
| `.` `-` `Space` | Type Morse pattern (transmit mode) |
| `Backspace` | Delete last character |
| `Enter` | Submit answer |
| `r` | Replay current Morse animation (receive mode) |
| `Enter` / `Space` | Advance after feedback |
| `q` | Quit (when input is empty) |

---

## AI Setup

Learn mode uses an AI model to explain concepts. It supports two backends:

### Option A — Local (Ollama)

Install [Ollama](https://ollama.com), pull a model, and run `hamrs concept`. No API key needed.

```bash
ollama pull glm-4.7-flash   # recommended
hamrs concept
```

Override the model or host:
```bash
OLLAMA_MODEL=gemma4 hamrs concept
OLLAMA_HOST=http://192.168.1.10:11434 hamrs concept
```

**Recommended local models** (best for technical explanation):
- `glm-4.7-flash` — strong instruction following, default
- `qwen3.5` — deeper reasoning on physics/math topics
- `gemma4` — solid alternative

### Option B — Anthropic API

`hamrs` automatically creates a commented-out `config.toml` on first run at `~/.config/hamrs-ca/config.toml` (or `$XDG_CONFIG_HOME/hamrs-ca/config.toml` if set). Open it and uncomment your API key:

```toml
anthropic_api_key = "sk-ant-..."
```

Claude takes priority over Ollama when a key is configured. Default model is `claude-sonnet-4-6`. Override with:

```toml
model = "claude-opus-4-7"
```

For CI or scripting, you can also set `HAMRS_ANTHROPIC_API_KEY` as an environment variable instead of using the config file.

### Customizing the system prompt

The AI's teaching style is controlled by a system prompt. To customize it:

```bash
mkdir -p ~/.config/hamrs-ca
cat > ~/.config/hamrs-ca/system_prompt.md << 'EOF'
You are a technical instructor for the Canadian Amateur Radio Basic Qualification exam.
[your custom instructions here]
EOF
```

If `~/.config/hamrs-ca/system_prompt.md` exists it overrides the built-in default. Delete it to revert.

---

## Question Bank

The exam questions are sourced from the ISED [Amateur Radio Operator Certificate Services](https://ised-isde.canada.ca/site/amateur-radio-operator-certificate-services/en/downloads) downloads page, which is public domain.

**984 questions across 8 sections:**

| Section | Topic |
|---------|-------|
| B-001 | Regulations & Licensing |
| B-002 | Operating Procedures |
| B-003 | Transmitters & Receivers |
| B-004 | Electronics |
| B-005 | Electrical Principles |
| B-006 | Antennas & Feedlines |
| B-007 | Propagation |
| B-008 | Interference |

---

## Progress Tracking

Session history and per-question accuracy are stored in a local SQLite database:

- **All platforms:** `~/.local/share/hamrs-ca/progress.db` (or `$XDG_DATA_HOME/hamrs-ca/progress.db` if set)

The weighting algorithm surfaces questions you miss more often: answered correctly 90%+ → weight 1 (rare), never seen → weight 3, consistently missed → weight 4 (frequent).

---

## About

Built by Gord Currie while studying for the Canadian amateur radio basic qualification. If you find it useful, contributions are welcome.

---

## Licence

MIT
