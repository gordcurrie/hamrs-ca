use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
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
        self.conn.execute_batch(
            "
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

            CREATE TABLE IF NOT EXISTS concept_progress (
                key        TEXT PRIMARY KEY,
                visited_at INTEGER NOT NULL
            );
        ",
        )?;
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

    pub fn mark_concept_visited(&self, key: &str) -> Result<()> {
        let now = unix_now();
        self.conn.execute(
            "INSERT OR REPLACE INTO concept_progress (key, visited_at) VALUES (?1, ?2)",
            params![key, now],
        )?;
        Ok(())
    }

    pub fn get_visited_concepts(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT key FROM concept_progress")?;
        let keys = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        Ok(keys)
    }

    pub fn reset_concept_topic(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM concept_progress WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn reset_concept_section(&self, section: u8) -> Result<()> {
        let prefix = format!("B-{section:03}-");
        self.conn.execute(
            "DELETE FROM concept_progress WHERE key LIKE ?1",
            params![format!("{prefix}%")],
        )?;
        Ok(())
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
    // Explicit XDG_DATA_HOME always wins — no migration fallback.
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(xdg).join("hamrs-ca").join("progress.db"));
    }

    let default_base = dirs::home_dir()
        .map(|h| h.join(".local").join("share"))
        .unwrap_or_else(|| PathBuf::from("."));
    let new_path = default_base.join("hamrs-ca").join("progress.db");

    // Migration: if new path doesn't exist but old platform-native path does,
    // keep using old location so existing progress isn't silently lost.
    if !new_path.exists() {
        if let Some(old_path) =
            dirs::data_local_dir().map(|d| d.join("hamrs-ca").join("progress.db"))
        {
            if old_path != new_path && old_path.exists() {
                return Ok(old_path);
            }
        }
    }

    Ok(new_path)
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
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        prev: Option<std::ffi::OsString>,
    }
    impl EnvGuard {
        fn set(key: &'static str, val: impl AsRef<std::ffi::OsStr>) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, val);
            Self { key, prev }
        }
        fn remove(key: &'static str) -> Self {
            let prev = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, prev }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn db_path_uses_xdg_data_home_override() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let _env = EnvGuard::set("XDG_DATA_HOME", tmp.path());
        let path = db_path().unwrap();
        assert_eq!(path, tmp.path().join("hamrs-ca").join("progress.db"));
    }

    #[test]
    fn db_path_empty_xdg_data_home_falls_back_to_home() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set("XDG_DATA_HOME", "");
        let path = db_path().unwrap();
        assert!(path.ends_with("hamrs-ca/progress.db"));
        if let Some(home) = dirs::home_dir() {
            assert!(path.is_absolute());
            assert!(path.starts_with(home.join(".local").join("share")) || path.starts_with(&home));
        }
    }

    #[test]
    fn db_path_unset_uses_home_local_share() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::remove("XDG_DATA_HOME");
        let path = db_path().unwrap();
        assert!(path.ends_with("hamrs-ca/progress.db"));
        if dirs::home_dir().is_some() {
            assert!(path.is_absolute());
        }
    }

    fn stats(attempts: u32, correct: u32) -> QuestionStats {
        QuestionStats {
            question_id: "test".into(),
            attempts,
            correct,
        }
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

    #[test]
    fn concept_mark_and_retrieve() {
        let db = Db::open_in_memory().unwrap();
        db.mark_concept_visited("B-001-003").unwrap();
        db.mark_concept_visited("B-001-007").unwrap();
        let visited = db.get_visited_concepts().unwrap();
        assert!(visited.contains("B-001-003"));
        assert!(visited.contains("B-001-007"));
        assert!(!visited.contains("B-002-001"));
    }

    #[test]
    fn concept_mark_idempotent() {
        let db = Db::open_in_memory().unwrap();
        db.mark_concept_visited("B-001-003").unwrap();
        db.mark_concept_visited("B-001-003").unwrap();
        let visited = db.get_visited_concepts().unwrap();
        assert_eq!(visited.len(), 1);
    }

    #[test]
    fn concept_reset_topic() {
        let db = Db::open_in_memory().unwrap();
        db.mark_concept_visited("B-001-003").unwrap();
        db.mark_concept_visited("B-001-007").unwrap();
        db.reset_concept_topic("B-001-003").unwrap();
        let visited = db.get_visited_concepts().unwrap();
        assert!(!visited.contains("B-001-003"));
        assert!(visited.contains("B-001-007"));
    }

    #[test]
    fn concept_reset_section() {
        let db = Db::open_in_memory().unwrap();
        db.mark_concept_visited("B-001-003").unwrap();
        db.mark_concept_visited("B-001-007").unwrap();
        db.mark_concept_visited("B-002-001").unwrap();
        db.reset_concept_section(1).unwrap();
        let visited = db.get_visited_concepts().unwrap();
        assert!(!visited.iter().any(|k| k.starts_with("B-001-")));
        assert!(visited.contains("B-002-001"));
    }
}
