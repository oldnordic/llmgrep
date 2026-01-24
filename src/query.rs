use crate::error::LlmError;
use crate::output::{
    CallMatch, CallSearchResponse, ReferenceMatch, ReferenceSearchResponse, SearchResponse,
    SpanContext, SymbolMatch,
};
use rusqlite::{params_from_iter, Connection, OpenFlags, ToSql};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MAX_REGEX_SIZE: usize = 10_000; // 10KB limit to prevent memory exhaustion

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SymbolNodeData {
    #[serde(default)]
    symbol_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    kind_normalized: Option<String>,
    #[serde(default)]
    fqn: Option<String>,
    #[serde(default)]
    canonical_fqn: Option<String>,
    #[serde(default)]
    display_fqn: Option<String>,
    byte_start: u64,
    byte_end: u64,
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReferenceNodeData {
    file: String,
    byte_start: u64,
    byte_end: u64,
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CallNodeData {
    file: String,
    caller: String,
    callee: String,
    #[serde(default)]
    caller_symbol_id: Option<String>,
    #[serde(default)]
    callee_symbol_id: Option<String>,
    byte_start: u64,
    byte_end: u64,
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
}

pub fn search_symbols(
    db_path: &Path,
    query: &str,
    path_filter: Option<&PathBuf>,
    kind_filter: Option<&str>,
    limit: usize,
    use_regex: bool,
    candidates: usize,
    with_context: bool,
    context_lines: usize,
    max_context_lines: usize,
    with_snippet: bool,
    with_fqn: bool,
    with_canonical_fqn: bool,
    with_display_fqn: bool,
    with_score: bool,
    max_snippet_bytes: usize,
) -> Result<(SearchResponse, bool), LlmError> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    let (sql, params) = build_search_query(
        query,
        path_filter,
        kind_filter,
        use_regex,
        false,
        candidates,
    );
    let mut stmt = conn.prepare_cached(&sql)?;

    let mut rows = stmt.query(params_from_iter(params))?;
    let mut results = Vec::new();
    let regex = if use_regex {
        Some(
            RegexBuilder::new(query)
                .size_limit(MAX_REGEX_SIZE)
                .build()
                .map_err(|e| LlmError::RegexRejected {
                    reason: format!("Regex too complex or invalid: {}", e),
                })?,
        )
    } else {
        None
    };
    let mut file_cache = HashMap::new();

    while let Some(row) = rows.next()? {
        let data: String = row.get(0)?;
        let file_path: String = row.get(1)?;
        let symbol: SymbolNodeData = serde_json::from_str(&data)?;
        let name = symbol.name.clone().unwrap_or_else(|| "<unknown>".to_string());
        let display_fqn = symbol.display_fqn.clone().unwrap_or_default();
        let fqn = symbol.fqn.clone().unwrap_or_default();

        if let Some(ref pattern) = regex {
            if !pattern.is_match(&name) && !pattern.is_match(&display_fqn) && !pattern.is_match(&fqn)
            {
                continue;
            }
        }

        let (snippet, snippet_truncated) = if with_snippet {
            snippet_from_file(
                &file_path,
                symbol.byte_start,
                symbol.byte_end,
                max_snippet_bytes,
                &mut file_cache,
            )
        } else {
            (None, None)
        };
        let context = if with_context {
            let capped = context_lines > max_context_lines;
            let effective_lines = context_lines.min(max_context_lines);
            span_context_from_file(
                &file_path,
                symbol.start_line,
                symbol.end_line,
                effective_lines,
                capped,
                &mut file_cache,
            )
        } else {
            None
        };

        let span = crate::output::Span {
            span_id: span_id(&file_path, symbol.byte_start, symbol.byte_end),
            file_path: file_path.clone(),
            byte_start: symbol.byte_start,
            byte_end: symbol.byte_end,
            start_line: symbol.start_line,
            start_col: symbol.start_col,
            end_line: symbol.end_line,
            end_col: symbol.end_col,
            context,
        };

        let match_id = match_id(&file_path, symbol.byte_start, symbol.byte_end, &name);
        let score = score_match(query, &name, &display_fqn, &fqn, regex.as_ref());
        let fqn = if with_fqn { symbol.fqn } else { None };
        let canonical_fqn = if with_canonical_fqn {
            symbol.canonical_fqn
        } else {
            None
        };
        let display_fqn = if with_display_fqn {
            symbol.display_fqn
        } else {
            None
        };
        results.push(SymbolMatch {
            match_id,
            span,
            name,
            kind: symbol.kind,
            parent: None,
            symbol_id: symbol.symbol_id,
            score: if with_score { Some(score) } else { None },
            fqn,
            canonical_fqn,
            display_fqn,
            snippet,
            snippet_truncated,
        });
    }

    let mut partial = false;
    let total_count = if use_regex {
        if results.len() >= candidates {
            partial = true;
        }
        results.len() as u64
    } else {
        let (count_sql, count_params) =
            build_search_query(query, path_filter, kind_filter, use_regex, true, 0);
        let count = conn.query_row(&count_sql, params_from_iter(count_params), |row| row.get(0))?;
        if candidates < count as usize {
            partial = true;
        }
        count
    };

    results.sort_by(|a, b| {
        b.score
            .unwrap_or(0)
            .cmp(&a.score.unwrap_or(0))
            .then_with(|| a.span.start_line.cmp(&b.span.start_line))
            .then_with(|| a.span.start_col.cmp(&b.span.start_col))
            .then_with(|| a.span.byte_start.cmp(&b.span.byte_start))
    });
    results.truncate(limit);

    Ok((
        SearchResponse {
            results,
            query: query.to_string(),
            path_filter: path_filter.map(|path| path.to_string_lossy().to_string()),
            kind_filter: kind_filter.map(|value| value.to_string()),
            total_count,
        },
        partial,
    ))
}

pub fn search_references(
    db_path: &Path,
    query: &str,
    path_filter: Option<&PathBuf>,
    limit: usize,
    use_regex: bool,
    candidates: usize,
    with_context: bool,
    context_lines: usize,
    max_context_lines: usize,
    with_snippet: bool,
    with_score: bool,
    max_snippet_bytes: usize,
) -> Result<(ReferenceSearchResponse, bool), LlmError> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (sql, params) =
        build_reference_query(query, path_filter, use_regex, false, candidates);
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    let regex = if use_regex {
        Some(Regex::new(query)?)
    } else {
        None
    };
    let mut file_cache = HashMap::new();
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        let data: String = row.get(0)?;
        let name: String = row.get(1)?;
        let target_symbol_id: Option<String> = row.get(2)?;
        let reference: ReferenceNodeData = serde_json::from_str(&data)?;
        let referenced_symbol = referenced_symbol_from_name(&name);

        if let Some(ref pattern) = regex {
            if !pattern.is_match(&referenced_symbol) {
                continue;
            }
        } else if !referenced_symbol.contains(query) {
            continue;
        }

        let score = score_match(query, &referenced_symbol, "", "", regex.as_ref());
        let context = if with_context {
            let capped = context_lines > max_context_lines;
            let effective_lines = context_lines.min(max_context_lines);
            span_context_from_file(
                &reference.file,
                reference.start_line,
                reference.end_line,
                effective_lines,
                capped,
                &mut file_cache,
            )
        } else {
            None
        };
        let (snippet, snippet_truncated) = if with_snippet {
            snippet_from_file(
                &reference.file,
                reference.byte_start,
                reference.byte_end,
                max_snippet_bytes,
                &mut file_cache,
            )
        } else {
            (None, None)
        };

        let span = crate::output::Span {
            span_id: span_id(&reference.file, reference.byte_start, reference.byte_end),
            file_path: reference.file.clone(),
            byte_start: reference.byte_start,
            byte_end: reference.byte_end,
            start_line: reference.start_line,
            start_col: reference.start_col,
            end_line: reference.end_line,
            end_col: reference.end_col,
            context,
        };
        let match_id = match_id(
            &reference.file,
            reference.byte_start,
            reference.byte_end,
            &referenced_symbol,
        );
        results.push(ReferenceMatch {
            match_id,
            span,
            referenced_symbol,
            reference_kind: None,
            target_symbol_id,
            score: if with_score { Some(score) } else { None },
            snippet,
            snippet_truncated,
        });
    }

    let mut partial = false;
    let total_count = if use_regex {
        if results.len() >= candidates {
            partial = true;
        }
        results.len() as u64
    } else {
        let (count_sql, count_params) =
            build_reference_query(query, path_filter, use_regex, true, 0);
        let count = conn.query_row(&count_sql, params_from_iter(count_params), |row| row.get(0))?;
        if candidates < count as usize {
            partial = true;
        }
        count
    };

    results.sort_by(|a, b| {
        b.score
            .unwrap_or(0)
            .cmp(&a.score.unwrap_or(0))
            .then_with(|| a.span.start_line.cmp(&b.span.start_line))
            .then_with(|| a.span.start_col.cmp(&b.span.start_col))
            .then_with(|| a.span.byte_start.cmp(&b.span.byte_start))
    });
    results.truncate(limit);

    Ok((
        ReferenceSearchResponse {
            results,
            query: query.to_string(),
            path_filter: path_filter.map(|path| path.to_string_lossy().to_string()),
            total_count,
        },
        partial,
    ))
}

pub fn search_calls(
    db_path: &Path,
    query: &str,
    path_filter: Option<&PathBuf>,
    limit: usize,
    use_regex: bool,
    candidates: usize,
    with_context: bool,
    context_lines: usize,
    max_context_lines: usize,
    with_snippet: bool,
    with_score: bool,
    max_snippet_bytes: usize,
) -> Result<(CallSearchResponse, bool), LlmError> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (sql, params) = build_call_query(query, path_filter, use_regex, false, candidates);
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    let regex = if use_regex {
        Some(Regex::new(query)?)
    } else {
        None
    };
    let mut file_cache = HashMap::new();
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        let data: String = row.get(0)?;
        let call: CallNodeData = serde_json::from_str(&data)?;

        if let Some(ref pattern) = regex {
            if !pattern.is_match(&call.caller) && !pattern.is_match(&call.callee) {
                continue;
            }
        } else if !call.caller.contains(query) && !call.callee.contains(query) {
            continue;
        }

        let caller_score = score_match(query, &call.caller, "", "", regex.as_ref());
        let callee_score = score_match(query, &call.callee, "", "", regex.as_ref());
        let score = caller_score.max(callee_score);

        let context = if with_context {
            let capped = context_lines > max_context_lines;
            let effective_lines = context_lines.min(max_context_lines);
            span_context_from_file(
                &call.file,
                call.start_line,
                call.end_line,
                effective_lines,
                capped,
                &mut file_cache,
            )
        } else {
            None
        };
        let (snippet, snippet_truncated) = if with_snippet {
            snippet_from_file(
                &call.file,
                call.byte_start,
                call.byte_end,
                max_snippet_bytes,
                &mut file_cache,
            )
        } else {
            (None, None)
        };

        let span = crate::output::Span {
            span_id: span_id(&call.file, call.byte_start, call.byte_end),
            file_path: call.file.clone(),
            byte_start: call.byte_start,
            byte_end: call.byte_end,
            start_line: call.start_line,
            start_col: call.start_col,
            end_line: call.end_line,
            end_col: call.end_col,
            context,
        };
        let name = format!("{}->{}", call.caller, call.callee);
        let match_id = match_id(&call.file, call.byte_start, call.byte_end, &name);
        results.push(CallMatch {
            match_id,
            span,
            caller: call.caller,
            callee: call.callee,
            caller_symbol_id: call.caller_symbol_id,
            callee_symbol_id: call.callee_symbol_id,
            score: if with_score { Some(score) } else { None },
            snippet,
            snippet_truncated,
        });
    }

    let mut partial = false;
    let total_count = if use_regex {
        if results.len() >= candidates {
            partial = true;
        }
        results.len() as u64
    } else {
        let (count_sql, count_params) =
            build_call_query(query, path_filter, use_regex, true, 0);
        let count = conn.query_row(&count_sql, params_from_iter(count_params), |row| row.get(0))?;
        if candidates < count as usize {
            partial = true;
        }
        count
    };

    results.sort_by(|a, b| {
        b.score
            .unwrap_or(0)
            .cmp(&a.score.unwrap_or(0))
            .then_with(|| a.span.start_line.cmp(&b.span.start_line))
            .then_with(|| a.span.start_col.cmp(&b.span.start_col))
            .then_with(|| a.span.byte_start.cmp(&b.span.byte_start))
    });
    results.truncate(limit);

    Ok((
        CallSearchResponse {
            results,
            query: query.to_string(),
            path_filter: path_filter.map(|path| path.to_string_lossy().to_string()),
            total_count,
        },
        partial,
    ))
}

fn build_search_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    kind_filter: Option<&str>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = Vec::new();

    if !use_regex {
        let like_query = like_pattern(query);
        where_clauses.push(
            "(s.name LIKE ? ESCAPE '\\' OR s.display_fqn LIKE ? ESCAPE '\\' OR s.fqn LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query));
    }

    if let Some(path) = path_filter {
        where_clauses.push("f.file_path LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_prefix(path)));
    }

    if let Some(kind) = kind_filter {
        where_clauses.push("(s.kind_normalized = ? OR s.kind = ?)".to_string());
        params.push(Box::new(kind.to_string()));
        params.push(Box::new(kind.to_string()));
    }

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        "SELECT s.data, f.file_path"
    };

    let mut sql = format!(
        "{select_clause}
FROM (
    SELECT id,
           data,
           json_extract(data, '$.name') AS name,
           json_extract(data, '$.display_fqn') AS display_fqn,
           json_extract(data, '$.fqn') AS fqn,
           json_extract(data, '$.kind') AS kind,
           json_extract(data, '$.kind_normalized') AS kind_normalized,
           json_extract(data, '$.start_line') AS start_line,
           json_extract(data, '$.start_col') AS start_col,
           json_extract(data, '$.byte_start') AS byte_start,
           json_extract(data, '$.byte_end') AS byte_end
    FROM graph_entities
    WHERE kind = 'Symbol'
) s
JOIN graph_edges e ON e.to_id = s.id AND e.edge_type = 'DEFINES'
JOIN (
    SELECT id, json_extract(data, '$.path') AS file_path
    FROM graph_entities
    WHERE kind = 'File'
) f ON f.id = e.from_id
WHERE {where_clause}",
        select_clause = select_clause,
        where_clause = if where_clauses.is_empty() {
            "1=1".to_string()
        } else {
            where_clauses.join(" AND ")
        },
    );

    if !count_only {
        sql.push_str(
            "\nORDER BY s.start_line, s.start_col, s.byte_start, s.byte_end, s.id\n",
        );
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params)
}

fn build_reference_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = vec!["r.kind = 'Reference'".to_string()];

    if !use_regex {
        let like_query = like_pattern(query);
        where_clauses.push("r.name LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_query));
    }

    if let Some(path) = path_filter {
        where_clauses.push("json_extract(r.data, '$.file') LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_prefix(path)));
    }

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        "SELECT r.data, r.name, json_extract(s.data, '$.symbol_id') AS target_symbol_id"
    };

    let mut sql = format!(
        "{select_clause}
FROM graph_entities r
LEFT JOIN graph_edges e ON e.from_id = r.id AND e.edge_type = 'REFERENCES'
LEFT JOIN graph_entities s ON s.id = e.to_id AND s.kind = 'Symbol'
WHERE {where_clause}",
        select_clause = select_clause,
        where_clause = where_clauses.join(" AND "),
    );

    if !count_only {
        sql.push_str(
            "\nORDER BY json_extract(r.data, '$.start_line'), json_extract(r.data, '$.start_col'), json_extract(r.data, '$.byte_start'), r.id\n",
        );
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params)
}

fn build_call_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = vec!["c.kind = 'Call'".to_string()];

    if !use_regex {
        let like_query = like_pattern(query);
        where_clauses.push(
            "(json_extract(c.data, '$.caller') LIKE ? ESCAPE '\\' OR json_extract(c.data, '$.callee') LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query));
    }

    if let Some(path) = path_filter {
        where_clauses.push("json_extract(c.data, '$.file') LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_prefix(path)));
    }

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        "SELECT c.data"
    };

    let mut sql = format!(
        "{select_clause}
FROM graph_entities c
WHERE {where_clause}",
        select_clause = select_clause,
        where_clause = where_clauses.join(" AND "),
    );

    if !count_only {
        sql.push_str(
            "\nORDER BY json_extract(c.data, '$.start_line'), json_extract(c.data, '$.start_col'), json_extract(c.data, '$.byte_start'), c.id\n",
        );
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params)
}

fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{}%", escaped)
}

fn like_prefix(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("{}%", escaped)
}

fn referenced_symbol_from_name(name: &str) -> String {
    name.strip_prefix("ref to ").unwrap_or(name).to_string()
}

struct FileCache {
    bytes: Vec<u8>,
    lines: Vec<String>,
}

fn load_file<'a>(
    path: &str,
    cache: &'a mut HashMap<String, FileCache>,
) -> Option<&'a FileCache> {
    if !cache.contains_key(path) {
        let bytes = std::fs::read(path).ok()?;
        let text = String::from_utf8_lossy(&bytes);
        let lines = text.split('\n').map(|line| line.to_string()).collect();
        cache.insert(
            path.to_string(),
            FileCache {
                bytes,
                lines,
            },
        );
    }
    cache.get(path)
}

fn snippet_from_file(
    file_path: &str,
    byte_start: u64,
    byte_end: u64,
    max_bytes: usize,
    cache: &mut HashMap<String, FileCache>,
) -> (Option<String>, Option<bool>) {
    if max_bytes == 0 {
        return (None, None);
    }
    let file = match load_file(file_path, cache) {
        Some(file) => file,
        None => return (None, None),
    };
    let start = byte_start as usize;
    let end = byte_end as usize;
    if start >= file.bytes.len() || end > file.bytes.len() || start >= end {
        return (None, None);
    }
    let capped_end = end.min(start + max_bytes);
    let truncated = capped_end < end;
    let snippet = String::from_utf8_lossy(&file.bytes[start..capped_end]).to_string();
    (Some(snippet), Some(truncated))
}

fn span_context_from_file(
    file_path: &str,
    start_line: u64,
    end_line: u64,
    context_lines: usize,
    capped: bool,
    cache: &mut HashMap<String, FileCache>,
) -> Option<SpanContext> {
    let file = load_file(file_path, cache)?;
    let line_count = file.lines.len() as u64;
    if line_count == 0 {
        return None;
    }
    let start_line = start_line.max(1).min(line_count);
    let end_line = end_line.max(start_line).min(line_count);
    let before_start = start_line.saturating_sub(context_lines as u64).max(1);
    let after_end = (end_line + context_lines as u64).min(line_count);

    let before = file.lines[(before_start - 1) as usize..(start_line - 1) as usize].to_vec();
    let selected = file.lines[(start_line - 1) as usize..end_line as usize].to_vec();
    let after = file.lines[end_line as usize..after_end as usize].to_vec();
    let truncated = capped
        || (context_lines > 0 && (before.len() < context_lines || after.len() < context_lines));

    Some(SpanContext {
        before,
        selected,
        after,
        truncated,
    })
}

fn score_match(
    query: &str,
    name: &str,
    display_fqn: &str,
    fqn: &str,
    regex: Option<&Regex>,
) -> u64 {
    let mut score = 0;

    if name == query {
        score = score.max(100);
    }
    if display_fqn == query {
        score = score.max(95);
    }
    if fqn == query {
        score = score.max(90);
    }

    if name.starts_with(query) {
        score = score.max(80);
    }
    if display_fqn.starts_with(query) {
        score = score.max(70);
    }
    if name.contains(query) {
        score = score.max(60);
    }
    if display_fqn.contains(query) {
        score = score.max(50);
    }
    if fqn.contains(query) {
        score = score.max(40);
    }

    if let Some(pattern) = regex {
        if pattern.is_match(name) {
            score = score.max(70);
        } else if pattern.is_match(display_fqn) {
            score = score.max(60);
        } else if pattern.is_match(fqn) {
            score = score.max(50);
        }
    }

    score
}

fn span_id(file_path: &str, byte_start: u64, byte_end: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    hasher.update(b":");
    hasher.update(byte_start.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(byte_end.to_string().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

fn match_id(file_path: &str, byte_start: u64, byte_end: u64, name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(b":");
    hasher.update(file_path.as_bytes());
    hasher.update(b":");
    hasher.update(byte_start.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(byte_end.to_string().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}
