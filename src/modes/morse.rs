use crate::morse;
use anyhow::{bail, Result};
use rand::seq::SliceRandom;
use std::io::{self, BufRead, Write};

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum MorseMode {
    Receive,
    Transmit,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Charset {
    Letters,
    Numbers,
    Both,
}

pub struct MorseConfig {
    pub mode: MorseMode,
    pub wpm: u32,
    pub charset: Charset,
    pub count: usize,
}

pub struct MorseItem {
    pub character: char,
    pub code: &'static str,
}

pub struct MorseSession {
    pub config: MorseConfig,
    pub items: Vec<MorseItem>,
}

impl MorseSession {
    pub fn build(mut config: MorseConfig) -> Self {
        let pool: Vec<(char, &'static str)> = morse::TABLE
            .iter()
            .filter(|(ch, _)| match config.charset {
                Charset::Letters => ch.is_ascii_alphabetic(),
                Charset::Numbers => ch.is_ascii_digit(),
                Charset::Both => true,
            })
            .map(|(ch, code)| (*ch, *code))
            .collect();

        let mut rng = rand::thread_rng();
        // Clamp and keep config in sync so session.config.count == session.items.len()
        config.count = config.count.min(pool.len());

        let items: Vec<MorseItem> = pool
            .choose_multiple(&mut rng, config.count)
            .map(|(ch, code)| MorseItem {
                character: *ch,
                code,
            })
            .collect();

        MorseSession { config, items }
    }
}

pub fn setup(
    mode_flag: Option<MorseMode>,
    wpm_flag: Option<u32>,
    count_flag: Option<usize>,
) -> Result<Option<MorseSession>> {
    println!();
    println!("  \x1b[1mMorse Code Practice\x1b[0m");
    println!();

    let mode = match mode_flag {
        Some(m) => m,
        None => match prompt_mode()? {
            Some(m) => m,
            None => return Ok(None),
        },
    };

    let wpm = match wpm_flag {
        Some(w) => {
            if !(1..=99).contains(&w) {
                bail!("--wpm must be between 1 and 99");
            }
            w
        }
        None => match prompt_wpm()? {
            Some(w) => w,
            None => return Ok(None),
        },
    };

    let charset = match prompt_charset()? {
        Some(c) => c,
        None => return Ok(None),
    };

    let count = match count_flag {
        Some(c) => {
            if c == 0 {
                bail!("--count must be at least 1");
            }
            c
        }
        None => match prompt_count()? {
            Some(c) => c,
            None => return Ok(None),
        },
    };

    let max_for_charset = match charset {
        Charset::Letters => 26,
        Charset::Numbers => 10,
        Charset::Both => 36,
    };
    if count > max_for_charset {
        println!(
            "  (only {} characters in selected charset — session will have {} items)",
            max_for_charset, max_for_charset
        );
    }

    println!();

    Ok(Some(MorseSession::build(MorseConfig {
        mode,
        wpm,
        charset,
        count,
    })))
}

fn prompt_mode() -> Result<Option<MorseMode>> {
    loop {
        println!("  Mode:");
        println!("    1.  Receive  — Morse shown, you type the character");
        println!("    2.  Transmit — character shown, you type the Morse");
        println!("    3.  Both     — mixed");
        println!();
        print!("  Choice (1–3), or q to quit: ");
        io::stdout().flush()?;

        let line = read_line()?;
        match line.trim() {
            "q" | "Q" => return Ok(None),
            "1" => return Ok(Some(MorseMode::Receive)),
            "2" => return Ok(Some(MorseMode::Transmit)),
            "3" => return Ok(Some(MorseMode::Both)),
            _ => println!("  Invalid choice.\n"),
        }
    }
}

fn prompt_wpm() -> Result<Option<u32>> {
    const PRESETS: &[u32] = &[5, 10, 13, 15, 20];
    loop {
        println!();
        println!("  Target WPM:");
        for (i, &w) in PRESETS.iter().enumerate() {
            println!("    {}.  {} WPM", i + 1, w);
        }
        println!("    c.  Custom");
        println!();
        print!("  Choice (1–{}), c, or q to quit: ", PRESETS.len());
        io::stdout().flush()?;

        let line = read_line()?;
        match line.trim() {
            "q" | "Q" => return Ok(None),
            "c" | "C" => {
                print!("  WPM (1–99): ");
                io::stdout().flush()?;
                let raw = read_line()?;
                match raw.trim().parse::<u32>() {
                    Ok(w) if (1..=99).contains(&w) => return Ok(Some(w)),
                    _ => println!("  Enter a number between 1 and 99.\n"),
                }
            }
            s => match s.parse::<usize>() {
                Ok(n) if n >= 1 && n <= PRESETS.len() => return Ok(Some(PRESETS[n - 1])),
                _ => println!("  Invalid choice.\n"),
            },
        }
    }
}

fn prompt_charset() -> Result<Option<Charset>> {
    loop {
        println!();
        println!("  Characters:");
        println!("    1.  Letters only (A–Z)");
        println!("    2.  Numbers only (0–9)");
        println!("    3.  Letters + numbers");
        println!();
        print!("  Choice (1–3), or q to quit: ");
        io::stdout().flush()?;

        let line = read_line()?;
        match line.trim() {
            "q" | "Q" => return Ok(None),
            "1" => return Ok(Some(Charset::Letters)),
            "2" => return Ok(Some(Charset::Numbers)),
            "3" => return Ok(Some(Charset::Both)),
            _ => println!("  Invalid choice.\n"),
        }
    }
}

fn prompt_count() -> Result<Option<usize>> {
    loop {
        println!();
        print!("  Items per session [20]: ");
        io::stdout().flush()?;

        let line = read_line()?;
        let s = line.trim();
        match s {
            "" => return Ok(Some(20)),
            "q" | "Q" => return Ok(None),
            _ => match s.parse::<usize>() {
                Ok(n) if n >= 1 => return Ok(Some(n)),
                _ => println!("  Enter a positive number.\n"),
            },
        }
    }
}

fn read_line() -> Result<String> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(charset: Charset, count: usize) -> MorseConfig {
        MorseConfig {
            mode: MorseMode::Receive,
            wpm: 13,
            charset,
            count,
        }
    }

    #[test]
    fn build_letters_only_yields_letter_items() {
        let session = MorseSession::build(config(Charset::Letters, 26));
        assert!(!session.items.is_empty());
        assert!(session
            .items
            .iter()
            .all(|item| item.character.is_ascii_alphabetic()));
    }

    #[test]
    fn build_numbers_only_yields_digit_items() {
        let session = MorseSession::build(config(Charset::Numbers, 10));
        assert!(!session.items.is_empty());
        assert!(session
            .items
            .iter()
            .all(|item| item.character.is_ascii_digit()));
    }

    #[test]
    fn build_clamps_count_to_pool_and_keeps_config_consistent() {
        let session = MorseSession::build(config(Charset::Numbers, 99));
        assert_eq!(session.items.len(), 10);
        assert_eq!(session.config.count, 10);
    }

    #[test]
    fn build_no_duplicate_characters() {
        let session = MorseSession::build(config(Charset::Both, 36));
        let mut chars: Vec<char> = session.items.iter().map(|i| i.character).collect();
        chars.sort();
        chars.dedup();
        assert_eq!(chars.len(), session.items.len());
    }

    #[test]
    fn build_items_have_correct_morse_codes() {
        let session = MorseSession::build(config(Charset::Both, 36));
        for item in &session.items {
            assert_eq!(crate::morse::encode(item.character), Some(item.code));
        }
    }
}
