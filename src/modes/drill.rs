use rand::seq::SliceRandom;
use std::io::{self, BufRead, Write};

pub struct Flashcard {
    pub prompt: String,
    pub answer: String,
}

pub struct DrillSession {
    pub label: &'static str,
    pub cards: Vec<Flashcard>,
}

struct QCode {
    code: &'static str,
    meaning: &'static str,
}

static Q_CODES: &[QCode] = &[
    QCode {
        code: "QRL?",
        meaning: "Is this frequency in use?",
    },
    QCode {
        code: "QRM",
        meaning: "I am being interfered with (man-made interference)",
    },
    QCode {
        code: "QRN",
        meaning: "I am troubled by static (natural noise)",
    },
    QCode {
        code: "QRS",
        meaning: "Send more slowly",
    },
    QCode {
        code: "QRX",
        meaning: "I will call you again (stand by)",
    },
    QCode {
        code: "QRZ?",
        meaning: "Who is calling me?",
    },
    QCode {
        code: "QSB",
        meaning: "Your signal is fading",
    },
    QCode {
        code: "QSY",
        meaning: "Change to another frequency",
    },
    QCode {
        code: "QTH",
        meaning: "My location is...",
    },
];

struct RValue {
    value: u8,
    meaning: &'static str,
}

static R_SCALE: &[RValue] = &[
    RValue {
        value: 1,
        meaning: "Unreadable",
    },
    RValue {
        value: 2,
        meaning: "Barely readable, occasional words",
    },
    RValue {
        value: 3,
        meaning: "Readable with considerable difficulty",
    },
    RValue {
        value: 4,
        meaning: "Readable with practically no difficulty",
    },
    RValue {
        value: 5,
        meaning: "Perfectly readable",
    },
];

struct SValue {
    value: u8,
    meaning: &'static str,
}

static S_SCALE: &[SValue] = &[
    SValue {
        value: 1,
        meaning: "Faintly perceptible",
    },
    SValue {
        value: 5,
        meaning: "Fairly good strength",
    },
    SValue {
        value: 7,
        meaning: "Moderately strong",
    },
    SValue {
        value: 9,
        meaning: "Extremely strong",
    },
];

static RST_FACTS: &[(&str, &str)] = &[
    (
        "What does RST stand for?",
        "Readability, Signal strength, Tone",
    ),
    (
        "How many digits does a voice (phone) signal report use?",
        "Two — RS (Readability and Signal strength; no Tone digit)",
    ),
    (
        "How many digits does a CW signal report use?",
        "Three — RST (Readability, Signal strength, Tone)",
    ),
    (
        "What does a signal report of \"5 7\" mean?",
        "Perfectly readable (R5), moderately strong signal (S7)",
    ),
    (
        "What does a signal report of \"3 3\" mean?",
        "Readable with considerable difficulty (R3), weak signal (S3)",
    ),
    ("How many dB equals one S-unit?", "6 dB"),
    (
        "Dropping from 100 W to 25 W changes your signal report by how much?",
        "One S-unit drop (6 dB = power divided by 4)",
    ),
    (
        "What does RST 579 mean in a Morse code contact?",
        "Perfectly readable (R5), moderately strong (S7), perfect CW tone (T9)",
    ),
    (
        "When asked for a signal report through a repeater, what do you report?",
        "The quality of the signal as heard through the repeater — not a direct estimate",
    ),
];

struct Phonetic {
    letter: char,
    word: &'static str,
}

static PHONETICS: &[Phonetic] = &[
    Phonetic {
        letter: 'A',
        word: "Alfa",
    },
    Phonetic {
        letter: 'B',
        word: "Bravo",
    },
    Phonetic {
        letter: 'C',
        word: "Charlie",
    },
    Phonetic {
        letter: 'D',
        word: "Delta",
    },
    Phonetic {
        letter: 'E',
        word: "Echo",
    },
    Phonetic {
        letter: 'F',
        word: "Foxtrot",
    },
    Phonetic {
        letter: 'G',
        word: "Golf",
    },
    Phonetic {
        letter: 'H',
        word: "Hotel",
    },
    Phonetic {
        letter: 'I',
        word: "India",
    },
    Phonetic {
        letter: 'J',
        word: "Juliett",
    },
    Phonetic {
        letter: 'K',
        word: "Kilo",
    },
    Phonetic {
        letter: 'L',
        word: "Lima",
    },
    Phonetic {
        letter: 'M',
        word: "Mike",
    },
    Phonetic {
        letter: 'N',
        word: "November",
    },
    Phonetic {
        letter: 'O',
        word: "Oscar",
    },
    Phonetic {
        letter: 'P',
        word: "Papa",
    },
    Phonetic {
        letter: 'Q',
        word: "Quebec",
    },
    Phonetic {
        letter: 'R',
        word: "Romeo",
    },
    Phonetic {
        letter: 'S',
        word: "Sierra",
    },
    Phonetic {
        letter: 'T',
        word: "Tango",
    },
    Phonetic {
        letter: 'U',
        word: "Uniform",
    },
    Phonetic {
        letter: 'V',
        word: "Victor",
    },
    Phonetic {
        letter: 'W',
        word: "Whiskey",
    },
    Phonetic {
        letter: 'X',
        word: "X-ray",
    },
    Phonetic {
        letter: 'Y',
        word: "Yankee",
    },
    Phonetic {
        letter: 'Z',
        word: "Zulu",
    },
];

struct DrillBand {
    name: &'static str,
    range: &'static str,
    status: &'static str,
    sub_band: Option<&'static str>,
    key_restriction: Option<&'static str>,
}

static DRILL_BANDS: &[DrillBand] = &[
    DrillBand {
        name: "LF",
        range: "135.7–137.8 kHz",
        status: "Secondary [2°]",
        sub_band: None,
        key_restriction: Some(
            "Shared with other services — frequently appears as a wrong-answer trap",
        ),
    },
    DrillBand {
        name: "160m",
        range: "1.8–2.0 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: None,
    },
    DrillBand {
        name: "80m",
        range: "3.5–4.0 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: None,
    },
    DrillBand {
        name: "40m",
        range: "7.0–7.3 MHz",
        status: "Primary [1°]",
        sub_band: Some("7.0–7.1 MHz: must not interfere with other services"),
        key_restriction: None,
    },
    DrillBand {
        name: "30m",
        range: "10.1–10.15 MHz",
        status: "Secondary [2°]",
        sub_band: None,
        key_restriction: Some("CW and digital only — no phone transmissions, no contests"),
    },
    DrillBand {
        name: "20m",
        range: "14.0–14.35 MHz",
        status: "Primary [1°]",
        sub_band: Some("14.0–14.2 MHz: must not interfere with other services"),
        key_restriction: None,
    },
    DrillBand {
        name: "17m",
        range: "18.068–18.168 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: None,
    },
    DrillBand {
        name: "15m",
        range: "21.0–21.45 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: None,
    },
    DrillBand {
        name: "12m",
        range: "24.89–24.99 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: None,
    },
    DrillBand {
        name: "10m",
        range: "28.0–29.7 MHz",
        status: "Primary [1°]",
        sub_band: Some("29.5–29.7 MHz sub-band"),
        key_restriction: None,
    },
    DrillBand {
        name: "6m",
        range: "50–54 MHz",
        status: "Primary [1°]",
        sub_band: Some("53–54 MHz sub-band"),
        key_restriction: None,
    },
    DrillBand {
        name: "2m",
        range: "144–148 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: Some("Protected from interference by other services"),
    },
    DrillBand {
        name: "1.25m",
        range: "222–225 MHz",
        status: "Primary [1°]",
        sub_band: None,
        key_restriction: None,
    },
    DrillBand {
        name: "70cm",
        range: "430–450 MHz",
        status: "Secondary [2°]",
        sub_band: None,
        key_restriction: Some("Must not cause interference to other radio services"),
    },
    DrillBand {
        name: "33cm",
        range: "902–928 MHz",
        status: "Secondary [2°]",
        sub_band: None,
        key_restriction: Some(
            "Not protected from interference; may be heavily occupied by licence-exempt devices",
        ),
    },
];

fn q_code_cards() -> Vec<Flashcard> {
    let mut cards = Vec::new();
    for q in Q_CODES {
        cards.push(Flashcard {
            prompt: format!("What does {} mean in amateur radio?", q.code),
            answer: q.meaning.to_string(),
        });
        cards.push(Flashcard {
            prompt: format!("What Q code means \"{}\"?", q.meaning),
            answer: q.code.to_string(),
        });
    }
    cards
}

fn rst_cards() -> Vec<Flashcard> {
    let mut cards = Vec::new();
    for r in R_SCALE {
        cards.push(Flashcard {
            prompt: format!("What does Readability {} (R{}) mean?", r.value, r.value),
            answer: r.meaning.to_string(),
        });
        cards.push(Flashcard {
            prompt: format!("Which Readability value means \"{}\"?", r.meaning),
            answer: format!("R{}", r.value),
        });
    }
    for s in S_SCALE {
        cards.push(Flashcard {
            prompt: format!("What does Signal Strength {} (S{}) mean?", s.value, s.value),
            answer: s.meaning.to_string(),
        });
        cards.push(Flashcard {
            prompt: format!("Which Signal Strength value means \"{}\"?", s.meaning),
            answer: format!("S{}", s.value),
        });
    }
    for (prompt, answer) in RST_FACTS {
        cards.push(Flashcard {
            prompt: prompt.to_string(),
            answer: answer.to_string(),
        });
    }
    cards
}

fn phonetic_cards() -> Vec<Flashcard> {
    let mut cards = Vec::new();
    for p in PHONETICS {
        cards.push(Flashcard {
            prompt: format!("What ITU phonetic word represents the letter {}?", p.letter),
            answer: p.word.to_string(),
        });
        cards.push(Flashcard {
            prompt: format!(
                "\"{}\" represents which letter in the ITU phonetic alphabet?",
                p.word
            ),
            answer: p.letter.to_string(),
        });
    }
    cards
}

fn band_cards() -> Vec<Flashcard> {
    let mut cards = Vec::new();
    for band in DRILL_BANDS {
        cards.push(Flashcard {
            prompt: format!("What is the frequency range of the {} band?", band.name),
            answer: band.range.to_string(),
        });
        cards.push(Flashcard {
            prompt: format!("Which amateur band covers {}?", band.range),
            answer: band.name.to_string(),
        });
        cards.push(Flashcard {
            prompt: format!(
                "Is the {} band a Primary or Secondary allocation?",
                band.name
            ),
            answer: band.status.to_string(),
        });
        if let Some(sub) = band.sub_band {
            cards.push(Flashcard {
                prompt: format!(
                    "What sub-band restriction applies within the {} band?",
                    band.name
                ),
                answer: sub.to_string(),
            });
        }
        if let Some(restriction) = band.key_restriction {
            cards.push(Flashcard {
                prompt: format!("What is the key exam fact about the {} band?", band.name),
                answer: restriction.to_string(),
            });
        }
    }
    cards
}

pub fn pick_session() -> anyhow::Result<Option<DrillSession>> {
    println!();
    println!("  \x1b[1mDrill — Memorization Study\x1b[0m");
    println!();
    println!("  Select a category:");
    println!("    [q]  Q Codes           ({} cards)", Q_CODES.len() * 2);
    println!(
        "    [r]  RST Signal Reports ({} cards)",
        R_SCALE.len() * 2 + S_SCALE.len() * 2 + RST_FACTS.len()
    );
    println!(
        "    [p]  Phonetic Alphabet  ({} cards)",
        PHONETICS.len() * 2
    );
    let band_count: usize = DRILL_BANDS
        .iter()
        .map(|b| 3 + b.sub_band.is_some() as usize + b.key_restriction.is_some() as usize)
        .sum();
    println!("    [b]  Frequency Bands    ({band_count} cards)");
    println!();

    let stdin = io::stdin();
    let mut lock = stdin.lock();
    loop {
        print!("  > ");
        io::stdout().flush()?;
        let mut input = String::new();
        if lock.read_line(&mut input)? == 0 {
            return Ok(None);
        }
        let choice = input.trim().to_lowercase();
        let (label, mut cards): (&'static str, Vec<Flashcard>) = match choice.as_str() {
            "q" => ("Q Codes", q_code_cards()),
            "r" => ("RST Signal Reports", rst_cards()),
            "p" => ("Phonetic Alphabet", phonetic_cards()),
            "b" => ("Frequency Bands", band_cards()),
            "" => return Ok(None),
            _ => {
                println!("  Please enter q, r, p, or b.");
                continue;
            }
        };
        cards.shuffle(&mut rand::rng());
        return Ok(Some(DrillSession { label, cards }));
    }
}
