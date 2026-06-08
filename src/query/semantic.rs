//! Semantic search using HNSW vector similarity.
//!
//! This module provides natural language code search by embedding the query
//! via a local Ollama instance and searching the persisted HNSW index in the
//! Magellan database.

use crate::error::LlmError;
use crate::output::{SemanticMatch, SemanticSearchResponse, Span};
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Embedding provider configuration read from `~/.config/magellan/config.toml`.
#[derive(Debug, Deserialize)]
struct MagellanConfig {
    embeddings: EmbeddingsConfig,
}

#[derive(Debug, Deserialize)]
struct EmbeddingsConfig {
    provider: String,
    base_url: String,
    model: String,
    #[serde(default)]
    api_key: String,
}

/// Options for a semantic search operation.
#[derive(Debug, Clone)]
pub struct SemanticSearchOptions<'a> {
    pub db_path: &'a Path,
    pub query: &'a str,
    pub limit: usize,
    pub path_filter: Option<&'a str>,
}

/// Run semantic search: embed query → load HNSW → search → resolve entities.
pub fn search_semantic(options: SemanticSearchOptions) -> Result<SemanticSearchResponse, LlmError> {
    if options.query.trim().is_empty() {
        return Err(LlmError::EmptyQuery);
    }

    // ------------------------------------------------------------------
    // 1. Open DB and verify HNSW index exists (fail fast before embedding)
    // ------------------------------------------------------------------
    let conn = Connection::open(options.db_path).map_err(LlmError::SqliteError)?;

    let hnsw_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='hnsw_indexes'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !hnsw_exists {
        return Err(LlmError::SearchFailed {
            reason: "No HNSW index found in database. Run `magellan embed --db <db>` first."
                .to_string(),
        });
    }

    // ------------------------------------------------------------------
    // 2. Read Magellan config to discover Ollama endpoint & model
    // ------------------------------------------------------------------
    let config = read_magellan_config()?;
    if config.embeddings.provider != "ollama" {
        return Err(LlmError::SearchFailed {
            reason: format!(
                "Semantic search only supports Ollama embeddings, found provider: {}",
                config.embeddings.provider
            ),
        });
    }

    // ------------------------------------------------------------------
    // 3. Embed the natural-language query via Ollama
    // ------------------------------------------------------------------
    let query_vector = embed_query(
        &config.embeddings.base_url,
        &config.embeddings.model,
        &config.embeddings.api_key,
        options.query,
    )?;

    // ------------------------------------------------------------------
    // 4. Load the persisted HNSW index
    // ------------------------------------------------------------------
    let index = sqlitegraph::hnsw::HnswIndex::load_with_vectors(&conn, "symbols").map_err(|e| {
        LlmError::SearchFailed {
            reason: format!("Failed to load HNSW index: {e}"),
        }
    })?;

    // ------------------------------------------------------------------
    // 4. Search the HNSW index
    // ------------------------------------------------------------------
    let hnsw_results =
        index
            .search(&query_vector, options.limit)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("HNSW search failed: {e}"),
            })?;

    if hnsw_results.is_empty() {
        return Ok(SemanticSearchResponse {
            results: Vec::new(),
            query: options.query.to_string(),
            total_count: 0,
            path_filter: options.path_filter.map(|s| s.to_string()),
        });
    }

    // ------------------------------------------------------------------
    // 5. Resolve vector IDs → graph_entities
    // ------------------------------------------------------------------
    let vector_ids: Vec<u64> = hnsw_results.iter().map(|(id, _)| *id).collect();
    let distances: HashMap<u64, f32> = hnsw_results.into_iter().collect();

    let mut results =
        resolve_vectors_to_entities(&conn, &vector_ids, &distances, options.path_filter)?;

    // Sort by semantic distance (ascending — lower is closer)
    results.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_count = results.len() as u64;

    Ok(SemanticSearchResponse {
        results,
        query: options.query.to_string(),
        total_count,
        path_filter: options.path_filter.map(|s| s.to_string()),
    })
}

// -----------------------------------------------------------------------
// Config helpers
// -----------------------------------------------------------------------

fn read_magellan_config() -> Result<MagellanConfig, LlmError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| LlmError::SearchFailed {
            reason: "Unable to determine home directory (HOME or USERPROFILE env var not set)"
                .to_string(),
        })?;
    let config_path = std::path::PathBuf::from(home).join(".config/magellan/config.toml");

    let contents = std::fs::read_to_string(&config_path).map_err(|e| LlmError::SearchFailed {
        reason: format!(
            "Cannot read Magellan config at {}: {e}. Ensure `~/.config/magellan/config.toml` exists with an [embeddings] section.",
            config_path.display()
        ),
    })?;

    let config: MagellanConfig = toml::from_str(&contents).map_err(|e| LlmError::SearchFailed {
        reason: format!("Failed to parse Magellan config: {e}"),
    })?;

    Ok(config)
}

// -----------------------------------------------------------------------
// Embedding helpers
// -----------------------------------------------------------------------

#[derive(serde::Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(serde::Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

fn embed_query(
    base_url: &str,
    model: &str,
    api_key: &str,
    prompt: &str,
) -> Result<Vec<f32>, LlmError> {
    let url = format!("{}/api/embeddings", base_url.trim_end_matches('/'));

    let body = OllamaEmbedRequest { model, prompt };
    let json_body = serde_json::to_string(&body).map_err(|e| LlmError::SearchFailed {
        reason: format!("Failed to serialize embedding request: {e}"),
    })?;

    let mut request = ureq::post(&url).header("Content-Type", "application/json");
    if !api_key.is_empty() {
        request = request.header("Authorization", &format!("Bearer {api_key}"));
    }
    let response = request
        .send(&json_body)
        .map_err(|e| LlmError::SearchFailed {
            reason: format!("Ollama embedding request failed: {e}"),
        })?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| LlmError::SearchFailed {
            reason: format!("Failed to read Ollama embedding response body: {e}"),
        })?;
    let resp: OllamaEmbedResponse =
        serde_json::from_str(&body).map_err(|e| LlmError::SearchFailed {
            reason: format!("Failed to parse Ollama embedding response: {e}"),
        })?;

    Ok(resp.embedding)
}

// -----------------------------------------------------------------------
// Resolution helpers
// -----------------------------------------------------------------------

/// Resolved entity row from graph_entities.
struct EntityRow {
    kind: String,
    name: String,
    file_path: String,
    data: String,
    start_line: Option<i64>,
    start_col: Option<i64>,
}

fn resolve_vectors_to_entities(
    conn: &Connection,
    vector_ids: &[u64],
    distances: &HashMap<u64, f32>,
    path_filter: Option<&str>,
) -> Result<Vec<SemanticMatch>, LlmError> {
    let mut results = Vec::with_capacity(vector_ids.len());

    for vid in vector_ids {
        // Each hnsw_vectors row stores metadata like {"entity_id": 12345}
        let metadata_json: Option<String> = conn
            .query_row(
                "SELECT metadata FROM hnsw_vectors WHERE id = ?1",
                [vid],
                |row| row.get(0),
            )
            .optional()
            .map_err(LlmError::SqliteError)?;

        let metadata_json = match metadata_json {
            Some(m) => m,
            None => continue,
        };

        let metadata: serde_json::Value =
            serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
        let entity_id = metadata.get("entity_id").and_then(|v| v.as_i64());

        let entity_id = match entity_id {
            Some(id) => id,
            None => continue,
        };

        // Lookup the entity in graph_entities
        let entity: Option<EntityRow> = conn
            .query_row(
                "SELECT kind, name, file_path, data, start_line, start_col
                 FROM graph_entities
                 WHERE id = ?1",
                [entity_id],
                |row| {
                    Ok(EntityRow {
                        kind: row.get(0)?,
                        name: row.get(1)?,
                        file_path: row.get(2)?,
                        data: row.get(3)?,
                        start_line: row.get(4)?,
                        start_col: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(LlmError::SqliteError)?;

        let entity = match entity {
            Some(e) => e,
            None => continue,
        };

        // Apply path filter if present
        if let Some(filter) = path_filter {
            if !entity.file_path.contains(filter) {
                continue;
            }
        }

        // Parse `data` JSON for additional fields
        let data_json: serde_json::Value =
            serde_json::from_str(&entity.data).unwrap_or(serde_json::json!({}));
        let language = data_json
            .get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let canonical_fqn = data_json
            .get("canonical_fqn")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let symbol_id = data_json
            .get("symbol_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let end_line = data_json
            .get("end_line")
            .and_then(|v| v.as_i64())
            .map(|v| v as u64);
        let end_col = data_json
            .get("end_col")
            .and_then(|v| v.as_i64())
            .map(|v| v as u64);

        let distance = distances.get(vid).copied().unwrap_or(1.0);
        // Convert cosine distance [0,2] to a similarity score [0,100] for display
        let score = ((1.0 - (distance / 2.0)).clamp(0.0, 1.0) * 100.0).round() as u64;

        results.push(SemanticMatch {
            match_id: format!("semantic-{}", vid),
            span: Span {
                span_id: format!("semantic-span-{}", vid),
                file_path: entity.file_path,
                byte_start: data_json
                    .get("byte_start")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                byte_end: data_json
                    .get("byte_end")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                start_line: entity.start_line.unwrap_or(1) as u64,
                start_col: entity.start_col.unwrap_or(1) as u64,
                end_line: end_line.unwrap_or(entity.start_line.unwrap_or(1) as u64),
                end_col: end_col.unwrap_or(1),
                context: None,
            },
            name: entity.name,
            kind: entity.kind,
            language,
            canonical_fqn,
            symbol_id,
            distance,
            score,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // graph_entities schema used by llmgrep
        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT,
                file_path TEXT,
                data TEXT NOT NULL,
                start_line INTEGER,
                start_col INTEGER
            )",
            [],
        )
        .unwrap();
        conn
    }

    fn seed_graph_entities(conn: &Connection) {
        conn.execute(
            "INSERT INTO graph_entities (id, kind, name, file_path, data, start_line, start_col)
             VALUES
                (1, 'File', 'main.rs', 'src/main.rs', '{\"path\":\"src/main.rs\"}', 1, 0),
                (10, 'Symbol', 'parse_args', 'src/main.rs',
                 '{\"name\":\"parse_args\",\"language\":\"Rust\",\"canonical_fqn\":\"crate::parse_args\",\"symbol_id\":\"sym10\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":20,\"end_col\":1}',
                 5, 0),
                (11, 'Symbol', 'handle_error', 'src/main.rs',
                 '{\"name\":\"handle_error\",\"language\":\"Rust\",\"canonical_fqn\":\"crate::handle_error\",\"symbol_id\":\"sym11\",\"byte_start\":300,\"byte_end\":400,\"start_line\":25,\"start_col\":0,\"end_line\":40,\"end_col\":1}',
                 25, 0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn test_search_semantic_rejects_empty_query() {
        let db_file = tempfile::NamedTempFile::new().unwrap();
        let options = SemanticSearchOptions {
            db_path: db_file.path(),
            query: "   ",
            limit: 10,
            path_filter: None,
        };
        let err = search_semantic(options).unwrap_err();
        assert!(matches!(err, LlmError::EmptyQuery));
    }

    #[test]
    fn test_search_semantic_no_hnsw_index() {
        let db_file = tempfile::NamedTempFile::new().unwrap();
        // Create a DB with graph_entities but no HNSW schema
        let conn = Connection::open(db_file.path()).unwrap();
        conn.execute(
            "CREATE TABLE graph_entities (id INTEGER PRIMARY KEY, kind TEXT, name TEXT, data TEXT)",
            [],
        )
        .unwrap();
        drop(conn);

        let options = SemanticSearchOptions {
            db_path: db_file.path(),
            query: "parse arguments",
            limit: 10,
            path_filter: None,
        };
        let err = search_semantic(options).unwrap_err();
        match err {
            LlmError::SearchFailed { reason } => {
                assert!(
                    reason.contains("No HNSW index found"),
                    "unexpected reason: {reason}"
                );
            }
            other => panic!("expected SearchFailed error, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_vectors_to_entities_basic() {
        let conn = create_test_conn();
        seed_graph_entities(&conn);

        // No hnsw_vectors table needed: resolve reads metadata from hnsw_vectors,
        // but we can skip that by directly testing with an empty distance map
        // Actually, resolve_vectors_to_entities reads from hnsw_vectors.
        // Let's create the table and insert a metadata row.
        conn.execute(
            "CREATE TABLE hnsw_vectors (
                id INTEGER PRIMARY KEY,
                index_id INTEGER,
                vector_data BLOB,
                metadata TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hnsw_vectors (id, index_id, metadata) VALUES (1, 1, '{\"entity_id\": 10}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hnsw_vectors (id, index_id, metadata) VALUES (2, 1, '{\"entity_id\": 11}')",
            [],
        )
        .unwrap();

        let mut distances = HashMap::new();
        distances.insert(1, 0.1f32);
        distances.insert(2, 0.5f32);

        let results = resolve_vectors_to_entities(&conn, &[1, 2], &distances, None).unwrap();
        assert_eq!(results.len(), 2);

        // Should be sorted by distance ascending
        assert_eq!(results[0].name, "parse_args");
        assert_eq!(results[0].distance, 0.1);
        assert_eq!(results[0].score, 95); // (1 - 0.1/2) * 100 = 95

        assert_eq!(results[1].name, "handle_error");
        assert_eq!(results[1].distance, 0.5);
    }

    #[test]
    fn test_resolve_vectors_to_entities_with_path_filter() {
        let conn = create_test_conn();
        seed_graph_entities(&conn);

        conn.execute(
            "CREATE TABLE hnsw_vectors (
                id INTEGER PRIMARY KEY,
                index_id INTEGER,
                vector_data BLOB,
                metadata TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hnsw_vectors (id, index_id, metadata) VALUES (1, 1, '{\"entity_id\": 10}')",
            [],
        )
        .unwrap();

        let mut distances = HashMap::new();
        distances.insert(1, 0.1f32);

        // Filter that matches
        let results =
            resolve_vectors_to_entities(&conn, &[1], &distances, Some("main.rs")).unwrap();
        assert_eq!(results.len(), 1);

        // Filter that does not match
        let results = resolve_vectors_to_entities(&conn, &[1], &distances, Some("lib.rs")).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_resolve_vectors_to_entities_skips_missing_metadata() {
        let conn = create_test_conn();
        seed_graph_entities(&conn);

        conn.execute(
            "CREATE TABLE hnsw_vectors (
                id INTEGER PRIMARY KEY,
                index_id INTEGER,
                vector_data BLOB,
                metadata TEXT
            )",
            [],
        )
        .unwrap();
        // Metadata with no entity_id -> should be skipped
        conn.execute(
            "INSERT INTO hnsw_vectors (id, index_id, metadata) VALUES (1, 1, '{\"label\": \"test\"}')",
            [],
        )
        .unwrap();

        let distances = HashMap::new();
        let results = resolve_vectors_to_entities(&conn, &[1], &distances, None).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_resolve_vectors_to_entities_skips_missing_entity() {
        let conn = create_test_conn();
        // No entities inserted

        conn.execute(
            "CREATE TABLE hnsw_vectors (
                id INTEGER PRIMARY KEY,
                index_id INTEGER,
                vector_data BLOB,
                metadata TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO hnsw_vectors (id, index_id, metadata) VALUES (1, 1, '{\"entity_id\": 999}')",
            [],
        )
        .unwrap();

        let distances = HashMap::new();
        let results = resolve_vectors_to_entities(&conn, &[1], &distances, None).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_read_magellan_config_success() {
        let tmpdir = tempfile::tempdir().unwrap();
        let config_dir = tmpdir.path().join(".config/magellan");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        std::fs::write(
            &config_path,
            r#"
[embeddings]
provider = "ollama"
base_url = "http://localhost:11434"
model = "qodo-embed-1.5b-q8-16k"
"#,
        )
        .unwrap();

        // Temporarily override HOME so read_magellan_config finds our test file
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tmpdir.path());
        let result = read_magellan_config();
        // Restore HOME
        match original_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }

        let config = result.unwrap();
        assert_eq!(config.embeddings.provider, "ollama");
        assert_eq!(config.embeddings.base_url, "http://localhost:11434");
        assert_eq!(config.embeddings.model, "qodo-embed-1.5b-q8-16k");
    }

    #[test]
    fn test_read_magellan_config_missing_file() {
        let tmpdir = tempfile::tempdir().unwrap();
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tmpdir.path());
        let result = read_magellan_config();
        match original_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }

        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::SearchFailed { reason } => {
                assert!(reason.contains("Cannot read Magellan config"));
            }
            other => panic!("expected SearchFailed, got {:?}", other),
        }
    }
}
