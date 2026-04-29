use crate::ai::{ConceptClient, Message};
use crate::db::Db;
use crate::questions::QuestionBank;
use anyhow::Result;
use std::collections::HashSet;
use std::io::{self, BufRead, Write};

const SECTION_NAMES: &[(u8, &str)] = &[
    (1, "Regulations & Licensing"),
    (2, "Operating Procedures"),
    (3, "Transmitters & Receivers"),
    (4, "Electronics"),
    (5, "Electrical Principles"),
    (6, "Antennas & Feedlines"),
    (7, "Propagation"),
    (8, "Interference"),
];

pub async fn run(bank: &QuestionBank, db: &Db) -> Result<()> {
    ConceptClient::ensure_config();
    let ai_available = ConceptClient::is_available().await;
    if !ai_available {
        ConceptClient::on_no_backend();
    }

    let mut visited = db.get_visited_concepts()?;

    loop {
        let Some(section) = pick_section(bank, &visited)? else {
            break;
        };

        let Some((subsection, hint)) = pick_subsection(bank, db, section, &mut visited)? else {
            continue;
        };

        if !run_topic_session(
            bank,
            db,
            section,
            subsection,
            &hint,
            ai_available,
            &mut visited,
        )
        .await?
        {
            break;
        }
    }

    Ok(())
}

fn section_progress(bank: &QuestionBank, section: u8, visited: &HashSet<String>) -> (usize, usize) {
    let mut seen = std::collections::BTreeSet::new();
    let mut done = 0usize;
    for q in bank.by_section(section) {
        if seen.insert(q.subsection) {
            let key = format!("B-{section:03}-{:03}", q.subsection);
            if visited.contains(&key) {
                done += 1;
            }
        }
    }
    (done, seen.len())
}

fn pick_section(bank: &QuestionBank, visited: &HashSet<String>) -> Result<Option<u8>> {
    loop {
        println!();
        println!("  \x1b[1mLearn Mode\x1b[0m — Select a section");
        println!();
        for (i, (num, name)) in SECTION_NAMES.iter().enumerate() {
            let (done, total) = section_progress(bank, *num, visited);
            let badge = if total > 0 && done == total {
                " \x1b[32m✓\x1b[0m".to_string()
            } else if done > 0 {
                format!(" \x1b[2m({done}/{total})\x1b[0m")
            } else {
                String::new()
            };
            println!("  {}.  {}{}", i + 1, name, badge);
        }
        println!();
        print!("  Section (1–{}), or q to quit: ", SECTION_NAMES.len());
        io::stdout().flush()?;

        let line = read_line()?;
        match line.trim() {
            "q" | "Q" => return Ok(None),
            s => match s.parse::<usize>() {
                Ok(n) if n >= 1 && n <= SECTION_NAMES.len() => {
                    return Ok(Some(SECTION_NAMES[n - 1].0));
                }
                _ => println!("  Invalid choice."),
            },
        }
    }
}

fn pick_subsection(
    bank: &QuestionBank,
    db: &Db,
    section: u8,
    visited: &mut HashSet<String>,
) -> Result<Option<(u8, String)>> {
    let mut subsections: Vec<(u8, usize, String)> = Vec::new();
    let mut current_sub = 0u8;
    let mut count = 0usize;
    let mut first_text = String::new();

    for q in bank.by_section(section) {
        if q.subsection != current_sub {
            if current_sub > 0 {
                subsections.push((current_sub, count, first_text.clone()));
            }
            current_sub = q.subsection;
            count = 1;
            first_text = truncate(&q.text, 65);
        } else {
            count += 1;
        }
    }
    if current_sub > 0 {
        subsections.push((current_sub, count, first_text));
    }

    let section_name = section_name(section);

    loop {
        println!();
        println!("  \x1b[1m{section_name}\x1b[0m — Select a topic");
        println!();
        for (i, (sub, count, hint)) in subsections.iter().enumerate() {
            let key = format!("B-{section:03}-{sub:03}");
            let check = if visited.contains(&key) {
                " \x1b[32m✓\x1b[0m"
            } else {
                ""
            };
            println!(
                "  {:2}.  [{section}-{sub:03}]  {hint}  ({count}q){check}",
                i + 1
            );
        }
        println!();
        print!("  Topic number, r to reset progress, or b to go back: ");
        io::stdout().flush()?;

        let line = read_line()?;
        match line.trim() {
            "b" | "B" => return Ok(None),
            "r" | "R" => {
                run_reset_menu(db, section, section_name, &subsections, visited)?;
            }
            s => {
                if let Ok(n) = s.parse::<usize>() {
                    if n >= 1 && n <= subsections.len() {
                        let (sub, _, hint) = &subsections[n - 1];
                        return Ok(Some((*sub, hint.clone())));
                    }
                }
                println!("  Invalid choice.");
            }
        }
    }
}

fn run_reset_menu(
    db: &Db,
    section: u8,
    section_name: &str,
    subsections: &[(u8, usize, String)],
    visited: &mut HashSet<String>,
) -> Result<()> {
    loop {
        println!();
        print!("  Reset — topic number, a for all of {section_name}, or b to cancel: ");
        io::stdout().flush()?;

        let line = read_line()?;
        match line.trim() {
            "b" | "B" => return Ok(()),
            "a" | "A" => {
                print!("  Reset all progress for {section_name}? (y/n): ");
                io::stdout().flush()?;
                let confirm = read_line()?;
                if confirm.trim() == "y" || confirm.trim() == "Y" {
                    db.reset_concept_section(section)?;
                    let prefix = format!("B-{section:03}-");
                    visited.retain(|k| !k.starts_with(&prefix));
                    println!("  Progress reset for {section_name}.");
                }
                return Ok(());
            }
            s => {
                if let Ok(n) = s.parse::<usize>() {
                    if n >= 1 && n <= subsections.len() {
                        let (sub, _, _) = &subsections[n - 1];
                        let key = format!("B-{section:03}-{sub:03}");
                        db.reset_concept_topic(&key)?;
                        visited.remove(&key);
                        println!("  Topic {key} unmarked.");
                        return Ok(());
                    }
                }
                println!("  Invalid choice.");
            }
        }
    }
}

/// Returns Ok(true) to continue to the next topic, Ok(false) when the user quits.
async fn run_topic_session(
    bank: &QuestionBank,
    db: &Db,
    section: u8,
    subsection: u8,
    hint: &str,
    ai_available: bool,
    visited: &mut HashSet<String>,
) -> Result<bool> {
    let section_name = section_name(section);
    let related: Vec<_> = bank.by_subsection(section, subsection).collect();
    let key = format!("B-{section:03}-{subsection:03}");

    let mut messages: Vec<Message> = Vec::new();
    let mut client: Option<ConceptClient> = None;

    let pregenerated = crate::content::get_pregenerated_content(&key);

    if let Some(content) = pregenerated {
        println!();
        print_section_header(&format!("{section_name} — {key}"));
        println!();
        termimad::print_text(content);
        println!();
        print_section_header("Related Exam Questions");
        println!();
        print_exam_questions(&related);

        let initial_prompt =
            build_initial_prompt(section, subsection, section_name, hint, &related);
        messages.push(Message {
            role: "user",
            content: initial_prompt,
        });
        messages.push(Message {
            role: "assistant",
            content: content.to_string(),
        });
    } else if ai_available {
        client = Some(match ConceptClient::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("\n{}\n", e);
                return Ok(true);
            }
        });

        let initial_prompt =
            build_initial_prompt(section, subsection, section_name, hint, &related);
        messages.push(Message {
            role: "user",
            content: initial_prompt,
        });

        println!();
        println!("\x1b[2m  Thinking...\x1b[0m");
        let response = client.as_ref().unwrap().explain(messages.clone()).await?;

        print!("\x1b[1A\x1b[2K");
        println!();
        print_section_header(&format!("{section_name} — {key}"));
        println!();
        termimad::print_text(&response);
        println!();
        print_section_header("Related Exam Questions");
        println!();
        print_exam_questions(&related);

        messages.push(Message {
            role: "assistant",
            content: response,
        });
    } else {
        println!();
        print_section_header(&format!("{section_name} — {key}"));
        println!();
        println!("  No explanation available — no AI backend configured.");
        println!();
        print_section_header("Related Exam Questions");
        println!();
        print_exam_questions(&related);
    }

    // Mark visited as soon as content is shown
    db.mark_concept_visited(&key)?;
    visited.insert(key.clone());

    loop {
        println!();
        if ai_available {
            print!("  Follow-up question, n / Enter for next topic, or q to quit: ");
        } else {
            print!("  n / Enter for next topic, or q to quit: ");
        }
        io::stdout().flush()?;

        let line = read_line()?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed == "n" {
            break;
        }
        if trimmed == "q" || trimmed == "Q" {
            return Ok(false);
        }

        if !ai_available {
            println!("  Follow-up questions are disabled. Add an AI backend to enable them.");
            continue;
        }

        if client.is_none() {
            client = Some(match ConceptClient::new() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("\n  Could not connect to AI backend: {e}\n");
                    continue;
                }
            });
        }

        messages.push(Message {
            role: "user",
            content: trimmed.to_string(),
        });

        println!();
        println!("\x1b[2m  Thinking...\x1b[0m");
        let response = client.as_ref().unwrap().explain(messages.clone()).await?;

        print!("\x1b[1A\x1b[2K");
        println!();
        termimad::print_text(&response);

        messages.push(Message {
            role: "assistant",
            content: response,
        });
    }

    Ok(true)
}

fn build_initial_prompt(
    section: u8,
    subsection: u8,
    section_name: &str,
    hint: &str,
    related: &[&crate::questions::Question],
) -> String {
    let question_list: String = related
        .iter()
        .enumerate()
        .map(|(i, q)| {
            format!(
                "{}. {}\n   a) {} (correct)\n   b) {}\n   c) {}\n   d) {}",
                i + 1,
                q.text,
                q.correct_answer,
                q.incorrect_answers[0],
                q.incorrect_answers[1],
                q.incorrect_answers[2],
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if section <= 2 {
        format!(
            "I'm studying for the Canadian Amateur Radio Basic Qualification exam. \
             Teach me subsection B-{section:03}-{subsection:03} ({section_name}: {hint}).\n\n\
             This is a regulations and procedures section. Please:\n\
             1. Explain what these rules actually mean in plain language — what they require and why.\n\
             2. Give the context for why these rules exist (ITU framework, interference protection, \
                safety, operating conventions, etc.) — understanding the reason helps it stick.\n\
             3. Flag any nuances, edge cases, or common gotchas that trip people up.\n\
             4. End with a clear list of the specific facts I need to memorize for the exam.\n\n\
             The exam questions for this subsection are:\n\n{question_list}"
        )
    } else {
        format!(
            "Explain the concept covered by ISED Canadian Amateur Radio Basic Qualification \
             subsection B-{section:03}-{subsection:03} ({section_name}: {hint}).\n\n\
             The exam questions for this subsection are:\n\n{question_list}\n\n\
             After your explanation, briefly note what the exam questions are specifically testing."
        )
    }
}

fn print_exam_questions(questions: &[&crate::questions::Question]) {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    let labels = ['A', 'B', 'C', 'D'];

    for (i, q) in questions.iter().enumerate() {
        println!("  Q{}.  {}", i + 1, q.text);

        let mut answers: Vec<(&str, bool)> = vec![
            (q.correct_answer.as_str(), true),
            (q.incorrect_answers[0].as_str(), false),
            (q.incorrect_answers[1].as_str(), false),
            (q.incorrect_answers[2].as_str(), false),
        ];
        answers.shuffle(&mut rng);

        for (j, (ans, is_correct)) in answers.iter().enumerate() {
            if *is_correct {
                println!("        {}. {} \x1b[32m✓\x1b[0m", labels[j], ans);
            } else {
                println!("        {}. {}", labels[j], ans);
            }
        }
        println!();
    }
}

fn print_section_header(title: &str) {
    let width = 72;
    let line = "─".repeat(width);
    println!("\x1b[1m  {title}\x1b[0m");
    println!("  {line}");
}

fn section_name(section: u8) -> &'static str {
    SECTION_NAMES
        .iter()
        .find(|(n, _)| *n == section)
        .map(|(_, name)| *name)
        .unwrap_or("Unknown")
}

fn read_line() -> Result<String> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(line)
}

fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        None => s.to_string(),
        Some((i, _)) => format!("{}…", &s[..i]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_ascii() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn truncate_multibyte_utf8() {
        // "é" is 2 bytes — the old &s[..max] would panic if max split this codepoint
        let s = "café au lait";
        assert_eq!(truncate(s, 3), "caf…");
        assert_eq!(truncate(s, 4), "café…");
        assert_eq!(truncate(s, 12), "café au lait");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate("", 5), "");
    }
}
