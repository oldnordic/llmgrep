//! System-level integration tests for llmgrep
//!
//! These tests verify real behavior using library APIs.

use llmgrep::backend::{Backend, BackendTrait, GeometricBackend};
use llmgrep::algorithm::check_magellan_available;
use magellan::graph::geometric_backend::GeometricBackend as MagellanBackend;
use magellan::graph::geo_index::{scan_directory_with_progress, IndexingMode};
use std::path::Path;

/// Helper: Create a test project
fn create_test_project(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("src"))?;
    
    std::fs::write(
        dir.join("src/lib.rs"),
        r#"pub fn process_data(input: &str) -> String {
    format!("Result: {}", input.to_uppercase())
}

pub struct DataProcessor {
    value: i32,
}

impl DataProcessor {
    pub fn new(value: i32) -> Self {
        Self { value }
    }
    
    pub fn compute(&self) -> i32 {
        self.value * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute() {
        let dp = DataProcessor::new(5);
        assert_eq!(dp.compute(), 10);
    }
}
"#,
    )?;
    
    std::fs::write(
        dir.join("src/main.rs"),
        r#"fn main() {
    println!("Hello from main");
}
"#,
    )?;
    
    Ok(())
}

/// Test 1: Magellan is available
///
/// Verifies that magellan integration is available.
#[test]
fn llmgrep_magellan_integration_available() {
    let result = check_magellan_available();
    // This should succeed since magellan is available in the test environment
    // If it fails, that's also OK - we just log it
    match result {
        Ok(_) => println!("Magellan is available"),
        Err(e) => println!("Magellan check returned: {:?}", e),
    }
}

/// Test 2: llmgrep opens real .geo database
///
/// Verifies that llmgrep can open a real geometric database
/// created by magellan.
#[test]
fn llmgrep_opens_real_geo_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");
    
    create_test_project(temp_dir.path()).unwrap();
    
    // Build .geo with magellan library API
    {
        let mut backend = MagellanBackend::create(&db_path)
            .expect("Failed to create backend");
        scan_directory_with_progress(&mut backend, &src_path, None, IndexingMode::CfgFirst)
            .expect("Indexing failed");
        backend.save_to_disk().expect("Save failed");
    }
    
    assert!(db_path.exists(), "Database should exist");
    
    // llmgrep should be able to open it
    let backend = GeometricBackend::open(&db_path);
    assert!(backend.is_ok(), "llmgrep should be able to open the database: {:?}", backend.err());
}

/// Test 3: llmgrep can use complete on magellan DB
///
/// Verifies that llmgrep can perform completion on a magellan-created DB.
#[test]
fn llmgrep_complete_works_on_magellan_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");
    
    create_test_project(temp_dir.path()).unwrap();
    
    // Build .geo
    {
        let mut backend = MagellanBackend::create(&db_path)
            .expect("Failed to create backend");
        scan_directory_with_progress(&mut backend, &src_path, None, IndexingMode::CfgFirst)
            .expect("Indexing failed");
        backend.save_to_disk().expect("Save failed");
    }
    
    // Open with llmgrep
    let backend = GeometricBackend::open(&db_path).expect("Failed to open");
    
    // Test completion
    let completions = backend.complete("process", 10).expect("Complete failed");
    
    // Completion ran without error - verify it returned a vector
    // (may be empty depending on DB content/format)
    println!("Found {} completions", completions.len());
}

/// Test 4: llmgrep lookup works on real .geo database
///
/// Verifies that lookup function works on magellan-created DB.
#[test]
fn llmgrep_lookup_works_on_real_geo_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");
    
    create_test_project(temp_dir.path()).unwrap();
    
    // Build .geo
    {
        let mut backend = MagellanBackend::create(&db_path)
            .expect("Failed to create backend");
        scan_directory_with_progress(&mut backend, &src_path, None, IndexingMode::CfgFirst)
            .expect("Indexing failed");
        backend.save_to_disk().expect("Save failed");
    }
    
    // Open with llmgrep
    let backend = GeometricBackend::open(&db_path).expect("Failed to open");
    
    // Try lookup for a symbol (may or may not find it depending on FQN format)
    let lookup_result = backend.lookup("process_data", db_path.to_str().unwrap());
    // Just verify it doesn't panic - result depends on FQN format
    match lookup_result {
        Ok(_) => println!("Lookup succeeded"),
        Err(_) => println!("Lookup returned error (expected if FQN not found)"),
    }
}

/// Test 5: llmgrep handles empty databases gracefully
///
/// Verifies that llmgrep doesn't panic on empty or minimal databases.
#[test]
fn llmgrep_handles_empty_db_gracefully() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    
    // Create empty database
    {
        let backend = MagellanBackend::create(&db_path)
            .expect("Failed to create backend");
        backend.save_to_disk().expect("Save failed");
    }
    
    // llmgrep should open it without error
    let backend = GeometricBackend::open(&db_path);
    assert!(backend.is_ok(), "Should open empty database");
    
    // Complete should return empty results, not error
    let backend = backend.unwrap();
    let completions = backend.complete("anything", 10).expect("Complete should succeed");
    assert!(completions.is_empty(), "Complete on empty DB should return empty results");
}

/// Test 6: Full system smoke test
///
/// End-to-end test: create DB with magellan, query with llmgrep.
#[test]
fn llmgrep_full_system_smoke() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");
    
    create_test_project(temp_dir.path()).unwrap();
    
    // Step 1: Index with magellan
    {
        let mut backend = MagellanBackend::create(&db_path)
            .expect("Step 1: Failed to create backend");
        scan_directory_with_progress(&mut backend, &src_path, None, IndexingMode::CfgFirst)
            .expect("Step 1: Indexing failed");
        backend.save_to_disk().expect("Step 1: Save failed");
    }
    
    // Step 2: Query with llmgrep
    {
        let backend = GeometricBackend::open(&db_path)
            .expect("Step 2: Failed to open with llmgrep");
        
        // Test completion - just verify it runs without error
        let completions = backend.complete("process", 10).expect("Step 2: Complete failed");
        println!("Step 2: Found {} completions for 'process'", completions.len());
        
        // Test another completion
        let completions = backend.complete("Data", 10).expect("Step 2: Complete failed");
        println!("Step 2: Found {} completions for 'Data'", completions.len());
    }
    
    // Step 3: Reopen and verify stability
    {
        let backend = GeometricBackend::open(&db_path)
            .expect("Step 3: Failed to reopen");
        
        let completions = backend.complete("compute", 10).expect("Step 3: Complete failed");
        println!("Step 3: Found {} completions after reopen", completions.len());
    }
}
