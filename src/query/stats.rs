//! Stats command: code health summary from Magellan metrics.
//!
//! Reports dead code symbols, high fan-in hotspots, CFG complexity
//! distribution, and coverage gaps. Supports JSON output.

use rusqlite::Connection;
use std::path::Path;

use crate::error::LlmError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsResponse {
    pub database: String,
    pub schema_version: Option<i64>,
    pub symbols: SymbolStats,
    pub dead_code: DeadCodeStats,
    pub hotspots: Vec<HotspotSymbol>,
    pub coverage: CoverageStats,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolStats {
    pub total: usize,
    pub by_kind: Vec<(String, usize)>,
    pub avg_fan_in: f64,
    pub avg_complexity: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadCodeStats {
    pub zero_fan_in: usize,
    pub zero_fan_out: usize,
    pub likely_dead: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HotspotSymbol {
    pub symbol_name: String,
    pub kind: String,
    pub file_path: String,
    pub score: i64,
    pub fan_in: i64,
    pub cyclomatic_complexity: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageStats {
    pub tables_exist: bool,
    pub covered_symbols: Option<usize>,
    pub total_symbols: usize,
}

pub fn run_stats(conn: &Connection, db_path: &Path) -> Result<StatsResponse, LlmError> {
    let schema_version: Option<i64> = conn
        .query_row(
            "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let symbols = gather_symbol_stats(conn)?;
    let dead_code = gather_dead_code(conn)?;
    let hotspots = gather_hotspots(conn, 10)?;
    let coverage = gather_coverage(conn)?;

    Ok(StatsResponse {
        database: db_path.display().to_string(),
        schema_version,
        symbols,
        dead_code,
        hotspots,
        coverage,
    })
}

fn gather_symbol_stats(conn: &Connection) -> Result<SymbolStats, LlmError> {
    let metrics_exist: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='symbol_metrics'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !metrics_exist {
        let total: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM graph_entities WHERE kind != 'File' AND kind != 'Module'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        return Ok(SymbolStats {
            total,
            by_kind: Vec::new(),
            avg_fan_in: 0.0,
            avg_complexity: 0.0,
        });
    }

    let total: usize = conn
        .query_row("SELECT COUNT(*) FROM symbol_metrics", [], |row| row.get(0))
        .unwrap_or(0);

    let avg_fan_in: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(fan_in), 0.0) FROM symbol_metrics",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let avg_complexity: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(cyclomatic_complexity), 0.0) FROM symbol_metrics",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let mut kind_stmt = conn.prepare(
        "SELECT kind, COUNT(*) as cnt FROM symbol_metrics GROUP BY kind ORDER BY cnt DESC",
    )?;
    let mut kind_rows = kind_stmt.query([])?;
    let mut by_kind = Vec::new();
    while let Some(row) = kind_rows.next()? {
        by_kind.push((row.get(0)?, row.get(1)?));
    }

    Ok(SymbolStats {
        total,
        by_kind,
        avg_fan_in,
        avg_complexity,
    })
}

fn gather_dead_code(conn: &Connection) -> Result<DeadCodeStats, LlmError> {
    let metrics_exist: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='symbol_metrics'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !metrics_exist {
        return Ok(DeadCodeStats {
            zero_fan_in: 0,
            zero_fan_out: 0,
            likely_dead: Vec::new(),
        });
    }

    let zero_fan_in: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM symbol_metrics WHERE fan_in = 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let zero_fan_out: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM symbol_metrics WHERE fan_out = 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT symbol_name FROM symbol_metrics \
         WHERE fan_in = 0 AND fan_out = 0 \
         ORDER BY symbol_name LIMIT 20",
    )?;
    let mut rows = stmt.query([])?;
    let mut likely_dead = Vec::new();
    while let Some(row) = rows.next()? {
        likely_dead.push(row.get(0)?);
    }

    Ok(DeadCodeStats {
        zero_fan_in,
        zero_fan_out,
        likely_dead,
    })
}

fn gather_hotspots(conn: &Connection, limit: usize) -> Result<Vec<HotspotSymbol>, LlmError> {
    let metrics_exist: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='symbol_metrics'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !metrics_exist {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT symbol_name, kind, file_path, \
                (fan_in * cyclomatic_complexity) AS score, \
                fan_in, cyclomatic_complexity \
         FROM symbol_metrics \
         ORDER BY score DESC \
         LIMIT {}",
        limit
    ))?;
    let mut rows = stmt.query([])?;
    let mut hotspots = Vec::new();
    while let Some(row) = rows.next()? {
        hotspots.push(HotspotSymbol {
            symbol_name: row.get(0)?,
            kind: row.get(1)?,
            file_path: row.get(2)?,
            score: row.get(3)?,
            fan_in: row.get(4)?,
            cyclomatic_complexity: row.get(5)?,
        });
    }
    Ok(hotspots)
}

fn gather_coverage(conn: &Connection) -> Result<CoverageStats, LlmError> {
    let tables = [
        "cfg_block_coverage",
        "cfg_edge_coverage",
        "cfg_coverage_meta",
    ];
    let mut all_exist = true;
    for table in &tables {
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1 LIMIT 1",
                [table],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !exists {
            all_exist = false;
            break;
        }
    }

    if !all_exist {
        let total_symbols: usize = conn
            .query_row("SELECT COUNT(*) FROM symbol_metrics", [], |row| row.get(0))
            .unwrap_or(0);
        return Ok(CoverageStats {
            tables_exist: false,
            covered_symbols: None,
            total_symbols,
        });
    }

    let covered: usize = conn
        .query_row(
            "SELECT COUNT(DISTINCT symbol_id) FROM cfg_block_coverage",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_symbols: usize = conn
        .query_row("SELECT COUNT(*) FROM symbol_metrics", [], |row| row.get(0))
        .unwrap_or(0);

    Ok(CoverageStats {
        tables_exist: true,
        covered_symbols: Some(covered),
        total_symbols,
    })
}
