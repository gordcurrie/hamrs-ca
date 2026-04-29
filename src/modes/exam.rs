use crate::db::Db;
use crate::questions::{Question, QuestionBank};
use rand::seq::SliceRandom;
use std::collections::HashMap;
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

/// Returns Ok(None) if the user quits, Ok(Some(None)) for all sections,
/// Ok(Some(Some(vec))) for a specific section.
pub fn pick_sections() -> anyhow::Result<Option<Option<Vec<u8>>>> {
    loop {
        println!();
        println!("  \x1b[1mPractice Quiz\x1b[0m — Choose sections");
        println!();
        println!("  0.  All sections");
        for (i, (_, name)) in SECTION_NAMES.iter().enumerate() {
            println!("  {}.  {}", i + 1, name);
        }
        println!();
        print!(
            "  Section (0=all, 1–{}), or q to quit: ",
            SECTION_NAMES.len()
        );
        io::stdout().flush()?;

        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        match line.trim() {
            "q" | "Q" => return Ok(None),
            "0" => return Ok(Some(None)),
            s => match s.parse::<usize>() {
                Ok(n) if n >= 1 && n <= SECTION_NAMES.len() => {
                    return Ok(Some(Some(vec![SECTION_NAMES[n - 1].0])));
                }
                _ => println!("  Invalid choice."),
            },
        }
    }
}

pub struct QuizSession {
    pub questions: Vec<ShuffledQuestion>,
    pub mode_label: &'static str,
    pub time_limit_secs: Option<u64>,
}

pub struct ShuffledQuestion {
    pub question: Question,
    pub answers: Vec<String>,
    pub correct_index: usize,
}

impl ShuffledQuestion {
    fn new(q: Question) -> Self {
        let mut rng = rand::thread_rng();

        // Build answer list with correct at index 0, then shuffle
        let mut indexed: Vec<(usize, String)> = vec![
            (0, q.correct_answer.clone()),
            (1, q.incorrect_answers[0].clone()),
            (2, q.incorrect_answers[1].clone()),
            (3, q.incorrect_answers[2].clone()),
        ];
        indexed.shuffle(&mut rng);

        let correct_index = indexed.iter().position(|(orig, _)| *orig == 0).unwrap();
        let answers = indexed.into_iter().map(|(_, s)| s).collect();

        Self {
            question: q,
            answers,
            correct_index,
        }
    }
}

pub fn build_practice_session(
    bank: &QuestionBank,
    db: &Db,
    sections: Option<&[u8]>,
    count: usize,
) -> anyhow::Result<QuizSession> {
    let pool: Vec<&Question> = bank
        .all()
        .iter()
        .filter(|q| sections.is_none_or(|s| s.contains(&q.section)))
        .collect();
    let questions = weighted_sample(pool, db, count)?;
    Ok(QuizSession {
        questions: questions.into_iter().map(ShuffledQuestion::new).collect(),
        mode_label: "Practice",
        time_limit_secs: None,
    })
}

pub fn build_exam_session(bank: &QuestionBank, db: &Db) -> anyhow::Result<QuizSession> {
    let pool: Vec<&Question> = bank.all().iter().collect();
    let questions = weighted_sample(pool, db, 100)?;
    Ok(QuizSession {
        questions: questions.into_iter().map(ShuffledQuestion::new).collect(),
        mode_label: "Full Exam",
        time_limit_secs: Some(90 * 60), // 90 minutes
    })
}

fn weighted_sample(pool: Vec<&Question>, db: &Db, count: usize) -> anyhow::Result<Vec<Question>> {
    let ids: Vec<String> = pool.iter().map(|q| q.id.clone()).collect();
    let stats = db.stats_for_questions(&ids)?;
    let weight_map: HashMap<&str, u32> = stats
        .iter()
        .map(|s| (s.question_id.as_str(), s.weight()))
        .collect();

    let mut rng = rand::thread_rng();

    // Build weighted index pool: each question repeated by its weight, then shuffle + dedup
    // This gives higher-weight questions a proportionally better chance of appearing early
    let mut weighted: Vec<usize> = pool
        .iter()
        .enumerate()
        .flat_map(|(i, q)| {
            let w = *weight_map.get(q.id.as_str()).unwrap_or(&3);
            std::iter::repeat_n(i, w as usize)
        })
        .collect();
    weighted.shuffle(&mut rng);

    let mut seen = vec![false; pool.len()];
    let selected: Vec<Question> = weighted
        .into_iter()
        .filter_map(|i| {
            if seen[i] {
                return None;
            }
            seen[i] = true;
            Some(pool[i].clone())
        })
        .take(count)
        .collect();

    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn make_question(id: &str) -> Question {
        Question {
            id: id.to_string(),
            section: 1,
            subsection: 1,
            text: "Test question".to_string(),
            correct_answer: "A".to_string(),
            incorrect_answers: ["B".to_string(), "C".to_string(), "D".to_string()],
        }
    }

    fn fresh_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn sample_returns_requested_count() {
        let db = fresh_db();
        let questions: Vec<Question> = (1..=30)
            .map(|i| make_question(&format!("B-001-001-{i:03}")))
            .collect();
        let pool: Vec<&Question> = questions.iter().collect();
        let result = weighted_sample(pool, &db, 20).unwrap();
        assert_eq!(result.len(), 20);
    }

    #[test]
    fn sample_no_duplicates() {
        let db = fresh_db();
        let questions: Vec<Question> = (1..=30)
            .map(|i| make_question(&format!("B-001-001-{i:03}")))
            .collect();
        let pool: Vec<&Question> = questions.iter().collect();
        let result = weighted_sample(pool, &db, 20).unwrap();
        let mut ids: Vec<&str> = result.iter().map(|q| q.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 20);
    }

    #[test]
    fn sample_pool_smaller_than_count_returns_all() {
        let db = fresh_db();
        let questions: Vec<Question> = (1..=5)
            .map(|i| make_question(&format!("B-001-001-{i:03}")))
            .collect();
        let pool: Vec<&Question> = questions.iter().collect();
        let result = weighted_sample(pool, &db, 20).unwrap();
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn sample_empty_pool_returns_empty() {
        let db = fresh_db();
        let result = weighted_sample(vec![], &db, 10).unwrap();
        assert!(result.is_empty());
    }
}
