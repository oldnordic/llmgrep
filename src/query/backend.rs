//! Database backend format detection.
//!
//! This module provides detection for different database backend formats.

use std::path::Path;

/// Database backend format detection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackendFormat {
    /// SQLite database format
    Sqlite,
    /// Native V3 binary format
    NativeV3,
    /// Unknown or invalid format
    Unknown,
}

/// Detect the backend format of a database file by checking magic bytes
///
/// # Arguments
/// * `path` - Path to the database file
///
/// # Returns
/// `BackendFormat` indicating the detected format
pub fn detect_backend_format(path: &Path) -> BackendFormat {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return BackendFormat::Unknown,
    };
    
    let mut header = [0u8; 16];
    match file.read_exact(&mut header) {
        Ok(_) => {}
        Err(_) => return BackendFormat::Unknown,
    }
    
    // Check for SQLite magic bytes: "SQLite format 3\0"
    if header[0..16] == *b"SQLite format 3\0" {
        return BackendFormat::Sqlite;
    }
    
    // Check for V3 native format magic: "SQLTGF"
    if header[0..6] == *b"SQLTGF" {
        return BackendFormat::NativeV3;
    }
    
    BackendFormat::Unknown
}
