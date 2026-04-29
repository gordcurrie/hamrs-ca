use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct Question {
    id: String,
    section: u8,
    subsection: u8,
    text: String,
    correct_answer: String,
    incorrect_answers: [String; 3],
}

fn main() {
    println!("cargo:rerun-if-changed=amat_basic_quest/amat_basic_quest_delim.txt");
    println!("cargo:rerun-if-changed=content");

    let src = fs::read_to_string("amat_basic_quest/amat_basic_quest_delim.txt")
        .expect("question bank file not found");

    let questions: Vec<Question> = src
        .lines()
        .skip(1) // header row
        .filter(|line| !line.trim().is_empty())
        .map(parse_question)
        .collect();

    let json = serde_json::to_string(&questions).expect("serialization failed");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("questions.json");
    fs::write(dest, json).expect("failed to write questions.json");

    generate_content_map(&out_dir);
}

fn generate_content_map(out_dir: &str) {
    let content_dir = Path::new("content");
    let mut entries: Vec<(String, String)> = Vec::new();

    if let Ok(dir) = fs::read_dir(content_dir) {
        let mut paths: Vec<_> = dir
            .flatten()
            .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
            .collect();
        paths.sort_by_key(|e| e.file_name());

        for entry in paths {
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());
            let key = path.file_stem().unwrap().to_string_lossy().to_string();
            let content = fs::read_to_string(&path).unwrap_or_default();
            entries.push((key, content));
        }
    }

    let mut rs = String::from(
        "pub fn get_pregenerated_content(key: &str) -> Option<&'static str> {\n    match key {\n",
    );
    for (key, content) in &entries {
        rs.push_str(&format!("        {:?} => Some({:?}),\n", key, content));
    }
    rs.push_str("        _ => None,\n    }\n}\n");

    let dest = Path::new(out_dir).join("content_map.rs");
    fs::write(dest, rs).expect("failed to write content_map.rs");
}

fn parse_question(line: &str) -> Question {
    let fields: Vec<&str> = line.splitn(11, ';').collect();
    assert!(fields.len() >= 6, "malformed line: {line}");

    let id = fields[0].trim().to_string();

    // ID format: B-SSS-SSS-QQQ
    let parts: Vec<&str> = id.splitn(4, '-').collect();
    let section: u8 = parts[1].parse().expect("invalid section");
    let subsection: u8 = parts[2].parse().expect("invalid subsection");

    Question {
        id,
        section,
        subsection,
        text: fields[1].trim().to_string(),
        correct_answer: fields[2].trim().to_string(),
        incorrect_answers: [
            fields[3].trim().to_string(),
            fields[4].trim().to_string(),
            fields[5].trim().to_string(),
        ],
    }
}
