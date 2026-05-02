use crate::questions::QuestionBank;
use std::collections::HashSet;

const CHART_COLS: usize = 44;
const LOG_LO: f64 = 5.0; // log10(100 kHz)
const LOG_HI: f64 = 9.0; // log10(1 GHz)

#[derive(Clone, Copy)]
enum Status {
    Primary,
    Secondary,
}

struct Band {
    name: &'static str,
    range: &'static str,
    low_hz: f64,
    high_hz: f64,
    status: Status,
    exam_note: &'static str,
    // Substrings that identify this band in question text / answers
    search_terms: &'static [&'static str],
}

fn hz_to_col(hz: f64) -> usize {
    let log = hz.log10().clamp(LOG_LO, LOG_HI);
    ((log - LOG_LO) / (LOG_HI - LOG_LO) * CHART_COLS as f64).round() as usize
}

static BANDS: &[Band] = &[
    Band {
        name: "LF",
        range: "135.7–137.8 kHz",
        low_hz: 135_700.0,
        high_hz: 137_800.0,
        status: Status::Secondary,
        exam_note: "Secondary; shared with other services — appears as a wrong-answer trap",
        search_terms: &["135.7", "137.8"],
    },
    Band {
        name: "160m",
        range: "1.8–2.0 MHz",
        low_hz: 1_800_000.0,
        high_hz: 2_000_000.0,
        status: Status::Primary,
        exam_note: "Primary allocation",
        search_terms: &["1.8 MHz", "1.800 MHz"],
    },
    Band {
        name: "80m",
        range: "3.5–4.0 MHz",
        low_hz: 3_500_000.0,
        high_hz: 4_000_000.0,
        status: Status::Primary,
        exam_note: "Primary allocation",
        search_terms: &["3.5 MHz to 4"],
    },
    Band {
        name: "40m",
        range: "7.0–7.3 MHz",
        low_hz: 7_000_000.0,
        high_hz: 7_300_000.0,
        status: Status::Primary,
        exam_note: "Primary; sub-band 7.0–7.1 MHz: must not interfere with other services",
        search_terms: &["7.0 MHz to 7", "7.000 MHz", "7.1 MHz"],
    },
    Band {
        name: "30m",
        range: "10.1–10.15 MHz",
        low_hz: 10_100_000.0,
        high_hz: 10_150_000.0,
        status: Status::Secondary,
        exam_note: "Secondary; CW and digital only — no phone, no contests",
        search_terms: &["10.100 MHz", "10.150 MHz"],
    },
    Band {
        name: "20m",
        range: "14.0–14.35 MHz",
        low_hz: 14_000_000.0,
        high_hz: 14_350_000.0,
        status: Status::Primary,
        exam_note: "Primary; sub-band 14.0–14.2 MHz: must not interfere with other services",
        search_terms: &["14.0 MHz", "14.000 MHz", "14.2 MHz"],
    },
    Band {
        name: "17m",
        range: "18.068–18.168 MHz",
        low_hz: 18_068_000.0,
        high_hz: 18_168_000.0,
        status: Status::Primary,
        exam_note: "Primary allocation",
        search_terms: &["18.068", "18.168"],
    },
    Band {
        name: "15m",
        range: "21.0–21.45 MHz",
        low_hz: 21_000_000.0,
        high_hz: 21_450_000.0,
        status: Status::Primary,
        exam_note: "Primary allocation",
        search_terms: &["21.000 MHz", "21.450 MHz"],
    },
    Band {
        name: "12m",
        range: "24.89–24.99 MHz",
        low_hz: 24_890_000.0,
        high_hz: 24_990_000.0,
        status: Status::Primary,
        exam_note: "Primary allocation",
        search_terms: &["24.89", "24.99"],
    },
    Band {
        name: "10m",
        range: "28.0–29.7 MHz",
        low_hz: 28_000_000.0,
        high_hz: 29_700_000.0,
        status: Status::Primary,
        exam_note: "Primary; sub-band 29.5–29.7 MHz",
        search_terms: &["28.000 MHz", "29.500 MHz", "29.700 MHz", "29.5 MHz", "29.7 MHz"],
    },
    Band {
        name: "6m",
        range: "50–54 MHz",
        low_hz: 50_000_000.0,
        high_hz: 54_000_000.0,
        status: Status::Primary,
        exam_note: "Primary; sub-band 53–54 MHz",
        search_terms: &["50.000 MHz", "50 MHz to 54", "53 MHz to 54"],
    },
    Band {
        name: "2m",
        range: "144–148 MHz",
        low_hz: 144_000_000.0,
        high_hz: 148_000_000.0,
        status: Status::Primary,
        exam_note: "Primary; protected from interference by other services",
        search_terms: &["144 MHz to 148", "144.0 MHz to 148", "145 MHz to 148"],
    },
    Band {
        name: "1.25m",
        range: "222–225 MHz",
        low_hz: 222_000_000.0,
        high_hz: 225_000_000.0,
        status: Status::Primary,
        exam_note: "Primary allocation",
        search_terms: &["222 MHz to 225"],
    },
    Band {
        name: "70cm",
        range: "430–450 MHz",
        low_hz: 430_000_000.0,
        high_hz: 450_000_000.0,
        status: Status::Secondary,
        exam_note: "Secondary; must not cause interference to other radio services",
        search_terms: &["430 MHz to 450", "430.0 MHz to 450"],
    },
    Band {
        name: "33cm",
        range: "902–928 MHz",
        low_hz: 902_000_000.0,
        high_hz: 928_000_000.0,
        status: Status::Secondary,
        exam_note: "Secondary; not protected from interference; may be heavily occupied by licence-exempt devices",
        search_terms: &["902 MHz to 928"],
    },
];

fn related_subsections(band: &Band, bank: &QuestionBank) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::new();

    for q in bank.all() {
        let haystack = format!(
            "{} {} {} {} {}",
            q.text,
            q.correct_answer,
            q.incorrect_answers[0],
            q.incorrect_answers[1],
            q.incorrect_answers[2],
        );
        if band.search_terms.iter().any(|t| haystack.contains(t)) {
            let key = format!("B-{:03}-{:03}", q.section, q.subsection);
            if seen.insert(key.clone()) {
                result.push(key);
            }
        }
    }
    result.sort();
    result
}

pub fn run(bank: &QuestionBank) {
    println!();
    println!("  \x1b[1mAmateur Radio Bands — Canadian Basic Qualification\x1b[0m");
    println!();
    println!(
        "  \x1b[32m█\x1b[0m Primary [1°]    allocated to amateurs; protected from interference"
    );
    println!("  \x1b[33m█\x1b[0m Secondary [2°]  must not interfere with other services; may not be protected");
    println!();
    print_spectrum();
    println!();
    print_reference(bank);
    println!();
}

fn print_spectrum() {
    // Decade positions within CHART_COLS=44: 100kHz=0, 1MHz=11, 10MHz=22, 100MHz=33, 1GHz=44
    println!("  100kHz   1MHz       10MHz     100MHz      1GHz");

    // Ruler: ├ at 0, ┼ at 11/22/33, ┤ at 44
    let ruler: String = (0..=CHART_COLS)
        .map(|i| match i {
            0 => '├',
            11 | 22 | 33 => '┼',
            44 => '┤',
            _ => '─',
        })
        .collect();
    println!("  {ruler}");

    for band in BANDS {
        let start = hz_to_col(band.low_hz).min(CHART_COLS.saturating_sub(3));
        let natural_end = hz_to_col(band.high_hz);
        let end = natural_end.max(start + 3).min(CHART_COLS);
        let width = end - start;
        let inner = width.saturating_sub(2).max(1);

        let (color, reset) = match band.status {
            Status::Primary => ("\x1b[32m", "\x1b[0m"),
            Status::Secondary => ("\x1b[33m", "\x1b[0m"),
        };

        let bar = format!("{color}[{}]{reset}", "─".repeat(inner));
        let pad_right = " ".repeat(CHART_COLS.saturating_sub(end));

        let badge = match band.status {
            Status::Primary => "\x1b[32m[1°]\x1b[0m",
            Status::Secondary => "\x1b[33m[2°]\x1b[0m",
        };

        println!(
            "  {}{bar}{pad_right}  {:<5} {:<18} {badge}",
            " ".repeat(start),
            band.name,
            band.range,
        );
    }
}

fn print_reference(bank: &QuestionBank) {
    println!("  {}", "─".repeat(72));
    println!("  \x1b[1mExam Key Facts\x1b[0m");
    println!();

    for band in BANDS {
        let badge = match band.status {
            Status::Primary => "\x1b[32m[1°]\x1b[0m",
            Status::Secondary => "\x1b[33m[2°]\x1b[0m",
        };

        let refs = related_subsections(band, bank);
        let ref_str = if refs.is_empty() {
            String::new()
        } else {
            format!("  \x1b[2m→ {}\x1b[0m", refs.join(", "))
        };

        println!(
            "  {badge} \x1b[1m{:<6}\x1b[0m {}{}",
            band.name, band.exam_note, ref_str
        );
    }
}
