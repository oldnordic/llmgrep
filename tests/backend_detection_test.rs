//! Integration tests for backend format detection.

use llmgrep::backend::Backend;
use std::path::PathBuf;
use tempfile::NamedTempFile;

#[test]
fn test_detect_sqlite_backend() {
    // Create a temporary file with SQLite header
    let temp = NamedTempFile::new().expect("failed to create temp file");
    let path = temp.path();

    // Write SQLite header (first 16 bytes of a valid SQLite database)
    // SQLite magic: "SQLite format 3\0"
    use std::io::Write;
    let mut file = std::fs::File::create(path).expect("failed to create file");
    file.write_all(b"SQLite format 3\0")
        .expect("failed to write SQLite header");
    file.sync_all().expect("failed to sync file");

    // Try to detect backend - should fail to open as valid SQLite DB
    // but backend format should be detected correctly
    let result = Backend::detect_and_open(path);

    // We expect DatabaseCorrupted or similar since it's not a real DB
    // But NOT BackendDetectionFailed
    match result {
        Ok(_) => {}
        Err(e) => {
            // Should NOT be these errors (which indicate detection failed)
            let error_msg = e.to_string();
            assert!(
                !error_msg.contains("Backend detection failed"),
                "Should not fail detection for SQLite header: {}",
                error_msg
            );
        }
    }
}

#[test]
fn test_detect_nonexistent_file() {
    let path = PathBuf::from("/nonexistent/database.db");

    let result = Backend::detect_and_open(&path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Backend detection failed")
            || error_msg.contains("not found")
            || error_msg.contains("No such file"),
        "Expected file not found error, got: {}",
        error_msg
    );
}

#[test]
fn test_detect_empty_file() {
    let temp = NamedTempFile::new().expect("failed to create temp file");
    let path = temp.path();

    let result = Backend::detect_and_open(path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    // Empty file is neither SQLite nor a known format
    assert!(
        error_msg.contains("Backend detection failed")
            || error_msg.contains("Unknown format")
            || error_msg.contains("not a valid"),
        "Expected format error for empty file, got: {}",
        error_msg
    );
}
