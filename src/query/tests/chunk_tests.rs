use super::*;
use rusqlite::Connection;
use tempfile::NamedTempFile;

fn create_test_db_with_chunks() -> (NamedTempFile, Connection) {
    let db_file = NamedTempFile::new().expect("failed to create temp file");
    let conn = Connection::open(db_file.path()).expect("failed to open database");

    conn.execute(
        "CREATE TABLE code_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            byte_start INTEGER NOT NULL,
            byte_end INTEGER NOT NULL,
            content TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            symbol_name TEXT,
            symbol_kind TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )
    .expect("failed to execute SQL");

    let hash1 = "a0d2da8d1f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c";
    conn.execute(
        "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind, created_at) VALUES
            ('/test/file.rs', 100, 200, 'fn test_func() { }', ?, 'test_func', 'Function', 1700000000),
            ('/test/file.rs', 300, 400, 'struct TestStruct { }', 'b1e3eb9e2f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d', 'TestStruct', 'Struct', 1700000001),
            ('/test/other.rs', 500, 600, 'fn helper() { }', 'c2f4fc0f3g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e', 'helper', 'Function', 1700000002)",
        [hash1],
    ).expect("failed to execute SQL");

    (db_file, conn)
}

#[test]
fn test_search_chunks_by_symbol_name() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunks = search_chunks_by_symbol_name(&conn, "test_func")
        .expect("failed to search chunks by symbol name");
    assert_eq!(chunks.len(), 1, "Should find 1 chunk for test_func");

    let chunk = &chunks[0];
    assert_eq!(chunk.file_path, "/test/file.rs");
    assert_eq!(chunk.byte_start, 100);
    assert_eq!(chunk.byte_end, 200);
    assert_eq!(chunk.content, "fn test_func() { }");
    assert_eq!(chunk.symbol_name, Some("test_func".to_string()));
    assert_eq!(chunk.symbol_kind, Some("Function".to_string()));
}

#[test]
fn test_search_chunks_by_symbol_name_not_found() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunks = search_chunks_by_symbol_name(&conn, "nonexistent")
        .expect("failed to search chunks by symbol name");
    assert_eq!(
        chunks.len(),
        0,
        "Should find 0 chunks for non-existent symbol"
    );
}

#[test]
fn test_search_chunks_by_span() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunk = search_chunks_by_span(&conn, "/test/file.rs", 100, 200)
        .expect("failed to search chunks by span");
    assert!(chunk.is_some(), "Should find chunk for exact span");

    let chunk = chunk.expect("chunk should be Some");
    assert_eq!(chunk.file_path, "/test/file.rs");
    assert_eq!(chunk.byte_start, 100);
    assert_eq!(chunk.byte_end, 200);
    assert_eq!(chunk.content, "fn test_func() { }");
    assert_eq!(chunk.symbol_name, Some("test_func".to_string()));
    assert_eq!(chunk.symbol_kind, Some("Function".to_string()));
}

#[test]
fn test_search_chunks_by_span_not_found() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunk = search_chunks_by_span(&conn, "/test/file.rs", 999, 1000)
        .expect("failed to search chunks by span");
    assert!(chunk.is_none(), "Should return None for non-existent span");

    let chunk = search_chunks_by_span(&conn, "/test/nonexistent.rs", 100, 200)
        .expect("failed to search chunks by span");
    assert!(chunk.is_none(), "Should return None for non-existent file");
}

#[test]
fn test_search_chunks_by_span_wrong_byte_range() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunk = search_chunks_by_span(&conn, "/test/file.rs", 101, 200)
        .expect("failed to search chunks by span");
    assert!(
        chunk.is_none(),
        "Should return None when byte_start doesn't match"
    );

    let chunk = search_chunks_by_span(&conn, "/test/file.rs", 100, 201)
        .expect("failed to search chunks by span");
    assert!(
        chunk.is_none(),
        "Should return None when byte_end doesn't match"
    );
}

#[test]
fn test_content_hash_format() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunks = search_chunks_by_symbol_name(&conn, "test_func")
        .expect("failed to search chunks by symbol name");
    assert_eq!(chunks.len(), 1);

    let hash = &chunks[0].content_hash;
    assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex characters");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should contain only hex characters"
    );
}

#[test]
fn test_symbol_kind_retrieval() {
    let (_db_file, conn) = create_test_db_with_chunks();

    let chunks = search_chunks_by_symbol_name(&conn, "test_func")
        .expect("failed to search chunks by symbol name");
    assert_eq!(chunks[0].symbol_kind, Some("Function".to_string()));

    let chunks = search_chunks_by_symbol_name(&conn, "TestStruct")
        .expect("failed to search chunks by symbol name");
    assert_eq!(chunks[0].symbol_kind, Some("Struct".to_string()));
}

#[test]
fn test_multiple_chunks_same_symbol() {
    let db_file = NamedTempFile::new().expect("failed to create temp file");
    let conn = Connection::open(db_file.path()).expect("failed to open database");

    conn.execute(
        "CREATE TABLE code_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            byte_start INTEGER NOT NULL,
            byte_end INTEGER NOT NULL,
            content TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            symbol_name TEXT,
            symbol_kind TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )
    .expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind, created_at) VALUES
            ('/test/file.rs', 100, 150, 'part1', 'hash1', 'my_symbol', 'Function', 1700000000),
            ('/test/file.rs', 150, 200, 'part2', 'hash2', 'my_symbol', 'Function', 1700000001)",
        [],
    ).expect("failed to execute SQL");

    let chunks = search_chunks_by_symbol_name(&conn, "my_symbol")
        .expect("failed to search chunks by symbol name");
    assert_eq!(chunks.len(), 2, "Should find 2 chunks for my_symbol");
}
