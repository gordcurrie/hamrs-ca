use crate::morse;
use anyhow::Result;
use rand::seq::SliceRandom;
use std::io::{self, BufRead, Write};

#[derive(Debug, Clone, Copy, PartialEq)]
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
    pub fn build(config: MorseConfig) -> Self {
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
        let count = config.count.min(pool.len());

        let items: Vec<MorseItem> = pool
            .choose_multiple(&mut rng, count)
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
        None => prompt_mode()?,
    };

    let wpm = match wpm_flag {
        Some(w) => w,
        None => prompt_wpm()?,
    };

    let charset = prompt_charset()?;

    let count = match count_flag {
        Some(c) => c,
        None => prompt_count()?,
    };

    println!();

    Ok(Some(MorseSession::build(MorseConfig {
        mode,
        wpm,
        charset,
        count,
    })))
}

fn prompt_mode() -> Result<MorseMode> {
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
            "q" | "Q" => std::process::exit(0),
            "1" => return Ok(MorseMode::Receive),
            "2" => return Ok(MorseMode::Transmit),
            "3" => return Ok(MorseMode::Both),
            _ => println!("  Invalid choice.\n"),
        }
    }
}

fn prompt_wpm() -> Result<u32> {
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
            "q" | "Q" => std::process::exit(0),
            "c" | "C" => {
                print!("  WPM (1–99): ");
                io::stdout().flush()?;
                let raw = read_line()?;
                match raw.trim().parse::<u32>() {
                    Ok(w) if (1..=99).contains(&w) => return Ok(w),
                    _ => println!("  Enter a number between 1 and 99.\n"),
                }
            }
            s => match s.parse::<usize>() {
                Ok(n) if n >= 1 && n <= PRESETS.len() => return Ok(PRESETS[n - 1]),
                _ => println!("  Invalid choice.\n"),
            },
        }
    }
}

fn prompt_charset() -> Result<Charset> {
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
            "q" | "Q" => std::process::exit(0),
            "1" => return Ok(Charset::Letters),
            "2" => return Ok(Charset::Numbers),
            "3" => return Ok(Charset::Both),
            _ => println!("  Invalid choice.\n"),
        }
    }
}

fn prompt_count() -> Result<usize> {
    loop {
        println!();
        print!("  Items per session [20]: ");
        io::stdout().flush()?;

        let line = read_line()?;
        let s = line.trim();
        if s.is_empty() || s == "q" || s == "Q" {
            if s == "q" || s == "Q" {
                std::process::exit(0);
            }
            return Ok(20);
        }
        match s.parse::<usize>() {
            Ok(n) if n >= 1 => return Ok(n),
            _ => println!("  Enter a positive number.\n"),
        }
    }
}

fn read_line() -> Result<String> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line)
}
