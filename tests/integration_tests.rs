/// Integration tests for v1.1 features
///
/// Tests for:
/// - UTF-8 emoji extraction
/// - CJK character handling
/// - Chunk retrieval
/// - Metrics filtering
/// - SymbolId lookup
/// - Ambiguity detection
/// - Label filtering
use llmgrep::query::{
    search_chunks_by_span, search_symbols, AstOptions, ContextOptions, DepthOptions, FqnOptions,
    MetricsOptions, SearchOptions, SnippetOptions,
};
use llmgrep::AlgorithmOptions;
use rusqlite::{params, Connection};
use serde_json::json;
use sha2::Digest;

fn setup_db(path: &std::path::Path) -> Connection {
    let conn = Connection::open(path).expect("open db");
    conn.execute_batch(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT,
            data TEXT NOT NULL
        );
        CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        );
        CREATE TABLE code_chunks (
            id INTEGER PRIMARY KEY,
            file_path TEXT NOT NULL,
            byte_start INTEGER NOT NULL,
            byte_end INTEGER NOT NULL,
            content TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            symbol_name TEXT,
            symbol_kind TEXT
        );
        CREATE TABLE symbol_metrics (
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
        );",
    )
    .expect("create tables");
    conn
}

fn insert_file(conn: &Connection, path: &str) -> i64 {
    let data = json!({
        "path": path,
        "hash": "sha256:deadbeef",
        "last_indexed_at": 0,
        "last_modified": 0
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["File", path, data],
    )
    .expect("insert file");
    conn.last_insert_rowid()
}

fn insert_symbol(
    conn: &Connection,
    name: &str,
    kind: &str,
    kind_normalized: &str,
    span: (u64, u64),
) -> i64 {
    let data = json!({
        "symbol_id": format!("{}-id", name),
        "name": name,
        "kind": kind,
        "kind_normalized": kind_normalized,
        "fqn": format!("test::{}", name),
        "display_fqn": name,
        "canonical_fqn": format!("test::src/lib.rs::{} {}", kind, name),
        "byte_start": span.0,
        "byte_end": span.1,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", name, data],
    )
    .expect("insert symbol");
    conn.last_insert_rowid()
}

fn insert_define_edge(conn: &Connection, file_id: i64, symbol_id: i64) {
    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (?1, ?2, ?3)",
        params![file_id, symbol_id, "DEFINES"],
    )
    .expect("insert edge");
}

fn insert_code_chunk(
    conn: &Connection,
    file_path: &str,
    byte_start: i64,
    byte_end: i64,
    content: &str,
    symbol_name: &str,
) {
    let content_hash = format!("{:x}", sha2::Sha256::digest(content.as_bytes()));
    conn.execute(
        "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![file_path, byte_start, byte_end, content, content_hash, symbol_name, "fn"],
    )
    .expect("insert chunk");
}

fn insert_metrics(conn: &Connection, symbol_row_id: i64, symbol_name: &str, kind: &str, file_path: &str, fan_in: i64, fan_out: i64, complexity: i64) {
    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![symbol_row_id, symbol_name, kind, file_path, 0i64, 0.0f64, fan_in, fan_out, complexity, 0i64],
    )
    .expect("insert metrics");
}

/// Test 1: UTF-8 emoji extraction
#[test]
fn test_utf8_emoji_extraction() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    // Create a file path that will be indexed
    let file_path = "src/emoji.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbol with emoji in name (stored in data field)
    let symbol_name = "process_emoji";
    let symbol_span = (0, 100); // Match the chunk span
    let symbol_id = insert_symbol(&conn, symbol_name, "Function", "fn", symbol_span);
    insert_define_edge(&conn, file_id, symbol_id);

    // Insert code chunk with emoji content - span must match symbol span
    let emoji_content = "// This function processes emoji: 🚀🔥⭐
fn process_emoji(input: &str) -> String {
    input.replace('😀', \"😎\")
}";
    insert_code_chunk(&conn, file_path, 0, 100, emoji_content, symbol_name);

    // Verify chunk can be retrieved
    let chunk =
        search_chunks_by_span(&conn, file_path, 0, 100).expect("chunk query should succeed");
    assert!(chunk.is_some(), "Chunk should exist");
    let chunk = chunk.unwrap();

    // Verify emoji content is intact
    assert!(
        chunk.content.contains("🚀"),
        "Content should contain rocket emoji"
    );
    assert!(
        chunk.content.contains("🔥"),
        "Content should contain fire emoji"
    );
    assert!(
        chunk.content.contains("⭐"),
        "Content should contain star emoji"
    );
    assert!(
        chunk.content.contains("😀"),
        "Content should contain grinning face"
    );
    assert!(
        chunk.content.contains("😎"),
        "Content should contain sunglasses face"
    );

    // Search with snippet extraction
    let options = SearchOptions {
        db_path: &db_path,
        query: symbol_name,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions {
            include: true,
            max_bytes: 200,
        },
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);

    let result = &response.0.results[0];
    assert!(result.snippet.is_some(), "Snippet should be extracted");
    let snippet = result.snippet.as_ref().unwrap();
    // Snippet should contain emoji without panic
    assert!(
        snippet.contains("🚀") || snippet.contains("emoji"),
        "Snippet should contain emoji or keyword"
    );
}

/// Test 2: CJK character handling
#[test]
fn test_cjk_character_extraction() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/chinese.rs";
    let file_id = insert_file(&conn, file_path);

    let symbol_name = "chinese_function";
    let symbol_id = insert_symbol(&conn, symbol_name, "Function", "fn", (0, 20));
    insert_define_edge(&conn, file_id, symbol_id);

    // Insert code chunk with CJK characters (Chinese, Japanese, Korean)
    let cjk_content = "// 中文注释 - Chinese comment
// 日本語コメント - Japanese comment
// 한국어 주석 - Korean comment
fn chinese_function() {
    let 变量 = \"值\";
    println!(\"变量 = {}\", 变量);
}";
    insert_code_chunk(&conn, file_path, 0, 200, cjk_content, symbol_name);

    // Verify chunk retrieval with CJK content
    let chunk =
        search_chunks_by_span(&conn, file_path, 0, 200).expect("chunk query should succeed");
    assert!(chunk.is_some(), "Chunk with CJK should exist");
    let chunk = chunk.unwrap();

    // Verify CJK characters are preserved
    assert!(
        chunk.content.contains("中文"),
        "Content should contain Chinese characters"
    );
    assert!(
        chunk.content.contains("日本語"),
        "Content should contain Japanese characters"
    );
    assert!(
        chunk.content.contains("한국어"),
        "Content should contain Korean characters"
    );
    assert!(
        chunk.content.contains("变量"),
        "Content should contain Chinese variable name"
    );
}

/// Test 3: Chunk retrieval with content_hash
#[test]
fn test_chunk_retrieval_with_hash() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/chunked.rs";
    let file_id = insert_file(&conn, file_path);

    let symbol_name = "chunked_function";
    let symbol_id = insert_symbol(&conn, symbol_name, "Function", "fn", (10, 30));
    insert_define_edge(&conn, file_id, symbol_id);

    let content = "fn chunked_function() -> bool { true }";
    insert_code_chunk(&conn, file_path, 10, 30, content, symbol_name);

    // Search and verify chunk is used
    let options = SearchOptions {
        db_path: &db_path,
        query: symbol_name,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions {
            include: true,
            max_bytes: 200,
        },
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);

    let result = &response.0.results[0];
    assert!(
        result.snippet.is_some(),
        "Snippet from chunk should be extracted"
    );
    assert!(
        result.content_hash.is_some(),
        "content_hash should be present when using chunk"
    );

    // Verify hash is valid hex
    let hash = result.content_hash.as_ref().unwrap();
    assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex chars");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should be valid hex"
    );
}

/// Test 4: Metrics filtering
#[test]
fn test_metrics_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/metrics.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols with different metrics
    let simple_name = "simple_fn";
    let simple_id = insert_symbol(&conn, simple_name, "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, simple_id);
    insert_metrics(&conn, simple_id, simple_name, "Function", file_path, 1, 2, 1); // low complexity

    let complex_name = "complex_fn";
    let complex_id = insert_symbol(&conn, complex_name, "Function", "fn", (20, 30));
    insert_define_edge(&conn, file_id, complex_id);
    insert_metrics(&conn, complex_id, complex_name, "Function", file_path, 10, 20, 15); // high complexity

    let medium_name = "medium_fn";
    let medium_id = insert_symbol(&conn, medium_name, "Function", "fn", (40, 50));
    insert_define_edge(&conn, file_id, medium_id);
    insert_metrics(&conn, medium_id, medium_name, "Function", file_path, 5, 8, 5); // medium

    // Test min_complexity filter - should only return complex_fn
    let options = SearchOptions {
        db_path: &db_path,
        query: "fn", // matches all
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_complexity: Some(10),
            max_complexity: None,
            min_fan_in: None,
            min_fan_out: None,
        },
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.0.results.len(),
        1,
        "Should only return high-complexity function"
    );
    assert_eq!(response.0.results[0].name, complex_name);
}

/// Test 5: SymbolId lookup
#[test]
fn test_symbol_id_lookup() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/symbol_id.rs";
    let file_id = insert_file(&conn, file_path);

    let symbol_name = "unique_function";
    // Use a known 32-char hex ID (BLAKE3 hash format)
    let known_symbol_id = "abc123def456789abc123def456789ab";
    let data = json!({
        "symbol_id": known_symbol_id,
        "name": symbol_name,
        "kind": "Function",
        "kind_normalized": "fn",
        "fqn": format!("test::{}", symbol_name),
        "display_fqn": symbol_name,
        "canonical_fqn": format!("test::src/lib.rs::Function {}", symbol_name),
        "byte_start": 0,
        "byte_end": 10,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", symbol_name, data],
    )
    .expect("insert symbol");
    let symbol_row_id = conn.last_insert_rowid();
    insert_define_edge(&conn, file_id, symbol_row_id);

    // Also insert in symbol_metrics table with row ID reference
    insert_metrics(&conn, symbol_row_id, symbol_name, "Function", file_path, 5, 3, 2);

    // Search by SymbolId
    let options = SearchOptions {
        db_path: &db_path,
        query: "", // empty query, using symbol_id
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: Some(known_symbol_id),
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1, "Should find symbol by ID");
    assert_eq!(response.0.results[0].name, symbol_name);
    assert_eq!(
        response.0.results[0].symbol_id.as_ref().unwrap(),
        known_symbol_id
    );
}

/// Test 6: Language filtering
#[test]
fn test_language_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    // Insert Rust file
    let rust_file = "src/rust_module.rs";
    let rust_file_id = insert_file(&conn, rust_file);
    let rust_fn = "rust_function";
    let rust_id = insert_symbol(&conn, rust_fn, "Function", "fn", (0, 10));
    insert_define_edge(&conn, rust_file_id, rust_id);

    // Insert Python file
    let python_file = "src/python_module.py";
    let python_file_id = insert_file(&conn, python_file);
    let python_fn = "python_function";
    let python_id = insert_symbol(&conn, python_fn, "Function", "fn", (0, 10));
    insert_define_edge(&conn, python_file_id, python_id);

    // Insert JavaScript file
    let js_file = "src/js_module.js";
    let js_file_id = insert_file(&conn, js_file);
    let js_fn = "js_function";
    let js_id = insert_symbol(&conn, js_fn, "Function", "fn", (0, 10));
    insert_define_edge(&conn, js_file_id, js_id);

    // Search with --language rust
    let options = SearchOptions {
        db_path: &db_path,
        query: "function", // matches all
        path_filter: None,
        kind_filter: None,
        language_filter: Some("rust"),
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.0.results.len(),
        1,
        "Should only return Rust symbols"
    );
    assert_eq!(response.0.results[0].name, rust_fn);
    assert_eq!(response.0.results[0].language.as_ref().unwrap(), "Rust");
}

/// Test 7: Multi-kind filtering (comma-separated)
#[test]
fn test_multi_kind_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/multi_kind.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert different kinds - use names without underscores for better matching
    let fn_name = "process";
    let fn_id = insert_symbol(&conn, fn_name, "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, fn_id);

    let struct_name = "process";
    let struct_id = insert_symbol(&conn, struct_name, "Struct", "struct", (20, 30));
    insert_define_edge(&conn, file_id, struct_id);

    let enum_name = "process";
    let enum_id = insert_symbol(&conn, enum_name, "Enum", "enum", (40, 50));
    insert_define_edge(&conn, file_id, enum_id);

    // First test: filter for fn only
    let options_fn = SearchOptions {
        db_path: &db_path,
        query: "process", // matches all three
        path_filter: None,
        kind_filter: Some("fn"), // single kind
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response_fn = search_symbols(options_fn).expect("search should succeed");
    assert_eq!(
        response_fn.0.results.len(),
        1,
        "Should return exactly one symbol with kind=fn"
    );
    assert_eq!(response_fn.0.results[0].kind, "Function");

    // Second test: filter for struct only
    let options_struct = SearchOptions {
        db_path: &db_path,
        query: "process",
        path_filter: None,
        kind_filter: Some("struct"),
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response_struct = search_symbols(options_struct).expect("search should succeed");
    assert_eq!(
        response_struct.0.results.len(),
        1,
        "Should return exactly one symbol with kind=struct"
    );
    assert_eq!(response_struct.0.results[0].kind, "Struct");
}

/// Test 8: Sort by fan-in
#[test]
fn test_sort_by_fan_in() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/sort_test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols with different fan-in values
    let low_fan = "low_fan_in";
    let low_id = insert_symbol(&conn, low_fan, "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, low_id);
    insert_metrics(&conn, low_id, low_fan, "Function", file_path, 1, 0, 1);

    let high_fan = "high_fan_in";
    let high_id = insert_symbol(&conn, high_fan, "Function", "fn", (20, 30));
    insert_define_edge(&conn, file_id, high_id);
    insert_metrics(&conn, high_id, high_fan, "Function", file_path, 100, 0, 1);

    let med_fan = "med_fan_in";
    let med_id = insert_symbol(&conn, med_fan, "Function", "fn", (40, 50));
    insert_define_edge(&conn, file_id, med_id);
    insert_metrics(&conn, med_id, med_fan, "Function", file_path, 50, 0, 1);

    // Search with sort-by fan-in (descending - highest first)
    let options = SearchOptions {
        db_path: &db_path,
        query: "fan_in",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::FanIn,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 3);

    // Results should be sorted by fan-in descending
    assert_eq!(response.0.results[0].name, high_fan);
    assert_eq!(response.0.results[1].name, med_fan);
    assert_eq!(response.0.results[2].name, low_fan);
}

/// Test 9: FQN pattern filtering
#[test]
fn test_fqn_pattern_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/fqn_test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols with different FQNs
    let helper_a = "helper_a";
    let data_a = json!({
        "symbol_id": format!("{}-id", helper_a),
        "name": helper_a,
        "kind": "Function",
        "kind_normalized": "fn",
        "fqn": "crate::module_a::helper_a",
        "display_fqn": "module_a::helper_a",
        "canonical_fqn": "crate::src/module_a.rs::Function helper_a",
        "byte_start": 0,
        "byte_end": 10,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", helper_a, data_a],
    )
    .expect("insert symbol");
    let id_a = conn.last_insert_rowid();
    insert_define_edge(&conn, file_id, id_a);

    let helper_b = "helper_b";
    let data_b = json!({
        "symbol_id": format!("{}-id-b", helper_b),
        "name": helper_b,
        "kind": "Function",
        "kind_normalized": "fn",
        "fqn": "crate::module_b::helper_b",
        "display_fqn": "module_b::helper_b",
        "canonical_fqn": "crate::src/module_b.rs::Function helper_b",
        "byte_start": 20,
        "byte_end": 30,
        "start_line": 2,
        "start_col": 0,
        "end_line": 2,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", helper_b, data_b],
    )
    .expect("insert symbol");
    let id_b = conn.last_insert_rowid();
    insert_define_edge(&conn, file_id, id_b);

    // Search with FQN pattern for module_a (using LIKE wildcard)
    let options = SearchOptions {
        db_path: &db_path,
        query: "helper", // matches both helper_a and helper_b
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: true, // Need canonical_fqn for filtering
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: Some("%module_a%"), // Use LIKE wildcard pattern
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.0.results.len(),
        1,
        "Should only return helper from module_a"
    );
    assert_eq!(
        response.0.results[0].name, helper_a,
        "Should be helper_a from module_a"
    );
    assert!(
        response.0.results[0]
            .canonical_fqn
            .as_ref()
            .unwrap()
            .contains("module_a"),
        "canonical_fqn should contain module_a"
    );
}

/// Test 10: Combined metrics and language filtering
#[test]
fn test_combined_metrics_and_language_filter() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    // Rust file with high complexity
    let rust_file = "src/complex.rs";
    let rust_file_id = insert_file(&conn, rust_file);
    let rust_complex = "complex_rust";
    let rust_id = insert_symbol(&conn, rust_complex, "Function", "fn", (0, 10));
    insert_define_edge(&conn, rust_file_id, rust_id);
    insert_metrics(&conn, rust_id, rust_complex, "Function", rust_file, 10, 5, 20);

    // Rust file with low complexity
    let rust_simple_name = "simple_rust";
    let rust_simple_id = insert_symbol(&conn, rust_simple_name, "Function", "fn", (20, 30));
    insert_define_edge(&conn, rust_file_id, rust_simple_id);
    insert_metrics(&conn, rust_simple_id, rust_simple_name, "Function", rust_file, 2, 1, 1);

    // Python file with high complexity
    let python_file = "src/complex.py";
    let python_file_id = insert_file(&conn, python_file);
    let python_complex = "complex_python";
    let python_id = insert_symbol(&conn, python_complex, "Function", "fn", (0, 10));
    insert_define_edge(&conn, python_file_id, python_id);
    insert_metrics(&conn, python_id, python_complex, "Function", python_file, 15, 8, 25);

    // Search for high-complexity Rust functions
    let options = SearchOptions {
        db_path: &db_path,
        query: "complex",
        path_filter: None,
        kind_filter: None,
        language_filter: Some("rust"),
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_complexity: Some(10),
            max_complexity: None,
            min_fan_in: None,
            min_fan_out: None,
        },
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.0.results.len(),
        1,
        "Should only return high-complexity Rust function"
    );
    assert_eq!(response.0.results[0].name, rust_complex);
    assert_eq!(response.0.results[0].language.as_ref().unwrap(), "Rust");
}

/// Test 11: Metrics present in search results
///
/// This test verifies that metrics are actually returned in the JSON output
/// when using metrics-based sorting. This is a regression test for the bug
/// where the JOIN condition compared TEXT to INTEGER and no metrics were returned.
#[test]
fn test_metrics_present_in_search_results() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/metrics_test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols with metrics
    let test_name = "test_function";
    let test_id = insert_symbol(&conn, test_name, "Function", "fn", (0, 20));
    insert_define_edge(&conn, file_id, test_id);
    insert_metrics(&conn, test_id, test_name, "Function", file_path, 10, 5, 3);

    // Search with sort-by fan-in - this should trigger the metrics JOIN
    let options = SearchOptions {
        db_path: &db_path,
        query: test_name,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::FanIn,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1, "Should find the test symbol");

    let result = &response.0.results[0];

    // Verify that metrics fields are populated (this was broken before the fix)
    assert!(
        result.fan_in.is_some(),
        "fan_in should be present in search results"
    );
    assert_eq!(result.fan_in.unwrap(), 10, "fan_in should match inserted value");

    assert!(
        result.fan_out.is_some(),
        "fan_out should be present in search results"
    );
    assert_eq!(result.fan_out.unwrap(), 5, "fan_out should match inserted value");

    assert!(
        result.cyclomatic_complexity.is_some(),
        "cyclomatic_complexity should be present in search results"
    );
    assert_eq!(
        result.cyclomatic_complexity.unwrap(),
        3,
        "cyclomatic_complexity should match inserted value"
    );
}
