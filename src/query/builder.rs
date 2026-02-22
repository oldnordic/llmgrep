//! SQL query builders for search operations.
//!
//! This module provides functions for building SQL queries with various
//! filtering options.

use crate::algorithm::{symbol_set_filter_strategy, SymbolSetStrategy};
use crate::query::options::MetricsOptions;
use crate::query::util::{like_pattern, like_prefix};
use crate::SortMode;
use rusqlite::ToSql;
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)] // All parameters are needed for flexible query building
pub(crate) fn build_search_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    kind_filter: Option<&str>,
    language_filter: Option<&str>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
    metrics: MetricsOptions,
    sort_by: SortMode,
    symbol_id: Option<&str>,
    fqn_pattern: Option<&str>,
    exact_fqn: Option<&str>,
    has_ast_table: bool,
    ast_kinds: &[String],
    _min_depth: Option<usize>,
    _max_depth: Option<usize>,
    inside_kind: Option<&str>,
    contains_kind: Option<&str>,
    symbol_set_filter: Option<&Vec<String>>,
) -> (String, Vec<Box<dyn ToSql>>, SymbolSetStrategy) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = Vec::new();

    // SymbolId mode: Direct lookup by BLAKE3 hash (bypasses name search)
    if let Some(sid) = symbol_id {
        where_clauses.push("json_extract(s.data, '$.symbol_id') = ?".to_string());
        params.push(Box::new(sid.to_string()));
    } else if !use_regex {
        // Standard name-based search (only if not using symbol_id)
        let like_query = like_pattern(query);
        where_clauses.push(
            "(s.name LIKE ? ESCAPE '\\' OR s.display_fqn LIKE ? ESCAPE '\\' OR s.fqn LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query));
    }

    // FQN pattern filter (LIKE match on canonical_fqn)
    if let Some(pattern) = fqn_pattern {
        where_clauses
            .push("json_extract(s.data, '$.canonical_fqn') LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(pattern.to_string()));
    }

    // Exact FQN filter (exact match on canonical_fqn)
    if let Some(exact) = exact_fqn {
        where_clauses.push("json_extract(s.data, '$.canonical_fqn') = ?".to_string());
        params.push(Box::new(exact.to_string()));
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

    // Language filter: Filter by inferred language from file extension
    // Note: This uses file extension matching since language labels aren't
    // directly stored in graph_entities. A future enhancement could use
    // label tables for faster filtering.
    if let Some(language) = language_filter {
        let extensions = match language {
            "rust" => ".rs",
            "python" => ".py",
            "javascript" => ".js",
            "typescript" => ".ts",
            "c" => ".c",
            "cpp" => ".cpp",
            "java" => ".java",
            "go" => ".go",
            _ => "", // Unknown language - no filter
        };
        if !extensions.is_empty() {
            where_clauses.push("f.file_path LIKE ? ESCAPE '\\'".to_string());
            params.push(Box::new(format!("%{}", extensions)));
        }
    }

    // AST kind filter: Filter by AST node kind(s) using overlap matching
    // This uses an EXISTS subquery to handle cases where AST nodes overlap
    // with symbol spans but don't have exact byte matches
    if !ast_kinds.is_empty() && has_ast_table {
        if ast_kinds.len() == 1 {
            // Single kind - use EXISTS with overlap check
            where_clauses.push(
                "EXISTS (
                    SELECT 1 FROM ast_nodes
                    WHERE kind = ?
                    AND byte_start < json_extract(s.data, '$.byte_end')
                    AND byte_end > json_extract(s.data, '$.byte_start')
                )"
                .to_string(),
            );
            params.push(Box::new(ast_kinds[0].clone()));
        } else {
            // Multiple kinds - use EXISTS with IN and overlap check
            let placeholders = vec!["?"; ast_kinds.len()].join(",");
            where_clauses.push(format!(
                "EXISTS (
                    SELECT 1 FROM ast_nodes
                    WHERE kind IN ({})
                    AND byte_start < json_extract(s.data, '$.byte_end')
                    AND byte_end > json_extract(s.data, '$.byte_start')
                )",
                placeholders
            ));
            for kind in ast_kinds {
                params.push(Box::new(kind.clone()));
            }
        }
    }
    // If ast_nodes table doesn't exist, we silently ignore the filter
    // (graceful degradation)

    // Add metrics filter WHERE clauses
    // For filters, we use IS NOT NULL to ensure symbols have metrics
    if let Some(min_cc) = metrics.min_complexity {
        where_clauses.push(
            "(sm.cyclomatic_complexity IS NOT NULL AND sm.cyclomatic_complexity >= ?)".to_string(),
        );
        params.push(Box::new(min_cc as i64));
    }
    if let Some(max_cc) = metrics.max_complexity {
        where_clauses.push(
            "(sm.cyclomatic_complexity IS NOT NULL AND sm.cyclomatic_complexity <= ?)".to_string(),
        );
        params.push(Box::new(max_cc as i64));
    }
    if let Some(min_fi) = metrics.min_fan_in {
        where_clauses.push("(sm.fan_in IS NOT NULL AND sm.fan_in >= ?)".to_string());
        params.push(Box::new(min_fi as i64));
    }
    if let Some(min_fo) = metrics.min_fan_out {
        where_clauses.push("(sm.fan_out IS NOT NULL AND sm.fan_out >= ?)".to_string());
        params.push(Box::new(min_fo as i64));
    }

    // Structural search: --inside KIND (find descendants within any ancestor of type KIND)
    if let Some(inside_kind) = inside_kind {
        if has_ast_table {
            // Use a correlated EXISTS subquery with recursive CTE to check all ancestors
            // This handles nested structures like: function -> block -> closure
            where_clauses.push(
                "EXISTS (
                    WITH RECURSIVE ancestors AS (
                        SELECT id, parent_id FROM ast_nodes WHERE id = an.id
                        UNION ALL
                        SELECT a.id, a.parent_id FROM ast_nodes a
                        JOIN ancestors anc ON a.id = anc.parent_id
                        WHERE a.parent_id IS NOT NULL
                    )
                    SELECT 1 FROM ast_nodes p
                    JOIN ancestors anc ON p.id = anc.parent_id
                    WHERE p.kind = ?
                )"
                .to_string(),
            );
            params.push(Box::new(inside_kind.to_string()));
        }
    }

    // Structural search: --contains KIND (find parents with children of type KIND)
    if let Some(contains_kind) = contains_kind {
        if has_ast_table {
            where_clauses.push("an.id IN (SELECT parent_id FROM ast_nodes WHERE kind = ? AND parent_id IS NOT NULL)".to_string());
            params.push(Box::new(contains_kind.to_string()));
        }
    }

    // Note: Depth filtering (min_depth, max_depth) is handled post-query
    // due to SQLite recursive CTE limitations in WHERE clauses.
    // See Task 6 for post-query filtering implementation.

    // SymbolSet filter condition
    let symbol_set_strategy = if let Some(symbol_ids) = symbol_set_filter {
        let strategy = symbol_set_filter_strategy(symbol_ids);
        match strategy {
            SymbolSetStrategy::InClause if !symbol_ids.is_empty() => {
                let placeholders = symbol_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                where_clauses.push(format!("json_extract(s.data, '$.symbol_id') IN ({})", placeholders));
                params.extend(symbol_ids.iter().map(|id| Box::new(id.clone()) as Box<dyn ToSql>));
            }
            SymbolSetStrategy::TempTable => {
                // Will be handled via JOIN in execution (Plan 11-04)
                where_clauses.push("EXISTS (SELECT 1 FROM symbol_set_filter WHERE symbol_set_filter.symbol_id = json_extract(s.data, '$.symbol_id'))".to_string());
            }
            _ => {}
        }
        strategy
    } else {
        SymbolSetStrategy::None
    };

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        // Include AST columns when ast_nodes table exists
        if has_ast_table {
            "SELECT s.data, f.file_path, sm.fan_in, sm.fan_out, sm.cyclomatic_complexity, json_extract(s.data, '$.symbol_id') AS symbol_id, an.id AS ast_id, an.kind AS ast_kind, an.parent_id AS ast_parent_id, an.byte_start AS ast_byte_start, an.byte_end AS ast_byte_end"
        } else {
            "SELECT s.data, f.file_path, sm.fan_in, sm.fan_out, sm.cyclomatic_complexity, json_extract(s.data, '$.symbol_id') AS symbol_id"
        }
    };

    let mut sql = format!(
        "{select_clause}
FROM (
    SELECT id,
           data,
           json_extract(data, '$.name') AS name,
           json_extract(data, '$.display_fqn') AS display_fqn,
           json_extract(data, '$.fqn') AS fqn,
           json_extract(data, '$.canonical_fqn') AS canonical_fqn,
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
LEFT JOIN symbol_metrics sm ON s.id = sm.symbol_id
{ast_join}
WHERE {where_clause}",
        select_clause = select_clause,
        ast_join = if has_ast_table {
            // Exact match on byte span - this is the correct approach for Magellan
            // The get_ast_context_for_symbol() function handles overlap matching when needed
            "LEFT JOIN ast_nodes an ON json_extract(s.data, '$.byte_start') = an.byte_start AND json_extract(s.data, '$.byte_end') = an.byte_end".to_string()
        } else {
            "".to_string()
        },
        where_clause = if where_clauses.is_empty() {
            "1=1".to_string()
        } else {
            where_clauses.join(" AND ")
        },
    );

    if !count_only {
        // Determine ORDER BY clause based on sort mode
        let order_by = match sort_by {
            SortMode::FanIn => {
                // Sort by fan_in descending, NULLs last
                "COALESCE(sm.fan_in, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::FanOut => {
                // Sort by fan_out descending, NULLs last
                "COALESCE(sm.fan_out, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::Complexity => {
                // Sort by cyclomatic_complexity descending, NULLs last
                "COALESCE(sm.cyclomatic_complexity, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::AstComplexity => {
                // Sort by AST complexity (cyclomatic_complexity), same as Complexity mode
                "COALESCE(sm.cyclomatic_complexity, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::NestingDepth => {
                // Nesting depth requires post-query calculation
                // Fall back to position ordering for now
                // Future: batch depth calculation then in-memory sort
                "s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::Position => {
                // Position-based ordering (faster, pure SQL ORDER BY)
                "s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::Relevance => {
                // Relevance ordering happens in-memory after scoring
                "s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
        };
        sql.push_str(&format!("\nORDER BY {}\n", order_by));
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params, symbol_set_strategy)
}

pub(crate) fn build_reference_query(
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

pub(crate) fn build_call_query(
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
