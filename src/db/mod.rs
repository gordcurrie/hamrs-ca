use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct QuestionStats {
    pub question_id: String,
    pub attempts: u32,
    pub correct: u32,
}

impl QuestionStats {
    pub fn weight(&self) -> u32 {
        if self.attempts == 0 {
            return 3;
        }
        let ratio = self.correct as f32 / self.attempts as f32;
        if ratio >= 0.9 {
            1
        } else if ratio >= 0.6 {
            2
        } else {
            4
        }
    }
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS sessions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                mode        TEXT NOT NULL,
                started_at  INTEGER NOT NULL,
                finished_at INTEGER,
                score       INTEGER,
                total       INTEGER
            );

            CREATE TABLE IF NOT EXISTS attempts (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  INTEGER NOT NULL REFERENCES sessions(id),
                question_id TEXT NOT NULL,
                correct     INTEGER NOT NULL,
                timestamp   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_attempts_question
                ON attempts(question_id);
        ")?;
        Ok(())
    }

    pub fn start_session(&self, mode: &str) -> Result<i64> {
        let now = unix_now();
        self.conn.execute(
            "INSERT INTO sessions (mode, started_at) VALUES (?1, ?2)",
            params![mode, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_session(&self, session_id: i64, score: u32, total: u32) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "UPDATE sessions SET finished_at=?1, score=?2, total=?3 WHERE id=?4",
            params![now, score, total, session_id],
        )?;
        Ok(())
    }

    pub fn record_attempt(&self, session_id: i64, question_id: &str, correct: bool) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "INSERT INTO attempts (session_id, question_id, correct, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, question_id, correct as i32, now],
        )?;
        Ok(())
    }

    pub fn stats_for_questions(&self, ids: &[String]) -> Result<Vec<QuestionStats>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders: String = (1..=ids.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT question_id, COUNT(*) as attempts, SUM(correct) as correct
             FROM attempts
             WHERE question_id IN ({placeholders})
             GROUP BY question_id"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let found: HashMap<String, QuestionStats> = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                Ok(QuestionStats {
                    question_id: row.get(0)?,
                    attempts: row.get::<_, u32>(1)?,
                    correct: row.get::<_, u32>(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|s| (s.question_id.clone(), s))
            .collect();

        Ok(ids
            .iter()
            .map(|id| {
                found.get(id).cloned().unwrap_or_else(|| QuestionStats {
                    question_id: id.clone(),
                    attempts: 0,
                    correct: 0,
                })
            })
            .collect())
    }

    pub fn recent_sessions(&self, limit: u32) -> Result<Vec<(String, u32, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT mode, score, total
             FROM sessions
             WHERE finished_at IS NOT NULL
             ORDER BY started_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(base.join("hamrs-ca").join("progress.db"))
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
impl Db {
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(attempts: u32, correct: u32) -> QuestionStats {
        QuestionStats { question_id: "test".into(), attempts, correct }
    }

    #[test]
    fn weight_unseen_question() {
        assert_eq!(stats(0, 0).weight(), 3);
    }

    #[test]
    fn weight_perfect_score() {
        assert_eq!(stats(10, 10).weight(), 1);
    }

    #[test]
    fn weight_at_90_percent_boundary() {
        assert_eq!(stats(10, 9).weight(), 1);
    }

    #[test]
    fn weight_just_below_90_percent() {
        // 8/10 = 80% → weight 2
        assert_eq!(stats(10, 8).weight(), 2);
    }

    #[test]
    fn weight_at_60_percent_boundary() {
        assert_eq!(stats(10, 6).weight(), 2);
    }

    #[test]
    fn weight_below_60_percent() {
        // 5/10 = 50% → weight 4
        assert_eq!(stats(10, 5).weight(), 4);
    }

    #[test]
    fn weight_zero_correct() {
        assert_eq!(stats(5, 0).weight(), 4);
    }
}
