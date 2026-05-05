mod ai;
mod content;
mod db;
mod modes;
mod morse;
mod questions;
mod tui;

/// Shared lock for tests that mutate process-wide environment variables.
/// A single crate-wide mutex prevents cross-module races that per-module
/// locks cannot guard against.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use ai::ConceptClient;

use anyhow::Result;
use clap::{Parser, Subcommand};
use db::Db;
use questions::QuestionBank;

#[derive(Parser)]
#[command(name = "hamrs", about = "Canadian amateur radio study tool", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Weighted practice quiz across all sections, or target specific ones
    Quiz {
        /// Section(s) to draw from, e.g. --section 5 or --section 5,6
        #[arg(short, long, value_delimiter = ',', value_name = "N")]
        section: Vec<u8>,
        /// Number of questions
        #[arg(short, long, default_value_t = 20, value_name = "N")]
        count: usize,
    },
    /// Full 100-question timed exam (simulates real ISED conditions)
    Exam,
    /// Learn any section with AI explanations — start here before quizzing
    Concept,
    /// Show recent session history
    Stats,
    /// Frequency band reference — log-scale spectrum chart with exam key facts
    Bands,
    /// Morse code practice [experimental] — receive (decode) and transmit (encode)
    Morse {
        /// Practice mode
        #[arg(short, long)]
        mode: Option<modes::morse::MorseMode>,
        /// Target words per minute
        #[arg(short, long, value_name = "N")]
        wpm: Option<u32>,
        /// Number of items per session
        #[arg(short, long, value_name = "N")]
        count: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    ConceptClient::ensure_config();
    let cli = Cli::parse();
    let db = Db::open()?;
    let bank = QuestionBank::load();

    match cli.command {
        Command::Quiz { section, count } => {
            let sections: Option<Vec<u8>> = if !section.is_empty() {
                Some(section)
            } else {
                match modes::exam::pick_sections()? {
                    None => return Ok(()),
                    Some(s) => s,
                }
            };
            let session =
                modes::exam::build_practice_session(&bank, &db, sections.as_deref(), count)?;
            tui::quiz::run(session, &db)?;
        }
        Command::Exam => {
            let session = modes::exam::build_exam_session(&bank, &db)?;
            tui::quiz::run(session, &db)?;
        }
        Command::Concept => {
            modes::concept::run(&bank, &db).await?;
        }
        Command::Stats => {
            print_stats(&db, &bank)?;
        }
        Command::Bands => {
            modes::bands::run(&bank);
        }
        Command::Morse { mode, wpm, count } => {
            if let Some(session) = modes::morse::setup(mode, wpm, count)? {
                tui::morse::run(session)?;
            }
        }
    }

    Ok(())
}

fn print_stats(db: &Db, bank: &QuestionBank) -> Result<()> {
    let sessions = db.recent_sessions(10)?;

    if sessions.is_empty() {
        println!("\n  No sessions yet. Run 'hamrs quiz' or 'hamrs exam' to get started.\n");
        return Ok(());
    }

    println!();
    println!(
        "  {:<12} {:<8} {:<8} {:<8} Result",
        "Mode", "Score", "Total", "Pct"
    );
    println!("  {}", "─".repeat(52));

    for (mode, score, total) in &sessions {
        let total = *total;
        let score = *score;
        let pct = (score * 100) / total.max(1);
        let result = if pct >= 80 {
            "★ Honours"
        } else if pct >= 70 {
            "✓ Pass"
        } else {
            "✗ Below passing"
        };
        println!(
            "  {:<12} {:<8} {:<8} {:<7}% {}",
            mode, score, total, pct, result
        );
    }

    println!();
    print_focus_areas(db, bank)?;
    Ok(())
}

fn print_focus_areas(db: &Db, bank: &QuestionBank) -> Result<()> {
    use std::collections::HashMap;

    let all_stats = db.all_question_stats()?;

    // Aggregate attempts/correct per (section, subsection) across all questions
    let mut by_section: HashMap<u8, HashMap<u8, (u32, u32)>> = HashMap::new();
    let mut section_names: HashMap<u8, &'static str> = HashMap::new();

    for q in bank.all() {
        section_names
            .entry(q.section)
            .or_insert_with(|| q.section_name());
        let entry = by_section
            .entry(q.section)
            .or_default()
            .entry(q.subsection)
            .or_insert((0, 0));
        if let Some(qs) = all_stats.get(&q.id) {
            entry.0 += qs.attempts;
            entry.1 += qs.correct;
        }
    }

    // Classify each section
    struct SectionSummary {
        num: u8,
        name: &'static str,
        needs_work: Vec<u8>,
        improving: Vec<u8>,
        not_started: Vec<u8>,
    }

    let mut sections: Vec<SectionSummary> = by_section
        .iter()
        .map(|(&sec, sub_map)| {
            let mut subs: Vec<u8> = sub_map.keys().cloned().collect();
            subs.sort();

            let mut needs_work = Vec::new();
            let mut improving = Vec::new();
            let mut not_started = Vec::new();

            for sub in &subs {
                let (attempts, correct) = sub_map[sub];
                if attempts == 0 {
                    not_started.push(*sub);
                } else {
                    let ratio = correct as f32 / attempts as f32;
                    if ratio < 0.6 {
                        needs_work.push(*sub);
                    } else if ratio < 0.9 {
                        improving.push(*sub);
                    }
                }
            }

            SectionSummary {
                num: sec,
                name: section_names[&sec],
                needs_work,
                improving,
                not_started,
            }
        })
        .collect();

    // Sort: sections with failing topics first, then partially started/improving, then solid
    sections.sort_by(|a, b| {
        let priority = |s: &SectionSummary| {
            if !s.needs_work.is_empty() {
                0
            } else if !s.improving.is_empty() || !s.not_started.is_empty() {
                1
            } else {
                2
            }
        };
        priority(a).cmp(&priority(b)).then(a.num.cmp(&b.num))
    });

    println!("  Focus Areas");
    println!("  {}", "─".repeat(60));

    for s in &sections {
        let all_not_started = s.needs_work.is_empty() && s.improving.is_empty();
        let all_solid =
            s.needs_work.is_empty() && s.improving.is_empty() && s.not_started.is_empty();

        let symbol = if !s.needs_work.is_empty() {
            "✗"
        } else if all_solid {
            "✓"
        } else if all_not_started {
            "-"
        } else {
            "~"
        };

        let detail = if all_solid {
            String::new()
        } else if all_not_started {
            "(not started)".to_string()
        } else {
            let fmt = |v: &[u8]| {
                v.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut parts = Vec::new();
            if !s.needs_work.is_empty() {
                parts.push(format!("review: {}", fmt(&s.needs_work)));
            }
            if !s.improving.is_empty() {
                parts.push(format!("practice: {}", fmt(&s.improving)));
            }
            if !s.not_started.is_empty() {
                parts.push(format!("not started: {}", fmt(&s.not_started)));
            }
            parts.join("  |  ")
        };

        println!("  §{:<2} {:<32} {}  {}", s.num, s.name, symbol, detail);
    }

    println!();
    Ok(())
}
