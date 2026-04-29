use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Question {
    pub id: String,
    pub section: u8,
    pub subsection: u8,
    pub text: String,
    pub correct_answer: String,
    pub incorrect_answers: [String; 3],
}

impl Question {
    pub fn section_name(&self) -> &'static str {
        match self.section {
            1 => "Regulations & Licensing",
            2 => "Operating Procedures",
            3 => "Transmitters & Receivers",
            4 => "Electronics",
            5 => "Electrical Principles",
            6 => "Antennas & Feedlines",
            7 => "Propagation",
            8 => "Interference",
            _ => "Unknown",
        }
    }
}

pub struct QuestionBank {
    questions: Vec<Question>,
}

impl QuestionBank {
    pub fn load() -> Self {
        // Embedded at compile time by build.rs — no file I/O at runtime
        let json = include_str!(concat!(env!("OUT_DIR"), "/questions.json"));
        let questions: Vec<Question> =
            serde_json::from_str(json).expect("embedded question bank is malformed");
        Self { questions }
    }

    pub fn all(&self) -> &[Question] {
        &self.questions
    }

    pub fn by_section(&self, section: u8) -> impl Iterator<Item = &Question> {
        self.questions.iter().filter(move |q| q.section == section)
    }

    pub fn by_subsection(&self, section: u8, subsection: u8) -> impl Iterator<Item = &Question> {
        self.questions
            .iter()
            .filter(move |q| q.section == section && q.subsection == subsection)
    }
}
