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
hamrs concept          # Learn any section with AI explanations — start here
hamrs quiz             # Weighted practice quiz, with interactive section picker
hamrs quiz -s 5        # Practice section 5 only (Electrical Principles)
hamrs quiz -s 5,6      # Practice sections 5 and 6
hamrs quiz -s 5 -c 10  # 10 questions from section 5
hamrs exam             # Full 100-question timed exam (90 min)
hamrs stats            # Recent session history
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

```bash
# Add to your shell profile (~/.zshrc or ~/.bashrc)
export ANTHROPIC_API_KEY="sk-ant-..."
```

If `ANTHROPIC_API_KEY` is set, it takes priority over Ollama. Default model is `claude-sonnet-4-6`. Override with:
```bash
HAMRS_MODEL=claude-opus-4-7 hamrs concept
```

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

- **macOS/Linux:** `~/.local/share/hamrs-ca/progress.db`

The weighting algorithm surfaces questions you miss more often: answered correctly 90%+ → weight 1 (rare), never seen → weight 3, consistently missed → weight 4 (frequent).

---

## About

Built by Gord Currie while studying for the Canadian amateur radio basic qualification. If you find it useful, contributions are welcome.

---

## Licence

MIT
