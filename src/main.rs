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
        "  {:<18} {:<8} {:<8} {:<8} Result",
        "Mode", "Score", "Total", "Pct"
    );
    println!("  {}", "─".repeat(58));

    for (mode, sections, score, total) in &sessions {
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
        let label = match sections.as_deref() {
            Some(s) => format!("{} §{}", mode, s),
            None => mode.clone(),
        };
        println!(
            "  {:<18} {:<8} {:<8} {:<7}% {}",
            label, score, total, pct, result
        );
    }

    println!();
    print_focus_areas(db, bank)?;
    Ok(())
}

struct SectionSummary {
    num: u8,
    name: &'static str,
    needs_work: Vec<u8>,
    improving: Vec<u8>,
    not_started: Vec<u8>,
    has_solid: bool,
}

fn classify_and_sort_sections(
    by_section: &std::collections::HashMap<u8, std::collections::HashMap<u8, (u32, u32)>>,
    section_names: &std::collections::HashMap<u8, &'static str>,
) -> Vec<SectionSummary> {
    let mut sections: Vec<SectionSummary> = by_section
        .iter()
        .map(|(&sec, sub_map)| {
            let mut subs: Vec<u8> = sub_map.keys().cloned().collect();
            subs.sort();

            let mut needs_work = Vec::new();
            let mut improving = Vec::new();
            let mut not_started = Vec::new();
            let mut has_solid = false;

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
                    } else {
                        has_solid = true;
                    }
                }
            }

            SectionSummary {
                num: sec,
                name: section_names[&sec],
                needs_work,
                improving,
                not_started,
                has_solid,
            }
        })
        .collect();

    // Sort: failing first, then in-progress, then not-started (by section number), then solid
    sections.sort_by(|a, b| {
        let bucket = |s: &SectionSummary| {
            if !s.needs_work.is_empty() {
                0u8
            } else if !s.improving.is_empty() || (s.has_solid && !s.not_started.is_empty()) {
                1
            } else if !s.not_started.is_empty() {
                2
            } else {
                3
            }
        };
        let urgency = |s: &SectionSummary| {
            if !s.has_solid && s.needs_work.is_empty() && s.improving.is_empty() {
                0
            } else {
                s.needs_work.len() + s.improving.len() + s.not_started.len()
            }
        };
        bucket(a)
            .cmp(&bucket(b))
            .then(urgency(b).cmp(&urgency(a)))
            .then(a.num.cmp(&b.num))
    });

    sections
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

    let sections = classify_and_sort_sections(&by_section, &section_names);

    println!("  Focus Areas");
    println!("  {}", "─".repeat(60));
    println!("  ✗ has failing topics  ~ in progress  ✓ solid  - not started");
    println!("  topic labels — review: <60%  |  practice: 60–<90%");
    println!();

    for s in &sections {
        let all_not_started = s.needs_work.is_empty() && s.improving.is_empty() && !s.has_solid;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_input(
        data: &[(u8, u8, u32, u32)], // (section, subsection, attempts, correct)
    ) -> (
        HashMap<u8, HashMap<u8, (u32, u32)>>,
        HashMap<u8, &'static str>,
    ) {
        let mut by_section: HashMap<u8, HashMap<u8, (u32, u32)>> = HashMap::new();
        let mut names: HashMap<u8, &'static str> = HashMap::new();
        for &(sec, sub, attempts, correct) in data {
            names.entry(sec).or_insert("Section");
            by_section
                .entry(sec)
                .or_default()
                .insert(sub, (attempts, correct));
        }
        (by_section, names)
    }

    #[test]
    fn classify_subsection_buckets() {
        let (by_section, names) = make_input(&[
            (1, 1, 0, 0),   // not started
            (1, 2, 10, 3),  // <60% → needs_work
            (1, 3, 10, 7),  // 60–90% → improving
            (1, 4, 10, 10), // ≥90% → solid
        ]);
        let sections = classify_and_sort_sections(&by_section, &names);
        assert_eq!(sections.len(), 1);
        let s = &sections[0];
        assert_eq!(s.not_started, vec![1]);
        assert_eq!(s.needs_work, vec![2]);
        assert_eq!(s.improving, vec![3]);
        assert!(s.has_solid);
    }

    #[test]
    fn sort_failing_before_improving_before_not_started_before_solid() {
        let (by_section, names) = make_input(&[
            (1, 1, 10, 10), // solid
            (2, 1, 0, 0),   // not started
            (3, 1, 10, 7),  // improving
            (4, 1, 10, 3),  // failing
        ]);
        let sections = classify_and_sort_sections(&by_section, &names);
        let order: Vec<u8> = sections.iter().map(|s| s.num).collect();
        assert_eq!(order, vec![4, 3, 2, 1]);
    }

    #[test]
    fn sort_not_started_by_section_number() {
        let (by_section, names) = make_input(&[(3, 1, 0, 0), (1, 1, 0, 0), (2, 1, 0, 0)]);
        let sections = classify_and_sort_sections(&by_section, &names);
        let order: Vec<u8> = sections.iter().map(|s| s.num).collect();
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn sort_failing_by_urgency_then_section_number() {
        let (by_section, names) = make_input(&[
            (1, 1, 10, 3), // 1 failing topic
            (2, 1, 10, 3), // 1 failing topic — same urgency, lower section number wins
            (3, 1, 10, 3), // 1 failing topic
            (3, 2, 10, 3), // 2nd failing topic — §3 has more, should be first
        ]);
        let sections = classify_and_sort_sections(&by_section, &names);
        let order: Vec<u8> = sections.iter().map(|s| s.num).collect();
        assert_eq!(order, vec![3, 1, 2]);
    }

    #[test]
    fn mixed_solid_and_not_started_is_in_progress_not_all_not_started() {
        let (by_section, names) = make_input(&[
            (1, 1, 10, 10), // solid
            (1, 2, 0, 0),   // not started
        ]);
        let sections = classify_and_sort_sections(&by_section, &names);
        let s = &sections[0];
        assert!(s.has_solid);
        assert!(!s.not_started.is_empty());
        assert!(s.needs_work.is_empty());
        assert!(s.improving.is_empty());
    }
}
