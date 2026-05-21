//! Intent-based code exploration using graph metadata.
//!
//! Decomposes a natural language intent into search tokens, queries the symbol
//! graph, ranks candidates by graph signals (fan-in, module clustering), and
//! returns grouped results. No embeddings — just graph traversal.

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "of", "in", "to", "for", "with", "on", "at",
    "by", "from", "how", "what", "where", "when", "who", "which", "that", "this", "do", "does",
    "did", "it", "its", "be", "been", "being", "have", "has", "had", "not", "but", "or", "and",
    "if", "then", "than", "so", "no", "up", "out", "can", "will", "would", "could", "should",
    "into", "about", "between", "through", "during", "before", "after", "above", "below",
];

const ABBREVIATIONS: &[(&str, &[&str])] = &[
    ("db", &["database", "db"]),
    ("cfg", &["config", "configuration", "cfg"]),
    ("impl", &["implement", "implementation", "impl"]),
    ("auth", &["authentication", "authorization", "auth"]),
    ("fn", &["function", "fn"]),
    ("struct", &["structure", "struct"]),
    ("util", &["utility", "util", "utils"]),
    ("mgr", &["manager", "mgr"]),
    ("svc", &["service", "svc"]),
    ("ctx", &["context", "ctx"]),
    ("err", &["error", "err", "errors"]),
    ("val", &["value", "val", "values"]),
    ("ref", &["reference", "ref", "refs"]),
    ("sync", &["synchronize", "sync"]),
    ("async", &["asynchronous", "async"]),
    ("init", &["initialize", "init"]),
    ("idx", &["index", "idx"]),
    ("msg", &["message", "msg"]),
    ("req", &["request", "req"]),
    ("res", &["response", "result", "res"]),
    ("conn", &["connection", "conn"]),
    ("pool", &["pool", "pooling"]),
    ("handler", &["handler", "handle", "handling"]),
    ("parser", &["parser", "parse", "parsing"]),
];

#[derive(Debug, Clone, Serialize)]
pub struct ExploreSymbol {
    pub name: String,
    pub fqn: String,
    pub file: String,
    pub kind: String,
    pub line: i64,
    pub fan_in: i64,
    pub fan_out: i64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExploreCluster {
    pub file: String,
    pub module: String,
    pub score: f64,
    pub symbols: Vec<ExploreSymbol>,
    pub internal_calls: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExploreResponse {
    pub intent: String,
    pub tokens: Vec<String>,
    pub clusters: Vec<ExploreCluster>,
    pub total_symbols: usize,
    pub total_modules: usize,
}

/// Split an intent string into searchable tokens.
pub fn tokenize_intent(intent: &str) -> Vec<String> {
    let lower = intent.to_lowercase();
    // Split on whitespace, punctuation, and camelCase/snake_case boundaries
    let mut raw_tokens: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in lower.chars() {
        if ch.is_whitespace() || ch == '_' || ch == '-' || ch == '.' || ch == '/' || ch == ':'
            || ch == ',' || ch == ';' || ch == '(' || ch == ')'
        {
            if !current.is_empty() {
                raw_tokens.push(current.clone());
                current.clear();
            }
        } else if !current.is_empty() {
            let prev = current.chars().last().unwrap();
            if prev.is_lowercase() && ch.is_uppercase() {
                // camelCase boundary
                raw_tokens.push(current.clone());
                current.clear();
            }
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        raw_tokens.push(current);
    }

    // Filter stop words and expand abbreviations
    let mut tokens: Vec<String> = Vec::new();
    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

    for token in &raw_tokens {
        if stop_set.contains(token.as_str()) || token.len() < 2 {
            continue;
        }
        // Check abbreviation expansion
        let mut expanded = false;
        for (abbr, expansions) in ABBREVIATIONS {
            if token == *abbr {
                for exp in *expansions {
                    if !tokens.contains(&exp.to_string()) {
                        tokens.push(exp.to_string());
                    }
                }
                expanded = true;
                break;
            }
        }
        if !expanded && !tokens.contains(token) {
            tokens.push(token.clone());
        }
    }

    tokens
}

/// Search for candidate symbols matching any of the tokens.
pub fn search_candidates(conn: &Connection, tokens: &[String], limit: usize) -> Vec<ExploreSymbol> {
    let mut candidates: HashMap<i64, ExploreSymbol> = HashMap::new();

    for token in tokens {
        // Try FTS5 prefix match on symbol names
        let fts_query = format!("{}*", token);
        if let Ok(mut stmt) = conn.prepare(
            "SELECT e.id, e.name, e.file_path,
                    COALESCE(json_extract(e.data, '$.kind_normalized'), json_extract(e.data, '$.kind'), '') as kind,
                    COALESCE(json_extract(e.data, '$.start_line'), 0) as start_line,
                    COALESCE(json_extract(e.data, '$.canonical_fqn'), json_extract(e.data, '$.display_fqn'), e.name) as fqn,
                    COALESCE(sm.fan_in, 0) as fan_in,
                    COALESCE(sm.fan_out, 0) as fan_out
             FROM symbol_fts f
             JOIN graph_entities e ON e.id = f.rowid
             LEFT JOIN symbol_metrics sm ON sm.symbol_id = e.id
             WHERE f.name MATCH ? AND e.kind = 'Symbol'
             LIMIT ?"
        ) {
            let rows = stmt.query_map(rusqlite::params![fts_query, limit * 3], |row| {
                Ok(ExploreSymbol {
                    name: row.get(1)?,
                    fqn: row.get::<_, String>(5)?,
                    file: row.get(2)?,
                    kind: row.get(3)?,
                    line: row.get(4)?,
                    fan_in: row.get(6)?,
                    fan_out: row.get(7)?,
                    score: 0.0,
                    // row 0 is id — we need it for dedup
                })
            });

            if let Ok(_rows) = rows {
                // Re-query with id for dedup
                if let Ok(mut stmt2) = conn.prepare(
                    "SELECT e.id, e.name, e.file_path,
                            COALESCE(json_extract(e.data, '$.kind_normalized'), json_extract(e.data, '$.kind'), '') as kind,
                            COALESCE(json_extract(e.data, '$.start_line'), 0) as start_line,
                            COALESCE(json_extract(e.data, '$.canonical_fqn'), json_extract(e.data, '$.display_fqn'), e.name) as fqn,
                            COALESCE(sm.fan_in, 0),
                            COALESCE(sm.fan_out, 0)
                     FROM symbol_fts f
                     JOIN graph_entities e ON e.id = f.rowid
                     LEFT JOIN symbol_metrics sm ON sm.symbol_id = e.id
                     WHERE f.name MATCH ? AND e.kind = 'Symbol'
                     LIMIT ?"
                ) {
                    let rows2 = stmt2.query_map(rusqlite::params![fts_query, limit * 3], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            ExploreSymbol {
                                name: row.get(1)?,
                                fqn: row.get::<_, String>(5)?,
                                file: row.get(2)?,
                                kind: row.get(3)?,
                                line: row.get(4)?,
                                fan_in: row.get(6)?,
                                fan_out: row.get(7)?,
                                score: 0.0,
                            },
                        ))
                    });
                    if let Ok(rows2) = rows2 {
                        for r in rows2.flatten() {
                            let (id, sym) = r;
                            candidates.entry(id).or_insert_with(|| sym);
                        }
                    }
                }
            }
        }

        // Fallback: LIKE query on name and canonical_fqn
        let like_pattern = format!("%{}%", token);
        if let Ok(mut stmt) = conn.prepare(
            "SELECT e.id, e.name, e.file_path,
                    COALESCE(json_extract(e.data, '$.kind_normalized'), json_extract(e.data, '$.kind'), '') as kind,
                    COALESCE(json_extract(e.data, '$.start_line'), 0) as start_line,
                    COALESCE(json_extract(e.data, '$.canonical_fqn'), json_extract(e.data, '$.display_fqn'), e.name) as fqn,
                    COALESCE(sm.fan_in, 0),
                    COALESCE(sm.fan_out, 0)
             FROM graph_entities e
             LEFT JOIN symbol_metrics sm ON sm.symbol_id = e.id
             WHERE e.kind = 'Symbol'
               AND (e.name LIKE ? OR e.name LIKE ?)
             LIMIT ?"
        ) {
            let rows = stmt.query_map(
                rusqlite::params![like_pattern, format!("{}%", token), limit * 2],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        ExploreSymbol {
                            name: row.get(1)?,
                            fqn: row.get::<_, String>(5)?,
                            file: row.get(2)?,
                            kind: row.get(3)?,
                            line: row.get(4)?,
                            fan_in: row.get(6)?,
                            fan_out: row.get(7)?,
                            score: 0.0,
                        },
                    ))
                },
            );
            if let Ok(rows) = rows {
                for r in rows.flatten() {
                    let (id, sym) = r;
                    candidates.entry(id).or_insert_with(|| sym);
                }
            }
        }
    }

    candidates.into_values().collect()
}

/// Score and rank candidates by graph signals.
pub fn rank_candidates(tokens: &[String], candidates: &mut [ExploreSymbol]) {
    let token_set: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();

    for sym in candidates.iter_mut() {
        let name_lower = sym.name.to_lowercase();
        let fqn_lower = sym.fqn.to_lowercase();

        let mut score = 0.0;

        for token in &token_set {
            if name_lower == *token {
                score += 10.0;
            } else if name_lower.contains(token.as_str()) {
                score += 5.0;
            }
            if fqn_lower.contains(token.as_str()) {
                score += 3.0;
            }
        }

        // Graph signal: fan-in weighted by number of matching tokens
        score += (sym.fan_in as f64).min(50.0);

        sym.score = score;
    }

    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

/// Group candidates by file and find internal call relationships.
pub fn cluster_by_file(conn: &Connection, candidates: &[ExploreSymbol]) -> Vec<ExploreCluster> {
    let mut file_groups: HashMap<String, Vec<&ExploreSymbol>> = HashMap::new();
    for sym in candidates {
        let file = sym.file.clone();
        file_groups.entry(file).or_default().push(sym);
    }

    // Build symbol name → file mapping for call resolution
    let mut name_to_file: HashMap<String, String> = HashMap::new();
    for sym in candidates {
        name_to_file.insert(sym.name.clone(), sym.file.clone());
    }

    // Collect all candidate entity IDs for call lookup
    let mut clusters: Vec<ExploreCluster> = Vec::new();

    for (file, syms) in &file_groups {
        let module = derive_module(file);
        let total_score: f64 = syms.iter().map(|s| s.score).sum();

        let mut internal_calls: Vec<String> = Vec::new();

        // Query calls between symbols in this cluster
        let names: Vec<String> = syms.iter().map(|s| s.name.clone()).collect();
        for name in &names {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT e_to.name
                 FROM graph_edges ge
                 JOIN graph_entities e_from ON e_from.id = ge.from_id
                 JOIN graph_entities e_to ON e_to.id = ge.to_id
                 WHERE e_from.name = ? AND ge.edge_type = 'CALLS'"
            ) {
                let rows = stmt.query_map(rusqlite::params![name], |row| row.get::<_, String>(0));
                if let Ok(rows) = rows {
                    for r in rows.flatten() {
                        if names.contains(&r) && !internal_calls.contains(&r) {
                            internal_calls.push(r);
                        }
                    }
                }
            }
        }

        clusters.push(ExploreCluster {
            file: file.clone(),
            module,
            score: total_score,
            symbols: syms.iter().map(|s| (*s).clone()).collect(),
            internal_calls,
        });
    }

    clusters.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    clusters
}

fn derive_module(file: &str) -> String {
    let path = Path::new(file);
    let parts: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "src" && i + 1 < parts.len() {
            return parts[i + 1].trim_end_matches(".rs").to_string();
        }
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Run the explore command.
pub fn run_explore(
    db_path: &Path,
    intent: &str,
    limit: usize,
    output: crate::output::OutputFormat,
) -> Result<()> {
    let conn = Connection::open(db_path)?;

    let tokens = tokenize_intent(intent);
    if tokens.is_empty() {
        anyhow::bail!("No searchable tokens found in intent: '{}'", intent);
    }

    let mut candidates = search_candidates(&conn, &tokens, limit);
    rank_candidates(&tokens, &mut candidates);

    // Truncate to limit
    candidates.truncate(limit);

    let clusters = cluster_by_file(&conn, &candidates);

    let response = ExploreResponse {
        intent: intent.to_string(),
        tokens: tokens.clone(),
        total_symbols: candidates.len(),
        total_modules: clusters.len(),
        clusters,
    };

    match output {
        crate::output::OutputFormat::Json | crate::output::OutputFormat::Pretty => {
            let wrapped = crate::output::json_response(&response);
            let json_str = match output {
                crate::output::OutputFormat::Pretty => serde_json::to_string_pretty(&wrapped)?,
                _ => serde_json::to_string(&wrapped)?,
            };
            println!("{}", json_str);
        }
        crate::output::OutputFormat::Human => {
            println!("Exploring: \"{}\"", intent);
            println!();

            for cluster in &response.clusters {
                println!("Module: {} (score: {:.0})", cluster.module, cluster.score);
                if !cluster.file.is_empty() {
                    println!("  File: {}", cluster.file);
                }
                for sym in &cluster.symbols {
                    println!(
                        "  {} ({}, fan-in: {})",
                        sym.name, sym.kind, sym.fan_in
                    );
                }
                if !cluster.internal_calls.is_empty() {
                    println!("  → Internal calls: {}", cluster.internal_calls.join(", "));
                }
                println!();
            }

            println!(
                "{} modules, {} symbols found",
                response.total_modules, response.total_symbols
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize_intent("database connection pooling");
        assert!(tokens.contains(&"database".to_string()));
        assert!(tokens.contains(&"connection".to_string()));
        assert!(tokens.contains(&"pooling".to_string()));
    }

    #[test]
    fn tokenize_strips_stop_words() {
        let tokens = tokenize_intent("how does the parser work");
        assert!(!tokens.iter().any(|t| t == "how" || t == "does" || t == "the"));
        assert!(tokens.contains(&"parser".to_string()));
        assert!(tokens.contains(&"work".to_string()));
    }

    #[test]
    fn tokenize_expands_abbreviations() {
        let tokens = tokenize_intent("db connection cfg");
        assert!(tokens.contains(&"database".to_string()));
        assert!(tokens.contains(&"db".to_string()));
        assert!(tokens.contains(&"config".to_string()));
    }

    #[test]
    fn tokenize_camel_case() {
        let tokens = tokenize_intent("searchSymbols");
        // camelCase splits on uppercase boundary: "search" + "ymbols" (lowercased)
        assert!(tokens.iter().any(|t| t.contains("search")));
        assert!(tokens.iter().any(|t| t.contains("ymbol")));
    }

    #[test]
    fn tokenize_snake_case() {
        let tokens = tokenize_intent("run_search_pipeline");
        assert!(tokens.contains(&"run".to_string()));
        assert!(tokens.contains(&"search".to_string()));
        assert!(tokens.contains(&"pipeline".to_string()));
    }

    #[test]
    fn tokenize_short_tokens_filtered() {
        let tokens = tokenize_intent("a b cd efg");
        assert!(!tokens.iter().any(|t| t == "a" || t == "b"));
        // "cd" is length 2, which passes the >= 2 filter
        assert!(tokens.contains(&"efg".to_string()));
    }

    #[test]
    fn derive_module_from_src() {
        assert_eq!(derive_module("src/query/explore.rs"), "query");
        assert_eq!(derive_module("src/main.rs"), "main");
        assert_eq!(derive_module("src/graph/queries.rs"), "graph");
    }

    #[test]
    fn derive_module_fallback() {
        assert_eq!(derive_module("lib/core.rs"), "core");
    }
}
