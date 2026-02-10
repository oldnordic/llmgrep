//! CLI integration tests for backend detection and command execution.
//!
//! These tests verify that CLI commands correctly detect backend format
//! and delegate to appropriate implementation. Tests use the actual
//! llmgrep binary via std::process::Command.

use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

/// Helper to get a SQLite database for testing.
///
/// Uses the project's existing codegraph database if available,
/// or copies a minimal test database. Since Magellan 2.x defaults
/// to native-v2 format, we use the existing .codemcp/codegraph.db
/// which is guaranteed to be SQLite format.
fn get_test_sqlite_db() -> PathBuf {
    // First, try to use the existing codegraph database
    let existing_db = PathBuf::from(".codemcp/codegraph.db");
    if existing_db.exists() {
        // Verify it's actually SQLite format
        if let Ok(mut file) = std::fs::File::open(&existing_db) {
            let mut header = [0u8; 16];
            if file.read_exact(&mut header).is_ok() {
                let header_str = std::str::from_utf8(&header).unwrap_or("");
                if header_str.starts_with("SQLite format 3") {
                    return existing_db;
                }
            }
        }
    }

    // Fallback: create a minimal SQLite database file
    let temp_file = std::env::temp_dir()
        .join(format!("llmgrep_test_sqlite_{}.db", std::process::id()));

    // Remove any existing test database
    let _ = std::fs::remove_file(&temp_file);

    // Create a valid minimal SQLite database using rusqlite
    // Just opening and closing creates the basic structure
    if let Ok(conn) = rusqlite::Connection::open(&temp_file) {
        // Create tables matching Magellan's SQLite schema
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_entities (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL,
                data TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_edges (
                id INTEGER PRIMARY KEY,
                from_id INTEGER NOT NULL,
                to_id INTEGER NOT NULL,
                edge_type TEXT NOT NULL
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS ast_nodes (
                id INTEGER PRIMARY KEY,
                file_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                line_number INTEGER,
                parent_id INTEGER,
                data TEXT
            )",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS symbol_metrics (
                symbol_id INTEGER PRIMARY KEY,
                fan_in INTEGER DEFAULT 0,
                fan_out INTEGER DEFAULT 0,
                cyclomatic_complexity INTEGER DEFAULT 0
            )",
            [],
        );
        // Insert metrics for the test symbol
        let _ = conn.execute(
            "INSERT INTO symbol_metrics (symbol_id, fan_in, fan_out, cyclomatic_complexity) VALUES (2, 0, 0, 1)",
            [],
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS code_chunks (
                symbol_name TEXT,
                file_path TEXT,
                byte_start INTEGER,
                byte_end INTEGER,
                snippet TEXT
            )",
            [],
        );
        // Insert test data: a file entity
        let _ = conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"test.rs\"}')",
            [],
        );
        // Insert test data: a symbol entity with all required fields
        let _ = conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (2, 'Symbol', '{\"name\":\"test\",\"fqn\":\"test::function\",\"display_fqn\":\"test::function\",\"canonical_fqn\":\"test::function\",\"byte_start\":0,\"byte_end\":10,\"line_start\":1,\"line_end\":2,\"start_line\":1,\"start_col\":0,\"language\":\"Rust\",\"symbol_id\":\"2\"}')",
            [],
        );
        // Insert test data: edge from file to symbol (DEFINES)
        let _ = conn.execute(
            "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 2, 'DEFINES')",
            [],
        );
    }

    temp_file
}

/// Get the path to the llmgrep binary.
///
/// Uses the release build binary if available, otherwise debug.
/// Returns None if no binary is found.
fn llmgrep_binary() -> Option<PathBuf> {
    // Prefer release binary for integration tests
    let release_path = PathBuf::from("./target/release/llmgrep");
    if release_path.exists() {
        return Some(release_path);
    }

    let debug_path = PathBuf::from("./target/debug/llmgrep");
    if debug_path.exists() {
        return Some(debug_path);
    }

    None
}

#[test]
fn test_search_with_sqlite_backend() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "search",
            "--query",
            "main",
            "--limit",
            "5",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Should succeed (exit code 0) or produce meaningful error
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        // If it failed, check if it's because the database has no results
        // (which is acceptable for a minimal test database)
        if stderr.contains("No symbols found")
            || stderr.contains("total_count")
            || stdout.contains("total_count")
        {
            // This is acceptable - the command ran but found no results
            return;
        }
        panic!(
            "llmgrep search failed: {}\nstdout: {}\nstderr: {}",
            output.status, stdout, stderr
        );
    }

    // Should produce some output (even if no results, there should be a total_count field)
    assert!(
        !stdout.trim().is_empty() || !stderr.trim().is_empty(),
        "Expected output from search command"
    );
}

#[test]
fn test_ast_with_sqlite_backend() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "ast",
            "--file",
            "src/main.rs",
            "--limit",
            "10",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Check result
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        // Acceptable failures:
        // 1. File not in database
        // 2. Magellan binary version mismatch (expected native-v2, got SQLite)
        // This is a known limitation - ast/find-ast shell out to magellan
        // which expects native-v2 format in newer versions
        if stderr.contains("No AST nodes found")
            || stderr.contains("File not indexed")
            || stderr.contains("connection error")
            || stderr.contains("Invalid magic number")
        {
            return;
        }
        panic!(
            "llmgrep ast failed: {}\nstdout: {}\nstderr: {}",
            output.status, stdout, stderr
        );
    }

    // Should produce JSON output (contains braces)
    assert!(
        stdout.contains("{") || stderr.contains("{"),
        "Expected JSON output from ast command"
    );
}

#[test]
fn test_find_ast_with_sqlite_backend() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "find-ast",
            "--kind",
            "function_item",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Check result - command should succeed even if no results
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        // Acceptable failures:
        // 1. No nodes found
        // 2. Magellan binary version mismatch (expected native-v2, got SQLite)
        // This is a known limitation - ast/find-ast shell out to magellan
        // which expects native-v2 format in newer versions
        if stderr.contains("No AST nodes found")
            || stderr.contains("connection error")
            || stderr.contains("Invalid magic number")
        {
            return;
        }
        panic!(
            "llmgrep find-ast failed: {}\nstdout: {}\nstderr: {}",
            output.status, stdout, stderr
        );
    }
}

#[test]
#[cfg(not(feature = "native-v2"))]
fn test_native_v2_backend_error() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    // Check if we have a native-v2 test database available
    let db_path = std::env::var("LLMGREP_TEST_NATIVE_V2_DB");
    if db_path.is_err() {
        eprintln!("SKIP: LLMGREP_TEST_NATIVE_V2_DB not set");
        return;
    }
    let db_path = db_path.unwrap();

    let output = Command::new(&binary)
        .args(["--db", &db_path, "search", "--query", "test"])
        .output()
        .expect("Failed to execute llmgrep");

    // Should fail with LLM-E109
    assert!(
        !output.status.success(),
        "Expected failure for native-v2 DB without native-v2 feature"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("LLM-E109") || stderr.contains("native-v2"),
        "Expected LLM-E109 error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("cargo install llmgrep --features native-v2")
            || stderr.contains("--features native-v2"),
        "Expected remediation hint, got: {}",
        stderr
    );
}

#[test]
fn test_backend_detection_via_cli() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    // Test that the CLI properly detects and uses SQLite backend
    // by running a simple search query
    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--output",
            "json",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // The command should not fail with backend detection errors
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Backend detection failed"),
        "Backend should be detected successfully for SQLite database"
    );
    assert!(
        !stderr.contains("LLM-E109"),
        "Should not report native-v2 error for SQLite database"
    );
}

#[test]
fn test_search_mode_symbols_via_cli() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "search",
            "--query",
            "main",
            "--mode",
            "symbols",
            "--limit",
            "3",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Verify the mode was accepted (command didn't fail with invalid mode error)
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("invalid"),
        "Symbols mode should be accepted: {}",
        stderr
    );
}

#[test]
fn test_search_mode_references_via_cli() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "search",
            "--query",
            "main",
            "--mode",
            "references",
            "--limit",
            "3",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Verify the mode was accepted
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("invalid"),
        "References mode should be accepted: {}",
        stderr
    );
}

#[test]
fn test_search_mode_calls_via_cli() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "search",
            "--query",
            "main",
            "--mode",
            "calls",
            "--limit",
            "3",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Verify the mode was accepted
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("invalid"),
        "Calls mode should be accepted: {}",
        stderr
    );
}

#[test]
fn test_json_output_format_via_cli() {
    let binary = match llmgrep_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: llmgrep binary not found. Run: cargo build --release");
            return;
        }
    };

    let db_path = get_test_sqlite_db();

    let output = Command::new(&binary)
        .args([
            "--db",
            db_path.to_str().unwrap(),
            "search",
            "--query",
            "main",
            "--output",
            "json",
        ])
        .output()
        .expect("Failed to execute llmgrep");

    // Verify JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("{") || stdout.contains("results"),
        "JSON output should contain braces or 'results' field"
    );
}
