//! Evolve command: score symbols by fan_in × complexity and write candidate_facts.
//!
//! The evolve command identifies high-impact symbols by computing a composite
//! score from metrics already in the Magellan database. Symbols above a
//! threshold are written as candidate facts for downstream review.

use rusqlite::Connection;

use crate::error::LlmError;

#[derive(Debug, Clone)]
pub struct EvolveOptions<'a> {
    pub query: &'a str,
    pub min_score: i64,
    pub dry_run: bool,
    pub limit: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvolveCandidate {
    pub symbol_id: i64,
    pub symbol_name: String,
    pub kind: String,
    pub file_path: String,
    pub fan_in: i64,
    pub cyclomatic_complexity: i64,
    pub score: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvolveResponse {
    pub candidates: Vec<EvolveCandidate>,
    pub total_count: usize,
    pub written: usize,
    pub dry_run: bool,
}

pub fn run_evolve(conn: &Connection, options: &EvolveOptions) -> Result<EvolveResponse, LlmError> {
    let metrics_exist: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='symbol_metrics'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !metrics_exist {
        return Err(LlmError::InvalidQuery {
            query: "symbol_metrics table not found. Re-index with 'magellan watch --scan-initial' to populate metrics.".to_string(),
        });
    }

    let like_pattern = if options.query == ".*" || options.query.is_empty() {
        "%".to_string()
    } else {
        format!("%{}%", options.query)
    };

    let sql = r#"
        SELECT sm.symbol_id, sm.symbol_name, sm.kind, sm.file_path,
               sm.fan_in, sm.cyclomatic_complexity,
               (sm.fan_in * sm.cyclomatic_complexity) AS score
        FROM symbol_metrics sm
        WHERE sm.symbol_name LIKE ?1
          AND (sm.fan_in * sm.cyclomatic_complexity) >= ?2
        ORDER BY score DESC
        LIMIT ?3
    "#;

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(rusqlite::params![
        like_pattern,
        options.min_score,
        options.limit as i64
    ])?;

    let mut candidates = Vec::new();
    while let Some(row) = rows.next()? {
        candidates.push(EvolveCandidate {
            symbol_id: row.get(0)?,
            symbol_name: row.get(1)?,
            kind: row.get(2)?,
            file_path: row.get(3)?,
            fan_in: row.get(4)?,
            cyclomatic_complexity: row.get(5)?,
            score: row.get(6)?,
        });
    }

    let total_count = candidates.len();
    let written = if options.dry_run {
        0
    } else {
        write_candidates(conn, &candidates)?
    };

    Ok(EvolveResponse {
        candidates,
        total_count,
        written,
        dry_run: options.dry_run,
    })
}

fn write_candidates(conn: &Connection, candidates: &[EvolveCandidate]) -> Result<usize, LlmError> {
    let facts_exist: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='candidate_facts'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !facts_exist {
        return Err(LlmError::InvalidQuery {
            query: "candidate_facts table not found. Re-index with a Magellan v14+ database."
                .to_string(),
        });
    }

    let mut written = 0;
    for c in candidates {
        let subject_key = format!("symbol:{}", c.symbol_name);
        let properties = serde_json::json!({
            "score": c.score,
            "fan_in": c.fan_in,
            "cyclomatic_complexity": c.cyclomatic_complexity,
            "file_path": c.file_path,
            "kind": c.kind,
        });

        conn.execute(
            "INSERT OR IGNORE INTO candidate_facts \
             (candidate_id, source_document_id, subject_type, subject_key, \
              predicate, object_type, object_key, properties_json, status, \
              rejection_reason, created_at, reviewed_at) \
             VALUES (?1, 0, 'symbol', ?2, 'evolve_candidate', 'metric', 'score', \
              ?3, 'pending', NULL, strftime('%s','now'), NULL)",
            rusqlite::params![
                format!("evolve_{}", c.symbol_id),
                subject_key,
                properties.to_string(),
            ],
        )?;
        written += 1;
    }

    Ok(written)
}
