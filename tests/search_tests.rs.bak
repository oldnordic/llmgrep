use llmgrep::query::{search_calls, search_references, search_symbols, SearchOptions, ContextOptions, SnippetOptions, FqnOptions, MetricsOptions};
use rusqlite::{params, Connection};
use serde_json::json;
use std::path::PathBuf;

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

fn insert_symbol(conn: &Connection, name: &str, kind: &str, kind_normalized: &str, span: (u64, u64)) -> i64 {
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

fn insert_reference(conn: &Connection, file: &str, referenced_symbol: &str, span: (u64, u64)) -> i64 {
    let data = json!({
        "file": file,
        "byte_start": span.0,
        "byte_end": span.1,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    let name = format!("ref to {}", referenced_symbol);
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Reference", name, data],
    )
    .expect("insert reference");
    conn.last_insert_rowid()
}

fn insert_reference_edge(conn: &Connection, reference_id: i64, symbol_id: i64) {
    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (?1, ?2, ?3)",
        params![reference_id, symbol_id, "REFERENCES"],
    )
    .expect("insert reference edge");
}

fn insert_call(
    conn: &Connection,
    file: &str,
    caller: &str,
    callee: &str,
    span: (u64, u64),
) -> i64 {
    let data = json!({
        "file": file,
        "caller": caller,
        "callee": callee,
        "caller_symbol_id": format!("{}-id", caller),
        "callee_symbol_id": format!("{}-id", callee),
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
        params!["Call", "call", data],
    )
    .expect("insert call");
    conn.last_insert_rowid()
}

#[test]
fn test_search_symbols_with_path_filter() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/main.rs");
    let symbol_id = insert_symbol(&conn, "main", "Function", "fn", (0, 4));
    insert_define_edge(&conn, file_id, symbol_id);

    let other_file_id = insert_file(&conn, "tests/main.rs");
    let other_symbol_id = insert_symbol(&conn, "main", "Function", "fn", (10, 14));
    insert_define_edge(&conn, other_file_id, other_symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "main",
        path_filter: Some(&PathBuf::from("src/")),
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.total_count, 1);
    assert_eq!(response.0.results[0].span.file_path, "src/main.rs");
}

#[test]
fn test_search_symbols_with_kind_filter() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");
    let fn_symbol_id = insert_symbol(&conn, "thing", "Function", "fn", (0, 5));
    insert_define_edge(&conn, file_id, fn_symbol_id);

    let type_symbol_id = insert_symbol(&conn, "thing", "TypeAlias", "type", (10, 15));
    insert_define_edge(&conn, file_id, type_symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "thing",
        path_filter: None,
        kind_filter: Some("fn"),
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].kind, "Function");
}

#[test]
fn test_search_symbols_rank_exact_match_first() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");
    let exact_id = insert_symbol(&conn, "alpha", "Function", "fn", (0, 5));
    insert_define_edge(&conn, file_id, exact_id);

    let prefix_id = insert_symbol(&conn, "alphabet", "Function", "fn", (10, 18));
    insert_define_edge(&conn, file_id, prefix_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "alpha",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    assert_eq!(response.0.results[0].name, "alpha");
}

#[test]
fn test_search_symbols_regex_filters() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/main.rs");
    let main_id = insert_symbol(&conn, "main", "Function", "fn", (0, 4));
    insert_define_edge(&conn, file_id, main_id);

    let other_id = insert_symbol(&conn, "mainly", "Function", "fn", (10, 16));
    insert_define_edge(&conn, file_id, other_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "^main$",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: true,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].name, "main");
}

#[test]
fn test_search_symbols_with_context_and_snippet() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = temp_dir.path().join("sample.rs");
    std::fs::write(&file_path, "fn hello() {}\nfn world() {}\n").expect("write file");
    let file_path_str = file_path.to_string_lossy().to_string();

    let file_id = insert_file(&conn, &file_path_str);
    let symbol_id = insert_symbol(&conn, "hello", "Function", "fn", (3, 8));
    insert_define_edge(&conn, file_id, symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "hello",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: true,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: true,
            max_bytes: 200,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    let result = &response.0.results[0];
    assert!(result.snippet.as_deref() == Some("hello"));
    let context = result.span.context.as_ref().expect("context");
    assert!(!context.truncated);
}

#[test]
fn test_search_symbols_context_truncated_at_file_edges() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = temp_dir.path().join("sample.rs");
    std::fs::write(&file_path, "fn hello() {}\n").expect("write file");
    let file_path_str = file_path.to_string_lossy().to_string();

    let file_id = insert_file(&conn, &file_path_str);
    let symbol_id = insert_symbol(&conn, "hello", "Function", "fn", (3, 8));
    insert_define_edge(&conn, file_id, symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "hello",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: true,
            lines: 3,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: true,
            max_bytes: 200,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    let context = response.0.results[0]
        .span
        .context
        .as_ref()
        .expect("context");
    assert!(context.truncated);
}

#[test]
fn test_search_symbols_context_truncated_by_cap() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = temp_dir.path().join("sample.rs");
    std::fs::write(
        &file_path,
        "line1\nline2\nline3\nline4\nline5\nline6\n",
    )
    .expect("write file");
    let file_path_str = file_path.to_string_lossy().to_string();

    let file_id = insert_file(&conn, &file_path_str);
    let symbol_id = insert_symbol(&conn, "hello", "Function", "fn", (6, 11));
    insert_define_edge(&conn, file_id, symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "hello",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: true,
            lines: 5,
            max_lines: 1,
        },
        snippet: SnippetOptions {
            include: true,
            max_bytes: 200,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options).expect("search");

    let context = response.0.results[0]
        .span
        .context
        .as_ref()
        .expect("context");
    assert!(context.truncated);
}

#[test]
fn test_search_symbols_with_fqn_toggle() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");
    let symbol_id = insert_symbol(&conn, "hello", "Function", "fn", (3, 8));
    insert_define_edge(&conn, file_id, symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "hello",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options)
    .expect("search");
    let result = &response.0.results[0];
    assert!(result.fqn.is_none());
    assert!(result.canonical_fqn.is_none());
    assert!(result.display_fqn.is_none());

    let options = SearchOptions {
        db_path: &db_path,
        query: "hello",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: true,
            display_fqn: true,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_symbols(options)
    .expect("search");
    let result = &response.0.results[0];
    assert!(result.fqn.is_some() || result.display_fqn.is_some() || result.canonical_fqn.is_some());
}

#[test]
fn test_search_references_basic() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");
    let symbol_id = insert_symbol(&conn, "target", "Function", "fn", (0, 6));
    insert_define_edge(&conn, file_id, symbol_id);

    let ref_id = insert_reference(&conn, "src/lib.rs", "target", (10, 16));
    insert_reference_edge(&conn, ref_id, symbol_id);

    let options = SearchOptions {
        db_path: &db_path,
        query: "target",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_references(options).expect("search");

    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].referenced_symbol, "target");
    assert!(response.0.results[0].target_symbol_id.is_some());
}

#[test]
fn test_search_calls_basic() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    insert_call(&conn, "src/lib.rs", "caller_fn", "callee_fn", (3, 12));

    let options = SearchOptions {
        db_path: &db_path,
        query: "caller_fn",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
    };
    let response = search_calls(options).expect("search");

    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].caller, "caller_fn");
    assert_eq!(response.0.results[0].callee, "callee_fn");
}

#[test]
fn test_combined_response_counts_match() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");
    let symbol_id = insert_symbol(&conn, "target", "Function", "fn", (0, 6));
    insert_define_edge(&conn, file_id, symbol_id);

    let ref_id = insert_reference(&conn, "src/lib.rs", "target", (10, 16));
    insert_reference_edge(&conn, ref_id, symbol_id);

    insert_call(&conn, "src/lib.rs", "caller_fn", "callee_fn", (3, 12));

    let (symbols, _) = {
        let options = SearchOptions {
            db_path: &db_path,
            query: "target",
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions {
                include: false,
                lines: 0,
                max_lines: 20,
            },
            snippet: SnippetOptions {
                include: false,
                max_bytes: 0,
            },
            fqn: FqnOptions {
                fqn: false,
                canonical_fqn: false,
                display_fqn: false,
            },
            include_score: true,
            sort_by: llmgrep::SortMode::default(),
            metrics: MetricsOptions::default(),
        };
        search_symbols(options).expect("symbols")
    };

    let (refs, _) = {
        let options = SearchOptions {
            db_path: &db_path,
            query: "target",
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions {
                include: false,
                lines: 0,
                max_lines: 20,
            },
            snippet: SnippetOptions {
                include: false,
                max_bytes: 0,
            },
            fqn: FqnOptions::default(),
            include_score: true,
            sort_by: llmgrep::SortMode::default(),
            metrics: MetricsOptions::default(),
        };
        search_references(options).expect("refs")
    };

    let (calls, _) = {
        let options = SearchOptions {
            db_path: &db_path,
            query: "caller_fn",
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions {
                include: false,
                lines: 0,
                max_lines: 20,
            },
            snippet: SnippetOptions {
                include: false,
                max_bytes: 0,
            },
            fqn: FqnOptions::default(),
            include_score: true,
            sort_by: llmgrep::SortMode::default(),
            metrics: MetricsOptions::default(),
        };
        search_calls(options).expect("calls")
    };

    let total = symbols.total_count + refs.total_count + calls.total_count;
    assert!(total >= 3);
}
