mod ai;
mod content;
mod db;
mod modes;
mod questions;
mod tui;

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
}

#[tokio::main]
async fn main() -> Result<()> {
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
            print_stats(&db)?;
        }
    }

    Ok(())
}

fn print_stats(db: &Db) -> Result<()> {
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
    Ok(())
}
