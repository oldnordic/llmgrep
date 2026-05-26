//! Opt-in telemetry for llmgrep usage tracking.
//!
//! Records command invocations to a local SQLite database at
//! `~/.magellan/llmgrep-telemetry.db`. Only writes when explicitly
//! enabled via `LLMGREP_TELEMETRY=1` environment variable or `--record`
//! CLI flag. No data is sent anywhere — this is purely local.

use rusqlite::Connection;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug)]
pub struct TelemetryGuard {
    db_path: PathBuf,
    command: String,
    start: Instant,
    enabled: bool,
}

impl TelemetryGuard {
    pub fn new(command: &str) -> Self {
        let enabled = std::env::var("LLMGREP_TELEMETRY")
            .map(|v| v == "1")
            .unwrap_or(false);

        let db_path = dirs_magellan().join("llmgrep-telemetry.db");

        Self {
            db_path,
            command: command.to_string(),
            start: Instant::now(),
            enabled,
        }
    }

    pub fn with_record(mut self) -> Self {
        self.enabled = true;
        self
    }

    pub fn record(&self, status: &str, result_count: usize) {
        if !self.enabled {
            return;
        }

        if let Err(e) = self.record_inner(status, result_count) {
            eprintln!("Note: telemetry write failed: {}", e);
        }
    }

    fn record_inner(
        &self,
        status: &str,
        result_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&self.db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS telemetry (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                command TEXT NOT NULL,
                status TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                result_count INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_telemetry_command ON telemetry(command);
            CREATE INDEX IF NOT EXISTS idx_telemetry_timestamp ON telemetry(timestamp);",
        )?;

        let duration_ms = self.start.elapsed().as_millis() as i64;
        let timestamp = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO telemetry (timestamp, command, status, duration_ms, result_count) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                timestamp,
                self.command,
                status,
                duration_ms,
                result_count as i64
            ],
        )?;

        Ok(())
    }
}

fn dirs_magellan() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".magellan")
    } else {
        PathBuf::from("/tmp/.magellan")
    }
}

pub fn get_telemetry_summary(conn: &Connection) -> Result<TelemetrySummary, rusqlite::Error> {
    let total_invocations: i64 = conn
        .query_row("SELECT COUNT(*) FROM telemetry", [], |row| row.get(0))
        .unwrap_or(0);

    let by_command: Vec<(String, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT command, COUNT(*) as cnt FROM telemetry GROUP BY command ORDER BY cnt DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut v = Vec::new();
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?));
        }
        v
    };

    let avg_duration_ms: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(duration_ms), 0.0) FROM telemetry",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    Ok(TelemetrySummary {
        total_invocations,
        by_command,
        avg_duration_ms,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TelemetrySummary {
    pub total_invocations: i64,
    pub by_command: Vec<(String, i64)>,
    pub avg_duration_ms: f64,
}
