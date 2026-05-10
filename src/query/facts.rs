//! Fact search implementation.
//!
//! Queries the `candidate_facts` table in Magellan schema 14+ databases.
//! Supports filtering by subject, predicate, object, status, and subject type.

use crate::error::LlmError;
use crate::output::FactsSearchResponse;
use rusqlite::Connection;
use std::path::Path;

/// Options for fact search queries.
#[derive(Debug, Clone)]
pub struct FactsSearchOptions<'a> {
    pub db_path: &'a Path,
    pub limit: usize,
    pub subject: Option<&'a str>,
    pub predicate: Option<&'a str>,
    pub object: Option<&'a str>,
    pub status: Option<&'a str>,
    pub subject_type: Option<&'a str>,
}

pub(crate) fn search_facts_impl(
    conn: &Connection,
    options: &FactsSearchOptions,
) -> Result<FactsSearchResponse, LlmError> {
    let table_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='candidate_facts'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !table_exists {
        return Ok(FactsSearchResponse {
            results: Vec::new(),
            total_count: 0,
        });
    }

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(subject) = options.subject {
        conditions.push("subject_key LIKE ?".to_string());
        params.push(Box::new(format!("%{}%", subject)));
    }

    if let Some(predicate) = options.predicate {
        conditions.push("predicate = ?".to_string());
        params.push(Box::new(predicate.to_string()));
    }

    if let Some(object) = options.object {
        conditions.push("object_key LIKE ?".to_string());
        params.push(Box::new(format!("%{}%", object)));
    }

    if let Some(status) = options.status {
        conditions.push("status = ?".to_string());
        params.push(Box::new(status.to_string()));
    }

    if let Some(subject_type) = options.subject_type {
        conditions.push("subject_type = ?".to_string());
        params.push(Box::new(subject_type.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM candidate_facts {}", where_clause);
    let count_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let total_count: u64 = conn
        .query_row(&count_sql, count_refs.as_slice(), |row| row.get(0))
        .unwrap_or(0);

    let sql = format!(
        "SELECT id, candidate_id, source_document_id, subject_type, subject_key, \
         predicate, object_type, object_key, properties_json, status, \
         rejection_reason, created_at, reviewed_at \
         FROM candidate_facts {} \
         ORDER BY created_at DESC \
         LIMIT ?",
        where_clause
    );

    let mut main_params = params;
    main_params.push(Box::new(options.limit as i64));
    let main_refs: Vec<&dyn rusqlite::types::ToSql> = main_params.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(main_refs.as_slice())?;

    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(crate::output::FactMatch {
            id: row.get(0)?,
            candidate_id: row.get(1)?,
            source_document_id: row.get(2)?,
            subject_type: row.get(3)?,
            subject_key: row.get(4)?,
            predicate: row.get(5)?,
            object_type: row.get(6)?,
            object_key: row.get(7)?,
            properties_json: row.get(8)?,
            status: row.get(9)?,
            rejection_reason: row.get(10)?,
            created_at: row.get(11)?,
            reviewed_at: row.get(12)?,
        });
    }

    Ok(FactsSearchResponse {
        results,
        total_count,
    })
}
