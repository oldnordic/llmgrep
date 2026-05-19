//! Document search implementation.
//!
//! Queries the `source_documents` table in Magellan schema 13+ databases.
//! Supports filtering by tags, wikilinks, source kind, timestamp, and path.

use crate::error::LlmError;
use crate::output::DocsSearchResponse;
use rusqlite::Connection;
use std::path::Path;

/// Options for document search queries.
#[derive(Debug, Clone)]
pub struct DocsSearchOptions<'a> {
    pub db_path: &'a Path,
    pub limit: usize,
    pub tags: Option<&'a str>,
    pub wikilinks: Option<&'a str>,
    pub source_kind: Option<&'a str>,
    pub since: Option<i64>,
    pub path: Option<&'a str>,
}

pub(crate) fn search_docs_impl(
    conn: &Connection,
    options: &DocsSearchOptions,
) -> Result<DocsSearchResponse, LlmError> {
    let table_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='source_documents'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !table_exists {
        return Ok(DocsSearchResponse {
            results: Vec::new(),
            total_count: 0,
            path_filter: options.path.map(|p| p.to_string()),
        });
    }

    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(tags) = options.tags {
        let tag_list: Vec<&str> = tags
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        if !tag_list.is_empty() {
            let tag_conds: Vec<String> = tag_list
                .iter()
                .map(|_| "(tags LIKE ?)".to_string())
                .collect();
            conditions.push(format!("({})", tag_conds.join(" OR ")));
            for tag in &tag_list {
                params.push(Box::new(format!("%{}%", tag)));
            }
        }
    }

    if let Some(wikilinks) = options.wikilinks {
        conditions.push("wikilinks LIKE ?".to_string());
        params.push(Box::new(format!("%{}%", wikilinks)));
    }

    if let Some(source_kind) = options.source_kind {
        conditions.push("source_kind = ?".to_string());
        params.push(Box::new(source_kind.to_string()));
    }

    if let Some(since) = options.since {
        conditions.push("observed_at > ?".to_string());
        params.push(Box::new(since));
    }

    if let Some(path) = options.path {
        conditions.push("path_or_uri LIKE ?".to_string());
        params.push(Box::new(format!("%{}%", path)));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM source_documents {}", where_clause);
    let count_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let total_count: u64 = conn
        .query_row(&count_sql, count_refs.as_slice(), |row| row.get(0))
        .unwrap_or(0);

    let sql = format!(
        "SELECT id, path_or_uri, source_kind, content_hash, observed_at, \
         source_timestamp, title, author, tags, wikilinks \
         FROM source_documents {} \
         ORDER BY observed_at DESC \
         LIMIT ?",
        where_clause
    );

    let mut main_params = params;
    main_params.push(Box::new(options.limit as i64));
    let main_refs: Vec<&dyn rusqlite::types::ToSql> =
        main_params.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(main_refs.as_slice())?;

    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push(crate::output::DocsMatch {
            id: row.get(0)?,
            path_or_uri: row.get(1)?,
            source_kind: row.get(2)?,
            content_hash: row.get(3)?,
            observed_at: row.get(4)?,
            source_timestamp: row.get(5)?,
            title: row.get(6)?,
            author: row.get(7)?,
            tags: row.get(8)?,
            wikilinks: row.get(9)?,
        });
    }

    Ok(DocsSearchResponse {
        results,
        total_count,
        path_filter: options.path.map(|p| p.to_string()),
    })
}
