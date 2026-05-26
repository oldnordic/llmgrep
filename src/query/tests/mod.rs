use super::builder::{build_call_query, build_reference_query, build_search_query};
use super::util::{like_pattern, like_prefix, load_file, normalize_kind_label, score_match};
use super::*;
use crate::algorithm::AlgorithmOptions;
use crate::SortMode;
use rusqlite::Connection;

mod chunk_tests;
mod metrics_tests;
mod query_builder_tests;
mod scoring_tests;
mod search_calls_tests;
mod search_references_tests;
mod search_symbols_tests;
mod symbol_id_tests;
mod util_tests;

fn create_test_db() -> (tempfile::NamedTempFile, Connection) {
    let db_file =
        tempfile::NamedTempFile::new().expect("failed to create temp file for test database");
    let conn = Connection::open(db_file.path()).expect("failed to open test database connection");

    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            data TEXT NOT NULL
        )",
        [],
    )
    .expect("failed to create graph_entities table");
    conn.execute(
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        )",
        [],
    )
    .expect("failed to create graph_edges table");
    conn.execute(
        "CREATE TABLE symbol_metrics (
            symbol_id INTEGER PRIMARY KEY,
            symbol_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_path TEXT NOT NULL,
            loc INTEGER NOT NULL DEFAULT 0,
            estimated_loc REAL NOT NULL DEFAULT 0.0,
            fan_in INTEGER NOT NULL DEFAULT 0,
            fan_out INTEGER NOT NULL DEFAULT 0,
            cyclomatic_complexity INTEGER NOT NULL DEFAULT 1,
            last_updated INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (symbol_id) REFERENCES graph_entities(id) ON DELETE CASCADE
        )",
        [],
    )
    .expect("failed to create symbol_metrics table");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
        [],
    ).expect("failed to insert test File entity");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Symbol', '{\"name\":\"test_func\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"test_func\",\"fqn\":\"module::test_func\",\"canonical_fqn\":\"/test/file.rs::test_func\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
            (11, 'Symbol', '{\"name\":\"TestStruct\",\"kind\":\"Struct\",\"kind_normalized\":\"struct\",\"display_fqn\":\"TestStruct\",\"fqn\":\"module::TestStruct\",\"canonical_fqn\":\"/test/file.rs::TestStruct\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
            (12, 'Symbol', '{\"name\":\"helper\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"helper\",\"fqn\":\"module::helper\",\"canonical_fqn\":\"/test/file.rs::helper\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
        [],
    ).expect("failed to insert test Symbol entities");

    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
        [],
    ).expect("failed to insert test DEFINES edges");

    (db_file, conn)
}
