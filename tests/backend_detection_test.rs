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
    file.write_all(b"SQLite format 3\0").expect("failed to write SQLite header");
    file.sync_all().expect("failed to sync file");

    // Try to detect backend - should fail to open as valid SQLite DB
    // but backend format should be detected correctly
    let result = Backend::detect_and_open(path);

    // We expect DatabaseCorrupted or similar since it's not a real DB
    // But NOT NativeV3BackendNotSupported or BackendDetectionFailed
    match result {
        Ok(_) => {}
        Err(e) => {
            // Should NOT be these errors (which indicate detection failed)
            let error_msg = e.to_string();
            assert!(!error_msg.contains("Backend detection failed"),
                "Should not fail detection for SQLite header: {}", error_msg);
            assert!(!error_msg.contains("native-v3 support"),
                "Should not think SQLite is native-v3: {}", error_msg);
        }
    }
}

#[test]
fn test_detect_nonexistent_file() {
    let path = PathBuf::from("/nonexistent/database.db");

    let result = Backend::detect_and_open(&path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Backend detection failed") ||
            error_msg.contains("not found") ||
            error_msg.contains("No such file"),
            "Expected file not found error, got: {}", error_msg);
}

#[test]
fn test_detect_empty_file() {
    let temp = NamedTempFile::new().expect("failed to create temp file");
    let path = temp.path();

    let result = Backend::detect_and_open(path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    // Empty file is neither SQLite nor Native-V2
    assert!(error_msg.contains("Backend detection failed") ||
            error_msg.contains("Unknown format") ||
            error_msg.contains("not a valid"),
            "Expected format error for empty file, got: {}", error_msg);
}

#[cfg(feature = "native-v3")]
#[test]
fn test_detect_native_v3_backend() {
    use std::io::Write;

    // Create a temporary file with Native-V2 header
    let temp = NamedTempFile::new().expect("failed to create temp file");
    let path = temp.path();

    // Write Native-V3 magic bytes: "SQLTGF" padded to 16 bytes
    let mut file = std::fs::File::create(path).expect("failed to create file");
    let mut header = b"SQLTGF".to_vec();
    header.resize(16, 0); // Pad to 16 bytes for header detection
    file.write_all(&header).expect("failed to write test data");
    file.sync_all().expect("failed to sync file");

    let result = Backend::detect_and_open(path);

    // Should attempt to open as Native-V2 (will fail since not a valid DB)
    // But should NOT be detected as SQLite
    match result {
        Ok(_) => {}
        Err(e) => {
            let error_msg = e.to_string();
            // Should NOT try to open as SQLite
            assert!(!error_msg.contains("SQLite"),
                "Should not detect Native-V2 header as SQLite: {}", error_msg);
        }
    }
}

#[cfg(not(feature = "native-v3"))]
#[test]
fn test_native_v3_not_supported_error() {
    use std::io::Write;

    // Create a temporary file with Native-V3 header
    let temp = NamedTempFile::new().expect("failed to create temp file");
    let path = temp.path();

    // Write Native-V3 magic bytes: "SQLTGF" padded to 16 bytes
    let mut file = std::fs::File::create(path).expect("failed to create file");
    let mut header = b"SQLTGF".to_vec();
    header.resize(16, 0); // Pad to 16 bytes for header detection
    file.write_all(&header).expect("failed to write test data");
    file.sync_all().expect("failed to sync file");

    let result = Backend::detect_and_open(path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("native-v3 support") ||
            error_msg.contains("LLM-E109"),
            "Expected native-v3 not supported error, got: {}", error_msg);
    assert!(error_msg.contains("cargo install llmgrep --features native-v3") ||
            error_msg.contains("--features native-v3"),
            "Error should suggest rebuilding with native-v3 feature");
}
